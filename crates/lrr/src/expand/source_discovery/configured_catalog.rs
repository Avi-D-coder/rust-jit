//! Actual standalone source-world preparation and ordinary catalog closure.
//! Neither this source certificate nor its locators supplies typed solver facts.
use super::PreparedLiveImplCatalogSources;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        seals::{DiscoveryInput, Seal, SealDomain, SealKey},
    },
    expand::source_discovery::{SourceCompilerPreparation, SourceDiscovery},
    frontend::procedural_exports::PreparedProcCatalog,
    service::source_preparation::PreparedCompileInputs,
    source::{changes::{PreparedSourceView, SourceSelection}, manifest::Namespace},
};
use std::{collections::BTreeMap, sync::Arc};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-IMPL-CATALOG-WORLD", message.into())
}

/// Only the actual sealed catalog publisher can construct this borrowed scope
/// receipt. It authorizes immutable derived metadata, never executable entry.
pub(crate) struct CatalogDeclarationPublication<'a, 'source> {
    discovery: &'a SourceDiscovery,
    ordinal: usize,
    scope: &'a crate::frontend::associated_declarations::CapturedDeclarationScope<'source>,
    parent: &'a crate::frontend::impl_index::DeclarationMacroIndex,
    seal: &'a Seal,
    completions: &'a BTreeMap<Digest, Arc<crate::frontend::impl_index::CompletedOriginalAnnotation>>,
}
impl CatalogDeclarationPublication<'_, '_> {
    pub(crate) fn scope(&self) -> &crate::frontend::associated_declarations::CapturedDeclarationScope<'_> { self.scope }
    pub(crate) fn parent(&self) -> &crate::frontend::impl_index::DeclarationMacroIndex { self.parent }
    pub(crate) fn seal(&self) -> &Seal { self.seal }
    pub(crate) fn completions(&self) -> &BTreeMap<Digest, Arc<crate::frontend::impl_index::CompletedOriginalAnnotation>> { self.completions }
    pub(crate) fn require_current(&self, db: &mut Database) -> Result<(), Diagnostic> {
        db.require_coordinator()?;
        let discovery = self.discovery;
        let plan = discovery.impl_catalog_plan.as_ref()
            .ok_or_else(|| fault("catalog metadata has no actual configured plan"))?;
        let source = discovery.sources.get(self.ordinal)
            .ok_or_else(|| fault("catalog metadata source ordinal is absent"))?;
        let Some((environment, engine, _)) = &discovery.declarative else {
            return Err(fault("catalog metadata compiler routes are absent"));
        };
        if !matches!(plan.key.domain, SealDomain::ImplCatalog { .. })
            || discovery.key != plan.key || self.seal.key() != plan.key
            || !discovery.seal.as_ref().is_some_and(|seal| std::ptr::eq(seal, self.seal))
            || !discovery.nested_sources.get(&self.ordinal).is_some_and(|parent| std::ptr::eq(parent.as_ref(), self.parent))
            || !source.same_publication(self.scope.source())
            || discovery.macro_export_sources.contains_key(&self.ordinal)
            || *environment != plan.environment || *engine != plan.engine
            || discovery.dependencies.fingerprint() != plan.source_dependencies
            || discovery.compiler_preparation.is_some() || discovery.compiler_profiles.is_none()
            || !discovery.producers.is_empty() || !self.parent.is_complete()
            || discovery.nested_sources.iter().any(|(ordinal, index)|
                !discovery.macro_export_sources.contains_key(ordinal) && !index.is_complete())
            || discovery.completed_argument_indexes.iter().any(|index| !index.is_complete())
            || !discovery.imports.pending.is_empty() || !discovery.imports.missing.is_empty()
            || !discovery.imports.inaccessible.is_empty() || !discovery.imports.ambiguous.is_empty()
        {
            return Err(fault("catalog metadata differs from its actual sealed source frontier"));
        }
        plan.key.validate()?;
        if !matches!(db.published_input(&InputKey::ExpansionEpoch), Some((InputValue::ExpansionEpoch(epoch), _)) if *epoch == plan.key.epoch)
            || !matches!(db.published_input(&InputKey::SourceDependencies), Some((InputValue::SourceDependencies(dependencies), _)) if dependencies.fingerprint() == plan.source_dependencies)
            || !matches!(db.published_input(&InputKey::UnitCompileEnv(plan.root.outline.crate_key)), Some((InputValue::UnitCompileEnv(actual), _)) if actual.unit() == plan.root.outline.crate_key && actual.fingerprint() == plan.environment)
        {
            return Err(fault("catalog metadata source/epoch/compiler inputs changed"));
        }
        for (key, stamp) in &discovery.compiler_inputs {
            if db.published_input(key).map(|(_, actual)| actual) != Some(*stamp) {
                return Err(fault("catalog metadata observed compiler input changed"));
            }
        }
        let catalogs = discovery.procedural_catalogs()?;
        if match plan.procedural {
            Some(expected) => !discovery.native_publication_certified
                || !catalogs.iter().any(|catalog| catalog.fingerprint() == expected),
            None => !catalogs.is_empty(),
        } {
            return Err(fault("catalog metadata procedural publication differs"));
        }
        self.scope.require_current(db)
    }
}

#[derive(Debug)]
pub(crate) struct ImplCatalogPlan {
    key: SealKey,
    root: SourceSelection,
    originals: Arc<[SourceSelection]>,
    generation: Digest,
    environment: Digest,
    engine: Digest,
    compiler: Digest,
    source_dependencies: Digest,
    source_closure: Digest,
    facade: Digest,
    procedural: Option<Digest>,
}
impl StableEncode for ImplCatalogPlan {
    const DOMAIN: &'static str = "lrr/v2/configured-standalone-impl-catalog-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(&QueryKey::Outline(self.root.outline.clone()));
        e.string(self.root.path.as_str());
        e.sequence(&self.originals, |e, source| {
            e.string(source.path.as_str());
            e.record(&QueryKey::Outline(source.outline.clone()));
        });
        e.digest(self.generation);
        e.digest(self.environment);
        e.digest(self.engine);
        e.digest(self.compiler);
        e.digest(self.source_dependencies);
        e.digest(self.source_closure);
        e.digest(self.facade);
        e.option(&self.procedural, |e, catalog| e.digest(*catalog));
    }
}

/// A dormant request over actual captured inputs. This is neither a completed
/// catalog nor a configured discovery, and has no constructor from a raw key.
pub(crate) struct PreparedImplCatalogDemand {
    plan: Arc<ImplCatalogPlan>,
    root: PreparedSourceView,
    sources: Vec<PreparedSourceView>,
    inputs: BTreeMap<InputKey, Digest>,
    procedural: Option<Arc<PreparedProcCatalog>>,
}
impl PreparedImplCatalogDemand {
    pub(crate) fn key(&self) -> &SealKey { &self.plan.key }
    pub(crate) fn procedural_catalog(&self) -> Option<&Arc<PreparedProcCatalog>> {
        self.procedural.as_ref()
    }
    pub(crate) fn require_current(
        &self,
        compiler: &PreparedCompileInputs,
        db: &Database,
    ) -> Result<(), Diagnostic> {
        db.require_coordinator()?;
        self.plan.key.validate()?;
        if compiler.impl_catalog_generation() != self.plan.generation
            || compiler.environment_identity() != self.plan.environment
            || compiler.catalog.owning_profile_identity()? != self.plan.compiler
            || crate::expand::source_declarative::engine_version() != self.plan.engine
        {
            return Err(fault("dormant catalog compiler configuration changed"));
        }
        for (key, expected) in &self.inputs {
            if db.published_input(key).map(|(_, stamp)| stamp) != Some(*expected) {
                return Err(fault("dormant catalog input changed or disappeared"));
            }
        }
        let current = db.prepared_source(&self.plan.root)?
            .ok_or_else(|| fault("dormant catalog source publication is absent"))?;
        if !current.same_publication(&self.root) {
            return Err(fault("dormant catalog source publication changed"));
        }
        if let Some(catalog) = &self.procedural {
            require_procedural(catalog, &self.plan.root, self.plan.key.epoch,
                self.plan.environment, self.plan.key.unit_graph)?;
        }
        if self.procedural.as_ref().map(|catalog| catalog.fingerprint()) != self.plan.procedural {
            return Err(fault("dormant catalog procedural identity changed"));
        }
        Ok(())
    }
    /// Enter the existing discovery machinery only on actual demand. Compiler
    /// and procedural route configuration remains the coordinator's next step.
    pub(crate) fn into_discovery(
        self,
        compiler: &PreparedCompileInputs,
        db: &Database,
    ) -> Result<SourceDiscovery, Diagnostic> {
        self.require_current(compiler, db)?;
        let scope = self.root.outline().root_scope.clone();
        let key = self.plan.key.clone();
        let environment_key = InputKey::UnitCompileEnv(self.plan.root.outline.crate_key);
        let stamp = *self.inputs.get(&environment_key)
            .ok_or_else(|| fault("dormant catalog compiler dependency is absent"))?;
        let mut discovery = SourceDiscovery::new_domain(
            self.sources, scope, Namespace::Value, key.unit_graph, key.epoch, key.domain, false,
        )?;
        discovery.impl_catalog_plan = Some(self.plan);
        // The ordinary work ledger must observe this before its fixed point.
        discovery.compiler_inputs.insert(environment_key, stamp);
        Ok(discovery)
    }
}
fn require_procedural(
    catalog: &PreparedProcCatalog,
    root: &SourceSelection,
    epoch: u64,
    environment: Digest,
    graph: Digest,
) -> Result<(), Diagnostic> {
    if catalog.consumer() != root.outline.crate_key || catalog.epoch() != epoch
        || catalog.environment() != environment || catalog.graph() != graph
    {
        return Err(fault("catalog procedural graph/consumer/epoch/environment differs"));
    }
    Ok(())
}

/// Private completion fields bind a real configured source plan to the common
/// discovery fixed point and entire actual immutable publication payload.
#[derive(Debug, Clone)]
pub(crate) struct ImplCatalogCompletion {
    plan: Arc<ImplCatalogPlan>,
    parent: Digest,
    payload: Digest,
}
impl ImplCatalogCompletion {
    pub(crate) fn root(&self) -> &SourceSelection { &self.plan.root }
    pub(crate) fn require_publication(&self, input: &DiscoveryInput) -> Result<(), Diagnostic> {
        if input.seal.key() != self.plan.key
            || input.seal.fingerprint() != self.parent
            || input.catalog_payload_fingerprint() != self.payload
        {
            return Err(fault(
                "source catalog completion differs from its actual publication",
            ));
        }
        Ok(())
    }
}
impl StableEncode for ImplCatalogCompletion {
    const DOMAIN: &'static str = "lrr/v1/impl-catalog-source-completion";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(self.plan.as_ref());
        e.digest(self.parent);
        e.digest(self.payload);
    }
}
impl ImplCatalogPlan {
    pub(in crate::expand::source_discovery) fn complete(
        self: &Arc<Self>,
        discovery: &SourceDiscovery,
        input: &mut DiscoveryInput,
        db: &crate::db::memo::Database,
    ) -> Result<ImplCatalogCompletion, Diagnostic> {
        let Some((environment, engine, _)) = &discovery.declarative else {
            return Err(fault("source catalog compiler routes are not prepared"));
        };
        if discovery.key != self.key
            || input.seal.key() != self.key
            || discovery.seal.as_ref().map(StableEncode::fingerprint)
                != Some(input.seal.fingerprint())
            || *environment != self.environment
            || *engine != self.engine
            || input.source_dependencies != self.source_dependencies
            || match self.procedural {
                Some(expected) => !input.procedural_catalogs.iter()
                    .any(|catalog| catalog.fingerprint() == expected),
                None => !input.procedural_catalogs.is_empty(),
            }
            || discovery.compiler_profiles.is_none()
            || !discovery.producers.is_empty()
            || discovery.nested_sources.iter().any(|(ordinal, index)| {
                !discovery.macro_export_sources.contains_key(ordinal) && !index.is_complete()
            })
            || discovery
                .completed_argument_indexes
                .iter()
                .any(|index| !index.is_complete())
            || !discovery.imports.pending.is_empty()
            || !discovery.imports.missing.is_empty()
            || !discovery.imports.inaccessible.is_empty()
            || !discovery.imports.ambiguous.is_empty()
        {
            return Err(fault(
                "source catalog retains an unprepared configured world or producer frontier",
            ));
        }
        // Even a catalog with no compiler macro invocation depends on the
        // actual owning environment. Bind its real published input stamp once.
        let key = crate::db::dependency::InputKey::UnitCompileEnv(self.root.outline.crate_key);
        let (value, stamp) = db
            .published_input(&key)
            .ok_or_else(|| fault("owning compiler environment is not published"))?;
        let crate::db::dependency::InputValue::UnitCompileEnv(environment) = value else {
            return Err(fault("owning compiler environment input differs"));
        };
        if environment.unit() != self.root.outline.crate_key
            || environment.fingerprint() != self.environment
        {
            return Err(fault(
                "owning compiler environment changed before catalog publication",
            ));
        }
        if input.compiler_inputs.get(&key) != Some(&stamp) {
            return Err(fault(
                "observed compiler input conflicts with the prepared catalog",
            ));
        }
        // All originally prepared units/files must remain in the actual final
        // closure. Additional module/include/generated sources enter only via
        // ordinary discovery's validated producer/publication mechanisms.
        let original: std::collections::BTreeSet<_> = input
            .sources
            .iter()
            .map(|source| {
                (
                    source.location_identity(),
                    QueryKey::Outline(source.outline.clone()).fingerprint(),
                )
            })
            .collect();
        for source in self.originals.iter() {
            if !original.contains(&(
                source.location_identity(),
                QueryKey::Outline(source.outline.clone()).fingerprint(),
            )) {
                return Err(fault("source catalog omits an actual configured source"));
            }
        }
        Ok(ImplCatalogCompletion {
            plan: self.clone(),
            parent: input.seal.fingerprint(),
            payload: input.catalog_payload_fingerprint(),
        })
    }
}
impl SourceDiscovery {
    /// Authenticate a dormant request using already captured immutable inputs.
    /// No name graph, compiler routes, discovery jobs or source IO are prepared.
    pub(crate) fn prepare_impl_catalog_demand(
        compiler: &PreparedCompileInputs,
        root: &PreparedSourceView,
        epoch: u64,
        procedural: Option<Arc<PreparedProcCatalog>>,
        db: &Database,
    ) -> Result<PreparedImplCatalogDemand, Diagnostic> {
        db.require_coordinator()?;
        let environment = compiler.environment_identity();
        let unit_graph = match &procedural {
            Some(catalog) => {
                require_procedural(catalog, root.selection(), epoch, environment, catalog.graph())?;
                catalog.graph()
            }
            None => root.selection().outline.crate_key,
        };
        let prepared = PreparedLiveImplCatalogSources::capture(
            root, unit_graph, epoch,
        )?;
        let super::LiveImplCatalogPreparation::NeedConfiguredWorld {
            captured: live,
            trait_facade: super::MandatoryTraitFacade::NeedHeaders { captured: facade },
        } = prepared
        else {
            return Err(fault(
                "actual source units or mandatory facade files are still missing",
            ));
        };
        let sources = compiler.impl_catalog_sources(&live, &facade)?;
        let root = sources
            .iter()
            .find(|source| source.selection() == live.selected_root())
            .ok_or_else(|| fault("configured source catalog root is absent"))?;
        let root_unit = root.selection().outline.crate_key;
        let mut originals: Vec<_> = sources
            .iter()
            .map(|source| source.selection().clone())
            .collect();
        originals.sort_by_key(|source| {
            (
                source.location_identity(),
                QueryKey::Outline(source.outline.clone()).fingerprint(),
            )
        });
        let generation = compiler.impl_catalog_generation();
        let engine = crate::expand::source_declarative::engine_version();
        let compiler_id = compiler.catalog.owning_profile_identity()?;
        let source_dependencies = live.source_dependencies().fingerprint();
        let mut graph = StableEncoder::new("lrr/v1/standalone-impl-catalog-source-closure");
        graph.digest(source_dependencies);
        graph.sequence(&originals, |e, source| {
            e.string(source.path.as_str());
            e.record(&source.outline);
        });
        let source_closure = graph.finish();
        let facade = facade.fingerprint();
        let procedural_identity = procedural.as_ref().map(|catalog| catalog.fingerprint());
        let mut identity = StableEncoder::new("lrr/v2/configured-impl-catalog-identity");
        identity.digest(unit_graph);
        identity.digest(source_closure);
        identity.digest(generation);
        identity.digest(environment);
        identity.digest(engine);
        identity.digest(compiler_id);
        identity.digest(facade);
        identity.option(&procedural_identity, |e, catalog| e.digest(*catalog));
        let key = SealKey {
            domain: SealDomain::ImplCatalog {
                id: identity.finish(),
            },
            unit_graph,
            epoch: live.graph_epoch().1,
        };
        key.validate()?;
        let plan = Arc::new(ImplCatalogPlan {
            key: key.clone(),
            root: live.selected_root().clone(),
            originals: originals.into(),
            generation,
            environment,
            engine,
            compiler: compiler_id,
            source_dependencies,
            source_closure,
            facade,
            procedural: procedural_identity,
        });
        let input_key = InputKey::UnitCompileEnv(root_unit);
        let (input, stamp) = db
            .published_input(&input_key)
            .ok_or_else(|| fault("actual owning compiler input is not published"))?;
        if !matches!(input, InputValue::UnitCompileEnv(actual) if actual.unit() == root_unit && actual.fingerprint() == environment)
        {
            return Err(fault(
                "actual owning compiler input differs from preparation",
            ));
        }
        let mut inputs = BTreeMap::from([(input_key, stamp)]);
        for input_key in [InputKey::SourceDependencies, InputKey::ExpansionEpoch,
            InputKey::CapturedSource(root.selection().location_identity())] {
            let (value, stamp) = db.published_input(&input_key)
                .ok_or_else(|| fault("dormant catalog source/epoch input is absent"))?;
            let valid = match (value, &input_key) {
                (InputValue::SourceDependencies(actual), InputKey::SourceDependencies) => actual.fingerprint() == source_dependencies,
                (InputValue::ExpansionEpoch(actual), InputKey::ExpansionEpoch) => *actual == epoch,
                (InputValue::CapturedSource(actual), InputKey::CapturedSource(_)) => actual == root.selection(),
                _ => false,
            };
            if !valid { return Err(fault("dormant catalog source/epoch input differs")); }
            inputs.insert(input_key, stamp);
        }
        let demand = PreparedImplCatalogDemand { plan, root: root.share(), sources, inputs, procedural };
        demand.require_current(compiler, db)?;
        Ok(demand)
    }
    /// Compatibility entry through the same authentic dormant plan. Common
    /// compiler/module preparation can still return NeedModule here.
    pub(crate) fn for_impl_catalog(
        compiler: &PreparedCompileInputs,
        root: &PreparedSourceView,
        epoch: u64,
        db: &Database,
    ) -> Result<SourceCompilerPreparation, Diagnostic> {
        Self::prepare_impl_catalog_demand(compiler, root, epoch, None, db)?
            .into_discovery(compiler, db)?
            .prepare_with_declarative_compiler(compiler.environment_identity(),
                crate::expand::source_declarative::engine_version(), compiler.catalog.clone())
    }
}

impl SourceDiscovery {
    /// One actual original completion source for cold and entered declaration names.
    pub(in crate::expand::source_discovery) fn completed_original_annotation_names(&self) -> Result<
        std::collections::BTreeMap<Digest, std::sync::Arc<std::collections::BTreeMap<Digest,
            std::sync::Arc<crate::frontend::impl_index::CompletedOriginalAnnotation>>>>, Diagnostic> {
        let mut fragments = std::collections::BTreeMap::new();
        for fragment in self.nested_completed.values() {
            let occurrence = fragment.index().output().record().key().invocation;
            if fragments.insert(occurrence, fragment.clone()).is_some() {
                return Err(fault("completed declaration occurrence repeats"));
            }
        }
        let mut completed = std::collections::BTreeMap::<Digest, std::collections::BTreeMap<_, _>>::new();
        for state in self.original_groups.values() {
            let Some(proof) = &state.proof else { continue };
            let source = &self.sources[state.ordinal];
            let parent = self.nested_sources.get(&state.ordinal)
                .ok_or_else(|| fault("completed annotation declaration parent absent"))?;
            let receipt = parent.completed_original_annotation(source, &state.group, proof)?
                .with_name_effects(parent, source, &fragments)?;
            let source_key = QueryKey::Outline(source.selection().outline.clone()).fingerprint();
            if completed.entry(source_key).or_default().insert(receipt.item(), std::sync::Arc::new(receipt)).is_some() {
                return Err(fault("original annotation declaration completion repeats"));
            }
        }
        Ok(completed.into_iter().map(|(source, entries)| (source, std::sync::Arc::new(entries))).collect())
    }
    /// Source metadata only, through the ordinary cold declaration publisher.
    /// The actual retained plan is shared with entry; no executable body or
    /// lexical/module binding is created by this coordinator step.
    pub(in crate::expand::source_discovery) fn publish_cold_catalog_declarations(
        &self,
        db: &mut crate::db::memo::Database,
    ) -> Result<(), Diagnostic> {
        use crate::{
            db::poll::QueryPoll,
            frontend::associated_declarations::{
                AssociatedDeclarationsPreparation, PreparedOriginalAssociatedDeclarations,
            },
        };
        let completed = self.completed_original_annotation_names()?;
        for (ordinal, source) in self.sources.iter().enumerate()
            .filter(|(ordinal, _)| !self.macro_export_sources.contains_key(ordinal)) {
            let AssociatedDeclarationsPreparation::Original(prepared) =
                PreparedOriginalAssociatedDeclarations::prepare(source)? else { continue };
            let source_key = QueryKey::Outline(source.selection().outline.clone()).fingerprint();
            let completions = completed.get(&source_key).map(|entries| &**entries);
            let empty = std::collections::BTreeMap::new();
            let completions = completions.unwrap_or(&empty);
            // One actual plan per owning body. Its original named items and
            // associated declarations share the same metadata kernel as entry.
            for scope in prepared.declaration_scopes() {
                let parent = self.nested_sources.get(&ordinal)
                    .ok_or_else(|| fault("cold declaration namespace parent absent"))?;
                let seal = self.seal.as_ref().ok_or_else(|| fault("cold declaration namespace seal absent"))?;
                let publication = CatalogDeclarationPublication {
                    discovery: self, ordinal, scope: &scope, parent, seal, completions,
                };
                match db.publish_catalog_declarations(&publication) {
                    QueryPoll::Ready(Ok(_)) => {}
                    QueryPoll::Ready(Err(error)) => return Err(error),
                    QueryPoll::NeedInput(_) => return Err(fault("cold catalog declaration input remains pending")),
                }
                db.publish_catalog_declaration_names(&publication)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
impl SourceDiscovery {
    /// Pure selection replay against the actual configured source/compiler
    /// namespace. This control does not append output, settle the source job
    /// or assert catalog completeness.
    pub(crate) fn inspect_original_derive_selection(
        &mut self,
        source: &SourceSelection,
        producer: Digest,
    ) -> Result<
        (
            crate::expand::source_derives::SourceDerivePoll,
            Digest,
            Digest,
        ),
        Diagnostic,
    > {
        self.inspect_original_derive_selection_with_completed(source, producer, &Default::default())
    }
    pub(crate) fn inspect_original_derive_selection_with_completed(
        &mut self,
        source: &SourceSelection,
        producer: Digest,
        completed: &std::collections::BTreeMap<
            Digest,
            Arc<crate::service::source_derive_requests::SourceDeriveSelection>,
        >,
    ) -> Result<
        (
            crate::expand::source_derives::SourceDerivePoll,
            Digest,
            Digest,
        ),
        Diagnostic,
    > {
        let ordinal = self
            .sources
            .iter()
            .position(|view| view.selection() == source)
            .ok_or_else(|| fault("derive control actual source absent"))?;
        let (environment, engine, providers) = self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("derive control actual compiler preparation absent"))?;
        let names = if self.seal.is_some() {
            // Completed discovery retains the actual final namespace snapshot.
            // Inspection must not request mutable access to its sealed graph.
            self.route_install.namespaces.clone()
                .ok_or_else(|| fault("sealed derive control namespace snapshot absent"))?
        } else {
            Self::install_macro_routes_with_modules(
                crate::expand::source_discovery::unique_graph(&mut self.graph)?,
                providers, &self.module_routes, &self.nested_sources,
                &self.nested_inert_proofs, &mut self.route_install,
            )?
        };
        let poll = crate::expand::source_derives::prepare_group(
            &self.sources[ordinal],
            &providers[ordinal],
            producer,
            &names,
            completed,
            &self.helper_declarations,
        )?;
        Ok((poll, *environment, *engine))
    }
    pub(crate) fn original_declaration_parent_for_control(
        &self,
        source: &SourceSelection,
    ) -> Result<Arc<crate::frontend::impl_index::DeclarationMacroIndex>, Diagnostic> {
        let ordinal = self
            .sources
            .iter()
            .position(|view| view.selection() == source)
            .ok_or_else(|| fault("original control source absent"))?;
        self.nested_sources
            .get(&ordinal)
            .cloned()
            .ok_or_else(|| fault("original control parent absent"))
    }
    /// Exercise the real declaration-fragment producer closure. The selected
    /// fixture output needs no external invocation; an outstanding demand fails
    /// this control instead of manufacturing a completed child.
    pub(crate) fn complete_original_derive_output_for_control(
        &mut self,
        source: &SourceSelection,
        parent: Arc<crate::frontend::impl_index::DeclarationMacroIndex>,
        output: Arc<crate::expand::generated_declarations::GeneratedDeclarations>,
    ) -> Result<Arc<crate::expand::declaration_fragment::CompletedDeclarationFragment>, Diagnostic>
    {
        let ordinal = self
            .sources
            .iter()
            .position(|view| view.selection() == source)
            .ok_or_else(|| fault("derive control source absent"))?;
        let (environment, engine, providers) = self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("derive control compiler preparation absent"))?;
        let names = if self.seal.is_some() {
            // Completed discovery retains the actual final namespace snapshot.
            // Inspection must not request mutable access to its sealed graph.
            self.route_install.namespaces.clone()
                .ok_or_else(|| fault("sealed derive control namespace snapshot absent"))?
        } else {
            Self::install_macro_routes_with_modules(
                crate::expand::source_discovery::unique_graph(&mut self.graph)?,
                providers, &self.module_routes, &self.nested_sources,
                &self.nested_inert_proofs, &mut self.route_install,
            )?
        };
        let mut fragment =
            crate::expand::declaration_fragment::DeclarationFragment::from_item_output(
                parent,
                crate::expand::declaration_fragment::DeclarationFragmentInput {
                    invocation: output.record().key().invocation,
                    request: self.expansion_budget.request_identity(),
                    graph: self.key.unit_graph,
                    compile_env: *environment,
                    engine: *engine,
                    epoch: self.epoch,
                    cfg: self.sources[ordinal].captured_macro_cfg()?.clone(),
                },
                self.expansion_budget.clone(),
                output,
            )?;
        match fragment.poll_with_sources(&providers[ordinal], &names, Some(&self.included_views))? {
            crate::expand::declaration_fragment::DeclarationFragmentStep::Complete(output) => {
                Ok(output)
            }
            crate::expand::declaration_fragment::DeclarationFragmentStep::NeedInput(input) => Err(
                fault(&format!("actual derive output input pending: {input:?}")),
            ),
            _ => Err(fault(
                "actual derive output has an unexecuted expansion demand",
            )),
        }
    }
    /// Source-only failure evidence; no alternate resolution or execution.
    pub(crate) fn catalog_pending_source_evidence(&self) -> Vec<String> {
        let mut rows = vec![format!(
            "ordinary producers={:?}; pending={:?}",
            self.producers,
            self.pending_nested.as_ref().map(|ticket| ticket.job())
        )];
        for (job, fragment) in &self.nested_fragments {
            rows.extend(fragment.pending_source_evidence().into_iter()
                .map(|row| format!("fragment job={job:?} {row}")));
        }
        for (ordinal, index) in &self.nested_sources {
            if !index.is_complete() {
                rows.push(format!(
                    "incomplete source ordinal{ordinal} path={:?} producers={:?}",
                    index.scan().path(),
                    index.scan().producers()
                ));
            }
        }
        for (job, ordinal) in &self.nested_jobs {
            if self
                .sources
                .get(*ordinal)
                .is_some_and(|source| source.outline().root_scope.crate_key == self.scope.crate_key)
            {
                rows.push(format!(
                    "root job={job:?} source={ordinal} completed={}",
                    self.nested_completed.contains_key(job)
                ));
            }
        }
        rows
    }
}
