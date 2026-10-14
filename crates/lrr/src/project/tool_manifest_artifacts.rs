//! Strict Cargo artifact association and bounded file capture; never executes or loads tools.
use super::{
    ArtifactKind, CheckedJson, InputKind, InputPlan, InputRoot, ToolArtifact, absolute, fault, text,
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    project::cargo_units::{
        ArtifactProfile, ArtifactProfiles, RawTarget, TargetKind, UnitGraph, UnitMode, UnitRole,
        kind,
    },
    source::snapshot::SourcePath,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilerArtifactMessage {
    reason: String,
    package_id: String,
    manifest_path: String,
    target: RawTarget,
    profile: ArtifactProfile,
    features: Vec<String>,
    filenames: Vec<String>,
    executable: Option<String>,
    fresh: bool,
}
impl CompilerArtifactMessage {
    pub fn decode(bytes: &[u8]) -> Result<Self, Diagnostic> {
        if bytes.len() > 1024 * 1024 {
            return Err(fault("Cargo artifact record exceeds 1 MiB"));
        }
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        CheckedJson::deserialize(&mut decoder).map_err(|e| fault(e.to_string()))?;
        decoder.end().map_err(|e| fault(e.to_string()))?;
        let record: Self = serde_json::from_slice(bytes).map_err(|e| fault(e.to_string()))?;
        if record.reason != "compiler-artifact"
            || record.package_id.is_empty()
            || record.filenames.is_empty()
        {
            return Err(fault("incomplete Cargo compiler-artifact record"));
        }
        text(&record.package_id)?;
        absolute(&record.manifest_path)?;
        absolute(&record.target.src_path)?;
        kind(&record.target)?;
        if record
            .target
            .crate_types
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != record.target.crate_types.len()
        {
            return Err(fault("duplicate artifact crate types"));
        }
        text(&record.target.name)?;
        if record.features.iter().collect::<BTreeSet<_>>().len() != record.features.len()
            || record.filenames.iter().collect::<BTreeSet<_>>().len() != record.filenames.len()
        {
            return Err(fault("duplicate artifact features or filenames"));
        }
        for feature in &record.features {
            text(feature)?;
        }
        for path in &record.filenames {
            absolute(path)?;
        }
        if let Some(executable) = &record.executable {
            absolute(executable)?;
            if !record.filenames.contains(executable) {
                return Err(fault("Cargo executable was not a reported artifact"));
            }
        }
        Ok(record)
    }
    pub fn fresh(&self) -> bool {
        self.fresh
    }
}
/// Validate every selected record before opening any artifact file capability.
pub fn capture_compiler_artifacts(
    graph: &UnitGraph,
    profiles: &ArtifactProfiles,
    messages: Vec<CompilerArtifactMessage>,
) -> Result<BTreeMap<Digest, ToolArtifact>, Diagnostic> {
    if graph.schema != 1 || graph.units.len() > 100_000 || messages.len() > 100_000 {
        return Err(fault("artifact graph bounds"));
    }
    let mut index: BTreeMap<_, Vec<Digest>> = BTreeMap::new();
    for (id, unit) in &graph.units {
        index
            .entry((
                unit.key.package.as_str(),
                unit.key.target_name.as_str(),
                unit.key.target_kind,
                unit.source_path.as_str(),
                unit.key.features.clone(),
            ))
            .or_default()
            .push(*id);
    }
    let mut selected = BTreeMap::new();
    let mut all_paths = BTreeSet::new();
    for message in messages {
        let target_kind = kind(&message.target)?;
        let key = (
            message.package_id.as_str(),
            message.target.name.as_str(),
            target_kind,
            message.target.src_path.as_str(),
            message.features.iter().cloned().collect(),
        );
        let candidates = index
            .get(&key)
            .into_iter()
            .flatten()
            .filter(|id| {
                graph.units.get(id).is_some_and(|unit| {
                    profiles.matches(**id, unit, &message.profile, &message.target)
                })
            })
            .copied()
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(fault(
                "Cargo artifact has no unique exact unit/profile; prepare one selected context at a time",
            ));
        }
        let id = candidates[0];
        let unit = graph
            .units
            .get(&id)
            .ok_or_else(|| fault("indexed artifact unit absent"))?;
        let artifact_kind = match (unit.key.role, unit.key.target_kind, unit.mode) {
            (UnitRole::BuildHost, TargetKind::BuildScript, UnitMode::Build) => {
                ArtifactKind::BuildProgram
            }
            (UnitRole::BuildHost, TargetKind::Library, UnitMode::Build) => {
                ArtifactKind::HostLibrary
            }
            (UnitRole::ProcMacroHost, TargetKind::ProcMacro, UnitMode::Build) => {
                ArtifactKind::ProcMacroLibrary
            }
            _ => return Err(fault("live/native runtime unit reported as compiled tool")),
        };
        let primary = match artifact_kind {
            ArtifactKind::BuildProgram => {
                if message.filenames.len() != 1 {
                    return Err(fault("ambiguous compiled build program"));
                }
                message.filenames[0].clone()
            }
            ArtifactKind::ProcMacroLibrary => {
                let extension = if unit.key.compile_target.ends_with("apple-darwin") {
                    "dylib"
                } else if unit.key.compile_target.ends_with("unknown-linux-gnu") {
                    "so"
                } else {
                    return Err(fault("unsupported proc-macro host"));
                };
                let libraries: Vec<_> = message
                    .filenames
                    .iter()
                    .filter(|p| Path::new(p).extension().is_some_and(|e| e == extension))
                    .collect();
                if libraries.len() != 1 {
                    return Err(fault("ambiguous macro library filenames"));
                }
                libraries[0].clone()
            }
            ArtifactKind::HostLibrary => {
                let libraries: Vec<_> = message
                    .filenames
                    .iter()
                    .filter(|p| Path::new(p).extension().is_some_and(|e| e == "rlib"))
                    .collect();
                if libraries.len() > 1 {
                    return Err(fault("ambiguous Rust host libraries"));
                }
                if let Some(library) = libraries.first() {
                    (*library).clone()
                } else {
                    let extension = if unit.key.compile_target.ends_with("apple-darwin") {
                        "dylib"
                    } else if unit.key.compile_target.ends_with("unknown-linux-gnu") {
                        "so"
                    } else {
                        return Err(fault("unsupported host library target"));
                    };
                    let dynamic: Vec<_> = message
                        .filenames
                        .iter()
                        .filter(|p| Path::new(p).extension().is_some_and(|e| e == extension))
                        .collect();
                    if dynamic.len() != 1 {
                        return Err(fault("host tool requires one reported Rust library"));
                    }
                    dynamic[0].clone()
                }
            }
        };
        for path in &message.filenames {
            if !all_paths.insert(path.clone()) {
                return Err(fault("different artifact records share a file"));
            }
            if all_paths.len() > 100_000 {
                return Err(fault("artifact filename limit"));
            }
        }
        if selected
            .insert(id, (primary, artifact_kind, message.filenames))
            .is_some()
        {
            return Err(fault("duplicate artifact report for exact unit"));
        }
    }
    let mut directories: BTreeMap<PathBuf, BTreeSet<SourcePath>> = BTreeMap::new();
    for path in &all_paths {
        let path = Path::new(path);
        let parent = path
            .parent()
            .ok_or_else(|| fault("artifact parent absent"))?;
        directories
            .entry(parent.to_owned())
            .or_default()
            .insert(SourcePath::new(Path::new(
                path.file_name()
                    .ok_or_else(|| fault("artifact filename absent"))?,
            ))?);
    }
    let roots = directories
        .into_iter()
        .map(|(path, files)| {
            InputRoot::open(
                InputKind::External,
                &path,
                files,
                BTreeSet::new(),
                false,
                BTreeSet::new(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let capture = InputPlan::new(roots, BTreeSet::new())?.capture()?;
    let mut artifacts = BTreeMap::new();
    for (id, (path, kind, filenames)) in selected {
        let mut hashes = BTreeMap::new();
        for name in filenames {
            let hash = capture
                .inventory()
                .external_files
                .get(&name)
                .copied()
                .flatten()
                .ok_or_else(|| fault("Cargo artifact file absent"))?;
            hashes.insert(name, hash);
        }
        let content = hashes
            .remove(&path)
            .ok_or_else(|| fault("primary artifact absent"))?;
        artifacts.insert(
            id,
            ToolArtifact {
                path,
                content,
                kind,
                auxiliary: hashes,
            },
        );
    }
    Ok(artifacts)
}
