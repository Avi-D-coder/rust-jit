//! Explicit coordinator include capture; pure queries never open these capabilities.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    source::{
        generated::GeneratedTree,
        include_paths::{
            GeneratedSourceRequest, IncludeAuthorities, IncludeKind, IncludeLocation,
            IncludeRootKind,
        },
        snapshot::{MAX_SOURCE_FILE, SourceBlob, SourcePath},
    },
};
use cap_std::fs::{Dir, Metadata};
use std::{collections::BTreeMap, io::Read};
fn fault(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
#[derive(Eq, PartialEq)]
struct Stamp {
    len: u64,
    modified: Option<cap_std::time::SystemTime>,
    identity: (u64, u64),
    change: (i64, i64),
}
impl Stamp {
    fn of(m: &Metadata) -> Self {
        use cap_std::fs::MetadataExt;
        Self {
            len: m.len(),
            modified: m.modified().ok(),
            identity: (m.dev(), m.ino()),
            change: (m.ctime(), m.ctime_nsec()),
        }
    }
}
pub use crate::source::included::CapturedInclude;
pub struct SourceCapture {
    authorities: IncludeAuthorities,
    source_dirs: BTreeMap<Digest, Dir>,
    outputs: BTreeMap<Digest, GeneratedTree>,
}
impl SourceCapture {
    fn verify_roots(&self) -> Result<(), Diagnostic> {
        use cap_std::fs::MetadataExt as _;
        use std::os::unix::fs::MetadataExt as _;
        for root in self.authorities.roots() {
            if root.kind != IncludeRootKind::Source {
                continue;
            }
            let dir = self
                .source_dirs
                .get(&root.identity)
                .ok_or_else(|| fault("LRR-E-SNAPSHOT-RACE", "source capability is absent"))?;
            let opened = dir.metadata(".").map_err(|_| {
                fault(
                    "LRR-E-SNAPSHOT-RACE",
                    "source capability root is unavailable",
                )
            })?;
            let named = std::fs::metadata(&root.path)
                .map_err(|_| fault("LRR-E-SNAPSHOT-RACE", "named source root is unavailable"))?;
            if (opened.dev(), opened.ino()) != (named.dev(), named.ino())
                || std::fs::canonicalize(&root.path).ok().as_ref() != Some(&root.path)
            {
                return Err(fault(
                    "LRR-E-SNAPSHOT-RACE",
                    "source root changed after capability open",
                ));
            }
        }
        Ok(())
    }
    /// Explicit coordinator effect. No staged record is returned before revalidation.
    pub fn capture_set(
        &self,
        requests: &[GeneratedSourceRequest],
    ) -> Result<crate::source::included::CapturedIncludeSet, Diagnostic> {
        self.capture_set_with(requests, || {})
    }
    fn capture_set_with(
        &self,
        requests: &[GeneratedSourceRequest],
        before_verify: impl FnOnce(),
    ) -> Result<crate::source::included::CapturedIncludeSet, Diagnostic> {
        if requests.len() > 100_000 {
            return Err(fault(
                "LRR-E-SOURCE-LIMIT",
                "include set count exceeds limit",
            ));
        }
        self.verify_roots()?;
        let mut records = BTreeMap::new();
        let mut bytes = 0usize;
        let mut metadata = 0usize;
        for request in requests {
            if records.contains_key(&request.identity()) {
                return Err(fault(
                    "LRR-E-SOURCE-PATH",
                    "duplicate include request in source set",
                ));
            }
            let record = self.capture(request)?;
            bytes = bytes
                .checked_add(record.blob().bytes().len())
                .filter(|n| *n <= crate::source::snapshot::MAX_SNAPSHOT_SOURCE)
                .ok_or_else(|| {
                    fault("LRR-E-SOURCE-LIMIT", "include set exceeds source byte cap")
                })?;
            metadata = metadata
                .checked_add(
                    record
                        .metadata_bytes()
                        .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "include metadata overflow"))?,
                )
                .and_then(|n| n.checked_add(96))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "include set exceeds metadata cap"))?;
            records.insert(request.identity(), record);
        }
        before_verify();
        for request in requests {
            self.revalidate(request, &records[&request.identity()])?;
        }
        self.verify_roots()?;
        let mut e = StableEncoder::new("lrr/v2/captured-include-set");
        e.digest(self.authorities.fingerprint());
        e.u64(
            u64::try_from(records.len())
                .map_err(|_| fault("LRR-E-SOURCE-LIMIT", "include count overflow"))?,
        );
        for (request, record) in &records {
            e.digest(*request);
            e.digest(record.identity());
        }
        Ok(crate::source::included::CapturedIncludeSet {
            identity: e.finish(),
            records,
        })
    }
    /// Recheck a prior immutable capture; changed bytes/physical address refuse publication.
    pub fn revalidate(
        &self,
        request: &GeneratedSourceRequest,
        record: &CapturedInclude,
    ) -> Result<(), Diagnostic> {
        if record.request() != request.identity() {
            return Err(fault(
                "LRR-E-SOURCE-PATH",
                "include revalidation request mismatch",
            ));
        }
        let current = self.capture(request)?;
        if current.identity() != record.identity() {
            return Err(fault(
                "LRR-E-SNAPSHOT-RACE",
                format!(
                    "included source changed; caller origin {}; include origin {}",
                    Digest(request.from().origin.0),
                    Digest(request.include_origin().0)
                ),
            ));
        }
        Ok(())
    }
    /// Effect: opens only explicit source authorities. Build outputs remain immutable snapshots.
    pub fn open(
        authorities: IncludeAuthorities,
        outputs: BTreeMap<Digest, GeneratedTree>,
    ) -> Result<Self, Diagnostic> {
        Self::open_with(authorities, outputs, |_| {})
    }
    fn open_with(
        authorities: IncludeAuthorities,
        outputs: BTreeMap<Digest, GeneratedTree>,
        mut before_open: impl FnMut(&std::path::Path),
    ) -> Result<Self, Diagnostic> {
        for (id, tree) in &outputs {
            tree.validate()?;
            let root = authorities
                .root(*id)
                .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "captured output authority is absent"))?;
            if root.kind
                != (IncludeRootKind::BuildOutput {
                    producer: tree.producer(),
                })
                || root.path != std::path::Path::new(tree.out_dir())
            {
                return Err(fault(
                    "LRR-E-SOURCE-PATH",
                    "captured output producer/root differs",
                ));
            }
        }
        let mut source_dirs = BTreeMap::new();
        for root in authorities.roots() {
            if root.kind != IncludeRootKind::Source {
                continue;
            }
            let actual = std::fs::canonicalize(&root.path)
                .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
            if actual != root.path {
                return Err(fault(
                    "LRR-E-SOURCE-PATH",
                    "source authority must be preparation-canonical",
                ));
            }
            use std::os::unix::fs::MetadataExt as _;
            let before = std::fs::metadata(&root.path)
                .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
            if !before.is_dir() {
                return Err(fault(
                    "LRR-E-SOURCE-PATH",
                    "source authority is not a directory",
                ));
            }
            let expected = (before.dev(), before.ino());
            before_open(&root.path);
            let dir = Dir::open_ambient_dir(&root.path, cap_std::ambient_authority())
                .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
            use cap_std::fs::MetadataExt as _;
            let opened = dir
                .metadata(".")
                .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
            let named = std::fs::metadata(&root.path)
                .map_err(|_| fault("LRR-E-SNAPSHOT-RACE", "source root disappeared during open"))?;
            if (opened.dev(), opened.ino()) != expected
                || (named.dev(), named.ino()) != expected
                || std::fs::canonicalize(&root.path).ok().as_ref() != Some(&root.path)
            {
                return Err(fault(
                    "LRR-E-SNAPSHOT-RACE",
                    "source root identity changed during open",
                ));
            }
            source_dirs.insert(root.identity, dir);
        }
        Ok(Self {
            authorities,
            source_dirs,
            outputs,
        })
    }
    /// Resolve a raw spelling using only named approved capabilities, never ambient canonicalization.
    fn resolve_physical(
        &self,
        request: &GeneratedSourceRequest,
    ) -> Result<std::path::PathBuf, Diagnostic> {
        self.resolve_path(request, false)?
            .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "included source path is absent"))
    }
    fn resolve_optional_physical(
        &self,
        request: &GeneratedSourceRequest,
    ) -> Result<Option<std::path::PathBuf>, Diagnostic> {
        self.resolve_path(request, true)
    }
    fn resolve_path(
        &self,
        request: &GeneratedSourceRequest,
        allow_missing: bool,
    ) -> Result<Option<std::path::PathBuf>, Diagnostic> {
        use std::{
            collections::VecDeque,
            ffi::OsString,
            path::{Component, Path, PathBuf},
        };
        fn parts(path: &Path) -> VecDeque<OsString> {
            path.components()
                .filter_map(|c| match c {
                    Component::Normal(v) => Some(v.to_owned()),
                    Component::ParentDir => Some(OsString::from("..")),
                    Component::CurDir => None,
                    _ => None,
                })
                .collect()
        }
        let mut pending = parts(request.original_target());
        let mut current = PathBuf::from("/");
        let mut links = 0usize;
        let mut work = 0usize;
        while let Some(part) = pending.pop_front() {
            work = work
                .checked_add(1)
                .filter(|n| *n <= 4096)
                .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "include path resolution work limit"))?;
            if part == ".." {
                current.pop();
                if current.as_os_str().is_empty() {
                    current.push("/");
                }
                continue;
            }
            current.push(part);
            let root = self
                .authorities
                .roots()
                .filter(|r| current.starts_with(&r.path))
                .max_by_key(|r| r.path.components().count());
            let Some(root) = root else {
                if self
                    .authorities
                    .roots()
                    .any(|r| r.path.starts_with(&current))
                {
                    continue;
                }
                return Err(fault(
                    "LRR-E-SOURCE-PATH",
                    "include path traverses unapproved source authority",
                ));
            };
            let relative = current.strip_prefix(&root.path).map_err(|_| {
                fault(
                    "LRR-E-SOURCE-PATH",
                    "include resolution root prefix differs",
                )
            })?;
            if relative.as_os_str().is_empty() {
                continue;
            }
            if let IncludeRootKind::BuildOutput { .. } = root.kind {
                let tree = self.outputs.get(&root.identity).ok_or_else(|| {
                    fault(
                        "LRR-E-GENERATED-SOURCE",
                        "include output snapshot is unavailable",
                    )
                })?;
                let name = SourcePath::new(relative)?;
                if tree.files().contains_key(&name) {
                    if !pending.is_empty() {
                        return Err(fault(
                            "LRR-E-SOURCE-PATH",
                            "include output file used as directory",
                        ));
                    }
                } else if !tree.directories().contains(&name)
                    && !tree
                        .files()
                        .keys()
                        .any(|p| p.as_str().starts_with(&format!("{}/", name.as_str())))
                {
                    return if allow_missing {
                        Ok(None)
                    } else {
                        Err(fault(
                            "LRR-E-GENERATED-SOURCE",
                            "include output path absent from snapshot",
                        ))
                    };
                }
                continue;
            }
            let dir = self
                .source_dirs
                .get(&root.identity)
                .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "include source capability is absent"))?;
            let metadata = match dir.symlink_metadata(relative) {
                Ok(metadata) => metadata,
                Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(None);
                }
                Err(error) => return Err(fault("LRR-E-SOURCE-PATH", error.to_string())),
            };
            if metadata.file_type().is_symlink() {
                links = links
                    .checked_add(1)
                    .filter(|n| *n <= 40)
                    .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "include symbolic link limit"))?;
                let target = dir
                    .read_link_contents(relative)
                    .map_err(|e| fault("LRR-E-SOURCE-PATH", e.to_string()))?;
                if target
                    .to_str()
                    .is_none_or(|s| s.len() > 64 * 1024 || s.contains('\0'))
                {
                    return Err(fault("LRR-E-SOURCE-PATH", "invalid include link target"));
                }
                if target.is_absolute() {
                    current = PathBuf::from("/");
                } else {
                    current.pop();
                }
                let mut next = parts(&target);
                next.append(&mut pending);
                if next.len() > 1024 {
                    return Err(fault("LRR-E-SOURCE-PATH", "include link component limit"));
                }
                pending = next;
            } else if !pending.is_empty() && !metadata.is_dir() {
                return Err(fault(
                    "LRR-E-SOURCE-PATH",
                    "included file used as directory",
                ));
            }
        }
        if !self
            .authorities
            .roots()
            .any(|r| current.starts_with(&r.path))
        {
            return Err(fault(
                "LRR-E-SOURCE-PATH",
                "resolved include has no source authority",
            ));
        }
        Ok(Some(current))
    }
    fn module_nonfile_identity(
        &self,
        candidate: &GeneratedSourceRequest,
    ) -> Result<Option<Digest>, Diagnostic> {
        let physical = self.resolve_optional_physical(candidate)?.ok_or_else(|| {
            fault(
                "LRR-E-SNAPSHOT-RACE",
                "module candidate disappeared during observation",
            )
        })?;
        let root = self
            .authorities
            .roots()
            .filter(|root| physical.starts_with(&root.path))
            .max_by_key(|root| root.path.components().count())
            .ok_or_else(|| {
                fault(
                    "LRR-E-SOURCE-PATH",
                    "module candidate has no physical authority",
                )
            })?;
        let relative = physical.strip_prefix(&root.path).map_err(|_| {
            fault(
                "LRR-E-SOURCE-PATH",
                "module candidate physical prefix differs",
            )
        })?;
        let mut identity = StableEncoder::new("lrr/v2/module-nonfile-observation");
        identity.digest(candidate.identity());
        identity.digest(root.identity);
        identity.string(
            relative
                .to_str()
                .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "module physical path is not UTF-8"))?,
        );
        match root.kind {
            IncludeRootKind::BuildOutput { .. } => {
                let tree = self.outputs.get(&root.identity).ok_or_else(|| {
                    fault("LRR-E-GENERATED-SOURCE", "module output snapshot absent")
                })?;
                if tree.files().contains_key(&SourcePath::new(relative)?) {
                    return Ok(None);
                }
                identity.digest(tree.fingerprint());
            }
            IncludeRootKind::Source => {
                use cap_std::fs::MetadataExt as _;
                let dir = self
                    .source_dirs
                    .get(&root.identity)
                    .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "module source capability absent"))?;
                let metadata = dir.metadata(relative).map_err(|_| {
                    fault(
                        "LRR-E-SNAPSHOT-RACE",
                        "module candidate metadata unavailable",
                    )
                })?;
                if metadata.is_file() {
                    return Ok(None);
                }
                identity.u64(metadata.dev());
                identity.u64(metadata.ino());
                identity.u64(metadata.len());
                identity.u32(metadata.mode());
                identity.bytes(&metadata.ctime().to_le_bytes());
                identity.bytes(&metadata.ctime_nsec().to_le_bytes());
            }
        }
        Ok(Some(identity.finish()))
    }
    /// Explicit module-candidate observation. Only NotFound becomes absence.
    fn observe_module(
        &self,
        request: &crate::source::module_capture::ModuleCaptureRequest,
    ) -> Result<Vec<crate::source::module_capture::ModuleObservation>, Diagnostic> {
        use crate::source::module_capture::ModuleObservation;
        if request.candidates().is_empty() || request.candidates().len() > 2 {
            return Err(fault("LRR-E-SOURCE-PATH", "module candidate count differs"));
        }
        request.metadata_bytes()?;
        self.verify_roots()?;
        let mut observations = Vec::with_capacity(request.candidates().len());
        for candidate in request.candidates() {
            if !self.authorities.matches_request(candidate)
                || candidate.kind() != IncludeKind::Syntax
            {
                return Err(fault(
                    "LRR-E-SOURCE-PATH",
                    "stale or foreign module capture request",
                ));
            }
            let present = match candidate.root_kind() {
                IncludeRootKind::BuildOutput { .. } => {
                    let tree = self.outputs.get(&candidate.root()).ok_or_else(|| {
                        fault("LRR-E-GENERATED-SOURCE", "module output snapshot absent")
                    })?;
                    tree.files().contains_key(candidate.path())
                        || tree.directories().contains(candidate.path())
                        || tree.files().keys().any(|path| {
                            path.as_str()
                                .strip_prefix(candidate.path().as_str())
                                .is_some_and(|suffix| suffix.starts_with('/'))
                        })
                }
                IncludeRootKind::Source => self.resolve_optional_physical(candidate)?.is_some(),
            };
            observations.push(if present {
                match self.module_nonfile_identity(candidate)? {
                    Some(identity) => ModuleObservation::NonFile { identity },
                    None => ModuleObservation::Present(Box::new(self.capture(candidate)?)),
                }
            } else {
                ModuleObservation::Absent
            });
        }
        Ok(observations)
    }
    fn module_identity(
        request: &crate::source::module_capture::ModuleCaptureRequest,
        observations: &[crate::source::module_capture::ModuleObservation],
    ) -> Digest {
        let mut identity = StableEncoder::new("lrr/v2/captured-module-candidates");
        identity.digest(request.identity());
        identity.sequence(observations, |e, observation| match observation {
            crate::source::module_capture::ModuleObservation::Absent => e.u8(0),
            crate::source::module_capture::ModuleObservation::NonFile { identity } => {
                e.u8(2);
                e.digest(*identity);
            }
            crate::source::module_capture::ModuleObservation::Present(record) => {
                e.u8(1);
                e.digest(record.identity());
            }
        });
        identity.finish()
    }
    /// Effectful capture only; returning this record does not adopt it into a query universe.
    pub fn capture_module(
        &self,
        request: &crate::source::module_capture::ModuleCaptureRequest,
    ) -> Result<crate::source::module_capture::CapturedModule, Diagnostic> {
        self.capture_module_with(request, || {})
    }
    fn capture_module_with(
        &self,
        request: &crate::source::module_capture::ModuleCaptureRequest,
        before_verify: impl FnOnce(),
    ) -> Result<crate::source::module_capture::CapturedModule, Diagnostic> {
        let observations = self.observe_module(request)?;
        let identity = Self::module_identity(request, &observations);
        before_verify();
        let checked = self.observe_module(request)?;
        if Self::module_identity(request, &checked) != identity {
            return Err(fault(
                "LRR-E-SNAPSHOT-RACE",
                "module source or candidate membership changed during capture",
            ));
        }
        self.verify_roots()?;
        let record = crate::source::module_capture::CapturedModule {
            request: request.identity(),
            identity,
            observations,
            directory: request.directory().to_owned(),
        };
        record.metadata_bytes()?;
        Ok(record)
    }
    pub fn revalidate_module(
        &self,
        request: &crate::source::module_capture::ModuleCaptureRequest,
        record: &crate::source::module_capture::CapturedModule,
    ) -> Result<(), Diagnostic> {
        if record.request() != request.identity() {
            return Err(fault(
                "LRR-E-SOURCE-PATH",
                "module revalidation request mismatch",
            ));
        }
        let current = self.capture_module(request)?;
        if current.identity() != record.identity() {
            return Err(fault(
                "LRR-E-SNAPSHOT-RACE",
                "module source or candidate membership changed",
            ));
        }
        Ok(())
    }
    pub fn capture(&self, request: &GeneratedSourceRequest) -> Result<CapturedInclude, Diagnostic> {
        self.capture_with(request, || {})
    }
    fn capture_with(
        &self,
        request: &GeneratedSourceRequest,
        after_read: impl FnOnce(),
    ) -> Result<CapturedInclude, Diagnostic> {
        self.capture_hooks(request, || {}, after_read)
    }
    fn capture_hooks(
        &self,
        request: &GeneratedSourceRequest,
        before_open: impl FnOnce(),
        after_read: impl FnOnce(),
    ) -> Result<CapturedInclude, Diagnostic> {
        let work = || -> Result<(Vec<u8>, IncludeLocation), Diagnostic> {
            if !self.authorities.matches_request(request) {
                return Err(fault(
                    "LRR-E-SOURCE-PATH",
                    "stale or foreign include capture request",
                ));
            }
            self.verify_roots()?;
            if let IncludeRootKind::BuildOutput { .. } = request.root_kind() {
                let tree = self.outputs.get(&request.root()).ok_or_else(|| {
                    fault(
                        "LRR-E-GENERATED-SOURCE",
                        "prepared output snapshot is unavailable",
                    )
                })?;
                let blob = tree.files().get(request.path()).ok_or_else(|| {
                    fault(
                        "LRR-E-GENERATED-SOURCE",
                        "include is absent from prepared output snapshot",
                    )
                })?;
                return Ok((blob.bytes().to_vec(), request.included_location()));
            }
            let physical = self.resolve_physical(request)?;
            let actual_root = self
                .authorities
                .roots()
                .filter(|r| physical.starts_with(&r.path))
                .max_by_key(|r| r.path.components().count())
                .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "canonical include has no authority"))?;
            let path =
                SourcePath::new(physical.strip_prefix(&actual_root.path).map_err(|_| {
                    fault("LRR-E-SOURCE-PATH", "canonical include prefix differs")
                })?)?;
            if let IncludeRootKind::BuildOutput { .. } = actual_root.kind {
                let tree = self.outputs.get(&actual_root.identity).ok_or_else(|| {
                    fault(
                        "LRR-E-GENERATED-SOURCE",
                        "canonical include output snapshot is unavailable",
                    )
                })?;
                let blob = tree.files().get(&path).ok_or_else(|| {
                    fault(
                        "LRR-E-GENERATED-SOURCE",
                        "canonical include is absent from output snapshot",
                    )
                })?;
                if self.resolve_physical(request)? != physical {
                    return Err(fault(
                        "LRR-E-SNAPSHOT-RACE",
                        "include alias changed during output selection",
                    ));
                }
                return Ok((
                    blob.bytes().to_vec(),
                    IncludeLocation {
                        unit: request.unit(),
                        root: actual_root.identity,
                        path,
                        origin: request.include_origin(),
                    },
                ));
            }
            let dir = self.source_dirs.get(&actual_root.identity).ok_or_else(|| {
                fault("LRR-E-SOURCE-PATH", "resolved source capability is absent")
            })?;
            let canonical = std::path::PathBuf::from(path.as_str());
            let relative = canonical.as_path();
            let before = dir
                .metadata(relative)
                .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
            if !before.is_file() {
                return Err(fault(
                    "LRR-E-SOURCE-IO",
                    "included source is not a regular file",
                ));
            }
            if before.len() > MAX_SOURCE_FILE as u64 {
                return Err(fault("LRR-E-SOURCE-LIMIT", "included source exceeds 8 MiB"));
            }
            before_open();
            use cap_std::fs::OpenOptionsExt as _;
            let flags = i32::try_from(rustix::fs::OFlags::NONBLOCK.bits())
                .map_err(|_| fault("LRR-E-SOURCE-IO", "nonblocking flag encoding failed"))?;
            let mut options = cap_std::fs::OpenOptions::new();
            options.read(true).custom_flags(flags);
            let mut file = dir
                .open_with(relative, &options)
                .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
            let stamp = Stamp::of(&before);
            if stamp
                != Stamp::of(
                    &file
                        .metadata()
                        .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?,
                )
            {
                return Err(fault("LRR-E-SNAPSHOT-RACE", "include changed before read"));
            }
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 16 * 1024];
            loop {
                let n = file
                    .read(&mut buffer)
                    .map_err(|e| fault("LRR-E-SOURCE-IO", e.to_string()))?;
                if n == 0 {
                    break;
                }
                if bytes
                    .len()
                    .checked_add(n)
                    .filter(|n| *n <= MAX_SOURCE_FILE)
                    .is_none()
                {
                    return Err(fault(
                        "LRR-E-SOURCE-LIMIT",
                        "included source grew past 8 MiB",
                    ));
                }
                bytes.try_reserve(n).map_err(|_| {
                    fault("LRR-E-SOURCE-LIMIT", "included source allocation failed")
                })?;
                bytes.extend_from_slice(&buffer[..n]);
            }
            after_read();
            let after = dir.metadata(relative).map_err(|_| {
                fault(
                    "LRR-E-SNAPSHOT-RACE",
                    "included source disappeared after read",
                )
            })?;
            if stamp != Stamp::of(&after)
                || stamp
                    != Stamp::of(&file.metadata().map_err(|_| {
                        fault("LRR-E-SNAPSHOT-RACE", "include metadata unavailable")
                    })?)
                || dir.canonicalize(relative).ok().as_ref() != Some(&canonical)
                || self.resolve_physical(request).ok().as_ref() != Some(&physical)
                || bytes.len() as u64 != before.len()
            {
                return Err(fault(
                    "LRR-E-SNAPSHOT-RACE",
                    "included source changed during capture",
                ));
            }
            Ok((
                bytes,
                IncludeLocation {
                    unit: request.unit(),
                    root: actual_root.identity,
                    path,
                    origin: request.include_origin(),
                },
            ))
        };
        let (bytes, location) = work().map_err(|error| {
            fault(
                error.code,
                format!(
                    "{}; caller {:?} (origin {}), include {:?} (origin {})",
                    error.message,
                    request
                        .from()
                        .path
                        .as_str()
                        .chars()
                        .take(256)
                        .collect::<String>(),
                    Digest(request.from().origin.0),
                    request
                        .original_target()
                        .to_string_lossy()
                        .chars()
                        .take(256)
                        .collect::<String>(),
                    Digest(request.include_origin().0)
                ),
            )
        })?;
        self.verify_roots().map_err(|error| {
            fault(
                error.code,
                format!(
                    "{}; caller origin {}; include origin {}",
                    error.message,
                    Digest(request.from().origin.0),
                    Digest(request.include_origin().0)
                ),
            )
        })?;
        let blob = match request.kind() {
            IncludeKind::Bytes => SourceBlob::included_bytes(bytes),
            _ => SourceBlob::new(bytes),
        }
        .map_err(|error| {
            fault(
                error.code,
                format!(
                    "{}; caller origin {}, include origin {}",
                    error.message,
                    Digest(request.from().origin.0),
                    Digest(request.include_origin().0)
                ),
            )
        })?;
        let virtual_location = request.included_location();
        let mut e = StableEncoder::new("lrr/v2/captured-include-addresses");
        e.digest(request.identity());
        e.digest(location.root);
        e.string(location.path.as_str());
        e.digest(virtual_location.root);
        e.string(virtual_location.path.as_str());
        e.bytes(&virtual_location.origin.0);
        e.digest(blob.content());
        Ok(CapturedInclude {
            request: request.identity(),
            identity: e.finish(),
            location,
            virtual_location,
            original: request.original_target().to_owned(),
            kind: request.kind(),
            blob,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expand::compile_input_identity::descriptor, source::include_paths::IncludeRoot,
        syntax::token_wire::Key,
    };
    #[test]
    fn include_capture_rejects_cross_root_alias_retarget_during_read() {
        let temp = std::env::temp_dir().join(format!(
            "lrr-include-alias-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&temp).unwrap();
        let temp = std::fs::canonicalize(temp).unwrap();
        for name in ["a", "b", "outside"] {
            std::fs::create_dir(temp.join(name)).unwrap();
        }
        std::fs::write(temp.join("b/file.rs"), "approved").unwrap();
        std::fs::write(temp.join("outside/file.rs"), "denied").unwrap();
        let alias = temp.join("a/link.rs");
        std::os::unix::fs::symlink("../b/file.rs", &alias).unwrap();
        let unit = Digest([1; 32]);
        let root = Digest([2; 32]);
        let authorities = IncludeAuthorities::new(
            unit,
            Digest([3; 32]),
            [
                IncludeRoot {
                    identity: root,
                    path: temp.join("a"),
                    kind: IncludeRootKind::Source,
                },
                IncludeRoot {
                    identity: Digest([6; 32]),
                    path: temp.join("b"),
                    kind: IncludeRootKind::Source,
                },
            ],
        )
        .unwrap();
        let from = IncludeLocation {
            unit,
            root,
            path: SourcePath::new("root.rs").unwrap(),
            origin: Key([4; 32]),
        };
        let request = authorities
            .request(
                &descriptor("include").unwrap().unwrap(),
                &from,
                "link.rs",
                Key([5; 32]),
            )
            .unwrap();
        let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
        let error = capture
            .capture_with(&request, || {
                std::fs::remove_file(&alias).unwrap();
                std::os::unix::fs::symlink("../outside/file.rs", &alias).unwrap();
            })
            .err()
            .unwrap();
        assert_eq!(error.code, "LRR-E-SNAPSHOT-RACE");
        assert!(error.message.contains(&Digest([4; 32]).hex()));
        assert!(error.message.contains(&Digest([5; 32]).hex()));
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn include_capture_rejects_actual_file_replacement_during_read() {
        let temp = std::env::temp_dir().join(format!(
            "lrr-include-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&temp).unwrap();
        let temp = std::fs::canonicalize(temp).unwrap();
        let unit = Digest([1; 32]);
        let root = Digest([2; 32]);
        std::fs::write(temp.join("file.rs"), "old").unwrap();
        let authorities = IncludeAuthorities::new(
            unit,
            Digest([3; 32]),
            [IncludeRoot {
                identity: root,
                path: temp.clone(),
                kind: IncludeRootKind::Source,
            }],
        )
        .unwrap();
        let from = IncludeLocation {
            unit,
            root,
            path: SourcePath::new("root.rs").unwrap(),
            origin: Key([4; 32]),
        };
        let request = authorities
            .request(
                &descriptor("include").unwrap().unwrap(),
                &from,
                "file.rs",
                Key([5; 32]),
            )
            .unwrap();
        let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
        let error = capture
            .capture_with(&request, || {
                std::fs::write(temp.join("replacement.rs"), "new").unwrap();
                std::fs::rename(temp.join("replacement.rs"), temp.join("file.rs")).unwrap();
            })
            .err()
            .unwrap();
        assert_eq!(error.code, "LRR-E-SNAPSHOT-RACE");
        assert!(error.message.contains(&Digest([4; 32]).hex()));
        assert!(error.message.contains(&Digest([5; 32]).hex()));
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn include_root_open_rejects_actual_directory_replacement() {
        let temp = std::env::temp_dir().join(format!(
            "lrr-root-open-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&temp).unwrap();
        let temp = std::fs::canonicalize(temp).unwrap();
        let old = temp.with_extension("old");
        let authorities = IncludeAuthorities::new(
            Digest([1; 32]),
            Digest([3; 32]),
            [IncludeRoot {
                identity: Digest([2; 32]),
                path: temp.clone(),
                kind: IncludeRootKind::Source,
            }],
        )
        .unwrap();
        let error = SourceCapture::open_with(authorities, BTreeMap::new(), |path| {
            std::fs::rename(path, &old).unwrap();
            std::fs::create_dir(path).unwrap();
        })
        .err()
        .unwrap();
        assert_eq!(error.code, "LRR-E-SNAPSHOT-RACE");
        std::fs::remove_dir_all(temp).unwrap();
        std::fs::remove_dir_all(old).unwrap();
    }

    #[test]
    fn include_file_open_rejects_fifo_swap_without_blocking() {
        let temp = std::env::temp_dir().join(format!(
            "lrr-fifo-open-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&temp).unwrap();
        let temp = std::fs::canonicalize(temp).unwrap();
        std::fs::write(temp.join("file.rs"), "before").unwrap();
        let unit = Digest([1; 32]);
        let root = Digest([2; 32]);
        let authorities = IncludeAuthorities::new(
            unit,
            Digest([3; 32]),
            [IncludeRoot {
                identity: root,
                path: temp.clone(),
                kind: IncludeRootKind::Source,
            }],
        )
        .unwrap();
        let from = IncludeLocation {
            unit,
            root,
            path: SourcePath::new("root.rs").unwrap(),
            origin: Key([4; 32]),
        };
        let request = authorities
            .request(
                &descriptor("include").unwrap().unwrap(),
                &from,
                "file.rs",
                Key([5; 32]),
            )
            .unwrap();
        let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
        let started = std::time::Instant::now();
        let error = capture
            .capture_hooks(
                &request,
                || {
                    let dir = &capture.source_dirs[&root];
                    dir.remove_file("file.rs").unwrap();
                    assert!(
                        std::process::Command::new("mkfifo")
                            .arg(temp.join("file.rs"))
                            .status()
                            .unwrap()
                            .success()
                    );
                },
                || {},
            )
            .err()
            .unwrap();
        assert_eq!(error.code, "LRR-E-SNAPSHOT-RACE");
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        std::fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn include_source_set_refuses_actual_change_between_staging_and_revalidation() {
        let temp = std::env::temp_dir().join(format!(
            "lrr-include-set-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&temp).unwrap();
        let temp = std::fs::canonicalize(temp).unwrap();
        std::fs::write(temp.join("one.rs"), b"31").unwrap();
        let unit = Digest([81; 32]);
        let root = Digest([82; 32]);
        let authorities = IncludeAuthorities::new(
            unit,
            Digest([83; 32]),
            vec![IncludeRoot {
                identity: root,
                path: temp.clone(),
                kind: IncludeRootKind::Source,
            }],
        )
        .unwrap();
        let from = IncludeLocation {
            unit,
            root,
            path: SourcePath::new("main.rs").unwrap(),
            origin: Key([84; 32]),
        };
        let request = authorities
            .request(
                &descriptor("include").unwrap().unwrap(),
                &from,
                "one.rs",
                Key([85; 32]),
            )
            .unwrap();
        let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
        let before = capture.capture_set(std::slice::from_ref(&request)).unwrap();
        let error = capture
            .capture_set_with(std::slice::from_ref(&request), || {
                std::fs::write(temp.join("one.rs"), b"73").unwrap()
            })
            .err()
            .unwrap();
        assert_eq!(error.code, "LRR-E-SNAPSHOT-RACE");
        assert_eq!(before.records()[&request.identity()].blob().bytes(), b"31");
        let stable = capture.capture_set(std::slice::from_ref(&request)).unwrap();
        assert_eq!(stable.records()[&request.identity()].blob().bytes(), b"73");
        std::fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn module_capture_rejects_candidate_membership_race_before_returning_a_record() {
        let temp = std::env::temp_dir().join(format!(
            "lrr-module-membership-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&temp).unwrap();
        let temp = std::fs::canonicalize(temp).unwrap();
        std::fs::write(temp.join("child.rs"), "pub fn child(){}").unwrap();
        let unit = Digest([1; 32]);
        let root = Digest([2; 32]);
        let authorities = IncludeAuthorities::new(
            unit,
            Digest([3; 32]),
            [IncludeRoot {
                identity: root,
                path: temp.clone(),
                kind: IncludeRootKind::Source,
            }],
        )
        .unwrap();
        let from = IncludeLocation {
            unit,
            root,
            path: SourcePath::new("main.rs").unwrap(),
            origin: Key([4; 32]),
        };
        let request = authorities
            .module_request(
                &from,
                std::path::Path::new(""),
                crate::syntax::modules::ModuleRequest {
                    module_path: vec!["child".into()],
                    directory: vec![],
                    file_relative: false,
                    name: "child".into(),
                    path_override: None,
                },
                Digest([5; 32]),
                Key([6; 32]),
            )
            .unwrap();
        let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
        let error = capture
            .capture_module_with(&request, || {
                std::fs::create_dir(temp.join("child")).unwrap();
                std::fs::write(temp.join("child/mod.rs"), "pub fn other(){}").unwrap();
            })
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-SNAPSHOT-RACE");
        std::fs::remove_dir_all(temp).unwrap();
    }
}
