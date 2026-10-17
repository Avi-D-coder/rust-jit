//! Borrowed admission observations from genuine completed source discovery.
//! The view owns no compiler/source/generated provider graph and cannot install
//! it in the runtime catalog or grant ordinary body/parser availability.
use super::*;
use crate::{
    core::digest::StableEncoder,
    db::{
        dependency::DependencyStamp, key::QueryKey, memo::Database, poll::QueryPoll,
        provider::SyntaxProvider, value::QueryValue,
    },
    expand::source_declarative::{SourceDeclarative, macro_exports::SourceMacroExportFacet},
};

/// Private construction joins the configured captured facet to the actual
/// discovery provider and a current, complete published seal. Its lifetime
/// keeps both original owners borrowed; it is not an owning facet transport.
pub struct SealedMacroExportView<'a> {
    facet: &'a SourceMacroExportFacet,
    provider: &'a SourceDeclarative,
    seal: &'a Seal,
    dependencies: [DependencyStamp; 3],
    identity: Digest,
    bytes: usize,
}
impl std::fmt::Debug for SealedMacroExportView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedMacroExportView")
            .field("identity", &self.identity)
            .field("facet", &self.facet.identity())
            .field("seal", &self.seal.key())
            .finish_non_exhaustive()
    }
}
impl SealedMacroExportView<'_> {
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn facet(&self) -> &SourceMacroExportFacet {
        self.facet
    }
    pub fn provider(&self) -> &SourceDeclarative {
        self.provider
    }
    pub fn seal(&self) -> &Seal {
        self.seal
    }
    pub fn dependencies(&self) -> &[DependencyStamp; 3] {
        &self.dependencies
    }
    /// Only this view's owned dependency data. All provider/source/compiler
    /// roots remain borrowed and must be accounted by their actual owners.
    pub fn exclusive_metadata_bytes(&self) -> usize {
        self.bytes
    }
    /// Preparation-time currentness demand. A retained borrowed view is not an
    /// unconditional lifetime/currentness grant after publication changes.
    pub fn revalidate(&self, db: &mut Database) -> QueryPoll<()> {
        let selected = match db.macro_export_source_dependencies(self.facet) {
            Ok(selected) => selected,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if selected[..] != self.dependencies[..2] {
            return QueryPoll::Ready(Err(fault("selected macro-export preparation changed")));
        }
        let DependencyStamp::Query { key, fingerprint } = &self.dependencies[2] else {
            return QueryPoll::Ready(Err(fault("macro-export seal dependency differs")));
        };
        match db.fetch(&SyntaxProvider, key.clone()) {
            QueryPoll::Ready(Ok(outcome)) => match outcome.value.as_ref() {
                QueryValue::DiscoverySeal(seal)
                    if seal.key() == self.seal.key() && outcome.fingerprint == *fingerprint =>
                {
                    QueryPoll::Ready(Ok(()))
                }
                _ => QueryPoll::Ready(Err(fault("macro-export completed seal changed"))),
            },
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}
impl SourceDiscovery {
    /// Bind only selected role-source preparation observations before work
    /// begins. Ordinary source inputs return None and retain their old route.
    pub fn with_macro_export_source_publications(
        mut self,
        db: &mut Database,
    ) -> Result<Self, Diagnostic> {
        if self.started
            || self.failed.is_some()
            || self.seal.is_some()
            || !self.macro_export_modules.is_empty()
            || (self.declarative.is_some() && !self.macro_export_sources.is_empty())
        {
            return Err(fault("macro-export source publication selection is frozen"));
        }
        let mut additions = BTreeMap::new();
        let mut roles = BTreeMap::new();
        let mut role_bytes = 0usize;
        for (ordinal, source) in self.sources.iter().enumerate() {
            let Some(observation) =
                db.macro_export_source_observation(source.selection(), self.key.unit_graph)?
            else {
                continue;
            };
            if self.declarative.is_some() {
                return Err(fault(
                    "macro-export role must be selected before compiler route setup",
                ));
            }
            if self.namespace != Namespace::Macro
                || self.key.domain
                    != (SealDomain::Scope {
                        id: self.scope.fingerprint(),
                        namespace: Namespace::Macro,
                    })
                || observation.outline_fingerprint() != source.outline().fingerprint()
            {
                return Err(fault("macro-export role source scope/outline differs"));
            }
            observation.require_source(source, self.key.unit_graph)?;
            role_bytes = role_bytes
                .checked_add(observation.metadata_bytes())
                .and_then(|bytes| bytes.checked_add(128 + 2 * std::mem::size_of::<usize>()))
                .ok_or_else(|| fault("macro-export source role metadata limit"))?;
            for stamp in observation.dependencies() {
                let DependencyStamp::Input { key, fingerprint } = stamp else {
                    return Err(fault(
                        "macro-export preparation is not an input observation",
                    ));
                };
                if self
                    .compiler_inputs
                    .get(key)
                    .is_some_and(|old| old != fingerprint)
                    || additions.get(key).is_some_and(|old| old != fingerprint)
                {
                    return Err(fault("macro-export source preparation changed"));
                }
                if !self.compiler_inputs.contains_key(key) && !additions.contains_key(key) {
                    additions.insert(key.clone(), *fingerprint);
                }
            }
            roles.insert(ordinal, std::sync::Arc::new(observation));
        }
        let role_units = roles
            .values()
            .map(|role| role.source().outline.crate_key)
            .collect::<std::collections::BTreeSet<_>>();
        for (ordinal, source) in self.sources.iter().enumerate() {
            if role_units.contains(&source.selection().outline.crate_key)
                && !roles.contains_key(&ordinal)
            {
                return Err(fault(
                    "selected macro-export unit source lacks its own role observation",
                ));
            }
        }
        // The selected keys are fixed-size scalar receipt/environment keys.
        // Charge both temporary and retained rows before mutating this owner.
        let row_bytes = std::mem::size_of::<(crate::db::dependency::InputKey, Digest)>()
            .checked_add(96)
            .and_then(|bytes| bytes.checked_mul(2))
            .ok_or_else(|| fault("macro-export selected input row overflow"))?;
        let input_bytes = additions
            .len()
            .checked_mul(row_bytes)
            .ok_or_else(|| fault("macro-export selected input metadata overflow"))?;
        self.retained_record_bytes
            .checked_add(input_bytes)
            .and_then(|bytes| bytes.checked_add(role_bytes))
            .ok_or_else(|| fault("macro-export selected input transient metadata limit"))?;
        let retained = self
            .retained_record_bytes
            .checked_sub(self.macro_export_source_bytes)
            .and_then(|bytes| bytes.checked_add(role_bytes))
            .and_then(|bytes| bytes.checked_add(input_bytes))
            .ok_or_else(|| fault("macro-export selected input metadata limit"))?;
        self.compiler_inputs.extend(additions);
        self.macro_export_sources = roles;
        self.macro_export_source_bytes = role_bytes;
        self.retained_record_bytes = retained;
        Ok(self)
    }
    /// Borrow the actual completed original macro-export provider after the
    /// ordinary DiscoverySeal provider validates its full dependency closure.
    /// Missing role-specific source publication remains a real query demand.
    pub fn completed_macro_export_facet<'a>(
        &'a self,
        db: &mut Database,
        facet: &'a SourceMacroExportFacet,
    ) -> QueryPoll<SealedMacroExportView<'a>> {
        let prepared = (|| {
            let _ = self.sealed_expansions()?;
            if self.namespace != Namespace::Macro
                || self.scope != facet.source().outline().root_scope
                || self.key.unit_graph != facet.unit_graph()
                || self.key.domain
                    != (SealDomain::Scope {
                        id: self.scope.fingerprint(),
                        namespace: Namespace::Macro,
                    })
            {
                return Err(fault("macro-export discovery source/scope/graph differs"));
            }
            let (_, engine, providers) = self
                .declarative
                .as_ref()
                .ok_or_else(|| fault("macro-export declarative discovery is absent"))?;
            if *engine != crate::expand::source_declarative::engine_version() {
                return Err(fault("macro-export source engine generation differs"));
            }
            let mut owners = self.sources.iter().enumerate().filter(|(_, source)| {
                source.selection() == facet.source().selection()
                    && source.same_publication(facet.source())
            });
            let (ordinal, _) = owners
                .next()
                .ok_or_else(|| fault("macro-export exact original source is absent"))?;
            if owners.next().is_some() {
                return Err(fault("macro-export exact original source is ambiguous"));
            }
            let provider = providers
                .get(ordinal)
                .ok_or_else(|| fault("macro-export owning provider is absent"))?;
            if provider.module_source().selection() != facet.source().selection()
                || !provider.module_source().same_publication(facet.source())
                || provider.macro_index().pending_producers().next().is_some()
            {
                return Err(fault("macro-export provider source or completion differs"));
            }
            let selected = db.macro_export_source_dependencies(facet)?;
            let seal = self
                .seal
                .as_ref()
                .ok_or_else(|| fault("macro-export discovery is incomplete"))?;
            let fingerprint = self
                .body_boundary_seal_fingerprint
                .ok_or_else(|| fault("macro-export completed seal identity is absent"))?;
            Ok((provider, seal, selected, fingerprint))
        })();
        let (provider, seal, [source, environment], fingerprint) = match prepared {
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
            return QueryPoll::Ready(Err(fault("macro-export full published seal differs")));
        }
        let dependencies = [
            source,
            environment,
            DependencyStamp::Query { key, fingerprint },
        ];
        let Some(bytes) = dependencies
            .iter()
            .try_fold(
                std::mem::size_of::<SealedMacroExportView<'_>>(),
                |bytes, stamp| bytes.checked_add(stamp.heap_bytes()),
            )
        else {
            return QueryPoll::Ready(Err(fault(
                "macro-export borrowed dependency metadata limit",
            )));
        };
        let mut identity = StableEncoder::new("lrr/v1/borrowed-sealed-macro-export");
        identity.digest(facet.identity());
        identity.sequence(&dependencies, |identity, dependency| {
            identity.record(dependency)
        });
        QueryPoll::Ready(Ok(SealedMacroExportView {
            facet,
            provider,
            seal,
            dependencies,
            identity: identity.finish(),
            bytes,
        }))
    }
}
