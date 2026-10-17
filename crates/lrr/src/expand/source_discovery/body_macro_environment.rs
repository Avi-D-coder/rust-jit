//! Cold selected-provider macro environment, admitted by a genuine published
//! discovery seal. No source/body identity alone can construct this receipt.
use super::*;
use crate::{
    core::digest::StableEncoder,
    db::{
        dependency::DependencyStamp, key::QueryKey, memo::Database, poll::QueryPoll,
        provider::SyntaxProvider, seals::SealKey, value::QueryValue,
    },
    frontend::source_macros::PublishedMacroIndex,
    source::changes::SourceSelection,
};
use std::sync::Arc;

pub struct SealedBodyMacroEnvironment {
    source: PreparedSourceView,
    index: Arc<PublishedMacroIndex>,
    source_environment: Arc<CompletedSourceEnvironment>,
    seal_dependency: DependencyStamp,
    identity: Digest,
    bytes: usize,
}
impl std::fmt::Debug for SealedBodyMacroEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedBodyMacroEnvironment")
            .field("source", self.source.selection())
            .field("identity", &self.identity)
            .field("seal", &self.seal())
            .finish_non_exhaustive()
    }
}
impl SealedBodyMacroEnvironment {
    pub fn source(&self) -> &PreparedSourceView {
        &self.source
    }
    pub fn index(&self) -> &PublishedMacroIndex {
        &self.index
    }
    pub fn index_root(&self) -> &Arc<PublishedMacroIndex> {
        &self.index
    }
    pub fn source_environment(&self) -> &Arc<CompletedSourceEnvironment> {
        &self.source_environment
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn dependency(&self) -> &DependencyStamp {
        &self.seal_dependency
    }
    pub fn seal(&self) -> &SealKey {
        let DependencyStamp::Query {
            key: QueryKey::DiscoverySeal(key),
            ..
        } = &self.seal_dependency
        else {
            unreachable!("private sealed environment construction")
        };
        key
    }
    pub fn seal_value(&self) -> Digest {
        let DependencyStamp::Query { fingerprint, .. } = &self.seal_dependency else {
            unreachable!("private sealed environment construction")
        };
        *fingerprint
    }
    /// Receipt allocation only. Query ownership separately accounts the actual
    /// selected index Arc and source roots exposed by index_root()/source().
    pub fn exclusive_metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        let DependencyStamp::Query { key, fingerprint } = &self.seal_dependency else {
            return QueryPoll::Ready(Err(fault("sealed body environment dependency differs")));
        };
        match db.fetch(&SyntaxProvider, key.clone()) {
            QueryPoll::Ready(Ok(outcome)) => match outcome.value.as_ref() {
                QueryValue::DiscoverySeal(seal)
                    if &seal.key() == self.seal() && outcome.fingerprint == *fingerprint =>
                {
                    QueryPoll::Ready(Ok(()))
                }
                _ => QueryPoll::Ready(Err(fault("sealed body macro environment changed"))),
            },
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}
/// Prepared once from an actual completed source provider and its inherited
/// context. It owns no CompletedSourceEnvironment Arc, avoiding a parent cycle.
pub(super) struct CompletedBodyMacroFacts {
    index: Arc<PublishedMacroIndex>,
    identity: Digest,
    receipt_bytes: usize,
}
impl CompletedBodyMacroFacts {
    pub(super) fn prepare(
        source: &PreparedSourceView,
        index: PublishedMacroIndex,
        compiler_identity: Option<Digest>,
    ) -> Result<Self, Diagnostic> {
        index.require_publication(source)?;
        let mut identity = StableEncoder::new("lrr/v1/selected-sealed-body-macro-environment");
        identity.record(&source.selection().outline);
        identity.string(source.selection().path.as_str());
        identity.digest(index.prepare_semantic_identity()?);
        identity.digest(super::super::source_declarative::engine_version());
        identity.option(&compiler_identity, |identity, compiler| {
            identity.digest(*compiler)
        });
        let receipt_bytes = std::mem::size_of::<SealedBodyMacroEnvironment>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|n| {
                n.checked_add(
                    source
                        .exclusive_metadata_bytes()
                        .ok()?
                        .checked_sub(std::mem::size_of::<PreparedSourceView>())?,
                )
            })
            .filter(|n| *n <= 1024 * 1024)
            .ok_or_else(|| fault("body environment receipt metadata limit"))?;
        Ok(Self {
            index: Arc::new(index),
            identity: identity.finish(),
            receipt_bytes,
        })
    }
    pub(super) fn index(&self) -> &Arc<PublishedMacroIndex> {
        &self.index
    }
}
impl CompletedSourceEnvironment {
    /// Selected owning-module envelope from immutable completed provider facts.
    /// The actual published seal is required; no provider/index reconstruction.
    pub(crate) fn sealed_body_macro_environment(
        self: &Arc<Self>,
        db: &mut Database,
        selection: &SourceSelection,
    ) -> QueryPoll<Arc<SealedBodyMacroEnvironment>> {
        let fact = match self.body_macro_fact(selection) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let DependencyStamp::Query {
            key: QueryKey::DiscoverySeal(seal_key),
            fingerprint,
        } = self.dependency()
        else {
            return QueryPoll::Ready(Err(fault(
                "completed body environment seal dependency differs",
            )));
        };
        match db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(seal_key.clone())) {
            QueryPoll::Ready(Ok(outcome))
                if matches!(outcome.value.as_ref(), QueryValue::DiscoverySeal(seal)
                if seal.key() == *seal_key && outcome.fingerprint == *fingerprint) => {}
            QueryPoll::Ready(Ok(_)) => {
                return QueryPoll::Ready(Err(fault(
                    "body environment current published discovery seal differs",
                )));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        QueryPoll::Ready((|| {
            let source = self.source_for_macro_fact(selection)?;
            fact.index.require_publication(source)?;
            let bytes = fact
                .receipt_bytes
                .checked_add(self.dependency().heap_bytes())
                .filter(|n| *n <= 1024 * 1024)
                .ok_or_else(|| fault("body environment receipt metadata limit"))?;
            Ok(Arc::new(SealedBodyMacroEnvironment {
                source: source.share(),
                index: Arc::clone(&fact.index),
                source_environment: Arc::clone(self),
                seal_dependency: self.dependency().clone(),
                identity: fact.identity,
                bytes,
            }))
        })())
    }
}
impl SourceDiscovery {
    /// Same completed-fact factory used by reached ordinary module bodies.
    pub fn sealed_body_macro_environment(
        &self,
        db: &mut Database,
        selection: &SourceSelection,
    ) -> QueryPoll<Arc<SealedBodyMacroEnvironment>> {
        let environment = match self.completed_source_environment(db) {
            QueryPoll::Ready(Ok(environment)) => environment,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        environment.sealed_body_macro_environment(db, selection)
    }
}
