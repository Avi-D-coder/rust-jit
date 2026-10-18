//! Current closed module-name answers from the ordinary sealed namespace query.
//! Semantic output identity is separate from the current certificate evidence.
//! This is not a use-site hygiene, local-prefix, prelude or binding grant.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        key::{BucketKey, CurrentModuleNameKey, QueryKey, SealedNameKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::names::{NameLookup, owned_name},
    source::{
        changes::{PreparedSourceView, SourceSelection},
        manifest::Namespace,
    },
    syntax::outline::ScopeKey,
};
use std::sync::Arc;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MODULE-NAME", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "current module name observation budget".into(),
    )
}
fn add(bytes: usize, n: usize) -> Result<usize, Diagnostic> {
    bytes.checked_add(n).ok_or_else(limit)
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ModuleNameCompletion {
    /// A missing current scope is never an Absent name result.
    Unsealed,
    /// Retained incomplete evidence grants no closed name answer.
    Incomplete,
    Complete,
}
/// Produced only through current source/cfg selection and genuine SealedName
/// fetch. No mutable/public constructor accepts a lookup or completeness flag.
pub struct CurrentModuleNameObservation {
    source: SourceSelection,
    scope: ScopeKey,
    namespace: Namespace,
    edition: u16,
    name: String,
    completion: ModuleNameCompletion,
    certificate: Option<DependencyStamp>,
    // The dependency stamp retains the actual resolver proof. Keep only its
    // bounded semantic projection here, not an unrelated outer memo owner.
    answer: Option<NameLookup>,
    answer_dependency: Option<DependencyStamp>,
    request_identity: Digest,
    semantic_identity: Digest,
    bytes: usize,
}
impl std::fmt::Debug for CurrentModuleNameObservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CurrentModuleNameObservation")
            .field("source", self.source())
            .field("scope", &self.scope)
            .field("namespace", &self.namespace)
            .field("edition", &self.edition)
            .field("name", &self.name)
            .field("completion", &self.completion)
            .field("lookup", &self.lookup())
            .field("semantic_identity", &self.semantic_identity)
            .finish()
    }
}
impl CurrentModuleNameObservation {
    pub(crate) fn prepare(
        db: &mut Database,
        source: &PreparedSourceView,
        scope: &ScopeKey,
        namespace: Namespace,
        name: &str,
    ) -> QueryPoll<Arc<Self>> {
        // Selection (including absence) is a dependency before retaining an
        // answer. A generated inline module belongs to the genuine parent's
        // semantic namespace even though its physical outline lacks it.
        let certificate = match db.current_scope_certificate(source.selection(), scope, namespace) {
            Ok(certificate) => certificate,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut observation = match Self::source_receipt(
            db,
            source,
            scope,
            namespace,
            name,
            certificate.as_deref(),
        ) {
            Ok(observation) => observation,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if let Some(certificate) = certificate {
            let InputValue::DiscoverySeal(input) = certificate.as_ref() else {
                return QueryPoll::Ready(Err(fault(
                    "current module certificate input variant differs",
                )));
            };
            match input.scope_certificate(
                scope,
                namespace,
                &source.selection().outline.parse.profile,
            ) {
                Ok(Some(actual))
                    if actual.edition == observation.edition
                        && input.sources.contains(source.selection()) => {}
                Ok(_) => {
                    return QueryPoll::Ready(Err(fault(
                        "current module certificate owner/source/edition differs",
                    )));
                }
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
            observation.completion = ModuleNameCompletion::Incomplete;
            if input.seal.unresolved_frontiers() == 0 {
                let key = QueryKey::SealedName(Box::new(SealedNameKey {
                    seal: input.seal.key(),
                    bucket: BucketKey {
                        scope: scope.clone(),
                        namespace,
                        name: observation.name.clone(),
                    },
                }));
                let extra = match key
                    .heap_bytes()
                    .checked_mul(3)
                    .and_then(|n| n.checked_add(std::mem::size_of::<DependencyStamp>()))
                    .and_then(|n| observation.bytes.checked_add(n))
                {
                    Some(bytes) => bytes,
                    _ => return QueryPoll::Ready(Err(limit())),
                };
                let outcome = match db.fetch(&SyntaxProvider, key.clone()) {
                    QueryPoll::Ready(Ok(outcome)) => outcome,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                if !outcome.cacheable {
                    return QueryPoll::Ready(Err(fault("current module answer is not retained")));
                }
                let QueryKey::SealedName(expected) = &key else {
                    unreachable!("private sealed key")
                };
                let QueryValue::SealedName {
                    key: actual,
                    lookup,
                } = outcome.value.as_ref()
                else {
                    return QueryPoll::Ready(Err(fault(
                        "current module answer source bucket differs",
                    )));
                };
                if actual != expected {
                    return QueryPoll::Ready(Err(fault(
                        "current module answer source bucket differs",
                    )));
                }
                let lookup_bytes = match lookup.metadata_bytes() {
                    Ok(bytes) => bytes,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                if extra.checked_add(lookup_bytes).is_none() {
                    return QueryPoll::Ready(Err(limit()));
                }
                // One cold copy of the selected bucket answer. Cached query
                // validation never reconstructs the namespace or this payload.
                observation.answer = Some(lookup.clone());
                observation.answer_dependency = Some(DependencyStamp::Query {
                    key,
                    fingerprint: outcome.fingerprint,
                });
                observation.completion = ModuleNameCompletion::Complete;
            }
            let key = InputKey::DiscoverySeal(input.seal.key());
            // current_scope_certificate already recorded this actual input.
            // Retain its cached evidence stamp, not the whole source universe.
            let Some((_, fingerprint)) = db.published_input(&key) else {
                return QueryPoll::Ready(Err(fault("current module certificate disappeared")));
            };
            observation.certificate = Some(DependencyStamp::Input { key, fingerprint });
        }
        observation.bytes = match observation.measure_bytes() {
            Some(bytes) => bytes,
            _ => return QueryPoll::Ready(Err(limit())),
        };
        let mut request = StableEncoder::new("lrr/v1/current-module-name-request");
        observation.encode_request(&mut request);
        observation.request_identity = request.finish();
        observation.semantic_identity = observation.fingerprint();
        QueryPoll::Ready(Ok(Arc::new(observation)))
    }
    fn source_receipt(
        db: &mut Database,
        source: &PreparedSourceView,
        scope: &ScopeKey,
        namespace: Namespace,
        name: &str,
        certificate: Option<&InputValue>,
    ) -> Result<Self, Diagnostic> {
        // The opaque view supplies exact immutable source/outline identity.
        // A closed answer additionally requires genuine published discovery,
        // which already validates declaration syntax; do not rescan every
        // outline scope/error for each requested name here. Unsealed has no
        // name answer and grants no declaration or absence authority.
        let selected = source.selection();
        selected.outline.parse.profile.validate_source()?;
        let certified_scope = match certificate {
            Some(InputValue::DiscoverySeal(input)) if input.sources.contains(selected) => input
                .scope_certificate(scope, namespace, &selected.outline.parse.profile)?
                .is_some(),
            _ => false,
        };
        if scope.owner.is_some()
            || (!source.outline().scopes.contains_key(scope) && !certified_scope)
            || scope.crate_key != selected.outline.crate_key
            || source.outline().profile != selected.outline.parse.profile
        {
            return Err(fault(
                "module name request is outside actual selected source scope",
            ));
        }
        let source_bytes = selected
            .metadata_bytes()?
            .checked_sub(std::mem::size_of::<SourceSelection>())
            .ok_or_else(limit)?;
        let reserved = add(
            add(std::mem::size_of::<Self>(), source_bytes)?,
            scope.heap_bytes(),
        )?;
        add(reserved, name.len().checked_mul(4).ok_or_else(limit)?)?;
        let owning = db
            .input(&InputKey::CapturedSource(selected.location_identity()))
            .ok_or_else(|| fault("module name actual captured source input absent"))?;
        if !matches!(owning.as_ref(), InputValue::CapturedSource(current) if current == selected) {
            return Err(fault("module name captured source selection changed"));
        }
        let cfg = db
            .input(&InputKey::CrateCfg(selected.outline.crate_key))
            .ok_or_else(|| fault("module name owning cfg input absent"))?;
        if !matches!(cfg.as_ref(), InputValue::CrateCfg(actual) if actual.fingerprint() == selected.outline.cfg)
        {
            return Err(fault("module name actual source cfg differs"));
        }
        let edition = selected.outline.parse.profile.edition;
        let name = owned_name(name, edition)?.as_str().to_owned();
        let bytes = add(reserved, name.capacity())?;
        Ok(Self {
            source: selected.clone(),
            scope: scope.clone(),
            namespace,
            edition,
            name,
            completion: ModuleNameCompletion::Unsealed,
            certificate: None,
            answer: None,
            answer_dependency: None,
            request_identity: Digest::of_bytes(b"unpublished module request"),
            semantic_identity: Digest::of_bytes(b"unpublished module name"),
            bytes,
        })
    }
    pub fn source(&self) -> &SourceSelection {
        &self.source
    }
    pub fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub fn namespace(&self) -> Namespace {
        self.namespace
    }
    pub fn module_edition(&self) -> u16 {
        self.edition
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn completion(&self) -> ModuleNameCompletion {
        self.completion
    }
    pub fn lookup(&self) -> Option<&NameLookup> {
        self.answer.as_ref()
    }
    pub fn request_identity(&self) -> Digest {
        self.request_identity
    }
    pub fn semantic_identity(&self) -> Digest {
        self.semantic_identity
    }
    /// Inline and owned source/key/answer storage. Evidence stamps never retain
    /// an old input universe or another query allocation.
    pub fn exclusive_metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub fn matches_request(&self, key: &CurrentModuleNameKey) -> bool {
        self.source().outline == key.source
            && self.source().path.as_str() == key.path
            && self.scope == key.bucket.scope
            && self.namespace == key.bucket.namespace
            && self.name == key.bucket.name
            && self.edition == key.source.parse.profile.edition
    }
    pub(crate) fn certificate_dependency(&self) -> Option<&DependencyStamp> {
        self.certificate.as_ref()
    }
    pub(crate) fn answer_dependency(&self) -> Option<&DependencyStamp> {
        self.answer_dependency.as_ref()
    }
    fn measure_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(
                self.source
                    .metadata_bytes()
                    .ok()?
                    .checked_sub(std::mem::size_of::<SourceSelection>())?,
            )?
            .checked_add(self.scope.heap_bytes())?
            .checked_add(self.name.capacity())?
            .checked_add(self.answer.as_ref().map_or(Some(0), |answer| {
                answer
                    .metadata_bytes()
                    .ok()?
                    .checked_sub(std::mem::size_of::<NameLookup>())
            })?)?
            .checked_add(
                self.certificate
                    .as_ref()
                    .map_or(0, DependencyStamp::heap_bytes),
            )?
            .checked_add(
                self.answer_dependency
                    .as_ref()
                    .map_or(0, DependencyStamp::heap_bytes),
            )
    }
    fn encode_request(&self, e: &mut StableEncoder) {
        let source = self.source();
        e.string(source.path.as_str());
        e.record(&source.outline);
        e.record(&self.scope);
        e.u32(self.namespace.tag());
        e.u16(self.edition);
        e.string(&self.name);
    }
}
impl StableEncode for CurrentModuleNameObservation {
    const DOMAIN: &'static str = "lrr/v1/current-module-name-semantic-answer";
    fn encode_fields(&self, e: &mut StableEncoder) {
        // Explicit real request/source and semantic answer. Evidence dependencies
        // retain current seal/epoch/selector authority separately, never erased.
        e.digest(self.request_identity);
        e.u8(match self.completion {
            ModuleNameCompletion::Unsealed => 0,
            ModuleNameCompletion::Incomplete => 1,
            ModuleNameCompletion::Complete => 2,
        });
        e.option(&self.lookup(), |e, answer| e.record(*answer));
    }
}

#[cfg(test)]
#[path = "current_module_name_tests.rs"]
mod tests;
