//! Coordinator-side source authority, bounded reads, and race-aware capture.
use super::{
    manifest::{CrateKey, PackageKey, TargetKind},
    snapshot::{
        CfgSet, MAX_SNAPSHOT_SOURCE, MAX_SOURCE_FILE, ProjectManifest, Snapshot, SourceBlob,
        SourcePath,
    },
};
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncode},
};
use cap_std::fs::{Dir, Metadata};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::Read,
    path::{Path, PathBuf},
};

fn fault(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
#[derive(Debug, Clone, Eq, PartialEq)]
struct Stamp {
    length: u64,
    modified: Option<cap_std::time::SystemTime>,
    identity: (u64, u64),
    change: (i64, i64),
}
impl Stamp {
    fn of(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        let (identity, change) = {
            use cap_std::fs::MetadataExt;
            (
                (metadata.dev(), metadata.ino()),
                (metadata.ctime(), metadata.ctime_nsec()),
            )
        };
        #[cfg(not(unix))]
        let (identity, change) = ((0, 0), (0, 0));
        Self {
            length: metadata.len(),
            modified: metadata.modified().ok(),
            identity,
            change,
        }
    }
}
enum CaptureFailure {
    Race,
    Fault(Diagnostic),
}
impl From<Diagnostic> for CaptureFailure {
    fn from(error: Diagnostic) -> Self {
        Self::Fault(error)
    }
}

pub struct SourceLoader {
    root: Dir,
    root_identity: String,
    selected: SourcePath,
    approved_includes: BTreeSet<SourcePath>,
    profile: crate::core::profile::ProfileIdentity,
}
impl SourceLoader {
    /// Effect: opens only the user-selected standalone source root authority.
    pub fn standalone(root_file: &Path) -> Result<Self, Diagnostic> {
        Self::standalone_with_profile(root_file, crate::core::profile::ProfileIdentity::current())
    }
    pub fn standalone_with_profile(
        root_file: &Path,
        profile: crate::core::profile::ProfileIdentity,
    ) -> Result<Self, Diagnostic> {
        profile.validate_live()?;
        let parent = root_file
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = root_file
            .file_name()
            .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "selected root needs a filename"))?;
        let root_path =
            std::fs::canonicalize(parent).map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
        let root_identity = root_path
            .to_str()
            .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "source root must be UTF-8"))?
            .to_owned();
        let root = Dir::open_ambient_dir(&root_path, cap_std::ambient_authority())
            .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
        Ok(Self {
            root,
            root_identity,
            selected: SourcePath::new(Path::new(name))?,
            approved_includes: BTreeSet::new(),
            profile,
        })
    }
    /// Coordinator manifest additions, all restricted to the selected source root.
    pub fn with_includes(mut self, includes: impl IntoIterator<Item = SourcePath>) -> Self {
        self.approved_includes.extend(includes);
        self
    }
    /// Effect: reads approved source files; no runtime file permissions are granted.
    pub fn capture(&self) -> Result<Snapshot, Diagnostic> {
        self.capture_with(|_| {})
    }
    fn capture_with(
        &self,
        mut after_read: impl FnMut(&SourcePath),
    ) -> Result<Snapshot, Diagnostic> {
        for _ in 0..3 {
            match self.attempt(&mut after_read) {
                Ok(snapshot) => return Ok(snapshot),
                Err(CaptureFailure::Race) => {}
                Err(CaptureFailure::Fault(error)) => return Err(error),
            }
        }
        Err(fault(
            "LRR-E-SNAPSHOT-RACE",
            "source changed during each of three capture attempts",
        ))
    }
    fn canonical(&self, path: &SourcePath) -> Result<SourcePath, Diagnostic> {
        let canonical = self
            .root
            .canonicalize(path.as_str())
            .map_err(|e| fault("LRR-E-SOURCE-PATH", format!("{}: {e}", path.as_str())))?;
        SourcePath::new(canonical)
    }
    fn exists(&self, path: &SourcePath) -> Result<bool, Diagnostic> {
        match self.root.metadata(path.as_str()) {
            Ok(metadata) => Ok(metadata.is_file()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(fault(
                "LRR-E-SOURCE-PATH",
                format!("{}: {error}", path.as_str()),
            )),
        }
    }
    fn attempt(
        &self,
        after_read: &mut impl FnMut(&SourcePath),
    ) -> Result<Snapshot, CaptureFailure> {
        let cfg = CfgSet::guest_default();
        let mut files = BTreeMap::new();
        let mut stamps = BTreeMap::new();
        let mut queue = VecDeque::from([(self.selected.clone(), Some(PathBuf::new()))]);
        queue.extend(
            self.approved_includes
                .iter()
                .cloned()
                .map(|path| (path, None)),
        );
        let mut seen = BTreeSet::new();
        let mut total = 0_usize;
        while let Some((path, directory)) = queue.pop_front() {
            if !seen.insert((path.clone(), directory.clone())) {
                continue;
            }
            if seen.len() > 100_000 {
                return Err(fault(
                    "LRR-E-SOURCE-LIMIT",
                    "source module graph exceeds 100,000 files",
                )
                .into());
            }
            self.canonical(&path)?; // Capability canonicalization refuses symlink escapes.
            let before = self
                .root
                .metadata(path.as_str())
                .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
            if !before.is_file() {
                return Err(fault("LRR-E-SOURCE-IO", "source is not a regular file").into());
            }
            if before.len() > MAX_SOURCE_FILE as u64 {
                return Err(fault("LRR-E-SOURCE-LIMIT", "source file exceeds 8 MiB").into());
            }
            let mut file = self
                .root
                .open(path.as_str())
                .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
            if Stamp::of(&before)
                != Stamp::of(
                    &file
                        .metadata()
                        .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?,
                )
            {
                return Err(CaptureFailure::Race);
            }
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 16 * 1024];
            loop {
                let count = file
                    .read(&mut buffer)
                    .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
                if count == 0 {
                    break;
                }
                let size = bytes
                    .len()
                    .checked_add(count)
                    .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "source size overflow"))?;
                if size > MAX_SOURCE_FILE {
                    return Err(fault("LRR-E-SOURCE-LIMIT", "source grew past 8 MiB").into());
                }
                bytes
                    .try_reserve(count)
                    .map_err(|e| fault("LRR-E-SOURCE-LIMIT", e.to_string()))?;
                bytes.extend_from_slice(&buffer[..count]);
            }
            after_read(&path);
            let after = self
                .root
                .metadata(path.as_str())
                .map_err(|_| CaptureFailure::Race)?;
            if Stamp::of(&before) != Stamp::of(&after) {
                return Err(CaptureFailure::Race);
            }
            total = total
                .checked_add(if files.contains_key(&path) {
                    0
                } else {
                    bytes.len()
                })
                .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "snapshot size overflow"))?;
            if total > MAX_SNAPSHOT_SOURCE {
                return Err(fault("LRR-E-SOURCE-LIMIT", "snapshot exceeds 128 MiB").into());
            }
            let blob = if directory.is_some() {
                SourceBlob::new(bytes)?
            } else {
                SourceBlob::included_bytes(bytes)?
            };
            if let Some(directory) = directory {
                for request in crate::syntax::modules::external_modules_in_profile(
                    blob.text()?,
                    &cfg,
                    &self.profile,
                )? {
                    let (child, child_directory) =
                        super::graph::module_target(&path, &directory, request, |p| {
                            self.exists(p)
                        })?;
                    queue.push_back((child, Some(child_directory)));
                }
            }
            stamps.insert(path.clone(), Stamp::of(&after));
            files.insert(path, blob);
        }
        for (path, stamp) in stamps {
            let current = self
                .root
                .metadata(path.as_str())
                .map_err(|_| CaptureFailure::Race)?;
            if Stamp::of(&current) != stamp {
                return Err(CaptureFailure::Race);
            }
        }
        let crate_key = CrateKey {
            package: PackageKey {
                source_identity: self.root_identity.clone(),
                package_name: "standalone".into(),
                version: "0.1.0".into(),
            },
            target_kind: TargetKind::Binary,
            target_name: self.selected.as_str().to_owned(),
            guest_edition: self.profile.edition,
            semantics_profile: self.profile.fingerprint(),
            cfg_fingerprint: cfg.fingerprint(),
            dependency_aliases: Digest::of_bytes(b""),
        };
        let selected_crate = crate_key.fingerprint();
        let project = ProjectManifest {
            identity: self.root_identity.clone(),
            root_source: self.selected.clone(),
            selected_crate,
            crates: BTreeMap::from([(selected_crate, crate_key)]),
        };
        Snapshot::new(
            project,
            files,
            BTreeMap::from([(selected_crate, cfg)]),
            BTreeMap::new(),
            None,
        )
        .map_err(CaptureFailure::Fault)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "lrr-source-{}-{}-{}",
                NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn write(&self, path: &str, text: &str) {
            let path = self.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        fn loader(&self) -> SourceLoader {
            SourceLoader::standalone(&self.0.join("main.rs")).unwrap()
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn snapshot_is_immutable_after_disk_edit() {
        let temp = Temp::new();
        temp.write("main.rs", "mod child; fn main() {}\n");
        temp.write("child.rs", "fn cold() { absent(); }");
        let loader = temp.loader();
        let old = loader.capture().unwrap();
        temp.write("child.rs", "fn changed() {}");
        let new = loader.capture().unwrap();
        assert_ne!(old.id(), new.id());
        assert!(
            old.files()[&SourcePath::new("child.rs").unwrap()]
                .text()
                .unwrap()
                .contains("absent")
        );
    }
    #[test]
    fn module_path_escape_denied() {
        let temp = Temp::new();
        temp.write("main.rs", "#[path=\"../escape.rs\"] mod escape;");
        assert_eq!(
            temp.loader().capture().unwrap_err().code,
            "LRR-E-SOURCE-PATH"
        );
    }
    #[cfg(unix)]
    #[test]
    fn module_symlink_escape_denied() {
        let temp = Temp::new();
        let outside = Temp::new();
        outside.write("secret.rs", "fn secret() {}");
        temp.write("main.rs", "mod child;");
        std::os::unix::fs::symlink(outside.0.join("secret.rs"), temp.0.join("child.rs")).unwrap();
        assert_eq!(
            temp.loader().capture().unwrap_err().code,
            "LRR-E-SOURCE-PATH"
        );
    }
    #[test]
    fn module_rs_ambiguity_is_error() {
        let temp = Temp::new();
        temp.write("main.rs", "mod child;");
        temp.write("child.rs", "");
        temp.write("child/mod.rs", "");
        assert_eq!(
            temp.loader().capture().unwrap_err().code,
            "LRR-E-MODULE-AMBIGUOUS"
        );
    }
    #[test]
    fn source_batch_is_atomic() {
        let temp = Temp::new();
        temp.write("main.rs", "mod child; fn main() {}");
        temp.write("child.rs", "fn old() {}");
        let original = temp.loader().capture().unwrap();
        let root = SourcePath::new("main.rs").unwrap();
        let child = SourcePath::new("child.rs").unwrap();
        let failed = super::super::changes::apply_batch(
            &original,
            vec![
                (root.clone(), Some(b"fn new() {}".to_vec())),
                (child.clone(), Some(vec![0xff])),
            ],
        );
        assert!(failed.is_err());
        assert!(
            original.files()[&root]
                .text()
                .unwrap()
                .contains("mod child")
        );
        let changed = super::super::changes::apply_batch(
            &original,
            vec![
                (root.clone(), Some(b"mod child; fn new() {}".to_vec())),
                (child, Some(b"fn new_child() {}".to_vec())),
            ],
        )
        .unwrap();
        assert_eq!(changed.parent(), Some(original.id()));
        assert_ne!(changed.id(), original.id());
        assert_eq!(
            std::fs::read_to_string(temp.0.join("main.rs")).unwrap(),
            "mod child; fn main() {}"
        );
        assert!(super::super::changes::apply_batch(&original, vec![(root, None)]).is_err());
    }
    #[test]
    fn racing_capture_reports_limit() {
        let temp = Temp::new();
        temp.write("main.rs", "fn main() {}");
        let mut attempts = 0;
        let error = temp
            .loader()
            .capture_with(|_| {
                attempts += 1;
                temp.write(
                    "main.rs",
                    &format!("fn main() {{}} // {}", "x".repeat(attempts)),
                );
            })
            .unwrap_err();
        assert_eq!(attempts, 3);
        assert_eq!(error.code, "LRR-E-SNAPSHOT-RACE");
    }
    #[test]
    fn snapshot_identity_excludes_parent_and_maps_utf8_lines() {
        let temp = Temp::new();
        temp.write("main.rs", "// é\nfn main() {}");
        let original = temp.loader().capture().unwrap();
        let repeated = super::super::changes::apply_batch(&original, vec![]).unwrap();
        assert_eq!(repeated.id(), original.id());
        let path = SourcePath::new("main.rs").unwrap();
        assert_eq!(
            original
                .source_map()
                .locate(original.files(), &path, 6)
                .unwrap(),
            super::super::snapshot::LineColumn { line: 2, column: 1 }
        );
        assert!(
            original
                .source_map()
                .locate(original.files(), &path, 4)
                .is_err()
        );
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;
    static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "lrr-source-extra-{}-{}-{}",
                NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn write(&self, path: &str, bytes: &[u8]) {
            let path = self.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        fn loader(&self) -> SourceLoader {
            SourceLoader::standalone(&self.0.join("main.rs")).unwrap()
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn approved_binary_include_is_captured_without_parsing() {
        let temp = Temp::new();
        temp.write("main.rs", b"fn main() {}");
        temp.write("data.bin", &[0xff, 0, 1]);
        let include = SourcePath::new("data.bin").unwrap();
        let snapshot = temp
            .loader()
            .with_includes([include.clone()])
            .capture()
            .unwrap();
        assert_eq!(snapshot.files()[&include].bytes(), &[0xff, 0, 1]);
        assert!(snapshot.files()[&include].text().is_err());
    }
    #[test]
    fn path_override_uses_file_parent_and_inline_override_directory() {
        let temp = Temp::new();
        temp.write(
            "main.rs",
            b"mod child; #[path=\"renamed\"] mod inline { #[path=\"other.rs\"] mod inner; }",
        );
        temp.write("child.rs", b"#[path=\"sibling.rs\"] mod nested;");
        temp.write("sibling.rs", b"fn cold() { absent(); }");
        temp.write("renamed/other.rs", b"fn cold() { another_absent(); }");
        let snapshot = temp.loader().capture().unwrap();
        assert!(
            snapshot
                .files()
                .contains_key(&SourcePath::new("sibling.rs").unwrap())
        );
        assert!(
            snapshot
                .files()
                .contains_key(&SourcePath::new("renamed/other.rs").unwrap())
        );
    }
    #[test]
    fn overlay_module_addition_requires_atomic_source_bytes() {
        let temp = Temp::new();
        temp.write("main.rs", b"fn main() {}");
        let old = temp.loader().capture().unwrap();
        let root = SourcePath::new("main.rs").unwrap();
        assert_eq!(
            super::super::changes::apply_batch(
                &old,
                vec![(root.clone(), Some(b"mod new;".to_vec()))]
            )
            .unwrap_err()
            .code,
            "LRR-E-SOURCE-MISSING"
        );
        let new = super::super::changes::apply_batch(
            &old,
            vec![
                (root, Some(b"mod new;".to_vec())),
                (
                    SourcePath::new("new.rs").unwrap(),
                    Some(b"fn cold() { absent(); }".to_vec()),
                ),
            ],
        )
        .unwrap();
        assert_eq!(new.files().len(), 2);
        assert_eq!(old.files().len(), 1);
    }
}

#[cfg(test)]
mod io_tests {
    #[test]
    fn source_io_error_is_configuration_failure() {
        let root = std::env::temp_dir().join(format!(
            "lrr-missing-source-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let result = super::SourceLoader::standalone(&root.join("absent.rs"));
        assert_eq!(result.err().unwrap().code, "LRR-E-SOURCE-IO");
    }
}
