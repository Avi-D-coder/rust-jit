//! Explicit configured SDK source preparation. No query or runtime entry opens
//! these capabilities; role-only sources never enter the ordinary source registry.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::{InputRequest, QueryPoll},
    },
    expand::{
        source_declarative::macro_exports::SourceMacroExportFacet,
        source_discovery::{SourceCompilerPreparation, SourceDiscovery},
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    project::tool_manifest::ToolManifest,
    service::{
        expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
        source_capture::SourceCapture,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        include_paths::{IncludeAuthorities, IncludeRoot, IncludeRootKind},
        macro_export_module::{MacroExportModuleAdoption, MacroExportModuleRequest},
        manifest::{BodyKey, Namespace},
        module_capture::{CapturedModule, ModuleObservation},
    },
    syntax::outline::ScopeKey,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-EXPORT-PREPARATION", message.into())
}
/// Complete actual selected role graph, retained outside ordinary source inputs.
/// The caller continues ordinary discovery polling/publication after Ready.
pub(crate) struct ConfiguredMacroExportSources {
    prepared: PreparedSourceInputs,
    facets: Vec<Arc<SourceMacroExportFacet>>,
    modules: Vec<Arc<MacroExportModuleAdoption>>,
    scope: ScopeKey,
    epoch: u64,
    preparation: Digest,
}
pub(crate) enum MacroExportModulePreparation {
    Ready {
        discovery: SourceDiscovery,
        sources: ConfiguredMacroExportSources,
    },
    NeedInput {
        sources: ConfiguredMacroExportSources,
        input: InputRequest,
    },
    NeedPreparation {
        sources: ConfiguredMacroExportSources,
        demand: RecordedModulePreparation,
    },
}
/// Exact absent *inventory rows*, not guessed filesystem absence or permission
/// to change the prepared manifest. Re-preparation must produce a new source set.
pub(crate) struct RecordedModulePreparation {
    preparation: Digest,
    input: InputRequest,
    missing: Vec<PathBuf>,
    metadata_bytes: usize,
}
impl RecordedModulePreparation {
    pub(crate) fn preparation(&self) -> Digest {
        self.preparation
    }
    pub(crate) fn input(&self) -> &InputRequest {
        &self.input
    }
    pub(crate) fn missing_paths(&self) -> &[PathBuf] {
        &self.missing
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
}
impl ConfiguredMacroExportSources {
    pub(crate) fn new(
        manifest: &ToolManifest,
        prepared: PreparedSourceInputs,
        mut facets: Vec<Arc<SourceMacroExportFacet>>,
        modules: Vec<Arc<MacroExportModuleAdoption>>,
        scope: ScopeKey,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        manifest.validate()?;
        if facets.is_empty()
            || facets.len() > 256
            || modules.len() > 256
            || epoch == 0
            || scope.owner.is_some()
            || scope.module_path.len() > 256
            || scope.module_path.iter().any(|part| part.len() > 64 * 1024)
        {
            return Err(fault("configured macro source count/epoch limit"));
        }
        facets.sort_by_key(|facet| facet.source().selection().location_identity());
        let first = facets[0].source();
        let mut prior = None;
        for facet in &facets {
            let location = facet.source().selection().location_identity();
            let selected = match prepared.select(facet.source().selection())? {
                SourceSelectionOutcome::Selected(source) => source,
                _ => return Err(fault("configured source selection is absent or changed")),
            };
            if prior == Some(location)
                || !selected.same_publication(facet.source())
                || !facet.source().same_publication(first)
            {
                return Err(fault(
                    "configured source set has duplicate or unrelated publication",
                ));
            }
            let actual = SourceMacroExportFacet::capture(
                manifest,
                facet.unit(),
                facet.recorded_path(),
                selected,
            )?;
            if actual.identity() != facet.identity() {
                return Err(fault(
                    "configured facet differs from actual manifest/source",
                ));
            }
            prior = Some(location);
        }
        let result = Self {
            prepared,
            facets,
            modules,
            scope,
            epoch,
            preparation: manifest.fingerprint(),
        };
        result.retained_admission_bytes()?;
        Ok(result)
    }
    pub(crate) fn facets(&self) -> &[Arc<SourceMacroExportFacet>] {
        &self.facets
    }
    pub(crate) fn modules(&self) -> &[Arc<MacroExportModuleAdoption>] {
        &self.modules
    }
    /// Conservative cold admission, not a deduplicated pool footprint. Every
    /// source state (including prepared) is already present under facet roots.
    pub(crate) fn retained_admission_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.scope.heap_bytes())
            .and_then(|n| {
                n.checked_add(
                    self.facets
                        .capacity()
                        .checked_mul(std::mem::size_of::<Arc<SourceMacroExportFacet>>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    self.modules
                        .capacity()
                        .checked_mul(std::mem::size_of::<Arc<MacroExportModuleAdoption>>())?,
                )
            })
            .ok_or_else(|| fault("configured source graph metadata overflow"))?;
        for count in self
            .facets
            .iter()
            .map(|facet| facet.metadata_bytes())
            .chain(
                self.modules
                    .iter()
                    .map(|module| module.retained_admission_bytes()),
            )
        {
            bytes = bytes
                .checked_add(count)
                .filter(|n| *n <= 320 * 1024 * 1024)
                .ok_or_else(|| fault("configured macro source graph retained admission limit"))?;
        }
        Ok(bytes)
    }
    fn discovery(
        &self,
        db: &mut Database,
        manifest: &ToolManifest,
    ) -> Result<SourceDiscovery, Diagnostic> {
        if manifest.fingerprint() != self.preparation {
            return Err(fault(
                "configured manifest changed; fresh preparation is required",
            ));
        }
        let epoch = db
            .input(&InputKey::ExpansionEpoch)
            .ok_or_else(|| fault("actual expansion epoch is unpublished"))?;
        if !matches!(epoch.as_ref(), InputValue::ExpansionEpoch(epoch) if *epoch == self.epoch) {
            return Err(fault("configured macro source epoch changed"));
        }
        SourceDiscovery::new(
            self.facets
                .iter()
                .map(|facet| facet.source().share())
                .collect(),
            self.scope.clone(),
            Namespace::Macro,
            manifest.graph.fingerprint(),
            self.epoch,
        )?
        .with_macro_export_source_publications(db)?
        .with_macro_export_module_adoptions(db, &self.modules)
    }
    /// Explicit coordinator effect: actual captures precede source refresh and
    /// current ancestry adoption. No filesystem read occurs for missing inventory.
    pub(crate) fn prepare(
        mut self,
        db: &mut Database,
        manifest: &ToolManifest,
        compiler: Arc<CompileInputCatalog>,
    ) -> Result<MacroExportModulePreparation, Diagnostic> {
        db.require_source_preparation()?;
        manifest.validate()?;
        let mut discovery = self.discovery(db, manifest)?;
        let environment = manifest
            .compile_env
            .get(&self.scope.crate_key)
            .ok_or_else(|| fault("selected configured compile environment absent"))?;
        let environment = crate::expand::builtins::FrozenCompileEnvironment::new(
            self.scope.crate_key,
            environment,
        )?
        .fingerprint();
        for phase in 0..256u32 {
            let (pending, producer) = match discovery.prepare_with_declarative_compiler(
                environment,
                crate::expand::source_declarative::engine_version(),
                Arc::clone(&compiler),
            )? {
                SourceCompilerPreparation::NeedModule { discovery, demand } => {
                    let producer = demand
                        .declaration(discovery.unresolved_ordinary_module_sources())?
                        .ok_or_else(|| {
                            fault(
                                "resolved module requires an actual configured source declaration",
                            )
                        })?
                        .producer();
                    (discovery, producer)
                }
                SourceCompilerPreparation::Ready(discovery) => {
                    // Full producer closure still needs actual declared modules even
                    // where no import happened to demand their membership first.
                    let producer = discovery
                        .unresolved_ordinary_module_sources()
                        .next()
                        .map(|row| row.producer());
                    let Some(producer) = producer else {
                        return Ok(MacroExportModulePreparation::Ready {
                            discovery,
                            sources: self,
                        });
                    };
                    (discovery, producer)
                }
            };
            let declaration = pending.original_module_declaration(producer)?;
            let parent = self
                .facets
                .iter()
                .find(|facet| facet.source().selection() == declaration.source().selection())
                .ok_or_else(|| {
                    fault("actual module parent is absent from selected configured facets")
                })?;
            let authorities = authorities(parent)?;
            let input = match pending.prepare_original_module_source(db, producer, &authorities) {
                QueryPoll::Ready(Ok(input)) => input,
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(input) => {
                    return Ok(MacroExportModulePreparation::NeedInput {
                        sources: self,
                        input,
                    });
                }
            };
            let InputRequest::CaptureModule {
                request,
                compile_environment,
            } = &input
            else {
                return Err(fault("actual module capture request kind differs"));
            };
            let mut missing = Vec::new();
            missing
                .try_reserve_exact(request.candidates().len())
                .map_err(|_| fault("module preparation path allocation"))?;
            for candidate in request.candidates() {
                let path = parent.package_root().join(candidate.path().as_str());
                let text = path
                    .to_str()
                    .ok_or_else(|| fault("recorded module path must be UTF-8"))?;
                if !manifest.inputs.source_files.contains_key(text) {
                    missing.push(path);
                }
            }
            if !missing.is_empty() {
                let metadata_bytes = missing.iter().try_fold(
                    std::mem::size_of::<RecordedModulePreparation>()
                        .checked_add(request.metadata_bytes()?)
                        .and_then(|n| {
                            n.checked_add(
                                missing
                                    .capacity()
                                    .checked_mul(std::mem::size_of::<PathBuf>())?,
                            )
                        })
                        .ok_or_else(|| fault("recorded module preparation metadata overflow"))?,
                    |n, path| {
                        n.checked_add(path.capacity())
                            .filter(|n| *n <= 32 * 1024 * 1024)
                            .ok_or_else(|| fault("recorded module preparation metadata limit"))
                    },
                )?;
                return Ok(MacroExportModulePreparation::NeedPreparation {
                    sources: self,
                    demand: RecordedModulePreparation {
                        preparation: manifest.fingerprint(),
                        input,
                        missing,
                        metadata_bytes,
                    },
                });
            }
            if self.facets.len() >= 256 || self.modules.len() >= 256 {
                return Err(fault("configured module source/adoption count limit"));
            }
            let capture = SourceCapture::open(authorities, BTreeMap::new())?;
            let captured = capture.capture_module(request)?;
            verify_recorded_candidates(manifest, parent, request, &captured)?;
            let selected = captured.selected()?;
            let child_selection = SourceSelection {
                path: selected.location().path.clone(),
                outline: crate::db::key::OutlineKey {
                    parse: crate::syntax::parse::ParseKey {
                        content: selected.blob().content(),
                        profile: declaration
                            .source()
                            .selection()
                            .outline
                            .parse
                            .profile
                            .clone(),
                    },
                    crate_key: declaration.scope().crate_key,
                    module_path: declaration.scope().module_path.clone(),
                    cfg: declaration.source().selection().outline.cfg,
                },
            };
            if self
                .facets
                .iter()
                .any(|facet| facet.source().outline().root_scope == *declaration.scope())
            {
                return Err(fault(
                    "configured module scope already has a distinct selected source",
                ));
            }
            let child = SourceUnitInput {
                path: child_selection.path.clone(),
                crate_key: child_selection.outline.crate_key,
                module_path: child_selection.outline.module_path.clone(),
                profile: child_selection.outline.parse.profile.clone(),
                cfg: declaration.source().captured_macro_cfg()?.clone(),
                blob: selected.blob().clone(),
            };
            let parent_location = parent.source().selection().location_identity();
            let parent_chain = pending.original_module_parent_adoption(producer)?.cloned();
            let context = DemandContext {
                request: input.fingerprint(),
                session: self.preparation,
                unit: parent.unit(),
                revision: db.source_revision(),
                epoch: self.epoch,
                source_generation: parent.preparation(),
                native_generation: None,
                compile_environment: *compile_environment,
            };
            let mut coordinator = ExpansionCoordinator::new(context.clone());
            let token = coordinator.issue(
                QueryKey::Outline(declaration.source().selection().outline.clone()),
                PhaseIdentity {
                    frame: self.preparation,
                    body: BodyKey {
                        definition: producer,
                        exact_body_bytes: input.fingerprint(),
                    },
                    index: phase,
                },
                input.clone(),
            )?;
            coordinator.start(&context, &token)?;
            coordinator.accept_module(&context, &token, &captured)?;
            coordinator.adopt_module(&context, &token, db, captured)?;
            // Refresh source facts as one publication; use the actual old source
            // state as reuse input, then remint current owners for EVERY ancestry.
            discovery = self.refresh(
                db,
                manifest,
                child,
                &child_selection,
                parent_location,
                producer,
                parent_chain.as_deref(),
                &input,
            )?;
        }
        Err(fault(
            "configured module preparation step/source count limit",
        ))
    }
    fn refresh(
        &mut self,
        db: &mut Database,
        manifest: &ToolManifest,
        child: SourceUnitInput,
        child_selection: &SourceSelection,
        parent_location: Digest,
        producer: Digest,
        parent_chain: Option<&MacroExportModuleAdoption>,
        captured_input: &InputRequest,
    ) -> Result<SourceDiscovery, Diagnostic> {
        if self.facets.len() >= 256 || self.modules.len() >= 256 {
            return Err(fault("configured module source/adoption count limit"));
        }
        if parent_chain.is_some_and(|chain| {
            chain
                .request()
                .child()
                .source()
                .selection()
                .location_identity()
                != parent_location
        }) {
            return Err(fault(
                "actual preceding module adoption belongs to another parent",
            ));
        }
        let mut links = Vec::new();
        links
            .try_reserve_exact(self.modules.len() + 1)
            .map_err(|_| fault("module ancestry refresh allocation"))?;
        for module in &self.modules {
            links.push((
                module
                    .request()
                    .child()
                    .source()
                    .outline()
                    .root_scope
                    .module_path
                    .len(),
                module
                    .request()
                    .parent()
                    .source()
                    .selection()
                    .location_identity(),
                module
                    .request()
                    .child()
                    .source()
                    .selection()
                    .location_identity(),
                module.request().declaration().producer(),
                capture_root(module.request().input())?,
            ));
        }
        links.push((
            child_selection.outline.module_path.len(),
            parent_location,
            child_selection.location_identity(),
            producer,
            capture_root(captured_input)?,
        ));
        links.sort();
        let mut units = Vec::new();
        units
            .try_reserve_exact(self.facets.len() + 1)
            .map_err(|_| fault("configured source refresh allocation"))?;
        for facet in &self.facets {
            let source = facet.source();
            units.push(SourceUnitInput {
                path: source.selection().path.clone(),
                crate_key: facet.unit(),
                module_path: source.selection().outline.module_path.clone(),
                profile: source.selection().outline.parse.profile.clone(),
                cfg: source.captured_macro_cfg()?.clone(),
                blob: source.captured_macro_blob()?.clone(),
            });
        }
        units.push(child);
        let source = self.facets[0].source();
        let mut environment = BTreeMap::new();
        let mut owning_environments = Vec::new();
        for (key, value) in source.prepared_inputs() {
            match (key, value) {
                (InputKey::CompileEnv(name), InputValue::CompileEnv(value)) => {
                    environment.insert(name.clone(), value.clone());
                }
                (InputKey::UnitCompileEnv(_), InputValue::UnitCompileEnv(value)) => {
                    owning_environments.push(Arc::clone(value))
                }
                _ => {}
            }
        }
        let mut prepared = PreparedSourceInputs::refresh(Some(&self.prepared), units, environment)?;
        for environment in owning_environments {
            prepared = prepared.with_compile_environment(environment)?;
        }
        let mut facets = Vec::new();
        facets
            .try_reserve_exact(self.facets.len() + 1)
            .map_err(|_| fault("refreshed facet allocation"))?;
        for facet in &self.facets {
            let selected = match prepared.select(facet.source().selection())? {
                SourceSelectionOutcome::Selected(source) => source,
                _ => return Err(fault("actual parent source changed during module refresh")),
            };
            facets.push(Arc::new(SourceMacroExportFacet::capture(
                manifest,
                facet.unit(),
                facet.recorded_path(),
                selected,
            )?));
        }
        let parent = facets
            .iter()
            .find(|facet| facet.source().selection().location_identity() == parent_location)
            .ok_or_else(|| fault("refreshed module parent is absent"))?;
        let recorded_path = parent.package_root().join(child_selection.path.as_str());
        let selected = match prepared.select(child_selection)? {
            SourceSelectionOutcome::Selected(source) => source,
            _ => {
                return Err(fault(
                    "actual captured child source is absent during refresh",
                ));
            }
        };
        facets.push(Arc::new(SourceMacroExportFacet::capture(
            manifest,
            child_selection.outline.crate_key,
            recorded_path
                .to_str()
                .ok_or_else(|| fault("configured child path must be UTF-8"))?,
            selected,
        )?));
        facets.sort_by_key(|facet| facet.source().selection().location_identity());
        let mut bytes = self.retained_admission_bytes()?;
        for facet in &facets {
            bytes = bytes
                .checked_add(facet.metadata_bytes())
                .filter(|n| *n <= 640 * 1024 * 1024)
                .ok_or_else(|| fault("old/new configured source temporary admission limit"))?;
        }
        // Existing atomic role-only publication. No PreparedSourceInputs::publish,
        // ordinary source input insertion or parse registry registration occurs.
        db.publish_macro_export_sources(&facets)?;
        let fresh = SourceDiscovery::new(
            facets.iter().map(|facet| facet.source().share()).collect(),
            self.scope.clone(),
            Namespace::Macro,
            manifest.graph.fingerprint(),
            self.epoch,
        )?
        .with_macro_export_source_publications(db)?;
        let by_location: BTreeMap<_, _> = facets
            .iter()
            .map(|facet| (facet.source().selection().location_identity(), facet))
            .collect();
        let mut modules: BTreeMap<Digest, Arc<MacroExportModuleAdoption>> = BTreeMap::new();
        for (_, parent, child, producer, root) in links {
            let parent = by_location
                .get(&parent)
                .ok_or_else(|| fault("actual ancestry parent absent after refresh"))?;
            let child = by_location
                .get(&child)
                .ok_or_else(|| fault("actual ancestry child absent after refresh"))?;
            let preceding = if parent.source().outline().root_scope.module_path.is_empty() {
                None
            } else {
                Some(Arc::clone(
                    modules
                        .get(&parent.source().selection().location_identity())
                        .ok_or_else(|| fault("actual refreshed preceding ancestry is absent"))?,
                ))
            };
            let request = MacroExportModuleRequest::prepare(
                db,
                Arc::clone(parent),
                preceding,
                Arc::clone(child),
                fresh.original_module_declaration(producer)?.share(),
                &authorities_with_root(parent, root)?,
            )?;
            let module = match request.adopted(db) {
                QueryPoll::Ready(Ok(module)) => module,
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(_) => {
                    return Err(fault(
                        "refreshed actual ancestry requires its original adopted record",
                    ));
                }
            };
            if modules
                .insert(child.source().selection().location_identity(), module)
                .is_some()
            {
                return Err(fault("refreshed ancestry has duplicate child source"));
            }
        }
        let modules: Vec<_> = modules.into_values().collect();
        let fresh = fresh.with_macro_export_module_adoptions(db, &modules)?;
        let next = Self {
            prepared,
            facets,
            modules,
            scope: self.scope.clone(),
            epoch: self.epoch,
            preparation: self.preparation,
        };
        next.retained_admission_bytes()?;
        *self = next;
        Ok(fresh)
    }
}
pub(crate) fn authorities(
    facet: &SourceMacroExportFacet,
) -> Result<IncludeAuthorities, Diagnostic> {
    let mut identity = StableEncoder::new("lrr/v1/configured-macro-export-source-root");
    identity.digest(facet.unit());
    identity.digest(facet.preparation());
    identity.string(
        facet
            .package_root()
            .to_str()
            .ok_or_else(|| fault("configured package root must be UTF-8"))?,
    );
    authorities_with_root(facet, identity.finish())
}
fn authorities_with_root(
    facet: &SourceMacroExportFacet,
    root: Digest,
) -> Result<IncludeAuthorities, Diagnostic> {
    IncludeAuthorities::new(
        facet.unit(),
        facet.preparation(),
        [IncludeRoot {
            identity: root,
            path: facet.package_root().to_path_buf(),
            kind: IncludeRootKind::Source,
        }],
    )
}
fn capture_root(input: &InputRequest) -> Result<Digest, Diagnostic> {
    let InputRequest::CaptureModule { request, .. } = input else {
        return Err(fault("retained module capture input kind differs"));
    };
    let root = request
        .candidates()
        .first()
        .ok_or_else(|| fault("retained module capture candidates absent"))?
        .root();
    if request.candidates().iter().any(|candidate| {
        candidate.root() != root || candidate.root_kind() != IncludeRootKind::Source
    }) {
        return Err(fault("retained module capture source root differs"));
    }
    Ok(root)
}
fn verify_recorded_candidates(
    manifest: &ToolManifest,
    parent: &SourceMacroExportFacet,
    request: &crate::source::module_capture::ModuleCaptureRequest,
    captured: &CapturedModule,
) -> Result<(), Diagnostic> {
    captured.validate(request)?;
    for (candidate, actual) in request.candidates().iter().zip(captured.observations()) {
        let path = parent.package_root().join(candidate.path().as_str());
        let expected = manifest
            .inputs
            .source_files
            .get(
                path.to_str()
                    .ok_or_else(|| fault("recorded candidate path must be UTF-8"))?,
            )
            .ok_or_else(|| fault("module candidate is missing from prepared source inventory"))?;
        let matches = match (expected, actual) {
            (None, ModuleObservation::Absent) => true,
            (Some(expected), ModuleObservation::Present(record)) => {
                *expected == record.blob().content()
            }
            _ => false,
        };
        if !matches {
            return Err(Diagnostic::new(
                "LRR-E-TOOL-PREPARE-STALE",
                "actual module candidate presence/content differs from recorded preparation".into(),
            ));
        }
    }
    Ok(())
}
