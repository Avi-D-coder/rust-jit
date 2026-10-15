//! Immutable retained syntax admitted against actual current source/epoch/seal
//! inputs. This supplies parsing provenance, not a namespace or VM entry grant.
use super::{
    dependency::{InputKey, InputValue},
    key::QueryKey,
    memo::Database,
    poll::QueryPoll,
    provider::SyntaxProvider,
    seals::SealKey,
    value::QueryValue,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::source_declarative::assembly::PreparedSourceSyntax,
    syntax::parse::{ParseKey, ParsedFile},
};
use std::{collections::BTreeMap, sync::Arc};

pub struct RetainedSyntaxInput {
    prepared: Arc<PreparedSourceSyntax>,
    seal: SealKey,
    publication: Digest,
    key: ParseKey,
    bytes: usize,
}
impl std::fmt::Debug for RetainedSyntaxInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedSyntaxInput")
            .field("key", &self.key)
            .field("seal", &self.seal)
            .finish()
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
fn add(a: usize, b: usize) -> Result<usize, Diagnostic> {
    a.checked_add(b)
        .filter(|n| *n <= 128 * 1024 * 1024)
        .ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "retained source input metadata budget".into(),
            )
        })
}
impl RetainedSyntaxInput {
    pub fn prepare(
        db: &mut Database,
        prepared: Arc<PreparedSourceSyntax>,
        seal: SealKey,
    ) -> QueryPoll<Arc<Self>> {
        if let Err(error) = db.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        let publication = match current(db, &prepared, &seal, None) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        let result = (|| {
            let mut identity = StableEncoder::new("lrr/v1/admitted-source-syntax");
            identity.digest(prepared.identity());
            identity.record(&seal);
            identity.digest(publication);
            let key = ParseKey {
                content: identity.finish(),
                profile: prepared.source().outline.parse.profile.clone(),
            };
            let bytes = add(
                add(
                    add(
                        std::mem::size_of::<Self>(),
                        key.profile.semantics.capacity(),
                    )?,
                    prepared.metadata_excluding_snapshot()?,
                )?,
                ParsedFile::snapshot_metadata_bytes(prepared.syntax())?,
            )?;
            let input = Arc::new(Self {
                prepared,
                seal,
                publication,
                key,
                bytes,
            });
            // Validate the complete structural replay before admitting input.
            // The temporary tree is not retained in the input or byte source map.
            input.replay()?;
            Ok(input)
        })();
        QueryPoll::Ready(result)
    }
    pub fn key(&self) -> &ParseKey {
        &self.key
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        Ok(self.bytes)
    }
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        match current(db, &self.prepared, &self.seal, Some(self.publication)) {
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Ok(())),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => QueryPoll::NeedInput(request),
        }
    }
    pub(crate) fn replay(&self) -> Result<ParsedFile, Diagnostic> {
        let mut parsed = ParsedFile::from_syntax_snapshot(
            self.prepared.syntax().clone(),
            self.key.profile.clone(),
        )?;
        // A typed structural input has its own canonical admission identity.
        // No SourceBlob is manufactured under this digest.
        parsed.content = self.key.content;
        parsed.generated_origin = Some(self.key.content);
        Ok(parsed)
    }
}
impl StableEncode for RetainedSyntaxInput {
    const DOMAIN: &'static str = "lrr/v1/retained-syntax-input";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.prepared.identity());
        e.record(&self.seal);
        e.digest(self.publication);
    }
}
fn current(
    db: &mut Database,
    prepared: &PreparedSourceSyntax,
    seal: &SealKey,
    expected: Option<Digest>,
) -> QueryPoll<Digest> {
    if prepared.epoch() != seal.epoch || !prepared.pending_calls().is_empty() {
        return QueryPoll::Ready(Err(fault(
            "retained syntax epoch differs or expansion calls remain pending",
        )));
    }
    if !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(), Some(InputValue::ExpansionEpoch(epoch)) if *epoch == prepared.epoch())
    {
        return QueryPoll::Ready(Err(fault(
            "retained syntax current expansion epoch is absent or stale",
        )));
    }
    match db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(seal.clone())) {
        QueryPoll::Ready(Ok(outcome)) if matches!(outcome.value.as_ref(), QueryValue::DiscoverySeal(actual) if actual.key() == *seal) =>
            {}
        QueryPoll::Ready(Ok(_)) => {
            return QueryPoll::Ready(Err(fault("retained syntax discovery result differs")));
        }
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    }
    let Some(certificate) = db.input(&InputKey::DiscoverySeal(seal.clone())) else {
        return QueryPoll::Ready(Err(fault("retained syntax discovery input disappeared")));
    };
    let InputValue::DiscoverySeal(input) = certificate.as_ref() else {
        return QueryPoll::Ready(Err(fault(
            "retained syntax discovery input variant differs",
        )));
    };
    let publication = certificate.fingerprint();
    if expected.is_some_and(|value| value != publication)
        || !input
            .sources
            .iter()
            .any(|source| source == prepared.source())
    {
        return QueryPoll::Ready(Err(fault(
            "retained syntax enclosing source publication differs",
        )));
    }
    if !matches!(db.input(&InputKey::CapturedSource(prepared.source().location_identity())).as_deref(), Some(InputValue::CapturedSource(source)) if source == prepared.source())
    {
        return QueryPoll::Ready(Err(fault("retained syntax captured caller changed")));
    }
    match db.prepared_source(prepared.source()) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return QueryPoll::Ready(Err(fault(
                "retained syntax captured caller lease is absent",
            )));
        }
        Err(error) => return QueryPoll::Ready(Err(error)),
    }
    if input.records.len() > 100_000 {
        return QueryPoll::Ready(Err(fault("retained syntax record inventory limit")));
    }
    let records = input
        .records
        .iter()
        .map(|record| (record.key.fingerprint(), record))
        .collect::<BTreeMap<_, _>>();
    if records.len() != input.records.len() {
        return QueryPoll::Ready(Err(fault("retained syntax record inventory duplicated")));
    }
    for output in prepared.outputs() {
        let actual = output.record().descriptor();
        if records
            .get(&actual.key.fingerprint())
            .is_none_or(|record| record.fingerprint() != actual.fingerprint())
            || input
                .seal
                .input_dependencies()
                .binary_search(&(actual.key.fingerprint(), actual.fingerprint()))
                .is_err()
        {
            return QueryPoll::Ready(Err(fault(
                "retained syntax record is absent from its discovery certificate",
            )));
        }
    }
    QueryPoll::Ready(Ok(publication))
}
