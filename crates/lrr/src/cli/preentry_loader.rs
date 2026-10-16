//! Independent pre-entry host metadata. No RunMachine, VM or executable thunk.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
        profile,
    },
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
    },
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::callables::CallSelection,
    source::{
        manifest::{BodyKey, ItemKind, Namespace},
        snapshot::{CfgSet, Snapshot},
    },
    syntax::body_index::BodyIndexLimits,
};
use serde::Serialize;
use std::{collections::BTreeMap, io::Write, path::Path};
const MAX_PACKET: usize = 4 * 1024 * 1024;
fn fault(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
#[derive(Serialize)]
struct File {
    path: String,
    content: Digest,
    bytes: u64,
}
#[derive(Serialize)]
struct SnapshotFacts {
    identity: Digest,
    owning_edition: u16,
    root_source: String,
    root_crate: Digest,
    root_cfg: Digest,
    files: Vec<File>,
}
#[derive(Serialize)]
struct Selection {
    kind: &'static str,
    declaration_namespace: &'static str,
    declaration_kind: &'static str,
    declared_name: String,
    module_path: Vec<String>,
    body: BodyKey,
    whole_body: bool,
}
#[derive(Serialize)]
struct IndexFacts {
    identity: Digest,
    body: BodyKey,
    profile: crate::core::profile::ProfileIdentity,
    cfg: Digest,
    nodes: u64,
    scopes: u64,
    slots: u32,
    metadata_bytes: u64,
}
#[derive(Serialize)]
struct SourceContext {
    path: String,
    outline: crate::db::key::OutlineKey,
    location: Digest,
}
#[derive(Serialize)]
struct DiscoveryFacts {
    seal_key: crate::db::seals::SealKey,
    seal: Digest,
    source_revision: u64,
    query_revision: u64,
    current_input_stamps: Vec<DependencyStamp>,
    seal_dependencies: Vec<(Digest, Digest)>,
    source_selections: Vec<SourceContext>,
    source_dependencies: Digest,
    compiler_environment: Digest,
    original_scope: crate::syntax::outline::ScopeKey,
}
#[derive(Serialize)]
pub(super) struct Packet {
    schema: &'static str,
    transport_stdout_role: &'static str,
    evidence_kind: &'static str,
    process_id: u32,
    request_context: Digest,
    status: &'static str,
    producer: Producer,
    source_root: Option<String>,
    snapshot: Option<SnapshotFacts>,
    root_cfg: Option<CfgSet>,
    entry_body: Option<BodyKey>,
    entry_selection: Option<Selection>,
    structural_index: Option<IndexFacts>,
    discovery: Option<DiscoveryFacts>,
    query_dependencies: Vec<DependencyStamp>,
    query_kinds: BTreeMap<String, u64>,
    guest_executions: u64,
    vm_dispatches: u64,
    executable_thunks: u64,
    compiler_tool_invocations: u64,
    procedural_macro_executions: u64,
    frontier: Option<Diagnostic>,
}
#[derive(Serialize)]
struct Producer {
    runtime_version: &'static str,
    semantics_profile: &'static str,
    lrir_schema: u32,
    metadata_schema: u32,
}
impl Packet {
    fn empty(context: Digest) -> Self {
        Self {
            schema: "lrr/v5/preentry-source-world/v1",
            transport_stdout_role: "host_metadata_json_not_guest_output",
            evidence_kind: "actual_standalone_source_capture_discovery_main_structure",
            process_id: std::process::id(),
            request_context: context,
            status: "frontier",
            producer: Producer {
                runtime_version: profile::RUNTIME_VERSION,
                semantics_profile: profile::SEMANTICS_PROFILE,
                lrir_schema: crate::ir::op::LRIR_RECIPE_SCHEMA,
                metadata_schema: 1,
            },
            source_root: None,
            snapshot: None,
            root_cfg: None,
            entry_body: None,
            entry_selection: None,
            structural_index: None,
            discovery: None,
            query_dependencies: Vec::new(),
            query_kinds: BTreeMap::new(),
            guest_executions: 0,
            vm_dispatches: 0,
            executable_thunks: 0,
            compiler_tool_invocations: 0,
            procedural_macro_executions: 0,
            frontier: None,
        }
    }
    pub(super) fn observe(path: &Path, edition: u16, context: Digest) -> Self {
        match Self::prepare(path, edition, context) {
            Ok(packet) => packet,
            Err(error) => {
                let mut packet = Self::empty(context);
                packet.frontier = Some(error);
                packet
            }
        }
    }
    fn prepare(path: &Path, edition: u16, context: Digest) -> Result<Self, Diagnostic> {
        // Same owning edition/cfg/module capture and pure compile-environment preparation as run.
        let (prepared, view, sources, snapshot) =
            super::capture_standalone_edition(path, false, edition)?;
        let (compiler, prepared, view, sources) =
            super::compile_inputs::PreparedCompileInputs::prepare(path, prepared, view, sources)?;
        if sources.len() > 2000 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "pre-entry source context count limit",
            ));
        }
        let mut source_selections = Vec::new();
        source_selections
            .try_reserve_exact(sources.len())
            .map_err(|_| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "pre-entry source selections allocation",
                )
            })?;
        for source in &sources {
            source_selections.push(source.selection().clone());
        }
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db)?;
        db.begin_expansion_epoch(1)?;
        let mut discovery = SourceDiscovery::new(
            sources,
            view.outline().root_scope.clone(),
            Namespace::Value,
            view.selection().outline.crate_key,
            1,
        )?
        .with_declarative_compiler(
            compiler.environment_identity(),
            crate::expand::source_declarative::engine_version(),
            compiler.catalog.clone(),
        )?;
        // poll performs structural discovery only. Never service any expansion or effect demand.
        let seal = match discovery.poll()? {
            SourceDiscoveryStep::Sealed(seal) => seal,
            SourceDiscoveryStep::Incomplete { diagnostic, .. } => return Err(diagnostic),
            SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
            | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
            | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveDeclaration { .. }
            | SourceDiscoveryStep::NeedAttributeSelection { .. }
            | SourceDiscoveryStep::NeedProceduralSelection { .. }
            | SourceDiscoveryStep::NeedExpansion { .. }
            | SourceDiscoveryStep::NeedGeneratedExpansion { .. } => {
                return Err(fault(
                    "LRR-E-PREENTRY-EXPANSION-REQUIRED",
                    "declaration discovery needs separately authorized expansion/library/world preparation; metadata leaf never invokes compiler or macro server",
                ));
            }
        };
        discovery.publish(&mut db)?;
        let callable = match discovery.entry_function(&mut db, "main") {
            QueryPoll::Ready(Ok(CallSelection::Source(callable))) => callable,
            QueryPoll::Ready(Ok(_)) => {
                return Err(fault(
                    "LRR-E-PREENTRY-MAIN",
                    "root main is absent, ambiguous, library-backed or non-source",
                ));
            }
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => {
                return Err(fault(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "root main requires additional declaration inputs",
                ));
            }
        };
        callable.revalidate(&mut db)?;
        let definition = callable.definition();
        if definition.namespace != Namespace::Value
            || definition.item_kind != ItemKind::Function
            || definition.declared_name != "main"
            || !definition.module_path.is_empty()
            || definition.crate_key != snapshot.project().selected_crate
        {
            return Err(fault(
                "LRR-E-PREENTRY-MAIN",
                "initial metadata lane requires the actual original root function main; reexport/generated entry needs later preparation",
            ));
        }
        let handle = callable
            .body()
            .ok_or_else(|| fault("LRR-E-PARSE-BLOCKS-ENTRY", "selected main body missing"))?;
        handle.require_entry()?;
        let body = handle.key();
        // This immutable view API indexes the full original selected body. It has no provider
        // or evaluator callback and never creates an executable recipe/guest frame.
        let index = view.index_body(view.selection(), &body, BodyIndexLimits::default())?;
        let cfg = view.captured_macro_cfg()?;
        if index.body() != &body || index.profile().edition != edition || index.cfg() != cfg {
            return Err(fault(
                "LRR-E-PREENTRY-CFG-UNSUPPORTED",
                "structural index/body/profile/cfg differs from actual captured owning world",
            ));
        }
        callable.revalidate(&mut db)?;
        let input = db
            .published_input(&InputKey::DiscoverySeal(seal.key()))
            .ok_or_else(|| {
                fault(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "current discovery input missing",
                )
            })?;
        let InputValue::DiscoverySeal(input) = input.0 else {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "discovery input variant differs",
            ));
        };
        if input.seal.fingerprint() != seal.fingerprint() {
            return Err(fault(
                "LRR-E-CATALOG-INCOMPLETE",
                "current discovery seal changed",
            ));
        }
        let source_dependencies = input.source_dependencies;
        if input.compiler_inputs.len() > 4096 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "pre-entry compiler dependency count limit",
            ));
        }
        let mut stamps = BTreeMap::new();
        for (key, fingerprint) in &input.compiler_inputs {
            stamps.insert(key.clone(), *fingerprint);
        }
        let mut keys = vec![
            InputKey::SourceDependencies,
            InputKey::CrateCfg(definition.crate_key),
            InputKey::DefinitionHeader(body.definition),
            InputKey::DefinitionSource(body.definition),
            InputKey::DefinitionBody(body.definition),
            InputKey::DeclarationContext(body.definition),
            InputKey::DiscoverySeal(seal.key()),
        ];
        for selected in &source_selections {
            keys.push(InputKey::CapturedSource(selected.location_identity()));
            keys.push(InputKey::SourceBlob(selected.outline.parse.content));
        }
        for key in keys {
            let (_, fingerprint) = db.published_input(&key).ok_or_else(|| {
                fault(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "current source/header/context/cfg/entry dependency missing",
                )
            })?;
            if stamps
                .insert(key, fingerprint)
                .is_some_and(|old| old != fingerprint)
            {
                return Err(fault(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "current input stamp conflict",
                ));
            }
        }
        if stamps.len() > 4096
            || seal.input_dependencies().len() > 4096
            || source_selections.len() > 10000
        {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "pre-entry dependency count limit",
            ));
        }
        let current_input_stamps = stamps
            .into_iter()
            .map(|(key, fingerprint)| DependencyStamp::Input { key, fingerprint })
            .collect();
        let mut query_dependencies = Vec::new();
        let mut query_kinds = BTreeMap::new();
        if db.trace.truncated != 0 || db.trace.events.len() > 4096 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "pre-entry query trace incomplete or over limit",
            ));
        }
        for event in &db.trace.events {
            match &event.key {
                QueryKey::CurrentModuleName(_)
                | QueryKey::SealedName(_)
                | QueryKey::DiscoverySeal(_)
                | QueryKey::ScopeDeclarationInventory(_)
                | QueryKey::ParseFile(_)
                | QueryKey::Outline(_)
                | QueryKey::ScopeBucket(_)
                | QueryKey::Signature(_)
                | QueryKey::FunctionSignature(_)
                | QueryKey::ReferenceHeader(_)
                | QueryKey::TypeAlias(_) => {}
                QueryKey::BodyStructure(_)
                | QueryKey::SourceConstraints(_)
                | QueryKey::SourceLocalPlan(_)
                | QueryKey::PatternPlan(_)
                | QueryKey::Thunk(_)
                | QueryKey::CurrentExpansionInput(_)
                | QueryKey::ExpansionInput(_)
                | QueryKey::TypeSchema(_)
                | QueryKey::ConstructorPlan(_)
                | QueryKey::BoundRecordConstructor(_)
                | QueryKey::Constant(_) => {
                    return Err(fault(
                        "LRR-E-PREENTRY-QUERY",
                        "metadata leaf observed a forbidden executable/expansion query",
                    ));
                }
            }
            let count = query_kinds
                .entry(event.key.kind().to_owned())
                .or_insert(0u64);
            *count = count
                .checked_add(1)
                .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "pre-entry query count overflow"))?;
            let count = query_dependencies
                .len()
                .checked_add(event.dependencies.len())
                .filter(|n| *n <= 4096)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "pre-entry query dependencies over limit",
                    )
                })?;
            query_dependencies
                .try_reserve_exact(count - query_dependencies.len())
                .map_err(|_| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "pre-entry query dependency allocation",
                    )
                })?;
            query_dependencies.extend(event.dependencies.iter().cloned());
        }
        query_dependencies.sort();
        query_dependencies.dedup();
        let mut packet = Self::empty(context);
        packet.source_root = Some(snapshot.project().identity.clone());
        packet.root_cfg = Some(cfg.clone());
        packet.snapshot = Some(snapshot_facts(&snapshot, edition)?);
        packet.entry_body = Some(body.clone());
        packet.entry_selection = Some(Selection {
            kind: "production_root_main_body_selection",
            declaration_namespace: "value",
            declaration_kind: "function",
            declared_name: definition.declared_name.clone(),
            module_path: definition.module_path.clone(),
            body: body.clone(),
            whole_body: true,
        });
        packet.structural_index = Some(IndexFacts {
            identity: index.fingerprint(),
            body,
            profile: index.profile().clone(),
            cfg: index.cfg().fingerprint(),
            nodes: to_u64(index.nodes().len())?,
            scopes: to_u64(index.scopes().len())?,
            slots: index.slot_count(),
            metadata_bytes: to_u64(index.estimated_bytes())?,
        });
        let mut captured_contexts = Vec::new();
        captured_contexts
            .try_reserve_exact(source_selections.len())
            .map_err(|_| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "pre-entry source context allocation",
                )
            })?;
        for selected in &source_selections {
            let mut path = String::new();
            path.try_reserve_exact(selected.path.as_str().len())
                .map_err(|_| fault("LRR-E-METADATA-LIMIT", "pre-entry source path allocation"))?;
            path.push_str(selected.path.as_str());
            captured_contexts.push(SourceContext {
                path,
                outline: selected.outline.clone(),
                location: selected.location_identity(),
            });
        }
        packet.discovery = Some(DiscoveryFacts {
            seal_key: seal.key(),
            seal: seal.fingerprint(),
            source_revision: db.source_revision(),
            query_revision: db.revision(),
            current_input_stamps,
            seal_dependencies: seal.input_dependencies().to_vec(),
            source_selections: captured_contexts,
            source_dependencies,
            compiler_environment: compiler.environment_identity(),
            original_scope: view.outline().root_scope.clone(),
        });
        packet.query_dependencies = query_dependencies;
        packet.query_kinds = query_kinds;
        packet.status = "ready";
        packet.render()?;
        Ok(packet)
    }
    pub(super) fn render(&self) -> Result<Vec<u8>, Diagnostic> {
        struct Bounded(Vec<u8>);
        impl Write for Bounded {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                let size = self
                    .0
                    .len()
                    .checked_add(bytes.len())
                    .filter(|n| *n <= MAX_PACKET)
                    .ok_or_else(|| std::io::Error::other("pre-entry packet byte limit"))?;
                self.0
                    .try_reserve(size - self.0.len())
                    .map_err(|_| std::io::Error::other("pre-entry packet allocation"))?;
                self.0.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = Bounded(Vec::new());
        serde_json::to_writer(&mut writer, self).map_err(|_| {
            fault(
                "LRR-E-PREENTRY-PROTOCOL",
                "bounded pre-entry packet serialization failed",
            )
        })?;
        Ok(writer.0)
    }
}
fn to_u64(value: usize) -> Result<u64, Diagnostic> {
    u64::try_from(value).map_err(|_| fault("LRR-E-METADATA-LIMIT", "metadata count width"))
}
fn snapshot_facts(snapshot: &Snapshot, edition: u16) -> Result<SnapshotFacts, Diagnostic> {
    if snapshot.files().len() > 10000 {
        return Err(fault(
            "LRR-E-METADATA-LIMIT",
            "pre-entry source count limit",
        ));
    }
    let mut files = Vec::new();
    files
        .try_reserve_exact(snapshot.files().len())
        .map_err(|_| fault("LRR-E-METADATA-LIMIT", "pre-entry source allocation"))?;
    let mut metadata = 0usize;
    for (path, blob) in snapshot.files() {
        metadata = metadata
            .checked_add(path.as_str().len())
            .and_then(|n| n.checked_add(128))
            .filter(|n| *n <= MAX_PACKET)
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "pre-entry source metadata limit"))?;
        files.push(File {
            path: path.as_str().into(),
            content: blob.content(),
            bytes: to_u64(blob.bytes().len())?,
        });
    }
    let project = snapshot.project();
    let cfg = snapshot
        .cfg_by_crate()
        .get(&project.selected_crate)
        .ok_or_else(|| fault("LRR-E-INTERNAL-SNAPSHOT", "pre-entry root cfg missing"))?;
    Ok(SnapshotFacts {
        identity: snapshot.id(),
        owning_edition: edition,
        root_source: project.root_source.as_str().into(),
        root_crate: project.selected_crate,
        root_cfg: cfg.fingerprint(),
        files,
    })
}
pub(super) fn entry(path: &Path, edition: u16, context: Digest) -> i32 {
    let packet = Packet::observe(path, edition, context);
    let code = if packet.status == "ready" { 0 } else { 2 };
    match packet.render() {
        Ok(bytes) => {
            let mut stdout = std::io::stdout().lock();
            if stdout
                .write_all(&bytes)
                .and_then(|_| stdout.write_all(b"\n"))
                .is_ok()
            {
                code
            } else {
                2
            }
        }
        Err(error) => {
            eprintln!("{error}");
            2
        }
    }
}
