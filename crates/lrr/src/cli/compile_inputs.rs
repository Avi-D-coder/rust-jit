//! Standalone preparation and explicit compiler-input coordination.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile,
    },
    db::{
        key::QueryKey,
        memo::Database,
        poll::{ExpansionRequest, InputRequest, QueryPoll},
    },
    expand::{
        builtins::FrozenCompileEnvironment,
        source_discovery::{
            ModuleSourceReadiness, OrdinaryModuleReadiness, SourceDiscovery, SourceDiscoveryStep,
        },
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    project::tool_manifest::CompileEnv,
    service::{
        eager_compile_inputs::EagerCompileInputPoll,
        expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply},
        source_capture::SourceCapture,
    },
    source::{
        changes::{PreparedSourceInputs, PreparedSourceView, SourceSelectionOutcome},
        include_paths::{IncludeAuthorities, IncludeRoot, IncludeRootKind},
        manifest::BodyKey,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::Arc,
};
fn fault(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
pub(super) struct PreparedCompileInputs {
    environment: FrozenCompileEnvironment,
    authorities: IncludeAuthorities,
    capture: SourceCapture,
    generation: Digest,
    module_layouts: BTreeMap<Digest, PathBuf>,
    pub catalog: Arc<CompileInputCatalog>,
}
fn captured_module_layouts(
    root: &PreparedSourceView,
    sources: &[PreparedSourceView],
) -> Result<BTreeMap<Digest, PathBuf>, Diagnostic> {
    let root_directory = Path::new(root.selection().path.as_str())
        .parent()
        .unwrap_or(Path::new(""))
        .to_owned();
    // Only the root crate's own modules share the root's directory layout;
    // dependency crates captured in the same publication have their own roots.
    let unit = root.selection().outline.crate_key;
    let sources = sources
        .iter()
        .filter(|source| source.selection().outline.crate_key == unit)
        .collect::<Vec<_>>();
    let source_index: BTreeMap<_, _> = sources
        .iter()
        .map(|source| (source.selection().location_identity(), *source))
        .collect();
    if source_index.len() != sources.len() {
        return Err(fault(
            "LRR-E-SOURCE-MODULE",
            "duplicate captured source layout",
        ));
    }
    let physical_index: BTreeMap<_, _> = sources
        .iter()
        .map(|source| {
            (
                (
                    &source.selection().path,
                    &source.outline().root_scope.module_path,
                ),
                *source,
            )
        })
        .collect();
    let paths: BTreeSet<_> = sources
        .iter()
        .map(|source| &source.selection().path)
        .collect();
    let mut metadata_bytes = 0usize;
    let mut queue = VecDeque::from([(root.selection().clone(), root_directory)]);
    let mut seen = BTreeSet::new();
    let mut layouts = BTreeMap::new();
    while let Some((selection, directory)) = queue.pop_front() {
        if !seen.insert((selection.location_identity(), directory.clone())) {
            continue;
        }
        metadata_bytes = metadata_bytes
            .checked_add(directory.as_os_str().len())
            .and_then(|n| n.checked_add(selection.path.as_str().len()))
            .and_then(|n| n.checked_add(256))
            .ok_or_else(|| {
                fault(
                    "LRR-E-SOURCE-LIMIT",
                    "captured module layout metadata limit",
                )
            })?;
        if layouts
            .insert(selection.location_identity(), directory.clone())
            .is_some_and(|old| old != directory)
        {
            return Err(fault(
                "LRR-E-SOURCE-MODULE",
                "captured module has conflicting directories",
            ));
        }
        let source = source_index
            .get(&selection.location_identity())
            .filter(|source| source.selection() == &selection)
            .ok_or_else(|| {
                fault(
                    "LRR-E-SOURCE-MODULE",
                    "captured module layout source absent",
                )
            })?;
        for request in crate::syntax::modules::external_modules(
            source.captured_macro_blob()?.text()?,
            source.captured_macro_cfg()?,
        )? {
            let mut module_path = source.outline().root_scope.module_path.clone();
            module_path.extend_from_slice(&request.module_path);
            let (child, child_directory) = crate::source::graph::module_target(
                &selection.path,
                &directory,
                request,
                |path| Ok(paths.contains(path)),
            )?;
            let child_source = physical_index.get(&(&child, &module_path)).ok_or_else(|| {
                fault("LRR-E-SOURCE-MODULE", "captured child module layout absent")
            })?;
            queue.try_reserve(1).map_err(|_| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "captured module layout queue allocation",
                )
            })?;
            queue.push_back((child_source.selection().clone(), child_directory));
        }
    }
    Ok(layouts)
}
type Prepared = (
    PreparedCompileInputs,
    PreparedSourceInputs,
    PreparedSourceView,
    Vec<PreparedSourceView>,
);
impl PreparedCompileInputs {
    pub fn prepare(
        path: &Path,
        prepared: PreparedSourceInputs,
        view: PreparedSourceView,
        sources: Vec<PreparedSourceView>,
    ) -> Result<Prepared, Diagnostic> {
        Self::prepare_with_environment(path, prepared, view, sources, None)
    }
    /// `prepare` with the root environment an explicit standalone preparation
    /// recorded for its consumer unit; it must be the root's captured cfg.
    pub fn prepare_with_environment(
        path: &Path,
        prepared: PreparedSourceInputs,
        view: PreparedSourceView,
        sources: Vec<PreparedSourceView>,
        recorded: Option<&CompileEnv>,
    ) -> Result<Prepared, Diagnostic> {
        let unit = view.outline().root_scope.crate_key;
        let standalone = CompileEnv {
            unit,
            host: profile::GUEST_TARGET.into(),
            target: profile::GUEST_TARGET.into(),
            cfg: view.captured_macro_cfg()?.clone(),
            cargo: BTreeMap::new(),
            rustc_env: BTreeMap::new(),
            declared: BTreeMap::new(),
            out_dir: None,
        };
        if let Some(recorded) = recorded
            && recorded.fingerprint() != standalone.fingerprint()
        {
            return Err(fault(
                "LRR-E-TOOL-PREPARE-STALE",
                "prepared root compile environment differs from the captured root",
            ));
        }
        let environment = FrozenCompileEnvironment::new(unit, &standalone)?;
        let prepared = prepared.with_compile_environment(Arc::new(environment.clone()))?;
        let select = |source: &PreparedSourceView| -> Result<PreparedSourceView, Diagnostic> {
            match prepared.select(source.selection())? {
                SourceSelectionOutcome::Selected(view) => Ok(view),
                _ => Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "compile preparation source selection changed",
                )),
            }
        };
        let root_view = select(&view)?;
        let mut fresh = Vec::new();
        fresh
            .try_reserve_exact(sources.len())
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "compile source view allocation"))?;
        for source in &sources {
            fresh.push(select(source)?);
        }
        let module_layouts = captured_module_layouts(&root_view, &fresh)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent =
            std::fs::canonicalize(parent).map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
        let root_text = parent
            .to_str()
            .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "standalone source root must be UTF-8"))?;
        let mut root = StableEncoder::new("lrr/v2/standalone-include-root");
        root.digest(unit);
        root.string(root_text);
        let root_identity = root.finish();
        let mut generation = StableEncoder::new("lrr/v2/standalone-compile-generation");
        generation.digest(root_identity);
        generation.digest(environment.fingerprint());
        let mut selections = Vec::new();
        selections.try_reserve_exact(fresh.len()).map_err(|_| {
            fault(
                "LRR-E-METADATA-LIMIT",
                "compile generation selection allocation",
            )
        })?;
        for source in &fresh {
            selections.push((
                source.selection().location_identity(),
                QueryKey::Outline(source.selection().outline.clone()).fingerprint(),
            ));
        }
        selections.sort();
        generation.sequence(&selections, |encoder, (location, outline)| {
            encoder.digest(*location);
            encoder.digest(*outline);
        });
        let generation = generation.finish();
        let source_root = IncludeRoot {
            identity: root_identity,
            path: parent,
            kind: IncludeRootKind::Source,
        };
        let authorities = IncludeAuthorities::new(unit, generation, [source_root.clone()])?;
        let capture = SourceCapture::open(
            IncludeAuthorities::new(unit, generation, [source_root])?,
            BTreeMap::new(),
        )?;
        let catalog = Arc::new(CompileInputCatalog::standalone(
            &root_view.outline().profile,
        )?);
        Ok((
            Self {
                environment,
                authorities,
                capture,
                generation,
                module_layouts,
                catalog,
            },
            prepared,
            root_view,
            fresh,
        ))
    }
    pub fn environment_identity(&self) -> Digest {
        self.environment.fingerprint()
    }
    /// Keep already admitted include captures available on both success and
    /// failure. An attachment error never replaces the primary discovery fault.
    pub fn discover_diagnostic(
        &self,
        discovery: &mut SourceDiscovery,
        db: &mut Database,
        snapshot: &mut Arc<crate::source::snapshot::Snapshot>,
    ) -> Result<(), Diagnostic> {
        self.discover_diagnostic_with(discovery, db, snapshot, None)
    }
    /// `discover_diagnostic` whose procedural steps execute through `procedural`.
    pub fn discover_diagnostic_with(
        &self,
        discovery: &mut SourceDiscovery,
        db: &mut Database,
        snapshot: &mut Arc<crate::source::snapshot::Snapshot>,
        procedural: Option<&mut super::proc_macros::ProceduralRun<'_>>,
    ) -> Result<(), Diagnostic> {
        let outcome = self.discover_with(discovery, db, procedural);
        let capture = discovery.diagnostic_snapshot(db, snapshot);
        match (outcome, capture) {
            (result, Ok(captured)) => {
                *snapshot = captured;
                result
            }
            (Err(mut primary), Err(attachment)) => {
                if primary.origin_attachment_failure.is_none() {
                    primary.origin_attachment_failure = Some(attachment.code);
                }
                Err(primary)
            }
            (Ok(()), Err(attachment)) => Err(attachment),
        }
    }
    pub fn discover(
        &self,
        discovery: &mut SourceDiscovery,
        db: &mut Database,
    ) -> Result<(), Diagnostic> {
        self.discover_with(discovery, db, None)
    }
    pub fn discover_with(
        &self,
        discovery: &mut SourceDiscovery,
        db: &mut Database,
        mut procedural: Option<&mut super::proc_macros::ProceduralRun<'_>>,
    ) -> Result<(), Diagnostic> {
        let context = DemandContext {
            request: discovery.key().fingerprint(),
            session: self.generation,
            unit: self.environment.unit(),
            revision: db.source_revision(),
            epoch: discovery.key().epoch,
            source_generation: self.generation,
            native_generation: None,
            compile_environment: self.environment.fingerprint(),
        };
        let mut coordinator = ExpansionCoordinator::new(context.clone());
        let mut phase_index = 0u32;
        let mut modules = BTreeMap::<Digest, InputRequest>::new();
        loop {
            let step = discovery.poll()?;
            if let Some(key) = step.compiler_key() {
                let phase = PhaseIdentity {
                    frame: self.generation,
                    body: BodyKey {
                        definition: key.invocation,
                        exact_body_bytes: key.input_tokens,
                    },
                    index: phase_index,
                };
                phase_index = phase_index
                    .checked_add(1)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-DISCOVERY-LIMIT",
                            "standalone compiler phase count limit",
                        )
                    })?;
                let query = QueryKey::CurrentExpansionInput(Box::new(key.clone()));
                match discovery.poll_compiler(db, &self.environment, &self.authorities)? {
                    EagerCompileInputPoll::NeedInput(
                        input @ InputRequest::CaptureInclude { .. },
                    ) => {
                        let InputRequest::CaptureInclude { request, .. } = &input else {
                            return Err(fault(
                                "LRR-E-COMPILE-INPUT",
                                "include request variant changed",
                            ));
                        };
                        let captured = self.capture.capture(request)?;
                        let token = coordinator.issue(query, phase, input)?;
                        coordinator.start(&context, &token)?;
                        coordinator.accept_include(&context, &token, &captured)?;
                        coordinator.adopt_include(&context, &token, db, captured)?;
                    }
                    EagerCompileInputPoll::Produced(record) => {
                        let input = InputRequest::Expand(Box::new(ExpansionRequest {
                            key: record.key().clone(),
                        }));
                        let token = coordinator.issue(query, phase, input)?;
                        coordinator.start(&context, &token)?;
                        coordinator.accept(
                            &context,
                            &token,
                            Reply::success(&token, record.output()),
                        )?;
                        coordinator.adopt(&context, &token, db, record)?;
                    }
                    EagerCompileInputPoll::Complete(record) => discovery.admit_compiler_expansion(
                        db,
                        &self.environment,
                        &self.authorities,
                        Arc::new(record),
                    )?,
                    EagerCompileInputPoll::NeedInput(_) => {
                        return Err(fault(
                            "LRR-E-COMPILE-INPUT",
                            "standalone compile input requires additional explicit preparation",
                        ));
                    }
                    EagerCompileInputPoll::NeedEnvironment { .. } => {
                        return Err(fault(
                            "LRR-E-COMPILE-INPUT",
                            "standalone environment macro requires declared captured environment",
                        ));
                    }
                    EagerCompileInputPoll::NeedOptionFacade { .. } => {
                        return Err(fault(
                            "LRR-E-COMPILE-INPUT",
                            "standalone compile input requires a prepared option facade",
                        ));
                    }
                    EagerCompileInputPoll::NeedExternal(request) => {
                        let token = coordinator.issue(query, phase, request.input())?;
                        coordinator.start(&context, &token)?;
                        let record = discovery.expand_compiler_string_operand(
                            db,
                            &self.environment,
                            &self.authorities,
                            &request,
                        )?;
                        coordinator.accept(
                            &context,
                            &token,
                            Reply::success(&token, record.output()),
                        )?;
                        coordinator.adopt(&context, &token, db, record)?;
                    }
                    EagerCompileInputPoll::NeedLookup { .. } => {
                        return Err(fault(
                            "LRR-E-CATALOG-INCOMPLETE",
                            "nested compile input requires complete source macro resolution and explicit expansion admission",
                        ));
                    }
                }
                continue;
            }
            let step = match procedural.as_deref_mut() {
                Some(run) => match run.step(db, discovery, step)? {
                    Some(step) => step,
                    None => continue,
                },
                None => step,
            };
            match step {
                SourceDiscoveryStep::NeedDeriveAppend { .. }
                | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
                | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
                | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
                | SourceDiscoveryStep::NeedDeriveDeclaration { .. }
                | SourceDiscoveryStep::NeedProceduralSelection { .. }
                | SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                    return Err(fault(
                        "LRR-E-MACRO-PERMISSION",
                        "procedural discovery requires an authorized prepared coordinator",
                    ));
                }
                SourceDiscoveryStep::NeedExpansion { request } => {
                    discovery.admit_expansion(Arc::new(request.expand()?))?
                }
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                    discovery.admit_expansion(Arc::new(request.expand()?))?
                }
                SourceDiscoveryStep::Sealed(_) => return Ok(()),
                SourceDiscoveryStep::Incomplete { diagnostic, .. } => {
                    let mut pending = Vec::new();
                    pending
                        .try_reserve_exact(
                            discovery
                                .unresolved_module_sources()
                                .count()
                                .checked_add(discovery.unresolved_ordinary_module_sources().count())
                                .ok_or_else(|| {
                                    fault("LRR-E-DISCOVERY-LIMIT", "module demand count limit")
                                })?,
                        )
                        .map_err(|_| fault("LRR-E-METADATA-LIMIT", "module demand allocation"))?;
                    pending.extend(
                        discovery
                            .unresolved_module_sources()
                            .map(|source| (source.producer(), false)),
                    );
                    pending.extend(
                        discovery
                            .unresolved_ordinary_module_sources()
                            .map(|source| (source.producer(), true)),
                    );
                    let mut progress = false;
                    for (producer, ordinary) in pending {
                        if discovery.module_namespace_complete(producer) {
                            continue;
                        }
                        if !modules.contains_key(&producer) {
                            let (caller, input) = if ordinary {
                                (
                                    discovery.ordinary_module_capture_caller(producer)?,
                                    discovery.prepare_ordinary_module_source(
                                        producer,
                                        &self.authorities,
                                    )?,
                                )
                            } else {
                                let caller = discovery.module_capture_caller(producer)?;
                                let directory =
                                    match discovery.adopted_module_caller_directory(producer)? {
                                        Some(directory) => directory,
                                        None => self
                                            .module_layouts
                                            .get(&caller.location_identity())
                                            .map(PathBuf::as_path)
                                            .ok_or_else(|| {
                                                fault(
                                                    "LRR-E-SOURCE-MODULE",
                                                    "module caller layout was not captured",
                                                )
                                            })?,
                                    };
                                let input = discovery.prepare_module_source_in_caller_context(
                                    producer,
                                    &self.authorities,
                                    directory,
                                )?;
                                (caller, input)
                            };
                            let InputRequest::CaptureModule { request, .. } = &input else {
                                return Err(fault(
                                    "LRR-E-COMPILE-INPUT",
                                    "module capture input variant changed",
                                ));
                            };
                            let captured = self.capture.capture_module(request)?;
                            let phase = PhaseIdentity {
                                frame: self.generation,
                                body: BodyKey {
                                    definition: producer,
                                    exact_body_bytes: input.fingerprint(),
                                },
                                index: phase_index,
                            };
                            phase_index = phase_index
                                .checked_add(1)
                                .ok_or_else(|| {
                                    fault(
                                        "LRR-E-DISCOVERY-LIMIT",
                                        "standalone module phase count limit",
                                    )
                                })?;
                            let token = coordinator.issue(
                                QueryKey::Outline(caller.outline),
                                phase,
                                input.clone(),
                            )?;
                            coordinator.start(&context, &token)?;
                            coordinator.accept_module(&context, &token, &captured)?;
                            coordinator.adopt_module(&context, &token, db, captured)?;
                            let staged = if ordinary {
                                discovery
                                    .stage_ordinary_module_source(db, &input)
                                    .map(|_| ())
                            } else {
                                discovery.stage_module_source(db, &input).map(|_| ())
                            };
                            match staged {
                                QueryPoll::Ready(Ok(_)) => {}
                                QueryPoll::Ready(Err(error)) => return Err(error),
                                QueryPoll::NeedInput(_) => {
                                    return Err(fault(
                                        "LRR-E-COMPILE-INPUT",
                                        "adopted module requires additional captured input",
                                    ));
                                }
                            }
                            modules.insert(producer, input);
                            progress = true;
                        } else {
                            let input = modules.get(&producer).ok_or_else(|| {
                                fault("LRR-E-COMPILE-INPUT", "module coordinator input absent")
                            })?;
                            let completed = if ordinary {
                                discovery
                                    .complete_ordinary_module_namespace(db, input)
                                    .map(|ready| matches!(ready, OrdinaryModuleReadiness::Ready(_)))
                            } else {
                                discovery
                                    .complete_module_namespace(db, input)
                                    .map(|ready| matches!(ready, ModuleSourceReadiness::Ready(_)))
                            };
                            match completed {
                                QueryPoll::Ready(Ok(true)) => progress = true,
                                QueryPoll::Ready(Ok(false)) => {}
                                QueryPoll::Ready(Err(error)) => return Err(error),
                                QueryPoll::NeedInput(_) => {
                                    return Err(fault(
                                        "LRR-E-COMPILE-INPUT",
                                        "module completion requires additional captured input",
                                    ));
                                }
                            }
                        }
                    }
                    if !progress {
                        return Err(diagnostic);
                    }
                }
            }
        }
    }
}

/// Explicit coordinator capture of one configured source-only macro facet.
/// The approved schema4 inventory supplies filesystem authority. Query providers
/// consume only the resulting immutable source/route handles, never this reader.
pub fn capture_macro_export_facet(
    manifest: &crate::project::tool_manifest::ToolManifest,
    unit: Digest,
    recorded_path: &Path,
    source: PreparedSourceView,
) -> Result<Arc<crate::expand::source_declarative::macro_exports::SourceMacroExportFacet>, Diagnostic>
{
    use std::io::Read;
    manifest.validate()?;
    let path = recorded_path.to_str().ok_or_else(|| {
        fault(
            "LRR-E-MACRO-EXPORT-PREPARATION",
            "macro-export path must be UTF-8",
        )
    })?;
    let expected = manifest
        .inputs
        .source_files
        .get(path)
        .and_then(|digest| *digest)
        .filter(|digest| *digest == source.selection().outline.parse.content)
        .ok_or_else(|| {
            fault(
                "LRR-E-MACRO-EXPORT-PREPARATION",
                "macro-export source is not recorded",
            )
        })?;
    // Validate the complete pure source/configuration join before opening a file.
    let facet = crate::expand::source_declarative::macro_exports::SourceMacroExportFacet::capture(
        manifest, unit, path, source,
    )?;
    let mut file = std::fs::File::open(recorded_path)
        .map_err(|error| fault("LRR-E-SOURCE-IO", error.to_string()))?;
    let size = usize::try_from(
        file.metadata()
            .map_err(|error| fault("LRR-E-SOURCE-IO", error.to_string()))?
            .len(),
    )
    .map_err(|_| fault("LRR-E-SOURCE-LIMIT", "macro-export source size overflow"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| fault("LRR-E-METADATA-LIMIT", "macro-export capture allocation"))?;
    let mut buffer = [0u8; 16384];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| fault("LRR-E-SOURCE-IO", error.to_string()))?;
        if count == 0 {
            break;
        }
        bytes
            .len()
            .checked_add(count)
            .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "macro-export capture length overflow"))?;
        bytes
            .try_reserve(count)
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "macro-export capture allocation"))?;
        bytes.extend_from_slice(&buffer[..count]);
    }
    if Digest::of_bytes(&bytes) != expected
        || bytes != facet.source().captured_macro_blob()?.bytes()
    {
        return Err(fault(
            "LRR-E-TOOL-PREPARE-STALE",
            "macro-export source changed since preparation",
        ));
    }
    Ok(Arc::new(facet))
}
