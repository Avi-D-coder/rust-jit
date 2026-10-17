//! Owning source-only transport from one genuine completed configured world.
//! This is not a runtime namespace, a fabricated body or a producer evaluator.
use super::*;
use crate::{
    core::digest::{StableEncode, StableEncoder},
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        seals::SealKey,
    },
    expand::{
        declaration_fragment::CompletedDeclarationFragment,
        source_declarative::{SourceDeclarative, macro_exports::SourceMacroExportFacet},
    },
    frontend::{
        impl_index::{ArgumentDeclarationMacroIndex, DeclarationMacroIndex},
        source_macro_routes::SourceMacroRoutes,
    },
};
use std::sync::Arc;
const MAX_ROOTS: usize = 100_000;
const MAX_FACETS: usize = 256;
const MAX_BYTES: usize = 512 * 1024 * 1024;
const ARC: usize = 2 * std::mem::size_of::<usize>();
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-EXPORT-PUBLICATION", message.into())
}
fn add(bytes: &mut usize, amount: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(amount)
        .filter(|n| *n <= MAX_BYTES)
        .ok_or_else(crate::runtime::arena::resource)?;
    Ok(())
}
fn rows<T>(bytes: &mut usize, count: usize) -> Result<(), Diagnostic> {
    if count > MAX_ROOTS {
        return Err(crate::runtime::arena::resource());
    }
    add(
        bytes,
        count
            .checked_mul(
                std::mem::size_of::<T>()
                    .checked_mul(16)
                    .and_then(|n| n.checked_add(256))
                    .ok_or_else(crate::runtime::arena::resource)?,
            )
            .ok_or_else(crate::runtime::arena::resource)?,
    )
}
/// Actual completed frame parent links and catalogues, with scheduling state
/// deliberately absent. Every catalog is moved, never reparsed or copied.
pub(crate) struct SealedExportGeneratedChild {
    parent: Option<Digest>,
    order: Vec<u32>,
    catalog: Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>,
}
pub(crate) struct SealedExportGeneratedFrame {
    module: Option<Digest>,
    source_derive: Option<Arc<crate::service::source_derive_requests::SourceDeriveSelection>>,
    source_prefix: Option<Arc<SourceDeclarative>>,
    catalog: Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>,
    children: BTreeMap<Digest, SealedExportGeneratedChild>,
    child_order: BTreeMap<Vec<u32>, Digest>,
    ordinal: usize,
    call: crate::syntax::outline::ItemInvocation,
}
pub(crate) struct SealedExportModule {
    provider: Arc<SourceDeclarative>,
    syntax: StagedModuleSyntax,
    ordinal: usize,
    parse_job: Digest,
    catalogs: BTreeMap<Digest, Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>>,
    annotation_plan: Option<
        Arc<crate::expand::source_declarative::original_annotations::OriginalAnnotationPlan>,
    >,
}
pub(crate) struct SealedMacroExportProviders {
    imports: Option<Arc<SealedMacroExportProviders>>,
    providers: Vec<SourceDeclarative>,
    source_environment: Digest,
    engine: Digest,
    facets: Vec<Arc<SourceMacroExportFacet>>,
    declarations: BTreeMap<usize, Arc<DeclarationMacroIndex>>,
    argument_indexes: Vec<ArgumentDeclarationMacroIndex>,
    fragments: BTreeMap<Digest, Arc<CompletedDeclarationFragment>>,
    argument_fragments: Vec<Arc<CompletedDeclarationFragment>>,
    item_outputs:
        BTreeMap<Digest, Arc<crate::expand::generated_declarations::GeneratedDeclarations>>,
    generated: BTreeMap<Digest, SealedExportGeneratedFrame>,
    modules: BTreeMap<Digest, SealedExportModule>,
    records: BTreeMap<Digest, Arc<crate::expand::record::ExpansionRecord>>,
    names: Arc<NameNamespaces>,
    routes: Vec<Arc<SourceMacroRoutes>>,
    seal: SealKey,
    seal_value: Digest,
    guard: Arc<PreparedInputGuard>,
    identity: Digest,
    bytes: usize,
}
impl std::fmt::Debug for SealedMacroExportProviders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedMacroExportProviders")
            .field("seal", &self.seal)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}
impl SealedMacroExportProviders {
    pub(crate) fn source_environment(&self) -> Digest {
        self.source_environment
    }
    pub(crate) fn engine(&self) -> Digest {
        self.engine
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn seal(&self) -> &SealKey {
        &self.seal
    }
    pub(crate) fn seal_value(&self) -> Digest {
        self.seal_value
    }
    pub(crate) fn guard(&self) -> &Arc<PreparedInputGuard> {
        &self.guard
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        // This is a cold coordinator boundary, never an installed runtime
        // guard. Each ancestor retains its own actual provider arena.
        let mut owner = Some(self);
        let mut count = 0usize;
        while let Some(current) = owner {
            count = count
                .checked_add(1)
                .filter(|count| *count <= 256)
                .ok_or_else(crate::runtime::arena::resource)?;
            current.guard.require_current(db)?;
            owner = current.imports.as_deref();
        }
        Ok(())
    }
    pub(crate) fn imports(&self) -> Option<&Arc<SealedMacroExportProviders>> {
        self.imports.as_ref()
    }
    pub(crate) fn providers(&self) -> &[SourceDeclarative] {
        &self.providers
    }
    pub(crate) fn facets(&self) -> &[Arc<SourceMacroExportFacet>] {
        &self.facets
    }
    pub(crate) fn names(&self) -> &Arc<NameNamespaces> {
        &self.names
    }
    pub(crate) fn routes(&self) -> &[Arc<SourceMacroRoutes>] {
        &self.routes
    }
    pub(crate) fn declarations(&self) -> &BTreeMap<usize, Arc<DeclarationMacroIndex>> {
        &self.declarations
    }
    pub(crate) fn argument_indexes(&self) -> &[ArgumentDeclarationMacroIndex] {
        &self.argument_indexes
    }
    pub(crate) fn fragments(&self) -> impl Iterator<Item = &Arc<CompletedDeclarationFragment>> {
        self.fragments.values().chain(&self.argument_fragments)
    }
    pub(crate) fn generated(&self) -> &BTreeMap<Digest, SealedExportGeneratedFrame> {
        &self.generated
    }
    pub(crate) fn modules(&self) -> &BTreeMap<Digest, SealedExportModule> {
        &self.modules
    }
    pub(crate) fn item_outputs(
        &self,
    ) -> &BTreeMap<Digest, Arc<crate::expand::generated_declarations::GeneratedDeclarations>> {
        &self.item_outputs
    }
    pub(crate) fn records(&self) -> &BTreeMap<Digest, Arc<crate::expand::record::ExpansionRecord>> {
        &self.records
    }
}
impl SealedExportGeneratedFrame {
    pub(crate) fn module(&self) -> Option<Digest> {
        self.module
    }
    pub(crate) fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub(crate) fn catalog(&self) -> &Arc<crate::frontend::generated_macros::GeneratedMacroCatalog> {
        &self.catalog
    }
    pub(crate) fn source_prefix(&self) -> Option<&Arc<SourceDeclarative>> {
        self.source_prefix.as_ref()
    }
    pub(crate) fn source_derive(
        &self,
    ) -> Option<&Arc<crate::service::source_derive_requests::SourceDeriveSelection>> {
        self.source_derive.as_ref()
    }
    pub(crate) fn call(&self) -> &crate::syntax::outline::ItemInvocation {
        &self.call
    }
    pub(crate) fn child_order(&self) -> &BTreeMap<Vec<u32>, Digest> {
        &self.child_order
    }
    pub(crate) fn children(&self) -> &BTreeMap<Digest, SealedExportGeneratedChild> {
        &self.children
    }
}
impl SealedExportGeneratedChild {
    pub(crate) fn parent(&self) -> Option<Digest> {
        self.parent
    }
    pub(crate) fn order(&self) -> &[u32] {
        &self.order
    }
    pub(crate) fn catalog(&self) -> &Arc<crate::frontend::generated_macros::GeneratedMacroCatalog> {
        &self.catalog
    }
}
impl SealedExportModule {
    pub(crate) fn provider(&self) -> &Arc<SourceDeclarative> {
        &self.provider
    }
    pub(crate) fn source(&self) -> &PreparedSourceView {
        self.syntax.prepared_source()
    }
    pub(crate) fn syntax_identity(&self) -> Digest {
        self.syntax.identity()
    }
    pub(crate) fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub(crate) fn parse_job(&self) -> Digest {
        self.parse_job
    }
    pub(crate) fn catalogs(
        &self,
    ) -> &BTreeMap<Digest, Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>> {
        &self.catalogs
    }
}
impl SourceDiscovery {
    /// Transfer only after every fallible authority/resource check succeeds.
    /// On NeedInput/error all provider ownership remains with this discovery.
    pub(crate) fn take_completed_macro_export_providers(
        &mut self,
        db: &mut Database,
        facets: &[Arc<SourceMacroExportFacet>],
    ) -> QueryPoll<Arc<SealedMacroExportProviders>> {
        self.take_export_providers_with_limit(db, facets, MAX_BYTES)
    }
    #[cfg(test)]
    pub(crate) fn take_completed_macro_export_providers_with_limit(
        &mut self,
        db: &mut Database,
        facets: &[Arc<SourceMacroExportFacet>],
        maximum: usize,
    ) -> QueryPoll<Arc<SealedMacroExportProviders>> {
        self.take_export_providers_with_limit(db, facets, maximum.min(MAX_BYTES))
    }
    fn take_export_providers_with_limit(
        &mut self,
        db: &mut Database,
        facets: &[Arc<SourceMacroExportFacet>],
        maximum: usize,
    ) -> QueryPoll<Arc<SealedMacroExportProviders>> {
        macro_rules! ready {
            ($x:expr) => {
                match $x {
                    QueryPoll::Ready(Ok(v)) => v,
                    QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                    QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
                }
            };
        }
        if let Err(error) = db.require_source_preparation() {
            return QueryPoll::Ready(Err(error));
        }
        if let Err(error) = self.sealed_expansions() {
            return QueryPoll::Ready(Err(error));
        }
        if self.namespace != Namespace::Macro
            || facets.is_empty()
            || facets.len() > MAX_FACETS
            || facets.len() != self.sources.len()
        {
            return QueryPoll::Ready(Err(invalid(
                "sealed export providers require the exact configured role source set",
            )));
        }
        let selected = facets
            .iter()
            .filter(|facet| facet.source().outline().root_scope == self.scope)
            .collect::<Vec<_>>();
        let [selected] = selected.as_slice() else {
            return QueryPoll::Ready(Err(invalid(
                "sealed export selected root is absent or ambiguous",
            )));
        };
        let view = ready!(self.completed_macro_export_facet(db, selected));
        let seal_value = view.dependencies()[2].clone();
        let DependencyStamp::Query {
            key,
            fingerprint: seal_value,
        } = seal_value
        else {
            return QueryPoll::Ready(Err(invalid("sealed export completion dependency differs")));
        };
        drop(view);
        let guard = Arc::new(ready!(db.prepare_input_guard(
            &SyntaxProvider,
            &key,
            seal_value
        )));
        let prepared = self.prepare_sealed_export_transfer(db, facets, &guard, seal_value, maximum);
        let (bytes, identity, routes, retained_facets) = match prepared {
            Ok(value) => value,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if let Err(error) = guard.require_current(db) {
            return QueryPoll::Ready(Err(error));
        }
        // Nothing below can return a recoverable error or invoke a provider.
        let (source_environment, engine, providers) = self
            .declarative
            .take()
            .expect("preflight retained actual providers");
        let generated = std::mem::take(&mut self.fragments)
            .into_iter()
            .map(|(key, frame)| {
                let children = frame
                    .children
                    .into_iter()
                    .map(|(key, child)| {
                        (
                            key,
                            SealedExportGeneratedChild {
                                parent: child.parent,
                                order: child.order,
                                catalog: Arc::new(child.catalog),
                            },
                        )
                    })
                    .collect();
                (
                    key,
                    SealedExportGeneratedFrame {
                        module: frame.module,
                        source_derive: frame.source_derive,
                        source_prefix: frame.source_prefix,
                        catalog: Arc::new(frame.catalog),
                        children,
                        child_order: frame.child_order,
                        ordinal: frame.ordinal,
                        call: frame.call,
                    },
                )
            })
            .collect();
        let modules = std::mem::take(&mut self.staged_modules)
            .into_iter()
            .map(|(key, module)| {
                (
                    key,
                    SealedExportModule {
                        provider: Arc::new(module.provider),
                        syntax: module.syntax,
                        ordinal: module.ordinal,
                        parse_job: module.parse_job,
                        catalogs: module.catalogs,
                        annotation_plan: module.annotation_plan,
                    },
                )
            })
            .collect();
        let result = Arc::new(SealedMacroExportProviders {
            imports: self.exports.take(),
            providers,
            source_environment,
            engine,
            facets: retained_facets,
            declarations: std::mem::take(&mut self.nested_sources),
            argument_indexes: std::mem::take(&mut self.completed_argument_indexes),
            fragments: std::mem::take(&mut self.nested_completed),
            argument_fragments: std::mem::take(&mut self.completed_argument_fragments),
            item_outputs: std::mem::take(&mut self.item_declaration_inputs),
            generated,
            modules,
            records: std::mem::take(&mut self.records),
            names: Arc::clone(&self.graph),
            routes,
            seal: self.key.clone(),
            seal_value,
            guard,
            identity,
            bytes,
        });
        self.failed = Some(Diagnostic::new(
            "LRR-E-MACRO-EXPORT-CONSUMED",
            "sealed macro export provider ownership was transferred".into(),
        ));
        QueryPoll::Ready(Ok(result))
    }
    fn prepare_sealed_export_transfer(
        &self,
        db: &mut Database,
        facets: &[Arc<SourceMacroExportFacet>],
        guard: &Arc<PreparedInputGuard>,
        seal_value: Digest,
        maximum: usize,
    ) -> Result<
        (
            usize,
            Digest,
            Vec<Arc<SourceMacroRoutes>>,
            Vec<Arc<SourceMacroExportFacet>>,
        ),
        Diagnostic,
    > {
        let (source_environment, engine, providers) = self
            .declarative
            .as_ref()
            .ok_or_else(|| invalid("sealed export provider ownership is absent"))?;
        if *engine != crate::expand::source_declarative::engine_version()
            || providers.len() != self.sources.len()
            || self.macro_export_sources.len() != providers.len()
        {
            return Err(invalid(
                "sealed export provider source/engine ownership differs",
            ));
        }
        let mut bytes = std::mem::size_of::<SealedMacroExportProviders>();
        if let Some(imports) = &self.exports {
            if imports.seal() == &self.key
                || imports.seal().unit_graph != self.key.unit_graph
                || imports.seal().epoch != self.epoch
            {
                return Err(invalid(
                    "sealed export imported owner graph or epoch differs",
                ));
            }
            // Reserve the new owner in the same ancestry limit the borrowed
            // definition service enforces; immutable ancestors cannot cycle.
            let mut count = 1usize;
            let mut parent = Some(imports.as_ref());
            while let Some(current) = parent {
                count = count
                    .checked_add(1)
                    .filter(|count| *count <= 256)
                    .ok_or_else(crate::runtime::arena::resource)?;
                parent = current.imports.as_deref();
            }
            imports.require_current(db)?;
            add(&mut bytes, imports.metadata_bytes())?;
        }
        add(&mut bytes, guard.metadata_bytes())?;
        add(&mut bytes, ARC * 3)?;
        rows::<Arc<SourceMacroExportFacet>>(&mut bytes, facets.len())?;
        let Some((InputValue::DiscoverySeal(actual), _)) =
            db.published_input(&InputKey::DiscoverySeal(self.key.clone()))
        else {
            return Err(invalid("sealed export actual discovery publication absent"));
        };
        rows::<(InputKey, Digest)>(&mut bytes, actual.compiler_inputs.len())?;
        let compiler_inputs: BTreeMap<_, _> = actual
            .compiler_inputs
            .iter()
            .map(|(key, fp)| (key.clone(), *fp))
            .collect();
        if compiler_inputs.len() != actual.compiler_inputs.len() {
            return Err(invalid(
                "sealed export actual compiler dependencies duplicated",
            ));
        }
        let mut locations = BTreeSet::new();
        let mut seen_states: Vec<PreparedSourceView> = Vec::new();
        let state_limit = MAX_FACETS
            .checked_add(self.staged_modules.len())
            .filter(|n| *n <= MAX_ROOTS)
            .ok_or_else(crate::runtime::arena::resource)?;
        rows::<PreparedSourceView>(&mut bytes, state_limit)?;
        seen_states
            .try_reserve_exact(state_limit)
            .map_err(|_| crate::runtime::arena::resource())?;
        for facet in facets {
            if facet.unit_graph() != self.key.unit_graph
                || facet.preparation() != facets[0].preparation()
                || !locations.insert(facet.source().selection().location_identity())
            {
                return Err(invalid(
                    "sealed export facets graph/preparation/location differs",
                ));
            }
            let actual = self
                .sources
                .iter()
                .enumerate()
                .find(|(_, source)| {
                    source.selection() == facet.source().selection()
                        && source.same_publication(facet.source())
                })
                .ok_or_else(|| invalid("sealed export facet actual provider source absent"))?;
            let provider = &providers[actual.0];
            if provider.module_source().selection() != facet.source().selection()
                || !provider.module_source().same_publication(facet.source())
                || provider.macro_index().pending_producers().next().is_some()
            {
                return Err(invalid(
                    "sealed export actual provider source/completion differs",
                ));
            }
            let role = provider
                .macro_export_source_role()
                .ok_or_else(|| invalid("sealed export actual provider role absent"))?;
            role.require_facet(facet)?;
            let deps = db.macro_export_source_dependencies(facet)?;
            if &deps != role.dependencies() {
                return Err(invalid(
                    "sealed export facet selected source/environment changed",
                ));
            }
            for dependency in &deps {
                let DependencyStamp::Input { key, fingerprint } = dependency else {
                    return Err(invalid("sealed export source dependency is not an input"));
                };
                if compiler_inputs.get(key) != Some(fingerprint) {
                    return Err(invalid(
                        "sealed export source dependency absent from published closure",
                    ));
                }
            }
            add(&mut bytes, facet.exclusive_metadata_bytes())?;
            add(&mut bytes, facet.provider_metadata_bytes())?;
            add(&mut bytes, facet.routes_metadata_bytes())?;
        }
        let mut source_bytes = |source: &PreparedSourceView| -> Result<usize, Diagnostic> {
            let handles = source
                .selection()
                .metadata_bytes()?
                .checked_mul(2)
                .ok_or_else(crate::runtime::arena::resource)?;
            if seen_states
                .iter()
                .any(|prior| prior.same_publication(source))
            {
                return Ok(handles);
            }
            let owner = facets
                .iter()
                .find(|facet| facet.source().same_publication(source));
            let bytes = if let Some(facet) = owner {
                facet.source_retained_admission_bytes()
            } else {
                self.staged_modules
                    .values()
                    .find(|module| module.syntax.prepared_source().same_publication(source))
                    .map(|module| module.syntax.metadata_bytes())
                    .ok_or_else(|| {
                        invalid("sealed export retained provider source allocation owner absent")
                    })?
            };
            seen_states.push(source.share());
            bytes
                .checked_add(handles)
                .ok_or_else(crate::runtime::arena::resource)
        };
        rows::<SourceDeclarative>(&mut bytes, providers.len())?;
        for provider in providers {
            add(
                &mut bytes,
                provider.retained_admission_bytes(MAX_BYTES, &mut source_bytes)?,
            )?;
        }
        rows::<(usize, Arc<DeclarationMacroIndex>)>(&mut bytes, self.nested_sources.len())?;
        for index in self.nested_sources.values() {
            if !index.is_complete() {
                return Err(invalid("sealed export declaration index is incomplete"));
            }
            add(&mut bytes, index.retained_metadata_bytes())?;
        }
        rows::<ArgumentDeclarationMacroIndex>(&mut bytes, self.completed_argument_indexes.len())?;
        for index in &self.completed_argument_indexes {
            if !index.is_complete() {
                return Err(invalid("sealed export argument index is incomplete"));
            }
            add(&mut bytes, index.metadata_bytes())?;
        }
        rows::<(Digest, Arc<CompletedDeclarationFragment>)>(
            &mut bytes,
            self.nested_completed.len(),
        )?;
        rows::<Arc<CompletedDeclarationFragment>>(
            &mut bytes,
            self.completed_argument_fragments.len(),
        )?;
        for fragment in self
            .nested_completed
            .values()
            .chain(&self.completed_argument_fragments)
        {
            add(&mut bytes, fragment.metadata_bytes())?;
        }
        rows::<(
            Digest,
            Arc<crate::expand::generated_declarations::GeneratedDeclarations>,
        )>(&mut bytes, self.item_declaration_inputs.len())?;
        for output in self.item_declaration_inputs.values() {
            add(&mut bytes, output.metadata_bytes()?)?;
        }
        rows::<(Digest, Arc<crate::expand::record::ExpansionRecord>)>(
            &mut bytes,
            self.records.len(),
        )?;
        for record in self.records.values() {
            add(&mut bytes, record.estimated_bytes()?)?;
        }
        rows::<(Digest, SealedExportGeneratedFrame)>(&mut bytes, self.fragments.len())?;
        for frame in self.fragments.values() {
            if frame.active_child.is_some()
                || frame.ordinal >= providers.len()
                || frame
                    .module
                    .is_some_and(|key| !self.staged_modules.contains_key(&key))
            {
                return Err(invalid("sealed export generated frame owner is incomplete"));
            }
            add(&mut bytes, frame.call.scope.heap_bytes())?;
            add(&mut bytes, frame.catalog.metadata_bytes()?)?;
            add(&mut bytes, ARC)?;
            if let Some(prefix) = &frame.source_prefix {
                add(
                    &mut bytes,
                    prefix.retained_admission_bytes(MAX_BYTES, &mut source_bytes)?,
                )?;
            }
            if let Some(derive) = &frame.source_derive {
                add(&mut bytes, derive.metadata_bytes()?)?;
            }
            rows::<(Digest, SealedExportGeneratedChild)>(&mut bytes, frame.children.len())?;
            if frame.child_order.len() != frame.children.len()
                || frame.child_order.iter().any(|(order, key)| {
                    frame
                        .children
                        .get(key)
                        .is_none_or(|child| &child.order != order)
                })
            {
                return Err(invalid("sealed export generated child ordering differs"));
            }
            rows::<(Vec<u32>, Digest)>(&mut bytes, frame.child_order.len())?;
            for order in frame.child_order.keys() {
                add(
                    &mut bytes,
                    order
                        .capacity()
                        .checked_mul(std::mem::size_of::<u32>())
                        .ok_or_else(crate::runtime::arena::resource)?,
                )?;
            }
            for child in frame.children.values() {
                if child
                    .parent
                    .is_some_and(|key| !frame.children.contains_key(&key))
                {
                    return Err(invalid("sealed export generated child parent absent"));
                }
                add(&mut bytes, child.catalog.metadata_bytes()?)?;
                add(&mut bytes, ARC)?;
                add(
                    &mut bytes,
                    child
                        .order
                        .capacity()
                        .checked_mul(std::mem::size_of::<u32>())
                        .ok_or_else(crate::runtime::arena::resource)?,
                )?;
            }
        }
        rows::<(Digest, SealedExportModule)>(&mut bytes, self.staged_modules.len())?;
        for module in self.staged_modules.values() {
            if !module.namespace_complete || module.ordinal >= providers.len() {
                return Err(invalid("sealed export staged module owner is incomplete"));
            }
            add(
                &mut bytes,
                module
                    .provider
                    .retained_admission_bytes(MAX_BYTES, &mut source_bytes)?,
            )?;
            add(&mut bytes, module.syntax.metadata_bytes())?;
            add(&mut bytes, ARC)?;
            if let Some(plan) = &module.annotation_plan {
                add(&mut bytes, plan.metadata_bytes())?;
            }
            rows::<(
                Digest,
                Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>,
            )>(&mut bytes, module.catalogs.len())?;
            for catalog in module.catalogs.values() {
                add(&mut bytes, catalog.metadata_bytes()?)?;
            }
        }
        add(&mut bytes, self.graph.exclusive_retained_bytes()?)?;
        let routes = Self::completed_macro_routes(providers, &self.module_routes)?;
        rows::<Arc<SourceMacroRoutes>>(&mut bytes, routes.capacity())?;
        for route in &routes {
            add(&mut bytes, route.metadata_bytes()?)?;
        }
        let mut retained_facets = Vec::new();
        retained_facets
            .try_reserve_exact(facets.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        add(
            &mut bytes,
            retained_facets
                .capacity()
                .checked_mul(std::mem::size_of::<Arc<SourceMacroExportFacet>>())
                .ok_or_else(crate::runtime::arena::resource)?,
        )?;
        retained_facets.extend(facets.iter().cloned());
        let mut e = StableEncoder::new("lrr/v2/owning-sealed-macro-export-providers/imports");
        e.option(&self.exports, |e, imports| e.digest(imports.identity()));
        e.record(&self.key);
        e.digest(*source_environment);
        e.digest(*engine);
        e.digest(seal_value);
        e.digest(guard.fingerprint());
        e.sequence(facets, |e, facet| e.digest(facet.identity()));
        e.sequence(providers, |e, provider| {
            e.string(provider.module_source().selection().path.as_str());
            e.record(&provider.module_source().selection().outline);
        });
        e.u64(self.nested_sources.len() as u64);
        for (ordinal, index) in &self.nested_sources {
            e.u64(*ordinal as u64);
            e.digest(index.scan().fingerprint());
        }
        e.u64(self.fragments.len() as u64);
        for (key, frame) in &self.fragments {
            e.digest(*key);
            e.option(&frame.module, |e, key| e.digest(*key));
            e.u64(frame.ordinal as u64);
            e.digest(frame.catalog.record_identity());
            e.u64(frame.children.len() as u64);
            for (key, child) in &frame.children {
                e.digest(*key);
                e.option(&child.parent, |e, key| e.digest(*key));
                e.sequence(&child.order, |e, n| e.u32(*n));
                e.digest(child.catalog.record_identity());
            }
        }
        e.u64(self.staged_modules.len() as u64);
        for (key, module) in &self.staged_modules {
            e.digest(*key);
            e.digest(module.syntax.identity());
            e.u64(module.ordinal as u64);
            e.digest(module.parse_job);
        }
        if bytes > maximum {
            return Err(crate::runtime::arena::resource());
        }
        Ok((bytes, e.finish(), routes, retained_facets))
    }
}
