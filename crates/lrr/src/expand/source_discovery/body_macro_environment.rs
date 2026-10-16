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
impl SourceDiscovery {
    /// Preparation only: select one genuinely completed provider/index. A
    /// generated body's actual boundary is joined separately by its owner.
    pub fn sealed_body_macro_environment(
        &self,
        db: &mut Database,
        selection: &SourceSelection,
    ) -> QueryPoll<Arc<SealedBodyMacroEnvironment>> {
        let prepared = (|| {
            let _ = self.sealed_expansions()?;
            let seal = self
                .seal
                .as_ref()
                .ok_or_else(|| fault("body environment discovery is unsealed"))?;
            let fingerprint = self
                .body_boundary_seal_fingerprint
                .ok_or_else(|| fault("body environment completed seal identity is absent"))?;
            let providers = &self
                .declarative
                .as_ref()
                .ok_or_else(|| fault("body environment source providers absent"))?
                .2;
            let source_key = QueryKey::Outline(selection.outline.clone()).fingerprint();
            let original = self.source_jobs.get(&source_key).copied();
            let module = self.module_parse_jobs.get(&source_key).copied();
            if original.is_some() && module.is_some() {
                return Err(fault(
                    "body environment selected source has duplicate owners",
                ));
            }
            // Delay the selected index construction until after the genuine
            // published seal query below; this first pass only identifies owner.
            if let Some(ordinal) = original {
                let source = self
                    .sources
                    .get(ordinal)
                    .ok_or_else(|| fault("body environment source owner absent"))?;
                let provider = providers
                    .get(ordinal)
                    .ok_or_else(|| fault("body environment provider absent"))?;
                if source.selection() != selection
                    || provider.module_source().selection() != selection
                    || !source.same_publication(provider.module_source())
                    || provider.macro_export_source_role().is_some()
                    || provider.macro_index().pending_producers().next().is_some()
                {
                    return Err(fault(
                        "body environment exact selected runtime source differs",
                    ));
                }
            } else if let Some(producer) = module {
                let staged = self
                    .staged_modules
                    .get(&producer)
                    .ok_or_else(|| fault("body environment module owner absent"))?;
                if !staged.namespace_complete
                    || staged.syntax.prepared_source().selection() != selection
                    || staged.provider.module_source().selection() != selection
                    || !staged
                        .syntax
                        .prepared_source()
                        .same_publication(staged.provider.module_source())
                    || staged.provider.macro_export_source_role().is_some()
                    || staged
                        .provider
                        .macro_index()
                        .pending_producers()
                        .next()
                        .is_some()
                {
                    return Err(fault(
                        "body environment exact selected module is incomplete",
                    ));
                }
            } else {
                return Err(fault(
                    "body environment selected source was not admitted by discovery",
                ));
            }
            Ok((seal, fingerprint, providers, original, module))
        })();
        let (seal, fingerprint, providers, original, module) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let key = QueryKey::DiscoverySeal(self.key.clone());
        let outcome = match db.fetch(&SyntaxProvider, key.clone()) {
            QueryPoll::Ready(Ok(outcome)) => outcome,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        if !matches!(outcome.value.as_ref(), QueryValue::DiscoverySeal(current)
            if current.key() == seal.key() && outcome.fingerprint == fingerprint)
        {
            return QueryPoll::Ready(Err(fault(
                "body environment current published discovery seal differs",
            )));
        }
        QueryPoll::Ready((|| {
            let (source, provider, inherited) = if let Some(ordinal) = original {
                let provider = &providers[ordinal];
                (provider.module_source(), provider, None)
            } else {
                let staged = self
                    .staged_modules
                    .get(&module.ok_or_else(|| fault("body environment owner disappeared"))?)
                    .ok_or_else(|| fault("body environment selected module disappeared"))?;
                (
                    staged.syntax.prepared_source(),
                    &staged.provider,
                    Some(self.module_projection_inherited(&staged.syntax)?),
                )
            };
            let index = provider.completed_runtime_index(
                providers,
                inherited.as_ref(),
                64 * 1024 * 1024,
            )?;
            let index_identity = index.prepare_semantic_identity()?;
            let mut identity = StableEncoder::new("lrr/v1/selected-sealed-body-macro-environment");
            identity.record(&selection.outline);
            identity.string(selection.path.as_str());
            identity.digest(index_identity);
            identity.digest(super::super::source_declarative::engine_version());
            let compiler_identity = provider
                .compiler
                .as_ref()
                .map(|catalog| catalog.owning_profile_identity())
                .transpose()?;
            identity.option(&compiler_identity, |identity, compiler| {
                identity.digest(*compiler)
            });
            let seal_dependency = DependencyStamp::Query { key, fingerprint };
            let bytes = std::mem::size_of::<SealedBodyMacroEnvironment>()
                .checked_add(3 * std::mem::size_of::<usize>())
                .and_then(|bytes| bytes.checked_add(seal_dependency.heap_bytes()))
                .filter(|bytes| *bytes <= 1024 * 1024)
                .ok_or_else(|| fault("body environment receipt metadata limit"))?;
            Ok(Arc::new(SealedBodyMacroEnvironment {
                source: source.share(),
                index: Arc::new(index),
                seal_dependency,
                identity: identity.finish(),
                bytes,
            }))
        })())
    }
}
