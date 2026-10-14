//! Cargo build-script records associated with exact graph producers before capture.
use super::{BuildOutput, CheckedJson, absolute, env_name, fault, text};
use crate::{
    core::digest::{Digest, StableEncode},
    project::cargo_units::{TargetKind, UnitGraph, UnitMode, UnitRole},
    source::generated::GeneratedRoot,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
const MAX_MESSAGE: usize = 1024 * 1024;
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildScriptMessage {
    reason: String,
    package_id: String,
    linked_libs: Vec<String>,
    linked_paths: Vec<String>,
    cfgs: Vec<String>,
    env: Vec<(String, String)>,
    out_dir: String,
}
impl BuildScriptMessage {
    /// Decode one selected build-script record. Ordinary log handling belongs to the driver.
    pub fn decode(bytes: &[u8]) -> Result<Self, crate::core::diagnostic::Diagnostic> {
        if bytes.len() > MAX_MESSAGE {
            return Err(fault("Cargo build output record exceeds 1 MiB"));
        }
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        CheckedJson::deserialize(&mut decoder).map_err(|e| fault(e.to_string()))?;
        decoder.end().map_err(|e| fault(e.to_string()))?;
        let message: Self = serde_json::from_slice(bytes).map_err(|e| fault(e.to_string()))?;
        if message.reason != "build-script-executed" {
            return Err(fault("expected Cargo build-script-executed record"));
        }
        text(&message.package_id)?;
        if message.package_id.is_empty() {
            return Err(fault("build output package absent"));
        }
        absolute(&message.out_dir)?;
        for value in message
            .linked_libs
            .iter()
            .chain(&message.linked_paths)
            .chain(&message.cfgs)
        {
            text(value)?;
        }
        if message.cfgs.iter().collect::<BTreeSet<_>>().len() != message.cfgs.len() {
            return Err(fault("duplicate build output cfg"));
        }
        let mut env = BTreeSet::new();
        for (name, value) in &message.env {
            env_name(name)?;
            text(value)?;
            if name == "OUT_DIR" || !env.insert(name) {
                return Err(fault("duplicate or reserved build output environment"));
            }
        }
        Ok(message)
    }
}
/// Association is pure. Ambiguous package contexts require a separate selected planner run.
#[derive(Debug, Clone)]
pub struct AssociatedBuildOutput {
    producer: Digest,
    consumers: BTreeSet<Digest>,
    message: BuildScriptMessage,
}
impl AssociatedBuildOutput {
    pub fn producer(&self) -> Digest {
        self.producer
    }
    pub fn consumers(&self) -> &BTreeSet<Digest> {
        &self.consumers
    }
    pub fn out_dir(&self) -> &str {
        &self.message.out_dir
    }
    /// Filesystem access only through an explicit OUT_DIR capability in preparation.
    pub fn capture(
        &self,
        graph: &UnitGraph,
    ) -> Result<BuildOutput, crate::core::diagnostic::Diagnostic> {
        let reassociated = associate_build_outputs(graph, vec![self.message.clone()])?;
        let checked = reassociated
            .get(&self.producer)
            .ok_or_else(|| fault("build graph producer changed before output capture"))?;
        if checked.consumers != self.consumers {
            return Err(fault("build graph changed before output capture"));
        }
        let unit = graph
            .units
            .get(&self.producer)
            .ok_or_else(|| fault("output producer disappeared"))?;
        let tree = GeneratedRoot::open(&unit.key, Path::new(&self.message.out_dir))?.capture()?;
        Ok(BuildOutput {
            producer: self.producer,
            consumers: self.consumers.clone(),
            tree,
            cfgs: self.message.cfgs.iter().cloned().collect(),
            rustc_env: self.message.env.iter().cloned().collect(),
            linked_libs: self.message.linked_libs.clone(),
            linked_paths: self.message.linked_paths.clone(),
            dependency_metadata: BTreeMap::new(),
        })
    }
}
/// A package-only Cargo record must identify exactly one runnable producer in this selected graph.
pub fn associate_build_outputs(
    graph: &UnitGraph,
    messages: Vec<BuildScriptMessage>,
) -> Result<BTreeMap<Digest, AssociatedBuildOutput>, crate::core::diagnostic::Diagnostic> {
    if graph.schema != 1 || graph.units.len() > 100_000 || messages.len() > 100_000 {
        return Err(fault("invalid build output graph bounds"));
    }
    let mut producers: BTreeMap<&str, Vec<Digest>> = BTreeMap::new();
    let mut consumers_by_producer: BTreeMap<Digest, BTreeSet<Digest>> = BTreeMap::new();
    let mut edge_count = 0usize;
    for (id, unit) in &graph.units {
        if unit.mode == UnitMode::RunBuildScript
            && unit.key.target_kind == TargetKind::BuildScript
            && unit.key.role == UnitRole::BuildHost
        {
            producers.entry(&unit.key.package).or_default().push(*id);
        }
        edge_count = edge_count
            .checked_add(unit.dependencies.len())
            .ok_or_else(|| fault("build output edge count overflow"))?;
        if edge_count > 100_000 {
            return Err(fault("build output graph edge limit"));
        }
        for edge in &unit.dependencies {
            consumers_by_producer
                .entry(edge.target)
                .or_default()
                .insert(*id);
        }
    }
    let mut associated = BTreeMap::new();
    let mut directories = BTreeSet::new();
    for message in messages {
        let candidates = producers
            .get(message.package_id.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if candidates.len() != 1 {
            return Err(fault(
                "Cargo build output has no unique graph unit; prepare one selected unit context at a time",
            ));
        }
        let producer = &candidates[0];
        let unit = graph
            .units
            .get(producer)
            .ok_or_else(|| fault("indexed build unit absent"))?;
        if unit.key.fingerprint() != *producer {
            return Err(fault("build output graph identity mismatch"));
        }
        let consumers = consumers_by_producer
            .get(producer)
            .cloned()
            .unwrap_or_default();
        for id in &consumers {
            let consumer = graph
                .units
                .get(id)
                .ok_or_else(|| fault("indexed build consumer absent"))?;
            if consumer.key.package != unit.key.package
                || consumer.key.target_kind == TargetKind::BuildScript
                || consumer.key.fingerprint() != *id
            {
                return Err(fault("build output consumer edge is inconsistent"));
            }
        }
        if consumers.is_empty() {
            return Err(fault("build output has no graph consumers"));
        }
        if !directories.insert(message.out_dir.clone()) {
            return Err(fault("different build outputs share OUT_DIR"));
        }
        if associated
            .insert(
                *producer,
                AssociatedBuildOutput {
                    producer: *producer,
                    consumers,
                    message,
                },
            )
            .is_some()
        {
            return Err(fault("duplicate Cargo output for graph unit"));
        }
    }
    Ok(associated)
}
