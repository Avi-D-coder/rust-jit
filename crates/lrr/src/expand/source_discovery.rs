//! Captured-source scope discovery using the existing ordinary import fixed point.
use super::discovery::{
    Discovery, DiscoveryJob, DiscoveryKind, DiscoveryLimits, DiscoveryStep, DiscoveryUpdate,
    JobTicket,
};
#[path = "source_discovery/macro_export_modules.rs"]
mod macro_export_modules;
#[path = "source_discovery/macro_export_view.rs"]
mod macro_export_view;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::seals::{Seal, SealDomain},
    frontend::names::{ImportResolution, NameLookup, NameNamespaces},
    source::{changes::PreparedSourceView, manifest::Namespace},
    syntax::outline::{ScopeKey, TextualEventKind},
};
pub use macro_export_view::SealedMacroExportView;
use std::collections::{BTreeMap, BTreeSet};
#[path = "source_discovery/body_macro_environment.rs"]
mod body_macro_environment;
pub use body_macro_environment::SealedBodyMacroEnvironment;
#[path = "source_discovery/completed_source_environment.rs"]
mod completed_source_environment;
pub use completed_source_environment::CompletedSourceEnvironment;
pub(crate) use completed_source_environment::SourceScopeOwner;
#[path = "source_discovery/compiler_preparation.rs"]
mod compiler_preparation;
#[path = "source_discovery/compiler_profiles.rs"]
mod compiler_profiles;
pub(crate) use compiler_preparation::SourceCompilerPreparation;
#[path = "source_discovery/original_modules.rs"]
mod original_modules;
use original_modules::{OrdinaryModuleOwner, OriginalModuleDeclaration};
#[path = "source_discovery/impl_catalog.rs"]
pub(crate) mod impl_catalog;
#[path = "source_discovery/item_boundaries.rs"]
mod item_boundaries;
#[path = "source_discovery/item_impl_children.rs"]
mod item_impl_children;
#[path = "source_discovery/item_impl_completion.rs"]
mod item_impl_completion;
#[path = "source_discovery/original_inert.rs"]
mod original_inert;
#[path = "source_discovery/source_syntax.rs"]
mod source_syntax;
pub(crate) use source_syntax::CompletedSourceSyntax;
#[path = "source_discovery/sealed_macro_export_providers.rs"]
mod sealed_macro_export_providers;
pub(crate) use sealed_macro_export_providers::SealedMacroExportProviders;
#[path = "source_discovery/type_scope_receipts.rs"]
mod type_scope_receipts;
pub(crate) use type_scope_receipts::TypeScopeReceipt;
fn unique_graph(
    graph: &mut std::sync::Arc<NameNamespaces>,
) -> Result<&mut NameNamespaces, Diagnostic> {
    std::sync::Arc::get_mut(graph)
        .ok_or_else(|| fault("completed source namespace graph is immutable"))
}
pub enum SourceDiscoveryStep {
    NeedSourceDeriveSelection {
        selection: std::sync::Arc<crate::service::source_derive_requests::SourceDeriveSelection>,
    },
    NeedSourceDeriveExpansion {
        request: std::sync::Arc<crate::service::function_macro_requests::PreparedFunctionRequest>,
    },
    NeedSourceDeriveAppend {
        request: std::sync::Arc<crate::service::function_macro_requests::PreparedFunctionRequest>,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    },
    NeedDeriveAppend {
        request: std::sync::Arc<super::generated_call_request::GeneratedCallRequest>,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    },
    NeedDeriveDeclaration {
        request:
            std::sync::Arc<crate::frontend::generated_derive_exports::DeriveDeclarationRequest>,
    },
    NeedAttributeSelection {
        selection:
            std::sync::Arc<crate::service::source_attribute_requests::SourceAttributeSelection>,
    },
    NeedProceduralSelection {
        selection: std::sync::Arc<ProceduralSourceSelection>,
    },
    NeedExpansion {
        request: std::sync::Arc<super::source_declarative::SourceDeclarativeRequest>,
    },
    NeedGeneratedExpansion {
        request: std::sync::Arc<super::generated_call_request::GeneratedCallRequest>,
    },
    Sealed(Seal),
    Incomplete {
        diagnostic: Diagnostic,
        blocked: Vec<(Digest, Vec<Digest>)>,
    },
}
pub struct ProceduralSourceSelection {
    module: Option<Digest>,
    nested: bool,
    input: crate::service::function_macro_requests::FunctionInvocationInput,
    definition: crate::expand::scopes::MacroDefinition,
    catalog: std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
}
impl ProceduralSourceSelection {
    pub(crate) fn generated(
        input: crate::service::function_macro_requests::FunctionInvocationInput,
        definition: crate::expand::scopes::MacroDefinition,
        catalog: std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
    ) -> Self {
        Self {
            module: None,
            nested: true,
            input,
            definition,
            catalog,
        }
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.input
            .metadata_bytes()?
            .checked_add(256)
            .ok_or_else(|| fault("procedural selection size overflow"))
    }
    pub(crate) fn revalidate_generated(
        &self,
        provider: &super::source_declarative::SourceDeclarative,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        owners: &[super::source_declarative::SourceDeclarative],
    ) -> Result<(), Diagnostic> {
        use crate::service::function_macro_requests::CapturedDeclarationParent;
        let parent = self
            .input
            .captured_parent()
            .ok_or_else(|| fault("captured selection parent absent"))?;
        let actual = match parent {
            CapturedDeclarationParent::Generated(index) => provider
                .generated_procedural_selection(
                    index.clone(),
                    self.input.invocation_identity(),
                    namespaces,
                    owners,
                )?,
            CapturedDeclarationParent::Argument(index) => provider.argument_procedural_selection(
                index.clone(),
                self.input.invocation_identity(),
                namespaces,
                owners,
            )?,
        }
        .ok_or_else(|| fault("captured procedural selection unresolved"))?;
        if actual.definition != self.definition
            || actual.catalog.fingerprint() != self.catalog.fingerprint()
        {
            return Err(fault("generated procedural selection changed"));
        }
        Ok(())
    }

    pub(crate) fn validates_binding(
        &self,
        key: &super::identity::ExpansionKey,
    ) -> Result<bool, Diagnostic> {
        let Some(export) = self
            .catalog
            .select(&self.definition, super::identity::ProceduralKind::Function)?
        else {
            return Ok(false);
        };
        Ok(key.invocation == self.input.invocation_identity()
            && key.grammar == self.input.grammar()
            && &key.macro_identity == export.identity()
            && key.consumer_compile_env == self.catalog.environment()
            && key.procedural_epoch == Some(self.catalog.epoch()))
    }
    pub fn input(&self) -> &crate::service::function_macro_requests::FunctionInvocationInput {
        &self.input
    }
    pub fn prepare(
        &self,
        session: &crate::service::macro_session::MacroSession,
        context: &crate::service::expansion::DemandContext,
        target: crate::service::item_macro_requests::ResolvedItemTarget<'_>,
    ) -> Result<crate::service::function_macro_requests::PreparedFunctionRequest, Diagnostic> {
        if session.export_catalog(context)?.fingerprint() != self.catalog.fingerprint() {
            return Err(fault("procedural selection catalog differs from session"));
        }
        self.input
            .resolve_definition(session, context, &self.definition, target)
    }
    /// `prepare` with the export and definition site the frozen session selects.
    pub fn prepare_selected(
        &self,
        session: &crate::service::macro_session::MacroSession,
        context: &crate::service::expansion::DemandContext,
    ) -> Result<crate::service::function_macro_requests::PreparedFunctionRequest, Diagnostic> {
        if session.export_catalog(context)?.fingerprint() != self.catalog.fingerprint() {
            return Err(fault("procedural selection catalog differs from session"));
        }
        self.input
            .resolve_selected(session, context, &self.definition)
    }
}
impl SourceDiscoveryStep {
    pub(crate) fn compiler_key(&self) -> Option<&crate::expand::identity::ExpansionKey> {
        match self {
            Self::NeedExpansion { request } if request.is_compiler() => Some(request.key()),
            Self::NeedGeneratedExpansion { request } if request.is_compiler() => {
                Some(request.key())
            }
            _ => None,
        }
    }
}
struct GeneratedCompilerScope<'a> {
    catalog: &'a crate::frontend::generated_macros::GeneratedMacroCatalog,
    captured: super::source_declarative::CapturedMacroContext<'a>,
    ancestors: Vec<&'a crate::frontend::generated_macros::GeneratedMacroCatalog>,
    inherited: Option<super::source_declarative::InheritedMacroContext<'a>>,
}
impl<'a> GeneratedCompilerScope<'a> {
    fn inherited_for_module(
        &self,
        producer: Digest,
    ) -> Result<super::source_declarative::InheritedMacroContext<'a>, Diagnostic> {
        let mut inherited = super::source_declarative::InheritedMacroContext::for_module(
            producer,
            self.catalog,
            &self.ancestors,
            self.captured,
        )?;
        if let Some(parent) = &self.inherited {
            inherited.extend_context(parent)?;
        }
        Ok(inherited)
    }
    fn compiler_context(&self) -> super::source_declarative::CapturedMacroContext<'_> {
        super::source_declarative::CapturedMacroContext {
            inherited: self.inherited.as_ref(),
            ..self.captured
        }
    }
}
struct GeneratedChild {
    parent: Option<Digest>,
    order: Vec<u32>,
    catalog: crate::frontend::generated_macros::GeneratedMacroCatalog,
    release: BTreeSet<Digest>,
}
struct GeneratedFragment {
    module: Option<Digest>,
    source_derive:
        Option<std::sync::Arc<crate::service::source_derive_requests::SourceDeriveSelection>>,
    source_prefix: Option<std::sync::Arc<super::source_declarative::SourceDeclarative>>,
    catalog: crate::frontend::generated_macros::GeneratedMacroCatalog,
    children: BTreeMap<Digest, GeneratedChild>,
    child_order: BTreeMap<Vec<u32>, Digest>,
    active_child: Option<Digest>,
    jobs: Vec<DiscoveryJob>,
    ordinal: usize,
    call: crate::syntax::outline::ItemInvocation,
    release: BTreeSet<Digest>,
    members: Vec<Digest>,
    dependencies: Vec<(Digest, Digest)>,
}
impl GeneratedFragment {
    fn completion_update(
        &self,
        extra_members: usize,
        extra_dependencies: usize,
    ) -> Result<DiscoveryUpdate, Diagnostic> {
        let mut update = DiscoveryUpdate::default();
        let members = self
            .members
            .len()
            .checked_add(extra_members)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| fault("fragment member count overflow"))?;
        let dependencies = self
            .dependencies
            .len()
            .checked_add(extra_dependencies)
            .ok_or_else(|| fault("fragment dependency count overflow"))?;
        update
            .members
            .try_reserve_exact(members)
            .map_err(|_| fault("fragment completion member allocation"))?;
        update
            .dependencies
            .try_reserve_exact(dependencies)
            .map_err(|_| fault("fragment completion dependency allocation"))?;
        update
            .jobs
            .try_reserve_exact(self.jobs.len())
            .map_err(|_| fault("fragment job update allocation"))?;
        update.jobs.extend_from_slice(&self.jobs);
        update.members.extend_from_slice(&self.members);
        if self.source_derive.is_none() {
            update.members.push(self.call.item_input);
        }
        update.dependencies.extend_from_slice(&self.dependencies);
        Ok(update)
    }
}
type ModuleBinding<'a> = (
    &'a crate::frontend::generated_headers::GeneratedModuleSource,
    &'a std::sync::Arc<super::record::ExpansionRecord>,
    &'a PreparedSourceView,
    &'a crate::syntax::token_wire::SpanRecord,
);
/// Readiness of source producers and the requested namespace's imports, not a scope seal.
pub enum ModuleSourceReadiness {
    Pending { waiting: Vec<Digest> },
    Ready(Box<ReadyModuleSource>),
}
pub enum OrdinaryModuleReadiness {
    Pending { waiting: Vec<Digest> },
    Ready(Box<ReadyOrdinaryModuleSource>),
}
pub struct ReadyOrdinaryModuleSource {
    identity: Digest,
    syntax: std::sync::Arc<crate::source::ordinary_module::OrdinaryModuleSyntaxInput>,
    macros: crate::frontend::source_macros::PublishedMacroIndex,
    namespace: Namespace,
    records: Vec<super::record::ExpansionDescriptor>,
}
impl ReadyOrdinaryModuleSource {
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn syntax(&self) -> &crate::source::ordinary_module::OrdinaryModuleSyntaxInput {
        &self.syntax
    }
    pub fn macros(&self) -> &crate::frontend::source_macros::PublishedMacroIndex {
        &self.macros
    }
    pub fn namespace(&self) -> Namespace {
        self.namespace
    }
    pub fn records(&self) -> &[super::record::ExpansionDescriptor] {
        &self.records
    }
}
enum ProjectionReadiness {
    Pending { waiting: Vec<Digest> },
    Ready(Box<ReadyProjection>),
}
struct ReadyProjection {
    identity: Digest,
    syntax: StagedModuleSyntax,
    macros: crate::frontend::source_macros::PublishedMacroIndex,
    namespace: Namespace,
    records: Vec<super::record::ExpansionDescriptor>,
}
pub struct ReadyModuleSource {
    identity: Digest,
    syntax: std::sync::Arc<crate::source::module_syntax::ModuleSyntaxInput>,
    macros: crate::frontend::source_macros::PublishedMacroIndex,
    namespace: Namespace,
    records: Vec<super::record::ExpansionDescriptor>,
}
impl ReadyModuleSource {
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn syntax(&self) -> &crate::source::module_syntax::ModuleSyntaxInput {
        &self.syntax
    }
    pub fn macros(&self) -> &crate::frontend::source_macros::PublishedMacroIndex {
        &self.macros
    }
    pub fn namespace(&self) -> Namespace {
        self.namespace
    }
    pub fn records(&self) -> &[super::record::ExpansionDescriptor] {
        &self.records
    }
}
pub struct ModuleSourceProgress {
    pub parsed: bool,
    pub declarations: usize,
    pub scheduled_imports: usize,
    pub namespace: Namespace,
    pub completed_import_facets: usize,
    pub scheduled_producers: usize,
    pub completed_invocations: usize,
}
#[derive(Clone)]
enum StagedModuleSyntax {
    Generated(std::sync::Arc<crate::source::module_syntax::ModuleSyntaxInput>),
    Ordinary {
        syntax: std::sync::Arc<crate::source::ordinary_module::OrdinaryModuleSyntaxInput>,
        parent: OrdinaryModuleOwner,
    },
}
impl StagedModuleSyntax {
    fn identity(&self) -> Digest {
        match self {
            Self::Generated(syntax) => syntax.identity(),
            Self::Ordinary { syntax, .. } => syntax.identity(),
        }
    }
    fn prepared_source(&self) -> &PreparedSourceView {
        match self {
            Self::Generated(syntax) => syntax.prepared_source(),
            Self::Ordinary { syntax, .. } => syntax.prepared_source(),
        }
    }
    fn metadata_bytes(&self) -> usize {
        match self {
            Self::Generated(syntax) => syntax.metadata_bytes(),
            Self::Ordinary { syntax, .. } => syntax.metadata_bytes(),
        }
    }
    fn generated(
        &self,
    ) -> Result<&std::sync::Arc<crate::source::module_syntax::ModuleSyntaxInput>, Diagnostic> {
        match self {
            Self::Generated(syntax) => Ok(syntax),
            Self::Ordinary { .. } => Err(fault(
                "ordinary module catalogue completion requires source parent context",
            )),
        }
    }
    fn producer(&self) -> Digest {
        match self {
            Self::Generated(syntax) => syntax.origin().producer(),
            Self::Ordinary { syntax, .. } => syntax.origin().declaration().producer(),
        }
    }
    fn capture(&self) -> &crate::source::module_capture::AdoptedModuleCapture {
        match self {
            Self::Generated(syntax) => syntax.origin().capture(),
            Self::Ordinary { syntax, .. } => syntax.origin().capture(),
        }
    }
}
enum AttributeSourceOwner {
    Original(usize),
    Module(Digest),
    Cold(usize),
    Fragment(usize),
}
struct StagedModule {
    ordinary_modules: Vec<crate::frontend::source_module_declarations::SourceModuleDeclaration>,
    provider: super::source_declarative::SourceDeclarative,
    annotation_plan: Option<
        std::sync::Arc<super::source_declarative::original_annotations::OriginalAnnotationPlan>,
    >,
    catalogs:
        BTreeMap<Digest, std::sync::Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>>,
    syntax: StagedModuleSyntax,
    ordinal: usize,
    parse_job: Digest,
    completed_invocations: BTreeSet<Digest>,
    imports: usize,
    source_producers: BTreeSet<Digest>,
    source_calls: BTreeMap<Digest, crate::syntax::outline::ItemInvocation>,
    namespace_complete: bool,
    producers: usize,
}
struct PendingSourceDerive {
    ticket: JobTicket,
    selection: std::sync::Arc<crate::service::source_derive_requests::SourceDeriveSelection>,
    request:
        Option<std::sync::Arc<crate::service::function_macro_requests::PreparedFunctionRequest>>,
    record: Option<std::sync::Arc<super::record::ExpansionRecord>>,
}
/// The configured name graph of one publication's discovery: a pure function
/// of the captured sources, their dependency rows, the compiler catalogue and
/// engine (see `configured_graph_key`). Every seal of a run over the same
/// publication (the Type then the Value fixed point) starts from it.
struct ConfiguredGraph {
    graph: NameNamespaces,
    imports: ImportResolution,
    routes: Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
    namespaces: std::sync::Arc<crate::frontend::macro_names::MacroNamespaces>,
    _catalog: Option<std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>>,
}
/// The last completed macro-route installation of one discovery graph. The
/// installation is a pure function of these inputs (its graph writes are
/// idempotent facts), so equal inputs reuse its namespace and graph facts.
#[derive(Default)]
struct RouteInstallMemo {
    routes: Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
    providers: Vec<RouteInstallProvider>,
    sources: Vec<(usize, Digest)>,
    inert: Vec<(Digest, Digest, Digest)>,
    namespaces: Option<std::sync::Arc<crate::frontend::macro_names::MacroNamespaces>>,
}
struct RouteInstallProvider {
    selection: crate::source::changes::SourceSelection,
    compiler: Option<std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>>,
    procedural: Option<std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>>,
    role: Option<std::sync::Arc<crate::db::memo::macro_export_inputs::MacroExportSourceObservation>>,
}
impl RouteInstallProvider {
    fn of(provider: &super::source_declarative::SourceDeclarative) -> Self {
        Self {
            selection: provider.module_source().selection().clone(),
            compiler: provider.compiler.clone(),
            procedural: provider.procedural.clone(),
            role: provider.macro_export_source_role().cloned(),
        }
    }
    fn same(&self, provider: &super::source_declarative::SourceDeclarative) -> bool {
        fn same<T>(a: &Option<std::sync::Arc<T>>, b: Option<&std::sync::Arc<T>>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
                _ => false,
            }
        }
        &self.selection == provider.module_source().selection()
            && same(&self.compiler, provider.compiler.as_ref())
            && same(&self.procedural, provider.procedural.as_ref())
            && same(&self.role, provider.macro_export_source_role())
    }
}
impl RouteInstallMemo {
    /// Installation reads only each nested index's (immutable) scan.
    fn source_rows(
        sources: &BTreeMap<usize, std::sync::Arc<crate::frontend::impl_index::DeclarationMacroIndex>>,
    ) -> Vec<(usize, Digest)> {
        sources
            .iter()
            .map(|(ordinal, index)| (*ordinal, index.scan().fingerprint()))
            .collect()
    }
    fn inert_rows(
        inert: &BTreeMap<Digest, super::nested_inert::NestedInertProof>,
    ) -> Vec<(Digest, Digest, Digest)> {
        inert
            .iter()
            .map(|(key, proof)| (*key, proof.producer(), proof.scan().fingerprint()))
            .collect()
    }
    fn hit(
        &self,
        providers: &[super::source_declarative::SourceDeclarative],
        routes: &[std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>],
        sources: &BTreeMap<usize, std::sync::Arc<crate::frontend::impl_index::DeclarationMacroIndex>>,
        inert: &BTreeMap<Digest, super::nested_inert::NestedInertProof>,
    ) -> Option<std::sync::Arc<crate::frontend::macro_names::MacroNamespaces>> {
        let namespaces = self.namespaces.as_ref()?;
        (self.providers.len() == providers.len()
            && self.providers.iter().zip(providers).all(|(a, b)| a.same(b))
            && self.routes.len() == routes.len()
            && self
                .routes
                .iter()
                .zip(routes)
                .all(|(a, b)| std::sync::Arc::ptr_eq(a, b) || a.same_routes(b))
            && self.sources == Self::source_rows(sources)
            && self.inert == Self::inert_rows(inert))
        .then(|| namespaces.clone())
    }
}
pub struct SourceDiscovery {
    sources: Vec<PreparedSourceView>,
    dependencies: std::sync::Arc<crate::source::dependencies::SourceDependencies>,
    scope: ScopeKey,
    namespace: Namespace,
    key: crate::db::seals::SealKey,
    graph: std::sync::Arc<NameNamespaces>,
    completed_source_environment: Option<std::sync::Arc<CompletedSourceEnvironment>>,
    // Source-only SDK owner. Contexts borrow it; ordinary runtime source and
    // namespace authority still require their existing independent admission.
    exports: Option<std::sync::Arc<SealedMacroExportProviders>>,
    work: Discovery,
    source_jobs: BTreeMap<Digest, usize>,
    original_modules: BTreeMap<Digest, OriginalModuleDeclaration>,
    imports: ImportResolution,
    /// The construction-time import fixed point was deferred: the declarative
    /// compiler configuration that follows installs its routes and resolves
    /// once on the completed graph (see `new_deferred_imports`).
    imports_deferred: bool,
    /// The construction graph is not built yet (see `new_deferred_imports`).
    graph_deferred: bool,
    route_install: RouteInstallMemo,
    /// Contained scope receipts of the last publication per namespace, keyed
    /// by everything they are computed from (republication reuses them).
    receipt_memo: std::sync::Mutex<
        BTreeMap<u32, (Digest, Vec<type_scope_receipts::TypeScopeReceipt>)>,
    >,
    producers: BTreeSet<Digest>,
    seal: Option<Seal>,
    body_boundary_seal_fingerprint: Option<Digest>,
    epoch: u64,
    started: bool,
    declarative: Option<(
        Digest,
        Digest,
        Vec<super::source_declarative::SourceDeclarative>,
    )>,
    compiler_profiles: Option<compiler_profiles::PreparedSourceCompilerFacets>,
    calls: BTreeMap<Digest, (usize, crate::syntax::outline::ItemInvocation)>,
    frontier_producers: BTreeMap<(ScopeKey, Digest), BTreeSet<Digest>>,
    nested_argument_summaries: BTreeMap<
        Digest,
        (
            std::sync::Arc<crate::syntax::declaration_scan::DeclarationScan>,
            Vec<std::sync::Arc<super::argument_declarations::ArgumentDeclarations>>,
        ),
    >,
    completed_argument_indexes: Vec<crate::frontend::impl_index::ArgumentDeclarationMacroIndex>,
    argument_discoveries: BTreeMap<Digest, super::argument_discovery::ArgumentDiscovery>,
    completed_argument_fragments:
        Vec<std::sync::Arc<super::declaration_fragment::CompletedDeclarationFragment>>,
    nested_inert_proofs: BTreeMap<Digest, super::nested_inert::NestedInertProof>,
    original_annotation_plans: BTreeMap<
        usize,
        std::sync::Arc<super::source_declarative::original_annotations::OriginalAnnotationPlan>,
    >,
    nested_runtime_proofs:
        BTreeMap<Digest, crate::frontend::runtime_macro_plan::RuntimeNoItemProof>,
    nested_sources:
        BTreeMap<usize, std::sync::Arc<crate::frontend::impl_index::DeclarationMacroIndex>>,
    nested_jobs: BTreeMap<Digest, usize>,
    original_prefix_jobs: BTreeSet<Digest>,
    source_replacements:
        BTreeMap<Digest, std::sync::Arc<super::source_replacement::SourceReplacementProof>>,
    replaced_source_producers:
        BTreeMap<Digest, std::sync::Arc<super::source_replacement::SourceReplacementProof>>,
    item_impl_jobs: BTreeMap<Digest, (Digest, Vec<Digest>, crate::syntax::outline::ScopeKey)>,
    item_root_declarations: BTreeMap<Digest, Digest>,
    child_impl_frontiers: BTreeMap<Digest, item_impl_children::ChildImplFrontier>,
    item_impl_completions:
        BTreeMap<Digest, std::sync::Arc<item_impl_completion::AncestorImplCompletion>>,
    item_declaration_inputs:
        BTreeMap<Digest, std::sync::Arc<super::generated_declarations::GeneratedDeclarations>>,
    nested_fragments: BTreeMap<Digest, super::declaration_fragment::DeclarationFragment>,
    nested_completed:
        BTreeMap<Digest, std::sync::Arc<super::declaration_fragment::CompletedDeclarationFragment>>,
    nested_bytes: usize,
    pending_nested: Option<JobTicket>,
    pending: Option<(
        JobTicket,
        usize,
        std::sync::Arc<super::source_declarative::SourceDeclarativeRequest>,
    )>,
    pending_procedural: Option<(JobTicket, usize, std::sync::Arc<ProceduralSourceSelection>)>,
    pending_attribute: Option<(
        JobTicket,
        AttributeSourceOwner,
        std::sync::Arc<crate::service::source_attribute_requests::SourceAttributeSelection>,
    )>,
    pending_source_derive: Option<PendingSourceDerive>,
    completed_source_derives: BTreeMap<
        Digest,
        std::sync::Arc<crate::service::source_derive_requests::SourceDeriveSelection>,
    >,
    source_derives_inflight: BTreeMap<Digest, Digest>,
    cold_attribute_owners: BTreeMap<Digest, usize>,
    cold_test_bindings: BTreeMap<Digest, (usize, Option<super::scopes::MacroDefinition>)>,
    attribute_selections: BTreeMap<
        Digest,
        std::sync::Arc<crate::service::source_attribute_requests::SourceAttributeSelection>,
    >,
    source_empty_derives: BTreeMap<Digest, super::source_derives::SourceEmptyDeriveProof>,
    source_derive_completions: BTreeMap<Digest, super::source_derives::SourceDeriveCompletionProof>,
    procedural_selections: BTreeMap<Digest, crate::expand::scopes::MacroDefinition>,
    procedural_selection_bytes: usize,
    admitting_procedural: bool,
    admitting_append: bool,
    pending_append: Option<(
        JobTicket,
        std::sync::Arc<super::generated_call_request::GeneratedCallRequest>,
        std::sync::Arc<super::record::ExpansionRecord>,
    )>,
    native_publication_certified: bool,
    /// The exact sealed content was published under a macro-run certificate.
    certified_publication: bool,
    fragments: BTreeMap<Digest, GeneratedFragment>,
    helper_receipt_bytes: usize,
    helper_declarations: BTreeMap<
        Digest,
        (
            std::sync::Arc<crate::frontend::generated_derive_exports::DeriveDeclarationRequest>,
            std::sync::Arc<crate::expand::derive_receipt::PreparedDeriveDeclaration>,
        ),
    >,
    pending_declaration: Option<(
        JobTicket,
        std::sync::Arc<crate::frontend::generated_derive_exports::DeriveDeclarationRequest>,
    )>,
    pending_generated: Option<(
        JobTicket,
        std::sync::Arc<super::generated_call_request::GeneratedCallRequest>,
    )>,
    failed: Option<Diagnostic>,
    generated_bytes: usize,
    expansion_budget: super::budget::SharedExpansionBudget,
    generated_headers: usize,
    records: BTreeMap<Digest, std::sync::Arc<super::record::ExpansionRecord>>,
    retained_record_bytes: usize,
    syntax_requests: BTreeMap<Digest, super::source_declarative::assembly::SyntaxRequestReceipt>,
    syntax_request_bytes: usize,
    generated_procedural_origins:
        BTreeMap<Digest, std::sync::Arc<crate::syntax::origin::MacroOrigin>>,
    generated_procedural_origin_bytes: usize,
    generated_owners: BTreeMap<Digest, (usize, ScopeKey)>,
    completed_empty_boundaries:
        BTreeMap<Digest, super::body_macro_boundary::CompletedEmptyBoundary>,
    pending_item_boundaries: BTreeMap<Digest, item_boundaries::PendingItemBoundary>,
    generated_source_modules: BTreeMap<Digest, Digest>,
    compiler_inputs: BTreeMap<crate::db::dependency::InputKey, Digest>,
    macro_export_sources: BTreeMap<
        usize,
        std::sync::Arc<crate::db::memo::macro_export_inputs::MacroExportSourceObservation>,
    >,
    macro_export_source_bytes: usize,
    macro_export_modules: BTreeMap<
        usize,
        std::sync::Arc<crate::source::macro_export_module::MacroExportModuleAdoption>,
    >,
    macro_export_module_bytes: usize,
    admitting_compiler: bool,
    module_sources: BTreeMap<Digest, crate::frontend::generated_headers::GeneratedModuleSource>,
    module_source_bytes: usize,
    pending_module: Option<(
        JobTicket,
        Digest,
        std::sync::Arc<super::source_declarative::SourceDeclarativeRequest>,
    )>,
    module_routes: BTreeMap<
        Digest,
        Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
    >,
    staged_modules: BTreeMap<Digest, StagedModule>,
    module_parse_jobs: BTreeMap<Digest, Digest>,
    parsed_modules: BTreeSet<Digest>,
    staged_module_bytes: usize,
    source_bases: BTreeMap<Digest, crate::syntax::token_wire::SpanRecord>,
    included_views:
        BTreeMap<Digest, std::sync::Arc<crate::source::include_view::IncludedSourceView>>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CATALOG-INCOMPLETE", message.into())
}
fn source_base_bytes(span: &crate::syntax::token_wire::SpanRecord) -> Result<usize, Diagnostic> {
    span.source
        .path
        .capacity()
        .checked_add(std::mem::size_of_val(span))
        .and_then(|n| n.checked_add(128))
        .ok_or_else(|| fault("invocation source base metadata overflow"))
}
fn retain_module_demands(
    sources: &mut BTreeMap<Digest, crate::frontend::generated_headers::GeneratedModuleSource>,
    retained_bytes: &mut usize,
    plan: &crate::frontend::generated_headers::GeneratedHeaderPlan,
) -> Result<(), Diagnostic> {
    sources
        .len()
        .checked_add(plan.module_sources().len())
        .ok_or_else(|| fault("generated module source count limit"))?;
    let mut bytes = *retained_bytes;
    for source in plan.module_sources() {
        if sources.contains_key(&source.producer()) {
            return Err(fault("duplicate generated module source demand"));
        }
        bytes = bytes
            .checked_add(source.metadata_bytes()?)
            .ok_or_else(|| fault("retained module source metadata limit"))?;
    }
    sources.extend(
        plan.module_sources()
            .iter()
            .map(|source| (source.producer(), source.clone())),
    );
    *retained_bytes = bytes;
    Ok(())
}
impl SourceDiscovery {
    pub fn new(
        sources: Vec<PreparedSourceView>,
        scope: ScopeKey,
        namespace: Namespace,
        unit_graph: Digest,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        let domain = SealDomain::Scope {
            id: scope.fingerprint(),
            namespace,
        };
        Self::new_domain(sources, scope, namespace, unit_graph, epoch, domain, true)
    }
    /// `new` for a discovery whose next step is a declarative compiler
    /// configuration (`with_declarative_compiler*`): that configuration
    /// installs the compiler routes and computes the one import fixed point on
    /// the completed graph, so construction does not compute a superseded one.
    /// Any other first use resolves first (`with_resolved_imports`, `poll`).
    pub fn new_deferred_imports(
        sources: Vec<PreparedSourceView>,
        scope: ScopeKey,
        namespace: Namespace,
        unit_graph: Digest,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        let domain = SealDomain::Scope {
            id: scope.fingerprint(),
            namespace,
        };
        Self::new_domain(sources, scope, namespace, unit_graph, epoch, domain, false)
    }
    /// Compute a deferred construction-time import fixed point now.
    pub fn with_resolved_imports(mut self) -> Result<Self, Diagnostic> {
        self.resolve_deferred_imports()?;
        Ok(self)
    }
    fn resolve_deferred_imports(&mut self) -> Result<(), Diagnostic> {
        if self.imports_deferred {
            self.build_deferred_graph()?;
            self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
            self.imports_deferred = false;
        }
        Ok(())
    }
    fn build_deferred_graph(&mut self) -> Result<(), Diagnostic> {
        if self.graph_deferred {
            self.graph = std::sync::Arc::new(Self::initial_graph(&self.sources, &self.dependencies)?);
            self.graph_deferred = false;
        }
        Ok(())
    }
    /// Discover every active test producer in the selected compilation unit.
    pub fn for_tests(
        sources: Vec<PreparedSourceView>,
        scope: ScopeKey,
        unit_graph: Digest,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        if scope.owner.is_some() || !scope.module_path.is_empty() {
            return Err(fault("test catalog requires the selected unit root"));
        }
        let domain = SealDomain::TestCatalog {
            id: scope.crate_key,
        };
        Self::new_domain(sources, scope, Namespace::Value, unit_graph, epoch, domain, true)
    }
    /// The construction graph: captured outlines, pending attribute
    /// declarations hidden, and the selected dependency edges.
    fn initial_graph(
        sources: &[PreparedSourceView],
        dependencies: &crate::source::dependencies::SourceDependencies,
    ) -> Result<NameNamespaces, Diagnostic> {
        let outlines = sources
            .iter()
            .map(PreparedSourceView::outline)
            .collect::<Vec<_>>();
        let mut graph = NameNamespaces::from_outlines(&outlines)?;
        let mut hidden = BTreeSet::new();
        for source in sources {
            for item in &source.outline().items {
                if crate::syntax::outline::attribute_producer(item).is_some() {
                    hidden.insert(item.def_key.fingerprint());
                }
            }
        }
        graph.hide_pending_attribute_declarations(&hidden);
        for source in sources {
            graph.require_view(source)?;
        }
        for (consumer, alias, provider, edition) in
            Self::validate_dependency_rows(sources, dependencies)?
        {
            graph.dependency(consumer, &alias, provider, edition)?;
        }
        Ok(graph)
    }
    /// Like rustc compiling a dependency, a discovery sees only the edges of
    /// crates it selects; a consumer's edge to it is not its input.
    fn validate_dependency_rows(
        sources: &[PreparedSourceView],
        dependencies: &crate::source::dependencies::SourceDependencies,
    ) -> Result<Vec<(Digest, String, Digest, u16)>, Diagnostic> {
        let mut rows = Vec::new();
        for (consumer, alias, provider) in dependencies.rows() {
            let Some(root) = sources.iter().find(|source| {
                source.outline().root_scope.crate_key == consumer
                    && source.outline().root_scope.module_path.is_empty()
            }) else {
                continue;
            };
            if !sources.iter().any(|source| {
                source.outline().root_scope.crate_key == provider
                    && source.outline().root_scope.module_path.is_empty()
            }) {
                return Err(fault("source dependency provider not selected"));
            }
            rows.push((consumer, alias.to_string(), provider, root.outline().profile.edition));
        }
        Ok(rows)
    }
    fn new_domain(
        mut sources: Vec<PreparedSourceView>,
        scope: ScopeKey,
        namespace: Namespace,
        unit_graph: Digest,
        epoch: u64,
        domain: SealDomain,
        resolve_imports: bool,
    ) -> Result<Self, Diagnostic> {
        if sources.is_empty() || scope.owner.is_some() {
            return Err(fault("scope discovery requires captured module source"));
        }
        sources.sort_by(|a, b| {
            a.outline()
                .root_scope
                .cmp(&b.outline().root_scope)
                .then_with(|| a.selection().path.cmp(&b.selection().path))
        });
        if !sources
            .iter()
            .any(|s| s.outline().scopes.contains_key(&scope))
        {
            return Err(fault("requested scope is absent from captured sources"));
        }
        for source in &sources {
            source.require_declaration_syntax()?;
        }
        let selected = sources
            .iter()
            .find(|source| source.outline().scopes.contains_key(&scope))
            .ok_or_else(|| fault("dependency graph owner absent"))?;
        let dependencies = selected.source_dependencies()?;
        if dependencies.has_aliases()
            && sources
                .iter()
                .any(|source| !source.same_publication(selected))
        {
            return Err(fault(
                "source dependency units require the same captured publication",
            ));
        }
        // A deferred discovery builds its graph when first needed: the
        // declarative configuration that follows may adopt the publication's
        // already configured graph instead (`ConfiguredGraph`).
        let mut graph = if resolve_imports {
            Self::initial_graph(&sources, &dependencies)?
        } else {
            Self::validate_dependency_rows(&sources, &dependencies)?;
            NameNamespaces::default()
        };
        let imports = if resolve_imports {
            graph.resolve_imports()?
        } else {
            ImportResolution::default()
        };
        let key = crate::db::seals::SealKey {
            domain: domain.clone(),
            unit_graph,
            epoch,
        };
        let mut work = Discovery::new(domain, unit_graph, epoch, DiscoveryLimits::default())?;
        let mut source_jobs = BTreeMap::new();
        let mut producers = BTreeSet::new();
        let mut calls = BTreeMap::new();
        let mut frontier_producers: BTreeMap<(ScopeKey, Digest), BTreeSet<Digest>> =
            BTreeMap::new();
        let mut call_bytes = 0usize;
        for (ordinal, source) in sources.iter().enumerate() {
            for call in source.outline().item_invocations.iter().cloned().chain(
                source
                    .outline()
                    .items
                    .iter()
                    .filter_map(crate::syntax::outline::attribute_producer),
            ) {
                call_bytes = call_bytes
                    .checked_add(768)
                    .and_then(|n| {
                        call.scope
                            .heap_bytes()
                            .checked_mul(2)
                            .and_then(|s| n.checked_add(s))
                    })
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "LRR-E-DISCOVERY-LIMIT",
                            "source producer index metadata limit".into(),
                        )
                    })?;

                if calls
                    .insert(call.item_input, (ordinal, call.clone()))
                    .is_some()
                {
                    return Err(fault("duplicate original invocation identity"));
                }
                frontier_producers
                    .entry((call.scope.clone(), call.frontier))
                    .or_default()
                    .insert(call.attribute_input.unwrap_or(call.item_input));
            }
        }

        let captured = sources
            .iter()
            .map(|s| s.outline().root_scope.clone())
            .collect::<BTreeSet<_>>();
        for (ordinal, source) in sources.iter().enumerate() {
            let order = u32::try_from(ordinal).map_err(|_| fault("source order overflow"))?;
            let identity =
                crate::db::key::QueryKey::Outline(source.selection().outline.clone()).fingerprint();
            if source_jobs.insert(identity, ordinal).is_some() {
                return Err(fault("duplicate captured source identity"));
            }
            work.enqueue(DiscoveryJob {
                module_order: order,
                source_offset: 0,
                kind: DiscoveryKind::ParseSource,
                identity,
            })?;
            for import in &source.outline().imports {
                work.enqueue(DiscoveryJob {
                    module_order: order,
                    source_offset: import.range.start,
                    kind: DiscoveryKind::ResolveImport,
                    identity: NameNamespaces::import_identity(import),
                })?;
            }
            for event in &source.outline().textual_events {
                if let TextualEventKind::Producer { identity, kind } = &event.kind {
                    if *kind == crate::syntax::outline::ProducerKind::ModuleSource
                        && captured.contains(&event.scope)
                    {
                        continue;
                    }
                    producers.insert(*identity);
                    work.enqueue(DiscoveryJob {
                        module_order: order,
                        source_offset: u32::try_from(event.at)
                            .map_err(|_| fault("producer source offset overflow"))?,
                        kind: DiscoveryKind::ResolveMacro,
                        identity: *identity,
                    })?;
                }
            }
            // Parse/attribute frontiers without a textual producer must also block.
            for (scope, members) in &source.outline().scopes {
                for blocker in &members.blockers {
                    if frontier_producers.contains_key(&(scope.clone(), blocker.syntax)) {
                        continue;
                    }

                    if producers.insert(blocker.syntax) {
                        work.enqueue(DiscoveryJob {
                            module_order: order,
                            source_offset: 0,
                            kind: DiscoveryKind::ResolveMacro,
                            identity: blocker.syntax,
                        })?;
                    }
                }
            }
        }
        let ordinary_ranges = calls
            .values()
            .map(|(ordinal, call)| (*ordinal, call.range.start, call.range.end))
            .collect::<BTreeSet<_>>();
        // Prefix maxima make enclosing ordinary ownership logarithmic, including
        // overlapping module transformations. The existing call limit bounds
        // this temporary index; range matches grant no transformation authority.
        let mut ordinary_coverage = BTreeMap::new();
        let mut coverage_owner = None;
        let mut coverage_end = 0;
        for (ordinal, start, end) in &ordinary_ranges {
            if coverage_owner != Some(*ordinal) {
                coverage_owner = Some(*ordinal);
                coverage_end = 0;
            }
            coverage_end = coverage_end.max(*end);
            ordinary_coverage.insert((*ordinal, *start), coverage_end);
        }
        let mut nested_sources = BTreeMap::new();
        let mut original_annotation_plans = BTreeMap::new();
        let mut nested_jobs = BTreeMap::new();
        let mut original_prefix_jobs = BTreeSet::new();
        let mut nested_bytes = 0usize;
        for (ordinal, source) in sources.iter().enumerate() {
            let scan = source.declaration_inventory(
                crate::syntax::declaration_scan::DeclarationScanLimits::default(),
            )?;
            let prefixes = if scan.generated_origin().is_none() {
                original_inert::capture(source, &scan, usize::MAX)?
            } else {
                // This lane supplies no original-source authority to generated
                // roots; their existing producer/context admission is separate.
                BTreeSet::new()
            };
            nested_bytes = nested_bytes
                .checked_add(
                    prefixes
                        .len()
                        .checked_mul(128)
                        .ok_or_else(|| fault("original root annotation metadata overflow"))?,
                )
                .ok_or_else(|| fault("original root annotation metadata limit"))?;

            original_prefix_jobs.extend(prefixes.iter().copied());
            if scan.generated_origin().is_none() {
                let plan = super::source_declarative::original_annotations::OriginalAnnotationPlan::capture(source, scan.clone())?;
                nested_bytes = nested_bytes
                    .checked_add(plan.metadata_bytes())
                    .and_then(|n| n.checked_add(128))
                    .ok_or_else(|| fault("original annotation plan metadata limit"))?;
                original_annotation_plans.insert(ordinal, std::sync::Arc::new(plan));
            }
            let mut index = crate::frontend::impl_index::DeclarationMacroIndex::new(scan)?;
            // The runtime outline owns module items, not associated items.
            // An associated header can have no executable owner; that does not
            // delegate its annotations to the ordinary item worklist.
            let mut associated_scopes = BTreeSet::new();
            for lexical in index.scan().lexical_scopes() {
                if lexical.kind
                    == crate::syntax::declaration_scan::DeclarationScopeKind::AssociatedItems
                {
                    associated_scopes.insert(lexical.identity);
                }
            }
            // An unowned compiler Test candidate on a non-function cannot be
            // discarded by delegation. Ordinary replacing item producers keep
            // ownership of their complete annotation sequence. This is syntax
            // routing only; cold resolution still establishes compiler identity.
            let mut cold_test_targets = BTreeSet::new();
            for row in index.scan().producers() {
                if row.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                    && row.unqualified_attribute.as_deref() == Some("test")
                    && !ordinary_coverage
                        .range((ordinal, 0)..=(ordinal, row.range.start))
                        .next_back()
                        .is_some_and(|(_, end)| row.range.end <= *end)
                    && !source.attribute_targets_free_function(index.scan(), row.identity)?
                {
                    cold_test_targets.insert(row.identity);
                }
            }
            let delegated = index
                .scan()
                .producers()
                .iter()
                .filter(|p| {
                    !prefixes.contains(&p.identity)
                        && (ordinary_ranges.contains(&(ordinal, p.range.start, p.range.end))
                        || (p.kind
                            == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                            && p.scope.owner.is_none()
                            && !associated_scopes.contains(&p.lexical_scope)
                            && !cold_test_targets.contains(&p.identity)))
                })
                .map(|p| p.identity)
                .collect::<BTreeSet<_>>();
            for identity in &delegated {
                index.delegate_original_producer(*identity)?;
            }
            nested_bytes = nested_bytes
                .checked_add(index.retained_metadata_bytes())
                .ok_or_else(|| fault("nested source inventory metadata limit"))?;
            for producer in index.scan().producers() {
                // Item/annotation jobs already owned by the ordinary source worklist.
                // Matching is by captured syntax range, never rendered token text.
                if delegated.contains(&producer.identity) {
                    continue;
                }
                if nested_jobs.insert(producer.identity, ordinal).is_some() {
                    return Err(fault("nested declaration producer identity/count conflict"));
                }
                work.enqueue(DiscoveryJob {
                    module_order: u32::try_from(ordinal)
                        .map_err(|_| fault("nested source order overflow"))?,
                    source_offset: producer.range.start,
                    kind: DiscoveryKind::ScanNestedDeclarations,
                    identity: producer.identity,
                })?;
            }
            nested_sources.insert(ordinal, std::sync::Arc::new(index));
        }
        let original_modules =
            original_modules::capture_original_modules(&sources, &mut nested_bytes)?;
        let expansion_budget = super::budget::SharedExpansionBudget::new(
            key.fingerprint(),
            key.unit_graph,
            key.epoch,
        )?;
        Ok(Self {
            sources,
            dependencies,
            scope,
            namespace,
            key,
            graph: std::sync::Arc::new(graph),
            completed_source_environment: None,
            exports: None,
            work,
            source_jobs,
            original_modules,
            imports,
            imports_deferred: !resolve_imports,
            graph_deferred: !resolve_imports,
            route_install: RouteInstallMemo::default(),
            receipt_memo: std::sync::Mutex::new(BTreeMap::new()),
            producers,
            seal: None,
            body_boundary_seal_fingerprint: None,
            epoch,
            started: false,
            declarative: None,
            compiler_profiles: None,
            calls,
            frontier_producers,
            nested_argument_summaries: BTreeMap::new(),
            completed_argument_indexes: Vec::new(),
            argument_discoveries: BTreeMap::new(),
            completed_argument_fragments: Vec::new(),
            nested_inert_proofs: BTreeMap::new(),
            original_annotation_plans,
            nested_runtime_proofs: BTreeMap::new(),
            nested_sources,
            nested_jobs,
            original_prefix_jobs,
            source_replacements: BTreeMap::new(),
            replaced_source_producers: BTreeMap::new(),
            item_impl_jobs: BTreeMap::new(),
            item_root_declarations: BTreeMap::new(),
            child_impl_frontiers: BTreeMap::new(),
            item_impl_completions: BTreeMap::new(),
            item_declaration_inputs: BTreeMap::new(),
            nested_fragments: BTreeMap::new(),
            nested_completed: BTreeMap::new(),
            nested_bytes,
            pending_nested: None,
            pending: None,
            pending_procedural: None,
            pending_attribute: None,
            pending_source_derive: None,
            completed_source_derives: BTreeMap::new(),
            source_derives_inflight: BTreeMap::new(),
            cold_attribute_owners: BTreeMap::new(),
            cold_test_bindings: BTreeMap::new(),
            attribute_selections: BTreeMap::new(),
            source_empty_derives: BTreeMap::new(),
            source_derive_completions: BTreeMap::new(),
            procedural_selections: BTreeMap::new(),
            procedural_selection_bytes: 0,
            admitting_procedural: false,
            admitting_append: false,
            pending_append: None,
            native_publication_certified: false,
            certified_publication: false,
            fragments: BTreeMap::new(),
            pending_generated: None,
            pending_declaration: None,
            helper_receipt_bytes: 0,
            helper_declarations: BTreeMap::new(),
            failed: None,
            generated_bytes: 0,
            expansion_budget,
            generated_headers: 0,
            records: BTreeMap::new(),
            retained_record_bytes: 0,
            syntax_requests: BTreeMap::new(),
            syntax_request_bytes: 0,
            generated_procedural_origins: BTreeMap::new(),
            generated_procedural_origin_bytes: 0,
            generated_owners: BTreeMap::new(),
            completed_empty_boundaries: BTreeMap::new(),
            pending_item_boundaries: BTreeMap::new(),
            generated_source_modules: BTreeMap::new(),
            compiler_inputs: BTreeMap::new(),
            macro_export_sources: BTreeMap::new(),
            macro_export_source_bytes: 0,
            macro_export_modules: BTreeMap::new(),
            macro_export_module_bytes: 0,
            admitting_compiler: false,
            module_sources: BTreeMap::new(),
            module_source_bytes: 0,
            pending_module: None,
            module_routes: BTreeMap::new(),
            staged_modules: BTreeMap::new(),
            module_parse_jobs: BTreeMap::new(),
            parsed_modules: BTreeSet::new(),
            staged_module_bytes: 0,
            included_views: BTreeMap::new(),
            source_bases: BTreeMap::new(),
        })
    }
    /// Freeze pure declarative preparation before discovery begins. This grants no tool authority.
    pub fn with_declarative(self, compile_env: Digest, engine: Digest) -> Result<Self, Diagnostic> {
        self.configure_declarative(compile_env, engine, None, None)
    }
    /// Install prepared macro-only aliases before original extern/import routes are constructed.
    /// This consumes owned catalog metadata and performs no macro execution or preparation.
    pub fn with_declarative_procedural(
        self,
        compile_env: Digest,
        engine: Digest,
        catalog: std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
    ) -> Result<Self, Diagnostic> {
        self.configure_declarative(compile_env, engine, None, Some(catalog))
    }
    pub fn with_procedural_catalog(
        mut self,
        catalog: std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
    ) -> Result<Self, Diagnostic> {
        if self.started || catalog.graph() != self.key.unit_graph || catalog.epoch() != self.epoch {
            return Err(fault(
                "procedural catalog graph/epoch/configuration differs",
            ));
        }
        let (environment, _, providers) = self
            .declarative
            .as_mut()
            .ok_or_else(|| fault("procedural discovery requires source providers"))?;
        if *environment != catalog.environment()
            || !providers.iter().any(|provider| {
                provider.module_source().outline().root_scope.crate_key == catalog.consumer()
            })
        {
            return Err(fault("procedural catalog consumer/environment differs"));
        }
        for provider in providers
            .iter_mut()
            .filter(|p| p.module_source().outline().root_scope.crate_key == catalog.consumer())
        {
            provider.install_procedural(catalog.clone())?;
        }
        Self::install_macro_routes_with_modules(
            unique_graph(&mut self.graph)?,
            providers,
            &self.module_routes,
            &self.nested_sources,
            &self.nested_inert_proofs,
            &mut self.route_install,
        )?;
        self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
        self.reserve_test_attribute_candidates(Some(catalog.consumer()), false)?;
        Ok(self)
    }
    /// Compiler and procedural catalog installation paths reserve original Test
    /// candidates before discovery can treat the spelling as compiler metadata.
    /// This routes captured syntax only; execution still requires macro authority.
    fn reserve_test_attribute_candidates(
        &mut self,
        consumer: Option<Digest>,
        functions_only: bool,
    ) -> Result<(), Diagnostic> {
        let mut pending = Vec::new();
        for (ordinal, source) in self.sources.iter().enumerate() {
            if consumer.is_some_and(|consumer| source.outline().root_scope.crate_key != consumer) {
                continue;
            }
            for item in &source.outline().items {
                // Non-function compiler Test targets retain their existing
                // declaration-scan owner and exact eligibility refusal.
                if functions_only
                    && item.def_key.item_kind != crate::source::manifest::ItemKind::Function
                {
                    continue;
                }
                if crate::syntax::outline::attribute_producer(item).is_some() {
                    continue;
                }
                if let Some(call) = crate::syntax::outline::potential_attribute_producer(item) {
                    if let Some((owner, installed)) = self.calls.get(&call.item_input) {
                        if *owner != ordinal || installed != &call {
                            return Err(fault("test attribute reservation ownership differs"));
                        }
                        continue;
                    }
                    pending
                        .try_reserve(1)
                        .map_err(|_| fault("test attribute producer allocation"))?;
                    pending.push((ordinal, item.def_key.fingerprint(), call));
                }
            }
        }
        for (ordinal, definition, call) in pending {
            self.declarative
                .as_mut()
                .ok_or_else(|| fault("test attribute provider absent"))?
                .2[ordinal]
                .reserve_test_attribute(&call)?;
            unique_graph(&mut self.graph)?
                .hide_pending_attribute_declarations(&BTreeSet::from([definition]));
            unique_graph(&mut self.graph)?
                .reserve_generated_frontier(call.scope.clone(), call.frontier)?;
            self.work.enqueue(DiscoveryJob {
                module_order: u32::try_from(ordinal)
                    .map_err(|_| fault("test attribute ordinal overflow"))?,
                source_offset: call.range.start,
                kind: DiscoveryKind::ResolveMacro,
                identity: call.item_input,
            })?;
            self.producers.insert(call.item_input);
            self.frontier_producers
                .entry((call.scope.clone(), call.frontier))
                .or_default()
                .insert(call.item_input);
            if self
                .calls
                .insert(call.item_input, (ordinal, call))
                .is_some()
            {
                return Err(fault("test attribute producer duplicated"));
            }
        }
        Ok(())
    }
    /// One deterministic source pass checks all live compiler consumers, keeping
    /// existing invocation owners when a procedural path already reserved them.
    fn reserve_compiler_test_attribute_candidates(&mut self) -> Result<(), Diagnostic> {
        self.reserve_test_attribute_candidates(None, true)
    }
    pub fn with_declarative_compiler(
        self,
        compile_env: Digest,
        engine: Digest,
        catalog: std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>,
    ) -> Result<Self, Diagnostic> {
        self.configure_declarative(compile_env, engine, Some(catalog), None)
    }
    /// Compiler facets plus one consumer's prepared procedural catalog, installed
    /// before original extern/import routes are constructed. No macro executes.
    pub fn with_declarative_compiler_procedural(
        self,
        compile_env: Digest,
        engine: Digest,
        catalog: std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>,
        procedural: std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
    ) -> Result<Self, Diagnostic> {
        self.configure_declarative(compile_env, engine, Some(catalog), Some(procedural))
    }
    pub(crate) fn prepare_with_declarative_compiler(
        self,
        compile_env: Digest,
        engine: Digest,
        catalog: std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>,
    ) -> Result<SourceCompilerPreparation, Diagnostic> {
        self.prepare_declarative(compile_env, engine, Some(catalog), None)
    }
    fn configure_declarative(
        self,
        compile_env: Digest,
        engine: Digest,
        compiler: Option<
            std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>,
        >,
        procedural: Option<
            std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
        >,
    ) -> Result<Self, Diagnostic> {
        self.prepare_declarative(compile_env, engine, compiler, procedural)?
            .require_ready()
    }
    /// Key of the configured graph (`ConfiguredGraph`), when this
    /// configuration is a pure function of the publication: no procedural or
    /// macro-export roles, no adopted module routes or inert proofs yet.
    fn configured_graph_key(
        &self,
        compile_env: Digest,
        engine: Digest,
        compiler: Option<&std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>>,
        procedural: bool,
        compiler_attributes: bool,
    ) -> Result<Option<Digest>, Diagnostic> {
        use crate::core::digest::StableEncoder;
        let Some(first) = self.sources.first() else {
            return Ok(None);
        };
        if procedural
            || !self.macro_export_sources.is_empty()
            || !self.macro_export_modules.is_empty()
            || !self.module_routes.is_empty()
            || !self.nested_inert_proofs.is_empty()
            || self.sources.iter().any(|source| !source.same_publication(first))
        {
            return Ok(None);
        }
        let mut e = StableEncoder::new("lrr/v1/configured-discovery-graph");
        e.sequence(&self.sources, |e, source| {
            e.digest(source.selection().location_identity());
            e.record(&source.selection().outline);
        });
        e.digest(self.dependencies.fingerprint());
        e.digest(self.key.unit_graph);
        e.u64(self.epoch);
        e.digest(compile_env);
        e.digest(engine);
        e.u8(u8::from(compiler_attributes));
        match compiler {
            Some(catalog) => {
                e.u8(1);
                e.digest(catalog.owning_profile_identity()?);
                e.u64(std::sync::Arc::as_ptr(catalog) as usize as u64);
            }
            None => e.u8(0),
        }
        e.sequence(&RouteInstallMemo::source_rows(&self.nested_sources), |e, (ordinal, scan)| {
            e.u64(*ordinal as u64);
            e.digest(*scan);
        });
        Ok(Some(e.finish()))
    }
    fn prepare_declarative(
        mut self,
        compile_env: Digest,
        engine: Digest,
        compiler: Option<
            std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>,
        >,
        procedural: Option<
            std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
        >,
    ) -> Result<SourceCompilerPreparation, Diagnostic> {
        if self.started || self.declarative.is_some() {
            return Err(fault(
                "declarative discovery configuration is already frozen",
            ));
        }
        if compiler
            .as_ref()
            .is_some_and(|catalog| catalog.macro_export_facets().next().is_some())
        {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-EXPORT-PUBLICATION",
                "configured macro-export source-currentness and SDK export graph publication is required before discovery installation".into(),
            ));
        }
        let procedural_consumer = procedural.as_ref().map(|catalog| catalog.consumer());
        let compiler_attributes = compiler
            .as_ref()
            .is_some_and(|catalog| catalog.attribute_catalog().is_some());
        let mut providers = self
            .sources
            .iter()
            .map(|s| super::source_declarative::SourceDeclarative::new(s.share()))
            .collect::<Result<Vec<_>, _>>()?;
        for (ordinal, role) in &self.macro_export_sources {
            providers
                .get_mut(*ordinal)
                .ok_or_else(|| fault("macro-export source owner is absent"))?
                .bind_macro_export_source(role.clone(), self.key.unit_graph)?;
        }
        for (ordinal, module) in &self.macro_export_modules {
            providers
                .get_mut(*ordinal)
                .ok_or_else(|| fault("macro-export adopted module owner is absent"))?
                .bind_macro_export_module(module.clone())?;
        }
        if let Some(catalog) = procedural {
            if catalog.graph() != self.key.unit_graph
                || catalog.epoch() != self.epoch
                || catalog.environment() != compile_env
                || !providers
                    .iter()
                    .any(|p| p.module_source().outline().root_scope.crate_key == catalog.consumer())
            {
                return Err(fault(
                    "procedural catalog graph/epoch/consumer/environment differs before route construction",
                ));
            }
            for provider in providers
                .iter_mut()
                .filter(|p| p.module_source().outline().root_scope.crate_key == catalog.consumer())
            {
                provider.install_procedural(catalog.clone())?;
            }
        }
        let mut prepared_nested_bytes = self.nested_bytes;
        let mut prepared_profiles = None;
        if let Some(catalog) = compiler.clone() {
            if catalog.definitions().next().is_none() {
                return Err(fault("compiler facet is empty"));
            }
            let prepared = compiler_profiles::PreparedSourceCompilerFacets::prepare(
                &providers,
                catalog,
                usize::MAX,
            )?;
            let retained = prepared_nested_bytes
                .checked_add(prepared.metadata_bytes())
                .ok_or_else(|| fault("owning compiler facet metadata limit"))?;
            prepared.install(&mut providers)?;
            prepared_nested_bytes = retained;
            prepared_profiles = Some(prepared);
        }
        super::source_declarative::SourceDeclarative::configure_captured_modules(&mut providers)?;
        for (ordinal, provider) in providers.iter_mut().enumerate() {
            if let Some(plan) = self.original_annotation_plans.get(&ordinal) {
                let bytes = provider.prepare_compiler_attributes(plan)?;
                prepared_nested_bytes = prepared_nested_bytes
                    .checked_add(bytes)
                    .ok_or_else(|| fault("compiler attribute preparation metadata limit"))?;
            }
        }
        let configured_key = self.configured_graph_key(
            compile_env,
            engine,
            compiler.as_ref(),
            procedural_consumer.is_some(),
            compiler_attributes,
        )?;
        if let Some(key) = configured_key
            && let Some(hit) = self.sources[0].publication_prepared(key)
            && let Ok(hit) = hit.downcast::<ConfiguredGraph>()
        {
            self.graph = std::sync::Arc::new(hit.graph.clone());
            self.graph_deferred = false;
            self.route_install = RouteInstallMemo {
                providers: providers.iter().map(RouteInstallProvider::of).collect(),
                sources: RouteInstallMemo::source_rows(&self.nested_sources),
                inert: RouteInstallMemo::inert_rows(&self.nested_inert_proofs),
                routes: hit.routes.clone(),
                namespaces: Some(hit.namespaces.clone()),
            };
            self.nested_bytes = prepared_nested_bytes;
            self.compiler_profiles = prepared_profiles;
            self.imports = hit.imports.clone();
            self.imports_deferred = false;
            self.declarative = Some((compile_env, engine, providers));
            if let Some(consumer) = procedural_consumer {
                self.reserve_test_attribute_candidates(Some(consumer), false)?;
            }
            if compiler_attributes {
                self.reserve_compiler_test_attribute_candidates()?;
            }
            return Ok(SourceCompilerPreparation::Ready(self));
        }
        self.build_deferred_graph()?;
        let routes = Self::completed_macro_routes(&providers, &self.module_routes)?;
        let namespaces = match Self::prepare_compiler_import_graph(&routes, &providers)? {
            crate::frontend::source_macro_routes::ModuleGraphPreparation::Ready(namespaces) => {
                namespaces
            }
            crate::frontend::source_macro_routes::ModuleGraphPreparation::NeedModule(demand) => {
                return Ok(SourceCompilerPreparation::NeedModule {
                    discovery: self,
                    demand,
                });
            }
        };
        let namespaces = Self::install_prepared_macro_routes(
            unique_graph(&mut self.graph)?,
            &providers,
            &self.nested_sources,
            &self.nested_inert_proofs,
            &routes,
            namespaces,
        )?;
        self.route_install = RouteInstallMemo {
            providers: providers.iter().map(RouteInstallProvider::of).collect(),
            sources: RouteInstallMemo::source_rows(&self.nested_sources),
            inert: RouteInstallMemo::inert_rows(&self.nested_inert_proofs),
            routes,
            namespaces: Some(std::sync::Arc::new(namespaces)),
        };
        self.nested_bytes = prepared_nested_bytes;
        self.compiler_profiles = prepared_profiles;
        self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
        self.imports_deferred = false;
        if let Some(key) = configured_key
            && let Some(namespaces) = &self.route_install.namespaces
        {
            self.sources[0].retain_publication_prepared(
                key,
                std::sync::Arc::new(ConfiguredGraph {
                    graph: (*self.graph).clone(),
                    imports: self.imports.clone(),
                    routes: self.route_install.routes.clone(),
                    namespaces: namespaces.clone(),
                    _catalog: compiler.clone(),
                }),
            );
        }
        self.declarative = Some((compile_env, engine, providers));
        if let Some(consumer) = procedural_consumer {
            self.reserve_test_attribute_candidates(Some(consumer), false)?;
        }
        if compiler_attributes {
            self.reserve_compiler_test_attribute_candidates()?;
        }
        Ok(SourceCompilerPreparation::Ready(self))
    }
    pub fn with_compiler(
        mut self,
        catalog: std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>,
    ) -> Result<Self, Diagnostic> {
        if self.started || catalog.definitions().next().is_none() {
            return Err(fault("compiler discovery configuration is late or empty"));
        }
        let compiler_attributes = catalog.attribute_catalog().is_some();
        let providers = &mut self
            .declarative
            .as_mut()
            .ok_or_else(|| fault("compiler discovery requires captured declarative indexes"))?
            .2;
        let prepared = compiler_profiles::PreparedSourceCompilerFacets::prepare(
            providers,
            catalog,
            usize::MAX,
        )?;
        let retained = self
            .nested_bytes
            .checked_add(prepared.metadata_bytes())
            .ok_or_else(|| fault("owning compiler facet metadata limit"))?;
        prepared.install(providers)?;
        self.nested_bytes = retained;
        self.compiler_profiles = Some(prepared);
        for (ordinal, provider) in providers.iter_mut().enumerate() {
            if let Some(plan) = self.original_annotation_plans.get(&ordinal) {
                let bytes = provider.prepare_compiler_attributes(plan)?;
                self.nested_bytes = self
                    .nested_bytes
                    .checked_add(bytes)
                    .ok_or_else(|| fault("compiler attribute preparation metadata limit"))?;
            }
        }
        Self::install_macro_routes_with_modules(
            unique_graph(&mut self.graph)?,
            providers,
            &self.module_routes,
            &self.nested_sources,
            &self.nested_inert_proofs,
            &mut self.route_install,
        )?;
        self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
        if compiler_attributes {
            self.reserve_compiler_test_attribute_candidates()?;
        }
        Ok(self)
    }
    pub fn poll_compiler(
        &mut self,
        db: &mut crate::db::memo::Database,
        environment: &crate::expand::builtins::FrozenCompileEnvironment,
        authorities: &crate::source::include_paths::IncludeAuthorities,
    ) -> Result<crate::service::eager_compile_inputs::EagerCompileInputPoll, Diagnostic> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        use crate::service::eager_compile_inputs::EagerCompileInputPoll;
        let (result, mut usage) = db.observe_expansion_inputs(|db| {
            self.poll_compiler_inner(db, environment, authorities)
        })?;
        if let EagerCompileInputPoll::Produced(record) | EagerCompileInputPoll::Complete(record) =
            &result
        {
            Self::observe_output(&mut usage, record)?;
        }
        self.expansion_budget.observe(&usage)?;
        Ok(result)
    }
    fn poll_compiler_inner(
        &mut self,
        db: &mut crate::db::memo::Database,
        environment: &crate::expand::builtins::FrozenCompileEnvironment,
        authorities: &crate::source::include_paths::IncludeAuthorities,
    ) -> Result<crate::service::eager_compile_inputs::EagerCompileInputPoll, Diagnostic> {
        match crate::db::provider::SyntaxProvider::compile_environment(db, environment.unit()) {
            crate::db::poll::QueryPoll::Ready(Ok(captured))
                if captured.fingerprint() == environment.fingerprint() => {}
            crate::db::poll::QueryPoll::Ready(Ok(_)) => {
                return Err(fault(
                    "compiler discovery environment differs from captured publication",
                ));
            }
            crate::db::poll::QueryPoll::Ready(Err(error)) => return Err(error),
            crate::db::poll::QueryPoll::NeedInput(input) => {
                return Ok(
                    crate::service::eager_compile_inputs::EagerCompileInputPoll::NeedInput(input),
                );
            }
        }
        if self.pending_generated.is_some() {
            return self.generated_compiler_work(db, environment, authorities, None);
        }
        let request = self
            .pending_module
            .as_ref()
            .map(|pending| pending.2.clone())
            .or_else(|| self.pending.as_ref().map(|pending| pending.2.clone()))
            .or_else(|| self.pending_declaration_request().cloned())
            .ok_or_else(|| fault("no compiler discovery demand pending"))?;
        let providers = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("compiler source indexes absent"))?
            .2;
        let namespaces = Self::install_macro_routes_with_modules(
            unique_graph(&mut self.graph)?,
            providers,
            &self.module_routes,
            &self.nested_sources,
            &self.nested_inert_proofs,
            &mut self.route_install,
        )?;
        request.poll_compiler(
            db,
            &namespaces,
            environment,
            authorities,
            self.compiler_caller_view(&request)?,
        )
    }
    fn compiler_caller_view(
        &self,
        request: &super::source_declarative::SourceDeclarativeRequest,
    ) -> Result<Option<&crate::source::include_view::IncludedSourceView>, Diagnostic> {
        let site = request.source_base()?;
        let mut matching = self.included_views.values().filter(|view| {
            view.location().unit.0 == site.source.unit.0
                && view.blob().content().0 == site.source.content.0
                && view.location().path.as_str() == site.source.path
                && view.origin() == site.origin
                && view.caller_context() == site.context
                && view
                    .tokens()
                    .spans()
                    .iter()
                    .any(|original| original == &site)
        });
        let first = matching.next();
        if first.is_some_and(|first| matching.any(|other| first.identity() != other.identity())) {
            return Err(fault("compiler declaration caller view is ambiguous"));
        }
        Ok(first.map(std::sync::Arc::as_ref))
    }
    fn pending_declaration_request(
        &self,
    ) -> Option<&std::sync::Arc<super::source_declarative::SourceDeclarativeRequest>> {
        let identity = self.pending_nested.as_ref()?.job().identity;
        self.nested_fragments
            .get(&identity)
            .and_then(|fragment| fragment.pending_request())
            .or_else(|| {
                self.argument_discoveries
                    .get(&identity)
                    .and_then(|arguments| arguments.pending_request())
            })
    }
    fn generated_compiler_scope(&self) -> Result<GeneratedCompilerScope<'_>, Diagnostic> {
        let (ticket, _) = self
            .pending_generated
            .as_ref()
            .ok_or_else(|| fault("generated compiler demand absent"))?;
        let fragment = self
            .fragments
            .get(&ticket.job().identity)
            .ok_or_else(|| fault("generated compiler fragment absent"))?;
        let providers = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("generated compiler source providers absent"))?
            .2;
        let inherited = if let Some(module) = fragment.module {
            let staged = self
                .staged_modules
                .get(&module)
                .ok_or_else(|| fault("generated compiler module source absent"))?;
            Some(self.module_projection_inherited(&staged.syntax)?)
        } else {
            None
        };
        let provider = match fragment.module {
            Some(module) => {
                &self
                    .staged_modules
                    .get(&module)
                    .ok_or_else(|| fault("generated compiler child provider absent"))?
                    .provider
            }
            None => providers
                .get(fragment.ordinal)
                .ok_or_else(|| fault("generated compiler provider ordinal absent"))?,
        };
        let captured = super::source_declarative::CapturedMacroContext {
            exports: self.exports.as_deref(),
            provider,
            owners: providers,
            before: u64::from(fragment.call.range.start),
            inherited: None,
        };
        let mut ancestors = Vec::new();
        ancestors
            .try_reserve_exact(256)
            .map_err(|_| fault("generated compiler ancestry allocation"))?;
        let catalog = match fragment.active_child {
            Some(id) => {
                let child = fragment
                    .children
                    .get(&id)
                    .ok_or_else(|| fault("generated compiler active child absent"))?;
                let mut parent = child.parent;
                while let Some(id) = parent {
                    let frame = fragment
                        .children
                        .get(&id)
                        .ok_or_else(|| fault("generated compiler parent absent"))?;
                    ancestors.push(&frame.catalog);
                    parent = frame.parent;
                }
                ancestors.push(&fragment.catalog);
                &child.catalog
            }
            None => &fragment.catalog,
        };
        Ok(GeneratedCompilerScope {
            catalog,
            captured,
            ancestors,
            inherited,
        })
    }
    fn generated_compiler_work(
        &self,
        db: &mut crate::db::memo::Database,
        environment: &crate::expand::builtins::FrozenCompileEnvironment,
        authorities: &crate::source::include_paths::IncludeAuthorities,
        view: Option<&mut Option<crate::source::include_view::IncludedSourceView>>,
    ) -> Result<crate::service::eager_compile_inputs::EagerCompileInputPoll, Diagnostic> {
        let request = &self
            .pending_generated
            .as_ref()
            .ok_or_else(|| fault("generated compiler demand absent"))?
            .1;
        let scope = self.generated_compiler_scope()?;
        let catalog = scope.catalog;
        let context = scope.compiler_context();
        let ancestors = &scope.ancestors;
        let source_base = self
            .source_bases
            .get(&catalog.record_identity())
            .ok_or_else(|| fault("generated invocation source base absent"))?;
        let included = self
            .included_views
            .values()
            .find(|view| {
                view.origin() == source_base.origin
                    && view.blob().content().0 == source_base.source.content.0
                    && view.location().path.as_str() == source_base.source.path
            })
            .map(std::sync::Arc::as_ref);
        let prepared = catalog.prepare_compiler(
            request,
            context,
            ancestors,
            authorities,
            included,
            source_base,
        )?;
        let (definition, input, site) = request.compiler_input();
        let eager = crate::service::eager_compile_inputs::EagerCompilerRequest {
            selected: definition,
            key: request.key(),
            original: input,
            site,
            source_revision: db.source_revision(),
            limits: Default::default(),
        };
        let sources = crate::service::eager_compile_inputs::EagerSourceContext {
            authorities,
            from: &prepared.from,
            included,
        };
        if let Some(view) = view {
            match crate::service::eager_compile_inputs::adopted_eager_include_view(
                db,
                eager,
                &prepared.resolver,
                &prepared.caller,
                environment,
                sources,
            ) {
                crate::db::poll::QueryPoll::Ready(result) => *view = result?,
                crate::db::poll::QueryPoll::NeedInput(_) => {
                    return Err(fault("generated include view requires adopted inputs"));
                }
            }
            let input = db
                .input(&crate::db::dependency::InputKey::ExpansionRecord(
                    request.key().fingerprint(),
                ))
                .ok_or_else(|| fault("generated compiler output absent"))?;
            let crate::db::dependency::InputValue::ExpansionRecord(record) = input.as_ref() else {
                return Err(fault("generated compiler output variant differs"));
            };
            return Ok(
                crate::service::eager_compile_inputs::EagerCompileInputPoll::Complete(
                    record.clone(),
                ),
            );
        }
        crate::service::eager_compile_inputs::poll_eager_compile_input_with_sources(
            db,
            eager,
            &prepared.resolver,
            &prepared.caller,
            environment,
            sources,
        )
    }
    /// Pure declarative work outside providers; this grants no procedural authority.
    /// Revalidate the current exact demand before executing the pinned adapter.
    pub fn expand_compiler_string_operand(
        &mut self,
        db: &mut crate::db::memo::Database,
        environment: &crate::expand::builtins::FrozenCompileEnvironment,
        authorities: &crate::source::include_paths::IncludeAuthorities,
        request: &crate::service::eager_compile_inputs::EagerExternalCall,
    ) -> Result<super::record::ExpansionRecord, Diagnostic> {
        let record =
            self.expand_compiler_string_operand_inner(db, environment, authorities, request)?;
        self.expansion_budget.record_output(&record)?;
        Ok(record)
    }
    fn expand_compiler_string_operand_inner(
        &mut self,
        db: &mut crate::db::memo::Database,
        environment: &crate::expand::builtins::FrozenCompileEnvironment,
        authorities: &crate::source::include_paths::IncludeAuthorities,
        request: &crate::service::eager_compile_inputs::EagerExternalCall,
    ) -> Result<super::record::ExpansionRecord, Diagnostic> {
        let crate::service::eager_compile_inputs::EagerCompileInputPoll::NeedExternal(expected) =
            self.poll_compiler(db, environment, authorities)?
        else {
            return Err(fault("no external compiler operand is currently demanded"));
        };
        if expected.key() != request.key()
            || expected.selected() != request.selected()
            || expected.site() != request.site()
            || expected
                .original()
                .encode()
                .map_err(|e| fault(&e.message))?
                != request.original().encode().map_err(|e| fault(&e.message))?
        {
            return Err(fault(
                "external compiler operand differs from current demand",
            ));
        }
        if request.key().grammar != super::identity::GrammarSlot::Expression
            || request.key().macro_identity
                != (super::identity::MacroIdentity::Declaration {
                    definition: request.selected().identity,
                })
        {
            return Err(fault(
                "external compiler operand requires an owned declarative definition",
            ));
        }
        if self.pending_generated.is_some() {
            let scope = self.generated_compiler_scope()?;
            return scope.catalog.expand_string_operand(
                request.selected(),
                request.key(),
                request.original(),
                request.site(),
                scope.compiler_context(),
                &scope.ancestors,
            );
        }
        if let Some((_, producer, _)) = &self.pending_module {
            let staged = self
                .staged_modules
                .get(producer)
                .ok_or_else(|| fault("compiler module source absent"))?;
            let providers = &self
                .declarative
                .as_ref()
                .ok_or_else(|| fault("compiler source providers absent"))?
                .2;
            let inherited = self.module_projection_inherited(&staged.syntax)?;
            let context = super::source_declarative::CapturedMacroContext {
                exports: self.exports.as_deref(),
                provider: &staged.provider,
                owners: providers,
                before: 0,
                inherited: Some(&inherited),
            };
            if let Some(items) = context.inherited_body(request.selected()) {
                return super::source_declarative::SourceDeclarativeRequest::module_generated(
                    request.selected(),
                    request.key().clone(),
                    request.original(),
                    request.site(),
                    items,
                )?
                .expand();
            }
            return context
                .definition_owner(request.selected())?
                .generated_request(
                    request.selected(),
                    request.key().clone(),
                    request.original(),
                    request.site(),
                )?
                .expand();
        }
        // Match poll_compiler's owner priority: generated, module, ordinary,
        // then cold/argument declaration work. A retained child cannot replace
        // the currently selected ordinary demand's body or diagnostic parents.
        if self.pending.is_none()
            && let Some(parent) = self.pending_declaration_request()
        {
            return parent.expand_declaration_compiler_operand(request);
        }
        let (ordinal, _) = self
            .calls
            .get(
                &self
                    .pending
                    .as_ref()
                    .ok_or_else(|| fault("compiler source demand absent"))?
                    .0
                    .job()
                    .identity,
            )
            .ok_or_else(|| fault("compiler source call absent"))?;
        let providers = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("compiler source providers absent"))?
            .2;
        let context = super::source_declarative::CapturedMacroContext {
            exports: self.exports.as_deref(),
            provider: &providers[*ordinal],
            owners: providers,
            before: 0,
            inherited: None,
        };
        context
            .definition_owner(request.selected())?
            .generated_request(
                request.selected(),
                request.key().clone(),
                request.original(),
                request.site(),
            )?
            .expand()
    }
    pub fn admit_compiler_expansion(
        &mut self,
        db: &mut crate::db::memo::Database,
        environment: &crate::expand::builtins::FrozenCompileEnvironment,
        authorities: &crate::source::include_paths::IncludeAuthorities,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        let crate::service::eager_compile_inputs::EagerCompileInputPoll::Complete(expected) =
            self.poll_compiler(db, environment, authorities)?
        else {
            return Err(fault(
                "compiler output cannot enter discovery before input adoption",
            ));
        };
        if expected.key() != record.key() || expected.output() != record.output() {
            return Err(fault(
                "compiler output differs from adopted input reproduction",
            ));
        }
        self.expansion_budget.check_record(&record)?;
        let mut included = None;
        if self.pending_generated.is_some() {
            self.generated_compiler_work(db, environment, authorities, Some(&mut included))?;
        } else {
            let request = self
                .pending_module
                .as_ref()
                .map(|pending| pending.2.clone())
                .or_else(|| self.pending.as_ref().map(|pending| pending.2.clone()))
                .or_else(|| self.pending_declaration_request().cloned())
                .ok_or_else(|| fault("compiler demand absent"))?;
            let providers = &self
                .declarative
                .as_ref()
                .ok_or_else(|| fault("compiler providers absent"))?
                .2;
            let namespaces = Self::install_macro_routes_with_modules(
                unique_graph(&mut self.graph)?,
                providers,
                &self.module_routes,
                &self.nested_sources,
                &self.nested_inert_proofs,
                &mut self.route_install,
            )?;
            included = request.included_view(
                db,
                &namespaces,
                environment,
                authorities,
                self.compiler_caller_view(&request)?,
            )?;
        }
        let included_base = included
            .as_ref()
            .map(|view| {
                view.tokens()
                    .spans()
                    .first()
                    .cloned()
                    .ok_or_else(|| fault("included invocation source base absent"))
            })
            .transpose()?;
        let base_bytes = included_base
            .as_ref()
            .map(source_base_bytes)
            .transpose()?
            .unwrap_or(0);
        let included_bytes = match included.as_ref() {
            Some(view) => view
                .estimated_bytes()
                .checked_add(128)
                .and_then(|n| n.checked_add(base_bytes))
                .ok_or_else(|| fault("included source metadata overflow"))?,
            None => 0,
        };
        let retained = self
            .retained_record_bytes
            .checked_add(included_bytes)
            .ok_or_else(|| fault("retained included source allocation limit"))?;
        let inputs = db.compiler_input_stamps()?;
        for (key, fingerprint) in &inputs {
            if self
                .compiler_inputs
                .get(key)
                .is_some_and(|old| old != fingerprint)
            {
                return Err(fault("compiler input changed during discovery"));
            }
        }
        let mut admitted = self.compiler_inputs.clone();
        for (key, fingerprint) in inputs {
            admitted.insert(key, fingerprint);
        }
        self.compiler_inputs = admitted;
        let identity = record.key().fingerprint();
        self.retained_record_bytes = retained;
        self.admitting_compiler = true;
        let result = self.admit_expansion(record);
        self.admitting_compiler = false;
        if result.is_ok()
            && let Some(view) = included
        {
            self.included_views
                .insert(identity, std::sync::Arc::new(view));
            self.source_bases
                .insert(identity, included_base.expect("included base checked"));
        }
        result
    }
    /// Exact immutable reply admission. Failed publication poisons this request; it cannot seal.
    pub fn admit_expansion(
        &mut self,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        if self.seal.is_some() || self.failed.is_some() {
            return self.admit_expansion_inner(record);
        }
        if let Some(ticket) = &self.pending_nested {
            if matches!(
                record.key().macro_identity,
                super::identity::MacroIdentity::Procedural { .. }
            ) && !self.admitting_procedural
            {
                return Err(fault("cold procedural reply requires coordinator adoption"));
            }
            if let Some(arguments) = self.argument_discoveries.get_mut(&ticket.job().identity) {
                return arguments.admit(record);
            }
            return self
                .nested_fragments
                .get_mut(&ticket.job().identity)
                .ok_or_else(|| fault("pending nested fragment absent"))?
                .admit(record);
        }
        let expected = self
            .pending_module
            .as_ref()
            .map(|pending| pending.2.key())
            .or_else(|| {
                self.pending_generated
                    .as_ref()
                    .map(|pending| pending.1.key())
            })
            .or_else(|| self.pending.as_ref().map(|pending| pending.2.key()));
        if expected != Some(record.key()) {
            return self.admit_expansion_inner(record);
        }
        let budget = self.expansion_budget.clone();
        budget.transaction(
            &BTreeMap::from([(record.key().fingerprint(), record.usage())]),
            || self.admit_expansion_inner(record),
        )
    }
    /// Bind pure assembly to the exact currently suspended source selection.
    pub fn bind_procedural_request(
        &mut self,
        request: std::sync::Arc<crate::service::function_macro_requests::PreparedFunctionRequest>,
    ) -> Result<(), Diagnostic> {
        let (ticket, ordinal, selection) = self
            .pending_procedural
            .as_ref()
            .ok_or_else(|| fault("no procedural source selection pending"))?;
        let selected = selection
            .catalog
            .select(
                &selection.definition,
                super::identity::ProceduralKind::Function,
            )?
            .ok_or_else(|| fault("procedural selected export absent"))?;
        if request.source() != selection.input.source_selection()
            || request.key().invocation != selection.input.invocation_identity()
            || &request.key().macro_identity != selected.identity()
            || request.key().procedural_epoch != Some(self.epoch)
            || request.key().consumer_compile_env != selection.catalog.environment()
        {
            return Err(fault(
                "procedural request differs from selected source/epoch/export",
            ));
        }
        let bytes = self
            .procedural_selection_bytes
            .checked_add(256)
            .ok_or_else(|| fault("procedural selection metadata limit"))?;
        if self
                .procedural_selections
                .contains_key(&ticket.job().identity) {
            return Err(fault("procedural selection count/conflict"));
        }
        let prepared = std::sync::Arc::new(
            super::source_declarative::SourceDeclarativeRequest::procedural(
                request,
                selection.definition.defining_unit,
            )?,
        );
        let selected_identity = selection.input.invocation_identity();
        let selected_definition = selection.definition.clone();
        if selection.nested && self.nested_fragments.contains_key(&ticket.job().identity) {
            self.nested_fragments
                .get_mut(&ticket.job().identity)
                .ok_or_else(|| fault("nested procedural fragment absent"))?
                .bind_procedural_request(prepared)?;
            self.pending_nested = Some(ticket.clone());
        } else if selection.nested
            && self
                .argument_discoveries
                .contains_key(&ticket.job().identity)
        {
            self.argument_discoveries
                .get_mut(&ticket.job().identity)
                .ok_or_else(|| fault("argument procedural assembly absent"))?
                .bind_procedural_request(prepared)?;
            self.pending_nested = Some(ticket.clone());
        } else if selection.nested {
            let parent = self
                .nested_sources
                .get(ordinal)
                .ok_or_else(|| fault("cold procedural parent absent"))?
                .clone();
            let (environment, engine, providers) = self
                .declarative
                .as_ref()
                .ok_or_else(|| fault("cold procedural providers absent"))?;
            let fragment = super::declaration_fragment::DeclarationFragment::from_prepared_source(
                parent,
                super::declaration_fragment::DeclarationFragmentInput {
                    invocation: ticket.job().identity,
                    request: self.expansion_budget.request_identity(),
                    graph: self.key.unit_graph,
                    compile_env: *environment,
                    engine: *engine,
                    epoch: self.epoch,
                    cfg: providers[*ordinal]
                        .module_source()
                        .captured_macro_cfg()?
                        .clone(),
                },
                self.expansion_budget.clone(),
                prepared,
            )?;
            self.nested_fragments
                .insert(ticket.job().identity, fragment);
            self.pending_nested = Some(ticket.clone());
        } else if let Some(producer) = selection.module {
            self.pending_module = Some((ticket.clone(), producer, prepared));
        } else {
            self.pending = Some((ticket.clone(), *ordinal, prepared));
        }
        self.procedural_selections
            .insert(selected_identity, selected_definition);
        self.procedural_selection_bytes = bytes;
        self.pending_procedural = None;
        Ok(())
    }
    pub fn bind_attribute_request(
        &mut self,
        request: std::sync::Arc<crate::service::function_macro_requests::PreparedFunctionRequest>,
    ) -> Result<(), Diagnostic> {
        let (ticket, owner, selection) = self
            .pending_attribute
            .as_ref()
            .ok_or_else(|| fault("no source attribute selection pending"))?;
        if !selection.matches_request(&request)?
            || request.key().invocation != selection.producer()
            || request.key().procedural_epoch != Some(self.epoch)
            || !matches!(
                request.key().macro_identity,
                super::identity::MacroIdentity::Procedural {
                    kind: super::identity::ProceduralKind::Attribute,
                    ..
                }
            )
        {
            return Err(fault(
                "source attribute request differs from exact selection",
            ));
        }
        // The frozen session has validated the export; preserve its selected source identity for replay.
        let bytes = self
            .procedural_selection_bytes
            .checked_add(selection.metadata_bytes()?)
            .ok_or_else(|| fault("source attribute selection metadata limit"))?;
        if self
                .attribute_selections
                .contains_key(&selection.producer()) {
            return Err(fault("source attribute selection count/conflict"));
        }
        let prepared = std::sync::Arc::new(
            super::source_declarative::SourceDeclarativeRequest::procedural(
                request,
                selection.definition().defining_unit,
            )?,
        );
        match owner {
            AttributeSourceOwner::Original(ordinal) => {
                self.pending = Some((ticket.clone(), *ordinal, prepared))
            }
            AttributeSourceOwner::Module(producer) => {
                self.pending_module = Some((ticket.clone(), *producer, prepared))
            }
            AttributeSourceOwner::Fragment(_ordinal) => {
                if let Some(fragment) = self.nested_fragments.get_mut(&ticket.job().identity) {
                    fragment.bind_attribute_request(prepared)?;
                } else if let Some(arguments) =
                    self.argument_discoveries.get_mut(&ticket.job().identity)
                {
                    arguments.bind_attribute_request(prepared)?;
                } else {
                    return Err(fault("generated attribute declaration fragment absent"));
                }
                self.pending_nested = Some(ticket.clone());
            }
            AttributeSourceOwner::Cold(ordinal) => {
                let (environment, engine, providers) = self
                    .declarative
                    .as_ref()
                    .ok_or_else(|| fault("cold attribute providers absent"))?;
                let parent = self
                    .nested_sources
                    .get(ordinal)
                    .ok_or_else(|| fault("cold attribute parent absent"))?
                    .clone();
                let fragment =
                    super::declaration_fragment::DeclarationFragment::from_prepared_source(
                        parent,
                        super::declaration_fragment::DeclarationFragmentInput {
                            invocation: ticket.job().identity,
                            request: self.expansion_budget.request_identity(),
                            graph: self.key.unit_graph,
                            compile_env: *environment,
                            engine: *engine,
                            epoch: self.epoch,
                            cfg: providers[*ordinal]
                                .module_source()
                                .captured_macro_cfg()?
                                .clone(),
                        },
                        self.expansion_budget.clone(),
                        prepared,
                    )?;
                if self
                    .nested_fragments
                    .insert(ticket.job().identity, fragment)
                    .is_some()
                    || self
                        .cold_attribute_owners
                        .insert(ticket.job().identity, *ordinal)
                        .is_some()
                {
                    return Err(fault("cold attribute fragment identity conflict"));
                }
                self.pending_nested = Some(ticket.clone());
            }
        }
        self.attribute_selections
            .insert(selection.producer(), selection.clone());
        self.procedural_selection_bytes = bytes;
        self.pending_attribute = None;
        Ok(())
    }
    fn replay_attribute_selection(
        &self,
        ordinal: usize,
        identity: Digest,
        providers: &[super::source_declarative::SourceDeclarative],
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
    ) -> Result<
        Option<std::sync::Arc<crate::service::source_attribute_requests::SourceAttributeSelection>>,
        Diagnostic,
    > {
        if self
            .attribute_selections
            .get(&identity)
            .is_some_and(|selection| selection.has_completed_derives())
        {
            return match super::source_derives::prepare_group(
                &self.sources[ordinal],
                &providers[ordinal],
                identity,
                namespaces,
                &self.completed_source_derives,
                &self.helper_declarations,
            )? {
                super::source_derives::SourceDerivePoll::Attribute(selection) => {
                    Ok(Some(selection))
                }
                _ => Ok(None),
            };
        }
        crate::service::source_attribute_requests::SourceAttributeSelection::select(
            &self.sources[ordinal],
            &providers[ordinal],
            identity,
            namespaces,
        )
        .map(|selection| selection.map(std::sync::Arc::new))
    }
    pub fn bind_source_derive_request(
        &mut self,
        request: std::sync::Arc<crate::service::function_macro_requests::PreparedFunctionRequest>,
    ) -> Result<(), Diagnostic> {
        let pending = self
            .pending_source_derive
            .as_mut()
            .ok_or_else(|| fault("no original derive selection pending"))?;
        if pending.request.is_some()
            || self.seal.is_some()
            || self.failed.is_some()
            || !pending.selection.matches_request(&request)?
            || request.key().procedural_epoch != Some(self.epoch)
        {
            return Err(fault(
                "original derive request differs from exact active selection",
            ));
        }
        pending.request = Some(request);
        Ok(())
    }
    pub(crate) fn source_derive_failure_origin(
        &self,
        request: &crate::service::function_macro_requests::PreparedFunctionRequest,
    ) -> Result<crate::syntax::origin::MacroOrigin, Diagnostic> {
        self.require_source_derive_request(request)?;
        self.pending_source_derive
            .as_ref()
            .ok_or_else(|| fault("source derive failure selection absent"))?
            .selection
            .diagnostic_origin(request)
    }
    pub(crate) fn require_source_derive_request(
        &self,
        request: &crate::service::function_macro_requests::PreparedFunctionRequest,
    ) -> Result<(), Diagnostic> {
        let pending = self
            .pending_source_derive
            .as_ref()
            .ok_or_else(|| fault("no original derive demand pending"))?;
        if pending.record.is_some()
            || self.seal.is_some()
            || self.failed.is_some()
            || pending
                .request
                .as_ref()
                .is_none_or(|bound| bound.key() != request.key())
            || !pending.selection.matches_request(request)?
        {
            return Err(fault(
                "original derive execution requires the exact bound active request",
            ));
        }
        Ok(())
    }
    pub(crate) fn validate_source_derive_database(
        &self,
        db: &mut crate::db::memo::Database,
        request: &crate::service::function_macro_requests::PreparedFunctionRequest,
    ) -> Result<(), Diagnostic> {
        self.require_source_derive_request(request)?;
        self.pending_source_derive
            .as_ref()
            .ok_or_else(|| fault("source derive phase absent"))?
            .selection
            .validate_database(db)
    }
    pub(crate) fn retain_source_derive_output(
        &mut self,
        db: &mut crate::db::memo::Database,
        request: &crate::service::function_macro_requests::PreparedFunctionRequest,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        db.require_coordinator()?;
        self.require_source_derive_request(request)?;
        let pending = self
            .pending_source_derive
            .as_ref()
            .ok_or_else(|| fault("source derive selection absent"))?;
        pending.selection.validate_database(db)?;
        use crate::db::dependency::{InputKey, InputValue};
        if record.key() != request.key()
            || !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),Some(InputValue::ExpansionEpoch(epoch)) if *epoch==self.epoch)
            || !matches!(db.input(&InputKey::ExpansionRecord(record.key().fingerprint())).as_deref(),Some(InputValue::ExpansionRecord(actual)) if actual.descriptor().fingerprint()==record.descriptor().fingerprint())
        {
            return Err(fault(
                "source derive output is not the exact adopted epoch record",
            ));
        }
        let stamps = db.compiler_input_stamps()?;
        let _additional = stamps
            .keys()
            .filter(|key| !self.compiler_inputs.contains_key(*key))
            .count();
        if stamps.iter().any(|(key, value)| {
            self.compiler_inputs
                .get(key)
                .is_some_and(|old| old != value)
        }) {
            return Err(fault("source derive input stamps changed or exceed limit"));
        }
        let id = record.key().fingerprint();
        if self.records.contains_key(&id) {
            return Err(fault("source derive record conflict or count limit"));
        }
        let retained = self
            .retained_record_bytes
            .checked_add(record.estimated_bytes()?)
            .and_then(|n| n.checked_add(512))
            .ok_or_else(|| fault("source derive record retention limit"))?;
        self.compiler_inputs.extend(stamps);
        self.records.insert(id, record.clone());
        self.retained_record_bytes = retained;
        self.pending_source_derive
            .as_mut()
            .ok_or_else(|| fault("source derive phase absent"))?
            .record = Some(record);
        Ok(())
    }
    pub(crate) fn require_source_derive_append(
        &self,
        request: &crate::service::function_macro_requests::PreparedFunctionRequest,
        record: &super::record::ExpansionRecord,
    ) -> Result<(), Diagnostic> {
        let pending = self
            .pending_source_derive
            .as_ref()
            .ok_or_else(|| fault("original derive append absent"))?;
        if self.seal.is_some()
            || self.failed.is_some()
            || pending
                .request
                .as_ref()
                .is_none_or(|bound| bound.key() != request.key())
            || !pending.selection.matches_request(request)?
            || pending.record.as_ref().is_none_or(|old| {
                old.key() != record.key()
                    || old.descriptor().fingerprint() != record.descriptor().fingerprint()
            })
            || record.key() != request.key()
        {
            return Err(fault(
                "source derive append differs from exact adopted output",
            ));
        }
        Ok(())
    }
    pub(crate) fn integrate_source_derive_append(
        &mut self,
        db: &mut crate::db::memo::Database,
        request: &crate::service::function_macro_requests::PreparedFunctionRequest,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        db.require_coordinator()?;
        self.require_source_derive_append(request, &record)?;
        let pending = self
            .pending_source_derive
            .as_ref()
            .ok_or_else(|| fault("source derive append phase absent"))?;
        pending.selection.validate_database(db)?;
        use crate::db::dependency::{InputKey, InputValue};
        if !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),Some(InputValue::ExpansionEpoch(epoch)) if *epoch==self.epoch)
            || !matches!(db.input(&InputKey::ExpansionRecord(record.key().fingerprint())).as_deref(),Some(InputValue::ExpansionRecord(actual)) if actual.descriptor().fingerprint()==record.descriptor().fingerprint())
        {
            return Err(fault(
                "source derive append database record or epoch differs",
            ));
        }
        let ticket = pending.ticket.clone();
        let selection = pending.selection.clone();
        let source_base = super::source_declarative::SourceDeclarativeRequest::procedural(
            pending
                .request
                .as_ref()
                .ok_or_else(|| fault("source derive bound request absent"))?
                .clone(),
            selection.scope().crate_key,
        )?
        .source_base()?;
        self.append_source_derive_output(ticket, selection, record, source_base)
    }
    /// Expand a builtin derive occurrence in process (the pinned adapter's
    /// compiler expander over the exact item input) and append its items
    /// through the same route as a procedural derive's adopted output.
    fn expand_builtin_source_derive(
        &mut self,
        ticket: JobTicket,
        selection: std::sync::Arc<crate::service::source_derive_requests::SourceDeriveSelection>,
    ) -> Result<(), Diagnostic> {
        let (compile_env, engine, _) = self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("builtin derive providers absent"))?;
        let request = selection.builtin_request(*compile_env, *engine)?;
        let record = std::sync::Arc::new(request.expand()?);
        let id = record.key().fingerprint();
        if self.records.contains_key(&id) {
            return Err(fault("builtin derive record conflict"));
        }
        let retained = self
            .retained_record_bytes
            .checked_add(record.estimated_bytes()?)
            .and_then(|n| n.checked_add(512))
            .ok_or_else(|| fault("builtin derive record retention overflow"))?;
        let source_base = request.source_base()?;
        self.records.insert(id, record.clone());
        self.retained_record_bytes = retained;
        let result = self.append_source_derive_output(ticket, selection, record, source_base);
        if result.is_err() {
            self.records.remove(&id);
        }
        result
    }
    /// Append one derive occurrence's output items after its original item:
    /// generated items, declarations, impl headers and macro catalog join the
    /// ordinary discovery jobs of the owning source.
    fn append_source_derive_output(
        &mut self,
        ticket: JobTicket,
        selection: std::sync::Arc<crate::service::source_derive_requests::SourceDeriveSelection>,
        record: std::sync::Arc<super::record::ExpansionRecord>,
        source_base: crate::syntax::token_wire::SpanRecord,
    ) -> Result<(), Diagnostic> {
        let (ordinal, call) = self
            .calls
            .get(&ticket.job().identity)
            .ok_or_else(|| fault("source derive original producer absent"))?;
        let ordinal = *ordinal;
        let call = call.clone();
        let id = record.key().fingerprint();
        if self.fragments.contains_key(&id) || self.source_derives_inflight.contains_key(&call.item_input) {
            return Err(fault("source derive append inventory conflict or limit"));
        }
        let items = super::generated_items::GeneratedItems::admit(
            record.key(),
            record.clone(),
            self.epoch,
        )?;
        let cfg = self.sources[ordinal].captured_macro_cfg()?;
        let declaration = std::sync::Arc::new(
            super::generated_declarations::GeneratedDeclarations::admit_source_derive_output(
                &selection,
                record.clone(),
                self.epoch,
                &self.sources[ordinal],
                cfg,
            )?,
        );
        let mut encoded = crate::core::digest::StableEncoder::new("lrr/v2/item-declaration-job");
        encoded.digest(declaration.fingerprint());
        let declaration_job = encoded.finish();
        let plan = crate::frontend::generated_headers::GeneratedHeaderPlan::prepare(
            &items,
            call.scope.clone(),
            crate::syntax::token_wire::Key(record.key().parent_context.0),
            self.sources[ordinal].captured_macro_cfg()?,
            self.epoch,
        )?;
        let impl_headers = declaration.item_impl_headers(&items, cfg, &plan)?;
        if self.nested_jobs.contains_key(&declaration_job) || self.item_declaration_inputs.contains_key(&declaration_job) || self.item_root_declarations.contains_key(&id) {
            return Err(fault(
                "source derive declaration inventory conflict or limit",
            ));
        }
        let nested_bytes = self
            .nested_bytes
            .checked_add(declaration.metadata_bytes()?)
            .and_then(|n| n.checked_add(impl_headers.len().checked_mul(256)?))
            .and_then(|n| n.checked_add(256))
            .and_then(|n| n.checked_add(call.scope.heap_bytes()))
            .ok_or_else(|| fault("source derive declaration metadata limit"))?;
        let impl_frontier = if impl_headers.is_empty() {
            None
        } else {
            Some(
                plan.impl_frontier()
                    .ok_or_else(|| fault("derive impl frontier absent"))?,
            )
        };
        if impl_frontier.is_some_and(|frontier| self.item_impl_jobs.contains_key(&frontier)) {
            return Err(fault("derive impl frontier conflict"));
        }
        let catalog = crate::frontend::generated_macros::GeneratedMacroCatalog::for_discovery(
            &items,
            call.scope.clone(),
            crate::syntax::token_wire::Key(record.key().parent_context.0),
            self.sources[ordinal].captured_macro_cfg()?,
            self.epoch,
        )?;
        let provider = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("source derive provider absent"))?
            .2[ordinal];
        let prefix = provider.derive_prefix_snapshot(&call)?;
        let allocated = self
            .retained_record_bytes
            .checked_add(catalog.metadata_bytes()?)
            .and_then(|n| n.checked_add(prefix.module_snapshot_bytes().ok()?))
            .and_then(|n| n.checked_add(source_base_bytes(&source_base).ok()?))
            .and_then(|n| n.checked_add(call.scope.heap_bytes()))
            .and_then(|n| n.checked_add(1024))
            .ok_or_else(|| fault("source derive append retention limit"))?;
        let wire = self
            .generated_bytes
            .checked_add(record.descriptor().wire_bytes)
            .ok_or_else(|| fault("source derive append wire limit"))?;
        let headers = self
            .generated_headers
            .checked_add(items.items().len())
            .ok_or_else(|| fault("source derive append header limit"))?;
        let imports = plan
            .discovery_imports()
            .iter()
            .map(NameNamespaces::import_identity)
            .collect::<BTreeSet<_>>();
        let release = catalog
            .outward()
            .iter()
            .map(|entry| entry.definition.identity)
            .chain(catalog.call_frontiers().iter().copied())
            .collect::<BTreeSet<_>>();
        let opaque = plan
            .blocked()
            .filter(|id| !imports.contains(id) && !release.contains(id))
            .copied()
            .collect::<BTreeSet<_>>();
        let job_count = imports
            .len()
            .checked_add(opaque.len())
            .and_then(|n| n.checked_add(2))
            .ok_or_else(|| fault("source derive append job limit"))?;
        let mut jobs = Vec::new();
        jobs.try_reserve_exact(job_count)
            .map_err(|_| fault("source derive append job allocation"))?;
        for identity in &imports {
            jobs.push(DiscoveryJob {
                module_order: ticket.job().module_order,
                source_offset: call.range.start,
                kind: DiscoveryKind::ResolveImport,
                identity: *identity,
            });
        }
        for identity in &opaque {
            jobs.push(DiscoveryJob {
                module_order: ticket.job().module_order,
                source_offset: call.range.start,
                kind: DiscoveryKind::ScanNestedDeclarations,
                identity: *identity,
            });
        }
        jobs.push(DiscoveryJob {
            module_order: ticket.job().module_order,
            source_offset: call.range.start,
            kind: DiscoveryKind::ScanNestedDeclarations,
            identity: id,
        });
        jobs.push(DiscoveryJob {
            module_order: ticket.job().module_order,
            source_offset: call.range.start,
            kind: DiscoveryKind::ScanNestedDeclarations,
            identity: declaration_job,
        });
        self.work.preflight_enqueue_batch(&mut jobs)?;
        let mut members = Vec::new();
        members
            .try_reserve_exact(
                items
                    .items()
                    .len()
                    .checked_add(2)
                    .ok_or_else(|| fault("source derive member limit"))?,
            )
            .map_err(|_| fault("source derive member allocation"))?;
        members.extend(items.items().iter().map(|h| h.key.fingerprint()));
        members.push(record.descriptor().fingerprint());
        members.push(selection.input().identity());
        let mut bound_impl_headers = Vec::new();
        bound_impl_headers
            .try_reserve_exact(impl_headers.len())
            .map_err(|_| fault("derive impl frontier allocation"))?;
        bound_impl_headers.extend(impl_headers.iter().map(|(_, header)| *header));
        let result = (|| {
            self.retain_module_sources(&plan)?;
            plan.publish(unique_graph(&mut self.graph)?)?;
            self.work.block(&ticket, vec![id])?;
            self.work.enqueue_batch(jobs)?;
            self.fragments.insert(
                id,
                GeneratedFragment {
                    module: None,
                    source_derive: Some(selection),
                    source_prefix: Some(prefix),
                    catalog,
                    children: BTreeMap::new(),
                    child_order: BTreeMap::new(),
                    active_child: None,
                    jobs: Vec::new(),
                    ordinal,
                    call: call.clone(),
                    release,
                    members,
                    dependencies: vec![(id, record.descriptor().fingerprint())],
                },
            );
            self.nested_jobs.insert(declaration_job, ordinal);
            self.item_declaration_inputs
                .insert(declaration_job, declaration);
            self.item_root_declarations.insert(id, declaration_job);
            if let Some(frontier) = impl_frontier {
                self.item_impl_jobs.insert(
                    frontier,
                    (declaration_job, bound_impl_headers, call.scope.clone()),
                );
            }
            self.nested_bytes = nested_bytes;
            self.generated_owners
                .insert(id, (ordinal, call.scope.clone()));
            self.source_bases.insert(id, source_base);
            self.source_derives_inflight.insert(call.item_input, id);
            self.producers.insert(id);
            self.producers.extend(opaque);
            self.retained_record_bytes = allocated;
            self.generated_bytes = wire;
            self.generated_headers = headers;
            self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
            self.pending_source_derive = None;
            Ok::<_, Diagnostic>(())
        })();
        if let Err(error) = &result {
            self.failed = Some(error.clone());
        }
        result
    }
    fn complete_source_derive(
        &mut self,
        ticket: &JobTicket,
        ordinal: usize,
        call: &crate::syntax::outline::ItemInvocation,
        proof: super::source_derives::SourceDeriveCompletionProof,
    ) -> Result<(), Diagnostic> {
        if self
            .source_derive_completions
            .contains_key(&call.item_input) || self.source_derives_inflight.contains_key(&call.item_input) || self
                .frontier_producers
                .get(&(call.scope.clone(), call.frontier))
                .is_none_or(|frontier| !frontier.contains(&call.item_input)) {
            return Err(fault(
                "source derive completion frontier or retention differs",
            ));
        }
        let mut update = DiscoveryUpdate {
            members: vec![call.item_input, proof.item, proof.fingerprint],
            ..Default::default()
        };
        self.work.preflight_complete(ticket, &mut update)?;
        let result = (|| {
            unique_graph(&mut self.graph)?.restore_source_attribute_declaration(
                self.sources[ordinal].outline(),
                proof.item,
            )?;
            self.declarative
                .as_mut()
                .ok_or_else(|| fault("source completion provider absent"))?
                .2[ordinal]
                .complete_without_macro_bindings(call.item_input)?;
            let frontier = self
                .frontier_producers
                .get_mut(&(call.scope.clone(), call.frontier))
                .ok_or_else(|| fault("source completion frontier absent"))?;
            if !frontier.remove(&call.item_input) {
                return Err(fault("source derive already completed"));
            }
            if frontier.is_empty() {
                unique_graph(&mut self.graph)?
                    .release_generated_frontier(&call.scope, call.frontier);
            }
            Self::install_macro_routes_with_modules(
                unique_graph(&mut self.graph)?,
                &self
                    .declarative
                    .as_ref()
                    .ok_or_else(|| fault("source completion providers absent"))?
                    .2,
                &self.module_routes,
                &self.nested_sources,
                &self.nested_inert_proofs,
                &mut self.route_install,
            )?;
            self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
            self.source_derive_completions
                .insert(call.item_input, proof);
            self.producers.remove(&call.item_input);
            self.work.complete(ticket, update)?;
            Ok::<_, Diagnostic>(())
        })();
        if let Err(error) = &result {
            self.failed = Some(error.clone());
        }
        result
    }
    /// Native output must already be adopted by the authorized run coordinator.
    pub fn admit_procedural_expansion(
        &mut self,
        db: &mut crate::db::memo::Database,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        use crate::db::dependency::{InputKey, InputValue};
        let pending = self
            .pending_module
            .as_ref()
            .map(|pending| &pending.2)
            .or_else(|| self.pending.as_ref().map(|pending| &pending.2))
            .or_else(|| {
                self.pending_nested
                    .as_ref()
                    .and_then(|ticket| self.nested_fragments.get(&ticket.job().identity))
                    .and_then(|fragment| fragment.pending_request())
                    .or_else(|| {
                        self.pending_nested
                            .as_ref()
                            .and_then(|ticket| {
                                self.argument_discoveries.get(&ticket.job().identity)
                            })
                            .and_then(|arguments| arguments.pending_request())
                    })
            });
        let matches_source = pending.is_some_and(|pending| {
            pending.procedural_request().is_some() && pending.key() == record.key()
        });
        let matches_generated = self.pending_generated.as_ref().is_some_and(|(_, pending)| {
            pending.procedural_request().is_some() && pending.key() == record.key()
        });
        if !matches_source && !matches_generated {
            return Err(fault(
                "native output differs from pending procedural request",
            ));
        }
        let adopted = db
            .input(&InputKey::ExpansionRecord(record.key().fingerprint()))
            .ok_or_else(|| fault("procedural source output requires coordinator adoption"))?;
        let InputValue::ExpansionRecord(adopted) = adopted.as_ref() else {
            return Err(fault("procedural adopted input kind differs"));
        };
        if adopted.key() != record.key()
            || adopted.descriptor().fingerprint() != record.descriptor().fingerprint()
        {
            return Err(fault("procedural adopted descriptor differs"));
        }
        if !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(), Some(InputValue::ExpansionEpoch(epoch)) if *epoch == self.epoch)
        {
            return Err(fault("procedural database epoch differs"));
        }
        let stamps = db.compiler_input_stamps()?;
        for (key, value) in &stamps {
            if self
                .compiler_inputs
                .get(key)
                .is_some_and(|old| old != value)
            {
                return Err(fault("procedural input changed during discovery"));
            }
        }
        let _additional = stamps
            .keys()
            .filter(|key| !self.compiler_inputs.contains_key(*key))
            .count();
        self.compiler_inputs.extend(stamps);
        self.admitting_procedural = true;
        let result = self.admit_expansion(record);
        self.admitting_procedural = false;
        result
    }
    fn observe_output(
        usage: &mut BTreeMap<Digest, super::record::ExpansionUsage>,
        record: &super::record::ExpansionRecord,
    ) -> Result<(), Diagnostic> {
        let identity = record.key().fingerprint();
        let observed = record.usage();
        if let Some(old) = usage.get(&identity) {
            if old.fingerprint != observed.fingerprint {
                return Err(Diagnostic::new(
                    "LRR-E-EXPANSION-CONFLICT",
                    "expansion output observation conflicts with adopted record".into(),
                ));
            }
        } else {
            usage.insert(identity, observed);
        }
        Ok(())
    }
    pub fn generated_token_count(&self) -> Result<usize, Diagnostic> {
        Ok(self.expansion_budget.totals()?.tokens)
    }
    pub fn accounted_expansion_count(&self) -> Result<usize, Diagnostic> {
        Ok(self.expansion_budget.totals()?.outputs)
    }
    pub fn with_expansion_budget(
        mut self,
        request: Digest,
        budget: super::budget::SharedExpansionBudget,
    ) -> Result<Self, Diagnostic> {
        if self.started || !budget.matches(request, self.key.unit_graph, self.epoch) {
            return Err(Diagnostic::new(
                "LRR-E-EXPANSION-INPUT",
                "discovery expansion budget owner differs or attachment is late".into(),
            ));
        }
        self.expansion_budget = budget;
        Ok(self)
    }
    fn admit_expansion_inner(
        &mut self,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        if self.seal.is_some() {
            return Err(Diagnostic::new(
                "LRR-E-EXPANSION-SEAL-VIOLATION",
                "scope discovery is already sealed".into(),
            ));
        }
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        if self.pending_module.is_some() {
            return self.admit_pending_module_expansion(record);
        }
        if self.pending_generated.is_some() {
            return self.admit_generated_expansion(record);
        }
        let (_, ordinal, request) = self
            .pending
            .as_ref()
            .ok_or_else(|| fault("no source expansion demand is pending"))?;
        if request.is_compiler() && !self.admitting_compiler {
            return Err(fault(
                "compiler discovery output requires explicit adopted input admission",
            ));
        }
        if request.procedural_request().is_some() && !self.admitting_procedural {
            return Err(fault("procedural output requires adopted input admission"));
        }
        if record.key() != request.key() {
            return Err(Diagnostic::new(
                "LRR-E-EXPANSION-INPUT",
                "source expansion reply does not match exact pending demand".into(),
            ));
        }
        let identity = record.key().fingerprint();
        if self.records.contains_key(&identity) {
            return Err(fault("expansion record identity was already admitted"));
        }
        let retained_bytes = self
            .retained_record_bytes
            .checked_add(record.estimated_bytes()?)
            .and_then(|n| n.checked_add(512))
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "retained expansion record allocation limit".into(),
                )
            })?;
        let source_base = request.source_base()?;
        let retained_bytes = retained_bytes
            .checked_add(source_base_bytes(&source_base)?)
            .ok_or_else(|| fault("retained invocation source base limit"))?;
        let ordinal = *ordinal;
        let (ticket, _, _) = self.pending.as_ref().expect("validated pending expansion");
        let ticket = ticket.clone();
        let (_, call) = self
            .calls
            .get(&ticket.job().identity)
            .ok_or_else(|| fault("pending source call absent"))?;
        let call = call.clone();
        let retained_bytes = retained_bytes
            .checked_add(call.scope.heap_bytes())
            .ok_or(Diagnostic::new(
                "LRR-E-DISCOVERY-LIMIT",
                "generated owner allocation limit".into(),
            ))?;
        let bytes = self
            .generated_bytes
            .checked_add(record.descriptor().wire_bytes)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "cumulative generated wire limit".into(),
                )
            })?;
        // The exact owned reply is immutable: invalid item syntax is a terminal
        // discovery failure, rather than a fresh expansion demand on the next poll.
        let items = match super::generated_items::GeneratedItems::admit(
            request.key(),
            record.clone(),
            self.epoch,
        ) {
            Ok(items) => items,
            Err(error) => {
                self.failed = Some(error.clone());
                return Err(error);
            }
        };
        let replacement = if call.attribute_input.is_some() {
            Some(std::sync::Arc::new(
                super::source_replacement::SourceReplacementProof::from_adopted(
                    &self.sources[ordinal],
                    &call,
                    &record,
                )?,
            ))
        } else {
            None
        };
        let replacement_preflight = if let Some(proof) = &replacement {
            if self.source_replacements.contains_key(&proof.fingerprint()) {
                return Err(fault("source replacement count/conflict"));
            }
            let mut parent = (**self
                .nested_sources
                .get(&ordinal)
                .ok_or_else(|| fault("replacement parent absent"))?)
            .clone();
            let retired = parent.retire_replaced_item(proof)?;
            let bytes = proof
                .metadata_bytes()?
                .checked_add(parent.retained_metadata_bytes())
                .and_then(|n| n.checked_add(retired.len().checked_mul(128)?))
                .ok_or_else(|| fault("replacement metadata limit"))?;
            Some((parent, retired, bytes))
        } else {
            None
        };
        let declaration_output = if self.declarative.is_some() {
            Some(std::sync::Arc::new(items.declaration_facet(
                request,
                &self.sources[ordinal],
                self.sources[ordinal].captured_macro_cfg()?,
                Default::default(),
            )?))
        } else {
            None
        };
        let plan = crate::frontend::generated_headers::GeneratedHeaderPlan::prepare(
            &items,
            call.scope.clone(),
            crate::syntax::token_wire::Key(request.key().parent_context.0),
            self.sources[ordinal].captured_macro_cfg()?,
            self.epoch,
        )?;
        let declaration_bytes = declaration_output
            .as_ref()
            .map(|output| output.metadata_bytes())
            .transpose()?
            .unwrap_or(0);
        // Keep the same opaque implementation frontier used by derive output
        // tied to the completed, source-owned declaration inventory. The item
        // header plan cannot discharge this frontier merely by seeing syntax.
        let impl_headers = declaration_output
            .as_ref()
            .map(|output| {
                output.item_impl_headers(&items, self.sources[ordinal].captured_macro_cfg()?, &plan)
            })
            .transpose()?
            .unwrap_or_default();
        let nested_bytes = self
            .nested_bytes
            .checked_add(declaration_bytes)
            .and_then(|n| n.checked_add(impl_headers.len().checked_mul(256)?))
            .and_then(|n| {
                n.checked_add(if impl_headers.is_empty() {
                    0
                } else {
                    call.scope.heap_bytes()
                })
            })
            .and_then(|n| n.checked_add(256))
            .ok_or_else(|| fault("item declaration metadata limit"))?;
        let declaration_identity = declaration_output.as_ref().map(|output| {
            let mut e = crate::core::digest::StableEncoder::new("lrr/v2/item-declaration-job");
            e.digest(output.fingerprint());
            e.finish()
        });
        if declaration_identity.is_some_and(|job| {
            self.nested_jobs.contains_key(&job)
                || self.item_declaration_inputs.contains_key(&job)
                || self.item_root_declarations.contains_key(&identity)
        }) {
            return Err(fault("item declaration job conflict"));
        }
        let final_nested_bytes = nested_bytes
            .checked_add(
                replacement_preflight
                    .as_ref()
                    .map(|(_, _, bytes)| *bytes)
                    .unwrap_or(0),
            )
            .ok_or_else(|| fault("replacement retained size limit"))?;
        let impl_frontier = if impl_headers.is_empty() {
            None
        } else {
            Some(
                plan.impl_frontier()
                    .ok_or_else(|| fault("item impl frontier absent"))?,
            )
        };
        if impl_frontier.is_some_and(|frontier| self.item_impl_jobs.contains_key(&frontier)) {
            return Err(fault("item impl frontier conflict"));
        }
        let headers = self
            .generated_headers
            .checked_add(items.items().len())
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "cumulative generated header limit".into(),
                )
            })?;
        let imports = plan
            .discovery_imports()
            .iter()
            .map(NameNamespaces::import_identity)
            .collect::<BTreeSet<_>>();
        let requires_macro_catalog = !plan.attribute_frontiers().is_empty()
            || !plan.module_sources().is_empty()
            || !plan.discovery_imports().is_empty()
            || !items.calls().is_empty()
            || items.items().iter().any(|h| {
                matches!(
                    h.kind,
                    super::generated_items::GeneratedHeaderKind::MacroRules
                        | super::generated_items::GeneratedHeaderKind::ExternCrate
                )
            });
        let mut catalog = if requires_macro_catalog {
            match crate::frontend::generated_macros::GeneratedMacroCatalog::for_discovery(
                &items,
                call.scope.clone(),
                crate::syntax::token_wire::Key(request.key().parent_context.0),
                self.sources[ordinal].captured_macro_cfg()?,
                self.epoch,
            ) {
                Ok(catalog) => Some(catalog),
                Err(error) if error.code == "LRR-E-CATALOG-INCOMPLETE" => None,
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        if let Some(helpers) = self
            .attribute_selections
            .get(&call.item_input)
            .and_then(|selection| selection.inherited_helpers())
            && let Some(catalog) = &mut catalog
        {
            catalog.inherit_original_helpers(helpers.clone())?;
        }
        let mut empty_boundary = if !requires_macro_catalog {
            if self.completed_empty_boundaries.contains_key(&identity) || self.pending_item_boundaries.contains_key(&identity) {
                return Err(fault("completed empty macro boundary count/conflict"));
            }
            Some(super::body_macro_boundary::CompletedEmptyBoundary {
                record: record.clone(),
                source_ordinal: ordinal,
                call: call.clone(),
            })
        } else {
            None
        };
        let retained_bytes = retained_bytes
            .checked_add(if empty_boundary.is_some() {
                256usize
                    .checked_add(call.scope.heap_bytes())
                    .ok_or_else(|| fault("empty macro boundary metadata overflow"))?
            } else {
                0
            })
            .ok_or_else(|| fault("empty macro boundary metadata overflow"))?;
        let retained_bytes = retained_bytes
            .checked_add(
                catalog
                    .as_ref()
                    .map(|c| c.metadata_bytes())
                    .transpose()?
                    .unwrap_or(0),
            )
            .ok_or(Diagnostic::new(
                "LRR-E-DISCOVERY-LIMIT",
                "retained generated catalogue metadata limit".into(),
            ))?;
        let macro_ids = catalog
            .as_ref()
            .map(|c| {
                c.outward()
                    .iter()
                    .map(|e| e.definition.identity)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let call_frontiers = catalog
            .as_ref()
            .map(|c| c.call_frontiers().clone())
            .unwrap_or_default();
        let module_ids = catalog
            .as_ref()
            .map(|c| {
                c.unresolved_module_sources()
                    .map(|d| d.producer())
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let can_publish_macros = plan.blocked().all(|id| {
            imports.contains(id)
                || macro_ids.contains(id)
                || call_frontiers.contains(id)
                || module_ids.contains(id)
        });
        let deferred =
            can_publish_macros && catalog.as_ref().is_some_and(|c| !c.ready_for_outward());
        let discharged = if can_publish_macros {
            macro_ids.union(&call_frontiers).copied().collect()
        } else {
            BTreeSet::new()
        };
        let opaque_count = plan
            .blocked()
            .filter(|id| !imports.contains(id) && !discharged.contains(id))
            .count();
        let job_count = imports
            .len()
            .checked_add(usize::from(deferred))
            .ok_or_else(|| fault("generated job count overflow"))?
            .checked_add(opaque_count)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated discovery job count limit".into(),
                )
            })?;
        let mut opaque = Vec::new();
        opaque.try_reserve_exact(opaque_count).map_err(|_| {
            Diagnostic::new(
                "LRR-E-DISCOVERY-LIMIT",
                "generated frontier allocation".into(),
            )
        })?;
        opaque.extend(
            plan.blocked()
                .filter(|id| !imports.contains(id) && !discharged.contains(id))
                .copied(),
        );
        let mut jobs = Vec::new();
        jobs.try_reserve_exact(job_count).map_err(|_| {
            Diagnostic::new("LRR-E-DISCOVERY-LIMIT", "generated job allocation".into())
        })?;
        for identity in &imports {
            jobs.push(DiscoveryJob {
                module_order: ticket.job().module_order,
                source_offset: call.range.start,
                kind: DiscoveryKind::ResolveImport,
                identity: *identity,
            });
        }
        for identity in &opaque {
            jobs.push(DiscoveryJob {
                module_order: ticket.job().module_order,
                source_offset: call.range.start,
                kind: DiscoveryKind::ScanNestedDeclarations,
                identity: *identity,
            });
        }
        if deferred {
            jobs.push(DiscoveryJob {
                module_order: ticket.job().module_order,
                source_offset: call.range.start,
                kind: DiscoveryKind::ScanNestedDeclarations,
                identity,
            });
        }
        if let Some(identity) = declaration_identity {
            jobs.try_reserve(1)
                .map_err(|_| fault("item declaration job allocation"))?;
            jobs.push(DiscoveryJob {
                module_order: ticket.job().module_order,
                source_offset: call.range.start,
                kind: DiscoveryKind::ScanNestedDeclarations,
                identity,
            });
        }
        let mut members = Vec::new();
        members
            .try_reserve_exact(
                items
                    .items()
                    .len()
                    .checked_add(2)
                    .ok_or_else(|| fault("generated membership count overflow"))?,
            )
            .map_err(|_| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated member allocation".into(),
                )
            })?;
        members.extend(items.items().iter().map(|h| h.key.fingerprint()));
        members.push(record.descriptor().fingerprint());
        if !deferred {
            members.push(call.item_input);
        }
        let mut update = DiscoveryUpdate {
            jobs,
            members,
            dependencies: vec![(
                request.key().fingerprint(),
                record.descriptor().fingerprint(),
            )],
        };
        if let Some(proof) = &replacement {
            update
                .dependencies
                .try_reserve_exact(1)
                .map_err(|_| fault("replacement dependency allocation"))?;
            update
                .dependencies
                .push((proof.fingerprint(), proof.fingerprint()));
        }
        update.dependencies.extend(
            self.compiler_inputs
                .iter()
                .map(|(key, fingerprint)| (key.fingerprint(), *fingerprint)),
        );
        let pending_item_boundary = if !opaque.is_empty() {
            empty_boundary
                .take()
                .map(|boundary| {
                    item_boundaries::PendingItemBoundary::prepare(
                        boundary,
                        &self.sources[ordinal],
                        self.epoch,
                        &update.jobs,
                    )
                })
                .transpose()?
        } else {
            None
        };
        let retained_bytes = retained_bytes
            .checked_add(
                pending_item_boundary
                    .as_ref()
                    .map(item_boundaries::PendingItemBoundary::metadata_bytes)
                    .transpose()?
                    .unwrap_or(0),
            )
            .ok_or_else(|| fault("pending item boundary metadata limit"))?;
        let mut bound_impl_headers = Vec::new();
        bound_impl_headers
            .try_reserve_exact(impl_headers.len())
            .map_err(|_| fault("item impl frontier allocation"))?;
        bound_impl_headers.extend(impl_headers.iter().map(|(_, header)| *header));
        let impl_scope = call.scope.clone();
        self.work.preflight_complete(&ticket, &mut update)?;
        // The original frontier remains until header/import publication succeeds.
        let result = (|| {
            self.retain_module_sources(&plan)?;
            plan.publish(unique_graph(&mut self.graph)?)?;
            let entry = self
                .frontier_producers
                .get_mut(&(call.scope.clone(), call.frontier))
                .ok_or_else(|| fault("source frontier accounting absent"))?;
            if !entry.remove(&call.item_input) {
                return Err(fault("source frontier producer already completed"));
            }
            if entry.is_empty() {
                unique_graph(&mut self.graph)?
                    .release_generated_frontier(&call.scope, call.frontier);
            }
            if opaque.is_empty() && !deferred {
                let provider = &mut self
                    .declarative
                    .as_mut()
                    .expect("pending declarative configuration")
                    .2[ordinal];
                if let Some(catalog) = catalog.take() {
                    provider.complete_generated_bindings(
                        call.item_input,
                        std::sync::Arc::new(catalog),
                    )?;
                } else {
                    provider.complete_without_macro_bindings(call.item_input)?;
                    if let Some(boundary) = empty_boundary {
                        self.completed_empty_boundaries.insert(identity, boundary);
                    }
                }
                for id in &discharged {
                    unique_graph(&mut self.graph)?.release_generated_frontier(&call.scope, *id);
                }
                Self::install_macro_routes_with_modules(
                    unique_graph(&mut self.graph)?,
                    &self
                        .declarative
                        .as_ref()
                        .expect("pending declarative configuration")
                        .2,
                    &self.module_routes,
                    &self.nested_sources,
                    &self.nested_inert_proofs,
                    &mut self.route_install,
                )?;
            }
            self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
            self.work.complete(&ticket, update)?;
            self.producers.remove(&call.item_input);
            self.producers.extend(opaque);
            Ok::<_, Diagnostic>(())
        })();
        if let Err(error) = result {
            self.failed = Some(error.clone());
            return Err(error);
        }
        self.generated_owners
            .insert(identity, (ordinal, call.scope.clone()));
        if deferred {
            self.producers.insert(identity);
            self.fragments.insert(
                identity,
                GeneratedFragment {
                    module: None,
                    source_derive: None,
                    source_prefix: None,
                    catalog: catalog.ok_or_else(|| fault("generated fragment catalog absent"))?,
                    children: BTreeMap::new(),
                    child_order: BTreeMap::new(),
                    active_child: None,
                    jobs: Vec::new(),
                    ordinal,
                    call,
                    release: discharged,
                    members: Vec::new(),
                    dependencies: Vec::new(),
                },
            );
        }
        if let (Some(job), Some(output)) = (declaration_identity, declaration_output) {
            if self.nested_jobs.insert(job, ordinal).is_some()
                || self.item_declaration_inputs.insert(job, output).is_some()
            {
                return Err(fault("item declaration job conflict"));
            }
            self.item_root_declarations.insert(identity, job);
            if let Some(frontier) = impl_frontier {
                self.item_impl_jobs
                    .insert(frontier, (job, bound_impl_headers, impl_scope));
            }
            self.nested_bytes = nested_bytes;
        }
        if let (Some(proof), Some((parent, retired, _bytes))) = (replacement, replacement_preflight)
        {
            self.source_replacements
                .insert(proof.fingerprint(), proof.clone());
            for producer in retired {
                self.replaced_source_producers
                    .insert(producer, proof.clone());
            }
            self.nested_sources
                .insert(ordinal, std::sync::Arc::new(parent));
            self.nested_bytes = final_nested_bytes;
        }
        self.source_bases.insert(identity, source_base);
        self.records.insert(identity, record);
        if let Some(boundary) = pending_item_boundary {
            self.pending_item_boundaries.insert(identity, boundary);
        }
        self.retained_record_bytes = retained_bytes;
        self.generated_bytes = bytes;
        self.generated_headers = headers;
        self.pending = None;
        Ok(())
    }
    fn admit_generated_expansion(
        &mut self,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        let (ticket, request) = self
            .pending_generated
            .as_ref()
            .ok_or_else(|| fault("generated demand absent"))?;
        if request.is_compiler() && !self.admitting_compiler {
            return Err(fault(
                "generated compiler output requires explicit adopted input admission",
            ));
        }
        if request.procedural_request().is_some() && !self.admitting_procedural {
            return Err(Diagnostic::new(
                "LRR-E-EXPANSION-INPUT",
                "generated procedural output requires explicit adopted input admission".into(),
            ));
        }
        if record.key() != request.key() {
            return Err(Diagnostic::new(
                "LRR-E-EXPANSION-INPUT",
                "generated reply differs from exact pending demand".into(),
            ));
        }
        if request.is_derive() && !self.admitting_append {
            return self.retain_generated_derive(record);
        }
        let identity = record.key().fingerprint();
        let producer_origin = request.diagnostic_origin();
        let origin_bytes = if let Some(origin) = &producer_origin {
            if origin.invocation_identity != record.key().invocation
                || self.generated_procedural_origins.contains_key(&identity)
            {
                return Err(fault("generated procedural origin adoption differs"));
            }
            self.generated_procedural_origin_bytes
                .checked_add(128)
                .and_then(|n| n.checked_add(origin.heap_bytes()?))
                .ok_or_else(|| fault("generated procedural origin retention limit"))?
        } else {
            self.generated_procedural_origin_bytes
        };
        if self.records.contains_key(&identity) && !self.admitting_append {
            return Err(fault("generated record already admitted"));
        }
        let wire = self
            .generated_bytes
            .checked_add(if self.admitting_append {
                0
            } else {
                record.descriptor().wire_bytes
            })
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "cumulative generated wire limit".into(),
                )
            })?;
        let allocated = self
            .retained_record_bytes
            .checked_add(if self.admitting_append {
                0
            } else {
                record.estimated_bytes()?
            })
            .and_then(|n| n.checked_add(512))
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "retained generated allocation limit".into(),
                )
            })?;
        // Retain rejection of this exact owned output before a later poll can
        // offer its generated expansion again. Foreign replies were checked above.
        let items = match super::generated_items::GeneratedItems::admit(
            request.key(),
            record.clone(),
            self.epoch,
        ) {
            Ok(items) => items,
            Err(error) => {
                self.failed = Some(error.clone());
                return Err(error);
            }
        };
        let headers = self
            .generated_headers
            .checked_add(items.items().len())
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "cumulative generated header limit".into(),
                )
            })?;
        let fragment = self
            .fragments
            .get_mut(&ticket.job().identity)
            .ok_or_else(|| fault("generated fragment absent"))?;
        let parent_id = fragment.active_child;
        let parent = match parent_id {
            Some(id) => {
                &fragment
                    .children
                    .get(&id)
                    .ok_or_else(|| fault("selected generated parent absent"))?
                    .catalog
            }
            None => &fragment.catalog,
        };
        let source_base = self
            .source_bases
            .get(&parent.record_identity())
            .ok_or_else(|| fault("generated parent source base absent"))?
            .clone();
        let allocated = allocated
            .checked_add(source_base_bytes(&source_base)?)
            .ok_or_else(|| fault("retained generated source base limit"))?;
        parent.preflight_park_child(record.key())?;
        let allocated = allocated
            .checked_add(parent.pending_scope()?.heap_bytes())
            .ok_or(Diagnostic::new(
                "LRR-E-DISCOVERY-LIMIT",
                "generated owner allocation limit".into(),
            ))?;

        let position = parent.pending_position()?;
        let prefix = match parent_id {
            Some(id) => {
                let frame = fragment
                    .children
                    .get(&id)
                    .ok_or_else(|| fault("generated parent order absent"))?;
                frame
                    .order
                    .get(
                        ..frame
                            .order
                            .len()
                            .checked_sub(1)
                            .ok_or_else(|| fault("generated parent order empty"))?,
                    )
                    .ok_or_else(|| fault("generated parent order invalid"))?
            }
            None => &[],
        };
        let order_len = prefix
            .len()
            .checked_add(2)
            .ok_or_else(|| fault("generated frame order length overflow"))?;
        let allocated = allocated
            .checked_add(
                order_len
                    .checked_mul(2 * std::mem::size_of::<u32>())
                    .and_then(|n| {
                        n.checked_add(std::mem::size_of::<super::identity::ExpansionKey>() + 320)
                    })
                    .ok_or_else(|| fault("generated frame metadata overflow"))?,
            )
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "retained generated frame metadata limit".into(),
                )
            })?;
        let mut order = Vec::new();
        order
            .try_reserve_exact(order_len)
            .map_err(|_| fault("generated frame order allocation"))?;
        order.extend_from_slice(prefix);
        order.push(position);
        // Source offsets are bounded below this sentinel. Descendants therefore
        // sort before their parent, while siblings retain source order.
        order.push(u32::MAX);
        if fragment.child_order.contains_key(&order) || fragment.children.contains_key(&identity) {
            return Err(fault("generated frame order or identity duplicated"));
        }
        let mut order_index = Vec::new();
        order_index
            .try_reserve_exact(order_len)
            .map_err(|_| fault("generated frame index allocation"))?;
        order_index.extend_from_slice(&order);
        let child = parent.child_for_discovery(&items)?;
        let plan =
            crate::frontend::generated_headers::GeneratedHeaderPlan::prepare_catalog(&child)?;
        let allocated = allocated
            .checked_add(child.metadata_bytes()?)
            .ok_or(Diagnostic::new(
                "LRR-E-DISCOVERY-LIMIT",
                "retained child catalogue metadata limit".into(),
            ))?;

        let mut release = child.call_frontiers().clone();
        release.extend(child.outward().iter().map(|e| e.definition.identity));
        let imports = plan
            .discovery_imports()
            .iter()
            .map(NameNamespaces::import_identity)
            .collect::<BTreeSet<_>>();
        let modules = child
            .unresolved_module_sources()
            .map(|d| d.producer())
            .collect::<BTreeSet<_>>();
        let impl_frontier = plan.impl_frontier();
        let child_impl = if let Some(frontier) = impl_frontier {
            if self.child_impl_frontiers.contains_key(&frontier) || self.item_impl_jobs.contains_key(&frontier) {
                return Err(fault("generated child impl frontier conflict or limit"));
            }
            let ancestor_job = self
                .item_root_declarations
                .get(&ticket.job().identity)
                .copied()
                .ok_or_else(|| fault("generated child declaration ancestor absent"))?;
            Some(item_impl_children::ChildImplFrontier::prepare(
                ancestor_job,
                child.retained_items().clone(),
                parent.retained_items().clone(),
                plan.retained_impl_projection(),
                child.owner().clone(),
                child.cfg().clone(),
                self.epoch,
            )?)
        } else {
            None
        };
        let nested_bytes = self
            .nested_bytes
            .checked_add(
                child_impl
                    .as_ref()
                    .map(item_impl_children::ChildImplFrontier::metadata_bytes)
                    .transpose()?
                    .unwrap_or(0),
            )
            .ok_or_else(|| fault("generated child impl retention limit"))?;
        if plan.blocked().any(|id| {
            !release.contains(id)
                && !imports.contains(id)
                && !modules.contains(id)
                && Some(*id) != impl_frontier
        }) {
            return Err(fault(
                "generated child requires further declaration discovery",
            ));
        }
        fragment
            .members
            .try_reserve(
                items
                    .items()
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| fault("fragment member count overflow"))?,
            )
            .map_err(|_| fault("fragment member allocation"))?;
        fragment
            .dependencies
            .try_reserve(1)
            .map_err(|_| fault("fragment dependency allocation"))?;
        let job_count = imports
            .len()
            .checked_add(usize::from(child_impl.is_some()))
            .ok_or_else(|| fault("generated child job count limit"))?;
        fragment
            .jobs
            .try_reserve(job_count)
            .map_err(|_| fault("fragment import job allocation"))?;
        let mut jobs = Vec::new();
        jobs.try_reserve_exact(job_count)
            .map_err(|_| fault("generated import job allocation"))?;
        jobs.extend(imports.into_iter().map(|identity| DiscoveryJob {
            module_order: ticket.job().module_order,
            source_offset: fragment.call.range.start,
            kind: DiscoveryKind::ResolveImport,
            identity,
        }));
        if let Some(frontier) = impl_frontier {
            jobs.push(DiscoveryJob {
                module_order: ticket.job().module_order,
                source_offset: fragment.call.range.start,
                kind: DiscoveryKind::ScanNestedDeclarations,
                identity: frontier,
            });
        }
        let mut update = fragment.completion_update(
            items
                .items()
                .len()
                .checked_add(1)
                .ok_or_else(|| fault("generated member count overflow"))?,
            1,
        )?;
        update
            .jobs
            .try_reserve(jobs.len())
            .map_err(|_| fault("generated import update allocation"))?;
        update.jobs.extend_from_slice(&jobs);
        update
            .members
            .extend(items.items().iter().map(|h| h.key.fingerprint()));
        update.members.push(record.descriptor().fingerprint());
        update
            .dependencies
            .push((identity, record.descriptor().fingerprint()));
        self.work.preflight_complete(ticket, &mut update)?;
        let result = (|| {
            retain_module_demands(
                &mut self.module_sources,
                &mut self.module_source_bytes,
                &plan,
            )?;
            if let Err(error) = plan.publish(unique_graph(&mut self.graph)?) {
                self.failed = Some(error.clone());
                return Err(error);
            }
            match parent_id {
                Some(id) => fragment
                    .children
                    .get_mut(&id)
                    .ok_or_else(|| fault("generated parent parking target absent"))?
                    .catalog
                    .park_child(record.key())?,
                None => fragment.catalog.park_child(record.key())?,
            };
            fragment.child_order.insert(order_index, identity);
            let child_owner = child.owner().clone();
            fragment.children.insert(
                identity,
                GeneratedChild {
                    parent: parent_id,
                    order,
                    catalog: child,
                    release,
                },
            );
            fragment.jobs.extend(jobs);
            if let (Some(frontier), Some(waiter)) = (impl_frontier, child_impl) {
                self.child_impl_frontiers.insert(frontier, waiter);
                self.producers.insert(frontier);
            }
            self.nested_bytes = nested_bytes;
            fragment
                .members
                .extend(items.items().iter().map(|h| h.key.fingerprint()));
            fragment.members.push(record.descriptor().fingerprint());
            fragment
                .dependencies
                .push((identity, record.descriptor().fingerprint()));
            if let Some(module) = fragment.module {
                self.generated_source_modules.insert(identity, module);
            }
            self.generated_owners
                .insert(identity, (fragment.ordinal, child_owner));
            self.source_bases.insert(identity, source_base);
            self.records.insert(identity, record);
            if let Some(origin) = producer_origin {
                self.generated_procedural_origins.insert(identity, origin);
            }
            self.generated_procedural_origin_bytes = origin_bytes;
            self.generated_bytes = wire;
            self.generated_headers = headers;
            self.retained_record_bytes = allocated;
            self.pending_generated = None;
            match unique_graph(&mut self.graph)?.resolve_imports() {
                Ok(imports) => self.imports = imports,
                Err(error) => {
                    self.failed = Some(error.clone());
                    return Err(error);
                }
            }
            Ok(())
        })();
        if let Err(error) = &result {
            self.failed = Some(error.clone());
        }
        result
    }
    pub(crate) fn require_derive_append(
        &self,
        request: &super::generated_call_request::GeneratedCallRequest,
        record: &super::record::ExpansionRecord,
    ) -> Result<(), Diagnostic> {
        if self.failed.is_some()
            || self.seal.is_some()
            || self
                .pending_append
                .as_ref()
                .is_none_or(|(_, pending, old)| {
                    pending.key() != request.key()
                        || old.descriptor().fingerprint() != record.descriptor().fingerprint()
                })
            || !request.is_derive()
        {
            return Err(fault("derive append differs from active retained output"));
        }
        Ok(())
    }
    pub fn defer_derive_append(
        &mut self,
        request: &super::generated_call_request::GeneratedCallRequest,
    ) -> Result<(), Diagnostic> {
        let (ticket, pending, _) = self
            .pending_append
            .as_ref()
            .ok_or_else(|| fault("derive append deferral absent"))?;
        if pending.key() != request.key() {
            return Err(fault("deferred append request differs"));
        }
        self.work.block(ticket, vec![request.key().invocation])?;
        self.pending_append = None;
        Ok(())
    }
    pub(crate) fn integrate_derive_append(
        &mut self,
        db: &mut crate::db::memo::Database,
        request: &super::generated_call_request::GeneratedCallRequest,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        db.require_coordinator()?;
        self.require_derive_append(request, &record)?;
        use crate::db::dependency::{InputKey, InputValue};
        if !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),Some(InputValue::ExpansionEpoch(epoch)) if *epoch==self.epoch)
            || !matches!(db.input(&InputKey::ExpansionRecord(record.key().fingerprint())).as_deref(),Some(InputValue::ExpansionRecord(old)) if old.descriptor().fingerprint()==record.descriptor().fingerprint())
        {
            return Err(fault("derive append database epoch or record differs"));
        }
        let (ticket, pending, old) = self
            .pending_append
            .take()
            .ok_or_else(|| fault("derive append absent"))?;
        self.pending_generated = Some((ticket, pending));
        self.admitting_append = true;
        self.admitting_procedural = true;
        let result = self.admit_generated_expansion(old);
        self.admitting_append = false;
        self.admitting_procedural = false;
        if let Err(error) = &result {
            self.failed = Some(error.clone());
        }
        result
    }
    fn retain_generated_derive(
        &mut self,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        let (ticket, request) = self
            .pending_generated
            .as_ref()
            .ok_or_else(|| fault("derive output demand absent"))?;
        if !self.admitting_procedural || !request.is_derive() || request.key() != record.key() {
            return Err(fault("derive output requires exact adopted native request"));
        }
        let id = record.key().fingerprint();
        if self.records.contains_key(&id) {
            return Err(fault("derive output already retained or record limit"));
        }
        super::generated_items::GeneratedItems::admit(request.key(), record.clone(), self.epoch)?;
        let wire = self
            .generated_bytes
            .checked_add(record.descriptor().wire_bytes)
            .ok_or_else(|| fault("derive cumulative wire limit"))?;
        let allocated = self
            .retained_record_bytes
            .checked_add(record.estimated_bytes()?)
            .and_then(|n| n.checked_add(512))
            .ok_or_else(|| fault("derive retained allocation limit"))?;
        let fragment = self
            .fragments
            .get_mut(&ticket.job().identity)
            .ok_or_else(|| fault("derive fragment absent"))?;
        let catalog = match fragment.active_child {
            Some(id) => {
                &mut fragment
                    .children
                    .get_mut(&id)
                    .ok_or_else(|| fault("derive child absent"))?
                    .catalog
            }
            None => &mut fragment.catalog,
        };
        catalog.preflight_derive_output(&record)?;
        let fingerprint = record.descriptor().fingerprint();
        self.work.preflight_resume_input(ticket, id, fingerprint)?;
        catalog.retain_derive_output(record.clone())?;
        self.work.resume_input(ticket, id, fingerprint)?;
        self.records.insert(id, record);
        self.generated_bytes = wire;
        self.retained_record_bytes = allocated;
        self.pending_generated = None;
        Ok(())
    }
    /// An explicit pending control; never a full fixture pass or completion grant.
    pub fn defer_generated_derive(
        &mut self,
        request: &super::generated_call_request::GeneratedCallRequest,
    ) -> Result<(), Diagnostic> {
        self.require_generated_procedural_request(request)?;
        if !request.is_derive() {
            return Err(fault("deferred request is not a derive"));
        }
        let (ticket, _) = self
            .pending_generated
            .as_ref()
            .ok_or_else(|| fault("derive deferral absent"))?;
        self.work.block(ticket, vec![request.key().invocation])?;
        self.pending_generated = None;
        Ok(())
    }
    pub fn retained_derive_output_count(&self) -> Result<usize, Diagnostic> {
        self.records.values().try_fold(0usize, |n, record| {
            if record.key().macro_identity.is_derive() {
                n.checked_add(1)
                    .ok_or_else(|| fault("derive output count overflow"))
            } else {
                Ok(n)
            }
        })
    }
    fn completed_macro_routes(
        providers: &[super::source_declarative::SourceDeclarative],
        modules: &BTreeMap<
            Digest,
            Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
        >,
    ) -> Result<
        Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
        Diagnostic,
    > {
        let mut routes = Vec::new();
        routes
            .try_reserve_exact(providers.len())
            .map_err(|_| fault("macro route catalogue allocation failed"))?;
        let mut route_bytes = 0usize;
        let mut seen_catalogs = BTreeSet::new();
        for provider in providers {
            if let Some(catalog) = &provider.procedural
                && seen_catalogs.insert(catalog.fingerprint())
            {
                for route in crate::frontend::source_macro_routes::SourceMacroRoutes::from_procedural_catalog(catalog)? {
                    route_bytes = route_bytes.checked_add(route.metadata_bytes()?).ok_or_else(|| fault("prepared procedural route metadata limit"))?;
                    routes.try_reserve(1).map_err(|_| fault("prepared procedural route allocation"))?;
                    routes.push(std::sync::Arc::new(route));
                }
            }
        }
        for provider in providers {
            for route in provider.routes_in(providers)? {
                route_bytes = route_bytes
                    .checked_add(route.metadata_bytes()?)
                    .ok_or_else(|| fault("macro route catalogue metadata budget exceeded"))?;
                routes
                    .try_reserve(1)
                    .map_err(|_| fault("macro route catalogue allocation failed"))?;
                routes.push(route);
            }
        }
        for module in modules.values() {
            for route in module {
                route_bytes = route_bytes
                    .checked_add(route.metadata_bytes()?)
                    .ok_or_else(|| fault("module macro route metadata limit"))?;
                routes
                    .try_reserve(1)
                    .map_err(|_| fault("module macro route allocation"))?;
                routes.push(route.clone());
            }
        }
        let mut compiler_catalogues = BTreeSet::new();
        for provider in providers {
            if provider.macro_export_source_role().is_some() {
                continue;
            }
            let Some(catalog) = &provider.compiler else {
                continue;
            };
            if !compiler_catalogues.insert(catalog.owning_profile_identity()?) {
                continue;
            }
            let core = catalog
                .definitions()
                .next()
                .ok_or_else(|| fault("compiler facet is empty"))?
                .defining_unit;
            let mut exports = vec![catalog.source_routes()?];
            for (_, unit) in catalog.facade_aliases() {
                if unit != core {
                    exports.push(catalog.source_exports(unit)?);
                }
            }
            for export in exports {
                route_bytes = route_bytes
                    .checked_add(export.metadata_bytes()?)
                    .ok_or_else(|| fault("owning compiler route metadata limit"))?;
                routes
                    .try_reserve_exact(1)
                    .map_err(|_| fault("owning compiler route allocation failed"))?;
                routes.push(std::sync::Arc::new(export));
            }
            if let Some(library) = catalog.library() {
                routes.extend(library.routes().iter().cloned());
            }
        }
        Ok(routes)
    }
    fn compiler_import_graph(
        routes: &[std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>],
        providers: &[super::source_declarative::SourceDeclarative],
    ) -> Result<crate::frontend::macro_names::MacroNamespaces, Diagnostic> {
        Self::prepare_compiler_import_graph(routes, providers)?.require_ready()
    }
    fn prepare_compiler_import_graph(
        routes: &[std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>],
        providers: &[super::source_declarative::SourceDeclarative],
    ) -> Result<crate::frontend::source_macro_routes::ModuleGraphPreparation, Diagnostic> {
        let mut imports = Vec::new();
        for provider in providers {
            if provider.macro_export_source_role().is_some() {
                continue;
            }
            if let Some(catalog) = &provider.compiler
                && let Some(import) = catalog.injected_import(provider.module_source())?
            {
                imports
                    .try_reserve(1)
                    .map_err(|_| fault("compiler injected import allocation"))?;
                imports.push(import);
            }
        }
        crate::frontend::source_macro_routes::SourceMacroRoutes::prepare_discovery_graph_with_compiler_imports(routes, &imports)
    }
    fn runtime_preludes<'a>(
        namespaces: &mut crate::frontend::macro_names::MacroNamespaces,
        providers: impl Iterator<Item = &'a super::source_declarative::SourceDeclarative>,
    ) -> Result<(), Diagnostic> {
        let mut installed: BTreeMap<ScopeKey, Digest> = BTreeMap::new();
        // Compiler globals are keyed by consumer crate only, so one owning
        // compiler facet installs them once per crate, not once per scope.
        let mut global_crates: BTreeSet<(Digest, Digest)> = BTreeSet::new();
        let mut globals_by_identity: BTreeMap<Digest, Vec<_>> = BTreeMap::new();
        let mut scope_bytes = 0usize;
        for provider in providers {
            let Some(catalog) = provider.compiler.as_ref() else {
                continue;
            };
            let runtime_unit = if provider.macro_export_source_role().is_some() {
                None
            } else {
                catalog.install_injected_import(namespaces, provider.module_source())?;
                Some(catalog.runtime_prelude_unit(
                    provider.module_source().macro_prelude_policy()?.standard,
                ))
            };
            let identity = catalog.owning_profile_identity()?;
            if !globals_by_identity.contains_key(&identity) {
                globals_by_identity.insert(
                    identity,
                    catalog.compiler_globals()?.into_iter().collect(),
                );
            }
            let globals = &globals_by_identity[&identity];
            for scope in provider
                .compiler_scopes()
                .filter(|scope| scope.owner.is_none())
            {
                if let Some(old) = installed.get(scope) {
                    if *old != identity {
                        return Err(fault(
                            "one source scope has conflicting owning compiler facets",
                        ));
                    }
                    continue;
                }
                scope_bytes = scope_bytes
                    .checked_add(scope.heap_bytes())
                    .and_then(|n| n.checked_add(128))
                    .ok_or_else(|| fault("compiler prelude scope metadata limit"))?;
                installed.insert(scope.clone(), identity);
                if global_crates.insert((scope.crate_key, identity)) {
                    for (name, definition) in globals {
                        namespaces.compiler_global(
                            scope.crate_key,
                            name.clone(),
                            definition.clone(),
                        )?;
                    }
                }
                if let Some(unit) = runtime_unit {
                    namespaces.set_prelude(scope.clone(), unit)?;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn contains_original_source(
        &self,
        selection: &crate::source::changes::SourceSelection,
    ) -> bool {
        self.sources
            .iter()
            .any(|source| source.selection() == selection)
    }
    pub(crate) fn completed_runtime_macro_namespace(
        &self,
    ) -> Result<crate::frontend::macro_names::MacroNamespaces, Diagnostic> {
        let _ = self.sealed_expansions()?;
        let providers = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("runtime declarative providers absent"))?
            .2;
        if providers
            .iter()
            .any(|provider| provider.macro_export_source_role().is_some())
        {
            return Err(fault("macro-only source cannot supply a runtime namespace"));
        }
        let routes = Self::completed_macro_routes(providers, &self.module_routes)?;
        // Without staged module providers this is exactly the namespace of the
        // last route installation when its inputs are unchanged.
        if self.staged_modules.is_empty()
            && let Some(namespaces) = self.route_install.hit(
                providers,
                &routes,
                &self.nested_sources,
                &self.nested_inert_proofs,
            )
        {
            return Ok((*namespaces).clone());
        }
        let mut namespaces = Self::compiler_import_graph(&routes, providers)?;

        Self::install_original_module_bindings(
            &mut namespaces,
            self.nested_sources.values().map(|index| index.scan()),
            &self.nested_inert_proofs,
        )?;
        Self::runtime_preludes(
            &mut namespaces,
            providers
                .iter()
                .chain(self.staged_modules.values().map(|module| &module.provider)),
        )?;
        Ok(namespaces)
    }
    pub(crate) fn completed_runtime_macro_indexes(
        &self,
    ) -> Result<
        Vec<(
            PreparedSourceView,
            crate::frontend::source_macros::PublishedMacroIndex,
        )>,
        Diagnostic,
    > {
        let _ = self.sealed_expansions()?;
        let providers = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("runtime declarative providers absent"))?
            .2;
        if providers
            .iter()
            .any(|provider| provider.macro_export_source_role().is_some())
        {
            return Err(fault("macro-only source cannot supply runtime indexes"));
        }
        let mut count = 0usize;
        for source in providers
            .iter()
            .map(|provider| provider.module_source())
            .chain(
                self.staged_modules
                    .values()
                    .map(|module| module.syntax.prepared_source()),
            )
        {
            if source.has_runtime_macro_syntax()? || source.has_lexical_callable_syntax()? {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| fault("runtime macro source count overflow"))?;
            }
        }
        let count = Some(count)
            .ok_or_else(|| fault("runtime macro source count limit"))?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| fault("runtime macro source allocation"))?;
        let mut remaining = usize::MAX;
        for provider in providers {
            if !(provider.module_source().has_runtime_macro_syntax()?
                || provider.module_source().has_lexical_callable_syntax()?)
            {
                continue;
            }
            let index = provider.completed_runtime_index(providers, None, remaining)?;
            remaining = remaining
                .checked_sub(index.runtime_bytes())
                .ok_or_else(|| fault("runtime macro aggregate metadata limit"))?;
            result.push((provider.module_source().share(), index));
        }
        for staged in self.staged_modules.values() {
            if !staged.syntax.prepared_source().has_runtime_macro_syntax()? {
                continue;
            }
            if !staged.namespace_complete {
                return Err(fault("runtime module namespace incomplete"));
            }
            let inherited = self.module_projection_inherited(&staged.syntax)?;
            let index =
                staged
                    .provider
                    .completed_runtime_index(providers, Some(&inherited), remaining)?;
            remaining = remaining
                .checked_sub(index.runtime_bytes())
                .ok_or_else(|| fault("runtime macro aggregate metadata limit"))?;
            result.push((staged.syntax.prepared_source().share(), index));
        }
        Ok(result)
    }
    /// Provenance facts accompany canonical routes; they never publish a route.
    /// Delegation removes scheduling work, but does not certify an annotation.
    fn install_original_module_bindings<'a>(
        namespaces: &mut crate::frontend::macro_names::MacroNamespaces,
        scans: impl Iterator<Item = &'a crate::syntax::declaration_scan::DeclarationScan>,
        inert: &BTreeMap<Digest, super::nested_inert::NestedInertProof>,
    ) -> Result<(), Diagnostic> {
        use crate::frontend::macro_names::{DeclarationModuleBinding, DeclarationModuleOrigin};
        for scan in scans {
            for (index, row) in scan.modules().iter().enumerate() {
                let mut binding = DeclarationModuleBinding::from_scan(
                    scan,
                    index,
                    DeclarationModuleOrigin::Original,
                )?;
                binding.pending = None;
                for input in &row.attribute_inputs {
                    let Some(proof) = inert.get(input) else {
                        binding.pending = Some(*input);
                        break;
                    };
                    if proof.producer() != *input
                        || proof.scan().fingerprint() != scan.fingerprint()
                    {
                        return Err(fault("module annotation proof attachment differs"));
                    }
                }
                namespaces.declare_scoped_module_binding(binding)?;
            }
        }
        Ok(())
    }
    fn install_macro_routes_with_modules(
        graph: &mut NameNamespaces,
        providers: &[super::source_declarative::SourceDeclarative],
        modules: &BTreeMap<
            Digest,
            Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
        >,
        sources: &BTreeMap<
            usize,
            std::sync::Arc<crate::frontend::impl_index::DeclarationMacroIndex>,
        >,
        inert: &BTreeMap<Digest, super::nested_inert::NestedInertProof>,
        memo: &mut RouteInstallMemo,
    ) -> Result<std::sync::Arc<crate::frontend::macro_names::MacroNamespaces>, Diagnostic> {
        let _span =
            crate::core::timing::Span::enter(|| "discovery install_macro_routes".into());
        let routes = Self::completed_macro_routes(providers, modules)?;
        if let Some(namespaces) = memo.hit(providers, &routes, sources, inert) {
            return Ok(namespaces);
        }
        let namespaces = Self::compiler_import_graph(&routes, providers)?;
        let namespaces = std::sync::Arc::new(Self::install_prepared_macro_routes(
            graph, providers, sources, inert, &routes, namespaces,
        )?);
        *memo = RouteInstallMemo {
            providers: providers.iter().map(RouteInstallProvider::of).collect(),
            sources: RouteInstallMemo::source_rows(sources),
            inert: RouteInstallMemo::inert_rows(inert),
            routes,
            namespaces: Some(namespaces.clone()),
        };
        Ok(namespaces)
    }
    fn install_prepared_macro_routes(
        graph: &mut NameNamespaces,
        providers: &[super::source_declarative::SourceDeclarative],
        sources: &BTreeMap<
            usize,
            std::sync::Arc<crate::frontend::impl_index::DeclarationMacroIndex>,
        >,
        inert: &BTreeMap<Digest, super::nested_inert::NestedInertProof>,
        routes: &[std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>],
        mut namespaces: crate::frontend::macro_names::MacroNamespaces,
    ) -> Result<crate::frontend::macro_names::MacroNamespaces, Diagnostic> {
        Self::install_original_module_bindings(
            &mut namespaces,
            sources.values().map(|index| index.scan()),
            inert,
        )?;
        Self::runtime_preludes(&mut namespaces, providers.iter())?;
        let mut facets = Vec::new();
        for routes in routes {
            let index = routes.import_route_index();
            for import in &routes.source_imports {
                if let Some(definition) = routes.import_definition_in(import, &namespaces, &index)? {
                    facets.push((import, definition));
                }
            }
        }
        // The graph facts below are a pure function of this key; a graph that
        // already received them (installation is idempotent) is unchanged.
        let key = Self::route_graph_key(providers, routes, &facets)?;
        if graph.route_install_key() != Some(key) {
            Self::install_route_graph_facts(graph, providers, routes, &facets)?;
            graph.set_route_install_key(key);
        }
        Ok(namespaces)
    }
    fn install_route_graph_facts(
        graph: &mut NameNamespaces,
        providers: &[super::source_declarative::SourceDeclarative],
        routes: &[std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>],
        facets: &[(
            &crate::syntax::imports::SourceImport,
            crate::expand::scopes::MacroDefinition,
        )],
    ) -> Result<(), Diagnostic> {
        for provider in providers {
            if let Some(catalog) = &provider.procedural {
                for export in catalog.exports() {
                    self::NameNamespaces::declare_module(
                        graph,
                        export.definition().defining_unit,
                        &[],
                        export.definition().definition_edition,
                        crate::syntax::visibility::VisibilityScope::Public,
                    )?;
                }
                for (alias, unit) in catalog.aliases() {
                    graph.dependency(
                        catalog.consumer(),
                        &format!("r#{}", alias.as_str()),
                        *unit,
                        provider.module_source().outline().profile.edition,
                    )?;
                }
            }
        }
        for provider in providers {
            if provider.macro_export_source_role().is_some() {
                continue;
            }
            if let Some(catalog) = &provider.compiler {
                catalog.install_ordinary_module_facets(graph)?;
            }
        }
        if providers.first().is_some_and(|p| p.compiler.is_some()) {
            for route in routes {
                for (unit, names) in &route.modules {
                    if names.is_empty() {
                        graph.declare_module(
                            *unit,
                            &[],
                            route.edition,
                            crate::syntax::visibility::VisibilityScope::Public,
                        )?;
                    }
                }
                if let Some((consumer, _)) =
                    route.modules.iter().find(|(_, names)| names.is_empty())
                {
                    for (alias, provider) in &route.dependencies {
                        graph.compiler_dependency(*consumer, alias.clone(), *provider)?;
                    }
                }
                for scope in &route.implicit_disabled {
                    graph.disable_compiler_prelude(scope.clone())?;
                }
            }
        }

        for provider in providers {
            if provider.macro_export_source_role().is_some() {
                continue;
            }
            if let Some(catalog) = &provider.compiler {
                graph.install_compiler_injection(catalog, provider.module_source())?;
                graph.install_standard_prelude(catalog, provider.module_source())?;
            }
        }
        // Closed library facade functions named by real library macro output.
        if let Some(library) = providers
            .iter()
            .filter(|provider| provider.macro_export_source_role().is_none())
            .find_map(|provider| provider.compiler.as_ref()?.library())
        {
            crate::facade::functions::install(graph, library)?;
        }
        for route in routes {
            for row in &route.extern_aliases {
                graph.extern_alias(row, route.edition)?;
            }
        }
        for route in routes.iter().flat_map(|r| r.definitions()) {
            let definition = crate::frontend::names::NameDefinition {
                namespace: Namespace::Macro,
                identity: route.definition.identity,
                defining_unit: route.definition.defining_unit,
                definition_edition: route.definition.definition_edition,
            };
            let scope = ScopeKey {
                crate_key: route.path.unit,
                module_path: route.path.components()[..route.path.components().len() - 1]
                    .iter()
                    .map(|n| n.as_str().to_owned())
                    .collect(),
                owner: None,
            };
            let access = crate::syntax::visibility::VisibilityScope::for_declaration(
                &route.visibility,
                &scope,
                route.definition.definition_edition,
            )?;

            graph.define(route.path.clone(), definition.clone(), access)?;
        }
        for (import, definition) in facets {
            graph.macro_import_facet(
                import,
                crate::frontend::names::NameDefinition {
                    namespace: Namespace::Macro,
                    identity: definition.identity,
                    defining_unit: definition.defining_unit,
                    definition_edition: definition.definition_edition,
                },
            )?;
        }
        Ok(())
    }
    fn route_graph_key(
        providers: &[super::source_declarative::SourceDeclarative],
        routes: &[std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>],
        facets: &[(
            &crate::syntax::imports::SourceImport,
            crate::expand::scopes::MacroDefinition,
        )],
    ) -> Result<Digest, Diagnostic> {
        use crate::core::digest::StableEncoder;
        fn definition(e: &mut StableEncoder, d: &crate::expand::scopes::MacroDefinition) {
            e.digest(d.identity);
            e.digest(d.defining_unit);
            e.u32(u32::from(d.definition_edition));
        }
        let mut e = StableEncoder::new("lrr/v1/route-graph-install");
        e.sequence(providers, |e, provider| {
            e.digest(provider.module_source().selection().location_identity());
            e.digest(provider.module_source().selection().outline.fingerprint());
            e.u8(u8::from(provider.macro_export_source_role().is_some()));
            e.option(&provider.procedural, |e, catalog| e.digest(catalog.fingerprint()));
            e.option(&provider.compiler, |e, catalog| {
                e.digest(catalog.owning_profile_identity().unwrap_or_default());
                e.u64(std::sync::Arc::as_ptr(catalog) as usize as u64);
            });
        });
        e.u8(u8::from(providers.first().is_some_and(|p| p.compiler.is_some())));
        e.sequence(routes, |e, route| {
            e.u32(u32::from(route.edition));
            e.sequence(&route.modules.iter().collect::<Vec<_>>(), |e, (unit, names)| {
                e.digest(*unit);
                e.sequence(names, |e, n| e.string(n.as_str()));
            });
            e.sequence(&route.dependencies.iter().collect::<Vec<_>>(), |e, (alias, unit)| {
                e.string(alias.as_str());
                e.digest(**unit);
            });
            e.sequence(&route.implicit_disabled.iter().collect::<Vec<_>>(), |e, scope| {
                e.record(*scope)
            });
            e.sequence(&route.extern_aliases, |e, row| e.digest(row.fingerprint()));
            e.sequence(route.definitions(), |e, row| {
                e.digest(row.path.unit);
                e.sequence(row.path.components(), |e, n| e.string(n.as_str()));
                definition(e, &row.definition);
                e.record(&row.visibility);
            });
        });
        e.sequence(facets, |e, (import, d)| {
            e.digest(NameNamespaces::import_identity(import));
            definition(e, d);
        });
        Ok(e.finish())
    }
    fn poll_generated_fragment(
        &mut self,
        identity: Digest,
    ) -> Result<crate::frontend::generated_macros::GeneratedCallDemand, Diagnostic> {
        let mut fragment = self
            .fragments
            .remove(&identity)
            .ok_or_else(|| fault("generated fragment absent"))?;
        let result = (|| {
            let providers = &self
                .declarative
                .as_ref()
                .ok_or_else(|| fault("generated enclosing provider absent"))?
                .2;
            let inherited = if let Some(module) = fragment.module {
                let staged = self
                    .staged_modules
                    .get(&module)
                    .ok_or_else(|| fault("generated module source absent"))?;
                Some(Self::projection_inherited_in(
                    &self.fragments,
                    providers,
                    &self.staged_modules,
                    self.exports.as_deref(),
                    &staged.syntax,
                    0,
                )?)
            } else {
                None
            };
            let source_prefix = fragment.source_prefix.clone();
            let provider = if let Some(prefix) = &source_prefix {
                prefix.as_ref()
            } else {
                match fragment.module {
                    Some(module) => {
                        &self
                            .staged_modules
                            .get(&module)
                            .ok_or_else(|| fault("generated child provider absent"))?
                            .provider
                    }
                    None => &providers[fragment.ordinal],
                }
            };
            let before = u64::from(fragment.call.range.start)
                .checked_add(u64::from(fragment.source_derive.is_some()))
                .ok_or_else(|| fault("source derive prefix position overflow"))?;
            let context = super::source_declarative::CapturedMacroContext {
                exports: self.exports.as_deref(),
                provider,
                owners: providers,
                before,
                inherited: inherited.as_ref(),
            };
            Self::poll_fragment_in(
                &mut fragment,
                unique_graph(&mut self.graph)?,
                &mut self.retained_record_bytes,
                context,
            )
        })();
        self.fragments.insert(identity, fragment);
        result
    }
    fn poll_fragment_in(
        fragment: &mut GeneratedFragment,
        graph: &mut NameNamespaces,
        retained_record_bytes: &mut usize,
        context: super::source_declarative::CapturedMacroContext<'_>,
    ) -> Result<crate::frontend::generated_macros::GeneratedCallDemand, Diagnostic> {
        use crate::frontend::generated_macros::GeneratedCallDemand;
        loop {
            if fragment.catalog.has_inflight_attributes()
                || fragment
                    .children
                    .values()
                    .any(|child| child.catalog.has_inflight_attributes())
            {
                let count = fragment
                    .children
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| fault("inflight attribute route count limit"))?;
                let mut routes = Vec::new();
                routes
                    .try_reserve_exact(count)
                    .map_err(|_| fault("inflight attribute route allocation"))?;
                routes.push(std::sync::Arc::new(
                    fragment.catalog.candidate_routes(context, &[])?,
                ));
                let mut bytes = routes[0].metadata_bytes()?;
                for child in fragment.children.values() {
                    let mut ancestors = Vec::new();
                    ancestors
                        .try_reserve_exact(256)
                        .map_err(|_| fault("inflight attribute route ancestor allocation"))?;
                    let mut parent = child.parent;
                    while let Some(identity) = parent {
                        let ancestor = fragment
                            .children
                            .get(&identity)
                            .ok_or_else(|| fault("inflight attribute ancestor absent"))?;
                        ancestors.push(&ancestor.catalog);
                        parent = ancestor.parent;
                    }
                    ancestors.push(&fragment.catalog);
                    let route = child.catalog.candidate_routes(context, &ancestors)?;
                    bytes = bytes
                        .checked_add(route.metadata_bytes()?)
                        .ok_or_else(|| fault("inflight attribute route metadata limit"))?;
                    routes.push(std::sync::Arc::new(route));
                }
                for child in fragment.children.values() {
                    let mut ancestors = Vec::new();
                    ancestors
                        .try_reserve_exact(256)
                        .map_err(|_| fault("inflight attribute ancestor allocation"))?;
                    let mut parent = child.parent;
                    while let Some(identity) = parent {
                        let ancestor = fragment
                            .children
                            .get(&identity)
                            .ok_or_else(|| fault("inflight attribute check ancestor absent"))?;
                        ancestors.push(&ancestor.catalog);
                        parent = ancestor.parent;
                    }
                    ancestors.push(&fragment.catalog);
                    child
                        .catalog
                        .check_inflight_attributes(context, &ancestors, &routes)?;
                }
                fragment
                    .catalog
                    .check_inflight_attributes(context, &[], &routes)?;
            }
            let mut frames = Vec::new();
            frames
                .try_reserve_exact(fragment.child_order.len())
                .map_err(|_| fault("generated ready frame allocation"))?;
            frames.extend(fragment.child_order.values().copied());
            let mut blocked = None;
            let mut progressed = false;
            for id in frames {
                let mut child = fragment
                    .children
                    .remove(&id)
                    .ok_or_else(|| fault("generated scheduled frame absent"))?;
                let mut ancestors = Vec::new();
                ancestors
                    .try_reserve_exact(child.order.len())
                    .map_err(|_| fault("generated ancestor allocation"))?;
                let mut parent = child.parent;
                while let Some(id) = parent {
                    let frame = fragment
                        .children
                        .get(&id)
                        .ok_or_else(|| fault("generated ancestor frame absent"))?;
                    ancestors.push(&frame.catalog);
                    parent = frame.parent;
                }
                ancestors.push(&fragment.catalog);
                let demand = child
                    .catalog
                    .next_call_with_ancestors(context, &ancestors)?;
                crate::frontend::generated_headers::GeneratedHeaderPlan::settled_originals(
                    &child.catalog,
                )?
                .publish(graph)?;
                match demand {
                    GeneratedCallDemand::NeedDeriveExecution { .. } => {
                        return Err(fault("uncached generated helper scope escaped catalog"));
                    }
                    GeneratedCallDemand::Append { request, record } => {
                        fragment.children.insert(id, child);
                        fragment.active_child = Some(id);
                        return Ok(GeneratedCallDemand::Append { request, record });
                    }
                    GeneratedCallDemand::NeedDeclaration(request) => {
                        fragment.children.insert(id, child);
                        fragment.active_child = Some(id);
                        return Ok(GeneratedCallDemand::NeedDeclaration(request));
                    }
                    GeneratedCallDemand::Ready(request) => {
                        fragment.children.insert(id, child);
                        fragment.active_child = Some(id);
                        return Ok(GeneratedCallDemand::Ready(request));
                    }
                    GeneratedCallDemand::NeedInput(input) => {
                        blocked.get_or_insert(input);
                        fragment.children.insert(id, child);
                    }
                    GeneratedCallDemand::Complete => {
                        let snapshot = child.catalog.child_route_snapshot(context, &ancestors)?;
                        let retained = (*retained_record_bytes)
                            .checked_add(snapshot.metadata_bytes()?)
                            .and_then(|n| n.checked_add(128))
                            .ok_or_else(|| {
                                Diagnostic::new(
                                    "LRR-E-DISCOVERY-LIMIT",
                                    "retained child route snapshot metadata limit".into(),
                                )
                            })?;
                        drop(ancestors);
                        let parent = match child.parent {
                            Some(parent) => {
                                &mut fragment
                                    .children
                                    .get_mut(&parent)
                                    .ok_or_else(|| fault("generated completion parent absent"))?
                                    .catalog
                            }
                            None => &mut fragment.catalog,
                        };
                        let remaining = usize::MAX
                            .checked_sub(retained)
                            .ok_or_else(|| fault("retained child metadata overflow"))?;
                        let retained = retained
                            .checked_add(parent.reserve_child_routes(&child.catalog, remaining)?)
                            .ok_or_else(|| {
                                Diagnostic::new(
                                    "LRR-E-DISCOVERY-LIMIT",
                                    "retained child route index metadata limit".into(),
                                )
                            })?;
                        let release_scope = child.catalog.owner().clone();
                        fragment.child_order.remove(&child.order);
                        parent.complete_child(child.catalog, snapshot)?;
                        crate::frontend::generated_headers::GeneratedHeaderPlan::settled_originals(
                            parent,
                        )?
                        .publish(graph)?;
                        for id in parent.settled_derive_items() {
                            graph.release_generated_frontier(parent.owner(), *id);
                        }

                        *retained_record_bytes = retained;
                        for frontier in child.release {
                            graph.release_generated_frontier(&release_scope, frontier);
                        }
                        progressed = true;
                        break;
                    }
                }
            }
            if progressed {
                continue;
            }
            let demand = fragment.catalog.next_call_with_source(context)?;
            crate::frontend::generated_headers::GeneratedHeaderPlan::settled_originals(
                &fragment.catalog,
            )?
            .publish(graph)?;
            match demand {
                GeneratedCallDemand::NeedDeriveExecution { .. } => {
                    return Err(fault("uncached generated helper scope escaped catalog"));
                }
                GeneratedCallDemand::Append { request, record } => {
                    fragment.active_child = None;
                    return Ok(GeneratedCallDemand::Append { request, record });
                }
                GeneratedCallDemand::NeedDeclaration(request) => {
                    fragment.active_child = None;
                    return Ok(GeneratedCallDemand::NeedDeclaration(request));
                }
                GeneratedCallDemand::Ready(request) => {
                    fragment.active_child = None;
                    return Ok(GeneratedCallDemand::Ready(request));
                }
                GeneratedCallDemand::NeedInput(input) => {
                    return Ok(GeneratedCallDemand::NeedInput(blocked.unwrap_or(input)));
                }
                GeneratedCallDemand::Complete if fragment.children.is_empty() => {
                    return Ok(GeneratedCallDemand::Complete);
                }
                GeneratedCallDemand::Complete => {
                    return Err(fault("generated parent completed with parked child frames"));
                }
            }
        }
    }
    fn finish_fragment(&mut self, ticket: &JobTicket) -> Result<(), Diagnostic> {
        let id = ticket.job().identity;
        let fragment = self
            .fragments
            .get_mut(&id)
            .ok_or_else(|| fault("completed fragment absent"))?;
        let mut update = fragment.completion_update(0, self.compiler_inputs.len())?;
        update.dependencies.extend(
            self.compiler_inputs
                .iter()
                .map(|(key, fingerprint)| (key.fingerprint(), *fingerprint)),
        );
        self.work.preflight_complete(ticket, &mut update)?;
        let fragment = self
            .fragments
            .remove(&id)
            .ok_or_else(|| fault("completed fragment absent"))?;
        let result = (|| {
            let provider = if let Some(module) = fragment.module {
                &mut self
                    .staged_modules
                    .get_mut(&module)
                    .ok_or_else(|| fault("completed module provider absent"))?
                    .provider
            } else {
                &mut self
                    .declarative
                    .as_mut()
                    .ok_or_else(|| fault("declarative providers absent"))?
                    .2[fragment.ordinal]
            };
            if let Some(selection) = fragment.source_derive {
                if self.source_derives_inflight.get(&fragment.call.item_input) != Some(&id)
                    || self
                        .completed_source_derives
                        .contains_key(&selection.input().identity())
                {
                    return Err(fault("source derive append completion differs"));
                }
                provider.publish_derive_bindings(
                    &fragment.call,
                    std::sync::Arc::new(fragment.catalog),
                    fragment
                        .source_prefix
                        .ok_or_else(|| fault("source derive prefix snapshot absent"))?,
                )?;
                self.completed_source_derives
                    .insert(selection.input().identity(), selection);
                self.source_derives_inflight
                    .remove(&fragment.call.item_input);
            } else {
                provider.complete_generated_bindings(
                    fragment.call.item_input,
                    std::sync::Arc::new(fragment.catalog),
                )?;
            }
            if let Some(module) = fragment.module {
                let staged = self
                    .staged_modules
                    .get_mut(&module)
                    .ok_or_else(|| fault("completed module source absent"))?;
                staged
                    .completed_invocations
                    .insert(fragment.call.item_input);
                let pending = staged
                    .syntax
                    .prepared_source()
                    .outline()
                    .item_invocations
                    .iter()
                    .any(|call| {
                        call.scope == fragment.call.scope
                            && call.frontier == fragment.call.frontier
                            && !staged.completed_invocations.contains(&call.item_input)
                    });
                if !pending {
                    unique_graph(&mut self.graph)?
                        .release_generated_frontier(&fragment.call.scope, fragment.call.frontier);
                }
                self.producers.remove(&fragment.call.item_input);
                self.refresh_staged_module_routes(module)?;
            }
            for frontier in fragment.release {
                unique_graph(&mut self.graph)?
                    .release_generated_frontier(&fragment.call.scope, frontier);
            }
            Self::install_macro_routes_with_modules(
                unique_graph(&mut self.graph)?,
                &self
                    .declarative
                    .as_ref()
                    .ok_or_else(|| fault("declarative providers absent"))?
                    .2,
                &self.module_routes,
                &self.nested_sources,
                &self.nested_inert_proofs,
                &mut self.route_install,
            )?;
            self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
            self.work.complete(ticket, update)?;
            self.producers.remove(&id);
            Ok::<_, Diagnostic>(())
        })();
        if let Err(error) = &result {
            self.failed = Some(error.clone());
        }
        result
    }
    pub(crate) fn into_runtime_parts(
        self,
    ) -> Result<(Vec<PreparedSourceView>, std::sync::Arc<NameNamespaces>), Diagnostic> {
        let _record_count = self.sealed_expansions()?.len();
        let environment = self
            .completed_source_environment
            .as_ref()
            .ok_or_else(|| fault("completed runtime source environment absent"))?;
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(environment.sources().len())
            .map_err(|_| fault("module runtime source allocation"))?;
        sources.extend(environment.sources().iter().map(PreparedSourceView::share));
        Ok((sources, std::sync::Arc::clone(environment.names_root())))
    }
    fn attach_incomplete_source_origin(
        &self,
        mut diagnostic: Diagnostic,
        blocked: &[(Digest, Vec<Digest>)],
    ) -> Result<Diagnostic, Diagnostic> {
        use crate::syntax::{
            declaration_scan::DeclarationProducerKind,
            imports::ImportSegment,
            origin::{MacroOrigin, SourceOrigin},
            token_wire::{Key, SourceIdentity},
        };
        // The complete typed blocked ledger is returned unchanged. A label is
        // selected by source ordinal/range from an actual original-source producer,
        // rather than digest order (which varies with captured source identity);
        // no missing definition or seal is inferred from a stalled fixed point.
        let mut first = None;
        for (identity, _) in blocked {
            let source_call = if let Some((ordinal, call)) = self.calls.get(identity) {
                (call.attribute_input.is_none()).then_some((*ordinal, call.range))
            } else if let Some(ordinal) = self.nested_jobs.get(identity) {
                self.nested_sources.get(ordinal).and_then(|index| {
                    let selected = self.sources.get(*ordinal)?.selection();
                    let scan = index.scan();
                    if scan.generated_origin().is_some()
                        || scan.parse() != &selected.outline.parse
                        || scan.path() != &selected.path
                    {
                        return None;
                    }
                    scan.producers()
                        .iter()
                        .find(|producer| {
                            producer.identity == *identity
                                && producer.kind == DeclarationProducerKind::Macro
                        })
                        .map(|producer| (*ordinal, producer.range))
                })
            } else {
                None
            };
            if let Some((ordinal, range)) = source_call {
                let key = (ordinal, range.start, range.end, *identity);
                if first.as_ref().is_none_or(|(previous, _)| key < *previous) {
                    first = Some((key, range));
                }
            }
        }
        if let Some(((ordinal, _, _, identity), range)) = first {
            let source = self
                .sources
                .get(ordinal)
                .ok_or_else(|| fault("blocked source owner is absent"))?;
            let call = source.macro_call_at(range)?;
            let Some(ImportSegment::Name(name)) = call.segments().last() else {
                return Ok(diagnostic);
            };
            let required = diagnostic
                .message
                .len()
                .checked_add(name.len())
                .and_then(|n| n.checked_add(32))
                .ok_or_else(|| fault("blocked macro diagnostic size limit"))?;
            diagnostic
                .message
                .try_reserve(required - diagnostic.message.len())
                .map_err(|_| fault("blocked macro diagnostic allocation"))?;
            use std::fmt::Write;
            write!(&mut diagnostic.message, "; blocked macro `{name}`")
                .map_err(|_| fault("blocked macro diagnostic formatting"))?;
            let selected = source.selection();
            diagnostic.macro_origin = Some(Box::new(MacroOrigin {
                invocation_identity: identity,
                invocation: Some(SourceOrigin {
                    source: SourceIdentity {
                        unit: Key(selected.outline.crate_key.0),
                        content: Key(selected.outline.parse.content.0),
                        path: selected.path.as_str().into(),
                    },
                    range: call.range(),
                }),
                definition: None,
                failure_kind: None,
                failure_span_key: None,
                failure_span: None,
                parents: Vec::new(),
            }));
            if let Some(fragment) = self.nested_fragments.get(&identity)
                && let Some((_, inputs)) = blocked.iter().find(|(job, _)| *job == identity)
            {
                for input in inputs {
                    let Some(site) = fragment.blocked_macro_site(*input)? else {
                        continue;
                    };
                    let required = diagnostic
                        .message
                        .len()
                        .checked_add(site.name.len())
                        .and_then(|n| n.checked_add(96))
                        .ok_or_else(|| fault("blocked generated diagnostic size limit"))?;
                    diagnostic
                        .message
                        .try_reserve(required - diagnostic.message.len())
                        .map_err(|_| fault("blocked generated diagnostic allocation"))?;
                    write!(
                        &mut diagnostic.message,
                        "; generated macro `{}` awaits declaration inputs",
                        site.name
                    )
                    .map_err(|_| fault("blocked generated diagnostic formatting"))?;
                    let origin = diagnostic
                        .macro_origin
                        .as_mut()
                        .ok_or_else(|| fault("blocked generated parent origin absent"))?;
                    origin.failure_kind =
                        Some(crate::syntax::origin::MacroFailureKind::UnresolvedGeneratedProducer);
                    origin.failure_span_key = Some(site.span.key());
                    origin.failure_span = Some(site.span.clone());
                    origin
                        .parents
                        .try_reserve_exact(1)
                        .map_err(|_| fault("blocked generated ancestry allocation"))?;
                    origin.parents.push(crate::syntax::origin::ExpansionOrigin {
                        expansion: site.expansion,
                        invocation_identity: Some(site.invocation),
                        invocation: None,
                    });
                    break;
                }
            }
        } else if let Some(spelling) = blocked
            .iter()
            .filter_map(|(identity, _)| self.graph.import_spelling(*identity))
            .min()
        {
            // No original macro producer is blocked: name the first stalled
            // ordinary import (by spelling, which is stable across captures).
            use std::fmt::Write;
            write!(&mut diagnostic.message, "; blocked import `{spelling}`")
                .map_err(|_| fault("blocked import diagnostic formatting"))?;
        }
        Ok(diagnostic)
    }
    /// Retained unresolved declarations; exposing a demand grants no file access.
    pub fn unresolved_module_sources(
        &self,
    ) -> impl Iterator<Item = &crate::frontend::generated_headers::GeneratedModuleSource> {
        self.module_sources.values()
    }
    pub fn unresolved_ordinary_module_sources(
        &self,
    ) -> impl Iterator<Item = &crate::frontend::source_module_declarations::SourceModuleDeclaration>
    {
        self.original_modules
            .values()
            .map(|row| &row.declaration)
            .chain(
                self.staged_modules
                    .values()
                    .flat_map(|module| module.ordinary_modules.iter()),
            )
            .filter(|declaration| self.producers.contains(&declaration.producer()))
    }
    fn ordinary_module_binding(
        &self,
        producer: Digest,
    ) -> Result<
        (
            OrdinaryModuleOwner,
            &crate::frontend::source_module_declarations::SourceModuleDeclaration,
        ),
        Diagnostic,
    > {
        if self.seal.is_some() || self.failed.is_some() {
            return Err(fault("ordinary module discovery is sealed or failed"));
        }
        let original = self
            .original_modules
            .get(&producer)
            .map(|row| (OrdinaryModuleOwner::Original(row.ordinal), &row.declaration));
        let mut matches = original
            .into_iter()
            .chain(self.staged_modules.iter().flat_map(|(parent, module)| {
                module
                    .ordinary_modules
                    .iter()
                    .filter(move |declaration| declaration.producer() == producer)
                    .map(move |declaration| (OrdinaryModuleOwner::Staged(*parent), declaration))
            }));
        let binding = matches
            .next()
            .ok_or_else(|| fault("ordinary module declaration absent"))?;
        let retained_complete = self.staged_modules.get(&producer).is_some_and(|child| {
            child.namespace_complete
                && matches!(&child.syntax, StagedModuleSyntax::Ordinary { syntax, .. }
                    if syntax.origin().declaration().fingerprint() == binding.1.fingerprint())
        });
        if matches.next().is_some() || (!self.producers.contains(&producer) && !retained_complete) {
            return Err(fault(
                "ordinary module declaration ambiguous or no longer pending",
            ));
        }
        Ok(binding)
    }
    pub fn prepare_ordinary_module_source(
        &self,
        producer: Digest,
        authorities: &crate::source::include_paths::IncludeAuthorities,
    ) -> Result<crate::db::poll::InputRequest, Diagnostic> {
        if !self.producers.contains(&producer) {
            return Err(fault("ordinary module capture requires pending producer"));
        }
        let (parent, declaration) = self.ordinary_module_binding(producer)?;
        let environment = self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("ordinary module compile environment absent"))?
            .0;
        let directory = match parent {
            OrdinaryModuleOwner::Original(_) => self.original_module_directory(producer)?,
            OrdinaryModuleOwner::Staged(parent) => self
                .staged_modules
                .get(&parent)
                .ok_or_else(|| fault("ordinary module staged parent absent"))?
                .syntax
                .capture()
                .record()
                .directory(),
        };
        declaration.capture_request(authorities, directory, environment)
    }
    pub(crate) fn ordinary_module_capture_caller(
        &self,
        producer: Digest,
    ) -> Result<crate::source::changes::SourceSelection, Diagnostic> {
        let (_, declaration) = self.ordinary_module_binding(producer)?;
        Ok(declaration.source().selection().clone())
    }
    pub fn ordinary_module_syntax_input(
        &self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
    ) -> crate::db::poll::QueryPoll<
        std::sync::Arc<crate::source::ordinary_module::OrdinaryModuleSyntaxInput>,
    > {
        use crate::db::{
            dependency::{InputKey, InputValue},
            poll::{InputRequest, QueryPoll},
            provider::SyntaxProvider,
        };
        let binding = (|| {
            let InputRequest::CaptureModule {
                request,
                compile_environment,
            } = input
            else {
                return Err(fault("ordinary module requires capture input"));
            };
            let (parent, declaration) = self.ordinary_module_binding(request.producer())?;
            if matches!(parent, OrdinaryModuleOwner::Original(ordinal) if self.macro_export_sources.contains_key(&ordinal))
            {
                return Err(Diagnostic::new("LRR-E-MACRO-EXPORT-PUBLICATION", "macro-export child requires role-only source adoption, not ordinary module syntax".into()));
            }
            let expected = self
                .declarative
                .as_ref()
                .ok_or_else(|| fault("ordinary module environment absent"))?
                .0;
            if *compile_environment != expected {
                return Err(fault("ordinary module compile environment changed"));
            }
            match parent {
                OrdinaryModuleOwner::Original(_) => {
                    self.require_original_module_source(db, request.producer())?;
                }
                OrdinaryModuleOwner::Staged(parent) => {
                    let parent = self
                        .staged_modules
                        .get(&parent)
                        .ok_or_else(|| fault("ordinary module parent capture owner absent"))?;
                    let capture = parent.syntax.capture();
                    let published = db
                        .input(&InputKey::ModuleRecord(capture.input_identity()))
                        .ok_or_else(|| fault("ordinary module parent capture absent"))?;
                    if !matches!(published.as_ref(), InputValue::ModuleRecord {captured,..} if captured.identity() == capture.record().identity())
                        || declaration.source().selection()
                            != parent.syntax.prepared_source().selection()
                    {
                        return Err(fault("ordinary module parent source changed"));
                    }
                }
            }
            Ok((declaration, expected))
        })();
        let (declaration, expected) = match binding {
            Ok(binding) => binding,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match SyntaxProvider::compile_environment(db, declaration.scope().crate_key) {
            QueryPoll::Ready(Ok(environment)) if environment.fingerprint() == expected => {}
            QueryPoll::Ready(Ok(_)) => {
                return QueryPoll::Ready(Err(fault("ordinary module adopted environment changed")));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let capture = match SyntaxProvider::module_capture(db, input) {
            QueryPoll::Ready(Ok(capture)) => capture,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready((|| {
            let view = std::sync::Arc::new(
                crate::source::ordinary_module::OrdinaryModuleSourceView::new(
                    declaration,
                    input,
                    capture,
                )?,
            );
            crate::source::ordinary_module::OrdinaryModuleSyntaxInput::prepare(view)
                .map(std::sync::Arc::new)
        })())
    }
    pub(crate) fn module_capture_caller(
        &self,
        producer: Digest,
    ) -> Result<crate::source::changes::SourceSelection, Diagnostic> {
        let (_, _, source, _) = self.module_binding(producer)?;
        Ok(source.selection().clone())
    }
    pub(crate) fn adopted_module_caller_directory(
        &self,
        producer: Digest,
    ) -> Result<Option<&std::path::Path>, Diagnostic> {
        let (demand, _, _, _) = self.module_binding(producer)?;
        self.generated_source_modules
            .get(&demand.header().expansion)
            .map(|module| {
                self.staged_modules
                    .get(module)
                    .map(|staged| staged.syntax.capture().record().directory())
                    .ok_or_else(|| fault("adopted module caller directory absent"))
            })
            .transpose()
    }
    pub(crate) fn module_namespace_complete(&self, producer: Digest) -> bool {
        self.staged_modules
            .get(&producer)
            .is_some_and(|module| module.namespace_complete)
    }
    /// Prepare a demand from its retained declaration and actual invocation file.
    /// `directory` is the module layout captured by the explicit source coordinator.
    pub fn prepare_module_source(
        &self,
        producer: Digest,
        authorities: &crate::source::include_paths::IncludeAuthorities,
        directory: &std::path::Path,
    ) -> Result<crate::db::poll::InputRequest, Diagnostic> {
        self.prepare_module_source_with_layout(
            producer,
            authorities,
            directory,
            &Default::default(),
        )
    }
    /// Derive an automatic caller layout from original syntax and retained expansion frames.
    pub(crate) fn prepare_module_source_in_caller_context(
        &self,
        producer: Digest,
        authorities: &crate::source::include_paths::IncludeAuthorities,
        file_directory: &std::path::Path,
    ) -> Result<crate::db::poll::InputRequest, Diagnostic> {
        let layout = self.module_caller_layout(producer)?;
        self.prepare_module_source_with_layout(producer, authorities, file_directory, &layout)
    }
    fn module_caller_layout(
        &self,
        producer: Digest,
    ) -> Result<crate::syntax::modules::InlineModuleLayout, Diagnostic> {
        let (_, record, source, _) = self.module_binding(producer)?;
        let scope = self.generated_module_scope(record.key().fingerprint())?;
        let current = self
            .generated_owners
            .get(&record.key().fingerprint())
            .ok_or_else(|| fault("module caller retained owner absent"))?;
        if scope.catalog.owner() != &current.1 {
            return Err(fault("module caller catalogue owner mismatch"));
        }
        let mut frames = scope
            .ancestors
            .iter()
            .rev()
            .copied()
            .chain(std::iter::once(scope.catalog));
        let mut parent = frames
            .next()
            .ok_or_else(|| fault("module caller layout root absent"))?;
        let validate = |catalog: &crate::frontend::generated_macros::GeneratedMacroCatalog| -> Result<(), Diagnostic> {
            let (ordinal, owner) = self.generated_owners.get(&catalog.record_identity())
                .ok_or_else(|| fault("module caller ancestor owner absent"))?;
            if *ordinal != current.0 || owner != catalog.owner()
                || (match self.generated_source_modules.get(&catalog.record_identity()) {
                    Some(module) => self.staged_modules.get(module).map(|staged| staged.syntax.prepared_source()),
                    None => self.sources.get(*ordinal),
                }).is_none_or(|actual| actual.selection() != source.selection())
                || catalog.cfg() != source.captured_macro_cfg()?
                || self.records.get(&catalog.record_identity()).is_none_or(|actual| {
                    actual.key().consumer_compile_env != record.key().consumer_compile_env
                        || actual.descriptor().fingerprint() != catalog.captured_owner().0.record().descriptor().fingerprint()
                })
            {
                return Err(fault("module caller ancestor source or environment mismatch"));
            }
            Ok(())
        };
        validate(parent)?;
        let mut layout = source.captured_inline_module_layout(parent.owner())?;
        for child in frames {
            validate(child)?;
            layout.append(parent.inline_caller_extension(child.owner())?)?;
            parent = child;
        }
        Ok(layout)
    }
    fn prepare_module_source_with_layout(
        &self,
        producer: Digest,
        authorities: &crate::source::include_paths::IncludeAuthorities,
        directory: &std::path::Path,
        layout: &crate::syntax::modules::InlineModuleLayout,
    ) -> Result<crate::db::poll::InputRequest, Diagnostic> {
        let (demand, record, source, base) = self.module_binding(producer)?;
        let from = crate::source::include_paths::IncludeLocation {
            unit: source.selection().outline.crate_key,
            root: authorities.unique_source_root()?,
            path: source.selection().path.clone(),
            origin: base.origin,
        };
        Ok(crate::db::poll::InputRequest::CaptureModule {
            request: Box::new(authorities.module_request_with_layout(
                &from,
                directory,
                demand.request().clone(),
                producer,
                demand.site(),
                layout,
            )?),
            compile_environment: record.key().consumer_compile_env,
        })
    }
    fn module_binding(&self, producer: Digest) -> Result<ModuleBinding<'_>, Diagnostic> {
        if self.seal.is_some() || self.failed.is_some() {
            return Err(fault("module discovery is sealed or failed"));
        }
        let demand = self
            .module_sources
            .get(&producer)
            .ok_or_else(|| fault("retained module demand absent"))?;
        let identity = demand.header().expansion;
        let record = self
            .records
            .get(&identity)
            .ok_or_else(|| fault("module parent expansion absent"))?;
        let (ordinal, owner) = self
            .generated_owners
            .get(&identity)
            .ok_or_else(|| fault("module parent owner absent"))?;
        let source = match self.generated_source_modules.get(&identity) {
            Some(module) => self
                .staged_modules
                .get(module)
                .ok_or_else(|| fault("module captured adopted caller absent"))?
                .syntax
                .prepared_source(),
            None => self
                .sources
                .get(*ordinal)
                .ok_or_else(|| fault("module captured caller absent"))?,
        };
        let base = self
            .source_bases
            .get(&identity)
            .ok_or_else(|| fault("module invocation file base absent"))?;
        if demand.scope().crate_key != owner.crate_key
            || !demand.scope().module_path.starts_with(&owner.module_path)
            || demand.scope().owner.is_some()
        {
            return Err(fault("module generated parent scope mismatch"));
        }
        // Virtual include bases need their retained physical layout before routing.
        if base.source.path != source.selection().path.as_str()
            || base.source.content.0 != source.selection().outline.parse.content.0
        {
            return Err(fault(
                "module invocation requires retained included-file layout",
            ));
        }
        Ok((demand, record, source, base))
    }
    /// Pure lookup: adoption is necessary, but does not discharge child discovery.
    pub fn module_source_view(
        &self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
    ) -> crate::db::poll::QueryPoll<crate::source::module_view::CapturedModuleSourceView> {
        use crate::db::{
            dependency::{InputKey, InputValue},
            poll::{InputRequest, QueryPoll},
            provider::SyntaxProvider,
        };
        let result = (|| {
            input.validate()?;
            let InputRequest::CaptureModule {
                request,
                compile_environment,
            } = input
            else {
                return Err(fault("module source view requires module input"));
            };
            let (demand, record, source, base) = self.module_binding(request.producer())?;
            if request.declaration() != demand.request()
                || *compile_environment != record.key().consumer_compile_env
                || request.candidates().is_empty()
                || request.candidates().iter().any(|candidate| {
                    candidate.from().unit != source.selection().outline.crate_key
                        || candidate.from().path.as_str() != base.source.path
                        || candidate.from().origin != base.origin
                        || candidate.include_origin() != demand.site()
                })
            {
                return Err(fault("module declaration/input binding mismatch"));
            }
            let selection = source.selection();
            if let Some(module) = self
                .generated_source_modules
                .get(&record.key().fingerprint())
            {
                let staged = self
                    .staged_modules
                    .get(module)
                    .ok_or_else(|| fault("module adopted caller absent"))?;
                let capture = staged.syntax.capture();
                let published = db
                    .input(&InputKey::ModuleRecord(capture.input_identity()))
                    .ok_or_else(|| fault("module adopted caller publication absent"))?;
                if !matches!(published.as_ref(), InputValue::ModuleRecord { captured, .. } if captured.identity() == capture.record().identity())
                    || staged.syntax.prepared_source().selection() != selection
                {
                    return Err(fault("module adopted caller publication changed"));
                }
            } else {
                let published = db
                    .input(&InputKey::CapturedSource(selection.location_identity()))
                    .ok_or_else(|| fault("module caller publication absent"))?;
                if !matches!(published.as_ref(), InputValue::CapturedSource(actual) if actual == selection)
                {
                    return Err(fault("module caller publication changed"));
                }
            }
            let cfg = source.captured_macro_cfg()?;
            let published_cfg = db
                .input(&InputKey::CrateCfg(selection.outline.crate_key))
                .ok_or_else(|| fault("module caller cfg publication absent"))?;
            if !matches!(published_cfg.as_ref(), InputValue::CrateCfg(actual) if actual == cfg) {
                return Err(fault("module caller cfg publication changed"));
            }
            Ok((demand, record, source, base, *compile_environment))
        })();
        let (demand, record, source, base, environment) = match result {
            Ok(binding) => binding,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match SyntaxProvider::compile_environment(db, source.selection().outline.crate_key) {
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::Ready(Ok(actual)) if actual.fingerprint() != environment => {
                return QueryPoll::Ready(Err(fault(
                    "module compile environment publication changed",
                )));
            }
            QueryPoll::Ready(Ok(_)) => {}
        }
        let capture = match SyntaxProvider::module_capture(db, input) {
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::Ready(Ok(capture)) => capture,
        };
        QueryPoll::Ready((|| {
            let parent = super::generated_items::GeneratedItems::admit(
                record.key(),
                record.clone(),
                self.epoch,
            )?;
            crate::source::module_view::CapturedModuleSourceView::new(
                capture,
                parent,
                demand.clone(),
                source.share(),
                base.clone(),
            )
        })())
    }
    /// Build frozen child declaration inputs; neither publish nor close a producer.
    pub fn module_syntax_input(
        &self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
    ) -> crate::db::poll::QueryPoll<std::sync::Arc<crate::source::module_syntax::ModuleSyntaxInput>>
    {
        use crate::db::poll::QueryPoll;
        match self.module_source_view(db, input) {
            QueryPoll::NeedInput(request) => QueryPoll::NeedInput(request),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::Ready(Ok(view)) => QueryPoll::Ready(
                crate::source::module_syntax::ModuleSyntaxInput::prepare(std::sync::Arc::new(view))
                    .map(std::sync::Arc::new),
            ),
        }
    }
    /// Register child declaration jobs from re-observed adopted inputs. No database publication.
    pub fn stage_module_source(
        &mut self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
    ) -> crate::db::poll::QueryPoll<std::sync::Arc<crate::source::module_syntax::ModuleSyntaxInput>>
    {
        use crate::db::poll::QueryPoll;
        let syntax = match self.module_syntax_input(db, input) {
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::Ready(Ok(syntax)) => syntax,
        };
        match self.stage_module_projection(db, input, StagedModuleSyntax::Generated(syntax.clone()))
        {
            QueryPoll::Ready(Ok(())) => QueryPoll::Ready(
                self.staged_modules
                    .get(&syntax.origin().producer())
                    .ok_or_else(|| fault("retained generated stage projection absent"))
                    .and_then(|staged| staged.syntax.generated().cloned()),
            ),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    pub fn stage_ordinary_module_source(
        &mut self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
    ) -> crate::db::poll::QueryPoll<
        std::sync::Arc<crate::source::ordinary_module::OrdinaryModuleSyntaxInput>,
    > {
        use crate::db::poll::QueryPoll;
        let syntax = match self.ordinary_module_syntax_input(db, input) {
            QueryPoll::Ready(Ok(syntax)) => syntax,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let producer = syntax.origin().declaration().producer();
        let parent = match self.ordinary_module_binding(producer) {
            Ok((parent, declaration))
                if declaration.fingerprint() == syntax.origin().declaration().fingerprint() =>
            {
                parent
            }
            Ok(_) => return QueryPoll::Ready(Err(fault("ordinary staged declaration changed"))),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match self.stage_module_projection(
            db,
            input,
            StagedModuleSyntax::Ordinary {
                syntax: syntax.clone(),
                parent,
            },
        ) {
            QueryPoll::Ready(Ok(())) => QueryPoll::Ready(
                self.staged_modules
                    .get(&producer)
                    .ok_or_else(|| fault("retained ordinary stage projection absent"))
                    .and_then(|staged| match &staged.syntax {
                        StagedModuleSyntax::Ordinary { syntax, .. } => Ok(syntax.clone()),
                        StagedModuleSyntax::Generated(_) => {
                            Err(fault("ordinary stage projection kind changed"))
                        }
                    }),
            ),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    fn module_projection_inherited<'a>(
        &'a self,
        syntax: &StagedModuleSyntax,
    ) -> Result<super::source_declarative::InheritedMacroContext<'a>, Diagnostic> {
        let providers = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("module inherited owners absent"))?
            .2;
        Self::projection_inherited_in(&self.fragments, providers, &self.staged_modules, self.exports.as_deref(), syntax, 0)
    }
    fn projection_inherited_in<'a>(
        fragments: &'a BTreeMap<Digest, GeneratedFragment>,
        providers: &'a [super::source_declarative::SourceDeclarative],
        modules: &'a BTreeMap<Digest, StagedModule>,
        exports: Option<&'a sealed_macro_export_providers::SealedMacroExportProviders>,
        syntax: &StagedModuleSyntax,
        depth: usize,
    ) -> Result<super::source_declarative::InheritedMacroContext<'a>, Diagnostic> {
        match syntax {
            StagedModuleSyntax::Generated(syntax) => Self::module_scope_in(
                fragments,
                providers,
                modules,
                exports,
                syntax.origin().parent_header().expansion,
                depth + 1,
            )?
            .inherited_for_module(syntax.origin().producer()),
            StagedModuleSyntax::Ordinary { syntax, parent } => {
                let provider = match parent {
                    OrdinaryModuleOwner::Original(ordinal) => providers
                        .get(*ordinal)
                        .ok_or_else(|| fault("ordinary module inherited original parent absent"))?,
                    OrdinaryModuleOwner::Staged(parent) => {
                        &modules
                            .get(parent)
                            .ok_or_else(|| fault("ordinary module inherited staged parent absent"))?
                            .provider
                    }
                };
                let context = super::source_declarative::CapturedMacroContext {
                    exports,
                    provider,
                    owners: providers,
                    before: 0,
                    inherited: None,
                };
                let mut inherited =
                    super::source_declarative::InheritedMacroContext::for_source_module(
                        syntax.origin().declaration().producer(),
                        context,
                    )?;
                if let OrdinaryModuleOwner::Staged(parent) = parent {
                    inherited.extend_context(&Self::projection_inherited_in(
                        fragments,
                        providers,
                        modules,
                        exports,
                        &modules
                            .get(parent)
                            .ok_or_else(|| fault("ordinary module inherited parent absent"))?
                            .syntax,
                        depth + 1,
                    )?)?;
                }
                Ok(inherited)
            }
        }
    }
    fn module_routes_with_access(
        &self,
        syntax: &StagedModuleSyntax,
        provider: &super::source_declarative::SourceDeclarative,
        deferred: bool,
    ) -> Result<
        Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
        Diagnostic,
    > {
        let providers = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("module route source providers absent"))?
            .2;
        let inherited = self.module_projection_inherited(syntax)?;
        let mut routes = provider.module_routes(providers, &inherited)?;
        let outline = syntax.prepared_source().outline();
        let access = self
            .graph
            .declared_module_access(&outline.root_scope, outline.profile.edition)?;
        let names = crate::frontend::source_macro_routes::module_names(
            &outline.root_scope.module_path,
            outline.profile.edition,
        )?;
        let root_route = routes
            .first_mut()
            .ok_or_else(|| fault("module root route absent"))?;
        let root = std::sync::Arc::get_mut(root_route)
            .ok_or_else(|| fault("module root route ownership changed"))?;
        if root
            .module_access
            .get(&(outline.root_scope.crate_key, names.clone()))
            .is_some_and(|old| old != &access)
        {
            return Err(fault("module root declaration access changed"));
        }
        root.module_access
            .insert((outline.root_scope.crate_key, names), access);
        if deferred {
            let _bytes = routes.iter().try_fold(0usize, |n, route| {
                n.checked_add(route.metadata_bytes()?)
                    .ok_or_else(|| fault("staged route copy metadata limit"))
            })?;
            for route in &mut routes {
                if std::sync::Arc::get_mut(route).is_none() {
                    *route = std::sync::Arc::new(route.owned_copy()?);
                }
                std::sync::Arc::get_mut(route)
                    .ok_or_else(|| fault("staged route ownership changed"))?
                    .defer_body_owners(outline.root_scope.crate_key, syntax.producer())?;
            }
        }
        Ok(routes)
    }
    /// Refresh candidate import/macro facts without completing any source frontier.
    fn refresh_staged_module_routes(&mut self, producer: Digest) -> Result<(), Diagnostic> {
        let staged = self
            .staged_modules
            .get(&producer)
            .ok_or_else(|| fault("module route refresh source absent"))?;
        let routes = self.module_routes_with_access(
            &staged.syntax,
            &staged.provider,
            !staged.namespace_complete,
        )?;
        let bytes = |routes: &[std::sync::Arc<
            crate::frontend::source_macro_routes::SourceMacroRoutes,
        >]| {
            routes.iter().try_fold(0usize, |n, route| {
                n.checked_add(route.metadata_bytes()?)
                    .ok_or_else(|| fault("module route refresh metadata limit"))
            })
        };
        let old = bytes(
            self.module_routes
                .get(&producer)
                .ok_or_else(|| fault("module route refresh prior facts absent"))?,
        )?;
        let new = bytes(&routes)?;
        let retained = self
            .retained_record_bytes
            .checked_add(new.saturating_sub(old))
            .ok_or_else(|| fault("module route refresh retained metadata limit"))?;
        self.module_routes.insert(producer, routes);
        self.retained_record_bytes = retained;
        Ok(())
    }
    fn stage_module_projection(
        &mut self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
        syntax: StagedModuleSyntax,
    ) -> crate::db::poll::QueryPoll<()> {
        use crate::db::poll::QueryPoll;
        let producer = syntax.producer();
        if let Some(staged) = self.staged_modules.get(&producer) {
            return QueryPoll::Ready(if staged.syntax.identity() == syntax.identity() {
                Ok(())
            } else {
                Err(fault("staged module identity changed"))
            });
        }
        let planned = (|| {
            if self.pending.is_some()
                || self.pending_source_derive.is_some()
                || self.pending_generated.is_some()
                || self.pending_module.is_some()
                || self.pending_procedural.is_some()
            {
                return Err(fault("module staging cannot replace a pending macro phase"));
            }
            self.work.preflight_namespace_change()?;
            self.staged_modules
                .len()
                .checked_add(self.sources.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| fault("staged module source count limit"))?;
            if let StagedModuleSyntax::Generated(generated) = &syntax {
                let parent =
                    self.generated_module_scope(generated.origin().parent_header().expansion)?;
                if !parent.catalog.unresolved_module_sources().any(|d| {
                    d.producer() == producer && d.header() == generated.origin().parent_header()
                }) {
                    return Err(fault("staged module parent catalogue binding mismatch"));
                }
            }
            let ordinary_modules = crate::frontend::source_module_declarations::SourceModuleDeclaration::from_captured(syntax.prepared_source())?;
            let ordinary_bytes = ordinary_modules.iter().try_fold(0usize, |n, declaration| {
                n.checked_add(declaration.metadata_bytes())
                    .ok_or_else(|| fault("ordinary module facts metadata overflow"))
            })?;
            let bytes = self
                .staged_module_bytes
                .checked_add(ordinary_bytes)
                .and_then(|n| n.checked_add(syntax.metadata_bytes()))
                .and_then(|n| n.checked_add(512))
                .ok_or_else(|| fault("staged module source metadata limit"))?;
            let outline = syntax.prepared_source().outline();
            let parse_job = crate::db::key::QueryKey::Outline(
                syntax.prepared_source().selection().outline.clone(),
            )
            .fingerprint();
            if self.source_jobs.contains_key(&parse_job)
                || self.module_parse_jobs.contains_key(&parse_job)
            {
                return Err(fault("staged module source context already exists"));
            }
            let ordinal = match &syntax {
                StagedModuleSyntax::Generated(generated) => {
                    self.generated_owners
                        .get(&generated.origin().parent_header().expansion)
                        .ok_or_else(|| fault("staged module source parent order absent"))?
                        .0
                }
                StagedModuleSyntax::Ordinary { parent, .. } => match parent {
                    OrdinaryModuleOwner::Original(ordinal) => *ordinal,
                    OrdinaryModuleOwner::Staged(parent) => {
                        self.staged_modules
                            .get(parent)
                            .ok_or_else(|| fault("ordinary module source parent order absent"))?
                            .ordinal
                    }
                },
            };
            let order =
                u32::try_from(ordinal).map_err(|_| fault("module source order overflow"))?;
            let mut jobs = vec![DiscoveryJob {
                module_order: order,
                source_offset: 0,
                kind: DiscoveryKind::ParseSource,
                identity: parse_job,
            }];
            for import in &outline.imports {
                jobs.push(DiscoveryJob {
                    module_order: order,
                    source_offset: import.range.start,
                    kind: DiscoveryKind::ResolveImport,
                    identity: NameNamespaces::import_identity(import),
                });
            }
            let mut producers = BTreeSet::new();
            for event in &outline.textual_events {
                if let TextualEventKind::Producer { identity, .. } = event.kind {
                    producers.insert(identity);
                    jobs.push(DiscoveryJob {
                        module_order: order,
                        source_offset: u32::try_from(event.at)
                            .map_err(|_| fault("module producer source offset overflow"))?,
                        kind: DiscoveryKind::ResolveMacro,
                        identity,
                    });
                }
            }
            let call_frontiers = outline
                .item_invocations
                .iter()
                .cloned()
                .chain(
                    outline
                        .items
                        .iter()
                        .filter_map(crate::syntax::outline::attribute_producer),
                )
                .map(|call| (call.scope.clone(), call.frontier))
                .collect::<BTreeSet<_>>();
            for (scope, members) in &outline.scopes {
                for blocker in &members.blockers {
                    if !call_frontiers.contains(&(scope.clone(), blocker.syntax))
                        && producers.insert(blocker.syntax)
                    {
                        jobs.push(DiscoveryJob {
                            module_order: order,
                            source_offset: 0,
                            kind: DiscoveryKind::ResolveMacro,
                            identity: blocker.syntax,
                        });
                    }
                }
            }
            self.work.preflight_enqueue_batch(&mut jobs)?;
            let mut stamps = self.compiler_inputs.clone();
            for key in [
                crate::db::dependency::InputKey::ModuleRecord(input.fingerprint()),
                crate::db::dependency::InputKey::UnitCompileEnv(outline.root_scope.crate_key),
            ] {
                let value = db
                    .input(&key)
                    .ok_or_else(|| fault("staged module adopted input disappeared"))?;
                let fingerprint = value.fingerprint();
                if stamps.get(&key).is_some_and(|old| *old != fingerprint) {
                    return Err(fault("staged module compiler input changed"));
                }
                stamps.insert(key, fingerprint);
            }
            let providers = &self
                .declarative
                .as_ref()
                .ok_or_else(|| fault("staged module providers absent"))?
                .2;
            if providers
                .get(ordinal)
                .ok_or_else(|| fault("staged module source owner absent"))?
                .macro_export_source_role()
                .is_some()
            {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-EXPORT-PUBLICATION",
                    "macro-export child module requires its own role-aware source adoption".into(),
                ));
            }
            let mut provider = super::source_declarative::SourceDeclarative::new(
                syntax.prepared_source().share(),
            )?;
            if let Some(catalog) = &providers
                .get(ordinal)
                .ok_or_else(|| fault("staged module procedural owner absent"))?
                .procedural
            {
                provider.install_procedural(catalog.clone())?;
            }
            provider.compiler = providers
                .get(ordinal)
                .ok_or_else(|| fault("staged module compiler owner absent"))?
                .compiler
                .clone();
            let annotation_scan = syntax
                .prepared_source()
                .declaration_inventory(Default::default())?;
            let annotation_plan = if annotation_scan.generated_origin().is_none() {
                Some(std::sync::Arc::new(super::source_declarative::original_annotations::OriginalAnnotationPlan::capture(
                    syntax.prepared_source(), annotation_scan,
                )?))
            } else {
                None
            };
            let mut source_calls = BTreeMap::new();
            for call in &outline.item_invocations {
                source_calls.insert(call.item_input, call.clone());
            }
            for item in &outline.items {
                if let Some(call) = crate::syntax::outline::attribute_producer(item) {
                    if producers.insert(call.item_input) {
                        jobs.push(DiscoveryJob {
                            module_order: order,
                            source_offset: call.range.start,
                            kind: DiscoveryKind::ResolveMacro,
                            identity: call.item_input,
                        });
                    }
                    source_calls.insert(call.item_input, call);
                }
            }
            if provider.procedural.is_some() {
                for item in &outline.items {
                    if let Some(call) = crate::syntax::outline::potential_attribute_producer(item) {
                        if crate::syntax::outline::attribute_producer(item).is_none() {
                            provider.reserve_test_attribute(&call)?;
                        }
                        if producers.insert(call.item_input) {
                            jobs.push(DiscoveryJob {
                                module_order: order,
                                source_offset: call.range.start,
                                kind: DiscoveryKind::ResolveMacro,
                                identity: call.item_input,
                            });
                        }
                        source_calls.insert(call.item_input, call);
                    }
                }
            }
            let call_bytes = source_calls.values().try_fold(0usize, |bytes, call| {
                bytes
                    .checked_add(call.scope.heap_bytes())
                    .and_then(|n| n.checked_add(256))
                    .ok_or_else(|| fault("staged source call metadata limit"))
            })?;
            self.work.preflight_enqueue_batch(&mut jobs)?;
            let routes = self.module_routes_with_access(&syntax, &provider, true)?;
            let mut route_bytes = 0usize;
            for route in &routes {
                route_bytes = route_bytes
                    .checked_add(route.metadata_bytes()?)
                    .ok_or_else(|| fault("staged module route metadata limit"))?;
            }
            let bytes = bytes
                .checked_add(route_bytes)
                .and_then(|n| n.checked_add(call_bytes))
                .and_then(|n| {
                    n.checked_add(
                        annotation_plan
                            .as_ref()
                            .map_or(0, |plan| plan.metadata_bytes()),
                    )
                })
                .and_then(|n| n.checked_add(provider.module_index_bytes().ok()?))
                .ok_or_else(|| fault("staged module macro index metadata limit"))?;
            Ok((
                bytes,
                parse_job,
                jobs,
                producers,
                stamps,
                provider,
                routes,
                ordinary_modules,
                ordinal,
                source_calls,
                annotation_plan,
            ))
        })();
        let (
            bytes,
            parse_job,
            jobs,
            producers,
            stamps,
            provider,
            routes,
            ordinary_modules,
            ordinal,
            source_calls,
            annotation_plan,
        ) = match planned {
            Ok(plan) => plan,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let applied = (|| {
            for declaration in &ordinary_modules {
                unique_graph(&mut self.graph)?.reserve_generated_frontier(
                    declaration.scope().clone(),
                    declaration.producer(),
                )?;
            }
            unique_graph(&mut self.graph)?.disclose_module_source_scope(
                producer,
                syntax.prepared_source().outline().root_scope.clone(),
            )?;
            unique_graph(&mut self.graph)?
                .admit_source_outline(syntax.prepared_source().outline())?;
            let hidden = syntax
                .prepared_source()
                .outline()
                .items
                .iter()
                .filter(|item| {
                    crate::syntax::outline::potential_attribute_producer(item)
                        .is_some_and(|call| source_calls.contains_key(&call.item_input))
                })
                .map(|item| item.def_key.fingerprint())
                .collect::<BTreeSet<_>>();
            unique_graph(&mut self.graph)?.hide_pending_attribute_declarations(&hidden);
            for call in source_calls
                .values()
                .filter(|call| call.attribute_input.is_some())
            {
                unique_graph(&mut self.graph)?
                    .reserve_generated_frontier(call.scope.clone(), call.frontier)?;
            }
            self.graph.require_source(
                syntax.prepared_source().selection(),
                syntax.prepared_source().outline(),
            )?;
            self.module_routes.insert(producer, routes);
            Self::install_macro_routes_with_modules(
                unique_graph(&mut self.graph)?,
                &self
                    .declarative
                    .as_ref()
                    .ok_or_else(|| fault("staged module providers absent"))?
                    .2,
                &self.module_routes,
                &self.nested_sources,
                &self.nested_inert_proofs,
                &mut self.route_install,
            )?;
            self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
            self.work.enqueue_batch(jobs)?;
            self.work.namespace_changed()?;
            Ok::<_, Diagnostic>(())
        })();
        if let Err(error) = applied {
            self.failed = Some(error.clone());
            return QueryPoll::Ready(Err(error));
        }
        self.compiler_inputs = stamps;
        self.staged_module_bytes = bytes;
        self.module_parse_jobs.insert(parse_job, producer);
        self.producers.extend(producers.iter().copied());
        self.staged_modules.insert(
            producer,
            StagedModule {
                ordinary_modules,
                provider,
                annotation_plan,
                catalogs: BTreeMap::new(),
                completed_invocations: BTreeSet::new(),
                syntax: syntax.clone(),
                ordinal,
                parse_job,
                imports: syntax.prepared_source().outline().imports.len(),
                producers: producers.len(),
                source_producers: producers,
                source_calls,
                namespace_complete: false,
            },
        );
        QueryPoll::Ready(Ok(()))
    }
    /// Prepare a physical child's exact declarative request without executing it or closing its frontier.
    pub fn module_macro_demand(
        &mut self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
        invocation: Digest,
    ) -> crate::db::poll::QueryPoll<super::source_declarative::SourceDeclarativeDemand> {
        use crate::db::poll::QueryPoll;
        let syntax = match self.module_syntax_input(db, input) {
            QueryPoll::Ready(Ok(syntax)) => syntax,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        QueryPoll::Ready((|| {
            let staged = self
                .staged_modules
                .get(&syntax.origin().producer())
                .ok_or_else(|| fault("module macro demand requires staged source"))?;
            if staged.syntax.identity() != syntax.identity() {
                return Err(fault("module macro demand staged identity changed"));
            }
            self.staged_module_demand(syntax.origin().producer(), invocation)
        })())
    }
    fn staged_procedural_selection(
        &mut self,
        producer: Digest,
        invocation: Digest,
    ) -> Result<Option<ProceduralSourceSelection>, Diagnostic> {
        let providers = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("module procedural providers absent"))?
            .2;
        let namespaces = Self::install_macro_routes_with_modules(
            unique_graph(&mut self.graph)?,
            providers,
            &self.module_routes,
            &self.nested_sources,
            &self.nested_inert_proofs,
            &mut self.route_install,
        )?;
        let staged = self
            .staged_modules
            .get(&producer)
            .ok_or_else(|| fault("module procedural source absent"))?;
        let Some(catalog) = &staged.provider.procedural else {
            return Ok(None);
        };
        let inherited = self.module_projection_inherited(&staged.syntax)?;
        let lookup = staged
            .provider
            .module_invocation(invocation, &namespaces, &inherited)?;
        let definition = match lookup {
            crate::frontend::macro_names::PathLookup::Defined(definition) => definition,
            crate::frontend::macro_names::PathLookup::Ambiguous(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "ambiguous physical module procedural selection".into(),
                ));
            }
            _ => return Ok(None),
        };
        if catalog
            .select(&definition, super::identity::ProceduralKind::Function)?
            .is_none()
        {
            return Ok(None);
        }
        let input = crate::service::function_macro_requests::FunctionInvocationInput::from_source(
            staged.syntax.prepared_source(),
            invocation,
        )?;
        Ok(Some(ProceduralSourceSelection {
            module: Some(producer),
            nested: false,
            input,
            definition,
            catalog: catalog.clone(),
        }))
    }
    fn staged_module_demand(
        &mut self,
        producer: Digest,
        invocation: Digest,
    ) -> Result<super::source_declarative::SourceDeclarativeDemand, Diagnostic> {
        use super::source_declarative::{SourceDeclarativeDemand, SourceDeclarativeRequest};
        use crate::frontend::macro_names::PathLookup;
        let providers = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("module macro providers absent"))?
            .2;
        let namespaces = Self::install_macro_routes_with_modules(
            unique_graph(&mut self.graph)?,
            providers,
            &self.module_routes,
            &self.nested_sources,
            &self.nested_inert_proofs,
            &mut self.route_install,
        )?;
        let staged = self
            .staged_modules
            .get(&producer)
            .ok_or_else(|| fault("module macro staged source absent"))?;
        let syntax = &staged.syntax;
        let provider = &staged.provider;
        let inherited = self.module_projection_inherited(syntax)?;
        let definition = match provider.module_invocation(invocation, &namespaces, &inherited)? {
            PathLookup::Defined(definition) => definition,
            PathLookup::NeedInput(id) => return Ok(SourceDeclarativeDemand::NeedInput(id)),
            PathLookup::Absent => return Ok(SourceDeclarativeDemand::NeedInput(invocation)),
            PathLookup::Ambiguous(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "ambiguous module macro".into(),
                ));
            }
            PathLookup::Inaccessible(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-INACCESSIBLE",
                    "inaccessible module macro".into(),
                ));
            }
        };
        let (env, engine, _) = self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("module macro environment absent"))?;
        if let Some(request) = provider.module_compiler_request(
            invocation,
            &definition,
            *env,
            *engine,
            Some(&inherited),
        )? {
            return Ok(SourceDeclarativeDemand::Ready(Box::new(request)));
        }
        let (key, tokens, site) =
            provider.module_invocation_input(invocation, &definition, *env, *engine)?;
        let context = super::source_declarative::CapturedMacroContext {
            exports: self.exports.as_deref(),
            provider,
            owners: providers,
            before: 0,
            inherited: Some(&inherited),
        };
        let request = if provider.macro_index().owns_definition(definition.identity) {
            provider.generated_request(&definition, key, &tokens, site)?
        } else if let Some(items) = context.inherited_body(&definition) {
            SourceDeclarativeRequest::module_generated(&definition, key, &tokens, site, items)?
        } else {
            context.definition_owner(&definition)?.generated_request(
                &definition,
                key,
                &tokens,
                site,
            )?
        };
        Ok(SourceDeclarativeDemand::Ready(Box::new(request)))
    }
    fn admit_pending_module_expansion(
        &mut self,
        record: std::sync::Arc<super::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        let (ticket, producer, request) = self
            .pending_module
            .as_ref()
            .ok_or_else(|| fault("module expansion demand absent"))?;
        if (request.is_compiler() && !self.admitting_compiler)
            || (request.procedural_request().is_some() && !self.admitting_procedural)
            || record.key() != request.key()
        {
            return Err(Diagnostic::new(
                "LRR-E-EXPANSION-INPUT",
                "module expansion differs from exact pending declarative demand".into(),
            ));
        }
        let ticket = ticket.clone();
        let producer = *producer;
        let request = request.clone();
        let staged = self
            .staged_modules
            .get(&producer)
            .ok_or_else(|| fault("module expansion source absent"))?;
        let source = staged.syntax.prepared_source();
        let call = staged
            .source_calls
            .get(&ticket.job().identity)
            .ok_or_else(|| fault("pending module invocation absent"))?
            .clone();
        let replacement = if call.attribute_input.is_some() {
            Some(std::sync::Arc::new(
                super::source_replacement::SourceReplacementProof::from_adopted(
                    source, &call, &record,
                )?,
            ))
        } else {
            None
        };
        let identity = record.key().fingerprint();
        if self.records.contains_key(&identity) {
            return Err(fault(
                "module expansion duplicate or retained record count limit",
            ));
        }
        let source_base = request.source_base()?;
        let items = super::generated_items::GeneratedItems::admit(
            request.key(),
            record.clone(),
            self.epoch,
        )?;
        let plan = crate::frontend::generated_headers::GeneratedHeaderPlan::prepare(
            &items,
            call.scope.clone(),
            crate::syntax::token_wire::Key(request.key().parent_context.0),
            source.captured_macro_cfg()?,
            self.epoch,
        )?;
        let catalog = std::sync::Arc::new(
            crate::frontend::generated_macros::GeneratedMacroCatalog::for_discovery(
                &items,
                call.scope.clone(),
                crate::syntax::token_wire::Key(request.key().parent_context.0),
                source.captured_macro_cfg()?,
                self.epoch,
            )?,
        );
        let ready = catalog.ready_for_outward();
        let deferred = !ready;
        let mut release = catalog.call_frontiers().clone();
        release.extend(
            catalog
                .outward()
                .iter()
                .map(|entry| entry.definition.identity),
        );
        let imports = plan
            .discovery_imports()
            .iter()
            .map(NameNamespaces::import_identity)
            .collect::<BTreeSet<_>>();
        let macros = catalog
            .outward()
            .iter()
            .map(|entry| entry.definition.identity)
            .collect::<BTreeSet<_>>();
        let opaque = plan
            .blocked()
            .filter(|id| !imports.contains(id) && !(ready && macros.contains(id)))
            .copied()
            .collect::<BTreeSet<_>>();
        let completed = ready && opaque.is_empty();
        let base_bytes = source_base_bytes(&source_base)?;
        let catalog_bytes = catalog
            .metadata_bytes()?
            .checked_mul(2)
            .ok_or_else(|| fault("module catalog/index metadata overflow"))?;
        let retained_bytes = self
            .retained_record_bytes
            .checked_add(record.estimated_bytes()?)
            .and_then(|n| n.checked_add(base_bytes))
            .and_then(|n| {
                n.checked_add(
                    replacement
                        .as_ref()
                        .map(|proof| proof.metadata_bytes())
                        .transpose()
                        .ok()?
                        .unwrap_or(0),
                )
            })
            .and_then(|n| n.checked_add(catalog_bytes))
            .and_then(|n| n.checked_add(call.scope.heap_bytes()))
            .and_then(|n| n.checked_add(1024))
            .ok_or_else(|| fault("retained module expansion metadata limit"))?;
        let generated_bytes = self
            .generated_bytes
            .checked_add(record.descriptor().wire_bytes)
            .ok_or_else(|| fault("module generated wire limit"))?;
        let headers = self
            .generated_headers
            .checked_add(items.items().len())
            .ok_or_else(|| fault("module generated header count limit"))?;
        let ordinal = staged.ordinal;
        let mut update = DiscoveryUpdate::default();
        for id in &imports {
            update.jobs.push(DiscoveryJob {
                module_order: ticket.job().module_order,
                source_offset: call.range.start,
                kind: DiscoveryKind::ResolveImport,
                identity: *id,
            });
        }
        for id in opaque.iter().filter(|id| !release.contains(id)) {
            update.jobs.push(DiscoveryJob {
                module_order: ticket.job().module_order,
                source_offset: call.range.start,
                kind: DiscoveryKind::ScanNestedDeclarations,
                identity: *id,
            });
        }
        if deferred {
            update.jobs.push(DiscoveryJob {
                module_order: ticket.job().module_order,
                source_offset: call.range.start,
                kind: DiscoveryKind::ScanNestedDeclarations,
                identity,
            });
        }
        update
            .members
            .extend(items.items().iter().map(|header| header.key.fingerprint()));
        update.members.push(record.descriptor().fingerprint());
        if completed {
            update.members.push(call.item_input);
        }
        update
            .dependencies
            .push((identity, record.descriptor().fingerprint()));
        update.dependencies.extend(
            self.compiler_inputs
                .iter()
                .map(|(key, fingerprint)| (key.fingerprint(), *fingerprint)),
        );
        if let Some(proof) = &replacement {
            update
                .dependencies
                .push((proof.fingerprint(), proof.fingerprint()));
        }
        self.work.preflight_complete(&ticket, &mut update)?;
        let applied = (|| {
            self.retain_module_sources(&plan)?;
            plan.publish(unique_graph(&mut self.graph)?)?;
            if completed {
                self.staged_modules
                    .get_mut(&producer)
                    .ok_or_else(|| fault("module provider disappeared"))?
                    .provider
                    .complete_generated_bindings(call.item_input, catalog.clone())?;
                let other_pending = self
                    .staged_modules
                    .get(&producer)
                    .ok_or_else(|| fault("module source disappeared"))?
                    .syntax
                    .prepared_source()
                    .outline()
                    .item_invocations
                    .iter()
                    .any(|other| {
                        other.item_input != call.item_input
                            && other.scope == call.scope
                            && other.frontier == call.frontier
                            && !self.staged_modules.get(&producer).is_some_and(|module| {
                                module.completed_invocations.contains(&other.item_input)
                            })
                    });
                if !other_pending {
                    unique_graph(&mut self.graph)?
                        .release_generated_frontier(&call.scope, call.frontier);
                }
                for id in &macros {
                    unique_graph(&mut self.graph)?.release_generated_frontier(&call.scope, *id);
                }
            }
            self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
            self.work.complete(&ticket, update)?;
            if completed {
                self.producers.remove(&call.item_input);
                let module = self
                    .staged_modules
                    .get_mut(&producer)
                    .ok_or_else(|| fault("module completion owner absent"))?;
                module.provider.module_index_bytes()?;
                module.completed_invocations.insert(call.item_input);
            }
            self.producers.extend(
                opaque
                    .iter()
                    .filter(|id| !deferred || !release.contains(id))
                    .copied(),
            );
            Ok::<_, Diagnostic>(())
        })();
        if let Err(error) = applied {
            self.failed = Some(error.clone());
            return Err(error);
        }
        if deferred {
            let catalog = std::sync::Arc::try_unwrap(catalog)
                .map_err(|_| fault("module fragment catalogue ownership differs"))?;
            self.fragments.insert(
                identity,
                GeneratedFragment {
                    module: Some(producer),
                    source_derive: None,
                    source_prefix: None,
                    catalog,
                    children: BTreeMap::new(),
                    child_order: BTreeMap::new(),
                    active_child: None,
                    jobs: Vec::new(),
                    ordinal,
                    call: call.clone(),
                    release,
                    members: Vec::new(),
                    dependencies: Vec::new(),
                },
            );
            self.producers.insert(identity);
        } else {
            self.staged_modules
                .get_mut(&producer)
                .ok_or_else(|| fault("module catalog owner disappeared"))?
                .catalogs
                .insert(identity, catalog);
        }
        if let Some(proof) = replacement {
            self.source_replacements.insert(proof.fingerprint(), proof);
        }
        self.generated_source_modules.insert(identity, producer);
        self.generated_owners
            .insert(identity, (ordinal, call.scope));
        self.source_bases.insert(identity, source_base);
        self.records.insert(identity, record);
        self.retained_record_bytes = retained_bytes;
        self.generated_bytes = generated_bytes;
        self.generated_headers = headers;
        self.pending_module = None;
        if completed && let Err(error) = self.refresh_staged_module_routes(producer) {
            self.failed = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }
    /// Re-observe adopted inputs and check actual source work without releasing any frontier.
    pub fn module_source_readiness(
        &mut self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
    ) -> crate::db::poll::QueryPoll<ModuleSourceReadiness> {
        use crate::db::poll::QueryPoll;
        let syntax = match self.module_syntax_input(db, input) {
            QueryPoll::Ready(Ok(syntax)) => syntax,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        match self.module_projection_readiness(db, StagedModuleSyntax::Generated(syntax)) {
            QueryPoll::Ready(Ok(ProjectionReadiness::Pending { waiting })) => {
                QueryPoll::Ready(Ok(ModuleSourceReadiness::Pending { waiting }))
            }
            QueryPoll::Ready(Ok(ProjectionReadiness::Ready(ready))) => QueryPoll::Ready((|| {
                Ok(ModuleSourceReadiness::Ready(Box::new(ReadyModuleSource {
                    syntax: ready.syntax.generated()?.clone(),
                    identity: ready.identity,
                    macros: ready.macros,
                    namespace: ready.namespace,
                    records: ready.records,
                })))
            })(
            )),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    pub fn ordinary_module_readiness(
        &mut self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
    ) -> crate::db::poll::QueryPoll<OrdinaryModuleReadiness> {
        use crate::db::poll::QueryPoll;
        let syntax = match self.ordinary_module_syntax_input(db, input) {
            QueryPoll::Ready(Ok(syntax)) => syntax,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let staged = match self
            .staged_modules
            .get(&syntax.origin().declaration().producer())
        {
            Some(staged) if staged.syntax.identity() == syntax.identity() => staged.syntax.clone(),
            _ => {
                return QueryPoll::Ready(Err(fault(
                    "ordinary readiness requires exact staged source",
                )));
            }
        };
        match self.module_projection_readiness(db, staged) {
            QueryPoll::Ready(Ok(ProjectionReadiness::Pending { waiting })) => {
                QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Pending { waiting }))
            }
            QueryPoll::Ready(Ok(ProjectionReadiness::Ready(ready))) => QueryPoll::Ready(Ok(
                OrdinaryModuleReadiness::Ready(Box::new(ReadyOrdinaryModuleSource {
                    syntax,
                    identity: ready.identity,
                    macros: ready.macros,
                    namespace: ready.namespace,
                    records: ready.records,
                })),
            )),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    fn staged_source_calls(
        &self,
        producer: Digest,
    ) -> Result<Vec<crate::syntax::outline::ItemInvocation>, Diagnostic> {
        let module = self
            .staged_modules
            .get(&producer)
            .ok_or_else(|| fault("staged source calls absent"))?;
        let mut calls = Vec::new();
        calls
            .try_reserve_exact(module.source_calls.len())
            .map_err(|_| fault("staged source call replay allocation"))?;
        calls.extend(module.source_calls.values().cloned());
        Ok(calls)
    }
    fn module_projection_readiness(
        &mut self,
        db: &mut crate::db::memo::Database,
        syntax: StagedModuleSyntax,
    ) -> crate::db::poll::QueryPoll<ProjectionReadiness> {
        use crate::db::poll::QueryPoll;
        QueryPoll::Ready((|| {
            for (key, fingerprint) in &self.compiler_inputs {
                if db
                    .input(key)
                    .is_none_or(|value| value.fingerprint() != *fingerprint)
                {
                    return Err(fault("module readiness compiler input stamp changed"));
                }
            }
            let producer = syntax.producer();
            let calls = self.staged_source_calls(producer)?;
            // Known ambiguity is a failure even while import jobs keep membership pending.
            // An unresolved candidate remains pending; it cannot certify absence.
            for call in &calls {
                if self.procedural_selections.contains_key(&call.item_input) {
                    let _candidate = self.staged_procedural_selection(producer, call.item_input)?;
                }
            }
            let staged = self
                .staged_modules
                .get(&producer)
                .ok_or_else(|| fault("module readiness requires staged source"))?;
            if syntax.identity() != staged.syntax.identity() {
                return Err(fault("module readiness adopted projection changed"));
            }
            let mut waiting = BTreeSet::new();
            if !self.parsed_modules.contains(&staged.parse_job) {
                waiting.insert(staged.parse_job);
            }
            waiting.extend(
                staged
                    .source_producers
                    .intersection(&self.producers)
                    .copied(),
            );
            waiting.extend(staged.provider.macro_index().pending_producers());
            waiting.extend(
                self.fragments.iter().filter_map(|(id, fragment)| {
                    (fragment.module == Some(producer)).then_some(*id)
                }),
            );
            let outline = syntax.prepared_source().outline();
            for call in &calls {
                if !staged.completed_invocations.contains(&call.item_input) {
                    waiting.insert(call.item_input);
                }
            }
            for import in &outline.imports {
                let id = NameNamespaces::import_identity(import);
                if self.imports.resolved.binary_search(&id).is_err()
                    && !self.graph.disclosed_import_facet(id, self.namespace)?
                {
                    waiting.insert(id);
                }
            }
            if !waiting.is_empty() {
                let mut inputs = Vec::new();
                inputs
                    .try_reserve_exact(waiting.len())
                    .map_err(|_| fault("module readiness pending allocation"))?;
                inputs.extend(waiting);
                return Ok(ProjectionReadiness::Pending { waiting: inputs });
            }
            let count = self
                .generated_owners
                .values()
                .filter(|(_, scope)| {
                    scope.crate_key == outline.root_scope.crate_key
                        && scope
                            .module_path
                            .starts_with(&outline.root_scope.module_path)
                })
                .count();
            let mut records = Vec::new();
            records
                .try_reserve_exact(count)
                .map_err(|_| fault("module readiness record allocation"))?;
            for (id, record) in &self.records {
                if self.generated_owners.get(id).is_some_and(|(_, scope)| {
                    scope.crate_key == outline.root_scope.crate_key
                        && scope
                            .module_path
                            .starts_with(&outline.root_scope.module_path)
                }) {
                    records.push(record.descriptor());
                }
            }
            let inherited = self.module_projection_inherited(&syntax)?;
            staged.provider.revalidate_module_catalogs(
                &self
                    .declarative
                    .as_ref()
                    .ok_or_else(|| fault("module readiness providers absent"))?
                    .2,
                &inherited,
            )?;
            let index_bytes = records
                .len()
                .checked_mul(128)
                .ok_or_else(|| fault("module readiness invocation index limit"))?;
            let mut invocation_keys = BTreeMap::new();
            for record in &records {
                if invocation_keys
                    .insert(record.key.invocation, &record.key)
                    .is_some()
                {
                    return Err(fault("module readiness duplicate invocation identity"));
                }
            }
            let annotation_plan = staged.annotation_plan.clone();
            let module_provider = staged.provider.module_snapshot()?;
            for call in &calls {
                if call.attribute_input.is_some() {
                    let names = Self::install_macro_routes_with_modules(
                        unique_graph(&mut self.graph)?,
                        &self
                            .declarative
                            .as_ref()
                            .ok_or_else(|| fault("module attribute replay providers absent"))?
                            .2,
                        &self.module_routes,
                        &self.nested_sources,
                        &self.nested_inert_proofs,
                        &mut self.route_install,
                    )?;
                    if let Some(proof) = self.source_empty_derives.get(&call.item_input) {
                        if super::source_derives::prepare_empty_with_annotations(
                            syntax.prepared_source(),
                            &module_provider,
                            call.item_input,
                            &names,
                            annotation_plan.as_deref(),
                        )?
                        .as_ref()
                            != Some(proof)
                        {
                            return Err(fault(
                                "staged compiler annotation changed before readiness",
                            ));
                        }
                    } else {
                        let selected =
                            self.attribute_selections
                                .get(&call.item_input)
                                .ok_or_else(|| {
                                    fault("staged attribute selection absent before readiness")
                                })?;
                        let current = crate::service::source_attribute_requests::SourceAttributeSelection::select(syntax.prepared_source(), &module_provider, call.item_input, &names)?.ok_or_else(|| fault("staged attribute selection incomplete before readiness"))?;
                        let expected = invocation_keys
                            .get(&call.item_input)
                            .ok_or_else(|| fault("staged attribute record absent"))?;
                        if selected.fingerprint() != current.fingerprint()
                            || expected.procedural_epoch != Some(self.epoch)
                        {
                            return Err(fault(
                                "staged attribute selection changed before readiness",
                            ));
                        }
                    }
                    continue;
                }
                let expected = invocation_keys
                    .get(&call.item_input)
                    .ok_or_else(|| fault("completed module call record absent"))?;
                if let Some(definition) = self.procedural_selections.get(&call.item_input).cloned()
                {
                    let selected = self
                        .staged_procedural_selection(producer, call.item_input)?
                        .ok_or_else(|| {
                            fault("module procedural selection incomplete before readiness")
                        })?;
                    let export = selected
                        .catalog
                        .select(
                            &selected.definition,
                            super::identity::ProceduralKind::Function,
                        )?
                        .ok_or_else(|| fault("module procedural export absent before readiness"))?;
                    if selected.definition != definition
                        || &expected.macro_identity != export.identity()
                        || expected.procedural_epoch != Some(self.epoch)
                    {
                        return Err(fault(
                            "physical module procedural selection changed before readiness",
                        ));
                    }
                    continue;
                }
                match self.staged_module_demand(producer, call.item_input)? {
                    super::source_declarative::SourceDeclarativeDemand::Ready(request)
                        if request.key() == *expected => {}
                    _ => return Err(fault("physical module selection changed before readiness")),
                }
            }
            let staged = self
                .staged_modules
                .get(&producer)
                .ok_or_else(|| fault("ready module source disappeared"))?;
            let base_bytes = staged
                .provider
                .module_index_bytes()?
                .checked_add(syntax.metadata_bytes())
                .and_then(|n| n.checked_add(std::mem::size_of::<ReadyProjection>()))
                .and_then(|n| n.checked_add(index_bytes))
                .ok_or_else(|| fault("module readiness base metadata overflow"))?;
            let _bytes = records.iter().try_fold(base_bytes, |n, record| {
                n.checked_add(record.estimated_bytes())
                    .ok_or_else(|| fault("module readiness record metadata overflow"))
            })?;
            let macros = staged.provider.completed_published_index()?;
            let mut key = crate::core::digest::StableEncoder::new("lrr/v3/module-source-readiness");
            key.record(&self.key);
            if let Some(prepared) = &self.compiler_profiles {
                key.bool(true);
                key.digest(prepared.identity());
            } else {
                key.bool(false);
            }
            key.digest(syntax.identity());
            key.u32(self.namespace.tag());
            key.u64(
                u64::try_from(self.compiler_inputs.len())
                    .map_err(|_| fault("module readiness stamp count"))?,
            );
            for (input, fingerprint) in &self.compiler_inputs {
                key.record(input);
                key.digest(*fingerprint);
            }
            key.u64(
                u64::try_from(records.len()).map_err(|_| fault("module readiness record count"))?,
            );
            for record in &records {
                key.record(record);
            }
            key.u64(
                u64::try_from(outline.imports.len())
                    .map_err(|_| fault("module readiness import count"))?,
            );
            for import in &outline.imports {
                key.digest(NameNamespaces::import_identity(import));
            }
            Ok(ProjectionReadiness::Ready(Box::new(ReadyProjection {
                identity: key.finish(),
                syntax,
                macros,
                namespace: self.namespace,
                records,
            })))
        })())
    }
    pub fn complete_ordinary_module_namespace(
        &mut self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
    ) -> crate::db::poll::QueryPoll<OrdinaryModuleReadiness> {
        use crate::db::poll::QueryPoll;
        let ready = match self.ordinary_module_readiness(db, input) {
            QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Ready(ready))) => ready,
            other => return other,
        };
        let result = (|| {
            let producer = ready.syntax.origin().declaration().producer();
            let staged = self
                .staged_modules
                .get(&producer)
                .ok_or_else(|| fault("ordinary ready source absent"))?;
            if staged.namespace_complete {
                return Ok(());
            }
            let StagedModuleSyntax::Ordinary { parent, .. } = staged.syntax else {
                return Err(fault("ordinary ready projection kind changed"));
            };
            let ordinal = staged.ordinal;
            let child = staged.provider.module_snapshot()?;
            let routes = self.module_routes_with_access(&staged.syntax, &staged.provider, false)?;
            let mut bytes = self
                .retained_record_bytes
                .checked_add(child.module_index_bytes()?)
                .and_then(|n| n.checked_add(ready.syntax.metadata_bytes()))
                .and_then(|n| n.checked_add(256))
                .ok_or_else(|| fault("ordinary catalogue retained metadata overflow"))?;
            for route in &routes {
                bytes = bytes
                    .checked_add(route.metadata_bytes()?)
                    .ok_or_else(|| fault("ordinary catalogue route size overflow"))?;
            }
            self.work.preflight_namespace_change()?;
            let applied = (|| {
                match parent {
                    OrdinaryModuleOwner::Original(parent) => {
                        self.declarative
                            .as_mut()
                            .ok_or_else(|| fault("ordinary original parent absent"))?
                            .2
                            .get_mut(parent)
                            .ok_or_else(|| fault("ordinary original parent ordinal absent"))?
                            .complete_ordinary_module(producer, child.clone())?;
                    }
                    OrdinaryModuleOwner::Staged(parent) => {
                        self.staged_modules
                            .get_mut(&parent)
                            .ok_or_else(|| fault("ordinary parent source absent"))?
                            .provider
                            .complete_ordinary_module(producer, child.clone())?;
                    }
                }
                self.declarative
                    .as_mut()
                    .ok_or_else(|| fault("ordinary completion original owner absent"))?
                    .2
                    .get_mut(ordinal)
                    .ok_or_else(|| fault("ordinary completion original owner ordinal absent"))?
                    .retain_source_module_owner(producer, child)?;
                self.module_routes.insert(producer, routes);
                Self::install_macro_routes_with_modules(
                    unique_graph(&mut self.graph)?,
                    &self
                        .declarative
                        .as_ref()
                        .ok_or_else(|| fault("ordinary completion indexes absent"))?
                        .2,
                    &self.module_routes,
                    &self.nested_sources,
                    &self.nested_inert_proofs,
                    &mut self.route_install,
                )?;
                unique_graph(&mut self.graph)?.release_generated_frontier(
                    ready.syntax.origin().declaration().scope(),
                    producer,
                );
                self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
                self.staged_modules
                    .get_mut(&producer)
                    .ok_or_else(|| fault("ordinary completion source disappeared"))?
                    .namespace_complete = true;
                self.producers.remove(&producer);
                self.work.namespace_changed()?;
                Ok::<_, Diagnostic>(())
            })();
            if let Err(error) = applied {
                self.failed = Some(error.clone());
                return Err(error);
            }
            self.retained_record_bytes = bytes;
            Ok(())
        })();
        QueryPoll::Ready(result.map(|()| OrdinaryModuleReadiness::Ready(ready)))
    }
    pub fn admit_module_macro_catalogue(
        &mut self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
    ) -> crate::db::poll::QueryPoll<ModuleSourceReadiness> {
        use crate::db::poll::QueryPoll;
        let ready = match self.module_source_readiness(db, input) {
            QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(ready))) => ready,
            other => return other,
        };
        let result = (|| {
            self.work.preflight_namespace_change()?;
            let producer = ready.syntax.origin().producer();
            let parent_id = ready.syntax.origin().parent_header().expansion;
            let parent = self.generated_module_scope(parent_id)?;
            if parent.catalog.module_source(producer).is_none() {
                return Err(fault("module source catalogue binding absent"));
            }
            if !parent
                .catalog
                .unresolved_module_sources()
                .any(|source| source.producer() == producer)
            {
                return Ok(());
            }
            let parent_bytes = parent.catalog.metadata_bytes()?;
            let source = self
                .staged_modules
                .get(&producer)
                .ok_or_else(|| fault("ready module provider absent"))?
                .provider
                .module_snapshot()?;
            let bytes = self
                .retained_record_bytes
                .checked_add(source.module_index_bytes()?)
                .and_then(|n| n.checked_add(ready.syntax.metadata_bytes()))
                .ok_or_else(|| fault("module macro catalogue retained metadata limit"))?;
            let mut owners = self.fragments.values_mut().filter(|fragment| {
                fragment.catalog.record_identity() == parent_id
                    || fragment.children.contains_key(&parent_id)
            });
            let fragment = owners
                .next()
                .ok_or_else(|| fault("pending module catalogue owner absent"))?;
            if owners.next().is_some() {
                return Err(fault("pending module catalogue owner ambiguous"));
            }
            let catalogue = match fragment.children.get_mut(&parent_id) {
                Some(child) => &mut child.catalog,
                None => &mut fragment.catalog,
            };
            if let Err(error) = catalogue.complete_module_catalogue(producer, source) {
                self.failed = Some(error.clone());
                return Err(error);
            }
            let growth = catalogue.metadata_bytes()?.saturating_sub(parent_bytes);
            let Some(bytes) = bytes
                .checked_add(growth)
            else {
                let error = fault("module macro catalogue growth exceeds retained metadata limit");
                self.failed = Some(error.clone());
                return Err(error);
            };
            self.retained_record_bytes = bytes;
            self.work.namespace_changed()?;
            Ok(())
        })();
        QueryPoll::Ready(result.map(|()| ModuleSourceReadiness::Ready(ready)))
    }
    pub fn complete_module_namespace(
        &mut self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
    ) -> crate::db::poll::QueryPoll<ModuleSourceReadiness> {
        use crate::db::poll::QueryPoll;
        let ready = match self.admit_module_macro_catalogue(db, input) {
            QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(ready))) => ready,
            other => return other,
        };
        let result = (|| {
            let producer = ready.syntax.origin().producer();
            if self
                .staged_modules
                .get(&producer)
                .is_some_and(|module| module.namespace_complete)
            {
                return Ok(());
            }
            self.work.preflight_namespace_change()?;
            let parent =
                self.generated_module_scope(ready.syntax.origin().parent_header().expansion)?;
            let inherited = parent.inherited_for_module(producer)?;
            let providers = &self
                .declarative
                .as_ref()
                .ok_or_else(|| fault("module namespace providers absent"))?
                .2;
            let routes = self
                .staged_modules
                .get(&producer)
                .ok_or_else(|| fault("module namespace source absent"))?
                .provider
                .module_routes(providers, &inherited)?;
            let bytes = routes
                .iter()
                .try_fold(self.retained_record_bytes, |n, route| {
                    n.checked_add(route.metadata_bytes()?)
                        .ok_or_else(|| fault("retained module route overflow"))
                })?;
            let owner = self
                .generated_owners
                .get(&ready.syntax.origin().parent_header().expansion)
                .ok_or_else(|| fault("module namespace parent owner absent"))?
                .1
                .clone();
            self.module_routes.insert(producer, routes);
            let applied = (|| {
                Self::install_macro_routes_with_modules(
                    unique_graph(&mut self.graph)?,
                    providers,
                    &self.module_routes,
                    &self.nested_sources,
                    &self.nested_inert_proofs,
                    &mut self.route_install,
                )?;
                unique_graph(&mut self.graph)?.release_generated_frontier(&owner, producer);
                self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
                self.staged_modules
                    .get_mut(&producer)
                    .ok_or_else(|| fault("module namespace source disappeared"))?
                    .namespace_complete = true;
                self.producers.remove(&producer);
                self.work.namespace_changed()?;
                Ok::<_, Diagnostic>(())
            })();
            if let Err(error) = applied {
                self.failed = Some(error.clone());
                return Err(error);
            }
            self.retained_record_bytes = bytes;
            Ok(())
        })();
        QueryPoll::Ready(result.map(|()| ModuleSourceReadiness::Ready(ready)))
    }
    pub fn module_source_progress(&self, producer: Digest) -> Option<ModuleSourceProgress> {
        let staged = self.staged_modules.get(&producer)?;
        let parsed = self.parsed_modules.contains(&staged.parse_job);
        Some(ModuleSourceProgress {
            parsed,
            declarations: if parsed {
                staged.syntax.prepared_source().outline().items.len()
            } else {
                0
            },
            namespace: self.namespace,
            completed_import_facets: staged
                .syntax
                .prepared_source()
                .outline()
                .imports
                .iter()
                .filter(|import| {
                    self.work.completed_job(
                        DiscoveryKind::ResolveImport,
                        NameNamespaces::import_identity(import),
                    )
                })
                .count(),
            scheduled_imports: staged.imports,
            scheduled_producers: staged.producers,
            completed_invocations: staged.completed_invocations.len(),
        })
    }
    /// Resolve a child's inherited textual context only from its adopted declaration.
    pub fn module_parent_macro_lookup(
        &self,
        db: &mut crate::db::memo::Database,
        input: &crate::db::poll::InputRequest,
        name: &str,
    ) -> crate::db::poll::QueryPoll<crate::frontend::macro_scopes::HygienicLookup> {
        use crate::db::poll::QueryPoll;
        let view = match self.module_source_view(db, input) {
            QueryPoll::Ready(Ok(view)) => view,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        QueryPoll::Ready((|| {
            let scope = self.generated_module_scope(view.parent_header().expansion)?;
            if scope.catalog.record_identity() != view.parent_header().expansion
                || !scope
                    .catalog
                    .module_source(view.producer())
                    .is_some_and(|d| d.header() == view.parent_header())
            {
                return Err(fault("module parent catalogue binding mismatch"));
            }
            scope.catalog.lookup_module_parent_using(
                view.producer(),
                name,
                view.invocation_base().context,
                Some(scope.compiler_context()),
                &scope.ancestors,
            )
        })())
    }
    fn generated_module_scope(
        &self,
        identity: Digest,
    ) -> Result<GeneratedCompilerScope<'_>, Diagnostic> {
        let providers = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("module parent captured providers absent"))?
            .2;
        Self::module_scope_in(
            &self.fragments,
            providers,
            &self.staged_modules,
            self.exports.as_deref(),
            identity,
            0,
        )
    }
    /// Cold sealed-demand export of the actual selected output's outer macro
    /// boundary. The query owner must retain it and its precise dependencies.
    pub(crate) fn generated_body_macro_boundary(
        &self,
        unit: &super::generated_syntax::GeneratedSyntaxInput,
    ) -> Result<std::sync::Arc<super::body_macro_boundary::GeneratedBodyBoundary>, Diagnostic> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        let seal = self
            .seal
            .as_ref()
            .ok_or_else(|| fault("generated body boundary discovery is unsealed"))?;
        let seal_value = self
            .body_boundary_seal_fingerprint
            .ok_or_else(|| fault("generated body boundary sealed value is absent"))?;
        let identity = unit.items.record().key().fingerprint();
        let (ordinal, owner) = self
            .generated_owners
            .get(&identity)
            .ok_or_else(|| fault("generated body boundary owner absent"))?;
        if owner != &unit.scope
            || unit.seal != self.key
            || unit.items.epoch() != self.epoch
            || unit.cfg.fingerprint() != self.sources[*ordinal].captured_macro_cfg()?.fingerprint()
            || self
                .records
                .get(&identity)
                .is_none_or(|record| !record.same_record(unit.items.record()))
        {
            return Err(fault("generated body boundary actual output/owner differs"));
        }
        let providers = &self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("generated body boundary source providers absent"))?
            .2;
        let (selected, modules, parents) =
            if let Some(empty) = self.completed_empty_boundaries.get(&identity) {
                if empty.source_ordinal != *ordinal
                    || empty.call.scope != *owner
                    || empty.call.item_input != unit.items.record().key().invocation
                    || !empty.record.same_record(unit.items.record())
                {
                    return Err(fault("completed empty macro insertion receipt differs"));
                }
                let provider = providers
                    .get(*ordinal)
                    .ok_or_else(|| fault("empty macro insertion provider absent"))?;
                let captured = super::source_declarative::CapturedMacroContext {
                    exports: self.exports.as_deref(),
                    provider,
                    owners: providers,
                    before: u64::from(empty.call.range.start),
                    inherited: None,
                };
                (
                    None,
                    std::sync::Arc::new(BTreeMap::new()),
                    captured.retain_source_scopes(owner)?,
                )
            } else {
                let scope = self.generated_module_scope(identity)?;
                if !scope
                    .catalog
                    .retained_items()
                    .record()
                    .same_record(unit.items.record())
                {
                    return Err(fault("generated body selected catalogue output differs"));
                }
                let mut parents = scope.compiler_context().retain_source_scopes(
                    scope
                        .ancestors
                        .last()
                        .map_or(scope.catalog.owner(), |root| root.owner()),
                )?;
                let extra = scope.ancestors.len();
                // Keep the existing innermost-to-outermost ancestor order exactly.
                let mut generated = Vec::new();
                generated
                    .try_reserve_exact(extra + parents.scopes.len())
                    .map_err(|_| crate::runtime::arena::resource())?;
                for (at, ancestor) in scope.ancestors.iter().enumerate() {
                    let child = if at == 0 {
                        scope.catalog
                    } else {
                        scope.ancestors[at - 1]
                    };
                    generated.push(
                        ancestor
                            .retain_outer(ancestor.invocation_outer(child.invocation())?)?
                            .ok_or_else(|| fault("generated ancestor root differs"))?,
                    );
                    parents
                        .items
                        .push(std::sync::Arc::clone(ancestor.retained_items()));
                }
                generated.append(&mut parents.scopes);
                parents.scopes = generated;
                (
                    Some(scope.catalog.retain_body_outer(owner, u64::MAX)?),
                    std::sync::Arc::clone(scope.catalog.retained_module_scopes()),
                    parents,
                )
            };
        for items in std::iter::once(&unit.items).chain(parents.items.iter()) {
            let record = items.record();
            if seal
                .input_dependencies()
                .binary_search(&(
                    record.key().fingerprint(),
                    record.descriptor().fingerprint(),
                ))
                .is_err()
            {
                return Err(fault(
                    "owning boundary record is absent from actual sealed closure",
                ));
            }
        }
        super::body_macro_boundary::GeneratedBodyBoundary::prepare(
            std::sync::Arc::clone(&unit.items),
            owner.clone(),
            selected,
            modules,
            parents,
            self.key.clone(),
            seal_value,
        )
    }
    fn module_scope_in<'a>(
        fragments: &'a BTreeMap<Digest, GeneratedFragment>,
        providers: &'a [super::source_declarative::SourceDeclarative],
        modules: &'a BTreeMap<Digest, StagedModule>,
        exports: Option<&'a sealed_macro_export_providers::SealedMacroExportProviders>,
        identity: Digest,
        depth: usize,
    ) -> Result<GeneratedCompilerScope<'a>, Diagnostic> {
        let mut candidates = fragments.values().filter(|fragment| {
            fragment.catalog.record_identity() == identity
                || fragment.children.contains_key(&identity)
        });
        let Some(fragment) = candidates.next() else {
            let mut found = None;
            for (module, provider) in providers
                .iter()
                .map(|p| (None, p))
                .chain(modules.iter().map(|(id, m)| (Some(*id), &m.provider)))
            {
                if let Some(completed) = provider.completed_catalogue(identity)? {
                    if found.is_some() {
                        return Err(fault("completed module source catalogue ambiguous"));
                    }
                    let inherited = match module {
                        Some(id) => {
                            let staged = modules
                                .get(&id)
                                .ok_or_else(|| fault("completed module caller absent"))?;
                            Some(Self::projection_inherited_in(
                                fragments,
                                providers,
                                modules,
                                exports,
                                &staged.syntax,
                                depth + 1,
                            )?)
                        }
                        None => None,
                    };
                    found = Some(GeneratedCompilerScope {
                        catalog: completed.catalog,
                        ancestors: completed.ancestors,
                        inherited,
                        captured: super::source_declarative::CapturedMacroContext {
                            exports,
                            provider,
                            owners: providers,
                            before: completed.before,
                            inherited: None,
                        },
                    });
                }
            }
            return found.ok_or_else(|| fault("module parent discovery fragment absent"));
        };
        if candidates.next().is_some() {
            return Err(fault("module parent discovery fragment is ambiguous"));
        }
        let inherited = if let Some(module) = fragment.module {
            let staged = modules
                .get(&module)
                .ok_or_else(|| fault("nested module caller absent"))?;
            Some(Self::projection_inherited_in(
                fragments,
                providers,
                modules,
                exports,
                &staged.syntax,
                depth + 1,
            )?)
        } else {
            None
        };
        let (catalog, mut parent) = match fragment.children.get(&identity) {
            Some(child) => (&child.catalog, child.parent),
            None => (&fragment.catalog, None),
        };
        let mut ancestors = Vec::new();
        if catalog.record_identity() != fragment.catalog.record_identity() {
            ancestors
                .try_reserve_exact(256)
                .map_err(|_| fault("module ancestor context allocation"))?;
            while let Some(id) = parent {
                let ancestor = fragment
                    .children
                    .get(&id)
                    .ok_or_else(|| fault("module ancestor frame absent"))?;
                ancestors.push(&ancestor.catalog);
                parent = ancestor.parent;
            }
            ancestors.push(&fragment.catalog);
        }
        let provider = match fragment.module {
            Some(module) => {
                &modules
                    .get(&module)
                    .ok_or_else(|| fault("module parent adopted provider absent"))?
                    .provider
            }
            None => providers
                .get(fragment.ordinal)
                .ok_or_else(|| fault("module parent provider ordinal absent"))?,
        };
        Ok(GeneratedCompilerScope {
            catalog,
            captured: super::source_declarative::CapturedMacroContext {
                exports,
                provider,
                owners: providers,
                before: u64::from(fragment.call.range.start),
                inherited: None,
            },
            ancestors,
            inherited,
        })
    }
    fn retain_module_sources(
        &mut self,
        plan: &crate::frontend::generated_headers::GeneratedHeaderPlan,
    ) -> Result<(), Diagnostic> {
        retain_module_demands(
            &mut self.module_sources,
            &mut self.module_source_bytes,
            plan,
        )
    }
    fn test_expansion_ancestry(
        &self,
    ) -> Result<crate::frontend::macro_origins::ExpansionAncestry, Diagnostic> {
        let mut ancestry = crate::frontend::macro_origins::ExpansionAncestry::default();
        if let Some((_, _, providers)) = &self.declarative {
            for provider in providers {
                ancestry.extend(provider.macro_index().expansion_ancestry())?;
            }
        }
        for fragment in self.fragments.values() {
            ancestry.extend(fragment.catalog.ancestry())?;
            for child in fragment.children.values() {
                ancestry.extend(child.catalog.ancestry())?;
            }
        }
        for module in self.staged_modules.values() {
            ancestry.extend(module.provider.macro_index().expansion_ancestry())?;
            for catalog in module.catalogs.values() {
                ancestry.extend(catalog.ancestry())?;
            }
        }
        // Item-only outputs can complete without allocating a macro lookup
        // catalog. Their checked captured-source invocation is an explicit root.
        for (identity, record) in &self.records {
            let origin = crate::syntax::token_wire::Key(identity.0);
            if !ancestry.contains(origin) && self.calls.contains_key(&record.key().invocation) {
                ancestry.register(origin, None)?;
            }
        }
        Ok(ancestry)
    }
    /// Enumerate final declarations only after this target's actual test seal is published.
    pub fn test_catalog(
        &self,
        db: &mut crate::db::memo::Database,
    ) -> Result<crate::engine::test_catalog::TestCatalog, Diagnostic> {
        use crate::{
            db::{
                dependency::{InputKey, InputValue},
                key::QueryKey,
                poll::QueryPoll,
                provider::SyntaxProvider,
                value::QueryValue,
            },
            syntax::outline::ItemOutline,
        };
        let _ = self.sealed_expansions()?;
        if self.key.domain
            != (SealDomain::TestCatalog {
                id: self.scope.crate_key,
            })
        {
            return Err(fault(
                "test enumeration requires the selected target's test catalog seal",
            ));
        }
        let seal = match db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(self.key.clone())) {
            QueryPoll::Ready(Ok(outcome)) => match outcome.value.as_ref() {
                QueryValue::DiscoverySeal(seal) if seal.key() == self.key => (**seal).clone(),
                _ => return Err(fault("test seal variant differs")),
            },
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => return Err(fault("test catalog publication is incomplete")),
        };
        let expansion_ancestry = self.test_expansion_ancestry()?;
        // Compiler Test has already registered a source function before later
        // attributes transform it. Bind its metadata to the exact output
        // ancestry and original value name; never synthesize an output item.
        let mut registrations = BTreeMap::new();
        for record in self.records.values() {
            if let Some(selection) = self.attribute_selections.get(&record.key().invocation)
                && let Some(item) = selection.consumed_test_registration()?
                && registrations
                    .insert(record.key().fingerprint(), item)
                    .is_some()
            {
                return Err(fault("compiler Test registration origin is duplicated"));
            }
        }
        for fragment in self.fragments.values() {
            insert_generated_test_registrations(&mut registrations, &fragment.catalog)?;
            for child in fragment.children.values() {
                insert_generated_test_registrations(&mut registrations, &child.catalog)?;
            }
        }
        for module in self.staged_modules.values() {
            for catalog in module.catalogs.values() {
                insert_generated_test_registrations(&mut registrations, catalog)?;
            }
        }
        let mut matched_registrations = BTreeSet::new();
        if let Some((_, _, providers)) = &self.declarative {
            for provider in providers
                .iter()
                .chain(self.staged_modules.values().map(|module| &module.provider))
            {
                for catalog in provider.completed_catalog_roots() {
                    insert_generated_test_registrations(&mut registrations, catalog)?;
                }
            }
        }
        let identities = self.graph.declared_value_identities(self.scope.crate_key)?;
        let mut source_definitions = BTreeMap::new();
        for item in self
            .sources
            .iter()
            .chain(
                self.staged_modules
                    .values()
                    .filter(|m| m.namespace_complete)
                    .map(|m| m.syntax.prepared_source()),
            )
            .flat_map(|source| source.outline().items.iter())
        {
            let identity = item.def_key.fingerprint();
            if !identities.contains(&identity) {
                continue;
            }
            if source_definitions.insert(identity, &item.def_key).is_some() {
                return Err(fault("test source definition identity is duplicated"));
            }
        }
        let mut ancestry_bytes = 0usize;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(identities.len())
            .map_err(|_| fault("test metadata allocation"))?;
        for identity in identities {
            let parse = match db.input(&InputKey::DefinitionSource(identity)).as_deref() {
                Some(InputValue::DefinitionSource(parse)) => parse.clone(),
                _ => return Err(fault("active test candidate source is absent")),
            };
            let (definition, ancestry) = if let Some(definition) = source_definitions.get(&identity)
            {
                ((*definition).clone(), Vec::new())
            } else {
                let unit = db
                    .input(&InputKey::GeneratedSyntax(parse.content))
                    .ok_or_else(|| fault("generated test candidate syntax absent"))?;
                let InputValue::GeneratedSyntax(unit) = unit.as_ref() else {
                    return Err(fault("generated test candidate syntax differs"));
                };
                if unit.seal != self.key {
                    return Err(fault("generated test candidate belongs to another catalog"));
                }
                let definition = unit
                    .definitions
                    .get(&identity)
                    .cloned()
                    .ok_or_else(|| fault("generated test candidate definition absent"))?;
                let expansion = unit.items.record().key().fingerprint();
                let ancestry =
                    expansion_ancestry.chain(crate::syntax::token_wire::Key(expansion.0))?;
                ancestry_bytes = ancestry
                    .len()
                    .checked_mul(std::mem::size_of::<Digest>())
                    .and_then(|n| ancestry_bytes.checked_add(n))
                    .ok_or_else(|| fault("test ancestry metadata limit"))?;
                (definition, ancestry)
            };
            if definition.item_kind != crate::source::manifest::ItemKind::Function {
                continue;
            }
            let header = match db.input(&InputKey::DefinitionHeader(identity)).as_deref() {
                Some(InputValue::DefinitionHeader(header)) => header.clone(),
                _ => return Err(fault("test candidate header absent")),
            };
            let context = match db.input(&InputKey::DeclarationContext(identity)).as_deref() {
                Some(InputValue::DeclarationContext(context)) => context.clone(),
                _ => return Err(fault("test candidate context absent")),
            };
            let body = match db.input(&InputKey::DefinitionBody(identity)).as_deref() {
                Some(InputValue::DefinitionBody(body)) => Some(body.clone()),
                None => None,
                _ => return Err(fault("test candidate body metadata differs")),
            };
            let mut registered = None;
            for origin in &ancestry {
                if let Some(item) = registrations.get(origin)
                    && item.def_key.declared_name == definition.declared_name
                    && item.scope.crate_key == context.scope.crate_key
                    && item.scope.module_path == context.scope.module_path
                    && item.scope.owner == context.scope.owner
                {
                    if registered.is_some() || !matched_registrations.insert(*origin) {
                        return Err(fault("compiler Test registration target is ambiguous"));
                    }
                    registered = Some(&item.header);
                }
            }
            entries.push(crate::engine::test_catalog::TestDeclaration {
                item: ItemOutline {
                    def_key: definition,
                    scope: context.scope,
                    header,
                    body,
                    constructor_fields: vec![],
                    variants: vec![],
                    declaration_span: context.range,
                    header_span: context.range,
                },
                identity,
                ancestry,
                registration: registered,
            });
        }
        if matched_registrations.len() != registrations.len() {
            return Err(Diagnostic::new("LRR-E-TEST-DISCOVERY", "a function registered by compiler Test was removed or renamed by a later attribute".into()));
        }
        crate::engine::test_catalog::TestCatalog::from_declarations(
            entries,
            seal,
            db.source_revision(),
            match self.completed_source_environment(db) {
                QueryPoll::Ready(Ok(environment)) => environment,
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(_) => {
                    return Err(fault("test source environment is not published"));
                }
            },
        )
    }
    pub(crate) fn completed_test_sources(&self) -> Result<Vec<PreparedSourceView>, Diagnostic> {
        let _ = self.sealed_expansions()?;
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(
                self.sources
                    .len()
                    .checked_add(self.staged_modules.len())
                    .ok_or_else(|| fault("test source count overflow"))?,
            )
            .map_err(|_| fault("test source allocation"))?;
        sources.extend(self.sources.iter().map(PreparedSourceView::share));
        sources.extend(
            self.staged_modules
                .values()
                .filter(|m| m.namespace_complete)
                .map(|m| m.syntax.prepared_source().share()),
        );
        Ok(sources)
    }
    /// Extend diagnostic capture only from admitted, revalidated syntax includes.
    /// No source read, parse, expansion, publication or worker effect occurs here.
    pub(crate) fn diagnostic_snapshot(
        &self,
        db: &mut crate::db::memo::Database,
        snapshot: &std::sync::Arc<crate::source::snapshot::Snapshot>,
    ) -> Result<std::sync::Arc<crate::source::snapshot::Snapshot>, Diagnostic> {
        use crate::{db::poll::QueryPoll, source::snapshot::Snapshot};
        if self.included_views.is_empty() {
            return Ok(snapshot.clone());
        }
        let invalid = || {
            Diagnostic::new(
                "LRR-E-INTERNAL-SOURCE-MAP",
                "included diagnostic capture differs".into(),
            )
        };
        let mut files = snapshot.files().clone();
        let mut retained_bytes = 0usize;
        for view in self.included_views.values() {
            if view.location().unit != snapshot.project().selected_crate {
                return Err(invalid());
            }
            match view.validate(db) {
                QueryPoll::Ready(result) => result?,
                QueryPoll::NeedInput(_) => return Err(invalid()),
            }
            if let Some(old) = files.get(&view.location().path) {
                if old.content() != view.blob().content() || old.bytes() != view.blob().bytes() {
                    return Err(invalid());
                }
                continue;
            }
            retained_bytes = retained_bytes
                .checked_add(view.location().path.as_str().len())
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-METADATA-LIMIT",
                        "included diagnostic capture metadata limit".into(),
                    )
                })?;
            files.insert(view.location().path.clone(), view.blob().clone());
        }
        Ok(std::sync::Arc::new(Snapshot::new(
            snapshot.project().clone(),
            files,
            snapshot.cfg_by_crate().clone(),
            snapshot.compile_env().clone(),
            Some(snapshot.id()),
        )?))
    }
    pub fn key(&self) -> &crate::db::seals::SealKey {
        &self.key
    }
    pub(crate) fn entry_function(
        &self,
        db: &mut crate::db::memo::Database,
        name: &str,
    ) -> crate::db::poll::QueryPoll<crate::frontend::callables::CallSelection> {
        use crate::db::{
            key::{BucketKey, QueryKey, SealedNameKey},
            poll::QueryPoll,
            provider::SyntaxProvider,
            value::QueryValue,
        };
        if self.namespace != Namespace::Value {
            return QueryPoll::Ready(Err(fault(
                "entry selection requires a value namespace seal",
            )));
        }
        if let Err(error) = self.sealed_expansions() {
            return QueryPoll::Ready(Err(error));
        }
        let key = QueryKey::SealedName(Box::new(SealedNameKey {
            seal: self.key.clone(),
            bucket: BucketKey {
                scope: self.scope.clone(),
                namespace: Namespace::Value,
                name: name.into(),
            },
        }));
        let found = match db.fetch(&SyntaxProvider, key) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::SealedName { lookup, .. } = found.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("entry namespace result differs")));
        };
        let mut sources = Vec::new();
        if sources.try_reserve_exact(self.sources.len()).is_err() {
            return QueryPoll::Ready(Err(fault("entry source catalogue allocation")));
        }
        sources.extend(self.sources.iter());
        if sources
            .try_reserve_exact(self.staged_modules.len())
            .is_err()
        {
            return QueryPoll::Ready(Err(fault("module entry source allocation")));
        }
        sources.extend(
            self.staged_modules
                .values()
                .filter(|module| module.namespace_complete)
                .map(|module| module.syntax.prepared_source()),
        );
        let resolver = match crate::frontend::callables::CallResolver::new(&self.graph, &sources) {
            Ok(resolver) => resolver,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        resolver.select(db, lookup.clone())
    }
    fn scope_edition(&self) -> Result<u16, Diagnostic> {
        self.sources
            .iter()
            .filter(|source| source.outline().scopes.contains_key(&self.scope))
            .max_by_key(|source| source.outline().root_scope.module_path.len())
            .map(|source| source.outline().profile.edition)
            .ok_or_else(|| fault("certified scope captured owner absent"))
    }
    /// Copy already bound producer facts before explicit server execution.
    /// This grants no publication or execution authority and never reparses.
    pub(crate) fn attribute_request_diagnostic_origin(
        &self,
        request: &crate::service::function_macro_requests::PreparedFunctionRequest,
    ) -> Result<Option<crate::syntax::origin::MacroOrigin>, Diagnostic> {
        let Some(selection) = self.attribute_selections.get(&request.key().invocation) else {
            return Ok(None);
        };
        if !selection.matches_request(request)? {
            return Err(fault("attribute failure origin request differs"));
        }
        selection.diagnostic_origin(request.key()).map(Some)
    }
    /// Read origin facts only after the execution caller validates the pending
    /// request. Original source calls keep their actual syntax range; generated
    /// tokens remain at their captured anchor, never at invented original offsets.
    pub(crate) fn source_request_diagnostic_origin(
        &self,
        request: &crate::service::function_macro_requests::PreparedFunctionRequest,
    ) -> Result<Option<crate::syntax::origin::MacroOrigin>, Diagnostic> {
        if let Some(origin) = self.attribute_request_diagnostic_origin(request)? {
            return Ok(Some(origin));
        }
        let Some(definition) = self.procedural_selections.get(&request.key().invocation) else {
            return Ok(None);
        };
        let span = request
            .request()
            .input
            .spans()
            .iter()
            .find(|span| span.key() == request.request().call_site)
            .ok_or_else(|| fault("source function failure origin site absent"))?;
        let mut path = String::new();
        path.try_reserve_exact(span.source.path.len())
            .map_err(|_| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "source function origin path allocation".into(),
                )
            })?;
        path.push_str(&span.source.path);
        let anchor = crate::syntax::spans::SourceRange::checked(span.start, span.end)?;
        let mut range = anchor;
        for source in &self.sources {
            let selected = source.selection();
            if selected.outline.crate_key.0 == span.source.unit.0
                && selected.outline.parse.content.0 == span.source.content.0
                && selected.path.as_str() == span.source.path
                && let Some(call) = source.macro_call_for_opening_at(anchor)?
            {
                range = call.range();
                break;
            }
        }
        let invocation = crate::syntax::origin::SourceOrigin {
            source: crate::syntax::token_wire::SourceIdentity {
                unit: crate::syntax::token_wire::Key(span.source.unit.0),
                content: crate::syntax::token_wire::Key(span.source.content.0),
                path,
            },
            range,
        };
        let mut selected = None;
        for catalog in self.procedural_catalogs()? {
            if catalog.consumer() != request.source().outline.crate_key
                || catalog.environment() != request.key().consumer_compile_env
                || Some(catalog.epoch()) != request.key().procedural_epoch
                || catalog
                    .select(definition, super::identity::ProceduralKind::Function)?
                    .is_none()
            {
                continue;
            }
            if selected.is_some() {
                return Err(fault("source function origin catalog ambiguous"));
            }
            selected = Some(catalog);
        }
        selected
            .ok_or_else(|| fault("source function origin catalog absent"))?
            .source_failure_origin(definition, request.key(), invocation)
            .map(Some)
    }
    pub(crate) fn require_procedural_request(
        &self,
        request: &crate::service::function_macro_requests::PreparedFunctionRequest,
    ) -> Result<(), Diagnostic> {
        let pending = if let Some((_, producer, selected)) = &self.pending_module {
            let source = self
                .staged_modules
                .get(producer)
                .ok_or_else(|| fault("pending procedural module source absent"))?
                .syntax
                .prepared_source()
                .selection();
            Some((selected, source))
        } else if let Some(ticket) = &self.pending_nested {
            let ordinal = self
                .nested_jobs
                .get(&ticket.job().identity)
                .ok_or_else(|| fault("cold procedural source owner absent"))?;
            self.nested_fragments
                .get(&ticket.job().identity)
                .and_then(|fragment| fragment.pending_request())
                .or_else(|| {
                    self.argument_discoveries
                        .get(&ticket.job().identity)
                        .and_then(|arguments| arguments.pending_request())
                })
                .map(|selected| (selected, self.sources[*ordinal].selection()))
        } else {
            self.pending
                .as_ref()
                .map(|(_, ordinal, selected)| (selected, self.sources[*ordinal].selection()))
        };
        if self.failed.is_some()
            || self.seal.is_some()
            || pending.is_none_or(|(selected, source)| {
                selected.procedural_request().is_none()
                    || selected.key() != request.key()
                    || source != request.source()
            })
        {
            return Err(fault(
                "macro execution requires the exact pending procedural discovery request",
            ));
        }
        Ok(())
    }
    pub(crate) fn require_generated_procedural_request(
        &self,
        request: &super::generated_call_request::GeneratedCallRequest,
    ) -> Result<(), Diagnostic> {
        if self.failed.is_some()
            || self.seal.is_some()
            || request.procedural_request().is_none()
            || self.pending_generated.as_ref().is_none_or(|(_, pending)| {
                pending.key() != request.key()
                    || pending.procedural_catalog() != request.procedural_catalog()
            })
        {
            return Err(fault(
                "macro execution requires the exact pending generated procedural discovery request",
            ));
        }
        Ok(())
    }
    pub(crate) fn generated_request_diagnostic_origin(
        &self,
        request: &super::generated_call_request::GeneratedCallRequest,
    ) -> Result<Option<crate::syntax::origin::MacroOrigin>, Diagnostic> {
        self.require_generated_procedural_request(request)?;
        let pending = &self
            .pending_generated
            .as_ref()
            .ok_or_else(|| fault("generated diagnostic pending request absent"))?
            .1;
        let origin = pending.diagnostic_origin();
        if origin.as_ref().is_some_and(|origin| {
            origin.invocation_identity != request.key().invocation
        }) {
            return Err(fault("generated diagnostic origin differs"));
        }
        Ok(origin.map(|origin| origin.as_ref().clone()))
    }
    pub(crate) fn require_run_budget(
        &self,
        budget: &super::budget::SharedExpansionBudget,
    ) -> Result<(), Diagnostic> {
        if !self.expansion_budget.shares_ledger(budget) {
            return Err(fault(
                "procedural discovery does not share the macro run budget ledger",
            ));
        }
        Ok(())
    }
    pub(crate) fn publish_macro_verified(
        &mut self,
        db: &mut crate::db::memo::Database,
        certificate: &crate::service::run::VerifiedMacroRunInputs,
    ) -> Result<(), Diagnostic> {
        let catalogs = self.procedural_catalogs()?;
        let mut descriptors = Vec::new();
        let ordinary = self.sealed_expansions()?;
        let nested = self.sealed_declaration_expansions()?;
        let count = ordinary
            .len()
            .checked_add(nested.len())
            .ok_or_else(|| fault("discovery certification record count limit"))?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(count)
            .map_err(|_| fault("discovery certification record allocation"))?;
        records.extend(ordinary.cloned());
        records.extend(nested);
        records.sort_by_key(|record| record.key().fingerprint());
        if records.windows(2).any(|rows| {
            rows[0].key() == rows[1].key()
                && rows[0].descriptor().fingerprint() != rows[1].descriptor().fingerprint()
        }) {
            return Err(fault("discovery certification record conflict"));
        }
        records.dedup_by_key(|record| record.key().fingerprint());
        let mut bytes = 0usize;
        for record in &records {
            bytes = bytes
                .checked_add(record.descriptor().estimated_bytes())
                .ok_or_else(|| fault("discovery certification descriptor metadata limit"))?;
        }
        descriptors
            .try_reserve_exact(records.len())
            .map_err(|_| fault("discovery certification allocation"))?;
        descriptors.extend(records.iter().map(|record| record.descriptor()));
        certificate.validate_discovery(db, &self.key, &catalogs, &descriptors)?;
        self.native_publication_certified = true;
        let result = self.publish(db);
        self.native_publication_certified = false;
        if result.is_ok() {
            self.certified_publication = true;
        }
        result
    }
    /// Re-publish this immutable sealed discovery, e.g. to renew module leases
    /// at run entry. A procedural discovery must already have been published
    /// under its macro-run certificate; direct first publication still refuses.
    pub(crate) fn republish(&mut self, db: &mut crate::db::memo::Database) -> Result<(), Diagnostic> {
        if !self.certified_publication {
            return self.publish(db);
        }
        self.native_publication_certified = true;
        let result = self.publish(db);
        self.native_publication_certified = false;
        result
    }
    fn procedural_catalogs(
        &self,
    ) -> Result<
        Vec<std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>>,
        Diagnostic,
    > {
        let mut catalogs = BTreeMap::new();
        let mut catalog_bytes = 0usize;
        if let Some((_, _, providers)) = &self.declarative {
            for catalog in providers.iter().filter_map(|p| p.procedural.as_ref()) {
                if !catalogs.contains_key(&catalog.fingerprint()) {
                    catalog_bytes = catalog_bytes
                        .checked_add(catalog.metadata_bytes()?)
                        .ok_or_else(|| fault("discovery procedural catalog metadata limit"))?;
                    catalogs.insert(catalog.fingerprint(), catalog.clone());
                }
            }
        }
        Ok(catalogs.into_values().collect())
    }
    /// Append the complete input set as one derived revision, never during a provider.
    pub fn publish(&self, db: &mut crate::db::memo::Database) -> Result<(), Diagnostic> {
        db.require_coordinator()?;
        if !self.native_publication_certified
            && (self
                .declarative
                .as_ref()
                .is_some_and(|(_, _, providers)| providers.iter().any(|p| p.procedural.is_some()))
                || self
                    .records
                    .values()
                    .any(|record| record.key().procedural_epoch.is_some()))
        {
            return Err(fault(
                "procedural discovery publication requires final macro run input verification",
            ));
        }
        self.expansion_budget.check_deadline()?;
        let records = self.sealed_expansions()?;
        let seal = self
            .seal
            .as_ref()
            .ok_or_else(|| fault("complete discovery certificate absent"))?;
        if seal.key() != self.key {
            return Err(fault("discovery certificate context differs"));
        }
        if let Some((_, _, providers)) = &self.declarative {
            super::source_declarative::SourceDeclarative::revalidate_captured_role_parents(
                providers,
            )?;
        }
        let mut macro_export_modules = Vec::new();
        macro_export_modules
            .try_reserve_exact(self.macro_export_modules.len())
            .map_err(|_| fault("macro-export module publication allocation"))?;
        macro_export_modules.extend(self.macro_export_modules.values().cloned());
        for module in &macro_export_modules {
            module.revalidate(db)?;
        }
        let module_handle_bytes = macro_export_modules
            .capacity()
            .checked_mul(std::mem::size_of::<
                std::sync::Arc<crate::source::macro_export_module::MacroExportModuleAdoption>,
            >())
            .ok_or_else(|| fault("macro-export module publication handle limit"))?;
        let mut replay_sources = Vec::new();
        for (ordinal, source) in self.sources.iter().enumerate() {
            if db
                .macro_export_source_observation(source.selection(), self.key.unit_graph)?
                .is_none()
            {
                continue;
            }
            let plan = self
                .original_annotation_plans
                .get(&ordinal)
                .ok_or_else(|| {
                    fault("macro-export replay requires its actual original source plan")
                })?;
            let replay =
                crate::source::compiler_replay::CompilerReplaySource::prepare(source, plan.scan())?;
            if replay_sources.is_empty() {
                let capacity = self.sources.len().min(256);
                capacity
                    .checked_mul(replay.exclusive_metadata_bytes())
                    .ok_or_else(|| fault("macro-export replay temporary metadata limit"))?;
                replay_sources
                    .try_reserve_exact(capacity)
                    .map_err(|_| fault("macro-export replay temporary allocation"))?;
            }
            replay_sources.push(replay);
        }
        let replay_bytes = replay_sources
            .capacity()
            .checked_mul(std::mem::size_of::<
                crate::source::compiler_replay::CompilerReplaySource<'_>,
            >())
            .ok_or_else(|| fault("macro-export replay temporary metadata overflow"))?;
        let scope = if !matches!(&self.key.domain, SealDomain::Scope { .. })
            || (self.namespace == Namespace::Macro && self.declarative.is_none())
        {
            None
        } else {
            Some(
                self.graph
                    .certified_scope(&self.scope, self.namespace, self.scope_edition()?)?,
            )
        };
        let mut bytes = seal
            .metadata_bytes()?
            .checked_add(replay_bytes)
            .and_then(|bytes| bytes.checked_add(module_handle_bytes))
            .ok_or_else(|| fault("macro-export replay publication metadata overflow"))?;
        if let Some(scope) = &scope {
            bytes = bytes
                .checked_add(scope.metadata_bytes()?)
                .ok_or_else(|| fault("namespace publication metadata overflow"))?;
        }
        for source in &self.sources {
            bytes = bytes
                .checked_add(source.selection().metadata_bytes()?)
                .ok_or_else(|| fault("discovery source metadata overflow"))?;
        }
        for record in self.records.values() {
            bytes = bytes
                .checked_add(record.descriptor().estimated_bytes())
                .ok_or_else(|| fault("discovery descriptor metadata overflow"))?;
        }
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(self.sources.len())
            .map_err(|_| fault("discovery source publication allocation"))?;
        sources.extend(self.sources.iter().map(|s| s.selection().clone()));
        let mut modules = Vec::new();
        modules
            .try_reserve_exact(self.staged_modules.len())
            .map_err(|_| fault("module publication projection allocation"))?;
        let mut ordinary_modules = Vec::new();
        ordinary_modules
            .try_reserve_exact(self.staged_modules.len())
            .map_err(|_| fault("ordinary module publication projection allocation"))?;
        sources
            .try_reserve_exact(self.staged_modules.len())
            .map_err(|_| fault("module publication source allocation"))?;
        for module in self.staged_modules.values() {
            if !module.namespace_complete {
                return Err(fault("module publication source namespace incomplete"));
            }
            sources.push(module.syntax.prepared_source().selection().clone());
            match &module.syntax {
                StagedModuleSyntax::Generated(syntax) => modules.push(syntax.clone()),
                StagedModuleSyntax::Ordinary { syntax, .. } => {
                    ordinary_modules.push(syntax.clone())
                }
            }
        }
        let mut descriptors = Vec::new();
        descriptors
            .try_reserve_exact(records.len())
            .map_err(|_| fault("discovery descriptor publication allocation"))?;
        let mut outputs = Vec::new();
        outputs
            .try_reserve_exact(records.len())
            .map_err(|_| fault("discovery record publication allocation"))?;
        for record in records {
            descriptors.push(record.descriptor());
            outputs.push(record.clone());
        }
        let item_output_count = outputs.len();
        for record in self.sealed_declaration_expansions()? {
            descriptors.push(record.descriptor());
            outputs.push(record);
        }
        let mut derived: std::collections::BTreeMap<
            crate::db::dependency::InputKey,
            crate::db::dependency::InputValue,
        > = std::collections::BTreeMap::new();
        let mut derived_bytes = 0usize;
        for inputs in modules
            .iter()
            .map(|module| module.inputs())
            .chain(ordinary_modules.iter().map(|module| module.inputs()))
        {
            for (key, value) in inputs {
                if let Some(old) = derived.get(key) {
                    if old.fingerprint() != value.fingerprint() {
                        return Err(fault("module derived input identity conflict"));
                    }
                    continue;
                }
                derived_bytes = derived_bytes
                    .checked_add(128)
                    .and_then(|n| n.checked_add(value.estimated_bytes().ok()?))
                    .ok_or_else(|| fault("module derived publication metadata limit"))?;
                derived.insert(key.clone(), value.clone());
            }
        }
        let mut generated_units = Vec::new();
        for record in outputs.iter().take(item_output_count) {
            let (ordinal, owner) = self
                .generated_owners
                .get(&record.key().fingerprint())
                .ok_or_else(|| fault("generated query owner metadata absent"))?;
            let (unit, mut rows) = super::generated_syntax::GeneratedSyntaxInput::prepare(
                record.clone(),
                owner.clone(),
                self.sources[*ordinal].captured_macro_cfg()?.clone(),
                self.key.clone(),
                self.attribute_selections
                    .get(&record.key().invocation)
                    .map(|selection| {
                        selection
                            .diagnostic_origin(record.key())
                            .map(std::sync::Arc::new)
                    })
                    .transpose()?
                    .or_else(|| {
                        self.generated_procedural_origins
                            .get(&record.key().fingerprint())
                            .cloned()
                    })
                    .or_else(|| {
                        self.included_views
                            .get(&record.key().fingerprint())
                            .map(|view| view.diagnostic_origin().clone())
                    }),
                self.included_views
                    .get(&record.key().fingerprint())
                    .cloned(),
            )?;
            generated_units.push(std::sync::Arc::clone(&unit));
            rows.insert(
                crate::db::dependency::InputKey::GeneratedSyntax(unit.key.content),
                crate::db::dependency::InputValue::GeneratedSyntax(unit),
            );
            for (key, value) in rows {
                derived_bytes = derived_bytes
                    .checked_add(128)
                    .and_then(|n| value.estimated_bytes().ok().and_then(|m| n.checked_add(m)))
                    .ok_or(Diagnostic::new(
                        "LRR-E-METADATA-LIMIT",
                        "generated query publication metadata limit".into(),
                    ))?;
                if derived.insert(key, value).is_some() {
                    return Err(fault("duplicate generated query input identity"));
                }
            }
        }
        if let Some(environment) = &self.completed_source_environment {
            environment.admit_generated_units(generated_units)?;
        }
        let type_scopes = self.type_scope_receipts(db, &sources, &descriptors, &derived)?;
        let value_scopes = self.value_scope_receipts(db, &sources, &descriptors, &derived)?;
        let catalogs = self.procedural_catalogs()?;
        let mut value_binding_preludes = Vec::new();
        if self.namespace == Namespace::Value
            && let Some((_, _, providers)) = &self.declarative
        {
            let mut bytes = 0usize;
            for provider in providers {
                let source = provider.module_source();
                if !source.selection().outline.module_path.is_empty()
                    || source.selection().outline.crate_key != self.scope.crate_key
                    || !self
                        .scope
                        .module_path
                        .starts_with(&source.selection().outline.module_path)
                {
                    continue;
                }
                let Some(catalog) = provider.compiler.as_ref() else {
                    continue;
                };
                if catalog
                    .canonical_standalone_member(&source.selection().outline.parse.profile)?
                    .is_none()
                {
                    continue;
                }
                let Some(proof) =
                    crate::facade::value_binding_prelude::ValueBindingPrelude::prepare(
                        source,
                        catalog,
                        &self.scope,
                    )?
                else {
                    continue;
                };
                bytes = bytes
                    .checked_add(proof.metadata_bytes()?)
                    .and_then(|n| n.checked_add(128))
                    .ok_or_else(|| fault("Value binding prelude publication metadata limit"))?;
                value_binding_preludes
                    .try_reserve(1)
                    .map_err(|_| fault("Value binding prelude publication allocation"))?;
                value_binding_preludes.push(proof);
            }
        }
        db.publish_discovery(
            crate::db::seals::DiscoveryInput {
                type_scopes,
                value_scopes,
                value_binding_preludes,
                seal: seal.clone(),
                declaration_inventory: std::sync::Arc::new(self.impl_header_inventory()?),
                source_dependencies: self.dependencies.fingerprint(),
                compiler_inputs: self.compiler_inputs.clone(),
                sources,
                records: descriptors,
                procedural_catalogs: catalogs,
                helper_declarations: self.helper_declarations.values().cloned().collect(),
                argument_summaries: self
                    .nested_argument_summaries
                    .iter()
                    .map(|(identity, (source, arguments))| {
                        (*identity, source.clone(), arguments.clone())
                    })
                    .collect(),
                argument_indexes: self.completed_argument_indexes.clone(),
                argument_fragments: self.completed_argument_fragments.clone(),
                source_replacements: self.source_replacements.values().cloned().collect(),
                declaration_fragments: {
                    let mut fragments = Vec::new();
                    fragments
                        .try_reserve_exact(self.nested_completed.len())
                        .map_err(|_| fault("declaration publication allocation"))?;
                    fragments.extend(self.nested_completed.values().cloned());
                    fragments.sort_by_key(|fragment| {
                        fragment.index().output().record().key().invocation
                    });
                    fragments
                },
                nested_inert_proofs: self.nested_inert_proofs.values().cloned().collect(),
                runtime_no_item_proofs: self
                    .nested_runtime_proofs
                    .iter()
                    .map(|(id, proof)| (*id, proof.clone()))
                    .collect(),
                scope,
            },
            &outputs,
            &modules,
            &ordinary_modules,
            &replay_sources,
            &macro_export_modules,
            derived,
        )
    }
    /// Immutable admitted inputs remain available while other producers are pending.
    pub fn expansion_record(
        &self,
        key: &super::identity::ExpansionKey,
    ) -> Option<std::sync::Arc<super::record::ExpansionRecord>> {
        self.records
            .get(&key.fingerprint())
            .filter(|record| record.key() == key)
            .cloned()
    }
    /// Deterministic record inputs for publication only after actual completion.
    pub fn sealed_expansions(
        &self,
    ) -> Result<
        impl ExactSizeIterator<Item = &std::sync::Arc<super::record::ExpansionRecord>>,
        Diagnostic,
    > {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        let seal = self
            .seal
            .as_ref()
            .ok_or_else(|| fault("expansion input publication requires complete discovery"))?;
        if self.records.iter().any(|(key, record)| {
            seal.input_dependencies()
                .binary_search(&(*key, record.descriptor().fingerprint()))
                .is_err()
        }) {
            return Err(fault(
                "discovery certificate does not bind an admitted record",
            ));
        }
        Ok(self.records.values())
    }
    pub fn defer_derive_declaration(
        &mut self,
        request: &crate::frontend::generated_derive_exports::DeriveDeclarationRequest,
    ) -> Result<(), Diagnostic> {
        let (ticket, pending) = self
            .pending_declaration
            .as_ref()
            .ok_or_else(|| fault("no helper declaration pending"))?;
        if pending.fingerprint() != request.fingerprint() {
            return Err(fault("deferred helper demand differs"));
        }
        self.work.block(ticket, vec![request.demand()])?;
        self.pending_declaration = None;
        Ok(())
    }
    pub fn adopt_derive_declaration(
        &mut self,
        db: &mut crate::db::memo::Database,
        request: &crate::frontend::generated_derive_exports::DeriveDeclarationRequest,
        receipt: std::sync::Arc<crate::expand::derive_receipt::PreparedDeriveDeclaration>,
    ) -> Result<(), Diagnostic> {
        db.require_coordinator()?;
        let (ticket, pending) = self
            .pending_declaration
            .as_ref()
            .ok_or_else(|| fault("no helper declaration pending"))?;
        if self.seal.is_some()
            || self.failed.is_some()
            || pending.fingerprint() != request.fingerprint()
        {
            return Err(fault("helper declaration differs from active producer"));
        }
        receipt.validate_request(request)?;
        use crate::db::dependency::{InputKey, InputValue};
        if !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),Some(InputValue::ExpansionEpoch(epoch)) if *epoch==self.epoch)
        {
            return Err(fault("helper receipt database epoch or parent differs"));
        }
        request.origin.validate_database(db)?;
        for (key, value) in db.compiler_input_stamps()? {
            if self
                .compiler_inputs
                .get(&key)
                .is_some_and(|old| old != &value)
            {
                return Err(fault(
                    "helper receipt source input changed during discovery",
                ));
            }
        }
        let retained = self
            .helper_receipt_bytes
            .checked_add(receipt.estimated_bytes()?)
            .and_then(|n| n.checked_add(request.metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(192))
            .ok_or_else(|| fault("discovery helper receipt metadata limit"))?;
        if let super::derive_origin::DeriveDeclarationOrigin::Source {
            selection,
            producer,
            ..
        } = &request.origin
        {
            let (ordinal, _) = self
                .calls
                .get(producer)
                .ok_or_else(|| fault("source helper producer absent"))?;
            if ticket.job().identity != *producer || self.sources[*ordinal].selection() != selection
            {
                return Err(fault("source helper receipt belongs to another producer"));
            }
            self.work
                .preflight_resume_input(ticket, request.demand(), receipt.fingerprint())?;
            self.work
                .resume_input(ticket, request.demand(), receipt.fingerprint())?;
            self.helper_receipt_bytes = retained;
            self.helper_declarations
                .insert(request.demand(), (pending.clone(), receipt));
            self.pending_declaration = None;
            return Ok(());
        }
        let fragment = self
            .fragments
            .get_mut(&ticket.job().identity)
            .ok_or_else(|| fault("helper declaration fragment absent"))?;
        let catalog = match fragment.active_child {
            Some(id) => {
                &mut fragment
                    .children
                    .get_mut(&id)
                    .ok_or_else(|| fault("helper declaration child absent"))?
                    .catalog
            }
            None => &mut fragment.catalog,
        };
        catalog.preflight_helper_receipt(request, &receipt)?;
        self.work
            .preflight_resume_input(ticket, request.demand(), receipt.fingerprint())?;
        let fingerprint = receipt.fingerprint();
        catalog.adopt_helper_receipt(request, receipt.clone())?;
        self.work
            .resume_input(ticket, request.demand(), fingerprint)?;
        self.helper_receipt_bytes = retained;
        self.helper_declarations
            .insert(request.demand(), (pending.clone(), receipt));
        self.pending_declaration = None;
        Ok(())
    }
    fn helper_catalogue_counts(&self) -> Result<BTreeMap<Digest, (usize, usize)>, Diagnostic> {
        let mut counts = BTreeMap::new();
        for fragment in self.fragments.values() {
            fragment.catalog.collect_helper_counts(&mut counts, 0)?;
            for child in fragment.children.values() {
                child.catalog.collect_helper_counts(&mut counts, 0)?;
            }
        }
        if let Some((_, _, providers)) = &self.declarative {
            for provider in providers {
                provider.collect_helper_counts(&mut counts)?;
            }
        }
        for module in self.staged_modules.values() {
            module.provider.collect_helper_counts(&mut counts)?;
        }
        Ok(counts)
    }
    pub fn derive_helper_attribute_count(&self) -> Result<usize, Diagnostic> {
        self.helper_catalogue_counts()?
            .values()
            .try_fold(0usize, |n, (_, helpers)| {
                n.checked_add(*helpers)
                    .ok_or_else(|| fault("helper attribute count overflow"))
            })
    }
    pub fn derive_declaration_count(&self) -> Result<usize, Diagnostic> {
        self.helper_catalogue_counts()?
            .values()
            .try_fold(0usize, |n, (receipts, _)| {
                n.checked_add(*receipts)
                    .ok_or_else(|| fault("helper receipt count overflow"))
            })
    }

    /// Owned structural facts only: this cannot certify complete graph/facade impl coverage.
    pub fn impl_header_inventory(
        &self,
    ) -> Result<crate::frontend::impl_index::ImplHeaderIndex, Diagnostic> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        let mut scans = Vec::new();
        for index in self.nested_sources.values() {
            scans
                .try_reserve(1)
                .map_err(|_| fault("nested scan handle allocation"))?;
            scans.push(index.scan_handle().clone());
        }
        for (_, arguments) in self.nested_argument_summaries.values() {
            scans
                .try_reserve(arguments.len())
                .map_err(|_| fault("intrinsic argument scan allocation"))?;
            scans.extend(arguments.iter().map(|argument| argument.scan().clone()));
        }
        for fragment in &self.completed_argument_fragments {
            for scan in fragment.inventory().scan_handles() {
                scans.push(scan.clone());
            }
        }
        for output in self.nested_completed.values() {
            scans
                .try_reserve(output.inventory().scan_handles().len())
                .map_err(|_| fault("nested generated scan handle allocation"))?;
            scans.extend_from_slice(output.inventory().scan_handles());
        }
        let inventory = crate::frontend::impl_index::ImplHeaderIndex::from_scans(scans)?;
        let mut proofs = Vec::new();
        proofs
            .try_reserve_exact(self.source_replacements.len())
            .map_err(|_| fault("replacement inventory allocation"))?;
        proofs.extend(self.source_replacements.values().cloned());
        let mut masks = Vec::new();
        for fragment in self
            .nested_completed
            .values()
            .chain(self.completed_argument_fragments.iter())
        {
            let rows = fragment.generated_replacement_masks()?;
            masks
                .try_reserve(rows.len())
                .map_err(|_| fault("generated replacement masks allocation"))?;
            masks.extend(rows);
        }
        inventory
            .exclude_replaced_sources(&proofs)?
            .exclude_generated_replacements(&masks)
    }
    /// Exact non-item grammar records; ordinary item facets remain separate.
    pub fn sealed_declaration_expansions(
        &self,
    ) -> Result<Vec<std::sync::Arc<super::record::ExpansionRecord>>, Diagnostic> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        if self.seal.is_none() {
            return Err(fault("declaration records require completed source scope"));
        }
        let mut records = Vec::new();
        for output in self.nested_completed.values() {
            records
                .try_reserve(output.records().len())
                .map_err(|_| fault("nested record handle allocation"))?;
            records.extend_from_slice(output.records());
        }
        for fragment in &self.completed_argument_fragments {
            records.extend_from_slice(fragment.records());
        }
        for record in &records {
            if self
                .records
                .get(&record.key().fingerprint())
                .is_some_and(|actual| {
                    actual.descriptor().fingerprint() != record.descriptor().fingerprint()
                })
            {
                return Err(fault("item/declaration record identity conflict"));
            }
        }
        records.retain(|record| !self.records.contains_key(&record.key().fingerprint()));
        records.sort_by_key(|r| r.key().fingerprint());
        if records.windows(2).any(|pair| {
            pair[0].key() == pair[1].key()
                && pair[0].descriptor().fingerprint() != pair[1].descriptor().fingerprint()
        }) {
            return Err(fault("declaration record projection conflict"));
        }

        records.dedup_by(|a, b| {
            a.key() == b.key() && a.descriptor().fingerprint() == b.descriptor().fingerprint()
        });
        Ok(records)
    }
    fn poll_argument_declarations(
        &mut self,
        ticket: &JobTicket,
        ordinal: usize,
    ) -> Result<Option<SourceDiscoveryStep>, Diagnostic> {
        use super::argument_discovery::ArgumentDiscoveryStep;
        let identity = ticket.job().identity;
        let (env, engine, providers) = self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("argument provider absent"))?;
        let provider = providers
            .get(ordinal)
            .ok_or_else(|| fault("argument provider owner absent"))?;
        let namespaces = Self::install_macro_routes_with_modules(
            unique_graph(&mut self.graph)?,
            providers,
            &self.module_routes,
            &self.nested_sources,
            &self.nested_inert_proofs,
            &mut self.route_install,
        )?;
        let discovery = self
            .argument_discoveries
            .get_mut(&identity)
            .ok_or_else(|| fault("argument discovery absent"))?;
        match discovery.poll(
            provider,
            &super::declaration_fragment::DeclarationFragmentInput {
                invocation: identity,
                request: self.expansion_budget.request_identity(),
                graph: self.key.unit_graph,
                compile_env: *env,
                engine: *engine,
                epoch: self.epoch,
                cfg: provider.module_source().captured_macro_cfg()?.clone(),
            },
            &self.expansion_budget,
            &namespaces,
        )? {
            ArgumentDiscoveryStep::NeedAttributeSelection(selection) => {
                self.pending_attribute = Some((
                    ticket.clone(),
                    AttributeSourceOwner::Fragment(ordinal),
                    selection.clone(),
                ));
                self.pending_nested = None;
                Ok(Some(SourceDiscoveryStep::NeedAttributeSelection {
                    selection,
                }))
            }
            ArgumentDiscoveryStep::NeedProceduralSelection(selection) => {
                self.pending_procedural = Some((ticket.clone(), ordinal, selection.clone()));
                self.pending_nested = None;
                Ok(Some(SourceDiscoveryStep::NeedProceduralSelection {
                    selection,
                }))
            }
            ArgumentDiscoveryStep::NeedExpansion(request) => {
                Ok(Some(SourceDiscoveryStep::NeedExpansion { request }))
            }
            ArgumentDiscoveryStep::NeedInput(input) => {
                self.work.block(ticket, vec![input])?;
                self.pending_nested = None;
                Ok(None)
            }
            ArgumentDiscoveryStep::Complete => {
                let catalog = provider
                    .compiler
                    .as_ref()
                    .ok_or_else(|| fault("argument compiler absent"))?
                    .runtime_catalog()?
                    .ok_or_else(|| fault("argument runtime catalog absent"))?;
                let mut parent = (**self
                    .nested_sources
                    .get(&ordinal)
                    .ok_or_else(|| fault("argument source index absent"))?)
                .clone();
                parent.accept_completed_argument_inventory(
                    identity,
                    &discovery.indexes,
                    &catalog,
                )?;
                let mut update = DiscoveryUpdate::default();
                update
                    .dependencies
                    .try_reserve_exact(self.compiler_inputs.len())
                    .map_err(|_| fault("argument compiler dependency allocation"))?;
                update.dependencies.extend(
                    self.compiler_inputs
                        .iter()
                        .map(|(key, fingerprint)| (key.fingerprint(), *fingerprint)),
                );
                update.members.push(identity);
                let mut retained = self
                    .nested_bytes
                    .checked_add(parent.retained_metadata_bytes())
                    .ok_or_else(|| fault("argument metadata overflow"))?;
                for index in &discovery.indexes {
                    retained = retained
                        .checked_add(index.metadata_bytes())
                        .ok_or_else(|| fault("argument index metadata limit"))?;
                }
                for argument in &discovery.arguments {
                    retained = retained
                        .checked_add(argument.metadata_bytes())
                        .ok_or_else(|| fault("argument metadata limit"))?;
                    update.members.push(argument.fingerprint());
                    update
                        .members
                        .extend(argument.scan().impls().iter().map(|h| h.identity));
                }
                for fragment in &discovery.completed {
                    retained = retained
                        .checked_add(fragment.metadata_bytes())
                        .ok_or_else(|| fault("argument fragment metadata limit"))?;
                    update
                        .members
                        .extend(fragment.inventory().headers().iter().map(|h| h.identity));
                    for record in fragment.records() {
                        update.members.push(record.descriptor().fingerprint());
                        update.dependencies.push((
                            record.key().fingerprint(),
                            record.descriptor().fingerprint(),
                        ));
                    }
                }
                let mut encoded =
                    crate::core::digest::StableEncoder::new("lrr/v2/intrinsic-argument-set");
                encoded.sequence(&discovery.arguments, |e, a| e.record(a.as_ref()));
                update.dependencies.push((identity, encoded.finish()));
                self.completed_argument_indexes
                    .try_reserve(discovery.indexes.len())
                    .map_err(|_| fault("argument index allocation"))?;
                self.completed_argument_fragments
                    .try_reserve(discovery.completed.len())
                    .map_err(|_| fault("argument fragment allocation"))?;
                self.work.preflight_complete(ticket, &mut update)?;
                self.work.complete(ticket, update)?;
                let discovery = self
                    .argument_discoveries
                    .remove(&identity)
                    .ok_or_else(|| fault("argument discovery disappeared"))?;
                self.completed_argument_indexes.extend(discovery.indexes);
                self.completed_argument_fragments
                    .extend(discovery.completed);
                self.completed_argument_indexes
                    .sort_by_key(|index| index.argument().fingerprint());
                self.completed_argument_fragments
                    .sort_by_key(|fragment| fragment.index().output().record().key().fingerprint());
                self.nested_argument_summaries.insert(
                    identity,
                    (parent.scan_handle().clone(), discovery.arguments),
                );
                self.nested_sources
                    .insert(ordinal, std::sync::Arc::new(parent));
                self.nested_bytes = retained;
                self.pending_nested = None;
                Ok(None)
            }
        }
    }
    fn poll_item_declarations(
        &mut self,
        ticket: &JobTicket,
        ordinal: usize,
    ) -> Result<Option<SourceDiscoveryStep>, Diagnostic> {
        use super::declaration_fragment::{
            DeclarationFragment, DeclarationFragmentInput, DeclarationFragmentStep,
        };
        let identity = ticket.job().identity;
        let (compile_env, engine, providers) = self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("item declaration providers absent"))?;
        let provider = providers
            .get(ordinal)
            .ok_or_else(|| fault("item declaration provider owner absent"))?;
        let namespaces = Self::install_macro_routes_with_modules(
            unique_graph(&mut self.graph)?,
            providers,
            &self.module_routes,
            &self.nested_sources,
            &self.nested_inert_proofs,
            &mut self.route_install,
        )?;
        if !self.nested_fragments.contains_key(&identity) {
            let output = self
                .item_declaration_inputs
                .get(&identity)
                .ok_or_else(|| fault("item declaration output absent"))?
                .clone();
            if self
                .fragments
                .contains_key(&output.record().key().fingerprint())
            {
                self.work
                    .block(ticket, vec![output.record().key().fingerprint()])?;
                self.pending_nested = None;
                return Ok(None);
            }
            let owner = provider.completed_root_catalog(output.record().key().invocation)?;
            let parent = self
                .nested_sources
                .get(&ordinal)
                .ok_or_else(|| fault("item declaration parent absent"))?
                .clone();
            let fragment = DeclarationFragment::from_item_output(
                parent,
                DeclarationFragmentInput {
                    invocation: output.record().key().invocation,
                    request: self.expansion_budget.request_identity(),
                    graph: self.key.unit_graph,
                    compile_env: *compile_env,
                    engine: *engine,
                    epoch: self.epoch,
                    cfg: provider.module_source().captured_macro_cfg()?.clone(),
                },
                self.expansion_budget.clone(),
                output,
            )?;
            let fragment = if let Some(owner) = owner {
                fragment.with_completed_item_owner(owner)?
            } else {
                fragment
            };
            self.nested_fragments.insert(identity, fragment);
        }
        let fragment = self
            .nested_fragments
            .get_mut(&identity)
            .ok_or_else(|| fault("item declaration fragment absent"))?;
        let step = match fragment.poll_with_namespaces(provider, &namespaces) {
            Ok(step) => step,
            Err(error) => {
                self.failed = Some(error.clone());
                return Err(error);
            }
        };
        match step {
            DeclarationFragmentStep::NeedExpansion(request) => {
                Ok(Some(SourceDiscoveryStep::NeedExpansion { request }))
            }
            DeclarationFragmentStep::NeedAttributeSelection(selection) => {
                self.pending_attribute = Some((
                    ticket.clone(),
                    AttributeSourceOwner::Fragment(ordinal),
                    selection.clone(),
                ));
                self.pending_nested = None;
                Ok(Some(SourceDiscoveryStep::NeedAttributeSelection {
                    selection,
                }))
            }
            DeclarationFragmentStep::NeedProceduralSelection(selection) => {
                self.pending_procedural = Some((ticket.clone(), ordinal, selection.clone()));
                self.pending_nested = None;
                Ok(Some(SourceDiscoveryStep::NeedProceduralSelection {
                    selection,
                }))
            }
            DeclarationFragmentStep::NeedInput(input) => {
                self.work.block(ticket, vec![input])?;
                self.pending_nested = None;
                Ok(None)
            }
            DeclarationFragmentStep::Complete(output) => {
                let retained = self
                    .nested_bytes
                    .checked_add(output.metadata_bytes())
                    .ok_or_else(|| fault("completed item declaration metadata limit"))?;
                let mut update = DiscoveryUpdate::default();
                update
                    .members
                    .try_reserve_exact(
                        output
                            .inventory()
                            .headers()
                            .len()
                            .checked_add(output.records().len())
                            .and_then(|n| n.checked_add(1))
                            .ok_or_else(|| fault("item declaration members overflow"))?,
                    )
                    .map_err(|_| fault("item declaration members allocation"))?;
                update
                    .dependencies
                    .try_reserve_exact(
                        output
                            .records()
                            .len()
                            .checked_add(1)
                            .and_then(|n| n.checked_add(self.compiler_inputs.len()))
                            .ok_or_else(|| fault("item declaration dependencies overflow"))?,
                    )
                    .map_err(|_| fault("item declaration dependencies allocation"))?;
                update.members.push(identity);

                update.dependencies.push((
                    output.index().output().record().key().invocation,
                    output.index().output().fingerprint(),
                ));
                update.members.extend(
                    output
                        .inventory()
                        .headers()
                        .iter()
                        .map(|header| header.identity),
                );
                for record in output.records() {
                    update.members.push(record.descriptor().fingerprint());
                    update.dependencies.push((
                        record.key().fingerprint(),
                        record.descriptor().fingerprint(),
                    ));
                }
                update.dependencies.extend(
                    self.compiler_inputs
                        .iter()
                        .map(|(key, value)| (key.fingerprint(), *value)),
                );
                self.work.preflight_complete(ticket, &mut update)?;
                self.work.complete(ticket, update)?;
                self.nested_completed.insert(identity, output);
                self.nested_fragments.remove(&identity);
                self.nested_bytes = retained;
                self.pending_nested = None;
                Ok(None)
            }
        }
    }
    fn poll_nested_declarations(
        &mut self,
        ticket: &JobTicket,
    ) -> Result<Option<SourceDiscoveryStep>, Diagnostic> {
        use super::declaration_fragment::{
            DeclarationFragment, DeclarationFragmentInput, DeclarationFragmentStep,
        };
        let identity = ticket.job().identity;
        let ordinal = *self
            .nested_jobs
            .get(&identity)
            .ok_or_else(|| fault("nested producer owner absent"))?;
        if self.item_declaration_inputs.contains_key(&identity) {
            return self.poll_item_declarations(ticket, ordinal);
        }
        let Some((compile_env, engine, providers)) = &self.declarative else {
            self.work.block(ticket, vec![identity])?;
            self.pending_nested = None;
            return Ok(None);
        };
        let provider = providers
            .get(ordinal)
            .ok_or_else(|| fault("nested provider absent"))?;
        if self.original_prefix_jobs.contains(&identity)
            && original_inert::matched_attribute_facet(&self.sources[ordinal], provider)?.is_none()
        {
            self.work.block(ticket, vec![identity])?;
            self.pending_nested = None;
            return Ok(None);
        }
        if self.argument_discoveries.contains_key(&identity) {
            return self.poll_argument_declarations(ticket, ordinal);
        }
        if !self.nested_fragments.contains_key(&identity) {
            let parent = self
                .nested_sources
                .get(&ordinal)
                .ok_or_else(|| fault("nested source index absent"))?
                .clone();
            let producer = parent.producer(identity)?;
            if let Some(input) = provider.declaration_parent_frontier(producer)? {
                self.work.block(ticket, vec![input])?;
                self.pending_nested = None;
                return Ok(None);
            }
            let namespaces = Self::install_macro_routes_with_modules(
                unique_graph(&mut self.graph)?,
                providers,
                &self.module_routes,
                &self.nested_sources,
                &self.nested_inert_proofs,
                &mut self.route_install,
            )?;
            if producer.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            {
                if let Some(selection) = crate::service::source_attribute_requests::SourceAttributeSelection::select_cold(&self.sources[ordinal], provider, parent.scan_handle().clone(), identity, &namespaces)? {
                    let selection = std::sync::Arc::new(selection);
                    self.pending_attribute = Some((ticket.clone(), AttributeSourceOwner::Cold(ordinal), selection.clone()));
                    self.pending_nested = None;
                    return Ok(Some(SourceDiscoveryStep::NeedAttributeSelection { selection }));
                }
                if producer.unqualified_attribute.as_deref() == Some("test") {
                    let lookup = crate::service::source_attribute_requests::SourceAttributeSelection::cold_lookup(&self.sources[ordinal], parent.scan(), identity, &namespaces)?;
                    let binding = match lookup {
                        crate::frontend::macro_names::PathLookup::Absent => Some(None),
                        crate::frontend::macro_names::PathLookup::Defined(definition) => {
                            if !super::builtin_attributes::permits_builtin_test_marker(
                                &definition,
                                provider.procedural.as_deref(),
                                provider
                                    .compiler
                                    .as_ref()
                                    .and_then(|c| c.attribute_catalog()),
                            )? {
                                None
                            } else {
                                Some(Some(definition))
                            }
                        }
                        crate::frontend::macro_names::PathLookup::Ambiguous(_) => {
                            return Err(Diagnostic::new(
                                "LRR-E-MACRO-AMBIGUOUS",
                                "cold test attribute namespace is ambiguous".into(),
                            ));
                        }
                        crate::frontend::macro_names::PathLookup::Inaccessible(_) => {
                            return Err(Diagnostic::new(
                                "LRR-E-MACRO-INACCESSIBLE",
                                "cold test attribute namespace is inaccessible".into(),
                            ));
                        }
                        _ => None,
                    };
                    let retained = binding.is_some();
                    if let Some(binding) = binding {
                        // A pinned compiler Test target must be eligible before
                        // its annotation is retired as inert or a seal can form.
                        if let Err(error) = self.sources[ordinal]
                            .validate_compiler_test_target(parent.scan(), identity)
                        {
                            self.failed = Some(error.clone());
                            return Err(error);
                        }
                        if !self.cold_test_bindings.contains_key(&identity) {
                            self.nested_bytes = self
                                .nested_bytes
                                .checked_add(256)
                                .ok_or_else(|| {
                                    fault("cold compiler test binding metadata limit")
                                })?;
                        }
                        self.cold_test_bindings.insert(identity, (ordinal, binding));
                    }
                    if !retained {
                        self.work.block(ticket, vec![identity])?;
                        self.pending_nested = None;
                        return Ok(None);
                    }
                }
            }
            if let Some(mut proof) = super::nested_inert::NestedInertProof::select(
                parent.scan_handle().clone(),
                identity,
            )? {
                if super::builtin_attributes::inert_descriptor(proof.name()?)?.is_some() {
                    let lookup = crate::service::source_attribute_requests::SourceAttributeSelection::cold_lookup(
                        &self.sources[ordinal], parent.scan(), identity, &namespaces,
                    )?;
                    let Some(binding) = super::builtin_attributes::certify_inert_binding(
                        proof.name()?,
                        &lookup,
                        provider.procedural.as_deref(),
                        provider
                            .compiler
                            .as_ref()
                            .and_then(|c| c.attribute_catalog()),
                    )?
                    else {
                        let input = match lookup {
                            crate::frontend::macro_names::PathLookup::NeedInput(input) => input,
                            _ => identity,
                        };
                        self.work.block(ticket, vec![input])?;
                        self.pending_nested = None;
                        return Ok(None);
                    };
                    proof = proof.with_attribute_facet(binding);
                }
                let retained = self
                    .nested_bytes
                    .checked_add(proof.metadata_bytes()?)
                    .and_then(|n| n.checked_add(parent.retained_metadata_bytes()))
                    .ok_or_else(|| fault("nested inert attribute metadata limit"))?;
                let mut parent = (*parent).clone();
                parent.accept_inert_attribute(identity, proof.name()?)?;
                let fingerprint = proof.fingerprint();
                let mut update = DiscoveryUpdate {
                    members: vec![identity, fingerprint],
                    dependencies: vec![(identity, fingerprint)],
                    ..Default::default()
                };
                self.work.preflight_complete(ticket, &mut update)?;
                self.work.complete(ticket, update)?;
                self.nested_sources
                    .insert(ordinal, std::sync::Arc::new(parent));
                self.nested_inert_proofs.insert(identity, proof);
                self.nested_bytes = retained;
                self.pending_nested = None;
                return Ok(None);
            }
            let namespaces = Self::install_macro_routes_with_modules(
                unique_graph(&mut self.graph)?,
                providers,
                &self.module_routes,
                &self.nested_sources,
                &self.nested_inert_proofs,
                &mut self.route_install,
            )?;
            if let Some(definition) =
                provider.declaration_procedural_definition(&parent, identity, &namespaces)?
            {
                let catalog = provider
                    .procedural
                    .as_ref()
                    .ok_or_else(|| fault("cold procedural catalog absent"))?
                    .clone();
                let input = crate::service::function_macro_requests::FunctionInvocationInput::from_declaration_source(&self.sources[ordinal], parent.scan(), identity)?;
                let selection = std::sync::Arc::new(ProceduralSourceSelection {
                    module: None,
                    nested: true,
                    input,
                    definition,
                    catalog,
                });
                self.pending_procedural = Some((ticket.clone(), ordinal, selection.clone()));
                self.pending_nested = None;
                return Ok(Some(SourceDiscoveryStep::NeedProceduralSelection {
                    selection,
                }));
            }
            if let Some(proof) = provider.declaration_runtime_no_item_proof(
                &parent,
                identity,
                &namespaces,
                providers,
            )? {
                let catalog = provider
                    .compiler
                    .as_ref()
                    .ok_or_else(|| fault("runtime compiler facet absent"))?
                    .runtime_catalog()?
                    .ok_or_else(|| fault("runtime compiler definitions absent"))?;
                let mut parent = (*parent).clone();
                parent.accept_runtime_no_item_proof(identity, &proof, &catalog)?;
                let retained = self
                    .nested_bytes
                    .checked_add(
                        proof
                            .allocated_bytes()
                            .ok_or_else(|| fault("runtime proof size overflow"))?,
                    )
                    .and_then(|n| n.checked_add(parent.retained_metadata_bytes()))
                    .ok_or_else(|| fault("runtime proof metadata limit"))?;
                let fingerprint = proof.fingerprint();
                let mut update = DiscoveryUpdate {
                    members: vec![identity, fingerprint],
                    dependencies: vec![(identity, fingerprint)],
                    ..Default::default()
                };
                self.work.preflight_complete(ticket, &mut update)?;
                self.work.complete(ticket, update)?;
                self.nested_sources
                    .insert(ordinal, std::sync::Arc::new(parent));
                self.nested_runtime_proofs.insert(identity, proof);
                self.nested_bytes = retained;
                self.pending_nested = None;
                return Ok(None);
            }
            if let Some(arguments) =
                provider.declaration_runtime_arguments(&parent, identity, &namespaces, providers)?
            {
                let discovery =
                    super::argument_discovery::ArgumentDiscovery::new(arguments, parent)?;
                self.argument_discoveries.insert(identity, discovery);
                return self.poll_argument_declarations(ticket, ordinal);
            }
            let mut prepared_request = provider.declaration_compiler_request(
                parent.clone(),
                identity,
                &namespaces,
                *compile_env,
                *engine,
            )?;
            if prepared_request.is_none() {
                match provider.declaration_demand(&parent, identity, *compile_env, *engine)? {
                    super::source_declarative::SourceDeclarativeDemand::NeedInput(_) => {
                        match provider.declaration_import_request(
                            &parent,
                            identity,
                            &namespaces,
                            providers,
                            *compile_env,
                            *engine,
                        )? {
                            super::source_declarative::SourceDeclarativeDemand::Ready(request) => {
                                prepared_request = Some(*request);
                            }
                            super::source_declarative::SourceDeclarativeDemand::NeedInput(
                                input,
                            ) => {
                                self.work.block(ticket, vec![input])?;
                                self.pending_nested = None;
                                return Ok(None);
                            }
                        }
                    }
                    super::source_declarative::SourceDeclarativeDemand::Ready(request) => {
                        prepared_request = Some(*request);
                    }
                }
            }
            let input = DeclarationFragmentInput {
                invocation: identity,
                request: self.expansion_budget.request_identity(),
                graph: self.key.unit_graph,
                compile_env: *compile_env,
                engine: *engine,
                epoch: self.epoch,
                cfg: provider.module_source().captured_macro_cfg()?.clone(),
            };
            let fragment = match prepared_request {
                Some(request) => DeclarationFragment::from_prepared_source(
                    parent,
                    input,
                    self.expansion_budget.clone(),
                    std::sync::Arc::new(request),
                )?,
                None => DeclarationFragment::new(
                    provider,
                    parent,
                    input,
                    self.expansion_budget.clone(),
                )?,
            };
            self.nested_fragments.insert(identity, fragment);
        }
        let namespaces = Self::install_macro_routes_with_modules(
            unique_graph(&mut self.graph)?,
            providers,
            &self.module_routes,
            &self.nested_sources,
            &self.nested_inert_proofs,
            &mut self.route_install,
        )?;
        let step = match self
            .nested_fragments
            .get_mut(&identity)
            .ok_or_else(|| fault("nested fragment absent"))?
            .poll_with_namespaces(provider, &namespaces)
        {
            Ok(step) => step,
            Err(error) => {
                self.failed = Some(error.clone());
                return Err(error);
            }
        };
        match step {
            DeclarationFragmentStep::NeedAttributeSelection(selection) => {
                self.pending_attribute = Some((
                    ticket.clone(),
                    AttributeSourceOwner::Fragment(ordinal),
                    selection.clone(),
                ));
                self.pending_nested = None;
                Ok(Some(SourceDiscoveryStep::NeedAttributeSelection {
                    selection,
                }))
            }
            DeclarationFragmentStep::NeedProceduralSelection(selection) => {
                self.pending_procedural = Some((ticket.clone(), ordinal, selection.clone()));
                self.pending_nested = None;
                Ok(Some(SourceDiscoveryStep::NeedProceduralSelection {
                    selection,
                }))
            }
            DeclarationFragmentStep::NeedExpansion(request) => {
                Ok(Some(SourceDiscoveryStep::NeedExpansion { request }))
            }
            DeclarationFragmentStep::NeedInput(input) => {
                self.work.block(ticket, vec![input])?;
                self.pending_nested = None;
                Ok(None)
            }
            DeclarationFragmentStep::Complete(output) => {
                let mut parent = (**self
                    .nested_sources
                    .get(&ordinal)
                    .ok_or_else(|| fault("nested source index absent"))?)
                .clone();
                parent.adopt_completed_child(
                    output.index().output().record().key(),
                    output.index(),
                )?;
                let mut replacement = None;
                let mut retired = Vec::new();
                if self.cold_attribute_owners.contains_key(&identity) {
                    let selection = self
                        .attribute_selections
                        .get(&identity)
                        .ok_or_else(|| fault("cold attribute completion selection absent"))?;
                    let record = output
                        .records()
                        .iter()
                        .find(|record| record.key().invocation == identity)
                        .ok_or_else(|| fault("cold attribute completion record absent"))?;
                    let proof = std::sync::Arc::new(
                        super::source_replacement::SourceReplacementProof::from_cold_adopted(
                            &self.sources[ordinal],
                            selection
                                .cold_scan()
                                .ok_or_else(|| fault("cold attribute completion scan absent"))?,
                            identity,
                            selection.item_range(),
                            record,
                        )?,
                    );
                    retired = parent.retire_replaced_item(&proof)?;
                    replacement = Some(proof);
                }
                let retained = self
                    .nested_bytes
                    .checked_add(output.metadata_bytes())
                    .and_then(|n| {
                        n.checked_add(
                            replacement
                                .as_ref()
                                .map(|proof| proof.metadata_bytes())
                                .transpose()
                                .ok()?
                                .unwrap_or(0),
                        )
                    })
                    .and_then(|n| n.checked_add(parent.retained_metadata_bytes()))
                    .ok_or_else(|| fault("nested declaration metadata limit"))?;
                let mut update = DiscoveryUpdate::default();
                update
                    .members
                    .try_reserve_exact(
                        output
                            .inventory()
                            .headers()
                            .len()
                            .checked_add(output.records().len())
                            .and_then(|n| n.checked_add(1))
                            .ok_or_else(|| fault("nested member count overflow"))?,
                    )
                    .map_err(|_| fault("nested member allocation"))?;
                update.members.push(identity);
                update
                    .members
                    .extend(output.inventory().headers().iter().map(|h| h.identity));
                update.members.extend(
                    output
                        .records()
                        .iter()
                        .map(|r| r.descriptor().fingerprint()),
                );
                update
                    .dependencies
                    .push((identity, output.index().output().fingerprint()));
                update
                    .dependencies
                    .extend(output.records().iter().map(|record| {
                        (
                            record.key().fingerprint(),
                            record.descriptor().fingerprint(),
                        )
                    }));
                update
                    .dependencies
                    .try_reserve_exact(self.compiler_inputs.len())
                    .map_err(|_| fault("nested compiler dependency allocation"))?;
                update.dependencies.extend(
                    self.compiler_inputs
                        .iter()
                        .map(|(key, fingerprint)| (key.fingerprint(), *fingerprint)),
                );
                if let Some(proof) = &replacement {
                    update
                        .dependencies
                        .push((proof.fingerprint(), proof.fingerprint()));
                }
                self.work.preflight_complete(ticket, &mut update)?;
                self.work.complete(ticket, update)?;
                if let Some(proof) = replacement {
                    self.source_replacements
                        .insert(proof.fingerprint(), proof.clone());
                    for id in retired {
                        self.replaced_source_producers.insert(id, proof.clone());
                    }
                }
                self.nested_sources
                    .insert(ordinal, std::sync::Arc::new(parent));
                self.nested_completed.insert(identity, output);
                self.nested_fragments.remove(&identity);
                self.nested_bytes = retained;
                self.pending_nested = None;
                Ok(None)
            }
        }
    }
    pub fn poll(&mut self) -> Result<SourceDiscoveryStep, Diagnostic> {
        self.resolve_deferred_imports()?;
        let step = self.poll_inner()?;
        match &step {
            SourceDiscoveryStep::NeedExpansion { request } => {
                self.remember_syntax_request(request)?
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                self.remember_generated_syntax_request(request)?
            }
            _ => {}
        }
        Ok(step)
    }
    fn poll_inner(&mut self) -> Result<SourceDiscoveryStep, Diagnostic> {
        self.expansion_budget.freeze()?;
        self.started = true;
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        if let Some(seal) = &self.seal {
            return Ok(SourceDiscoveryStep::Sealed(seal.clone()));
        }
        if let Some(ticket) = self.pending_nested.clone()
            && let Some(step) = self.poll_nested_declarations(&ticket)?
        {
            return Ok(step);
        }
        if let Some(pending) = &self.pending_source_derive {
            if let Some(request) = &pending.request {
                return Ok(match &pending.record {
                    Some(record) => SourceDiscoveryStep::NeedSourceDeriveAppend {
                        request: request.clone(),
                        record: record.clone(),
                    },
                    None => SourceDiscoveryStep::NeedSourceDeriveExpansion {
                        request: request.clone(),
                    },
                });
            }
            return Ok(SourceDiscoveryStep::NeedSourceDeriveSelection {
                selection: pending.selection.clone(),
            });
        }
        if let Some((_, _, request)) = &self.pending_module {
            return Ok(SourceDiscoveryStep::NeedExpansion {
                request: request.clone(),
            });
        }
        if let Some((_, _, selection)) = &self.pending_attribute {
            return Ok(SourceDiscoveryStep::NeedAttributeSelection {
                selection: selection.clone(),
            });
        }
        if let Some((_, _, selection)) = &self.pending_procedural {
            return Ok(SourceDiscoveryStep::NeedProceduralSelection {
                selection: selection.clone(),
            });
        }
        if let Some((_, request)) = &self.pending_declaration {
            return Ok(SourceDiscoveryStep::NeedDeriveDeclaration {
                request: request.clone(),
            });
        }
        if let Some((_, request, record)) = &self.pending_append {
            return Ok(SourceDiscoveryStep::NeedDeriveAppend {
                request: request.clone(),
                record: record.clone(),
            });
        }
        if let Some((_, request)) = &self.pending_generated {
            return Ok(SourceDiscoveryStep::NeedGeneratedExpansion {
                request: request.clone(),
            });
        }
        if let Some((_, _, request)) = &self.pending {
            return Ok(SourceDiscoveryStep::NeedExpansion {
                request: request.clone(),
            });
        }
        loop {
            self.complete_ready_item_boundaries()?;
            if let Some((_, _, providers)) = &mut self.declarative {
                match super::source_declarative::SourceDeclarative::synchronize_macro_uses(
                    providers,
                ) {
                    Ok(true) => self.work.namespace_changed()?,
                    Ok(false) => {}
                    Err(error) => {
                        self.failed = Some(error.clone());
                        return Err(error);
                    }
                }
            }
            match self.work.poll()? {
                DiscoveryStep::Run(ticket) => {
                    let job = ticket.job();
                    match job.kind {
                        DiscoveryKind::ParseSource => {
                            if let Some(producer) = self.module_parse_jobs.get(&job.identity) {
                                let syntax = &self
                                    .staged_modules
                                    .get(producer)
                                    .ok_or_else(|| fault("module parse job lacks staged syntax"))?
                                    .syntax;
                                let outline = syntax.prepared_source().outline();
                                let mut members = outline
                                    .items
                                    .iter()
                                    .map(|item| item.def_key.fingerprint())
                                    .collect::<Vec<_>>();
                                members.push(syntax.identity());
                                let mut dependencies = vec![(job.identity, outline.fingerprint())];
                                dependencies.extend(
                                    self.compiler_inputs.iter().map(|(key, fingerprint)| {
                                        (key.fingerprint(), *fingerprint)
                                    }),
                                );
                                self.work.complete(
                                    &ticket,
                                    DiscoveryUpdate {
                                        members,
                                        dependencies,
                                        ..Default::default()
                                    },
                                )?;
                                self.parsed_modules.insert(job.identity);
                                continue;
                            }
                            let source = &self.sources[*self
                                .source_jobs
                                .get(&job.identity)
                                .ok_or_else(|| fault("source job lacks captured publication"))?];
                            let outline = source.outline();
                            let members = outline
                                .items
                                .iter()
                                .map(|item| item.def_key.fingerprint())
                                .collect();
                            self.work.complete(
                                &ticket,
                                DiscoveryUpdate {
                                    members,
                                    dependencies: {
                                        let mut dependencies =
                                            vec![(job.identity, outline.fingerprint())];
                                        // These pre-poll selected inputs belong to the
                                        // complete discovery. Attach them once rather
                                        // than copying the whole map for every source.
                                        if self.source_jobs.get(&job.identity) == Some(&0) {
                                            dependencies
                                                .try_reserve_exact(self.compiler_inputs.len())
                                                .map_err(|_| {
                                                    fault("source selected input allocation")
                                                })?;
                                            dependencies.extend(self.compiler_inputs.iter().map(
                                                |(key, fingerprint)| {
                                                    (key.fingerprint(), *fingerprint)
                                                },
                                            ));
                                        }
                                        if let Some((_, _, providers)) = &self.declarative {
                                            for catalog in providers
                                                .iter()
                                                .filter_map(|p| p.procedural.as_ref())
                                            {
                                                dependencies.push((
                                                    catalog.fingerprint(),
                                                    catalog.fingerprint(),
                                                ));
                                            }
                                        }
                                        dependencies
                                    },
                                    ..Default::default()
                                },
                            )?;
                        }
                        DiscoveryKind::ResolveImport => {
                            let staged_import = self.staged_modules.values().any(|staged| {
                                staged
                                    .syntax
                                    .prepared_source()
                                    .outline()
                                    .imports
                                    .iter()
                                    .any(|import| {
                                        NameNamespaces::import_identity(import) == job.identity
                                    })
                            });
                            if self.imports.resolved.binary_search(&job.identity).is_ok()
                                || (staged_import
                                    && self.namespace != Namespace::Macro
                                    && self
                                        .graph
                                        .disclosed_import_facet(job.identity, self.namespace)?)
                            {
                                self.work.complete(
                                    &ticket,
                                    DiscoveryUpdate {
                                        members: vec![job.identity],
                                        ..Default::default()
                                    },
                                )?;
                            } else {
                                let waiting = if self.producers.is_empty() {
                                    vec![job.identity]
                                } else {
                                    self.producers.iter().copied().collect()
                                };
                                self.work.block(&ticket, waiting)?;
                            }
                        }
                        DiscoveryKind::ResolveMacro => {
                            if let Some(staged) = self.staged_modules.get(&job.identity)
                                && staged.namespace_complete
                                && matches!(staged.syntax, StagedModuleSyntax::Ordinary { .. })
                            {
                                self.work.complete(
                                    &ticket,
                                    DiscoveryUpdate {
                                        members: vec![job.identity, staged.syntax.identity()],
                                        ..Default::default()
                                    },
                                )?;
                                continue;
                            }
                            let module =
                                self.staged_modules.iter().find_map(|(producer, staged)| {
                                    staged
                                        .source_calls
                                        .contains_key(&job.identity)
                                        .then_some(*producer)
                                });
                            if let Some(producer) = module {
                                if self
                                    .staged_modules
                                    .get(&producer)
                                    .and_then(|module| module.source_calls.get(&job.identity))
                                    .is_some_and(|call| call.attribute_input.is_some())
                                {
                                    let providers = &self
                                        .declarative
                                        .as_ref()
                                        .ok_or_else(|| fault("staged attribute providers absent"))?
                                        .2;
                                    let names = Self::install_macro_routes_with_modules(
                                        unique_graph(&mut self.graph)?,
                                        providers,
                                        &self.module_routes,
                                        &self.nested_sources,
                                        &self.nested_inert_proofs,
                                        &mut self.route_install,
                                    )?;
                                    let staged = self
                                        .staged_modules
                                        .get(&producer)
                                        .ok_or_else(|| fault("staged attribute source absent"))?;
                                    let source = staged.syntax.prepared_source();
                                    if let Some(proof) =
                                        super::source_derives::prepare_empty_with_annotations(
                                            source,
                                            &staged.provider,
                                            job.identity,
                                            &names,
                                            staged.annotation_plan.as_deref(),
                                        )?
                                    {
                                        let call = staged.source_calls[&job.identity].clone();
                                        let mut update = DiscoveryUpdate {
                                            members: vec![
                                                job.identity,
                                                proof.item,
                                                proof.fingerprint,
                                            ],
                                            ..Default::default()
                                        };
                                        self.work.preflight_complete(&ticket, &mut update)?;
                                        unique_graph(&mut self.graph)?
                                            .restore_source_attribute_declaration(
                                                source.outline(),
                                                proof.item,
                                            )?;
                                        let staged = self
                                            .staged_modules
                                            .get_mut(&producer)
                                            .ok_or_else(|| {
                                                fault(
                                                    "staged compiler annotation source disappeared",
                                                )
                                            })?;
                                        staged
                                            .provider
                                            .complete_original_inert_item(job.identity, &proof)?;
                                        staged.completed_invocations.insert(job.identity);
                                        unique_graph(&mut self.graph)?
                                            .release_generated_frontier(&call.scope, call.frontier);
                                        self.source_empty_derives.insert(job.identity, proof);
                                        self.producers.remove(&job.identity);
                                        self.work.complete(&ticket, update)?;
                                        self.refresh_staged_module_routes(producer)?;
                                        continue;
                                    }
                                    if let Some(selection) = crate::service::source_attribute_requests::SourceAttributeSelection::select(source, &staged.provider, job.identity, &names)? {
                                        let selection = std::sync::Arc::new(selection);
                                        self.pending_attribute = Some((ticket, AttributeSourceOwner::Module(producer), selection.clone()));
                                        return Ok(SourceDiscoveryStep::NeedAttributeSelection { selection });
                                    }
                                    self.work.block(&ticket, vec![job.identity])?;
                                    continue;
                                }
                                if let Some(selection) =
                                    self.staged_procedural_selection(producer, job.identity)?
                                {
                                    let ordinal = self
                                        .staged_modules
                                        .get(&producer)
                                        .ok_or_else(|| {
                                            fault("module procedural owner disappeared")
                                        })?
                                        .ordinal;
                                    let selection = std::sync::Arc::new(selection);
                                    self.pending_procedural =
                                        Some((ticket, ordinal, selection.clone()));
                                    return Ok(SourceDiscoveryStep::NeedProceduralSelection {
                                        selection,
                                    });
                                }
                                match self.staged_module_demand(producer, job.identity)? {
                                    super::source_declarative::SourceDeclarativeDemand::Ready(request) => {
                                        let request = std::sync::Arc::from(request);
                                        self.pending_module = Some((ticket, producer, std::sync::Arc::clone(&request)));
                                        return Ok(SourceDiscoveryStep::NeedExpansion { request });
                                    }
                                    super::source_declarative::SourceDeclarativeDemand::NeedInput(input) => {
                                        self.work.block(&ticket, vec![input])?;
                                        continue;
                                    }
                                }
                            }
                            if let Some((ordinal, call)) = self.calls.get(&job.identity)
                                && call.attribute_input == Some(job.identity)
                                && let Some((_, _, providers)) = &self.declarative
                            {
                                if let Some(fragment) =
                                    self.source_derives_inflight.get(&job.identity)
                                {
                                    self.work.block(&ticket, vec![*fragment])?;
                                    continue;
                                }
                                let ordinal = *ordinal;
                                let namespaces = Self::install_macro_routes_with_modules(
                                    unique_graph(&mut self.graph)?,
                                    providers,
                                    &self.module_routes,
                                    &self.nested_sources,
                                    &self.nested_inert_proofs,
                                    &mut self.route_install,
                                )?;
                                if let Some(proof) =
                                    super::source_derives::prepare_empty_with_annotations(
                                        &self.sources[ordinal],
                                        &providers[ordinal],
                                        job.identity,
                                        &namespaces,
                                        self.original_annotation_plans
                                            .get(&ordinal)
                                            .map(std::sync::Arc::as_ref),
                                    )?
                                {
                                    let call = call.clone();
                                    let mut update = DiscoveryUpdate {
                                        members: vec![job.identity, proof.item, proof.fingerprint],
                                        // This is a derived source member, not an external input.
                                        // The captured outline dependency already binds its facts;
                                        // exact compiler selection replays before sealing.
                                        ..Default::default()
                                    };
                                    self.work.preflight_complete(&ticket, &mut update)?;
                                    unique_graph(&mut self.graph)?
                                        .restore_source_attribute_declaration(
                                            self.sources[ordinal].outline(),
                                            proof.item,
                                        )?;
                                    self.declarative
                                        .as_mut()
                                        .ok_or_else(|| fault("source provider absent"))?
                                        .2[ordinal]
                                        .complete_original_inert_item(job.identity, &proof)?;
                                    let frontier = self
                                        .frontier_producers
                                        .get_mut(&(call.scope.clone(), call.frontier))
                                        .ok_or_else(|| {
                                            fault("source empty derive frontier absent")
                                        })?;
                                    if !frontier.remove(&job.identity) {
                                        return Err(fault("source empty derive already settled"));
                                    }
                                    if frontier.is_empty() {
                                        unique_graph(&mut self.graph)?
                                            .release_generated_frontier(&call.scope, call.frontier);
                                    }
                                    self.source_empty_derives.insert(job.identity, proof);
                                    self.work.complete(&ticket, update)?;
                                    self.producers.remove(&job.identity);
                                    Self::install_macro_routes_with_modules(
                                        unique_graph(&mut self.graph)?,
                                        &self
                                            .declarative
                                            .as_ref()
                                            .ok_or_else(|| {
                                                fault("source compiler annotation providers absent")
                                            })?
                                            .2,
                                        &self.module_routes,
                                        &self.nested_sources,
                                        &self.nested_inert_proofs,
                                        &mut self.route_install,
                                    )?;
                                    self.imports =
                                        unique_graph(&mut self.graph)?.resolve_imports()?;
                                    continue;
                                }
                                match super::source_derives::prepare_group(
                                    &self.sources[ordinal],
                                    &providers[ordinal],
                                    job.identity,
                                    &namespaces,
                                    &self.completed_source_derives,
                                    &self.helper_declarations,
                                )? {
                                    super::source_derives::SourceDerivePoll::NeedDeclaration(
                                        request,
                                    ) => {
                                        self.pending_declaration = Some((ticket, request.clone()));
                                        return Ok(SourceDiscoveryStep::NeedDeriveDeclaration {
                                            request,
                                        });
                                    }
                                    super::source_derives::SourceDerivePoll::Ready(selection)
                                        if selection.is_builtin() =>
                                    {
                                        self.expand_builtin_source_derive(ticket, selection)?;
                                        continue;
                                    }
                                    super::source_derives::SourceDerivePoll::Ready(selection) => {
                                        let bytes = self
                                            .procedural_selection_bytes
                                            .checked_add(selection.metadata_bytes()?)
                                            .ok_or_else(|| {
                                                fault("original derive selection retention limit")
                                            })?;
                                        self.pending_source_derive = Some(PendingSourceDerive {
                                            ticket,
                                            selection: selection.clone(),
                                            request: None,
                                            record: None,
                                        });
                                        self.procedural_selection_bytes = bytes;
                                        return Ok(
                                            SourceDiscoveryStep::NeedSourceDeriveSelection {
                                                selection,
                                            },
                                        );
                                    }
                                    super::source_derives::SourceDerivePoll::Attribute(
                                        selection,
                                    ) => {
                                        self.pending_attribute = Some((
                                            ticket,
                                            AttributeSourceOwner::Original(ordinal),
                                            selection.clone(),
                                        ));
                                        return Ok(SourceDiscoveryStep::NeedAttributeSelection {
                                            selection,
                                        });
                                    }
                                    super::source_derives::SourceDerivePoll::Complete(proof) => {
                                        let call = call.clone();
                                        self.complete_source_derive(
                                            &ticket, ordinal, &call, proof,
                                        )?;
                                        continue;
                                    }
                                    super::source_derives::SourceDerivePoll::Pending => {}
                                }
                                if let Some(selection) = crate::service::source_attribute_requests::SourceAttributeSelection::select(
                                    &self.sources[ordinal], &providers[ordinal], job.identity, &namespaces)? {
                                    let selection = std::sync::Arc::new(selection);
                                    self.pending_attribute = Some((ticket, AttributeSourceOwner::Original(ordinal), selection.clone()));
                                    return Ok(SourceDiscoveryStep::NeedAttributeSelection { selection });
                                }
                                self.work.block(&ticket, vec![job.identity])?;
                                continue;
                            }
                            if let (Some((compile_env, engine, providers)), Some((ordinal, call))) =
                                (&self.declarative, self.calls.get(&job.identity))
                                && call.attribute_input.is_none()
                            {
                                if let Some(catalog) = &providers[*ordinal].procedural {
                                    let namespaces = Self::install_macro_routes_with_modules(
                                        unique_graph(&mut self.graph)?,
                                        providers,
                                        &self.module_routes,
                                        &self.nested_sources,
                                        &self.nested_inert_proofs,
                                        &mut self.route_install,
                                    )?;
                                    self.imports =
                                        unique_graph(&mut self.graph)?.resolve_imports()?;
                                    if let crate::frontend::macro_names::PathLookup::Defined(
                                        definition,
                                    ) = providers[*ordinal]
                                        .resolved_function_call(job.identity, &namespaces)?
                                        && catalog
                                            .select(
                                                &definition,
                                                super::identity::ProceduralKind::Function,
                                            )?
                                            .is_some()
                                    {
                                        let input = crate::service::function_macro_requests::FunctionInvocationInput::from_source(&self.sources[*ordinal], job.identity)?;
                                        let selection =
                                            std::sync::Arc::new(ProceduralSourceSelection {
                                                module: None,
                                                nested: false,
                                                input,
                                                definition,
                                                catalog: catalog.clone(),
                                            });
                                        self.pending_procedural =
                                            Some((ticket, *ordinal, selection.clone()));
                                        return Ok(SourceDiscoveryStep::NeedProceduralSelection {
                                            selection,
                                        });
                                    }
                                }
                                let direct = providers[*ordinal].demand_in(
                                    job.identity,
                                    *compile_env,
                                    *engine,
                                    &Default::default(),
                                    providers,
                                )?;
                                let demand = if matches!(
                                    direct,
                                    super::source_declarative::SourceDeclarativeDemand::Ready(_)
                                ) {
                                    direct
                                } else {
                                    let namespaces = Self::install_macro_routes_with_modules(
                                        unique_graph(&mut self.graph)?,
                                        providers,
                                        &self.module_routes,
                                        &self.nested_sources,
                                        &self.nested_inert_proofs,
                                        &mut self.route_install,
                                    )?;
                                    self.imports =
                                        unique_graph(&mut self.graph)?.resolve_imports()?;
                                    providers[*ordinal].demand_in(
                                        job.identity,
                                        *compile_env,
                                        *engine,
                                        &namespaces,
                                        providers,
                                    )?
                                };
                                let demand = if matches!(
                                    demand,
                                    super::source_declarative::SourceDeclarativeDemand::NeedInput(
                                        _
                                    )
                                ) {
                                    let namespaces = Self::install_macro_routes_with_modules(
                                        unique_graph(&mut self.graph)?,
                                        providers,
                                        &self.module_routes,
                                        &self.nested_sources,
                                        &self.nested_inert_proofs,
                                        &mut self.route_install,
                                    )?;
                                    providers[*ordinal].demand_for_discovery(
                                        job.identity,
                                        *compile_env,
                                        *engine,
                                        &namespaces,
                                        providers,
                                    )?
                                } else {
                                    demand
                                };
                                match demand {
                                    super::source_declarative::SourceDeclarativeDemand::Ready(request) => {
                                        let request = std::sync::Arc::from(request);
                                        self.pending = Some((ticket, *ordinal, std::sync::Arc::clone(&request)));
                                        return Ok(SourceDiscoveryStep::NeedExpansion {request});
                                    }
                                    super::source_declarative::SourceDeclarativeDemand::NeedInput(input) => {
                                        self.work.block(&ticket, vec![input])?;
                                        continue;
                                    }
                                }
                            }
                            self.work.block(&ticket, vec![job.identity])?;
                        }
                        DiscoveryKind::ScanNestedDeclarations => {
                            if let Some(proof) = self.replaced_source_producers.get(&job.identity) {
                                self.work.complete(
                                    &ticket,
                                    DiscoveryUpdate {
                                        members: vec![job.identity, proof.fingerprint()],
                                        dependencies: vec![(
                                            proof.fingerprint(),
                                            proof.fingerprint(),
                                        )],
                                        ..Default::default()
                                    },
                                )?;
                                continue;
                            }

                            if let Some(waiter) = self.child_impl_frontiers.get(&job.identity) {
                                let ancestor = waiter.ancestor_job();
                                let Some(completed) = self.nested_completed.get(&ancestor) else {
                                    self.work.block(&ticket, vec![ancestor])?;
                                    continue;
                                };
                                if !self.item_impl_completions.contains_key(&ancestor) {
                                    // Reserve the index's maximum temporary budget before
                                    // constructing it; retain and charge its actual size once.
                                    let prepared = std::sync::Arc::new(
                                        item_impl_completion::AncestorImplCompletion::prepare(
                                            completed.clone(),
                                            ancestor,
                                            self.epoch,
                                            waiter.cfg_fingerprint(),
                                        )?,
                                    );
                                    let bytes = self
                                        .nested_bytes
                                        .checked_add(prepared.metadata_bytes())
                                        .and_then(|bytes| bytes.checked_add(128))
                                        .ok_or_else(|| fault("child impl completion retention"))?;
                                    self.item_impl_completions.insert(ancestor, prepared);
                                    self.nested_bytes = bytes;
                                }
                                let prepared = self
                                    .item_impl_completions
                                    .get(&ancestor)
                                    .ok_or_else(|| fault("child impl completion cache absent"))?;
                                if !std::sync::Arc::ptr_eq(completed, prepared.completed()) {
                                    return Err(fault("child impl actual completed owner changed"));
                                }
                                let headers = waiter.resolve(prepared, self.epoch)?;
                                let scope = waiter.scope().clone();
                                // The waiter is a derived coordinator proof. Its
                                // authority is the same published fragment and
                                // records validated above, not a new input row.
                                let output = prepared.completed().index().output();
                                let dependency =
                                    (output.record().key().invocation, output.fingerprint());
                                let mut members = Vec::new();
                                members
                                    .try_reserve_exact(
                                        headers.len().checked_add(1).ok_or_else(|| {
                                            fault("child impl member count overflow")
                                        })?,
                                    )
                                    .map_err(|_| fault("child impl member allocation"))?;
                                members.push(job.identity);
                                members.extend(headers);
                                let result = (|| {
                                    self.work.complete(
                                        &ticket,
                                        DiscoveryUpdate {
                                            members,
                                            dependencies: vec![dependency],
                                            ..Default::default()
                                        },
                                    )?;
                                    unique_graph(&mut self.graph)?
                                        .release_generated_frontier(&scope, job.identity);
                                    self.producers.remove(&job.identity);
                                    self.child_impl_frontiers.remove(&job.identity);
                                    Ok::<_, Diagnostic>(())
                                })();
                                if let Err(error) = result {
                                    self.failed = Some(error.clone());
                                    return Err(error);
                                }
                                continue;
                            }
                            if let Some((declaration_job, header, scope)) =
                                self.item_impl_jobs.get(&job.identity).cloned()
                            {
                                if let Some(fragment) = self.nested_completed.get(&declaration_job)
                                {
                                    if !header.iter().all(|expected| {
                                        fragment
                                            .inventory()
                                            .headers()
                                            .iter()
                                            .any(|actual| actual.header_identity == *expected)
                                    }) {
                                        return Err(fault("completed item impl header absent"));
                                    }
                                    let mut members = Vec::new();
                                    members
                                        .try_reserve_exact(header.len().checked_add(1).ok_or_else(
                                            || fault("impl frontier members overflow"),
                                        )?)
                                        .map_err(|_| fault("impl frontier member allocation"))?;
                                    members.push(job.identity);
                                    members.extend(header.iter().copied());
                                    self.work.complete(
                                        &ticket,
                                        DiscoveryUpdate {
                                            members,
                                            dependencies: vec![(
                                                fragment.index().output().record().key().invocation,
                                                fragment.index().output().fingerprint(),
                                            )],
                                            ..Default::default()
                                        },
                                    )?;
                                    unique_graph(&mut self.graph)?
                                        .release_generated_frontier(&scope, job.identity);
                                    self.producers.remove(&job.identity);
                                } else {
                                    self.work.block(&ticket, vec![declaration_job])?;
                                }
                                continue;
                            }
                            if self.nested_jobs.contains_key(&job.identity) {
                                self.pending_nested = Some(ticket.clone());
                                if let Some(step) = self.poll_nested_declarations(&ticket)? {
                                    return Ok(step);
                                }
                                continue;
                            }

                            if let Some(module) = self.staged_modules.get(&job.identity)
                                && module.namespace_complete
                            {
                                self.work.complete(
                                    &ticket,
                                    DiscoveryUpdate {
                                        members: vec![job.identity, module.syntax.identity()],
                                        ..Default::default()
                                    },
                                )?;
                                continue;
                            }
                            if self.fragments.contains_key(&job.identity) {
                                let demand = self.poll_generated_fragment(job.identity);
                                let demand = match demand {
                                    Ok(demand) => demand,
                                    Err(error) => {
                                        self.failed = Some(error.clone());
                                        return Err(error);
                                    }
                                };
                                match demand {
                                    crate::frontend::generated_macros::GeneratedCallDemand::NeedDeriveExecution {..}=>return Err(fault("uncached generated helper scope escaped catalog")),
                                    crate::frontend::generated_macros::GeneratedCallDemand::Append {request,record}=>{
                                        self.pending_append=Some((ticket,request.clone(),record.clone()));return Ok(SourceDiscoveryStep::NeedDeriveAppend {request,record});
                                    }
                                    crate::frontend::generated_macros::GeneratedCallDemand::NeedDeclaration(request)=>{
                                        self.pending_declaration=Some((ticket,request.clone()));
                                        return Ok(SourceDiscoveryStep::NeedDeriveDeclaration {request});
                                    }
                                    crate::frontend::generated_macros::GeneratedCallDemand::Ready(request)=>{
                                        self.pending_generated=Some((ticket,request.clone()));
                                        return Ok(SourceDiscoveryStep::NeedGeneratedExpansion {request});
                                    }
                                    crate::frontend::generated_macros::GeneratedCallDemand::NeedInput(input)=>self.work.block(&ticket,vec![input])?,
                                    crate::frontend::generated_macros::GeneratedCallDemand::Complete=>self.finish_fragment(&ticket)?,
                                }
                            } else {
                                self.work.block(&ticket, vec![job.identity])?;
                            }
                        }
                        _ => return Err(fault("unexpected captured source discovery job")),
                    }
                }
                DiscoveryStep::Incomplete {
                    diagnostic,
                    blocked,
                } => {
                    if self.calls.keys().any(|identity| {
                        self.procedural_selections.contains_key(identity)
                            || self.attribute_selections.contains_key(identity)
                    }) && let Some((_, _, providers)) = &self.declarative
                    {
                        let namespaces = Self::install_macro_routes_with_modules(
                            unique_graph(&mut self.graph)?,
                            providers,
                            &self.module_routes,
                            &self.nested_sources,
                            &self.nested_inert_proofs,
                            &mut self.route_install,
                        )?;
                        for (identity, (ordinal, _)) in &self.calls {
                            if self.attribute_selections.contains_key(identity)
                                && let Err(error) = self.replay_attribute_selection(
                                    *ordinal,
                                    *identity,
                                    providers,
                                    &namespaces,
                                )
                            {
                                self.failed = Some(error.clone());
                                return Err(error);
                            }
                            if self.procedural_selections.contains_key(identity)
                                && matches!(
                                    providers[*ordinal]
                                        .resolved_function_call(*identity, &namespaces)?,
                                    crate::frontend::macro_names::PathLookup::Ambiguous(_)
                                )
                            {
                                let error = Diagnostic::new("LRR-E-MACRO-AMBIGUOUS", "original procedural selection became ambiguous during discovery".into());
                                self.failed = Some(error.clone());
                                return Err(error);
                            }
                        }
                    }
                    let diagnostic = self.attach_incomplete_source_origin(diagnostic, &blocked)?;
                    return Ok(SourceDiscoveryStep::Incomplete {
                        diagnostic,
                        blocked,
                    });
                }
                DiscoveryStep::Sealed(seal) => {
                    if !self.pending_item_boundaries.is_empty() {
                        let error = fault("scope seal retains pending item boundaries");
                        self.failed = Some(error.clone());
                        return Err(error);
                    }
                    if let Some((_, _, providers)) = &self.declarative {
                        let namespaces = match Self::install_macro_routes_with_modules(
                            unique_graph(&mut self.graph)?,
                            providers,
                            &self.module_routes,
                            &self.nested_sources,
                            &self.nested_inert_proofs,
                            &mut self.route_install,
                        ) {
                            Ok(namespaces) => namespaces,
                            Err(error) => {
                                self.failed = Some(error.clone());
                                return Err(error);
                            }
                        };
                        self.imports = unique_graph(&mut self.graph)?.resolve_imports()?;
                        for (identity, definition) in &self.procedural_selections {
                            if let Some(ordinal) = self.nested_jobs.get(identity) {
                                let parent = self.nested_sources.get(ordinal).ok_or_else(|| {
                                    fault("cold procedural revalidation parent absent")
                                })?;
                                let actual = providers[*ordinal]
                                    .declaration_procedural_definition(
                                        parent,
                                        *identity,
                                        &namespaces,
                                    )?;
                                let output =
                                    self.nested_completed.get(identity).ok_or_else(|| {
                                        fault("cold procedural completed output absent")
                                    })?;
                                let catalog =
                                    providers[*ordinal].procedural.as_ref().ok_or_else(|| {
                                        fault("cold procedural revalidation catalog absent")
                                    })?;
                                let selected = catalog
                                    .select(definition, super::identity::ProceduralKind::Function)?
                                    .ok_or_else(|| fault("cold procedural export absent"))?;
                                if actual.as_ref() != Some(definition)
                                    || &output.index().output().record().key().macro_identity
                                        != selected.identity()
                                    || output.index().output().record().key().procedural_epoch
                                        != Some(self.epoch)
                                {
                                    return Err(fault(
                                        "cold procedural source selection changed before seal",
                                    ));
                                }
                            }
                        }
                        for (identity, fragment) in &self.nested_completed {
                            let ordinal = self
                                .nested_jobs
                                .get(identity)
                                .ok_or_else(|| fault("nested procedural owner absent"))?;
                            fragment.revalidate_procedural_selections(
                                &providers[*ordinal],
                                &namespaces,
                                providers,
                            )?;
                        }
                        for fragment in &self.completed_argument_fragments {
                            let source = fragment.source_scan();
                            let provider = providers
                                .iter()
                                .find(|p| {
                                    p.module_source().selection().outline.parse == *source.parse()
                                        && p.module_source().selection().path == *source.path()
                                })
                                .ok_or_else(|| fault("argument procedural provider absent"))?;
                            fragment.revalidate_procedural_selections(
                                provider,
                                &namespaces,
                                providers,
                            )?;
                        }
                        if let Err(error) = self.revalidate_original_selections(&namespaces) {
                            self.failed = Some(error.clone());
                            return Err(error);
                        }
                    }
                    // The job ledger alone is insufficient: namespace frontiers
                    // must also be discharged before a seal can escape to callers.
                    if !(self.namespace == Namespace::Macro && self.declarative.is_none()) {
                        let checked = self.scope_edition().and_then(|edition| {
                            self.graph
                                .certified_scope(&self.scope, self.namespace, edition)
                        });
                        if let Err(error) = checked {
                            self.failed = Some(error.clone());
                            return Err(error);
                        }
                    }
                    // Compute this existing query-value identity once at
                    // completion, never by walking the closure on body entry.
                    self.body_boundary_seal_fingerprint = Some(
                        crate::db::value::QueryValue::DiscoverySeal(Box::new(seal.clone()))
                            .fingerprint(),
                    );
                    self.prepare_completed_source_environment(&seal)?;
                    self.seal = Some(seal.clone());
                    return Ok(SourceDiscoveryStep::Sealed(seal));
                }
            }
        }
    }
    /// Available pure candidates never become a certificate without strict replay.
    fn revalidate_original_selections(
        &self,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
    ) -> Result<(), Diagnostic> {
        let Some((compile_env, engine, providers)) = &self.declarative else {
            return Ok(());
        };
        if let Some(prepared) = &self.compiler_profiles {
            prepared.revalidate(providers)?;
        }
        super::source_declarative::SourceDeclarative::revalidate_captured_role_parents(providers)?;
        let mut records = Vec::new();
        self.calls
            .len()
            .checked_mul(std::mem::size_of::<(Digest, &super::record::ExpansionRecord)>())
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "selection revalidation metadata limit".into(),
                )
            })?;
        records.try_reserve_exact(self.calls.len()).map_err(|_| {
            Diagnostic::new(
                "LRR-E-DISCOVERY-LIMIT",
                "selection revalidation allocation".into(),
            )
        })?;
        for record in self.records.values() {
            if self.calls.contains_key(&record.key().invocation) {
                if records.len() >= self.calls.len() {
                    return Err(fault("original selection record count differs"));
                }
                records.push((record.key().invocation, record.as_ref()));
            }
        }
        records.sort_by_key(|(id, _)| *id);
        if records.windows(2).any(|w| w[0].0 == w[1].0) {
            return Err(fault("duplicate original selection records"));
        }
        for (identity, (ordinal, _)) in &self.calls {
            if let Some(proof) = self.source_derive_completions.get(identity) {
                match super::source_derives::prepare_group(
                    &self.sources[*ordinal],
                    &providers[*ordinal],
                    *identity,
                    namespaces,
                    &self.completed_source_derives,
                    &self.helper_declarations,
                )? {
                    super::source_derives::SourceDerivePoll::Complete(current)
                        if &current == proof => {}
                    _ => return Err(fault("source derive completion changed before seal")),
                }
                continue;
            }
            if let Some(proof) = self.source_empty_derives.get(identity) {
                if super::source_derives::prepare_empty_with_annotations(
                    &self.sources[*ordinal],
                    &providers[*ordinal],
                    *identity,
                    namespaces,
                    self.original_annotation_plans
                        .get(ordinal)
                        .map(std::sync::Arc::as_ref),
                )?
                .as_ref()
                    != Some(proof)
                {
                    return Err(fault("source empty derive selection changed before seal"));
                }
                continue;
            }
            let index = records
                .binary_search_by_key(identity, |(id, _)| *id)
                .map_err(|_| fault("original selection record absent"))?;
            if let Some(selected) = self.attribute_selections.get(identity) {
                let current = self
                    .replay_attribute_selection(*ordinal, *identity, providers, namespaces)?
                    .ok_or_else(|| fault("source attribute selection incomplete before seal"))?;
                if current.fingerprint() != selected.fingerprint()
                    || records[index].1.key().procedural_epoch != Some(self.epoch)
                {
                    return Err(fault("source attribute selection changed before seal"));
                }
                continue;
            }
            if let Some(definition) = self.procedural_selections.get(identity) {
                let current = providers[*ordinal].resolved_function_call(*identity, namespaces)?;
                let catalog = providers[*ordinal]
                    .procedural
                    .as_ref()
                    .ok_or_else(|| fault("procedural revalidation catalog absent"))?;
                let selected = catalog
                    .select(definition, super::identity::ProceduralKind::Function)?
                    .ok_or_else(|| fault("procedural revalidation export absent"))?;
                if current != crate::frontend::macro_names::PathLookup::Defined(definition.clone())
                    || &records[index].1.key().macro_identity != selected.identity()
                    || records[index].1.key().procedural_epoch != Some(self.epoch)
                {
                    return Err(fault("procedural source selection changed before seal"));
                }
                continue;
            }
            match providers[*ordinal].demand_in(
                *identity,
                *compile_env,
                *engine,
                namespaces,
                providers,
            )? {
                super::source_declarative::SourceDeclarativeDemand::Ready(request)
                    if request.key() == records[index].1.key() => {}
                super::source_declarative::SourceDeclarativeDemand::Ready(_) => {
                    return Err(Diagnostic::new(
                        "LRR-E-EXPANSION-INPUT",
                        "original macro selection changed before scope sealing".into(),
                    ));
                }
                super::source_declarative::SourceDeclarativeDemand::NeedInput(_) => {
                    return Err(fault(
                        "original macro selection remains incomplete before scope sealing",
                    ));
                }
            }
        }
        for provider in providers {
            // Candidate routes cannot stand in for final Attribute selection.
            // The completion loop above reselects every stored proof strictly.
            provider.require_final_compiler_attributes(&self.source_empty_derives)?;
            provider.revalidate_generated_selections(providers)?;
        }
        let mut module_records = BTreeMap::new();
        for record in self.records.values() {
            if module_records
                .insert(record.key().invocation, record)
                .is_some()
            {
                return Err(fault("staged attribute replay duplicate invocation"));
            }
        }
        for module in self.staged_modules.values() {
            for call in module
                .source_calls
                .values()
                .filter(|call| call.attribute_input.is_some())
            {
                if !module.namespace_complete
                    || !module.completed_invocations.contains(&call.item_input)
                {
                    return Err(fault(
                        "staged attribute namespace incomplete before sealing",
                    ));
                }
                if let Some(proof) = self.source_empty_derives.get(&call.item_input) {
                    if super::source_derives::prepare_empty_with_annotations(
                        module.syntax.prepared_source(),
                        &module.provider,
                        call.item_input,
                        namespaces,
                        module.annotation_plan.as_deref(),
                    )?
                    .as_ref()
                        != Some(proof)
                    {
                        return Err(fault("staged compiler annotation changed before sealing"));
                    }
                } else {
                    let selected = self
                        .attribute_selections
                        .get(&call.item_input)
                        .ok_or_else(|| fault("staged attribute selection absent before sealing"))?;
                    let current = crate::service::source_attribute_requests::SourceAttributeSelection::select(module.syntax.prepared_source(), &module.provider, call.item_input, namespaces)?.ok_or_else(|| fault("staged attribute selection incomplete before sealing"))?;
                    let record = module_records
                        .get(&call.item_input)
                        .ok_or_else(|| fault("staged attribute record absent before sealing"))?;
                    if selected.fingerprint() != current.fingerprint()
                        || record.key().procedural_epoch != Some(self.epoch)
                    {
                        return Err(fault("staged attribute selection changed before sealing"));
                    }
                }
            }
        }
        for (identity, (ordinal, binding)) in &self.cold_test_bindings {
            let scan = self.sources[*ordinal].declaration_inventory(Default::default())?;
            let lookup =
                crate::service::source_attribute_requests::SourceAttributeSelection::cold_lookup(
                    &self.sources[*ordinal],
                    &scan,
                    *identity,
                    namespaces,
                )?;
            let current = match lookup {
                crate::frontend::macro_names::PathLookup::Absent => None,
                crate::frontend::macro_names::PathLookup::Defined(definition) => Some(definition),
                _ => {
                    return Err(fault(
                        "cold compiler test binding incomplete before sealing",
                    ));
                }
            };
            if &current != binding {
                return Err(fault("cold compiler test binding changed before sealing"));
            }
            if let Some(definition) = &current
                && !super::builtin_attributes::permits_builtin_test_marker(
                    definition,
                    providers[*ordinal].procedural.as_deref(),
                    providers[*ordinal]
                        .compiler
                        .as_ref()
                        .and_then(|c| c.attribute_catalog()),
                )?
            {
                return Err(fault("cold compiler Test facet changed before sealing"));
            }
        }
        let mut original_replays = BTreeMap::new();
        for (identity, proof) in &self.nested_inert_proofs {
            if self.original_prefix_jobs.contains(identity) {
                let ordinal = *self
                    .nested_jobs
                    .get(identity)
                    .ok_or_else(|| fault("original inert prefix source owner absent"))?;
                let replay_count = original_replays.len();
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    original_replays.entry(ordinal)
                {
                    replay_count
                        .checked_add(1)
                        .filter(|count| *count <= self.sources.len())
                        .and_then(|count| {
                            count.checked_mul(
                                std::mem::size_of::<
                                    crate::source::compiler_replay::CompilerReplaySource<'_>,
                                >()
                                .checked_add(128)?,
                            )
                        })
                        .ok_or_else(|| fault("original prefix replay temporary metadata limit"))?;
                    let source = self
                        .sources
                        .get(ordinal)
                        .ok_or_else(|| fault("original prefix replay source is absent"))?;
                    let plan = self
                        .original_annotation_plans
                        .get(&ordinal)
                        .ok_or_else(|| fault("original prefix replay source plan is absent"))?;
                    let current = crate::source::compiler_replay::CompilerReplaySource::prepare(
                        source,
                        plan.scan(),
                    )?;
                    entry.insert(current);
                }
                original_inert::revalidate(
                    original_replays
                        .get(&ordinal)
                        .ok_or_else(|| fault("original prefix replay source is absent"))?,
                    providers
                        .get(ordinal)
                        .ok_or_else(|| fault("original prefix replay provider is absent"))?,
                    proof,
                    namespaces,
                    &self.original_prefix_jobs,
                )?;
                continue;
            }
            if let Some(expected) = proof.attribute_facet() {
                let ordinal = *self
                    .nested_jobs
                    .get(identity)
                    .ok_or_else(|| fault("cold inert facet source owner absent"))?;
                let lookup = crate::service::source_attribute_requests::SourceAttributeSelection::cold_lookup(
                    &self.sources[ordinal], proof.scan(), *identity, namespaces,
                )?;
                let actual = super::builtin_attributes::certify_inert_binding(
                    proof.name()?,
                    &lookup,
                    providers[ordinal].procedural.as_deref(),
                    providers[ordinal]
                        .compiler
                        .as_ref()
                        .and_then(|c| c.attribute_catalog()),
                )?
                .ok_or_else(|| fault("cold inert facet incomplete before sealing"))?;
                if actual != expected {
                    return Err(fault("cold inert facet changed before sealing"));
                }
            }
        }
        for (identity, ordinal) in &self.cold_attribute_owners {
            let selected = self
                .attribute_selections
                .get(identity)
                .ok_or_else(|| fault("cold attribute replay selection absent"))?;
            let scan = self.sources[*ordinal].declaration_inventory(Default::default())?;
            let current =
                crate::service::source_attribute_requests::SourceAttributeSelection::select_cold(
                    &self.sources[*ordinal],
                    &providers[*ordinal],
                    scan,
                    *identity,
                    namespaces,
                )?
                .ok_or_else(|| fault("cold attribute selection incomplete before sealing"))?;
            if selected.fingerprint() != current.fingerprint() {
                return Err(fault("cold attribute selection changed before sealing"));
            }
        }
        Ok(())
    }
    /// Read-only lookup from the requested sealed scope. No graph mutation escapes.
    pub fn lookup(&self, name: &str) -> Result<NameLookup, Diagnostic> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        if self.seal.is_none() {
            return Err(fault("namespace lookup requires completed scope discovery"));
        }
        self.graph.lookup_unqualified(
            self.namespace,
            name,
            &self.scope,
            self.scope_edition()?,
            None,
        )
    }
}

fn insert_generated_test_registrations<'a>(
    registrations: &mut BTreeMap<Digest, &'a crate::syntax::outline::ItemOutline>,
    catalog: &'a crate::frontend::generated_macros::GeneratedMacroCatalog,
) -> Result<(), Diagnostic> {
    let mut pending = vec![(catalog, 0usize)];
    let mut visited = 0usize;
    while let Some((catalog, depth)) = pending.pop() {
        visited = visited
            .checked_add(1)
            .ok_or_else(|| fault("compiler Test registration catalog count limit"))?;
        for child in catalog.selection_catalog_children() {
            pending
                .try_reserve(1)
                .map_err(|_| fault("compiler Test registration work allocation"))?;
            pending.push((child, depth + 1));
        }
        for (origin, item) in catalog.test_registrations() {
            if let Some(previous) = registrations.get(&origin) {
                if previous.def_key != item.def_key
                    || previous.scope != item.scope
                    || previous.header != item.header
                    || previous.declaration_span != item.declaration_span
                {
                    return Err(fault(
                        "generated compiler Test registration origin conflicts",
                    ));
                }
            } else {
                registrations.insert(origin, item);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod seal_tests {
    use super::*;
    #[test]
    fn ready_jobs_cannot_seal_a_scope_with_a_retained_namespace_frontier() {
        use crate::{
            core::profile::ProfileIdentity,
            db::{key::OutlineKey, memo::Database},
            source::{
                changes::{
                    PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
                },
                snapshot::{CfgSet, SourceBlob, SourcePath},
            },
            syntax::parse::ParseKey,
        };
        let unit = Digest([61; 32]);
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(b"fn main()->i64{7_i64}".to_vec()).unwrap();
        let path = SourcePath::new("seal-frontier.rs").unwrap();
        let selection = SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let inputs = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
            panic!("source absent");
        };
        let scope = view.outline().root_scope.clone();
        let mut work = SourceDiscovery::new(
            vec![view],
            scope.clone(),
            Namespace::Value,
            Digest([62; 32]),
            1,
        )
        .unwrap();
        unique_graph(&mut work.graph)
            .unwrap()
            .reserve_generated_frontier(scope, Digest([63; 32]))
            .unwrap();
        let error = work
            .poll()
            .err()
            .expect("ready job ledger hid an unresolved namespace frontier");
        assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
        assert!(work.sealed_expansions().is_err());
        assert!(work.lookup("main").is_err());
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        let revision = db.revision();
        assert!(work.publish(&mut db).is_err());
        assert_eq!(db.revision(), revision);
        assert!(
            db.input(&crate::db::dependency::InputKey::DiscoverySeal(
                work.key().clone()
            ))
            .is_none()
        );
    }
}
#[cfg(test)]
mod module_binding_coordinator_tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        expand::scopes::MacroName,
        frontend::macro_names::{DeclarationModuleOrigin, MacroNamespaces, ModuleLookup},
        source::snapshot::{CfgSet, SourceBlob, SourcePath},
        syntax::{declaration_scan::DeclarationScan, parse::ParsedFile},
    };
    fn scan(text: &str) -> std::sync::Arc<DeclarationScan> {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        std::sync::Arc::new(
            parsed
                .scan_declarations(
                    SourcePath::new("module-proof.rs").unwrap(),
                    ScopeKey {
                        crate_key: Digest([211; 32]),
                        module_path: vec![],
                        owner: None,
                    },
                    &CfgSet::default(),
                    Default::default(),
                )
                .unwrap(),
        )
    }
    fn binding<'a>(
        names: &'a MacroNamespaces,
        scan: &DeclarationScan,
    ) -> &'a crate::frontend::macro_names::DeclarationModuleBinding {
        let row = &scan.modules()[0];
        let rows = names
            .declaration_module_bindings(
                row.lexical_scope,
                &MacroName::parse(&row.name, scan.parse().profile.edition).unwrap(),
                &row.scope,
                row.scope.owner,
                |_| Ok(()),
            )
            .unwrap();
        assert_eq!(rows.len(), 1);
        rows[0]
    }
    // The stock file is unchanged; this control captures its headers under the
    // LRR Rust2024 profile, without claiming default-2015 compiletest acceptance.
    #[test]
    fn rustc_macro_paths_coordinator_keeps_original_headers_and_opaque_macro_arguments() {
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/macro-paths.rs"
        );
        let scan = scan(CASE);
        assert_eq!(
            scan.modules()
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            vec!["foo", "bar", "baz"]
        );
        let mut names = MacroNamespaces::default();
        SourceDiscovery::install_original_module_bindings(
            &mut names,
            std::iter::once(scan.as_ref()),
            &BTreeMap::new(),
        )
        .unwrap();
        for row in scan.modules() {
            let facts = names
                .declaration_module_bindings(
                    row.lexical_scope,
                    &MacroName::parse(&row.name, 2024).unwrap(),
                    &row.scope,
                    row.scope.owner,
                    |_| Ok(()),
                )
                .unwrap();
            assert_eq!(facts.len(), 1);
            assert_eq!(facts[0].identity, row.identity);
            assert_eq!(facts[0].origin, DeclarationModuleOrigin::Original);
            assert_eq!(facts[0].pending, None);
        }
        assert_eq!(scan.modules()[1].scope.module_path, vec!["foo"]);
    }
    #[test]
    fn original_module_coordinator_retains_source_identity_without_publishing_route() {
        let scan = scan("pub mod kept {} fn cold() { mod local {} }");
        let mut names = MacroNamespaces::default();
        SourceDiscovery::install_original_module_bindings(
            &mut names,
            std::iter::once(scan.as_ref()),
            &BTreeMap::new(),
        )
        .unwrap();
        let fact = binding(&names, &scan);
        assert_eq!(fact.identity, scan.modules()[0].identity);
        assert_eq!(fact.origin, DeclarationModuleOrigin::Original);
        assert_eq!(fact.pending, None);
        for row in scan.modules() {
            assert!(matches!(
                names
                    .lookup_module_from(
                        row.scope.crate_key,
                        &[MacroName::parse(&row.name, 2024).unwrap()],
                        &row.scope
                    )
                    .unwrap(),
                ModuleLookup::Absent
            ));
        }
    }
    // Builtin inert attributes (`allow`, `doc`) are not declaration
    // producers, so no inert proof releases a module annotation for them.
}

#[cfg(test)]
mod required_item_admission_tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        db::{
            dependency::InputKey,
            key::{OutlineKey, QueryKey},
            memo::Database,
        },
        frontend::compile_input_catalog::CompileInputCatalog,
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    use std::sync::Arc;

    fn capture(text: &str) -> (SourceDiscovery, PreparedSourceInputs) {
        let profile = ProfileIdentity::current();
        let mut cfg = CfgSet::guest_default();
        cfg.atoms.insert("test".into());
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        let path = SourcePath::new("required-items.rs").unwrap();
        let unit = Digest::of_bytes(b"required item admission regression");
        let selection = SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let inputs = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
            panic!("captured source absent");
        };
        let work = SourceDiscovery::for_tests(
            vec![view.share()],
            view.outline().root_scope.clone(),
            Digest::of_bytes(b"required item admission graph"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest([73; 32]),
            Digest([74; 32]),
            Arc::new(CompileInputCatalog::standalone(&ProfileIdentity::current()).unwrap()),
        )
        .unwrap();
        (work, inputs)
    }
    fn request(
        work: &mut SourceDiscovery,
    ) -> Arc<super::super::source_declarative::SourceDeclarativeRequest> {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => request,
            _ => panic!("required source expansion absent"),
        }
    }
    #[test]
    fn generated_impl_slot_test_target_is_rejected_before_scope_sealing() {
        // An associated output has MACRO_ITEMS syntax but retains its captured
        // insertion scope. The official test-attrs method position is invalid.
        let (mut work, inputs) = capture(
            "macro_rules! emit { () => { #[test] fn new() {} }; } struct A; fn cold() { impl A { emit!(); } }",
        );
        let demand = request(&mut work);
        assert_eq!(
            demand.key().grammar,
            super::super::identity::GrammarSlot::ImplItems
        );
        work.admit_expansion(Arc::new(demand.expand().unwrap()))
            .unwrap();
        let result = work.poll();
        if let Ok(SourceDiscoveryStep::Sealed(_)) = &result {
            let mut observation = Database::default();
            observation.trace.detailed = true;
            inputs.publish(&mut observation).unwrap();
            observation.begin_expansion_epoch(1).unwrap();
            work.publish(&mut observation).unwrap();
            let catalog = work.test_catalog(&mut observation).unwrap();
            eprintln!(
                "actual associated ImplItems output: sealed=true, catalog_cases={}, expansions={}, body_queries={}",
                catalog.cases().len(),
                work.accounted_expansion_count().unwrap(),
                observation
                    .trace
                    .events
                    .iter()
                    .filter(|event| matches!(
                        event.key,
                        QueryKey::FunctionSignature(_)
                            | QueryKey::BodyStructure(_)
                            | QueryKey::Thunk(_)
                    ))
                    .count()
            );
        }
        let error = result
            .err()
            .expect("associated output Test was silently retired");
        assert_eq!(error.code, "LRR-E-TEST-DISCOVERY");
        assert_eq!(
            error.message,
            "compiler Test attribute requires a free function"
        );
        assert_eq!(work.poll().err().unwrap().code, error.code);
        assert!(work.seal.is_none());
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        assert_eq!(work.publish(&mut db).unwrap_err().code, error.code);
        assert_eq!(work.test_catalog(&mut db).unwrap_err().code, error.code);
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }
    #[test]
    fn generated_statement_slot_local_test_keeps_free_function_eligibility() {
        let (mut work, inputs) = capture(
            "macro_rules! emit { () => { #[test] fn local() {} }; } fn cold() { emit!(); }",
        );
        let demand = request(&mut work);
        assert_eq!(
            demand.key().grammar,
            super::super::identity::GrammarSlot::Statements
        );
        work.admit_expansion(Arc::new(demand.expand().unwrap()))
            .unwrap();
        assert!(matches!(
            work.poll().unwrap(),
            SourceDiscoveryStep::Sealed(_)
        ));
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        work.publish(&mut db).unwrap();
        assert!(work.test_catalog(&mut db).unwrap().cases().is_empty());
        assert_eq!(work.accounted_expansion_count().unwrap(), 1);
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }
    #[test]
    fn generated_trait_slot_test_target_is_rejected_before_scope_sealing() {
        let (mut work, inputs) = capture(
            "macro_rules! emit { () => { #[test] fn new(); }; } fn cold() { trait A { emit!(); } }",
        );
        let demand = request(&mut work);
        assert_eq!(
            demand.key().grammar,
            super::super::identity::GrammarSlot::TraitItems
        );
        work.admit_expansion(Arc::new(demand.expand().unwrap()))
            .unwrap();
        let error = work
            .poll()
            .err()
            .expect("generated trait Test was silently retired");
        assert_eq!(error.code, "LRR-E-TEST-DISCOVERY");
        assert_eq!(
            error.message,
            "compiler Test attribute requires a free function"
        );
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        for repeated in [
            work.poll().err().unwrap(),
            work.publish(&mut db).unwrap_err(),
            work.test_catalog(&mut db).unwrap_err(),
        ] {
            assert_eq!(repeated.code, error.code);
            assert_eq!(repeated.message, error.message);
        }
        assert!(
            db.input(&InputKey::DiscoverySeal(work.key().clone()))
                .is_none()
        );
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }
    #[test]
    fn generated_associated_output_nested_local_test_keeps_free_function_eligibility() {
        let (mut work, inputs) = capture(
            "macro_rules! emit { () => { fn method() { #[test] fn local() {} } }; } struct A; fn cold() { impl A { emit!(); } }",
        );
        let demand = request(&mut work);
        assert_eq!(
            demand.key().grammar,
            super::super::identity::GrammarSlot::ImplItems
        );
        work.admit_expansion(Arc::new(demand.expand().unwrap()))
            .unwrap();
        assert!(matches!(
            work.poll().unwrap(),
            SourceDiscoveryStep::Sealed(_)
        ));
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        work.publish(&mut db).unwrap();
        assert!(work.test_catalog(&mut db).unwrap().cases().is_empty());
        assert_eq!(work.accounted_expansion_count().unwrap(), 1);
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }
    #[test]
    fn generated_associated_test_target_refusal_retains_primary_failure() {
        // Minimal associated attachment from the official test-attrs fixture.
        let (mut work, inputs) = capture(
            "macro_rules! emit { () => { struct A; impl A { #[test] fn recovery_witness() {} } }; } emit!(); #[test] fn known() {}",
        );
        let demand = request(&mut work);
        work.admit_expansion(Arc::new(demand.expand().unwrap()))
            .unwrap();
        let primary = work
            .poll()
            .err()
            .expect("generated associated Test remained unresolved");
        assert_eq!(primary.code, "LRR-E-TEST-DISCOVERY");
        assert_eq!(
            primary.message,
            "compiler Test attribute requires a free function"
        );
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        let revision = db.revision();
        for error in [
            work.poll().err().unwrap(),
            work.publish(&mut db).unwrap_err(),
            work.test_catalog(&mut db).unwrap_err(),
        ] {
            assert_eq!(error.code, primary.code);
            assert_eq!(error.message, primary.message);
        }
        assert_eq!(db.revision(), revision);
        assert_eq!(work.accounted_expansion_count().unwrap(), 1);
        assert!(
            db.input(&InputKey::DiscoverySeal(work.key().clone()))
                .is_none()
        );
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }
    #[test]
    fn generated_non_function_test_target_refusal_retains_primary_failure() {
        for source in [
            "macro_rules! emit { () => { #[test] const FOO: i32 = -1_i32; }; } emit!();",
            "macro_rules! emit { () => { #[test] mod test {} }; } emit!();",
        ] {
            let (mut work, inputs) = capture(source);
            let demand = request(&mut work);
            work.admit_expansion(Arc::new(demand.expand().unwrap()))
                .unwrap();
            let primary = work
                .poll()
                .err()
                .expect("invalid generated Test stayed pending");
            assert_eq!(primary.code, "LRR-E-TEST-DISCOVERY");
            assert_eq!(
                primary.message,
                "compiler Test attribute requires a free function"
            );
            let mut db = Database::default();
            db.trace.detailed = true;
            inputs.publish(&mut db).unwrap();
            db.begin_expansion_epoch(1).unwrap();
            let revision = db.revision();
            for error in [
                work.poll().err().unwrap(),
                work.publish(&mut db).unwrap_err(),
                work.test_catalog(&mut db).unwrap_err(),
            ] {
                assert_eq!(error.code, primary.code);
                assert_eq!(error.message, primary.message);
            }
            assert_eq!(db.revision(), revision);
            assert_eq!(work.accounted_expansion_count().unwrap(), 1);
            assert!(
                db.input(&InputKey::DiscoverySeal(work.key().clone()))
                    .is_none()
            );
            assert!(db.trace.events.iter().all(|event| !matches!(
                event.key,
                QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
            )));
        }
    }
    #[test]
    fn original_non_function_test_targets_refuse_before_any_catalog() {
        // Exact declarations selected from official test-on-not-fn.rs.
        for source in ["#[test] mod test {}", "#[test] const FOO: i32 = -1_i32;"] {
            let (mut work, _) = capture(source);
            let error = work
                .poll()
                .err()
                .expect("invalid Test target sealed an empty catalog");
            assert_eq!(error.code, "LRR-E-TEST-DISCOVERY");
            assert_eq!(
                error.message,
                "compiler Test attribute requires a free function"
            );
        }
    }
    #[test]
    fn associated_trait_and_cfg_attr_test_targets_are_not_free_functions() {
        for source in [
            "trait A { #[test] fn witness(); } #[test] fn known() {}",
            "struct A; impl A { #[cfg_attr(test, test)] fn witness() {} } #[test] fn known() {}",
        ] {
            let (mut work, _) = capture(source);
            let error = work
                .poll()
                .err()
                .expect("associated compiler Test was retired");
            assert_eq!(error.code, "LRR-E-TEST-DISCOVERY");
            assert_eq!(
                error.message,
                "compiler Test attribute requires a free function"
            );
        }
    }
    #[test]
    fn cfg_disabled_associated_test_does_not_reject_active_catalog() {
        let (mut work, inputs) = capture(
            "struct A; impl A { #[cfg(any())] #[test] fn witness() {} } #[test] fn known() {}",
        );
        assert!(matches!(
            work.poll().unwrap(),
            SourceDiscoveryStep::Sealed(_)
        ));
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        work.publish(&mut db).unwrap();
        let catalog = work.test_catalog(&mut db).unwrap();
        assert_eq!(
            catalog
                .cases()
                .iter()
                .map(|case| case.name.as_str())
                .collect::<Vec<_>>(),
            ["known"]
        );
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }
    #[test]
    fn ineligible_compiler_test_method_refuses_discovery_before_catalog_seal() {
        // Minimal attachment from the official non-associated-functions case.
        let (mut work, inputs) =
            capture("struct A; impl A { #[test] fn recovery_witness() {} } #[test] fn known() {}");
        let primary = work
            .poll()
            .err()
            .expect("invalid Test target silently sealed");
        assert_eq!(primary.code, "LRR-E-TEST-DISCOVERY");
        assert_eq!(
            primary.message,
            "compiler Test attribute requires a free function"
        );
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        let revision = db.revision();
        for error in [
            work.poll().err().expect("invalid target rediscovered"),
            work.publish(&mut db).unwrap_err(),
            work.test_catalog(&mut db).unwrap_err(),
        ] {
            assert_eq!(error.code, primary.code);
            assert_eq!(error.message, primary.message);
        }
        assert_eq!(db.revision(), revision);
        assert!(
            db.input(&InputKey::DiscoverySeal(work.key().clone()))
                .is_none()
        );
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }
    #[test]
    fn malformed_exact_items_reply_retains_primary_failure_and_cannot_publish_catalog() {
        let (mut work, inputs) = capture(
            "macro_rules! broken { () => { struct A { inner } }; } broken!(); #[test] fn known() {}",
        );
        let demand = request(&mut work);
        let record = Arc::new(demand.expand().unwrap());
        let primary = work.admit_expansion(record.clone()).unwrap_err();
        assert_eq!(primary.code, "LRR-E-MACRO-OUTPUT");
        assert!(primary.message.starts_with("invalid Items expansion: "));
        for _ in 0..2 {
            let repeated = work
                .poll()
                .err()
                .expect("failed mandatory expansion was offered again");
            assert_eq!(repeated.code, primary.code);
            assert_eq!(repeated.message, primary.message);
        }
        let repeated = work.admit_expansion(record).unwrap_err();
        assert_eq!(repeated.code, primary.code);
        assert_eq!(repeated.message, primary.message);
        assert_eq!(work.accounted_expansion_count().unwrap(), 0);
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        let revision = db.revision();
        for error in [
            work.sealed_expansions().err().unwrap(),
            work.publish(&mut db).unwrap_err(),
            work.test_catalog(&mut db).unwrap_err(),
        ] {
            assert_eq!(error.code, primary.code);
            assert_eq!(error.message, primary.message);
        }
        assert_eq!(db.revision(), revision);
        assert!(
            db.input(&InputKey::DiscoverySeal(work.key().clone()))
                .is_none()
        );
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }
    #[test]
    fn foreign_items_reply_does_not_poison_the_owned_request() {
        let (mut work, inputs) =
            capture("macro_rules! correct { () => { #[test] fn known() {} }; } correct!();");
        let demand = request(&mut work);
        let (mut other, _) =
            capture("macro_rules! other { () => { struct A { inner } }; } other!();");
        let foreign = request(&mut other);
        let error = work
            .admit_expansion(Arc::new(foreign.expand().unwrap()))
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-EXPANSION-INPUT");
        assert_eq!(
            error.message,
            "source expansion reply does not match exact pending demand"
        );
        assert_eq!(request(&mut work).key(), demand.key());
        assert_eq!(work.accounted_expansion_count().unwrap(), 0);
        work.admit_expansion(Arc::new(demand.expand().unwrap()))
            .unwrap();
        assert!(matches!(
            work.poll().unwrap(),
            SourceDiscoveryStep::Sealed(_)
        ));
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        work.publish(&mut db).unwrap();
        let catalog = work.test_catalog(&mut db).unwrap();
        assert_eq!(
            catalog
                .cases()
                .iter()
                .map(|case| case.name.as_str())
                .collect::<Vec<_>>(),
            ["known"]
        );
    }
    #[test]
    fn malformed_nested_generated_items_reply_keeps_the_primary_failure() {
        let (mut work, inputs) = capture(
            "macro_rules! broken { () => { struct A { inner } }; } macro_rules! outer { () => { broken!(); }; } outer!(); #[test] fn known() {}",
        );
        let outer = request(&mut work);
        work.admit_expansion(Arc::new(outer.expand().unwrap()))
            .unwrap();
        assert_eq!(work.accounted_expansion_count().unwrap(), 1);
        let generated = match work.poll().unwrap() {
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                Arc::new(request.expand().unwrap())
            }
            _ => panic!("nested generated item demand absent"),
        };
        let (mut other, _) =
            capture("macro_rules! foreign { () => { struct Other { inner } }; } foreign!();");
        let foreign = request(&mut other);
        let foreign_error = work
            .admit_expansion(Arc::new(foreign.expand().unwrap()))
            .unwrap_err();
        assert_eq!(foreign_error.code, "LRR-E-EXPANSION-INPUT");
        assert_eq!(
            foreign_error.message,
            "generated reply differs from exact pending demand"
        );
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                assert_eq!(request.key(), generated.key())
            }
            _ => panic!("foreign reply poisoned the generated pending demand"),
        }
        assert_eq!(work.accounted_expansion_count().unwrap(), 1);
        let primary = work.admit_expansion(generated.clone()).unwrap_err();
        assert_eq!(primary.code, "LRR-E-MACRO-OUTPUT");
        assert!(primary.message.starts_with("invalid Items expansion: "));
        match work.poll() {
            Err(repeated) => {
                assert_eq!(repeated.code, primary.code);
                assert_eq!(repeated.message, primary.message);
            }
            Ok(SourceDiscoveryStep::NeedGeneratedExpansion { request }) => panic!(
                "rejected immutable generated Items reply was offered again: {}",
                request.key().fingerprint().hex()
            ),
            Ok(_) => panic!("failed nested generated reply returned successful discovery work"),
        }
        let repeated = work.admit_expansion(generated).unwrap_err();
        assert_eq!(repeated.code, primary.code);
        assert_eq!(repeated.message, primary.message);
        assert_eq!(work.accounted_expansion_count().unwrap(), 1);
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        let revision = db.revision();
        for error in [
            work.sealed_expansions().err().unwrap(),
            work.publish(&mut db).unwrap_err(),
            work.test_catalog(&mut db).unwrap_err(),
        ] {
            assert_eq!(error.code, primary.code);
            assert_eq!(error.message, primary.message);
        }
        assert_eq!(db.revision(), revision);
        assert!(
            db.input(&InputKey::DiscoverySeal(work.key().clone()))
                .is_none()
        );
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }
}

#[cfg(test)]
#[path = "source_discovery/original_inert_tests.rs"]
mod original_inert_tests;

#[cfg(test)]
#[path = "source_discovery/compiler_profiles_tests.rs"]
mod compiler_profiles_tests;
