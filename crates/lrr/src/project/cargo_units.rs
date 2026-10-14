//! Versioned Cargo unit-graph decoding and stable index remapping. No effects.
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncode, StableEncoder},
    profile::is_supported_preparation_target,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const MAX_GRAPH_BYTES: usize = 16 * 1024 * 1024;
const MAX_UNITS: usize = 100_000;
const MAX_GRAPH_DEPTH: usize = 256;
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-UNIT-GRAPH", message.into())
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum UnitRole {
    Live,
    Native,
    BuildHost,
    ProcMacroHost,
    MacroExport,
}
impl UnitRole {
    fn tag(self) -> u32 {
        match self {
            Self::Live => 1,
            Self::Native => 2,
            Self::BuildHost => 3,
            Self::ProcMacroHost => 4,
            Self::MacroExport => 5,
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum TargetKind {
    Library,
    Binary,
    IntegrationTest,
    BuildScript,
    ProcMacro,
}
impl TargetKind {
    fn tag(self) -> u32 {
        match self {
            Self::Library => 1,
            Self::Binary => 2,
            Self::IntegrationTest => 3,
            Self::BuildScript => 4,
            Self::ProcMacro => 5,
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum UnitMode {
    Build,
    Check,
    Test,
    RunBuildScript,
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoUnitKey {
    pub package: String,
    pub target_name: String,
    pub target_kind: TargetKind,
    pub role: UnitRole,
    pub compile_target: String,
    pub test_intent: bool,
    pub edition: u16,
    pub features: BTreeSet<String>,
    pub profile_fingerprint: Digest,
    pub cfg_fingerprint: Digest,
    pub dependency_edges: Digest,
    pub rustc_fingerprint: Digest,
}
impl StableEncode for CargoUnitKey {
    const DOMAIN: &'static str = "lrr/v2/cargo-unit";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(&self.package);
        e.string(&self.target_name);
        e.u32(self.target_kind.tag());
        e.u32(self.role.tag());
        e.string(&self.compile_target);
        e.bool(self.test_intent);
        e.u16(self.edition);
        e.sequence(&self.features.iter().collect::<Vec<_>>(), |e, s| {
            e.string(s)
        });
        e.digest(self.profile_fingerprint);
        e.digest(self.cfg_fingerprint);
        e.digest(self.dependency_edges);
        e.digest(self.rustc_fingerprint);
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyEdge {
    pub alias: String,
    pub target: Digest,
    pub public: bool,
    pub no_prelude: bool,
    pub no_unused: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedUnit {
    pub key: CargoUnitKey,
    pub mode: UnitMode,
    pub source_path: String,
    pub dependencies: Vec<DependencyEdge>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitGraph {
    pub schema: u32,
    pub roots: BTreeSet<Digest>,
    pub units: BTreeMap<Digest, NormalizedUnit>,
}
impl StableEncode for UnitGraph {
    const DOMAIN: &'static str = "lrr/v2/cargo-unit-graph";
    fn encode_fields(&self, encoder: &mut StableEncoder) {
        encoder.u32(self.schema);
        encoder.u64(u64::try_from(self.roots.len()).expect("bounded unit graph root count"));
        for root in &self.roots {
            encoder.digest(*root);
        }
        encoder.map(&self.units, |encoder, identity, unit| {
            encoder.digest(*identity);
            encoder.record(&unit.key);
            encoder.string(&unit.source_path);
            encoder.u32(match unit.mode {
                UnitMode::Build => 1,
                UnitMode::Check => 2,
                UnitMode::Test => 3,
                UnitMode::RunBuildScript => 4,
            });
            encoder.sequence(&unit.dependencies, |encoder, edge| {
                encoder.string(&edge.alias);
                encoder.digest(edge.target);
                encoder.bool(edge.public);
                encoder.bool(edge.no_prelude);
                encoder.bool(edge.no_unused);
            });
        });
    }
}
#[derive(Debug, Clone)]
pub enum Selection {
    Library { test: bool, name: String },
    Binary(String),
    IntegrationTest(String),
}
pub struct UnitContext {
    pub package: String,
    pub selection: Selection,
    pub ordinary_role: UnitRole,
    pub host: String,
    pub target: String,
    pub cfg_by_target: BTreeMap<String, Digest>,
    pub rustc_fingerprint: Digest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGraph {
    version: u32,
    roots: Vec<usize>,
    units: Vec<RawUnit>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUnit {
    pkg_id: String,
    target: RawTarget,
    profile: BTreeMap<String, Value>,
    platform: Option<String>,
    mode: String,
    features: Vec<String>,
    dependencies: Vec<RawEdge>,
}
#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawTarget {
    pub(crate) kind: Vec<String>,
    pub(crate) crate_types: Vec<String>,
    pub(crate) name: String,
    pub(crate) src_path: String,
    pub(crate) edition: String,
    pub(crate) doc: bool,
    pub(crate) doctest: bool,
    pub(crate) test: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEdge {
    index: usize,
    extern_crate_name: String,
    public: bool,
    noprelude: bool,
    nounused: bool,
}

fn encode_json(e: &mut StableEncoder, value: &Value) -> Result<(), Diagnostic> {
    match value {
        Value::Null => e.u32(0),
        Value::Bool(v) => {
            e.u32(1);
            e.bool(*v);
        }
        Value::Number(v) => {
            if let Some(v) = v.as_u64() {
                e.u32(2);
                e.u64(v);
            } else if let Some(v) = v.as_i64() {
                e.u32(3);
                e.bytes(&v.to_le_bytes());
            } else {
                return Err(fault("non-integral profile number"));
            }
        }
        Value::String(v) => {
            e.u32(4);
            e.string(v);
        }
        Value::Array(v) => {
            e.u32(5);
            e.u64(v.len() as u64);
            for v in v {
                encode_json(e, v)?;
            }
        }
        Value::Object(v) => {
            e.u32(6);
            e.u64(v.len() as u64);
            let sorted: BTreeMap<_, _> = v.iter().collect();
            for (k, v) in sorted {
                e.string(k);
                encode_json(e, v)?;
            }
        }
    }
    Ok(())
}
fn profile_fingerprint(profile: &BTreeMap<String, Value>) -> Result<Digest, Diagnostic> {
    const FIELDS: &[&str] = &[
        "name",
        "opt_level",
        "lto",
        "codegen_backend",
        "codegen_units",
        "debuginfo",
        "split_debuginfo",
        "debug_assertions",
        "overflow_checks",
        "rpath",
        "incremental",
        "panic",
        "strip",
        "trim_paths",
    ];
    if profile.len() != FIELDS.len() || FIELDS.iter().any(|name| !profile.contains_key(*name)) {
        return Err(fault("unknown or missing pinned Cargo profile field"));
    }
    for name in [
        "debug_assertions",
        "overflow_checks",
        "rpath",
        "incremental",
    ] {
        if !profile[name].is_boolean() {
            return Err(fault(format!("invalid profile {name}")));
        }
    }
    if !matches!(
        profile["opt_level"].as_str(),
        Some("0" | "1" | "2" | "3" | "s" | "z")
    ) || !matches!(profile["panic"].as_str(), Some("abort" | "unwind"))
    {
        return Err(fault("unknown profile optimization/panic value"));
    }
    if !matches!(profile["name"].as_str(),Some(name) if !name.is_empty()) {
        return Err(fault("missing profile name"));
    }
    if !matches!(
        profile["lto"].as_str(),
        Some("false" | "true" | "off" | "thin" | "fat")
    ) {
        return Err(fault("unknown profile LTO value"));
    }
    if !(profile["codegen_backend"].is_null()
        || matches!(profile["codegen_backend"].as_str(),Some(s) if !s.is_empty()))
    {
        return Err(fault("invalid codegen backend identity"));
    }
    if !(profile["codegen_units"].is_null()
        || matches!(profile["codegen_units"].as_u64(), Some(1..=4_294_967_295)))
    {
        return Err(fault("invalid codegen unit count"));
    }
    if !(matches!(profile["debuginfo"].as_u64(), Some(0..=2))
        || matches!(
            profile["debuginfo"].as_str(),
            Some("none" | "line-directives-only" | "line-tables-only" | "limited" | "full")
        ))
    {
        return Err(fault("unknown debug information value"));
    }
    if !(profile["split_debuginfo"].is_null()
        || matches!(
            profile["split_debuginfo"].as_str(),
            Some("off" | "packed" | "unpacked")
        ))
    {
        return Err(fault("unknown split debug information value"));
    }
    let strip = &profile["strip"];
    if !(matches!(strip.as_str(), Some("none" | "debuginfo" | "symbols"))
        || strip.as_object().is_some_and(|v| {
            v.len() == 1
                && (matches!(
                    v.get("deferred").and_then(Value::as_str),
                    Some("None" | "Debuginfo" | "Symbols")
                ) || v
                    .get("resolved")
                    .and_then(Value::as_object)
                    .is_some_and(|resolved| {
                        resolved.len() == 1
                            && matches!(
                                resolved.get("Named").and_then(Value::as_str),
                                Some("none" | "debuginfo" | "symbols")
                            )
                    }))
        }))
    {
        return Err(fault("unknown strip profile representation"));
    }
    if !matches!(
        profile["trim_paths"].as_str(),
        Some("none" | "all" | "diagnostics" | "macro" | "object")
    ) {
        return Err(fault("unknown trim-paths profile value"));
    }
    let mut e = StableEncoder::new("lrr/v2/cargo-profile");
    e.u64(profile.len() as u64);
    for (k, v) in profile {
        e.string(k);
        encode_json(&mut e, v)?;
    }
    Ok(e.finish())
}
pub(crate) fn kind(target: &RawTarget) -> Result<TargetKind, Diagnostic> {
    let kind = match target.kind.as_slice() {
        [s] => match s.as_str() {
            "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" => TargetKind::Library,
            "bin" => TargetKind::Binary,
            "test" => TargetKind::IntegrationTest,
            "custom-build" => TargetKind::BuildScript,
            "proc-macro" => TargetKind::ProcMacro,
            _ => return Err(fault("unsupported Cargo target kind")),
        },
        _ => return Err(fault("ambiguous Cargo target kind")),
    };
    if target.crate_types.is_empty()
        || target.crate_types.iter().any(|s| {
            !matches!(
                s.as_str(),
                "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "bin" | "proc-macro"
            )
        })
    {
        return Err(fault("unknown Cargo crate type"));
    }
    if target.name.is_empty() || target.src_path.is_empty() {
        return Err(fault("missing target identity/source"));
    }
    let _ = (target.doc, target.doctest); // Target capabilities are not unit test intent.
    Ok(kind)
}
impl UnitGraph {
    pub fn parse(bytes: &[u8], context: &UnitContext) -> Result<Self, Diagnostic> {
        if bytes.len() > MAX_GRAPH_BYTES {
            return Err(fault("unit graph exceeds 16 MiB"));
        }
        if !is_supported_preparation_target(&context.target)
            || matches!(
                context.ordinary_role,
                UnitRole::BuildHost | UnitRole::ProcMacroHost
            )
        {
            return Err(fault("invalid graph execution target/ordinary world"));
        }
        let raw: RawGraph = serde_json::from_slice(bytes)
            .map_err(|e| fault(format!("invalid pinned unit graph schema: {e}")))?;
        if raw.version != 1
            || raw.units.is_empty()
            || raw.units.len() > MAX_UNITS
            || raw.roots.len() != 1
        {
            return Err(fault(
                "unsupported graph version, size, or target selection",
            ));
        }
        let root = raw.roots[0];
        let selected = raw
            .units
            .get(root)
            .ok_or_else(|| fault("root index out of bounds"))?;
        let selected_kind = kind(&selected.target)?;
        let (wanted_kind, wanted_name, test_intent) = match &context.selection {
            Selection::Library { test, name } => (TargetKind::Library, name, *test),
            Selection::Binary(name) => (TargetKind::Binary, name, false),
            Selection::IntegrationTest(name) => (TargetKind::IntegrationTest, name, true),
        };
        if selected.pkg_id != context.package
            || selected_kind != wanted_kind
            || selected.target.name != *wanted_name
            || selected.platform.as_deref() != Some(context.target.as_str())
        {
            return Err(fault("root does not match exact requested unit"));
        }
        if context.ordinary_role != UnitRole::Native && selected.mode != "check" {
            return Err(fault("compile-time planner root must be a check unit"));
        }
        if !matches!(
            selected.target.edition.as_str(),
            "2015" | "2018" | "2021" | "2024"
        ) {
            return Err(Diagnostic::new(
                "LRR-E-LIVE-EDITION",
                "selected root must use Rust 2015, 2018, 2021 or 2024".into(),
            ));
        }
        if test_intent
            && (!selected.target.test
                || selected.profile.get("name").and_then(Value::as_str) != Some("test"))
        {
            return Err(fault("requested test intent absent from graph"));
        }
        let mut remap = vec![None; raw.units.len()];
        let mut active = vec![false; raw.units.len()];
        let mut normalized = BTreeMap::new();
        struct Visit<'a> {
            raw: &'a RawGraph,
            context: &'a UnitContext,
            root: usize,
            test: bool,
            remap: &'a mut [Option<Digest>],
            active: &'a mut [bool],
            out: &'a mut BTreeMap<Digest, NormalizedUnit>,
        }
        impl Visit<'_> {
            fn unit(&mut self, index: usize, depth: usize) -> Result<Digest, Diagnostic> {
                if index >= self.raw.units.len() {
                    return Err(fault("dependency index out of bounds"));
                }
                if let Some(key) = self.remap[index] {
                    return Ok(key);
                }
                if depth >= MAX_GRAPH_DEPTH || self.active[index] {
                    return Err(fault("unit graph dependency cycle/depth limit"));
                }
                self.active[index] = true;
                let u = &self.raw.units[index];
                if u.pkg_id.is_empty()
                    || u.target.src_path.contains('\0')
                    || !std::path::Path::new(&u.target.src_path).is_absolute()
                {
                    return Err(fault("invalid package/source identity"));
                }
                let target_kind = kind(&u.target)?;
                let mode = match u.mode.as_str() {
                    "build" => UnitMode::Build,
                    "check" => UnitMode::Check,
                    "test" => UnitMode::Test,
                    "run-custom-build" => UnitMode::RunBuildScript,
                    _ => return Err(fault("unknown Cargo unit mode")),
                };
                if mode == UnitMode::RunBuildScript && target_kind != TargetKind::BuildScript {
                    return Err(fault("non-script run-custom-build unit"));
                }
                let role = if target_kind == TargetKind::ProcMacro {
                    UnitRole::ProcMacroHost
                } else if target_kind == TargetKind::BuildScript || u.platform.is_none() {
                    UnitRole::BuildHost
                } else {
                    self.context.ordinary_role
                };
                let compile_target = match role {
                    UnitRole::BuildHost | UnitRole::ProcMacroHost => self.context.host.clone(),
                    _ => u
                        .platform
                        .clone()
                        .ok_or_else(|| fault("runtime unit missing target"))?,
                };
                if compile_target != self.context.host && compile_target != self.context.target {
                    return Err(fault("unit has unrecorded target"));
                }
                let cfg_target = u.platform.as_ref().unwrap_or(&self.context.host);
                let base_cfg = self
                    .context
                    .cfg_by_target
                    .get(cfg_target)
                    .ok_or_else(|| fault("unit cfg was not captured"))?;
                let test = (index == self.root && self.test) || mode == UnitMode::Test;
                let features: BTreeSet<_> = u.features.iter().cloned().collect();
                if features.len() != u.features.len() {
                    return Err(fault("duplicate unit feature"));
                }
                let mut cfg = StableEncoder::new("lrr/v2/cargo-unit-cfg");
                cfg.digest(*base_cfg);
                cfg.bool(test);
                cfg.bool(u.profile.get("debug_assertions") == Some(&Value::Bool(true)));
                cfg.sequence(&features.iter().collect::<Vec<_>>(), |e, s| e.string(s));
                let mut edges = vec![];
                for edge in &u.dependencies {
                    if edge.extern_crate_name.is_empty() {
                        return Err(fault("missing dependency alias"));
                    }
                    let target = self.unit(edge.index, depth + 1)?;
                    edges.push(DependencyEdge {
                        alias: edge.extern_crate_name.clone(),
                        target,
                        public: edge.public,
                        no_prelude: edge.noprelude,
                        no_unused: edge.nounused,
                    });
                }
                edges.sort();
                if edges.windows(2).any(|w| w[0].alias == w[1].alias) {
                    return Err(fault("duplicate dependency alias in a unit"));
                }
                let mut encoded = StableEncoder::new("lrr/v2/cargo-unit-edges");
                encoded.sequence(&edges, |e, d| {
                    e.string(&d.alias);
                    e.digest(d.target);
                    e.bool(d.public);
                    e.bool(d.no_prelude);
                    e.bool(d.no_unused);
                });
                let edition = u
                    .target
                    .edition
                    .parse()
                    .map_err(|_| fault("invalid Cargo edition"))?;
                if !matches!(edition, 2015 | 2018 | 2021 | 2024) {
                    return Err(fault("unsupported unit edition"));
                }
                let key = CargoUnitKey {
                    package: u.pkg_id.clone(),
                    target_name: u.target.name.clone(),
                    target_kind,
                    role,
                    compile_target,
                    test_intent: test,
                    edition,
                    features,
                    profile_fingerprint: profile_fingerprint(&u.profile)?,
                    cfg_fingerprint: cfg.finish(),
                    dependency_edges: encoded.finish(),
                    rustc_fingerprint: self.context.rustc_fingerprint,
                };
                let identity = key.fingerprint();
                if let Some(old) = self.out.get(&identity)
                    && (old.mode != mode || old.source_path != u.target.src_path)
                {
                    return Err(fault("distinct Cargo units collide after remapping"));
                }
                self.out.insert(
                    identity,
                    NormalizedUnit {
                        key,
                        mode,
                        source_path: u.target.src_path.clone(),
                        dependencies: edges,
                    },
                );
                self.remap[index] = Some(identity);
                self.active[index] = false;
                Ok(identity)
            }
        }
        let mut visit = Visit {
            raw: &raw,
            context,
            root,
            test: test_intent,
            remap: &mut remap,
            active: &mut active,
            out: &mut normalized,
        };
        let identity = visit.unit(root, 0)?;
        if visit.remap.iter().any(Option::is_none) {
            return Err(fault(
                "extra units outside the requested dependency closure",
            ));
        }
        Ok(Self {
            schema: 1,
            roots: BTreeSet::from([identity]),
            units: normalized,
        })
    }
}

/// The profile subset emitted in Cargo compiler-artifact records.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactProfile {
    pub opt_level: String,
    pub debuginfo: Value,
    pub debug_assertions: bool,
    pub overflow_checks: bool,
    pub test: bool,
}
/// Private profile facts derived from the same validated exact graph selection.
pub struct ArtifactProfiles {
    units: BTreeMap<Digest, (RawTarget, ArtifactProfile)>,
}
impl ArtifactProfiles {
    pub fn parse(bytes: &[u8], context: &UnitContext) -> Result<Self, Diagnostic> {
        let graph = UnitGraph::parse(bytes, context)?;
        let raw: RawGraph = serde_json::from_slice(bytes).map_err(|e| fault(e.to_string()))?;
        let mut profiles = BTreeMap::new();
        for unit in raw.units {
            let fingerprint = profile_fingerprint(&unit.profile)?;
            let target = unit.target.clone();
            let key = (
                unit.pkg_id,
                unit.target.name,
                unit.target.src_path,
                fingerprint,
                unit.features.into_iter().collect::<BTreeSet<_>>(),
            );
            let facts = ArtifactProfile {
                opt_level: unit.profile["opt_level"]
                    .as_str()
                    .ok_or_else(|| fault("profile opt-level absent"))?
                    .into(),
                debuginfo: unit.profile["debuginfo"].clone(),
                debug_assertions: unit.profile["debug_assertions"]
                    .as_bool()
                    .ok_or_else(|| fault("profile debug assertions absent"))?,
                overflow_checks: unit.profile["overflow_checks"]
                    .as_bool()
                    .ok_or_else(|| fault("profile overflow checks absent"))?,
                test: false,
            };
            if profiles
                .insert(key, (target.clone(), facts.clone()))
                .is_some_and(|old| old != (target, facts))
            {
                return Err(fault("inconsistent artifact profile evidence"));
            }
        }
        let mut units = BTreeMap::new();
        for (id, unit) in graph.units {
            let key = (
                unit.key.package.clone(),
                unit.key.target_name.clone(),
                unit.source_path.clone(),
                unit.key.profile_fingerprint,
                unit.key.features.clone(),
            );
            let (target, mut facts) = profiles
                .get(&key)
                .ok_or_else(|| fault("normalized artifact profile absent"))?
                .clone();
            facts.test = unit.key.test_intent;
            units.insert(id, (target, facts));
        }
        Ok(Self { units })
    }
    pub(crate) fn debug_assertions(
        &self,
        id: Digest,
        unit: &NormalizedUnit,
    ) -> Result<bool, Diagnostic> {
        if unit.key.fingerprint() != id {
            return Err(fault("compile cfg unit identity mismatch"));
        }
        let (target, facts) = self
            .units
            .get(&id)
            .ok_or_else(|| fault("compile cfg profile absent"))?;
        if target.src_path != unit.source_path {
            return Err(fault("compile cfg source mismatch"));
        }
        Ok(facts.debug_assertions)
    }
    pub(crate) fn matches(
        &self,
        id: Digest,
        unit: &NormalizedUnit,
        profile: &ArtifactProfile,
        target: &RawTarget,
    ) -> bool {
        unit.key.fingerprint() == id
            && self.units.get(&id).is_some_and(|(expected, facts)| {
                expected.src_path == unit.source_path && expected == target && facts == profile
            })
    }
}
