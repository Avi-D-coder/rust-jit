//! The SAME compiler/source route preparation can demand a real physical
//! module before installation. An unresolved graph never becomes runnable.
use super::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum OrdinaryCompilerModuleHold {
    MissingDeclaration,
    ForeignUnit(Digest),
    ConfiguredSourceRole(Digest),
    ParentLineage(Digest),
}
/// An actual retained declaration/caller request. No replacement constructors.
pub(crate) struct OrdinaryCompilerModuleCapture {
    caller: crate::source::changes::SourceSelection,
    original: PreparedSourceView,
    input: crate::db::poll::InputRequest,
}
impl OrdinaryCompilerModuleCapture {
    pub(crate) fn caller(&self) -> &crate::source::changes::SourceSelection {
        &self.caller
    }
    pub(crate) fn original(&self) -> &PreparedSourceView {
        &self.original
    }
    pub(crate) fn input(&self) -> &crate::db::poll::InputRequest {
        &self.input
    }
}
pub(crate) enum PendingOrdinaryCompilerModule {
    Capture(OrdinaryCompilerModuleCapture),
    Held(OrdinaryCompilerModuleHold),
}

pub(super) struct PendingCompilerConfiguration {
    pub(super) compiler:
        Option<std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>>,
    procedural: Option<std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>>,
    pub(super) procedural_consumer: Option<Digest>,
    pub(super) compiler_attributes: bool,
    routes: Option<PendingCompilerRoutes>,
    demand: Option<crate::frontend::source_macro_routes::ModulePreparationDemand>,
}
struct PendingCompilerRoutes {
    providers: Vec<RouteInstallProvider>,
    modules: BTreeMap<
        Digest,
        Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
    >,
    sources: Vec<(usize, Digest)>,
    inert: Vec<(Digest, Digest, Digest)>,
    routes: Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
}
impl PendingCompilerRoutes {
    fn matches(&self, discovery: &SourceDiscovery) -> bool {
        let Some((_, _, providers)) = &discovery.declarative else {
            return false;
        };
        self.providers.len() == providers.len()
            && self.providers.iter().zip(providers).all(|(a, b)| a.same(b))
            && self.modules.len() == discovery.module_routes.len()
            && self
                .modules
                .iter()
                .zip(&discovery.module_routes)
                .all(|((a, rows), (b, actual))| {
                    a == b
                        && rows.len() == actual.len()
                        && rows
                            .iter()
                            .zip(actual)
                            .all(|(a, b)| std::sync::Arc::ptr_eq(a, b) || a.same_routes(b))
                })
            && self.sources == RouteInstallMemo::source_rows(&discovery.nested_sources)
            && self.inert == RouteInstallMemo::inert_rows(&discovery.nested_inert_proofs)
    }
}
impl PendingCompilerConfiguration {
    pub(super) fn new(
        compiler: Option<
            std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>,
        >,
        procedural: Option<
            std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
        >,
        compiler_attributes: bool,
    ) -> Self {
        let procedural_consumer = procedural.as_ref().map(|catalog| catalog.consumer());
        Self {
            compiler,
            procedural,
            procedural_consumer,
            compiler_attributes,
            routes: None,
            demand: None,
        }
    }
    pub(super) fn cached_demand(
        &self,
        discovery: &SourceDiscovery,
    ) -> Option<crate::frontend::source_macro_routes::ModulePreparationDemand> {
        if !self.routes.as_ref()?.matches(discovery) {
            return None;
        }
        self.demand.as_ref().map(|demand| {
            crate::frontend::source_macro_routes::ModulePreparationDemand {
                import: demand.import.clone(),
                missing: demand.missing.clone(),
            }
        })
    }
    pub(super) fn retain_demand(
        &mut self,
        demand: &crate::frontend::source_macro_routes::ModulePreparationDemand,
    ) {
        self.demand = Some(
            crate::frontend::source_macro_routes::ModulePreparationDemand {
                import: demand.import.clone(),
                missing: demand.missing.clone(),
            },
        );
    }
    pub(super) fn clear_demand(&mut self) {
        self.demand = None;
    }
    pub(super) fn routes(
        &mut self,
        discovery: &SourceDiscovery,
    ) -> Result<
        &[std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>],
        Diagnostic,
    > {
        if !self
            .routes
            .as_ref()
            .is_some_and(|routes| routes.matches(discovery))
        {
            let (_, _, providers) = discovery
                .declarative
                .as_ref()
                .ok_or_else(|| fault("pending compiler providers are absent"))?;
            let routes =
                SourceDiscovery::completed_macro_routes(providers, &discovery.module_routes)?;
            self.routes = Some(PendingCompilerRoutes {
                providers: providers.iter().map(RouteInstallProvider::of).collect(),
                modules: discovery.module_routes.clone(),
                sources: RouteInstallMemo::source_rows(&discovery.nested_sources),
                inert: RouteInstallMemo::inert_rows(&discovery.nested_inert_proofs),
                routes,
            });
            self.demand = None;
        }
        Ok(&self.routes.as_ref().expect("retained route inputs").routes)
    }
}

pub(super) enum RouteInstallPreparation {
    Ready(std::sync::Arc<crate::frontend::macro_names::MacroNamespaces>),
    NeedModule(crate::frontend::source_macro_routes::ModulePreparationDemand),
}
impl RouteInstallPreparation {
    pub(super) fn require_ready(
        self,
    ) -> Result<std::sync::Arc<crate::frontend::macro_names::MacroNamespaces>, Diagnostic> {
        match self {
            Self::Ready(namespaces) => Ok(namespaces),
            Self::NeedModule(demand) => Err(demand.diagnostic()),
        }
    }
}
pub(crate) enum SourceCompilerPreparation {
    Ready(SourceDiscovery),
    NeedModule {
        discovery: SourceDiscovery,
        demand: crate::frontend::source_macro_routes::ModulePreparationDemand,
    },
}
impl SourceCompilerPreparation {
    pub(super) fn require_ready(self) -> Result<SourceDiscovery, Diagnostic> {
        match self {
            Self::Ready(discovery) => Ok(discovery),
            Self::NeedModule { demand, .. } => Err(demand.diagnostic()),
        }
    }
}

impl SourceDiscovery {
    /// Only the retained actual compiler-phase demand can choose the next
    /// physical source. Already staged ancestors are not new capture work.
    pub(crate) fn prepare_pending_compiler_ordinary_module(
        &self,
        db: &mut crate::db::memo::Database,
        owning_unit: Digest,
        authorities: &crate::source::include_paths::IncludeAuthorities,
    ) -> crate::db::poll::QueryPoll<PendingOrdinaryCompilerModule> {
        use crate::db::{poll::QueryPoll, provider::SyntaxProvider};
        let selected: Result<_, Diagnostic> = (|| {
            db.require_coordinator()?;
            let pending = self
                .compiler_preparation
                .as_ref()
                .ok_or_else(|| fault("ordinary compiler module requires retained preparation"))?;
            let demand = pending.cached_demand(self).ok_or_else(|| {
                fault("compiler module demand must resume its actual route inputs")
            })?;
            let Some(declaration) = demand.declaration(
                self.unresolved_ordinary_module_sources()
                    .filter(|row| self.module_source_progress(row.producer()).is_none()),
            )?
            else {
                return Ok(Err(OrdinaryCompilerModuleHold::MissingDeclaration));
            };
            let producer = declaration.producer();
            if declaration.scope().crate_key != owning_unit {
                return Ok(Err(OrdinaryCompilerModuleHold::ForeignUnit(
                    declaration.scope().crate_key,
                )));
            }
            let (parent, actual) = self.ordinary_module_binding(producer)?;
            if actual.fingerprint() != declaration.fingerprint() {
                return Err(fault("ordinary pending compiler declaration differs"));
            }
            // The original source owner must be from the same already-open
            // service preparation. Follow actual immutable staging ancestry.
            let mut ancestor = parent;
            let original = loop {
                match ancestor {
                    OrdinaryModuleOwner::Original(ordinal) => {
                        if self.macro_export_sources.contains_key(&ordinal) {
                            return Ok(Err(OrdinaryCompilerModuleHold::ConfiguredSourceRole(
                                producer,
                            )));
                        }
                        break self
                            .sources
                            .get(ordinal)
                            .ok_or_else(|| fault("ordinary original source owner absent"))?
                            .share();
                    }
                    OrdinaryModuleOwner::Staged(parent) => {
                        let staged = self
                            .staged_modules
                            .get(&parent)
                            .ok_or_else(|| fault("ordinary staged ancestor absent"))?;
                        let StagedModuleSyntax::Ordinary { parent, .. } = &staged.syntax else {
                            return Ok(Err(OrdinaryCompilerModuleHold::ParentLineage(producer)));
                        };
                        ancestor = *parent;
                    }
                }
            };
            let caller = self.ordinary_module_capture_caller(producer)?;
            self.require_ordinary_module_parent_current(db, producer, parent, declaration)?;
            let input = self.prepare_ordinary_module_source(producer, authorities)?;
            let expected = self
                .declarative
                .as_ref()
                .ok_or_else(|| fault("ordinary compiler environment absent"))?
                .0;
            Ok(Ok((
                caller,
                original,
                input,
                expected,
                declaration.scope().crate_key,
            )))
        })();
        let (caller, original, input, expected, unit) = match selected {
            Ok(Ok(actual)) => actual,
            Ok(Err(why)) => return QueryPoll::Ready(Ok(PendingOrdinaryCompilerModule::Held(why))),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match SyntaxProvider::compile_environment(db, unit) {
            QueryPoll::Ready(Ok(environment)) if environment.fingerprint() == expected => {
                QueryPoll::Ready(Ok(PendingOrdinaryCompilerModule::Capture(
                    OrdinaryCompilerModuleCapture {
                        caller,
                        original,
                        input,
                    },
                )))
            }
            QueryPoll::Ready(Ok(_)) => {
                QueryPoll::Ready(Err(fault("pending ordinary compiler environment changed")))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    pub(super) fn require_same_compiler_preparation(
        &self,
        environment: Digest,
        engine: Digest,
        compiler: &Option<
            std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>,
        >,
        procedural: &Option<
            std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
        >,
    ) -> Result<(), Diagnostic> {
        fn same<T>(a: &Option<std::sync::Arc<T>>, b: &Option<std::sync::Arc<T>>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
                _ => false,
            }
        }
        let pending = self
            .compiler_preparation
            .as_ref()
            .ok_or_else(|| fault("retained compiler preparation is absent"))?;
        let (actual_environment, actual_engine, _) = self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("retained compiler source context is absent"))?;
        if *actual_environment != environment
            || *actual_engine != engine
            || !same(&pending.compiler, compiler)
            || !same(&pending.procedural, procedural)
        {
            return Err(fault(
                "repeated compiler preparation changes its actual configuration",
            ));
        }
        Ok(())
    }
    /// Resume only the retained actual configuration. No replacement source,
    /// compiler, environment, procedural catalog or readiness token is accepted.
    pub(crate) fn resume_compiler_preparation(
        mut self,
    ) -> Result<SourceCompilerPreparation, Diagnostic> {
        if self.started || self.seal.is_some() || self.failed.is_some() {
            return Err(fault(
                "compiler preparation cannot resume a started, sealed or failed discovery",
            ));
        }
        let mut pending = self
            .compiler_preparation
            .take()
            .ok_or_else(|| fault("retained compiler preparation is absent"))?;
        if let Some(demand) = pending.cached_demand(&self) {
            self.compiler_preparation = Some(pending);
            return Ok(SourceCompilerPreparation::NeedModule {
                discovery: self,
                demand,
            });
        }
        let (environment, engine, _) = self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("retained compiler source context is absent"))?;
        // This key is recomputed after any real module adoption. A pre-adoption
        // key must never cache a graph containing unstamped adopted sources.
        let configured_key = self.configured_graph_key(
            *environment,
            *engine,
            pending.compiler.as_ref(),
            pending.procedural_consumer.is_some(),
            pending.compiler_attributes,
        )?;
        if let Some(key) = configured_key
            && let Some(hit) = self.sources[0].publication_prepared(key)
            && let Ok(hit) = hit.downcast::<ConfiguredGraph>()
        {
            let providers = &self.declarative.as_ref().expect("retained context").2;
            self.graph = std::sync::Arc::new(hit.graph.clone());
            self.graph_deferred = false;
            self.route_install = RouteInstallMemo {
                providers: providers.iter().map(RouteInstallProvider::of).collect(),
                sources: RouteInstallMemo::source_rows(&self.nested_sources),
                inert: RouteInstallMemo::inert_rows(&self.nested_inert_proofs),
                routes: hit.routes.clone(),
                namespaces: Some(hit.namespaces.clone()),
            };
            self.imports = hit.imports.clone();
            self.imports_deferred = false;
            return self.finish_compiler_preparation(pending);
        }
        self.build_deferred_graph()?;
        let prepared = {
            let routes = pending.routes(&self)?;
            Self::prepare_install_macro_routes(
                unique_graph(&mut self.graph)?,
                &self.declarative.as_ref().expect("retained context").2,
                routes,
                &self.nested_sources,
                &self.nested_inert_proofs,
                &mut self.route_install,
            )?
        };
        match prepared {
            RouteInstallPreparation::NeedModule(demand) => {
                pending.retain_demand(&demand);
                self.compiler_preparation = Some(pending);
                return Ok(SourceCompilerPreparation::NeedModule {
                    discovery: self,
                    demand,
                });
            }
            RouteInstallPreparation::Ready(_) => {}
        }
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
                    _catalog: pending.compiler.clone(),
                }),
            );
        }
        self.finish_compiler_preparation(pending)
    }
    /// Read-only actual preparation demand, never a declaration/capture grant.
    pub(crate) fn pending_compiler_module_demand(
        &self,
    ) -> Option<&crate::frontend::source_macro_routes::ModulePreparationDemand> {
        self.compiler_preparation.as_ref()?.demand.as_ref()
    }
    fn finish_compiler_preparation(
        mut self,
        pending: PendingCompilerConfiguration,
    ) -> Result<SourceCompilerPreparation, Diagnostic> {
        if self.compiler_preparation.is_some()
            || self.route_install.namespaces.is_none()
            || self.imports_deferred
        {
            return Err(fault(
                "compiler preparation finalizer requires actual installed routes",
            ));
        }
        if let Some(consumer) = pending.procedural_consumer {
            self.reserve_test_attribute_candidates(Some(consumer), false)?;
        }
        if pending.compiler_attributes {
            self.reserve_compiler_test_attribute_candidates()?;
        }
        Ok(SourceCompilerPreparation::Ready(self))
    }
    /// The same route installer, with typed missing-module retention during
    /// configuration. Ordinary immediate-completion callers keep their wrapper.
    pub(super) fn install_module_routes_for_preparation(&mut self) -> Result<(), Diagnostic> {
        let Some(mut pending) = self.compiler_preparation.take() else {
            Self::install_macro_routes_with_modules(
                unique_graph(&mut self.graph)?,
                &self
                    .declarative
                    .as_ref()
                    .ok_or_else(|| fault("module providers absent"))?
                    .2,
                &self.module_routes,
                &self.nested_sources,
                &self.nested_inert_proofs,
                &mut self.route_install,
            )?;
            return Ok(());
        };
        let result = (|| {
            let routes = pending.routes(self)?;
            Self::prepare_install_macro_routes(
                unique_graph(&mut self.graph)?,
                &self
                    .declarative
                    .as_ref()
                    .ok_or_else(|| fault("module providers absent"))?
                    .2,
                routes,
                &self.nested_sources,
                &self.nested_inert_proofs,
                &mut self.route_install,
            )
        })();
        match result {
            Ok(RouteInstallPreparation::NeedModule(demand)) => pending.retain_demand(&demand),
            Ok(RouteInstallPreparation::Ready(_)) => pending.clear_demand(),
            Err(error) => {
                self.compiler_preparation = Some(pending);
                return Err(error);
            }
        }
        self.compiler_preparation = Some(pending);
        Ok(())
    }
}
