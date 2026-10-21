//! Standalone procedural-macro auxiliaries: rustc/compiletest `--extern NAME`
//! for a `--crate-type proc-macro` crate, spelled `--extern proc-macro:NAME=PATH`.
//!
//! Explicit preparation, before source capture: each procedural crate root is
//! compiled as a host tool with the pinned compiler, the way compiletest builds
//! a `//@ proc-macro:` auxiliary (`--crate-type proc-macro --extern proc_macro`),
//! and the artifact is cached by compiler, edition, path and source identity.
//! The run then carries a schema4 preparation whose unit graph has the
//! standalone root as its single live consumer and one macro-host unit per
//! auxiliary. Expansion happens only through `service::run::MacroRun` in the
//! matched external macro server; this process never loads a macro library,
//! and the selected live source is never compiled.
use super::crate_graph::{ExternArg, ExternCrates};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile,
    },
    project::{
        cargo_units::{
            CargoUnitKey, DependencyEdge, NormalizedUnit, TargetKind, UnitGraph, UnitMode,
            UnitRole,
        },
        tool_manifest::{
            ArtifactKind, CompileEnv, InputKind, InputPlan, InputRoot,
            STANDALONE_PLANNER_SELECTION, ToolArtifact, ToolBinary, ToolManifest,
            ToolchainRecord,
        },
        toolchain::ProjectToolchain,
    },
    source::snapshot::{CfgSet, SourcePath},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::Command,
};

fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-TOOL-PREPARE", message.into())
}

/// The explicit preparation of one standalone run's procedural auxiliaries.
pub(super) struct StandaloneMacroTools {
    pub manifest: ToolManifest,
    pub plan: InputPlan,
    /// The standalone root crate's unit; the captured root crate takes this key.
    pub root: Digest,
}
impl StandaloneMacroTools {
    pub fn root_environment(&self) -> Result<&CompileEnv, Diagnostic> {
        self.manifest
            .compile_env
            .get(&self.root)
            .ok_or_else(|| fault("standalone root compile environment absent"))
    }
}

/// The cfg of a standalone root crate, as `crate_graph::capture` selects it.
pub(super) fn root_cfg(test_mode: bool) -> CfgSet {
    let mut cfg = CfgSet::guest_default();
    if test_mode {
        cfg.atoms.insert("test".into());
    }
    cfg
}

fn absolute(path: &Path) -> Result<String, Diagnostic> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| Diagnostic::new("LRR-E-SOURCE-IO", format!("{}: {e}", path.display())))?;
    canonical
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| fault("procedural preparation path must be UTF-8"))
}

/// Artifact cache: `LRR_PROC_MACRO_CACHE`, else `lrr-proc-macros` next to the
/// running binary (under the Cargo target directory for development builds).
fn cache_root() -> Result<PathBuf, Diagnostic> {
    let root = match std::env::var_os("LRR_PROC_MACRO_CACHE") {
        Some(root) => PathBuf::from(root),
        None => std::env::current_exe()
            .map_err(|e| fault(e.to_string()))?
            .parent()
            .ok_or_else(|| fault("executable directory absent"))?
            .join("lrr-proc-macros"),
    };
    std::fs::create_dir_all(&root).map_err(|e| fault(format!("{}: {e}", root.display())))?;
    std::fs::canonicalize(&root).map_err(|e| fault(e.to_string()))
}

fn hex(digest: Digest) -> String {
    digest.0.iter().map(|b| format!("{b:02x}")).collect()
}

/// Compile one procedural crate root as compiletest builds `//@ proc-macro:`
/// auxiliaries, reusing an identical earlier preparation.
fn compile(
    tools: &ProjectToolchain,
    cache: &Path,
    source: &str,
    edition: u16,
) -> Result<PathBuf, Diagnostic> {
    let bytes =
        std::fs::read(source).map_err(|e| Diagnostic::new("LRR-E-SOURCE-IO", e.to_string()))?;
    let mut key = StableEncoder::new("lrr/v2/standalone-proc-macro-artifact");
    key.digest(tools.compiler_content);
    key.string(&tools.compiler_version);
    key.string(source);
    key.u16(edition);
    key.digest(Digest::of_bytes(&bytes));
    let directory = cache.join(hex(key.finish()));
    let find = |directory: &Path| -> Result<Option<PathBuf>, Diagnostic> {
        if !directory.is_dir() {
            return Ok(None);
        }
        let mut found = None;
        for entry in std::fs::read_dir(directory).map_err(|e| fault(e.to_string()))? {
            let path = entry.map_err(|e| fault(e.to_string()))?.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with(std::env::consts::DLL_PREFIX)
                        && name.ends_with(std::env::consts::DLL_SUFFIX)
                })
            {
                if found.is_some() {
                    return Err(fault("procedural preparation produced several libraries"));
                }
                found = Some(path);
            }
        }
        Ok(found)
    };
    if let Some(artifact) = find(&directory)? {
        return Ok(artifact);
    }
    let staging = cache.join(format!(
        "staging-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| fault(e.to_string()))?
            .as_nanos()
    ));
    std::fs::create_dir_all(&staging).map_err(|e| fault(e.to_string()))?;
    // compiletest: `--crate-type proc-macro --extern proc_macro`, host target,
    // the auxiliary's own edition; its stage compiler accepts `#![feature]`.
    let output = Command::new(&tools.compiler)
        .args(["--crate-type", "proc-macro", "--extern", "proc_macro"])
        .arg("--edition")
        .arg(edition.to_string())
        .arg("--out-dir")
        .arg(&staging)
        .arg(source)
        .env("RUSTC_BOOTSTRAP", "1")
        .output()
        .map_err(|e| fault(format!("procedural auxiliary compiler: {e}")))?;
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(fault(format!(
            "procedural auxiliary {source} failed to compile:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    if find(&staging)?.is_none() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(fault("procedural auxiliary produced no library"));
    }
    if std::fs::rename(&staging, &directory).is_err() {
        // A concurrent identical preparation published first; use it.
        let _ = std::fs::remove_dir_all(&staging);
    }
    find(&directory)?.ok_or_else(|| fault("procedural preparation cache entry absent"))
}

fn edges_digest(edges: &[DependencyEdge]) -> Digest {
    let mut encoder = StableEncoder::new("lrr/v2/cargo-unit-edges");
    encoder.sequence(edges, |e, d| {
        e.string(&d.alias);
        e.digest(d.target);
        e.bool(d.public);
        e.bool(d.no_prelude);
        e.bool(d.no_unused);
    });
    encoder.finish()
}

fn version(command: &mut Command) -> Result<String, Diagnostic> {
    let output = command.output().map_err(|e| fault(e.to_string()))?;
    if !output.status.success() {
        return Err(fault("tool version capture failed"));
    }
    String::from_utf8(output.stdout).map_err(|_| fault("tool version is not UTF-8"))
}

fn server_version(server: &Path) -> Result<String, Diagnostic> {
    version(
        Command::new(server)
            .env_clear()
            .env("RUST_ANALYZER_INTERNALS_DO_NOT_USE", "this is unstable")
            .arg("--version"),
    )
}

/// The matched macro server. Auxiliaries that print to native stdout corrupt
/// the stock sysroot server's protocol stream (E-M013-001), so the explicitly
/// prepared stdio-v1 repair is used when it is built (`LRR_PROC_MACRO_SERVER`,
/// else this checkout's `tools/prepared_macro_server`) and reports the exact
/// pinned banner for this compiler; otherwise the sysroot server.
fn select_server(tools: &ProjectToolchain) -> Result<(PathBuf, String), Diagnostic> {
    let candidate = std::env::var_os("LRR_PROC_MACRO_SERVER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/prepared_macro_server/target/debug/rust-analyzer-proc-macro-srv")
        });
    if let Ok(candidate) = std::fs::canonicalize(&candidate)
        && let Ok(banner) = server_version(&candidate)
        && banner.trim()
            == crate::project::tool_manifest::standalone_repaired_server_banner(
                &tools.compiler_commit,
            )
    {
        return Ok((candidate, banner));
    }
    Ok((tools.server.clone(), server_version(&tools.server)?))
}

/// Prepare the run's procedural auxiliaries, or `None` when it has none.
pub(super) fn prepare(
    path: &Path,
    edition: u16,
    test_mode: bool,
    externs: &ExternCrates,
) -> Result<Option<StandaloneMacroTools>, Diagnostic> {
    let rows: Vec<&ExternArg> = externs.crates.iter().filter(|row| row.proc_macro).collect();
    if rows.is_empty() {
        return Ok(None);
    }
    let tools = crate::project::toolchain::capture("1.99.0")?;
    let cache = cache_root()?;
    let profile_fingerprint = Digest::of_bytes(b"lrr/v2/standalone-dev-profile");
    let mut units = BTreeMap::new();
    let mut artifacts = BTreeMap::new();
    let mut compile_env = BTreeMap::new();
    let mut files = BTreeMap::<PathBuf, BTreeSet<SourcePath>>::new();
    let mut grant = |source: &str| -> Result<(), Diagnostic> {
        let source = Path::new(source);
        files
            .entry(
                source
                    .parent()
                    .ok_or_else(|| fault("source parent absent"))?
                    .to_owned(),
            )
            .or_default()
            .insert(SourcePath::new(Path::new(
                source
                    .file_name()
                    .ok_or_else(|| fault("source file name absent"))?,
            ))?);
        Ok(())
    };
    let mut edges = Vec::new();
    let mut by_alias = BTreeMap::new();
    for row in rows {
        let source = absolute(&row.path)?;
        let macro_edition = externs.editions.get(&row.name).copied().unwrap_or(edition);
        let artifact = compile(&tools, &cache, &source, macro_edition)?;
        let artifact_bytes = std::fs::read(&artifact).map_err(|e| fault(e.to_string()))?;
        let mut cfg = CfgSet::guest_default();
        cfg.atoms.insert("proc_macro".into());
        let key = CargoUnitKey {
            package: format!("standalone-proc-macro:{source}"),
            target_name: row.name.clone(),
            target_kind: TargetKind::ProcMacro,
            role: UnitRole::ProcMacroHost,
            compile_target: tools.host.clone(),
            test_intent: false,
            edition: macro_edition,
            features: BTreeSet::new(),
            profile_fingerprint,
            cfg_fingerprint: cfg.fingerprint(),
            dependency_edges: edges_digest(&[]),
            rustc_fingerprint: tools.compiler_content,
        };
        let id = key.fingerprint();
        if let Some(old) = by_alias.insert(row.name.clone(), id) {
            if old != id {
                return Err(fault(format!(
                    "`--extern proc-macro:{}` names two different crates",
                    row.name
                )));
            }
            continue;
        }
        grant(&source)?;
        let directory = Path::new(&source)
            .parent()
            .and_then(Path::to_str)
            .ok_or_else(|| fault("procedural source directory absent"))?
            .to_owned();
        compile_env.insert(
            id,
            CompileEnv {
                unit: id,
                host: tools.host.clone(),
                target: tools.host.clone(),
                cfg,
                cargo: BTreeMap::from([("CARGO_MANIFEST_DIR".into(), directory)]),
                rustc_env: BTreeMap::new(),
                declared: BTreeMap::new(),
                out_dir: None,
            },
        );
        artifacts.insert(
            id,
            ToolArtifact {
                auxiliary: BTreeMap::new(),
                path: absolute(&artifact)?,
                content: Digest::of_bytes(&artifact_bytes),
                kind: ArtifactKind::ProcMacroLibrary,
            },
        );
        units.insert(
            id,
            NormalizedUnit {
                key,
                mode: UnitMode::Build,
                source_path: source,
                dependencies: vec![],
            },
        );
        edges.push(DependencyEdge {
            alias: row.name.clone(),
            target: id,
            public: false,
            no_prelude: !row.prelude,
            no_unused: false,
        });
    }
    edges.sort();
    let root_source = absolute(path)?;
    grant(&root_source)?;
    let cfg = root_cfg(test_mode);
    let root_key = CargoUnitKey {
        package: format!("standalone:{root_source}"),
        target_name: Path::new(&root_source)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("main")
            .to_owned(),
        target_kind: TargetKind::Binary,
        role: UnitRole::Live,
        compile_target: profile::GUEST_TARGET.into(),
        test_intent: test_mode,
        edition,
        features: BTreeSet::new(),
        profile_fingerprint,
        cfg_fingerprint: cfg.fingerprint(),
        dependency_edges: edges_digest(&edges),
        rustc_fingerprint: tools.compiler_content,
    };
    let root = root_key.fingerprint();
    // Identical to the environment `compile_inputs` freezes for the root.
    compile_env.insert(
        root,
        CompileEnv {
            unit: root,
            host: profile::GUEST_TARGET.into(),
            target: profile::GUEST_TARGET.into(),
            cfg,
            cargo: BTreeMap::new(),
            rustc_env: BTreeMap::new(),
            declared: BTreeMap::new(),
            out_dir: None,
        },
    );
    units.insert(
        root,
        NormalizedUnit {
            key: root_key,
            mode: if test_mode {
                UnitMode::Test
            } else {
                UnitMode::Check
            },
            source_path: root_source,
            dependencies: edges,
        },
    );
    let plan = InputPlan::new(
        files
            .into_iter()
            .map(|(directory, files)| {
                InputRoot::open(
                    InputKind::Source,
                    &directory,
                    files,
                    BTreeSet::new(),
                    false,
                    BTreeSet::new(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        BTreeSet::new(),
    )?;
    let inputs = plan.capture()?.inventory().clone();
    let (server, server_version) = select_server(&tools)?;
    let server_bytes = std::fs::read(&server).map_err(|e| fault(e.to_string()))?;
    let text = |path: &Path| -> Result<String, Diagnostic> {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| fault("tool path must be UTF-8"))
    };
    let compiler = ToolBinary {
        path: text(&tools.compiler)?,
        content: tools.compiler_content,
        selection: "1.99.0".into(),
        version_verbose: tools.compiler_version.clone(),
    };
    let manifest = ToolManifest {
        schema_version: 4,
        semantics_profile: profile::SEMANTICS_PROFILE.into(),
        toolchain: ToolchainRecord {
            planner: ToolBinary {
                selection: STANDALONE_PLANNER_SELECTION.into(),
                ..compiler.clone()
            },
            compiler,
            compiler_commit: tools.compiler_commit.clone(),
            host: tools.host.clone(),
            sysroot: text(&tools.sysroot)?,
            macro_server: ToolBinary {
                path: text(&server)?,
                content: Digest::of_bytes(&server_bytes),
                selection: "1.99.0".into(),
                version_verbose: server_version,
            },
            macro_protocol: 6,
            component_manifest: text(&tools.server_component_manifest)?,
            component_manifest_content: tools.component_manifest_content,
            prepared_macro_server: None,
        },
        graph: UnitGraph {
            schema: 1,
            roots: BTreeSet::from([root]),
            units,
        },
        inputs,
        artifacts,
        compile_env,
        build_outputs: BTreeMap::new(),
    };
    manifest.validate()?;
    Ok(Some(StandaloneMacroTools {
        manifest,
        plan,
        root,
    }))
}

/// One standalone request's procedural macro run: a single consumer session
/// shared by every discovery seal of the request epoch (Type, then Value or
/// TestCatalog), so each invocation executes once.
pub(super) struct ProceduralRun<'a> {
    tools: &'a StandaloneMacroTools,
    run: crate::service::run::MacroRun<'a>,
    request: Digest,
    catalog: std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
    cache: crate::expand::cache::ExpansionOutputCache,
    pending: Option<std::sync::Arc<crate::service::function_macro_requests::PreparedFunctionRequest>>,
    phase: u32,
}
impl<'a> ProceduralRun<'a> {
    pub fn begin(
        db: &mut crate::db::memo::Database,
        tools: &'a StandaloneMacroTools,
    ) -> Result<Self, Diagnostic> {
        use crate::db::dependency::{InputKey, InputValue};
        let epoch = match db.input(&InputKey::ExpansionEpoch).as_deref() {
            Some(InputValue::ExpansionEpoch(epoch)) => *epoch,
            _ => return Err(fault("procedural run requires a published expansion epoch")),
        };
        let mut request = StableEncoder::new("lrr/v2/standalone-procedural-request");
        request.digest(tools.manifest.fingerprint());
        request.u64(epoch);
        let request = request.finish();
        let context = crate::service::expansion::DemandContext {
            request,
            session: tools.manifest.fingerprint(),
            unit: tools.root,
            revision: db.source_revision(),
            epoch,
            source_generation: tools.manifest.inputs.fingerprint(),
            native_generation: None,
            compile_environment: tools.root_environment()?.fingerprint(),
        };
        let mut run =
            crate::service::run::MacroRun::begin(db, context, &tools.manifest, &tools.plan, true)?;
        let catalog = run.catalog(db, tools.root)?;
        Ok(Self {
            tools,
            run,
            request,
            catalog,
            cache: crate::expand::cache::ExpansionOutputCache::new(usize::MAX)?,
            pending: None,
            phase: 0,
        })
    }
    pub(crate) fn source_catalog(
        &self,
    ) -> std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog> {
        std::sync::Arc::clone(&self.catalog)
    }
    /// The prepared unit graph every procedural discovery seal is keyed by.
    pub fn graph(&self) -> Digest {
        self.tools.manifest.graph.fingerprint()
    }
    /// Install the compiler facets and this consumer's procedural catalog, and
    /// attach the run's expansion ledger.
    pub fn configure(
        &self,
        discovery: crate::expand::source_discovery::SourceDiscovery,
        compile_env: Digest,
        compiler: std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>,
    ) -> Result<crate::expand::source_discovery::SourceDiscovery, Diagnostic> {
        discovery
            .with_declarative_compiler_procedural(
                compile_env,
                crate::expand::source_declarative::engine_version(),
                compiler,
                self.catalog.clone(),
            )?
            .with_expansion_budget(self.request, self.run.expansion_budget())
    }
    pub(crate) fn configure_prepared(
        &self,
        discovery: crate::expand::source_discovery::SourceDiscovery,
        compile_env: Digest,
        compiler: std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>,
    ) -> Result<crate::expand::source_discovery::SourceCompilerPreparation, Diagnostic> {
        discovery
            .with_expansion_budget(self.request, self.run.expansion_budget())?
            .prepare_with_declarative_compiler_procedural(
                compile_env,
                crate::expand::source_declarative::engine_version(),
                compiler,
                self.catalog.clone(),
            )
    }
    fn phase(
        &mut self,
        key: &crate::expand::identity::ExpansionKey,
    ) -> Result<crate::service::expansion::PhaseIdentity, Diagnostic> {
        let index = self.phase;
        self.phase = index
            .checked_add(1)
            .ok_or_else(|| fault("procedural phase count overflow"))?;
        Ok(crate::service::expansion::PhaseIdentity {
            frame: self.request,
            body: crate::source::manifest::BodyKey {
                definition: key.invocation,
                exact_body_bytes: key.input_tokens,
            },
            index,
        })
    }
    /// Fulfil one procedural discovery step through the matched server.
    /// Returns the step back when it is not procedural.
    pub fn step(
        &mut self,
        db: &mut crate::db::memo::Database,
        discovery: &mut crate::expand::source_discovery::SourceDiscovery,
        step: crate::expand::source_discovery::SourceDiscoveryStep,
    ) -> Result<Option<crate::expand::source_discovery::SourceDiscoveryStep>, Diagnostic> {
        use crate::expand::source_discovery::SourceDiscoveryStep as Step;
        match step {
            Step::NeedProceduralSelection { selection } => {
                let request = self.run.prepare_selected_source(db, &selection)?;
                discovery.bind_procedural_request(request.clone())?;
                self.pending = Some(request);
            }
            Step::NeedAttributeSelection { selection } => {
                let request = self.run.prepare_attribute_selection(db, &selection)?;
                discovery.bind_attribute_request(request.clone())?;
                self.pending = Some(request);
            }
            Step::NeedExpansion { request } if request.procedural_request().is_some() => {
                let prepared = self
                    .pending
                    .take()
                    .filter(|prepared| prepared.key() == request.key())
                    .ok_or_else(|| fault("bound procedural request differs from expansion"))?;
                let phase = self.phase(prepared.key())?;
                self.run
                    .fulfil_discovery(db, discovery, phase, &prepared, &mut self.cache)?;
            }
            Step::NeedGeneratedExpansion { request } if request.procedural_request().is_some() => {
                let phase = self.phase(request.key())?;
                self.run.fulfil_generated_discovery(
                    db,
                    discovery,
                    phase,
                    &request,
                    &mut self.cache,
                )?;
            }
            Step::NeedSourceDeriveSelection { selection } => {
                let request = self.run.prepare_source_derive_selection(db, &selection)?;
                discovery.bind_source_derive_request(request)?;
            }
            Step::NeedSourceDeriveExpansion { request } => {
                let phase = self.phase(request.key())?;
                self.run
                    .fulfil_source_derive(db, discovery, phase, &request, &mut self.cache)?;
            }
            Step::NeedSourceDeriveAppend { request, record } => {
                self.run
                    .integrate_source_derive_append(db, discovery, &request, record)?;
            }
            Step::NeedDeriveAppend { request, record } => {
                self.run
                    .integrate_derive_append(db, discovery, &request, record)?;
            }
            Step::NeedDeriveDeclaration { request } => {
                // A derive emitted by a pure declarative expansion: adopt that
                // exact parent record so its provenance is checkable.
                if let crate::expand::derive_origin::DeriveDeclarationOrigin::Generated(origin) =
                    &request.origin
                    && origin.key.procedural_epoch.is_none()
                {
                    let record = discovery
                        .expansion_record(&origin.key)
                        .filter(|record| record.descriptor().fingerprint() == origin.fingerprint())
                        .ok_or_else(|| fault("derive declaration parent record absent"))?;
                    db.publish_expansion(record.as_ref().clone())?;
                }
                match self.run.resolve_derive_declaration(db, &request)? {
                    crate::service::derive_declarations::DeriveDeclarationPoll::Ready(receipt) => {
                        discovery.adopt_derive_declaration(
                            db,
                            &request,
                            std::sync::Arc::new(receipt),
                        )?
                    }
                    _ => {
                        return Err(Diagnostic::new(
                            "LRR-E-CATALOG-INCOMPLETE",
                            "procedural derive declaration requires macro-generated producer source"
                                .into(),
                        ));
                    }
                }
            }
            other => return Ok(Some(other)),
        }
        Ok(None)
    }
    /// Publish a consumer seal; the run stays open for the request's next seal.
    pub fn publish(
        &mut self,
        db: &mut crate::db::memo::Database,
        discovery: &mut crate::expand::source_discovery::SourceDiscovery,
    ) -> Result<(), Diagnostic> {
        self.run
            .publish_discovery_retained(db, discovery)
            .map(|_| ())
    }
}

#[cfg(test)]
#[path = "proc_macros_tests.rs"]
mod tests;

impl crate::service::source_preparation::SourceExpansionExecutor for ProceduralRun<'_> {
    fn step(
        &mut self,
        db: &mut crate::db::memo::Database,
        discovery: &mut crate::expand::source_discovery::SourceDiscovery,
        step: crate::expand::source_discovery::SourceDiscoveryStep,
    ) -> Result<Option<crate::expand::source_discovery::SourceDiscoveryStep>, Diagnostic> {
        ProceduralRun::step(self, db, discovery, step)
    }
}
