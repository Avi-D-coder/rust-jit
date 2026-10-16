//! One shared immutable result of actual discovery. Raw graph access is a cold
//! preparation input; it neither publishes a source nor certifies a use-site.
use super::*;
use crate::db::{dependency::DependencyStamp, key::QueryKey};
use std::sync::Arc;

pub(crate) enum SourceScopeOwner<'a> {
    Selected(&'a PreparedSourceView),
    Ambiguous(&'a [u32]),
    Missing,
}
pub struct CompletedSourceEnvironment {
    names: Arc<NameNamespaces>,
    sources: Arc<Vec<PreparedSourceView>>,
    scope_owners: BTreeMap<ScopeKey, Vec<u32>>,
    seal_dependency: DependencyStamp,
    bytes: usize,
    names_bytes: usize,
    sources_bytes: usize,
}
impl std::fmt::Debug for CompletedSourceEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletedSourceEnvironment")
            .field("seal_dependency", &self.seal_dependency)
            .field("sources", &self.sources.len())
            .finish_non_exhaustive()
    }
}
impl CompletedSourceEnvironment {
    pub fn names(&self) -> &NameNamespaces {
        &self.names
    }
    pub fn names_root(&self) -> &Arc<NameNamespaces> {
        &self.names
    }
    /// Actual runtime source leases only. MacroExport facet sources retain their
    /// separate catalog ownership and acquire no runtime source authority here.
    pub fn sources(&self) -> &[PreparedSourceView] {
        &self.sources
    }
    pub fn sources_root(&self) -> &Arc<Vec<PreparedSourceView>> {
        &self.sources
    }
    /// Actual source ownership only, not a namespace/visibility certificate.
    /// Duplicate admitted owners remain explicit rather than choosing an ordinal.
    pub(crate) fn scope_owner(&self, scope: &ScopeKey) -> SourceScopeOwner<'_> {
        match self.scope_owners.get(scope).map(Vec::as_slice) {
            Some([ordinal]) => self
                .sources
                .get(*ordinal as usize)
                .map(SourceScopeOwner::Selected)
                .unwrap_or(SourceScopeOwner::Missing),
            Some(ordinals) => SourceScopeOwner::Ambiguous(ordinals),
            None => SourceScopeOwner::Missing,
        }
    }
    pub fn dependency(&self) -> &DependencyStamp {
        &self.seal_dependency
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn names_metadata_bytes(&self) -> usize {
        self.names_bytes
    }
    pub(crate) fn sources_metadata_bytes(&self) -> usize {
        self.sources_bytes
    }
    pub(crate) fn retained_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.bytes
            .checked_add(self.names_bytes)
            .and_then(|n| n.checked_add(self.sources_bytes))
            .and_then(|n| n.checked_add(6 * std::mem::size_of::<usize>()))
            .ok_or_else(crate::runtime::arena::resource)
    }
}
impl SourceDiscovery {
    /// Coordinator/preparation demand. The real published seal is fetched before
    /// sharing a completed graph; a source/graph digest cannot mint this lease.
    pub(crate) fn completed_source_environment(
        &self,
        db: &mut crate::db::memo::Database,
    ) -> crate::db::poll::QueryPoll<Arc<CompletedSourceEnvironment>> {
        use crate::db::{poll::QueryPoll, provider::SyntaxProvider, value::QueryValue};
        let root = match self.sealed_expansions().and_then(|_| {
            self.completed_source_environment
                .as_ref()
                .ok_or_else(|| fault("completed source environment absent"))
        }) {
            Ok(root) => root,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let DependencyStamp::Query { key, fingerprint } = root.dependency() else {
            return QueryPoll::Ready(Err(fault(
                "completed source environment dependency differs",
            )));
        };
        match db.fetch(&SyntaxProvider, key.clone()) {
            QueryPoll::Ready(Ok(value))
                if matches!(value.value.as_ref(),QueryValue::DiscoverySeal(seal)
                if seal.key()==self.key && value.fingerprint==*fingerprint) =>
            {
                match db.retain_completed_source_environment(root) {
                    Ok(()) => QueryPoll::Ready(Ok(Arc::clone(root))),
                    Err(error) => QueryPoll::Ready(Err(error)),
                }
            }
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Err(fault(
                "completed source environment publication changed",
            ))),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    pub(super) fn prepare_completed_source_environment(
        &mut self,
        seal: &Seal,
    ) -> Result<(), Diagnostic> {
        if self.completed_source_environment.is_some() || seal.key() != self.key {
            return Err(fault("completed source environment owner differs"));
        }
        let providers = self.declarative.as_ref().map(|(_, _, providers)| providers);
        if providers.is_some_and(|providers| providers.len() != self.sources.len()) {
            return Err(fault("completed source provider/source ownership differs"));
        }
        let count = self
            .sources
            .len()
            .checked_add(self.staged_modules.len())
            .filter(|n| *n <= 100_000)
            .ok_or_else(|| fault("completed source environment source limit"))?;
        count
            .checked_mul(std::mem::size_of::<PreparedSourceView>())
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(count)
            .map_err(|_| fault("completed source environment source allocation"))?;
        for (ordinal, source) in self.sources.iter().enumerate() {
            if providers
                .and_then(|providers| providers.get(ordinal))
                .is_some_and(|provider| provider.macro_export_source_role().is_some())
            {
                continue;
            }
            self.graph
                .require_source(source.selection(), source.outline())?;
            sources.push(source.share());
        }
        for module in self.staged_modules.values() {
            if module.namespace_complete && module.provider.macro_export_source_role().is_none() {
                let source = module.syntax.prepared_source();
                self.graph
                    .require_source(source.selection(), source.outline())?;
                sources.push(source.share());
            }
        }
        let sources_bytes = sources.iter().try_fold(
            std::mem::size_of::<Vec<PreparedSourceView>>()
                .checked_add(
                    sources
                        .capacity()
                        .checked_mul(std::mem::size_of::<PreparedSourceView>())
                        .ok_or_else(crate::runtime::arena::resource)?,
                )
                .ok_or_else(crate::runtime::arena::resource)?,
            |bytes, source| {
                bytes
                    .checked_add(
                        source
                            .exclusive_metadata_bytes()?
                            .checked_sub(std::mem::size_of::<PreparedSourceView>())
                            .ok_or_else(crate::runtime::arena::resource)?,
                    )
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(crate::runtime::arena::resource)
            },
        )?;
        let mut scope_owners: BTreeMap<ScopeKey, Vec<u32>> = BTreeMap::new();
        let mut owner_bytes = 0usize;
        let mut owner_rows = 0usize;
        for (ordinal, source) in sources.iter().enumerate() {
            let outline = source.outline();
            // A `mod child;` row reserves a scope but does not own that file's
            // contents. Actual child capture owns its root scope independently.
            let external: BTreeSet<_> = outline
                .textual_events
                .iter()
                .filter_map(|event| {
                    matches!(
                        &event.kind,
                        TextualEventKind::Producer {
                            kind: crate::syntax::outline::ProducerKind::ModuleSource,
                            ..
                        }
                    )
                    .then_some(&event.scope)
                })
                .collect();
            for scope in outline.scopes.keys() {
                if scope.owner.is_some()
                    || (scope != &outline.root_scope && external.contains(scope))
                {
                    continue;
                }
                owner_rows = owner_rows
                    .checked_add(1)
                    .filter(|n| *n <= 100_000)
                    .ok_or_else(crate::runtime::arena::resource)?;
                let owners = scope_owners.entry(scope.clone()).or_default();
                owners
                    .try_reserve_exact(1)
                    .map_err(|_| crate::runtime::arena::resource())?;
                owners.push(u32::try_from(ordinal).map_err(|_| crate::runtime::arena::resource())?);
            }
        }
        for (scope, owners) in &scope_owners {
            owner_bytes = owner_bytes
                .checked_add(scope.heap_bytes())
                .and_then(|n| n.checked_add(256 + 16 * std::mem::size_of::<(ScopeKey, Vec<u32>)>()))
                .and_then(|n| {
                    n.checked_add(owners.capacity().checked_mul(std::mem::size_of::<u32>())?)
                })
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        let fingerprint = self
            .body_boundary_seal_fingerprint
            .ok_or_else(|| fault("completed source environment seal identity absent"))?;
        let seal_dependency = DependencyStamp::Query {
            key: QueryKey::DiscoverySeal(self.key.clone()),
            fingerprint,
        };
        let bytes = std::mem::size_of::<CompletedSourceEnvironment>()
            .checked_add(seal_dependency.heap_bytes())
            .and_then(|n| n.checked_add(owner_bytes))
            .ok_or_else(crate::runtime::arena::resource)?;
        let names_bytes = self.graph.exclusive_retained_bytes()?;
        // Share only after all fallible preparation has passed. Subsequent
        // mutation is rejected by unique_graph; no COW allocation is possible.
        self.completed_source_environment = Some(Arc::new(CompletedSourceEnvironment {
            names: Arc::clone(&self.graph),
            sources: Arc::new(sources),
            scope_owners,
            seal_dependency,
            bytes,
            names_bytes,
            sources_bytes,
        }));
        Ok(())
    }
}
