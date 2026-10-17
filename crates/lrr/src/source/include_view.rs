//! Validated included source location and immutable input leases; no source capture effect.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        poll::{InputRequest, QueryPoll},
    },
    expand::identity::ExpansionKey,
    source::{include_paths::IncludeLocation, snapshot::SourceBlob},
    syntax::token_wire::{Key, TokenForest},
};
use std::sync::Arc;
#[derive(Debug, Clone)]
pub(crate) struct ViewDependency {
    pub key: InputKey,
    pub fingerprint: Digest,
    pub request: InputRequest,
}
#[derive(Debug)]
pub struct IncludedSourceView {
    identity: Digest,
    estimated_bytes: usize,
    producer: ExpansionKey,
    origin: Key,
    location: IncludeLocation,
    original: std::path::PathBuf,
    blob: SourceBlob,
    tokens: TokenForest,
    dependencies: Vec<ViewDependency>,
    diagnostic_origin: Arc<crate::syntax::origin::MacroOrigin>,
    _source_lease: Arc<InputValue>,
    _expansion_lease: Arc<InputValue>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INCLUDE-SNAPSHOT", message.into())
}
impl IncludedSourceView {
    pub(crate) fn new(
        producer: ExpansionKey,
        tokens: TokenForest,
        source: Arc<InputValue>,
        expansion: Arc<InputValue>,
        dependencies: Vec<ViewDependency>,
        diagnostic_origin: Arc<crate::syntax::origin::MacroOrigin>,
    ) -> Result<Self, Diagnostic> {
        let InputValue::IncludeRecord { captured, .. } = source.as_ref() else {
            return Err(fault("included source view requires a capture record"));
        };
        let InputValue::ExpansionRecord(record) = expansion.as_ref() else {
            return Err(fault("included source view requires an expansion record"));
        };
        if captured.kind() != crate::source::include_paths::IncludeKind::Syntax
            || record.key() != &producer
            || dependencies.len() < 2
        {
            return Err(fault("included source view identity/kind mismatch"));
        }
        if diagnostic_origin.invocation_identity != producer.invocation {
            return Err(fault("included diagnostic producer differs"));
        }
        let mut bytes = tokens
            .allocated_bytes()
            .map_err(|e| fault(&e.to_string()))?
            .checked_add(std::mem::size_of::<Self>())
            .and_then(|n| n.checked_add(diagnostic_origin.heap_bytes()?))
            .and_then(|n| n.checked_add(captured.virtual_location().path.as_str().len()))
            .and_then(|n| n.checked_add(captured.original_path().as_os_str().len()))
            .and_then(|n| {
                n.checked_add(
                    dependencies
                        .capacity()
                        .checked_mul(std::mem::size_of::<ViewDependency>())?,
                )
            })
            .ok_or_else(|| fault("included source view metadata overflow"))?;
        for dependency in &dependencies {
            dependency.request.validate()?;
            let extra = match &dependency.request {
                InputRequest::Expand(r) => std::mem::size_of_val(r.as_ref())
                    .checked_add(crate::expand::record::key_heap_bytes(&r.key)),
                InputRequest::CaptureInclude { request, .. } => request.metadata_bytes(),
                InputRequest::CaptureModule { .. }
                | InputRequest::Discover { .. }
                | InputRequest::CaptureGeneratedSource(_)
                | InputRequest::CaptureCompileEnvironment { .. } => {
                    return Err(fault("included source view dependency kind mismatch"));
                }
            }
            .ok_or_else(|| fault("included source view request metadata overflow"))?;
            bytes = bytes
                .checked_add(extra)
                .ok_or_else(|| fault("included source view metadata limit"))?;
        }
        let mut identity = StableEncoder::new("lrr/v2/included-source-view");
        identity.record(&producer);
        identity.digest(record.output());
        identity.digest(captured.identity());
        identity.record(diagnostic_origin.as_ref());
        let origin = Key(captured.identity().0);
        let location = captured.virtual_location().clone();
        let original = captured.original_path().to_owned();
        let blob = captured.blob().clone();
        Ok(Self {
            identity: identity.finish(),
            estimated_bytes: bytes,
            producer,
            origin,
            location,
            original,
            blob,
            tokens,
            dependencies,
            diagnostic_origin,
            _source_lease: source,
            _expansion_lease: expansion,
        })
    }
    /// Owned source anchors from the validated include route, root to leaf.
    /// A compiler builtin has no invented user definition location.
    pub(crate) fn capture_diagnostic_origin(
        producer: &ExpansionKey,
        original: &TokenForest,
        site: Key,
        parent: Option<&Self>,
    ) -> Result<Arc<crate::syntax::origin::MacroOrigin>, Diagnostic> {
        use crate::syntax::{
            origin::{ExpansionOrigin, MacroOrigin, SourceOrigin},
            spans::SourceRange,
            token_wire::SourceIdentity,
        };
        let limit = || {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "include diagnostic origin limit".into(),
            )
        };
        let span = original
            .spans()
            .iter()
            .find(|span| span.key() == site)
            .ok_or_else(|| fault("include diagnostic call site absent"))?;
        let count = parent
            .map_or(Some(0), |parent| {
                parent.diagnostic_origin.parents.len().checked_add(1)
            })            .ok_or_else(limit)?;
        let mut parents = Vec::new();
        parents.try_reserve_exact(count).map_err(|_| limit())?;
        let mut bytes = std::mem::size_of::<MacroOrigin>()
            .checked_add(
                parents
                    .capacity()
                    .checked_mul(std::mem::size_of::<ExpansionOrigin>())
                    .ok_or_else(limit)?,
            )
            .ok_or_else(limit)?;
        if let Some(parent) = parent {
            for (expansion, identity, location) in parent
                .diagnostic_origin
                .parents
                .iter()
                .map(|p| (p.expansion, p.invocation_identity, p.invocation.as_ref()))
                .chain(std::iter::once((
                    parent.producer.fingerprint(),
                    Some(parent.producer.invocation),
                    parent.diagnostic_origin.invocation.as_ref(),
                )))
            {
                let invocation = location
                    .map(|at| at.copy_with_limit(usize::MAX))
                    .transpose()?;
                bytes = bytes
                    .checked_add(
                        invocation
                            .as_ref()
                            .map_or(0, |at| at.source.path.capacity()),
                    )
                    .ok_or_else(limit)?;
                parents.push(ExpansionOrigin {
                    expansion,
                    invocation_identity: identity,
                    invocation,
                });
            }
        }
        let mut path = String::new();
        path.try_reserve_exact(span.source.path.len())
            .map_err(|_| limit())?;
        path.push_str(&span.source.path);
        let invocation = SourceOrigin {
            source: SourceIdentity {
                unit: span.source.unit,
                content: span.source.content,
                path,
            },
            range: SourceRange::checked(span.start, span.end)?,
        };
        let origin = MacroOrigin {
            invocation_identity: producer.invocation,
            invocation: Some(invocation),
            definition: None,
            failure_kind: None,
            failure_span_key: None,
            failure_span: None,
            parents,
        };
        Ok(Arc::new(origin))
    }
    pub(crate) fn diagnostic_origin(&self) -> &Arc<crate::syntax::origin::MacroOrigin> {
        &self.diagnostic_origin
    }
    pub(crate) fn estimated_bytes(&self) -> usize {
        self.estimated_bytes
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn origin(&self) -> Key {
        self.origin
    }
    pub fn matches_authorities(
        &self,
        authorities: &crate::source::include_paths::IncludeAuthorities,
    ) -> bool {
        self.dependencies.iter().any(|d|matches!(&d.request,InputRequest::CaptureInclude{request,..} if authorities.matches_request(request)))
    }
    pub fn producer(&self) -> &ExpansionKey {
        &self.producer
    }
    pub fn location(&self) -> &IncludeLocation {
        &self.location
    }
    pub fn original_path(&self) -> &std::path::Path {
        &self.original
    }
    pub(crate) fn dependencies(&self) -> &[ViewDependency] {
        &self.dependencies
    }
    pub fn blob(&self) -> &SourceBlob {
        &self.blob
    }
    pub fn tokens(&self) -> &TokenForest {
        &self.tokens
    }
    pub fn caller_context(&self) -> Key {
        Key(self.producer.parent_context.0)
    }
    /// Read-only replay for a source admission consumer. Missing inputs are a
    /// preparation prerequisite here; this method cannot capture or expand.
    pub(crate) fn require_published(&self, db: &Database) -> Result<(), Diagnostic> {
        for dependency in &self.dependencies {
            if !matches!(db.published_input(&dependency.key), Some((_, fingerprint))
                if fingerprint == dependency.fingerprint)
            {
                return Err(fault("retained included source input is absent or changed"));
            }
        }
        Ok(())
    }
    pub(crate) fn require_record(
        &self,
        record: &crate::expand::record::ExpansionRecord,
    ) -> Result<(), Diagnostic> {
        if record.key() != &self.producer
            || !matches!(self._expansion_lease.as_ref(), InputValue::ExpansionRecord(actual)
                if actual.same_record(record))
        {
            return Err(fault("included source producer record differs"));
        }
        Ok(())
    }
    /// Re-observe every producing/capture input before using this retained virtual source.
    pub fn validate(&self, db: &mut Database) -> QueryPoll<()> {
        for dependency in &self.dependencies {
            let Some(input) = db.input(&dependency.key) else {
                return QueryPoll::NeedInput(dependency.request.clone());
            };
            if input.fingerprint() != dependency.fingerprint {
                return QueryPoll::Ready(Err(fault("retained included source input changed")));
            }
        }
        QueryPoll::Ready(Ok(()))
    }
}
