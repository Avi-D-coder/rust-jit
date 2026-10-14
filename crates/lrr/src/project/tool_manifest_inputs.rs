//! Explicit preparation input grants, bounded reads, and pre/post capture witnesses.
use super::{InputInventory, PreparedFiles, ToolManifest, ToolStore, ToolchainRecord};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    source::snapshot::SourcePath,
};
use cap_std::fs::{Dir, Metadata, MetadataExt};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::Read,
    path::{Path, PathBuf},
};
const MAX_ENTRIES: usize = 100_000;
const MAX_BYTES: usize = 128 * 1024 * 1024;
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-TOOLCHAIN-CAPABILITY", message.into())
}
fn race() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-SNAPSHOT-RACE",
        "preparation input changed during capture".into(),
    )
}
fn stale() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-TOOL-PREPARE-STALE",
        "preparation inputs changed during tool work; repeat explicit preparation".into(),
    )
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum InputKind {
    Source,
    External,
}
#[derive(Debug, Clone, Eq, PartialEq)]
struct Stamp {
    dev: u64,
    ino: u64,
    len: u64,
    modified: Option<cap_std::time::SystemTime>,
    change: (i64, i64),
}
impl Stamp {
    fn of(m: &Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            len: m.len(),
            modified: m.modified().ok(),
            change: (m.ctime(), m.ctime_nsec()),
        }
    }
}
/// One explicitly granted directory; all requested paths stay below its capability.
pub struct InputRoot {
    root: Dir,
    original: PathBuf,
    kind: InputKind,
    files: BTreeSet<SourcePath>,
    directories: BTreeSet<SourcePath>,
    whole_root: bool,
    excluded: BTreeSet<SourcePath>,
}
impl InputRoot {
    pub fn open(
        kind: InputKind,
        root: &Path,
        files: BTreeSet<SourcePath>,
        directories: BTreeSet<SourcePath>,
        whole_root: bool,
        excluded: BTreeSet<SourcePath>,
    ) -> Result<Self, Diagnostic> {
        super::absolute(
            root.to_str()
                .ok_or_else(|| fault("input root must be UTF-8"))?,
        )?;
        if files
            .len()
            .checked_add(directories.len())
            .and_then(|n| n.checked_add(excluded.len()))
            .is_none_or(|n| n > MAX_ENTRIES)
        {
            return Err(fault("too many preparation grants"));
        }
        if files.iter().chain(&directories).any(|path| {
            excluded
                .iter()
                .any(|deny| Path::new(path.as_str()).starts_with(deny.as_str()))
        }) {
            return Err(fault("requested input is excluded from its grant"));
        }
        let capability = Dir::open_ambient_dir(root, cap_std::ambient_authority())
            .map_err(|e| fault(e.to_string()))?;
        Ok(Self {
            root: capability,
            original: root.to_owned(),
            kind,
            files,
            directories,
            whole_root,
            excluded,
        })
    }
    fn denied(&self, path: &Path) -> bool {
        self.excluded
            .iter()
            .any(|deny| path.starts_with(deny.as_str()))
    }
    fn verify_root(&self) -> Result<(u64, u64), Diagnostic> {
        let current = Dir::open_ambient_dir(&self.original, cap_std::ambient_authority())
            .map_err(|_| race())?;
        let now = current.dir_metadata().map_err(|_| race())?;
        let held = self.root.dir_metadata().map_err(|_| race())?;
        if (now.dev(), now.ino()) != (held.dev(), held.ino()) {
            return Err(race());
        }
        Ok((held.dev(), held.ino()))
    }
    fn capture(&self, capture: &mut InputCapture, total: &mut usize) -> Result<(), Diagnostic> {
        let root_identity = self.verify_root()?;
        let name = self
            .original
            .to_str()
            .ok_or_else(|| fault("input root encoding"))?
            .to_owned();
        if capture.roots.insert(name, root_identity).is_some() {
            return Err(fault("duplicate preparation input root"));
        }
        let mut queue = VecDeque::new();
        if self.whole_root {
            queue.push_back((PathBuf::from("."), Vec::<PathBuf>::new()));
        }
        for directory in &self.directories {
            queue.push_back((PathBuf::from(directory.as_str()), vec![]));
        }
        let mut files = self.files.clone();
        let mut seen = BTreeSet::new();
        while let Some((path, ancestors)) = queue.pop_front() {
            if !seen.insert(path.clone()) {
                continue;
            }
            if seen.len() > MAX_ENTRIES || ancestors.len() >= 256 {
                return Err(fault("preparation directory limit"));
            }
            let canonical = self
                .root
                .canonicalize(&path)
                .map_err(|e| fault(e.to_string()))?;
            if ancestors.contains(&canonical) {
                return Err(fault("preparation directory symlink cycle"));
            }
            let before = self.root.metadata(&path).map_err(|_| race())?;
            if !before.is_dir() {
                return Err(fault("granted source directory is not a directory"));
            }
            let mut members = BTreeSet::new();
            let mut next_ancestors = ancestors;
            next_ancestors.push(canonical.clone());
            for entry in self.root.read_dir(&path).map_err(|_| race())? {
                let entry = entry.map_err(|_| race())?;
                let relative = path.join(entry.file_name());
                let normalized = SourcePath::new(&relative)?;
                if self.denied(Path::new(normalized.as_str())) {
                    continue;
                }
                let child = self
                    .root
                    .metadata(normalized.as_str())
                    .map_err(|e| fault(e.to_string()))?;
                members.insert(SourcePath::new(Path::new(&entry.file_name()))?);
                if child.is_dir() {
                    queue.push_back((PathBuf::from(normalized.as_str()), next_ancestors.clone()));
                } else if child.is_file() {
                    files.insert(normalized);
                } else {
                    return Err(fault("preparation input is not a regular file/directory"));
                }
                if members
                    .len()
                    .checked_add(files.len())
                    .and_then(|n| n.checked_add(queue.len()))
                    .is_none_or(|n| n > MAX_ENTRIES)
                {
                    return Err(fault("preparation input membership limit"));
                }
            }
            let absolute = self
                .original
                .join(&path)
                .components()
                .collect::<PathBuf>()
                .to_str()
                .ok_or_else(|| fault("input path encoding"))?
                .to_owned();
            if capture
                .inventory
                .directory_membership
                .insert(absolute.clone(), members)
                .is_some()
            {
                return Err(fault("duplicate directory grant"));
            }
            capture.stamps.insert(
                absolute,
                (
                    canonical,
                    Stamp::of(&before),
                    Stamp::of(&self.root.symlink_metadata(&path).map_err(|_| race())?),
                ),
            );
            if Stamp::of(&self.root.metadata(&path).map_err(|_| race())?) != Stamp::of(&before) {
                return Err(race());
            }
        }
        for path in files {
            if self.denied(Path::new(path.as_str())) {
                return Err(fault("file escaped grant exclusions"));
            }
            let absolute = self
                .original
                .join(path.as_str())
                .to_str()
                .ok_or_else(|| fault("input path encoding"))?
                .to_owned();
            let map = match self.kind {
                InputKind::Source => &mut capture.inventory.source_files,
                InputKind::External => &mut capture.inventory.external_files,
            };
            let before = match self.root.metadata(path.as_str()) {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    if map.insert(absolute, None).is_some() {
                        return Err(fault("duplicate file grant"));
                    }
                    continue;
                }
                Err(e) => return Err(fault(e.to_string())),
            };
            if !before.is_file() || before.len() > MAX_BYTES as u64 {
                return Err(fault("input file is not a bounded regular file"));
            }
            let canonical = self
                .root
                .canonicalize(path.as_str())
                .map_err(|e| fault(e.to_string()))?;
            let link = self
                .root
                .symlink_metadata(path.as_str())
                .map_err(|_| race())?;
            let mut file = self
                .root
                .open(path.as_str())
                .map_err(|e| fault(e.to_string()))?;
            if Stamp::of(&file.metadata().map_err(|_| race())?) != Stamp::of(&before) {
                return Err(race());
            }
            let mut bytes = vec![];
            let mut buffer = [0; 16 * 1024];
            loop {
                let count = file.read(&mut buffer).map_err(|e| fault(e.to_string()))?;
                if count == 0 {
                    break;
                }
                *total = total
                    .checked_add(count)
                    .ok_or_else(|| fault("input size overflow"))?;
                if *total > MAX_BYTES {
                    return Err(fault("preparation input bytes exceed 128 MiB"));
                }
                bytes.extend_from_slice(&buffer[..count]);
            }
            if Stamp::of(&file.metadata().map_err(|_| race())?) != Stamp::of(&before)
                || Stamp::of(&self.root.metadata(path.as_str()).map_err(|_| race())?)
                    != Stamp::of(&before)
            {
                return Err(race());
            }
            if map
                .insert(absolute.clone(), Some(Digest::of_bytes(&bytes)))
                .is_some()
            {
                return Err(fault("duplicate file grant"));
            }
            capture
                .stamps
                .insert(absolute, (canonical, Stamp::of(&before), Stamp::of(&link)));
        }
        for (absolute, (canonical, target, link)) in &capture.stamps {
            let path = Path::new(absolute);
            if let Ok(relative) = path.strip_prefix(&self.original) {
                let relative = if relative.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    relative
                };
                if self.root.canonicalize(relative).map_err(|_| race())? != *canonical
                    || Stamp::of(&self.root.metadata(relative).map_err(|_| race())?) != *target
                    || Stamp::of(&self.root.symlink_metadata(relative).map_err(|_| race())?)
                        != *link
                {
                    return Err(race());
                }
            }
        }
        self.verify_root()?;
        Ok(())
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InputCapture {
    inventory: InputInventory,
    stamps: BTreeMap<String, (PathBuf, Stamp, Stamp)>,
    roots: BTreeMap<String, (u64, u64)>,
}
impl InputCapture {
    pub fn inventory(&self) -> &InputInventory {
        &self.inventory
    }
    pub fn verify_unchanged(&self, after: &Self) -> Result<(), Diagnostic> {
        if self != after { Err(stale()) } else { Ok(()) }
    }
}
pub struct InputPlan {
    roots: Vec<InputRoot>,
    environment: BTreeSet<String>,
}
impl InputPlan {
    pub fn new(roots: Vec<InputRoot>, environment: BTreeSet<String>) -> Result<Self, Diagnostic> {
        if roots
            .len()
            .checked_add(environment.len())
            .is_none_or(|n| n > MAX_ENTRIES)
        {
            return Err(fault("preparation grant limit"));
        }
        for name in &environment {
            super::env_name(name)?;
        }
        Ok(Self { roots, environment })
    }
    pub fn capture(&self) -> Result<InputCapture, Diagnostic> {
        self.capture_with(
            || {},
            |name| match std::env::var(name) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(std::env::VarError::NotUnicode(_)) => {
                    Err(fault("declared environment is not UTF-8"))
                }
            },
        )
    }
    fn capture_with(
        &self,
        between: impl FnOnce(),
        mut environment: impl FnMut(&str) -> Result<Option<String>, Diagnostic>,
    ) -> Result<InputCapture, Diagnostic> {
        let mut pass = || {
            let mut capture = InputCapture {
                inventory: InputInventory::default(),
                stamps: BTreeMap::new(),
                roots: BTreeMap::new(),
            };
            let mut total = 0usize;
            for root in &self.roots {
                root.capture(&mut capture, &mut total)?;
                capture.inventory.validate()?;
            }
            for name in &self.environment {
                let value = environment(name)?;
                if let Some(value) = &value {
                    super::text(value)?;
                }
                capture.inventory.environment.insert(name.clone(), value);
            }
            capture.inventory.validate()?;
            Ok::<_, Diagnostic>(capture)
        };
        let first = pass()?;
        between();
        let second = pass()?;
        if first != second {
            return Err(race());
        }
        Ok(first)
    }
}
impl ToolStore {
    pub fn publish_captured(
        &self,
        manifest: &ToolManifest,
        before: &InputCapture,
        after: &InputCapture,
    ) -> Result<Digest, Diagnostic> {
        before.verify_unchanged(after)?;
        if manifest.inputs != before.inventory {
            return Err(stale());
        }
        self.publish(manifest, &after.inventory)
    }
}
/// Pure adoption decision using current record, input and prepared-file witnesses; never runs tools.
pub fn verify_adoption(
    manifest: &ToolManifest,
    current_toolchain: &ToolchainRecord,
    current_inputs: &InputCapture,
    current_files: &PreparedFiles,
) -> Result<(), Diagnostic> {
    manifest.validate()?;
    current_files.verify_manifest(manifest)?;
    if manifest.toolchain.fingerprint() != current_toolchain.fingerprint()
        || manifest.inputs != current_inputs.inventory
    {
        return Err(stale());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::{
            Barrier,
            atomic::{AtomicU64, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let id = NEXT
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .unwrap();
            let root = std::env::temp_dir().join(format!(
                "lrr-preparation-inputs-{}-{}-{id}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn plan(&self) -> InputPlan {
            InputPlan::new(
                vec![
                    InputRoot::open(
                        InputKind::Source,
                        &self.0,
                        BTreeSet::new(),
                        BTreeSet::new(),
                        true,
                        BTreeSet::from([SourcePath::new(Path::new("target")).unwrap()]),
                    )
                    .unwrap(),
                ],
                BTreeSet::new(),
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn paths(values: &[&str]) -> BTreeSet<SourcePath> {
        values
            .iter()
            .map(|s| SourcePath::new(Path::new(s)).unwrap())
            .collect()
    }
    #[test]
    fn explicit_grants_capture_sources_external_absence_environment_and_membership() {
        let source = Fixture::new();
        let external = Fixture::new();
        fs::create_dir(source.0.join("src")).unwrap();
        fs::write(source.0.join("src/lib.rs"), b"fn live() {}\n").unwrap();
        fs::create_dir(source.0.join("target")).unwrap();
        fs::write(source.0.join("target/unselected"), b"do not capture").unwrap();
        fs::write(external.0.join("declared"), b"external input").unwrap();
        let plan = InputPlan::new(
            vec![
                InputRoot::open(
                    InputKind::Source,
                    &source.0,
                    paths(&["missing.rs"]),
                    BTreeSet::new(),
                    true,
                    paths(&["target"]),
                )
                .unwrap(),
                InputRoot::open(
                    InputKind::External,
                    &external.0,
                    paths(&["declared", "absent"]),
                    BTreeSet::new(),
                    false,
                    BTreeSet::new(),
                )
                .unwrap(),
            ],
            BTreeSet::from(["DECLARED".into(), "UNSET".into()]),
        )
        .unwrap();
        let capture = plan
            .capture_with(
                || {},
                |name| Ok((name == "DECLARED").then(|| "value".into())),
            )
            .unwrap();
        let inventory = capture.inventory();
        assert_eq!(inventory.source_files.len(), 2);
        assert_eq!(
            inventory.source_files[&source.0.join("missing.rs").to_str().unwrap().to_owned()],
            None
        );
        assert_eq!(
            inventory.source_files[&source.0.join("src/lib.rs").to_str().unwrap().to_owned()],
            Some(Digest::of_bytes(b"fn live() {}\n"))
        );
        assert_eq!(inventory.external_files.len(), 2);
        assert_eq!(inventory.environment["DECLARED"], Some("value".into()));
        assert_eq!(inventory.environment["UNSET"], None);
        assert_eq!(
            inventory.directory_membership[source.0.to_str().unwrap()],
            paths(&["src"])
        );
        assert_eq!(
            inventory.directory_membership[source.0.join("src").to_str().unwrap()],
            paths(&["lib.rs"])
        );
        fs::write(source.0.join("src/new.rs"), b"new").unwrap();
        let after = plan
            .capture_with(
                || {},
                |name| Ok((name == "DECLARED").then(|| "value".into())),
            )
            .unwrap();
        assert_eq!(
            capture.verify_unchanged(&after).unwrap_err().code,
            "LRR-E-TOOL-PREPARE-STALE"
        );
    }
    #[test]
    fn controlled_input_race_refuses_capture_then_stable_capture_succeeds() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("lib.rs"), b"before").unwrap();
        let barrier = Barrier::new(2);
        let plan = fixture.plan();
        std::thread::scope(|scope| {
            let writer = scope.spawn(|| {
                barrier.wait();
                fs::write(fixture.0.join("lib.rs"), b"after").unwrap();
                barrier.wait();
            });
            let result = plan.capture_with(
                || {
                    barrier.wait();
                    barrier.wait();
                },
                |_| Ok(None),
            );
            assert_eq!(result.unwrap_err().code, "LRR-E-SNAPSHOT-RACE");
            writer.join().unwrap();
        });
        let stable = plan.capture().unwrap();
        assert_eq!(
            stable.inventory.source_files[fixture.0.join("lib.rs").to_str().unwrap()],
            Some(Digest::of_bytes(b"after"))
        );
        stable.verify_unchanged(&plan.capture().unwrap()).unwrap();
    }
    #[test]
    fn restored_bytes_replaced_inode_and_environment_changes_remain_stale() {
        let fixture = Fixture::new();
        let path = fixture.0.join("lib.rs");
        fs::write(&path, b"same").unwrap();
        let plan = InputPlan::new(
            vec![
                InputRoot::open(
                    InputKind::Source,
                    &fixture.0,
                    paths(&["lib.rs"]),
                    BTreeSet::new(),
                    false,
                    BTreeSet::new(),
                )
                .unwrap(),
            ],
            BTreeSet::new(),
        )
        .unwrap();
        let before = plan.capture().unwrap();
        fs::write(fixture.0.join("replacement"), b"same").unwrap();
        fs::rename(fixture.0.join("replacement"), &path).unwrap();
        let after = plan.capture().unwrap();
        assert_eq!(before.inventory, after.inventory);
        assert_eq!(
            before.verify_unchanged(&after).unwrap_err().code,
            "LRR-E-TOOL-PREPARE-STALE"
        );
        let env = InputPlan::new(vec![], BTreeSet::from(["DECLARED".into()])).unwrap();
        let mut calls = 0;
        assert_eq!(
            env.capture_with(
                || {},
                |_| {
                    calls += 1;
                    Ok((calls == 2).then(String::new))
                }
            )
            .unwrap_err()
            .code,
            "LRR-E-SNAPSHOT-RACE"
        );
    }
    #[test]
    fn root_replacement_and_symlink_escape_or_cycle_are_rejected() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("lib.rs"), b"live").unwrap();
        let plan = fixture.plan();
        let retired = fixture.0.with_extension("retired");
        fs::rename(&fixture.0, &retired).unwrap();
        fs::create_dir(&fixture.0).unwrap();
        assert_eq!(plan.capture().unwrap_err().code, "LRR-E-SNAPSHOT-RACE");
        fs::remove_dir_all(&retired).unwrap();
        let external = Fixture::new();
        fs::write(external.0.join("secret"), b"outside").unwrap();
        std::os::unix::fs::symlink(external.0.join("secret"), fixture.0.join("escape")).unwrap();
        assert_eq!(
            fixture.plan().capture().unwrap_err().code,
            "LRR-E-TOOLCHAIN-CAPABILITY"
        );
        fs::remove_file(fixture.0.join("escape")).unwrap();
        std::os::unix::fs::symlink(".", fixture.0.join("cycle")).unwrap();
        assert_eq!(
            fixture.plan().capture().unwrap_err().code,
            "LRR-E-TOOLCHAIN-CAPABILITY"
        );
        fs::remove_file(fixture.0.join("cycle")).unwrap();
        fs::write(fixture.0.join("lib.rs"), b"live").unwrap();
        std::os::unix::fs::symlink("lib.rs", fixture.0.join("alias.rs")).unwrap();
        let inside = fixture.plan().capture().unwrap();
        assert_eq!(
            inside.inventory.source_files[fixture.0.join("lib.rs").to_str().unwrap()],
            inside.inventory.source_files[fixture.0.join("alias.rs").to_str().unwrap()]
        );
    }
}
