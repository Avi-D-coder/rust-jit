//! Schema4 preparation records. Decoding is pure; publication is coordinator-only.
use super::cargo_units::{TargetKind, UnitGraph, UnitMode, UnitRole};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::{SEMANTICS_PROFILE, is_supported_preparation_target},
    },
    source::{
        generated::GeneratedTree,
        snapshot::{CfgSet, SourcePath},
    },
};
use cap_std::fs::{Dir, OpenOptions};
#[path = "tool_manifest_inputs.rs"]
mod inputs;
pub use inputs::{InputCapture, InputKind, InputPlan, InputRoot, verify_adoption};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
const MAX_WIRE: usize = 64 * 1024 * 1024;
const MAX_RECORDS: usize = 100_000;
const MAX_STRING: usize = 64 * 1024;
pub(super) fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-TOOLCHAIN-CAPABILITY", message.into())
}
fn stale() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-TOOL-PREPARE-STALE",
        "recorded preparation inputs changed; repeat explicit preparation".into(),
    )
}
fn text(value: &str) -> Result<(), Diagnostic> {
    if value.len() > MAX_STRING || value.contains('\0') {
        Err(fault("invalid or oversized manifest string"))
    } else {
        Ok(())
    }
}
pub(super) fn absolute(value: &str) -> Result<(), Diagnostic> {
    text(value)?;
    if !Path::new(value).is_absolute()
        || Path::new(value).components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(fault("preparation path must be absolute"));
    }
    if Path::new(value)
        .components()
        .collect::<std::path::PathBuf>()
        .as_os_str()
        != value
    {
        return Err(fault("preparation path must be normalized"));
    }
    Ok(())
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolBinary {
    pub path: String,
    pub content: Digest,
    pub selection: String,
    pub version_verbose: String,
}
impl StableEncode for ToolBinary {
    const DOMAIN: &'static str = "lrr/v2/tool-binary";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(&self.path);
        e.digest(self.content);
        e.string(&self.selection);
        e.string(&self.version_verbose);
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainRecord {
    pub planner: ToolBinary,
    pub compiler: ToolBinary,
    pub compiler_commit: String,
    pub host: String,
    pub sysroot: String,
    pub macro_server: ToolBinary,
    pub macro_protocol: u32,
    pub component_manifest: String,
    pub component_manifest_content: Digest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepared_macro_server: Option<super::prepared_macro_server::PreparedMacroServer>,
}
impl StableEncode for ToolchainRecord {
    const DOMAIN: &'static str = "lrr/v2/toolchain-record/prepared-server-v1";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.planner);
        e.record(&self.compiler);
        e.string(&self.compiler_commit);
        e.string(&self.host);
        e.string(&self.sysroot);
        e.record(&self.macro_server);
        e.u32(self.macro_protocol);
        e.string(&self.component_manifest);
        e.digest(self.component_manifest_content);
        match &self.prepared_macro_server {
            None => e.u8(0),
            Some(record) => {
                e.u8(1);
                e.record(record)
            }
        }
    }
}
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputInventory {
    pub source_files: BTreeMap<String, Option<Digest>>,
    pub external_files: BTreeMap<String, Option<Digest>>,
    pub environment: BTreeMap<String, Option<String>>,
    pub directory_membership: BTreeMap<String, BTreeSet<SourcePath>>,
}
impl InputInventory {
    pub fn validate(&self) -> Result<(), Diagnostic> {
        let n = [
            self.source_files.len(),
            self.external_files.len(),
            self.environment.len(),
            self.directory_membership.len(),
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(|| fault("input count overflow"))?;
        if n > MAX_RECORDS {
            return Err(fault("too many preparation inputs"));
        }
        for (path, hash) in &self.source_files {
            absolute(path)?;
            if let Some(other) = self.external_files.get(path)
                && other != hash
            {
                return Err(fault("conflicting recorded source/external input"));
            }
        }
        for path in self.external_files.keys() {
            absolute(path)?;
        }
        for (key, value) in &self.environment {
            env_name(key)?;
            if let Some(value) = value {
                text(value)?;
            }
        }
        let mut count = n;
        for (path, members) in &self.directory_membership {
            absolute(path)?;
            count = count
                .checked_add(members.len())
                .ok_or_else(|| fault("membership count overflow"))?;
            if count > MAX_RECORDS {
                return Err(fault("input membership limit"));
            }
        }
        Ok(())
    }
}
impl StableEncode for InputInventory {
    const DOMAIN: &'static str = "lrr/v2/tool-input-inventory";
    fn encode_fields(&self, e: &mut StableEncoder) {
        for files in [&self.source_files, &self.external_files] {
            e.map(files, |e, path, hash| {
                e.string(path);
                e.option(hash, |e, h| e.digest(*h));
            });
        }
        e.map(&self.environment, |e, key, value| {
            e.string(key);
            e.option(value, |e, v| e.string(v));
        });
        e.map(&self.directory_membership, |e, path, members| {
            e.string(path);
            e.u64(members.len() as u64);
            for member in members {
                e.string(member.as_str());
            }
        });
    }
}
fn env_name(name: &str) -> Result<(), Diagnostic> {
    text(name)?;
    if name.is_empty() || name.contains('=') {
        return Err(fault("invalid compile environment name"));
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompileEnv {
    pub unit: Digest,
    pub host: String,
    pub target: String,
    pub cfg: CfgSet,
    pub cargo: BTreeMap<String, String>,
    pub rustc_env: BTreeMap<String, String>,
    pub declared: BTreeMap<String, Option<String>>,
    pub out_dir: Option<String>,
}
impl StableEncode for CompileEnv {
    const DOMAIN: &'static str = "lrr/v2/unit-compile-env";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.unit);
        e.string(&self.host);
        e.string(&self.target);
        e.record(&self.cfg);
        for map in [&self.cargo, &self.rustc_env] {
            e.map(map, |e, k, v| {
                e.string(k);
                e.string(v);
            });
        }
        e.map(&self.declared, |e, k, v| {
            e.string(k);
            e.option(v, |e, v| e.string(v));
        });
        e.option(&self.out_dir, |e, v| e.string(v));
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum ArtifactKind {
    HostLibrary,
    BuildProgram,
    ProcMacroLibrary,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolArtifact {
    pub auxiliary: BTreeMap<String, Digest>,
    pub path: String,
    pub content: Digest,
    pub kind: ArtifactKind,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildOutput {
    pub producer: Digest,
    pub consumers: BTreeSet<Digest>,
    pub tree: GeneratedTree,
    pub cfgs: BTreeSet<String>,
    pub rustc_env: BTreeMap<String, String>,
    pub linked_libs: Vec<String>,
    pub linked_paths: Vec<String>,
    pub dependency_metadata: BTreeMap<String, String>,
}
impl StableEncode for BuildOutput {
    const DOMAIN: &'static str = "lrr/v2/build-output";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.producer);
        e.u64(self.consumers.len() as u64);
        for unit in &self.consumers {
            e.digest(*unit);
        }
        e.record(&self.tree);
        e.u64(self.cfgs.len() as u64);
        for cfg in &self.cfgs {
            e.string(cfg);
        }
        for map in [&self.rustc_env, &self.dependency_metadata] {
            e.map(map, |e, k, v| {
                e.string(k);
                e.string(v);
            });
        }
        e.sequence(&self.linked_libs, |e, v| e.string(v));
        e.sequence(&self.linked_paths, |e, v| e.string(v));
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolManifest {
    pub schema_version: u32,
    pub semantics_profile: String,
    pub toolchain: ToolchainRecord,
    pub graph: UnitGraph,
    pub inputs: InputInventory,
    pub artifacts: BTreeMap<Digest, ToolArtifact>,
    pub compile_env: BTreeMap<Digest, CompileEnv>,
    pub build_outputs: BTreeMap<Digest, BuildOutput>,
}
impl StableEncode for ToolManifest {
    const DOMAIN: &'static str = "lrr/v2/tool-manifest-schema4";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(self.schema_version);
        e.string(&self.semantics_profile);
        e.record(&self.toolchain);
        e.record(&self.inputs);
        e.u32(self.graph.schema);
        e.u64(self.graph.roots.len() as u64);
        for root in &self.graph.roots {
            e.digest(*root);
        }
        e.map(&self.graph.units, |e, id, u| {
            e.digest(*id);
            e.record(&u.key);
            e.string(&u.source_path);
            e.u32(match u.mode {
                UnitMode::Build => 1,
                UnitMode::Check => 2,
                UnitMode::Test => 3,
                UnitMode::RunBuildScript => 4,
            });
            e.sequence(&u.dependencies, |e, d| {
                e.string(&d.alias);
                e.digest(d.target);
                e.bool(d.public);
                e.bool(d.no_prelude);
                e.bool(d.no_unused);
            });
        });
        e.map(&self.artifacts, |e, id, a| {
            e.digest(*id);
            e.string(&a.path);
            e.digest(a.content);
            e.map(&a.auxiliary, |e, path, content| {
                e.string(path);
                e.digest(*content);
            });
            e.u32(match a.kind {
                ArtifactKind::HostLibrary => 1,
                ArtifactKind::BuildProgram => 2,
                ArtifactKind::ProcMacroLibrary => 3,
            });
        });
        e.map(&self.compile_env, |e, id, env| {
            e.digest(*id);
            e.record(env);
        });
        e.map(&self.build_outputs, |e, id, out| {
            e.digest(*id);
            e.record(out);
        });
    }
}
impl ToolManifest {
    pub fn validate(&self) -> Result<(), Diagnostic> {
        if self.schema_version != 4 || self.semantics_profile != SEMANTICS_PROFILE {
            return Err(fault(
                "requires preparation schema4/lrr-2024-v2; repeat explicit preparation",
            ));
        }
        for tool in [
            &self.toolchain.planner,
            &self.toolchain.compiler,
            &self.toolchain.macro_server,
        ] {
            absolute(&tool.path)?;
            text(&tool.selection)?;
            text(&tool.version_verbose)?;
            if tool.version_verbose.is_empty() {
                return Err(fault("missing tool version capture"));
            }
        }
        if self.toolchain.planner.selection != "nightly-2026-10-01"
            || self.toolchain.compiler.selection != "1.99.0"
            || self.toolchain.macro_server.selection != "1.99.0"
            || self.toolchain.macro_protocol != 6
            || self.toolchain.compiler_commit.len() != 40
            || !self
                .toolchain
                .compiler_commit
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
            || !is_supported_preparation_target(&self.toolchain.host)
        {
            return Err(fault("unmatched toolchain/server tuple"));
        }
        absolute(&self.toolchain.sysroot)?;
        absolute(&self.toolchain.component_manifest)?;
        if !Path::new(&self.toolchain.component_manifest).starts_with(&self.toolchain.sysroot) {
            return Err(fault("server/component outside recorded sysroot"));
        }
        match &self.toolchain.prepared_macro_server {
            None if !Path::new(&self.toolchain.macro_server.path)
                .starts_with(&self.toolchain.sysroot) =>
            {
                return Err(fault("server/component outside recorded sysroot"));
            }
            None => {}
            Some(prepared) => prepared.validate(&self.toolchain)?,
        }
        self.inputs.validate()?;
        if self.graph.schema != 1
            || self.graph.roots.len() != 1
            || self.graph.units.is_empty()
            || self.graph.units.len() > MAX_RECORDS
            || self.artifacts.len() > MAX_RECORDS
            || self.compile_env.len() > MAX_RECORDS
            || self.build_outputs.len() > MAX_RECORDS
        {
            return Err(fault("unsupported or oversized unit graph"));
        }
        let mut visited = BTreeSet::new();
        let mut active = BTreeSet::new();
        fn visit(
            graph: &UnitGraph,
            id: Digest,
            depth: usize,
            active: &mut BTreeSet<Digest>,
            visited: &mut BTreeSet<Digest>,
        ) -> Result<(), Diagnostic> {
            if visited.contains(&id) {
                return Ok(());
            }
            if depth >= 256 || !active.insert(id) {
                return Err(fault("persisted unit graph cycle/depth limit"));
            }
            let unit = graph
                .units
                .get(&id)
                .ok_or_else(|| fault("dangling unit dependency"))?;
            for edge in &unit.dependencies {
                visit(graph, edge.target, depth + 1, active, visited)?;
            }
            active.remove(&id);
            visited.insert(id);
            Ok(())
        }
        for root in &self.graph.roots {
            visit(&self.graph, *root, 0, &mut active, &mut visited)?;
        }
        if visited.len() != self.graph.units.len() {
            return Err(fault("unreachable persisted unit"));
        }
        for (id, unit) in &self.graph.units {
            if *id != unit.key.fingerprint()
                || unit.key.rustc_fingerprint != self.toolchain.compiler.content
                || !matches!(unit.key.edition, 2015 | 2018 | 2021 | 2024)
                || !is_supported_preparation_target(&unit.key.compile_target)
            {
                return Err(fault("unit key/compiler identity mismatch"));
            }
            absolute(&unit.source_path)?;
            text(&unit.key.package)?;
            text(&unit.key.target_name)?;
            if !matches!(
                self.inputs.source_files.get(&unit.source_path),
                Some(Some(_))
            ) {
                return Err(fault("unit source absent from inventory"));
            }
            if unit
                .dependencies
                .windows(2)
                .any(|w| w[0] >= w[1] || w[0].alias == w[1].alias)
            {
                return Err(fault("unordered or duplicate unit dependencies"));
            }
            let mut edges = StableEncoder::new("lrr/v2/cargo-unit-edges");
            edges.sequence(&unit.dependencies, |e, d| {
                e.string(&d.alias);
                e.digest(d.target);
                e.bool(d.public);
                e.bool(d.no_prelude);
                e.bool(d.no_unused);
            });
            if unit.key.dependency_edges != edges.finish() {
                return Err(fault("unit edge fingerprint mismatch"));
            }
            if matches!(unit.key.role, UnitRole::BuildHost | UnitRole::ProcMacroHost)
                && unit.key.compile_target != self.toolchain.host
            {
                return Err(fault("host tool unit target mismatch"));
            }
        }
        let mut artifact_files = BTreeSet::new();
        for (id, artifact) in &self.artifacts {
            for path in std::iter::once(&artifact.path).chain(artifact.auxiliary.keys()) {
                absolute(path)?;
                if !artifact_files.insert(path) {
                    return Err(fault("compiled artifacts share a filename"));
                }
                if artifact_files.len() > MAX_RECORDS {
                    return Err(fault("compiled artifact filename limit"));
                }
            }
            let unit = self
                .graph
                .units
                .get(id)
                .ok_or_else(|| fault("artifact unit absent"))?;
            absolute(&artifact.path)?;
            let permitted = match artifact.kind {
                ArtifactKind::ProcMacroLibrary => {
                    unit.key.role == UnitRole::ProcMacroHost
                        && unit.key.target_kind == TargetKind::ProcMacro
                }
                ArtifactKind::BuildProgram => {
                    unit.key.role == UnitRole::BuildHost
                        && unit.key.target_kind == TargetKind::BuildScript
                }
                ArtifactKind::HostLibrary => {
                    unit.key.role == UnitRole::BuildHost
                        && unit.key.target_kind == TargetKind::Library
                }
            };
            if !permitted || unit.mode != UnitMode::Build {
                return Err(fault("live/native unit cannot be a compiled tool artifact"));
            }
        }
        for (id, env) in &self.compile_env {
            let unit = self
                .graph
                .units
                .get(id)
                .ok_or_else(|| fault("compile environment unit absent"))?;
            if env.unit != *id
                || env.host != self.toolchain.host
                || env.target != unit.key.compile_target
            {
                return Err(fault("compile environment unit/target mismatch"));
            }
            for map in [&env.cargo, &env.rustc_env] {
                for (k, v) in map {
                    env_name(k)?;
                    text(v)?;
                }
            }
            for (k, v) in &env.declared {
                env_name(k)?;
                if self.inputs.environment.get(k) != Some(v) {
                    return Err(fault("declared environment input unrecorded"));
                }
            }
            if let Some(path) = &env.out_dir {
                absolute(path)?;
            }
            if env.cargo.get("OUT_DIR") != env.out_dir.as_ref()
                || env.rustc_env.contains_key("OUT_DIR")
            {
                return Err(fault(
                    "compile environment OUT_DIR disagrees with captured output",
                ));
            }
        }
        let mut consumer_outputs = BTreeMap::new();
        for (id, out) in &self.build_outputs {
            let producer = self
                .graph
                .units
                .get(id)
                .ok_or_else(|| fault("build output unit absent"))?;
            if out.producer != *id
                || out.tree.producer() != *id
                || producer.mode != UnitMode::RunBuildScript
                || producer.key.role != UnitRole::BuildHost
                || producer.key.target_kind != TargetKind::BuildScript
                || out.consumers.is_empty()
            {
                return Err(fault("build output producer mismatch"));
            }
            out.tree.validate()?;
            for consumer in &out.consumers {
                let unit = self
                    .graph
                    .units
                    .get(consumer)
                    .ok_or_else(|| fault("build consumer absent"))?;
                let env = self
                    .compile_env
                    .get(consumer)
                    .ok_or_else(|| fault("build consumer environment absent"))?;
                if producer.key.package != unit.key.package
                    || !unit.dependencies.iter().any(|d| d.target == *id)
                    || env.out_dir.as_deref() != Some(out.tree.out_dir())
                    || env.rustc_env != out.rustc_env
                    || consumer_outputs.insert(*consumer, *id).is_some()
                {
                    return Err(fault(
                        "build output consumer association is ambiguous or inconsistent",
                    ));
                }
            }
            for cfg in &out.cfgs {
                text(cfg)?;
            }
            for (k, v) in &out.rustc_env {
                env_name(k)?;
                text(v)?;
            }
            for (k, v) in &out.dependency_metadata {
                text(k)?;
                text(v)?;
            }
            for value in out.linked_libs.iter().chain(&out.linked_paths) {
                text(value)?;
            }
        }
        for (id, env) in &self.compile_env {
            if env.out_dir.is_some() && !consumer_outputs.contains_key(id) {
                return Err(fault("uncaptured OUT_DIR in compile environment"));
            }
        }
        Ok(())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Diagnostic> {
        if bytes.len() > MAX_WIRE {
            return Err(fault("preparation manifest exceeds 64 MiB"));
        }
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        CheckedJson::deserialize(&mut decoder).map_err(|e| fault(e.to_string()))?;
        decoder.end().map_err(|e| fault(e.to_string()))?;
        let manifest: Self = serde_json::from_slice(bytes).map_err(|e| fault(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }
    pub fn encode(&self) -> Result<Vec<u8>, Diagnostic> {
        self.validate()?;
        let mut output = BoundedWriter(vec![]);
        serde_json::to_writer(&mut output, self).map_err(|e| fault(e.to_string()))?;
        Ok(output.0)
    }
}
struct BoundedWriter(Vec<u8>);
impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .0
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > MAX_WIRE)
        {
            return Err(std::io::Error::other("preparation manifest exceeds 64 MiB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
// This walk avoids materializing JSON values; only bounded object keys are kept.
type CheckedJson = CheckedJsonShape<true>;
struct CheckedJsonShape<const SETS: bool>;
impl<'de, const SETS: bool> Deserialize<'de> for CheckedJsonShape<SETS> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor<const SETS: bool>;
        impl<'de, const SETS: bool> serde::de::Visitor<'de> for Visitor<SETS> {
            type Value = CheckedJsonShape<SETS>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON with unique object keys")
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<CheckedJsonShape<SETS>, E> {
                Ok(CheckedJsonShape::<SETS>)
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<CheckedJsonShape<SETS>, E> {
                Ok(CheckedJsonShape::<SETS>)
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<CheckedJsonShape<SETS>, E> {
                Ok(CheckedJsonShape::<SETS>)
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<CheckedJsonShape<SETS>, E> {
                if SETS {
                    Err(E::custom("non-integral preparation number"))
                } else {
                    Ok(CheckedJsonShape::<SETS>)
                }
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<CheckedJsonShape<SETS>, E> {
                if v.len() > MAX_STRING {
                    Err(E::custom("oversized preparation string"))
                } else {
                    Ok(CheckedJsonShape::<SETS>)
                }
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<CheckedJsonShape<SETS>, E> {
                Ok(CheckedJsonShape::<SETS>)
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<CheckedJsonShape<SETS>, E> {
                Ok(CheckedJsonShape::<SETS>)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> Result<CheckedJsonShape<SETS>, A::Error> {
                let mut count = 0usize;
                while a.next_element::<CheckedJsonShape<SETS>>()?.is_some() {
                    count += 1;
                    if count > 8 * 1024 * 1024 {
                        return Err(serde::de::Error::custom("preparation array limit"));
                    }
                }
                Ok(CheckedJsonShape::<SETS>)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut a: A,
            ) -> Result<CheckedJsonShape<SETS>, A::Error> {
                let mut keys = BTreeSet::new();
                while let Some(key) = a.next_key::<String>()? {
                    if key.len() > MAX_STRING || !keys.insert(key.clone()) {
                        return Err(serde::de::Error::custom(
                            "duplicate/oversized preparation key",
                        ));
                    }
                    if keys.len() > MAX_RECORDS {
                        return Err(serde::de::Error::custom("preparation map limit"));
                    }
                    match key.as_str() {
                        "features" | "roots" | "consumers" | "directories" | "atoms" if SETS => {
                            a.next_value::<CheckedStringSet>()?;
                        }
                        "directory_membership" | "values" if SETS => {
                            a.next_value::<CheckedMembership>()?;
                        }
                        _ => {
                            a.next_value::<CheckedJsonShape<SETS>>()?;
                        }
                    }
                }
                Ok(CheckedJsonShape::<SETS>)
            }
        }
        d.deserialize_any(Visitor::<SETS>)
    }
}
struct CheckedStringSet;
impl<'de> Deserialize<'de> for CheckedStringSet {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = CheckedStringSet;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded array of unique strings")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> Result<CheckedStringSet, A::Error> {
                let mut seen = BTreeSet::new();
                while let Some(value) = a.next_element::<String>()? {
                    if value.len() > MAX_STRING || !seen.insert(value) || seen.len() > MAX_RECORDS {
                        return Err(serde::de::Error::custom(
                            "duplicate/oversized preparation set",
                        ));
                    }
                }
                Ok(CheckedStringSet)
            }
        }
        d.deserialize_seq(Visitor)
    }
}
struct CheckedMembership;
impl<'de> Deserialize<'de> for CheckedMembership {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = CheckedMembership;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded membership map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut a: A,
            ) -> Result<CheckedMembership, A::Error> {
                let mut keys = BTreeSet::new();
                while let Some(key) = a.next_key::<String>()? {
                    if key.len() > MAX_STRING || !keys.insert(key) || keys.len() > MAX_RECORDS {
                        return Err(serde::de::Error::custom(
                            "duplicate/oversized membership key",
                        ));
                    }
                    a.next_value::<CheckedStringSet>()?;
                }
                Ok(CheckedMembership)
            }
        }
        d.deserialize_map(Visitor)
    }
}
/// Content-addressed immutable publication in an explicitly selected store root.
pub struct ToolStore {
    directory: Dir,
}
impl ToolStore {
    pub fn open(root: &Path) -> Result<Self, Diagnostic> {
        Ok(Self {
            directory: Dir::open_ambient_dir(root, cap_std::ambient_authority())
                .map_err(|e| fault(e.to_string()))?,
        })
    }
    pub fn publish(
        &self,
        manifest: &ToolManifest,
        observed_inputs: &InputInventory,
    ) -> Result<Digest, Diagnostic> {
        observed_inputs.validate()?;
        if observed_inputs.fingerprint() != manifest.inputs.fingerprint() {
            return Err(stale());
        }
        let bytes = manifest.encode()?;
        let id = manifest.fingerprint();
        let final_name = format!("{}.json", id.hex());
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let nonce = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| fault("publication ID exhausted"))?;
        let temp = format!(".stage-{}-{nonce}", std::process::id());
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = self
            .directory
            .open_with(&temp, &options)
            .map_err(|e| fault(e.to_string()))?;
        let result = (|| {
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|e| fault(e.to_string()))?;
            match self
                .directory
                .hard_link(&temp, &self.directory, &final_name)
            {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let old = self.load(id)?;
                    if old.encode()? != bytes {
                        return Err(fault("content-addressed preparation differs"));
                    }
                }
                Err(e) => return Err(fault(e.to_string())),
            }
            Ok(id)
        })();
        let cleanup = self
            .directory
            .remove_file(&temp)
            .map_err(|e| fault(e.to_string()));
        match result {
            Err(e) => Err(e),
            Ok(id) => {
                cleanup?;
                Ok(id)
            }
        }
    }
    pub fn load(&self, id: Digest) -> Result<ToolManifest, Diagnostic> {
        let name = format!("{}.json", id.hex());
        let metadata = self
            .directory
            .metadata(&name)
            .map_err(|e| fault(e.to_string()))?;
        if !metadata.is_file() || metadata.len() > MAX_WIRE as u64 {
            return Err(fault("preparation record is not a bounded regular file"));
        }
        let mut file = self
            .directory
            .open(name)
            .map_err(|e| fault(e.to_string()))?;
        let mut bytes = vec![];
        Read::by_ref(&mut file)
            .take((MAX_WIRE + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| fault(e.to_string()))?;
        let manifest = ToolManifest::decode(&bytes)?;
        if manifest.fingerprint() != id {
            return Err(fault("preparation filename/content identity mismatch"));
        }
        Ok(manifest)
    }
}

#[path = "tool_manifest_build.rs"]
mod build;
pub use build::{AssociatedBuildOutput, BuildScriptMessage, associate_build_outputs};

#[path = "tool_manifest_files.rs"]
mod files;
pub use files::PreparedFiles;

#[path = "tool_manifest_artifacts.rs"]
mod artifacts;
pub use artifacts::{CompilerArtifactMessage, capture_compiler_artifacts};

#[path = "tool_manifest_env.rs"]
mod env;
pub use env::{CompileEnvInputs, PackageEnvironments};
