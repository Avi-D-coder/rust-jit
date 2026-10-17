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

fn fault(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message.into())
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourcePath(String);
impl SourcePath {
    pub(crate) fn heap_bytes(&self) -> usize {
        self.0.capacity()
    }
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Diagnostic> {
        if path.as_ref().as_os_str().as_encoded_bytes().contains(&0) {
            return Err(fault("LRR-E-SOURCE-PATH", "source path contains NUL"));
        }
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
        let path = Self::new(&s)?;
        if path.as_str() != s {
            return Err(fault(
                "LRR-E-SOURCE-PATH",
                "serialized source path must be normalized",
            ));
        }
        Ok(path)
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
    /// Ephemeral allocation accounting identity, never a semantic key.
    pub(crate) fn storage_id(&self) -> usize {
        self.bytes.as_ptr() as usize
    }
    pub fn new(bytes: Vec<u8>) -> Result<Self, Diagnostic> {
        std::str::from_utf8(&bytes)
            .map_err(|e| fault("LRR-E-SOURCE-UTF8", format!("invalid source encoding: {e}")))?;
        Ok(Self {
            content: Digest::of_bytes(&bytes),
            bytes: bytes.into(),
        })
    }
    /// Captured include_bytes data is not source text and is never parsed.
    pub(crate) fn included_bytes(bytes: Vec<u8>) -> Result<Self, Diagnostic> {
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
    const DOMAIN: &'static str = "lrr/v2/project-manifest";
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
#[serde(deny_unknown_fields)]
pub struct CfgSet {
    pub atoms: BTreeSet<String>,
    pub values: BTreeMap<String, BTreeSet<String>>,
}
impl StableEncode for CfgSet {
    const DOMAIN: &'static str = "lrr/v2/crate-cfg";
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
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>();
        let mut add = |n: usize| {
            bytes = bytes
                .checked_add(n)
                .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "cfg metadata accounting overflow"))?;
            Ok::<_, Diagnostic>(())
        };
        for atom in &self.atoms {
            add(64 + std::mem::size_of::<String>())?;
            add(atom.capacity())?;
        }
        for (key, values) in &self.values {
            add(64 + std::mem::size_of::<String>() + std::mem::size_of_val(values))?;
            add(key.capacity())?;
            for value in values {
                add(64 + std::mem::size_of::<String>())?;
                add(value.capacity())?;
            }
        }
        Ok(bytes)
    }
    pub fn guest_default() -> Self {
        Self {
            atoms: BTreeSet::from(["unix".into(), "debug_assertions".into()]),
            values: BTreeMap::from([
                (
                    "target_os".into(),
                    BTreeSet::from([std::env::consts::OS.into()]),
                ),
                (
                    "target_arch".into(),
                    BTreeSet::from([std::env::consts::ARCH.into()]),
                ),
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
    pub(crate) fn accounted_bytes(&self, allocations: &mut BTreeSet<usize>) -> usize {
        let mut bytes = std::mem::size_of::<Self>();
        for (path, starts) in &self.line_starts {
            bytes = bytes.saturating_add(
                64 + std::mem::size_of::<SourcePath>()
                    + path.0.capacity()
                    + std::mem::size_of::<Arc<[u32]>>(),
            );
            if allocations.insert(starts.as_ptr() as usize) {
                bytes = bytes.saturating_add(starts.len() * std::mem::size_of::<u32>() + 16);
            }
        }
        bytes
    }
    fn new(files: &BTreeMap<SourcePath, SourceBlob>) -> Result<Self, Diagnostic> {
        let limit = || fault("LRR-E-METADATA-LIMIT", "source line-map allocation limit");
        let mut line_starts = BTreeMap::new();
        let mut retained_bytes = 0usize;
        for (path, blob) in files {
            // Preflight the exact line table before its allocation. Source file
            // and snapshot byte ceilings remain the source-capture contract.
            let count = blob
                .bytes()
                .iter()
                .filter(|byte| **byte == b'\n')
                .count()
                .checked_add(1)
                .ok_or_else(limit)?;
            retained_bytes = retained_bytes
                .checked_add(
                    count
                        .checked_mul(std::mem::size_of::<u32>())
                        .ok_or_else(limit)?,
                )
                .ok_or_else(limit)?;
            let mut starts = Vec::new();
            starts.try_reserve_exact(count).map_err(|_| limit())?;
            starts.push(0);
            for (offset, byte) in blob.bytes().iter().enumerate() {
                if *byte == b'\n' {
                    starts.push(
                        u32::try_from(offset.checked_add(1).ok_or_else(limit)?)
                            .map_err(|_| limit())?,
                    );
                }
            }
            line_starts.insert(path.clone(), Arc::from(starts));
        }
        Ok(Self { line_starts })
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
    pub(crate) fn source_map_bytes(&self, allocations: &mut BTreeSet<usize>) -> usize {
        self.source_map.accounted_bytes(allocations)
    }
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
        let _total = files
            .values()
            .try_fold(0_usize, |total, blob| total.checked_add(blob.bytes().len()))
            .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "snapshot source accounting overflow"))?;
        let source_map = SourceMap::new(&files)?;
        let mut snapshot = Self {
            id: Digest::default(),
            parent,
            project,
            files,
            source_map,
            cfg_by_crate,
            explicit_compile_env,
            facade_version: 1,
            semantics_version: 2,
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
    /// Source bytes and line-map payloads have separate accounting groups.
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>();
        let mut add = |n: usize| {
            bytes = bytes.checked_add(n).ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "snapshot metadata accounting overflow",
                )
            })?;
            Ok::<_, Diagnostic>(())
        };
        add(self.project.identity.capacity())?;
        add(self.project.root_source.0.capacity())?;
        for key in self.project.crates.values() {
            add(64 + std::mem::size_of::<Digest>() + std::mem::size_of::<CrateKey>())?;
            for string in [
                &key.package.source_identity,
                &key.package.package_name,
                &key.package.version,
                &key.target_name,
            ] {
                add(string.capacity())?;
            }
        }
        for path in self.files.keys() {
            add(64 + std::mem::size_of::<SourcePath>() + std::mem::size_of::<SourceBlob>())?;
            add(path.0.capacity())?;
        }
        for cfg in self.cfg_by_crate.values() {
            add(64 + std::mem::size_of::<Digest>())?;
            add(cfg.metadata_bytes()?)?;
        }
        for (key, value) in &self.explicit_compile_env {
            add(64 + 2 * std::mem::size_of::<String>())?;
            add(key.capacity())?;
            add(value.capacity())?;
        }
        Ok(bytes)
    }
}
impl StableEncode for Snapshot {
    const DOMAIN: &'static str = "lrr/v2/snapshot";
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
    fn rustc_stock_line_map_preserves_utf8_crlf_and_trailing_line_offsets() {
        if let Some(expected) = std::env::var_os("LRR_VALIDATION_SOURCE_ROOT") {
            let root = std::fs::canonicalize(expected).unwrap();
            let compiled = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap();
            assert_eq!(std::fs::canonicalize(compiled).unwrap(), root);
            assert_eq!(
                include_bytes!("snapshot.rs").as_slice(),
                std::fs::read(root.join("crates/lrr/src/source/snapshot.rs")).unwrap()
            );
        }
        let original = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-invocation-span-error-7970.rs"
        );
        // Prefix/newline adaptations stay outside the unchanged licensed source.
        for text in [
            original.to_owned(),
            format!("// α β\r\n{}", original.replace('\n', "\r\n")),
            format!("\n{original}\n"),
        ] {
            let path = SourcePath::new("stock.rs").unwrap();
            let files = BTreeMap::from([(
                path.clone(),
                SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            )]);
            let map = SourceMap::new(&files).unwrap();
            for offset in 0..=text.len() {
                let observed = map.locate(&files, &path, u32::try_from(offset).unwrap());
                if !text.is_char_boundary(offset) {
                    assert_eq!(observed.unwrap_err().code, "LRR-E-INTERNAL-SOURCE-MAP");
                    continue;
                }
                let prefix = &text[..offset];
                let expected_line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
                let expected_column = prefix.rsplit('\n').next().unwrap().chars().count() + 1;
                assert_eq!(
                    observed.unwrap(),
                    LineColumn {
                        line: u32::try_from(expected_line).unwrap(),
                        column: u32::try_from(expected_column).unwrap()
                    }
                );
            }
        }
    }
    #[test]
    fn standalone_cfg_matches_actual_execution_platform() {
        let cfg = CfgSet::guest_default();
        assert_eq!(
            cfg.values["target_os"],
            BTreeSet::from([std::env::consts::OS.to_owned()])
        );
        assert_eq!(
            cfg.values["target_arch"],
            BTreeSet::from([std::env::consts::ARCH.to_owned()])
        );
        assert_eq!(
            cfg.values["target_pointer_width"],
            BTreeSet::from(["64".to_owned()])
        );
        assert_eq!(
            cfg.values["target_endian"],
            BTreeSet::from(["little".to_owned()])
        );
    }
    #[test]
    fn source_paths_are_normalized_and_cannot_escape() {
        assert_eq!(SourcePath::new("a/../b.rs").unwrap().as_str(), "b.rs");
        assert!(SourcePath::new("../b.rs").is_err());
        assert!(SourcePath::new("/b.rs").is_err());
        assert!(SourcePath::new("").is_err());
        assert!(serde_json::from_str::<SourcePath>("\"../escape.rs\"").is_err());
    }
    #[test]
    fn source_blob_rejects_invalid_encoding() {
        assert_eq!(
            SourceBlob::new(vec![0xff]).unwrap_err().code,
            "LRR-E-SOURCE-UTF8"
        );
    }
}
