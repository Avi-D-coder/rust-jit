//! Actual standalone source-world preparation and ordinary catalog closure.
//! Neither this source certificate nor its locators supplies typed solver facts.
use super::PreparedLiveImplCatalogSources;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey,
        seals::{DiscoveryInput, SealDomain, SealKey},
    },
    expand::source_discovery::{SourceCompilerPreparation, SourceDiscovery},
    service::source_preparation::PreparedCompileInputs,
    source::{changes::SourceSelection, manifest::Namespace},
};
use std::sync::Arc;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-IMPL-CATALOG-WORLD", message.into())
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
}
impl StableEncode for ImplCatalogPlan {
    const DOMAIN: &'static str = "lrr/v1/configured-standalone-impl-catalog-plan";
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
    }
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
    /// Start a fresh global work ledger over the actual explicitly prepared
    /// standalone world. No prior Scope or TestCatalog certificate is consumed.
    /// Common compiler/module preparation can still return NeedModule here.
    pub(crate) fn for_impl_catalog(
        compiler: &PreparedCompileInputs,
        root: &crate::source::changes::PreparedSourceView,
        epoch: u64,
        db: &crate::db::memo::Database,
    ) -> Result<SourceCompilerPreparation, Diagnostic> {
        let prepared = PreparedLiveImplCatalogSources::capture(
            root,
            root.source_dependencies()?.fingerprint(),
            epoch,
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
        let scope = root.outline().root_scope.clone();
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
        let environment = compiler.environment_identity();
        let generation = compiler.impl_catalog_generation();
        let engine = crate::expand::source_declarative::engine_version();
        let compiler_id = compiler.catalog.owning_profile_identity()?;
        let mut graph = StableEncoder::new("lrr/v1/standalone-impl-catalog-unit-graph");
        graph.digest(live.source_dependencies().fingerprint());
        graph.sequence(&originals, |e, source| {
            e.string(source.path.as_str());
            e.record(&source.outline);
        });
        let unit_graph = graph.finish();
        let mut identity = StableEncoder::new("lrr/v1/configured-impl-catalog-identity");
        identity.digest(unit_graph);
        identity.digest(generation);
        identity.digest(environment);
        identity.digest(engine);
        identity.digest(compiler_id);
        identity.digest(facade.fingerprint());
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
        });
        let mut discovery = Self::new_domain(
            sources,
            scope,
            Namespace::Value,
            unit_graph,
            key.epoch,
            key.domain,
            false,
        )?;
        discovery.impl_catalog_plan = Some(plan);
        // The common ParseSource completion records prepared input stamps in
        // the ledger. Register this before polling, never append dependencies
        // to an already completed seal at publication.
        let input_key = crate::db::dependency::InputKey::UnitCompileEnv(root_unit);
        let (input, stamp) = db
            .published_input(&input_key)
            .ok_or_else(|| fault("actual owning compiler input is not published"))?;
        if !matches!(input, crate::db::dependency::InputValue::UnitCompileEnv(actual) if actual.unit() == root_unit && actual.fingerprint() == environment)
        {
            return Err(fault(
                "actual owning compiler input differs from preparation",
            ));
        }
        discovery.compiler_inputs.insert(input_key, stamp);
        discovery.prepare_with_declarative_compiler(environment, engine, compiler.catalog.clone())
    }
}

impl SourceDiscovery {
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
        for (ordinal, source) in self.sources.iter().enumerate()
            .filter(|(ordinal, _)| !self.macro_export_sources.contains_key(ordinal)) {
            let AssociatedDeclarationsPreparation::Original(prepared) =
                PreparedOriginalAssociatedDeclarations::prepare(source)? else { continue };
            let mut completions = std::collections::BTreeMap::new();
            for state in self.original_groups.values().filter(|state| state.ordinal == ordinal) {
                let Some(proof) = &state.proof else { continue };
                let parent = self.nested_sources.get(&ordinal)
                    .ok_or_else(|| fault("completed annotation declaration parent absent"))?;
                let receipt = parent.completed_original_annotation(source, &state.group, proof)?;
                if completions.insert(receipt.item(), receipt).is_some() {
                    return Err(fault("original annotation declaration completion repeats"));
                }
            }
            // One actual plan per owning body. Its original named items and
            // associated declarations share the same metadata kernel as entry.
            for scope in prepared.declaration_scopes() {
                match db.publish_captured_declarations(&scope, &completions) {
                    QueryPoll::Ready(Ok(_)) => {}
                    QueryPoll::Ready(Err(error)) => return Err(error),
                    QueryPoll::NeedInput(_) => return Err(fault("cold catalog declaration input remains pending")),
                }
                let parent = self.nested_sources.get(&ordinal)
                    .ok_or_else(|| fault("cold declaration namespace parent absent"))?;
                let seal = self.seal.as_ref().ok_or_else(|| fault("cold declaration namespace seal absent"))?;
                db.publish_cold_declaration_names(&scope, parent, seal)?;
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
