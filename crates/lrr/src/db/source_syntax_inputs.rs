//! Coordinator publication and immutable result ownership for pure structural
//! preparation. Neither input nor result grants namespace or frame authority.
use super::*;
use crate::{
    core::digest::StableEncoder,
    db::{
        key::PreparedBodySyntaxKey, provider::SyntaxProvider, retained_syntax::RetainedSyntaxInput,
    },
    expand::{
        source_declarative::assembly::PreparedSourceSyntax,
        source_discovery::{CompletedSourceEnvironment, CompletedSourceSyntax},
    },
    source::changes::SourceSelection,
    syntax::parse::ParsedFile,
};
const ARC: usize = 2 * std::mem::size_of::<usize>();
fn invalid(message: &str) -> Diagnostic {
    fault("LRR-E-EXPANSION-INPUT", message)
}
fn size(a: usize, b: usize) -> Result<usize, Diagnostic> {
    a.checked_add(b)
        .ok_or_else(crate::runtime::arena::resource)
}
pub struct CompletedSourceSyntaxInput {
    plan: Arc<CompletedSourceSyntax>,
    publication: Digest,
    source_revision: u64,
    fingerprint: Digest,
}
impl std::fmt::Debug for CompletedSourceSyntaxInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletedSourceSyntaxInput")
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}
impl CompletedSourceSyntaxInput {
    pub(crate) fn plan(&self) -> &Arc<CompletedSourceSyntax> {
        &self.plan
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + ARC
    }
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        macro_rules! ready {
            ($x:expr) => {
                match $x {
                    QueryPoll::Ready(Ok(v)) => v,
                    QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                    QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
                }
            };
        }
        if db.source_revision != self.source_revision {
            return QueryPoll::Ready(Err(invalid(
                "body syntax input belongs to another source publication",
            )));
        }
        let actual = ready!(current(db, &self.plan, Some(self.publication)));
        if actual != self.publication {
            return QueryPoll::Ready(Err(invalid("body syntax completion publication changed")));
        }
        QueryPoll::Ready(Ok(()))
    }
}
/// Retaining this result retains the actual source plan and prepared tree.
/// No raw prepared Arc is exported: admission transfers a clone into the
/// existing accounted RetainedSyntaxInput ownership path.
pub struct PreparedBodySyntax {
    key: PreparedBodySyntaxKey,
    plan: Arc<CompletedSourceSyntax>,
    prepared: Arc<PreparedSourceSyntax>,
    input: Digest,
    source_revision: u64,
    fingerprint: Digest,
    bytes: usize,
}
impl std::fmt::Debug for PreparedBodySyntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedBodySyntax")
            .field("key", &self.key)
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}
impl PreparedBodySyntax {
    pub(crate) fn prepare(
        db: &mut Database,
        key: &PreparedBodySyntaxKey,
        input: &Arc<CompletedSourceSyntaxInput>,
    ) -> QueryPoll<Arc<Self>> {
        match input.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        }
        QueryPoll::Ready(Self::construct(db, key, input))
    }
    fn construct(
        db: &Database,
        key: &PreparedBodySyntaxKey,
        input: &Arc<CompletedSourceSyntaxInput>,
    ) -> Result<Arc<Self>, Diagnostic> {
        let source = key.selection()?;
        if input.plan.source() != &source
            || input.plan.epoch() != key.epoch
            || input.plan.fingerprint() != key.plan
            || db.source_revision != input.source_revision
        {
            return Err(invalid(
                "body syntax demand differs from actual retained plan",
            ));
        }
        let prepared = input.plan.prepare_body(key.definition)?;
        if prepared.source() != &source
            || prepared.epoch() != key.epoch
            || !prepared.permits_body(key.definition)
        {
            return Err(invalid(
                "body syntax result differs from actual retained demand",
            ));
        }
        let bytes = size(
            size(
                size(size(std::mem::size_of::<Self>(), key.heap_bytes())?, ARC)?,
                prepared.metadata_excluding_snapshot()?,
            )?,
            ParsedFile::snapshot_metadata_bytes(prepared.syntax())?,
        )?;
        let mut e = StableEncoder::new("lrr/v1/prepared-body-syntax-result");
        e.record(key);
        e.digest(input.fingerprint);
        e.digest(prepared.identity());
        Ok(Arc::new(Self {
            key: key.clone(),
            plan: Arc::clone(input.plan()),
            prepared,
            input: input.fingerprint,
            source_revision: input.source_revision,
            fingerprint: e.finish(),
            bytes,
        }))
    }
    pub(crate) fn key(&self) -> &PreparedBodySyntaxKey {
        &self.key
    }
    pub(crate) fn plan(&self) -> &Arc<CompletedSourceSyntax> {
        &self.plan
    }
    pub(crate) fn syntax(&self) -> &PreparedSourceSyntax {
        &self.prepared
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    /// Excludes the separately retained plan; includes prepared snapshot/tree.
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn admit(&self, db: &mut Database) -> QueryPoll<Arc<RetainedSyntaxInput>> {
        if db.source_revision != self.source_revision
            || !matches!(db.published_input(&InputKey::CompletedSourceSyntax(self.key.plan)), Some((InputValue::CompletedSourceSyntax(actual), _)) if actual.fingerprint() == self.input)
        {
            return QueryPoll::Ready(Err(invalid(
                "prepared body syntax input is no longer published",
            )));
        }
        RetainedSyntaxInput::prepare(db, Arc::clone(&self.prepared), self.plan.seal().clone())
    }
}
fn current(
    db: &mut Database,
    plan: &Arc<CompletedSourceSyntax>,
    expected: Option<Digest>,
) -> QueryPoll<Digest> {
    macro_rules! ready {
        ($x:expr) => {
            match $x {
                QueryPoll::Ready(Ok(v)) => v,
                QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
            }
        };
    }
    if !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(), Some(InputValue::ExpansionEpoch(epoch)) if *epoch == plan.epoch())
        || !matches!(db.input(&InputKey::CapturedSource(plan.source().location_identity())).as_deref(), Some(InputValue::CapturedSource(actual)) if actual == plan.source())
    {
        return QueryPoll::Ready(Err(invalid("body syntax source/epoch publication differs")));
    }
    let seal = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::DiscoverySeal(plan.seal().clone())
    ));
    if seal.fingerprint != plan.seal_value()
        || !matches!(seal.value.as_ref(), QueryValue::DiscoverySeal(actual) if actual.key() == *plan.seal())
    {
        return QueryPoll::Ready(Err(invalid("body syntax actual discovery result differs")));
    }
    let Some(certificate) = db.input(&InputKey::DiscoverySeal(plan.seal().clone())) else {
        return QueryPoll::Ready(Err(invalid("body syntax actual discovery input absent")));
    };
    let InputValue::DiscoverySeal(actual) = certificate.as_ref() else {
        return QueryPoll::Ready(Err(invalid("body syntax discovery input variant differs")));
    };
    let publication = certificate.fingerprint();
    if let Some(expected) = expected {
        // The opaque input already certified source membership and the real
        // captured lease against this exact publication, once at registration.
        return if publication == expected {
            QueryPoll::Ready(Ok(publication))
        } else {
            QueryPoll::Ready(Err(invalid("body syntax completion publication changed")))
        };
    }
    if !actual.sources.iter().any(|source| source == plan.source()) {
        return QueryPoll::Ready(Err(invalid(
            "body syntax source absent from actual discovery",
        )));
    }
    match db.prepared_source(plan.source()) {
        Ok(Some(_)) => QueryPoll::Ready(Ok(certificate.fingerprint())),
        Ok(None) => QueryPoll::Ready(Err(invalid(
            "body syntax actual captured source lease absent",
        ))),
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
impl Database {
    /// Register only the real plan obtained from an opaque completed root.
    /// This is a coordinator boundary, never a query-provider side effect.
    pub(crate) fn publish_completed_source_syntax(
        &mut self,
        completed: &Arc<CompletedSourceEnvironment>,
        source: &SourceSelection,
        definition: Digest,
    ) -> QueryPoll<PreparedBodySyntaxKey> {
        macro_rules! ready {
            ($x:expr) => {
                match $x {
                    QueryPoll::Ready(Ok(v)) => v,
                    QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                    QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
                }
            };
        }
        if let Err(error) = self.require_source_preparation() {
            return QueryPoll::Ready(Err(error));
        }
        let plan = match completed.source_syntax(source) {
            Ok(plan) => plan,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let publication = ready!(current(self, plan, None));
        if !matches!(completed.dependency(), DependencyStamp::Query { key:QueryKey::DiscoverySeal(seal), fingerprint } if seal == plan.seal() && *fingerprint == plan.seal_value())
        {
            return QueryPoll::Ready(Err(invalid("body syntax completed owner differs")));
        }
        let key = PreparedBodySyntaxKey {
            source: source.outline.clone(),
            path: source.path.as_str().to_owned(),
            epoch: plan.epoch(),
            plan: plan.fingerprint(),
            definition,
        };
        if let Err(error) = key.selection() {
            return QueryPoll::Ready(Err(error));
        }
        let mut e = StableEncoder::new("lrr/v1/completed-source-syntax-input");
        e.digest(plan.fingerprint());
        e.digest(publication);
        e.u64(self.source_revision);
        let fingerprint = e.finish();
        let input_key = InputKey::CompletedSourceSyntax(plan.fingerprint());
        if let Some(existing) = self.inputs.get(&input_key) {
            return match existing.value.as_ref() {
                InputValue::CompletedSourceSyntax(actual) if actual.fingerprint == fingerprint => {
                    QueryPoll::Ready(Ok(key))
                }
                _ => QueryPoll::Ready(Err(invalid(
                    "body syntax derived plan publication conflicts",
                ))),
            };
        }
        let revision = match self.revision.checked_add(1) {
            Some(revision) => revision,
            None => {
                return QueryPoll::Ready(Err(fault(
                    "LRR-E-ID-EXHAUSTED",
                    "query revision exhausted",
                )));
            }
        };
        let input = Arc::new(CompletedSourceSyntaxInput {
            plan: Arc::clone(plan),
            publication,
            source_revision: self.source_revision,
            fingerprint,
        });
        let value = InputValue::CompletedSourceSyntax(Arc::clone(&input));
        let actual_fingerprint = value.fingerprint();
        let row_bytes = match input_metadata_bytes(&input_key, &value).and_then(|n| {
            account_add(
                n,
                account_add(
                    self.source_plan_pins.extra_bytes(
                        crate::db::value::SourceArtifactRef::CompletedSourceSyntax(plan),
                    )?,
                    self.source_plan_pins.extra_bytes(
                        crate::db::value::SourceArtifactRef::SourceSyntaxInput(&input),
                    )?,
                )?,
            )
        }) {
            Ok(n) => n,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let _ = row_bytes; // E-R5-006: no byte accounting.
        if let Err(error) = self.source_plan_pins.register_syntax_input(&input) {
            return QueryPoll::Ready(Err(error));
        }
        self.inputs.insert(
            input_key,
            InputRecord {
                value: Arc::new(value),
                fingerprint: actual_fingerprint,
            },
        );
        self.revision = revision;
        QueryPoll::Ready(Ok(key))
    }
}
