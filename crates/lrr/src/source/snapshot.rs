//! Immutable source bundles and maps; filesystem authority stays in loader.
use super::manifest::CrateKey;
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncode, StableEncoder},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
    sync::Arc,
};

pub const MAX_SOURCE_FILE: usize = 8 * 1024 * 1024;
pub const MAX_SNAPSHOT_SOURCE: usize = 128 * 1024 * 1024;
fn fault(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        code,
        message: message.into(),
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourcePath(String);
impl SourcePath {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Diagnostic> {
        let mut parts = Vec::new();
        for part in path.as_ref().components() {
            match part {
                Component::Normal(name) => parts.push(
                    name.to_str()
                        .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "source path must be UTF-8"))?
                        .to_owned(),
                ),
                Component::CurDir => {}
                Component::ParentDir => {
                    if parts.pop().is_none() {
                        return Err(fault(
                            "LRR-E-SOURCE-PATH",
                            "source path escapes approved root",
                        ));
                    }
                }
                _ => {
                    return Err(fault(
                        "LRR-E-SOURCE-PATH",
                        "source path must be root relative",
                    ));
                }
            }
        }
        if parts.is_empty() {
            return Err(fault("LRR-E-SOURCE-PATH", "empty source path"));
        }
        Ok(Self(parts.join("/")))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for SourcePath {
    type Error = Diagnostic;
    fn try_from(s: String) -> Result<Self, Diagnostic> {
        Self::new(s)
    }
}
impl From<SourcePath> for String {
    fn from(path: SourcePath) -> String {
        path.0
    }
}

#[derive(Debug, Clone)]
pub struct SourceBlob {
    content: Digest,
    bytes: Arc<[u8]>,
}
impl SourceBlob {
    pub fn new(bytes: Vec<u8>) -> Result<Self, Diagnostic> {
        if bytes.len() > MAX_SOURCE_FILE {
            return Err(fault("LRR-E-SOURCE-LIMIT", "source file exceeds 8 MiB"));
        }
        std::str::from_utf8(&bytes)
            .map_err(|e| fault("LRR-E-SOURCE-UTF8", format!("invalid source encoding: {e}")))?;
        Ok(Self {
            content: Digest::of_bytes(&bytes),
            bytes: bytes.into(),
        })
    }
    /// Captured include_bytes data is not source text and is never parsed.
    pub(crate) fn included_bytes(bytes: Vec<u8>) -> Result<Self, Diagnostic> {
        if bytes.len() > MAX_SOURCE_FILE {
            return Err(fault("LRR-E-SOURCE-LIMIT", "included file exceeds 8 MiB"));
        }
        Ok(Self {
            content: Digest::of_bytes(&bytes),
            bytes: bytes.into(),
        })
    }
    pub fn content(&self) -> Digest {
        self.content
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn text(&self) -> Result<&str, Diagnostic> {
        std::str::from_utf8(&self.bytes)
            .map_err(|e| fault("LRR-E-INTERNAL-SOURCE-MAP", e.to_string()))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub identity: String,
    pub root_source: SourcePath,
    pub selected_crate: Digest,
    pub crates: BTreeMap<Digest, CrateKey>,
}
impl StableEncode for ProjectManifest {
    const DOMAIN: &'static str = "lrr/v1/project-manifest";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(&self.identity);
        e.string(self.root_source.as_str());
        e.digest(self.selected_crate);
        e.map(&self.crates, |e, key, value| {
            e.digest(*key);
            e.record(value);
        });
    }
}
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CfgSet {
    pub atoms: BTreeSet<String>,
    pub values: BTreeMap<String, BTreeSet<String>>,
}
impl StableEncode for CfgSet {
    const DOMAIN: &'static str = "lrr/v1/crate-cfg";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u64(self.atoms.len() as u64);
        for atom in &self.atoms {
            e.string(atom);
        }
        e.map(&self.values, |e, key, values| {
            e.string(key);
            e.u64(values.len() as u64);
            for value in values {
                e.string(value);
            }
        });
    }
}
impl CfgSet {
    pub fn guest_default() -> Self {
        Self {
            atoms: BTreeSet::from(["unix".into(), "debug_assertions".into()]),
            values: BTreeMap::from([
                ("target_os".into(), BTreeSet::from(["linux".into()])),
                ("target_arch".into(), BTreeSet::from(["x86_64".into()])),
                ("target_endian".into(), BTreeSet::from(["little".into()])),
                ("target_pointer_width".into(), BTreeSet::from(["64".into()])),
            ]),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    line_starts: BTreeMap<SourcePath, Arc<[u32]>>,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct LineColumn {
    pub line: u32,
    pub column: u32,
}
impl SourceMap {
    fn new(files: &BTreeMap<SourcePath, SourceBlob>) -> Self {
        let line_starts = files
            .iter()
            .map(|(path, blob)| {
                let mut starts = vec![0];
                for (offset, byte) in blob.bytes().iter().enumerate() {
                    if *byte == b'\n' {
                        starts.push((offset + 1) as u32);
                    }
                }
                (path.clone(), Arc::from(starts))
            })
            .collect();
        Self { line_starts }
    }
    pub fn locate(
        &self,
        files: &BTreeMap<SourcePath, SourceBlob>,
        path: &SourcePath,
        offset: u32,
    ) -> Result<LineColumn, Diagnostic> {
        let starts = self
            .line_starts
            .get(path)
            .ok_or_else(|| fault("LRR-E-INTERNAL-SOURCE-MAP", "source-map path missing"))?;
        let text = files
            .get(path)
            .ok_or_else(|| fault("LRR-E-INTERNAL-SOURCE-MAP", "source bytes missing"))?
            .text()?;
        let offset = offset as usize;
        if offset > text.len() || !text.is_char_boundary(offset) {
            return Err(fault(
                "LRR-E-INTERNAL-SOURCE-MAP",
                "source offset is outside a UTF-8 boundary",
            ));
        }
        let line = starts.partition_point(|start| *start as usize <= offset) - 1;
        let column = text[starts[line] as usize..offset].chars().count();
        Ok(LineColumn {
            line: line as u32 + 1,
            column: column as u32 + 1,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    id: Digest,
    parent: Option<Digest>,
    project: ProjectManifest,
    files: BTreeMap<SourcePath, SourceBlob>,
    source_map: SourceMap,
    cfg_by_crate: BTreeMap<Digest, CfgSet>,
    explicit_compile_env: BTreeMap<String, String>,
    facade_version: u32,
    semantics_version: u32,
}
impl Snapshot {
    pub fn new(
        project: ProjectManifest,
        files: BTreeMap<SourcePath, SourceBlob>,
        cfg_by_crate: BTreeMap<Digest, CfgSet>,
        explicit_compile_env: BTreeMap<String, String>,
        parent: Option<Digest>,
    ) -> Result<Self, Diagnostic> {
        if !files.contains_key(&project.root_source) {
            return Err(fault(
                "LRR-E-SOURCE-MISSING",
                "selected root source is missing",
            ));
        }
        if !project.crates.contains_key(&project.selected_crate) {
            return Err(fault("LRR-E-SOURCE-MISSING", "selected crate is missing"));
        }
        let total = files
            .values()
            .try_fold(0_usize, |total, blob| total.checked_add(blob.bytes().len()))
            .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "snapshot source accounting overflow"))?;
        if total > MAX_SNAPSHOT_SOURCE {
            return Err(fault("LRR-E-SOURCE-LIMIT", "snapshot exceeds 128 MiB"));
        }
        let source_map = SourceMap::new(&files);
        let mut snapshot = Self {
            id: Digest::default(),
            parent,
            project,
            files,
            source_map,
            cfg_by_crate,
            explicit_compile_env,
            facade_version: 1,
            semantics_version: 1,
        };
        snapshot.id = snapshot.fingerprint();
        Ok(snapshot)
    }
    pub fn id(&self) -> Digest {
        self.id
    }
    pub fn parent(&self) -> Option<Digest> {
        self.parent
    }
    pub fn project(&self) -> &ProjectManifest {
        &self.project
    }
    pub fn files(&self) -> &BTreeMap<SourcePath, SourceBlob> {
        &self.files
    }
    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }
    pub fn cfg_by_crate(&self) -> &BTreeMap<Digest, CfgSet> {
        &self.cfg_by_crate
    }
    pub fn compile_env(&self) -> &BTreeMap<String, String> {
        &self.explicit_compile_env
    }
}
impl StableEncode for Snapshot {
    const DOMAIN: &'static str = "lrr/v1/snapshot";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(crate::core::profile::SEMANTICS_PROFILE);
        e.u16(crate::core::profile::LIVE_EDITION);
        e.record(&self.project);
        e.map(&self.files, |e, path, blob| {
            e.string(path.as_str());
            e.digest(blob.content());
        });
        e.map(&self.cfg_by_crate, |e, key, cfg| {
            e.digest(*key);
            e.record(cfg);
        });
        e.map(&self.explicit_compile_env, |e, key, value| {
            e.string(key);
            e.string(value);
        });
        e.u32(self.facade_version);
        e.u32(self.semantics_version);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_paths_are_normalized_and_cannot_escape() {
        assert_eq!(SourcePath::new("a/../b.rs").unwrap().as_str(), "b.rs");
        assert!(SourcePath::new("../b.rs").is_err());
        assert!(SourcePath::new("/b.rs").is_err());
        assert!(SourcePath::new("").is_err());
        assert!(serde_json::from_str::<SourcePath>("\"../escape.rs\"").is_err());
    }
    #[test]
    fn source_blob_rejects_invalid_encoding_and_limit() {
        assert_eq!(
            SourceBlob::new(vec![0xff]).unwrap_err().code,
            "LRR-E-SOURCE-UTF8"
        );
        assert_eq!(
            SourceBlob::new(vec![b'a'; MAX_SOURCE_FILE + 1])
                .unwrap_err()
                .code,
            "LRR-E-SOURCE-LIMIT"
        );
    }
}
