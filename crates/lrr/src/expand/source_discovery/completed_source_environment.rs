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
    body_macros: BTreeMap<Digest, (u32, super::body_macro_environment::CompletedBodyMacroFacts)>,
    macro_admission_bytes: usize,
    source_syntax: BTreeMap<Digest, Arc<super::source_syntax::CompletedSourceSyntax>>,
    seal_dependency: DependencyStamp,
    bytes: usize,
    names_bytes: usize,
    sources_bytes: usize,
    // Impl members of this environment's nominal types, derived once on
    // first demand from `names`, `sources` and the generated item units, and
    // shared by every reader (method probing, associated paths, drop/copy).
    members: std::sync::OnceLock<Arc<crate::frontend::member_index::MemberIndex>>,
    // Macro-generated item units of this sealed discovery, admitted when it
    // publishes them (after the environment itself is completed).
    generated: std::sync::OnceLock<Vec<Arc<crate::expand::generated_syntax::GeneratedSyntaxInput>>>,
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
    /// The impl member index of this immutable environment (type-relative
    /// associated paths and method probing), built at most once.
    pub(crate) fn member_index(
        &self,
    ) -> Result<Arc<crate::frontend::member_index::MemberIndex>, Diagnostic> {
        if let Some(members) = self.members.get() {
            return Ok(Arc::clone(members));
        }
        let sources: Vec<_> = self.sources.iter().collect();
        let mut built = crate::frontend::member_index::MemberIndex::build(&self.names, &sources)?;
        for unit in self.generated.get().into_iter().flatten() {
            built.add_generated(&self.names, unit)?;
        }
        // Associated functions written by impl/trait-item expansions into
        // original owners are members of those same owners.
        for plan in self.source_syntax.values() {
            for member in plan.generated_associated_members()? {
                built.add_associated(
                    member.owner,
                    member.owner_trait,
                    member.constant,
                    &member.name,
                    member.identity,
                    member.provided,
                );
            }
        }
        Ok(Arc::clone(self.members.get_or_init(|| Arc::new(built))))
    }
    /// Admit the generated item units this discovery published. The member
    /// index must not be demanded before publication, or it would miss them.
    pub(crate) fn admit_generated_units(
        &self,
        units: Vec<Arc<crate::expand::generated_syntax::GeneratedSyntaxInput>>,
    ) -> Result<(), Diagnostic> {
        if self.members.get().is_some() && !units.is_empty() {
            return Err(fault(
                "completed member index was demanded before generated publication",
            ));
        }
        let _ = self.generated.set(units);
        Ok(())
    }
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
    pub(crate) fn macro_index_roots(
        &self,
    ) -> impl Iterator<Item = &Arc<crate::frontend::source_macros::PublishedMacroIndex>> {
        self.body_macros.values().map(|(_, fact)| fact.index())
    }
    pub(super) fn source_for_macro_fact(
        &self,
        selection: &crate::source::changes::SourceSelection,
    ) -> Result<&PreparedSourceView, Diagnostic> {
        let key = QueryKey::Outline(selection.outline.clone()).fingerprint();
        let (ordinal, _) = self
            .body_macros
            .get(&key)
            .ok_or_else(|| fault("completed module macro provider absent"))?;
        self.sources
            .get(*ordinal as usize)
            .filter(|source| source.selection() == selection)
            .ok_or_else(|| fault("completed module macro source selection differs"))
    }
    pub(super) fn body_macro_fact(
        &self,
        selection: &crate::source::changes::SourceSelection,
    ) -> Result<&super::body_macro_environment::CompletedBodyMacroFacts, Diagnostic> {
        self.source_for_macro_fact(selection)?;
        let key = QueryKey::Outline(selection.outline.clone()).fingerprint();
        self.body_macros
            .get(&key)
            .map(|(_, fact)| fact)
            .ok_or_else(|| fault("completed module macro provider absent"))
    }
    /// Actual physical map-owned handles, only for retention accounting.
    pub(crate) fn syntax_plan_roots(
        &self,
    ) -> impl Iterator<Item = &Arc<super::source_syntax::CompletedSourceSyntax>> {
        self.source_syntax.values()
    }
    pub(crate) fn source_syntax(
        &self,
        selection: &crate::source::changes::SourceSelection,
    ) -> Result<&Arc<super::source_syntax::CompletedSourceSyntax>, Diagnostic> {
        let key = QueryKey::Outline(selection.outline.clone()).fingerprint();
        self.source_syntax
            .get(&key)
            .filter(|plan| plan.source() == selection)
            .ok_or_else(|| fault("completed source syntax owner absent or differs"))
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
            .and_then(|n| n.checked_add(self.macro_admission_bytes))
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
            .ok_or_else(|| fault("completed source environment source limit"))?;
        count
            .checked_mul(std::mem::size_of::<PreparedSourceView>())
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
                .require_view(source)?;
            sources.push(source.share());
        }
        for module in self.staged_modules.values() {
            if module.namespace_complete && module.provider.macro_export_source_role().is_none() {
                if module
                    .provider
                    .macro_index()
                    .pending_producers()
                    .next()
                    .is_some()
                {
                    return Err(fault(
                        "completed runtime module macro provider remains pending",
                    ));
                }
                let source = module.syntax.prepared_source();
                self.graph
                    .require_view(source)?;
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
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        // Required module/source macro closure only; no executable body is
        // parsed/lowered here. Runtime factories share these immutable indexes.
        let macro_source_bytes = sources
            .len()
            .checked_mul(256 + 16 * std::mem::size_of::<(Digest, u32)>())
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut macro_sources = BTreeMap::new();
        for (ordinal, source) in sources.iter().enumerate() {
            let key = QueryKey::Outline(source.selection().outline.clone()).fingerprint();
            if macro_sources
                .insert(
                    key,
                    u32::try_from(ordinal).map_err(|_| crate::runtime::arena::resource())?,
                )
                .is_some()
            {
                return Err(fault("completed macro source inventory key duplicated"));
            }
        }
        let mut body_macros = BTreeMap::new();
        let mut macro_admission_bytes = 0usize;
        let mut macro_map_bytes = 0usize;
        if let Some(providers) = providers {
            let mut remaining = usize::MAX;
            for provider in providers
                .iter()
                .filter(|p| p.macro_export_source_role().is_none())
            {
                if provider.macro_index().pending_producers().next().is_some() {
                    return Err(fault(
                        "completed runtime module macro provider remains pending",
                    ));
                }
                let source = provider.module_source();
                let index = provider.completed_runtime_index(providers, None, remaining)?;
                remaining = remaining
                    .checked_sub(index.runtime_bytes())
                    .ok_or_else(crate::runtime::arena::resource)?;
                let compiler = provider
                    .compiler
                    .as_ref()
                    .map(|catalog| catalog.owning_profile_identity())
                    .transpose()?;
                let fact = super::body_macro_environment::CompletedBodyMacroFacts::prepare(
                    source, index, compiler,
                )?;
                insert_macro_fact(
                    &sources,
                    &macro_sources,
                    &mut body_macros,
                    &mut macro_map_bytes,
                    &mut macro_admission_bytes,
                    source,
                    fact,
                )?;
            }
            for module in self
                .staged_modules
                .values()
                .filter(|m| m.namespace_complete && m.provider.macro_export_source_role().is_none())
            {
                let source = module.syntax.prepared_source();
                let inherited = self.module_projection_inherited(&module.syntax)?;
                let index = module.provider.completed_runtime_index(
                    providers,
                    Some(&inherited),
                    remaining,
                )?;
                remaining = remaining
                    .checked_sub(index.runtime_bytes())
                    .ok_or_else(crate::runtime::arena::resource)?;
                let compiler = module
                    .provider
                    .compiler
                    .as_ref()
                    .map(|catalog| catalog.owning_profile_identity())
                    .transpose()?;
                let fact = super::body_macro_environment::CompletedBodyMacroFacts::prepare(
                    source, index, compiler,
                )?;
                insert_macro_fact(
                    &sources,
                    &macro_sources,
                    &mut body_macros,
                    &mut macro_map_bytes,
                    &mut macro_admission_bytes,
                    source,
                    fact,
                )?;
            }
            if body_macros.len() != sources.len() {
                return Err(fault("completed module macro ownership count differs"));
            }
        }
        macro_source_bytes
            .checked_add(macro_map_bytes)
            .ok_or_else(crate::runtime::arena::resource)?;
        drop(macro_sources);
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
            .and_then(|n| n.checked_add(macro_map_bytes))
            .ok_or_else(crate::runtime::arena::resource)?;
        let source_syntax = self.prepare_completed_source_syntax(seal)?;
        let syntax_bytes = source_syntax.values().try_fold(0usize, |bytes, plan| {
            bytes
                .checked_add(plan.metadata_bytes())
                .and_then(|n| {
                    n.checked_add(
                        256 + 16
                            * std::mem::size_of::<(
                                Digest,
                                Arc<super::source_syntax::CompletedSourceSyntax>,
                            )>(),
                    )
                })
                .ok_or_else(crate::runtime::arena::resource)
        })?;
        let bytes = bytes
            .checked_add(syntax_bytes)
            .ok_or_else(crate::runtime::arena::resource)?;
        let names_bytes = self.graph.exclusive_retained_bytes()?;
        // Share only after all fallible preparation has passed. Subsequent
        // mutation is rejected by unique_graph; no COW allocation is possible.
        self.completed_source_environment = Some(Arc::new(CompletedSourceEnvironment {
            names: Arc::clone(&self.graph),
            sources: Arc::new(sources),
            scope_owners,
            body_macros,
            macro_admission_bytes,
            source_syntax,
            seal_dependency,
            bytes,
            names_bytes,
            sources_bytes,
            members: std::sync::OnceLock::new(),
            generated: std::sync::OnceLock::new(),
        }));
        Ok(())
    }
}

fn insert_macro_fact(
    sources: &[PreparedSourceView],
    owners: &BTreeMap<Digest, u32>,
    facts: &mut BTreeMap<Digest, (u32, super::body_macro_environment::CompletedBodyMacroFacts)>,
    map_bytes: &mut usize,
    admission_bytes: &mut usize,
    source: &PreparedSourceView,
    fact: super::body_macro_environment::CompletedBodyMacroFacts,
) -> Result<(), Diagnostic> {
    // Deterministic direct source inventory join; publication equality remains
    // actual authority rather than the locator digest alone.
    let key = QueryKey::Outline(source.selection().outline.clone()).fingerprint();
    let ordinal = *owners
        .get(&key)
        .ok_or_else(|| fault("completed macro source owner absent"))?;
    if !sources
        .get(ordinal as usize)
        .is_some_and(|owner| owner.same_publication(source))
    {
        return Err(fault("completed macro source publication differs"));
    }
    if facts.contains_key(&key) {
        return Err(fault("completed macro source key duplicated"));
    }
    *map_bytes = map_bytes
        .checked_add(
            256 + 16
                * std::mem::size_of::<(
                    Digest,
                    (u32, super::body_macro_environment::CompletedBodyMacroFacts),
                )>(),
        )
        .ok_or_else(crate::runtime::arena::resource)?;
    *admission_bytes = admission_bytes
        .checked_add(fact.index().runtime_bytes())
        .and_then(|n| n.checked_add(fact.index().exclusive_metadata_bytes().ok()?))
        .and_then(|n| n.checked_add(16 * std::mem::size_of::<usize>()))
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
        .ok_or_else(crate::runtime::arena::resource)?;
    facts.insert(key, (ordinal, fact));
    Ok(())
}
