//! Bounded preparation-file capture. Reading never executes or loads an artifact.
use super::{
    InputCapture, InputKind, InputPlan, InputRoot, ToolManifest, ToolStore, absolute, fault, stale,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    source::snapshot::SourcePath,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
/// A current filesystem witness bound to one validated manifest, including tool binaries.
#[derive(Debug, Clone)]
pub struct PreparedFiles {
    manifest: Digest,
    capture: InputCapture,
}
impl PreparedFiles {
    /// Explicit preparation/adoption boundary: only the validated record's declared files.
    pub fn capture(manifest: &ToolManifest) -> Result<Self, Diagnostic> {
        manifest.validate()?;
        let mut expected = BTreeMap::new();
        for (path, digest) in [
            (
                &manifest.toolchain.planner.path,
                manifest.toolchain.planner.content,
            ),
            (
                &manifest.toolchain.compiler.path,
                manifest.toolchain.compiler.content,
            ),
            (
                &manifest.toolchain.macro_server.path,
                manifest.toolchain.macro_server.content,
            ),
            (
                &manifest.toolchain.component_manifest,
                manifest.toolchain.component_manifest_content,
            ),
        ]
        .into_iter()
        .chain(
            manifest
                .toolchain
                .prepared_macro_server
                .iter()
                .flat_map(|prepared| prepared.files()),
        )
        .chain(manifest.artifacts.values().flat_map(|artifact| {
            std::iter::once((&artifact.path, artifact.content)).chain(
                artifact
                    .auxiliary
                    .iter()
                    .map(|(path, content)| (path, *content)),
            )
        })) {
            absolute(path)?;
            if expected
                .insert(path.clone(), digest)
                .is_some_and(|old| old != digest)
            {
                return Err(fault("conflicting expected contents for prepared file"));
            }
        }
        let mut directories: BTreeMap<PathBuf, BTreeSet<SourcePath>> = BTreeMap::new();
        for path in expected.keys() {
            let path = Path::new(path);
            let parent = path
                .parent()
                .ok_or_else(|| fault("prepared file parent absent"))?;
            let name = path
                .file_name()
                .ok_or_else(|| fault("prepared file name absent"))?;
            directories
                .entry(parent.to_owned())
                .or_default()
                .insert(SourcePath::new(Path::new(name))?);
        }
        let roots = directories
            .into_iter()
            .map(|(directory, files)| {
                InputRoot::open(
                    InputKind::External,
                    &directory,
                    files,
                    BTreeSet::new(),
                    false,
                    BTreeSet::new(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let capture = InputPlan::new(roots, BTreeSet::new())?.capture()?;
        for (path, digest) in expected {
            if capture.inventory().external_files.get(&path) != Some(&Some(digest)) {
                return Err(stale());
            }
        }
        Ok(Self {
            manifest: manifest.fingerprint(),
            capture,
        })
    }
    pub(super) fn verify_manifest(&self, manifest: &ToolManifest) -> Result<(), Diagnostic> {
        if self.manifest != manifest.fingerprint() {
            Err(stale())
        } else {
            Ok(())
        }
    }
    pub fn verify_unchanged(&self, after: &Self) -> Result<(), Diagnostic> {
        if self.manifest != after.manifest {
            return Err(stale());
        }
        self.capture.verify_unchanged(&after.capture)
    }
}
impl ToolStore {
    /// Publish only when source/input and compiled-file witnesses both remain unchanged.
    pub fn publish_verified(
        &self,
        manifest: &ToolManifest,
        before: &InputCapture,
        after: &InputCapture,
        files_before: &PreparedFiles,
        files_after: &PreparedFiles,
    ) -> Result<Digest, Diagnostic> {
        files_before.verify_manifest(manifest)?;
        files_after.verify_manifest(manifest)?;
        files_before.verify_unchanged(files_after)?;
        self.publish_captured(manifest, before, after)
    }
}
