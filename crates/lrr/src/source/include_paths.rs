//! Pure include authority routing. Capture/canonicalization remain coordinator effects.
use super::snapshot::SourcePath;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    expand::compile_input_identity::{CompileInputDescriptor, CompileInputKind},
    syntax::token_wire::Key,
};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IncludeRootKind {
    Source,
    BuildOutput { producer: Digest },
}
#[derive(Debug, Clone)]
pub struct IncludeRoot {
    pub identity: Digest,
    pub path: PathBuf,
    pub kind: IncludeRootKind,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IncludeLocation {
    pub unit: Digest,
    pub root: Digest,
    pub path: SourcePath,
    pub origin: Key,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IncludeKind {
    Syntax,
    Utf8,
    Bytes,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GeneratedSourceRequest {
    authority: Digest,
    identity: Digest,
    unit: Digest,
    generation: Digest,
    root: Digest,
    root_kind: IncludeRootKind,
    path: SourcePath,
    from: IncludeLocation,
    include_origin: Key,
    kind: IncludeKind,
    /// Preserve spelling for capability canonicalization; lexical routing is not a read grant.
    original_target: PathBuf,
}
impl GeneratedSourceRequest {
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(self.path.as_str().len())?
            .checked_add(self.from.path.as_str().len())?
            .checked_add(self.original_target.capacity())
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn unit(&self) -> Digest {
        self.unit
    }
    pub fn generation(&self) -> Digest {
        self.generation
    }
    pub fn root(&self) -> Digest {
        self.root
    }
    pub fn root_kind(&self) -> IncludeRootKind {
        self.root_kind
    }
    pub fn path(&self) -> &SourcePath {
        &self.path
    }
    pub fn from(&self) -> &IncludeLocation {
        &self.from
    }
    pub fn include_origin(&self) -> Key {
        self.include_origin
    }
    pub fn kind(&self) -> IncludeKind {
        self.kind
    }
    pub fn original_target(&self) -> &Path {
        &self.original_target
    }
    pub fn included_location(&self) -> IncludeLocation {
        IncludeLocation {
            unit: self.unit,
            root: self.root,
            path: self.path.clone(),
            origin: self.include_origin,
        }
    }
}
pub struct IncludeAuthorities {
    unit: Digest,
    generation: Digest,
    roots: BTreeMap<Digest, IncludeRoot>,
    fingerprint: Digest,
}
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-PATH", message.into())
}
fn normalize_absolute(path: &Path) -> Result<PathBuf, Diagnostic> {
    let text = path
        .to_str()
        .ok_or_else(|| fault("include path must be UTF-8"))?;
    if text.len() > 64 * 1024 || text.as_bytes().contains(&0) || !path.is_absolute() {
        return Err(fault("invalid absolute include path"));
    }
    let mut result = PathBuf::from("/");
    let mut depth = 0usize;
    for part in path.components() {
        match part {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(value) => {
                depth += 1;
                if depth > 256 {
                    return Err(fault("include path depth limit"));
                }
                result.push(value);
            }
            Component::ParentDir => {
                if depth != 0 {
                    result.pop();
                    depth -= 1;
                }
            }
            Component::Prefix(_) => return Err(fault("unsupported include path prefix")),
        }
    }
    Ok(result)
}
impl IncludeAuthorities {
    pub(crate) fn unique_source_root(&self) -> Result<Digest, Diagnostic> {
        let mut roots = self
            .roots
            .values()
            .filter(|root| matches!(root.kind, IncludeRootKind::Source));
        let root = roots
            .next()
            .ok_or_else(|| fault("include caller source root absent"))?;
        if roots.next().is_some() {
            return Err(fault(
                "include caller requires an explicit unique source root",
            ));
        }
        Ok(root.identity)
    }

    pub fn unit(&self) -> Digest {
        self.unit
    }

    pub fn roots(&self) -> impl Iterator<Item = &IncludeRoot> {
        self.roots.values()
    }
    pub fn matches_request(&self, request: &GeneratedSourceRequest) -> bool {
        request.authority == self.fingerprint
            && request.unit == self.unit
            && request.generation == self.generation
    }
    /// Preparation supplies canonical absolute source/build-output authorities for this unit.
    pub fn new(
        unit: Digest,
        generation: Digest,
        roots: impl IntoIterator<Item = IncludeRoot>,
    ) -> Result<Self, Diagnostic> {
        let mut retained = BTreeMap::new();
        let mut paths = std::collections::BTreeSet::new();
        let mut bytes = 0usize;
        for mut root in roots {
            if retained.len() >= 4096 {
                return Err(fault("include authority count limit"));
            }
            root.path = normalize_absolute(&root.path)?;
            bytes = bytes
                .checked_add(root.path.as_os_str().len() + 256)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("include authority metadata limit"))?;
            if !paths.insert(root.path.clone()) || retained.insert(root.identity, root).is_some() {
                return Err(fault("duplicate include authority identity or path"));
            }
        }
        let mut e = StableEncoder::new("lrr/v2/include-authorities");
        e.digest(unit);
        e.digest(generation);
        for (id, root) in &retained {
            e.digest(*id);
            e.string(
                root.path
                    .to_str()
                    .ok_or_else(|| fault("include authority encoding"))?,
            );
            match root.kind {
                IncludeRootKind::Source => e.u8(0),
                IncludeRootKind::BuildOutput { producer } => {
                    e.u8(1);
                    e.digest(producer);
                }
            }
        }
        Ok(Self {
            unit,
            generation,
            roots: retained,
            fingerprint: e.finish(),
        })
    }
    pub fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub fn root(&self, id: Digest) -> Option<&IncludeRoot> {
        self.roots.get(&id)
    }
    pub fn request(
        &self,
        descriptor: &CompileInputDescriptor,
        from: &IncludeLocation,
        value: &str,
        include_origin: Key,
    ) -> Result<GeneratedSourceRequest, Diagnostic> {
        self.request_with_base(descriptor, from, value, include_origin, None)
    }
    pub fn request_from_included(
        &self,
        descriptor: &CompileInputDescriptor,
        view: &crate::source::include_view::IncludedSourceView,
        value: &str,
        include_origin: Key,
    ) -> Result<GeneratedSourceRequest, Diagnostic> {
        if !view.matches_authorities(self) {
            return Err(fault(
                "included file base belongs to another source authority",
            ));
        }
        self.request_with_base(
            descriptor,
            view.location(),
            value,
            include_origin,
            Some(view.original_path()),
        )
    }
    fn request_with_base(
        &self,
        descriptor: &CompileInputDescriptor,
        from: &IncludeLocation,
        value: &str,
        include_origin: Key,
        raw_file: Option<&Path>,
    ) -> Result<GeneratedSourceRequest, Diagnostic> {
        let kind = match descriptor.kind() {
            CompileInputKind::Include => IncludeKind::Syntax,
            CompileInputKind::IncludeStr => IncludeKind::Utf8,
            CompileInputKind::IncludeBytes => IncludeKind::Bytes,
            _ => return Err(fault("selected builtin is not an include")),
        };
        self.request_kind_with_base(
            kind,
            descriptor.definition_identity(),
            from,
            value,
            include_origin,
            raw_file,
        )
    }
    pub(crate) fn request_kind_with_base(
        &self,
        kind: IncludeKind,
        producer: Digest,
        from: &IncludeLocation,
        value: &str,
        include_origin: Key,
        raw_file: Option<&Path>,
    ) -> Result<GeneratedSourceRequest, Diagnostic> {
        let route = || -> Result<GeneratedSourceRequest, Diagnostic> {
            if from.path.as_str().len() > 64 * 1024 {
                return Err(fault("include caller path limit"));
            }
            if from.unit != self.unit {
                return Err(fault("include caller belongs to another unit"));
            }
            if value.is_empty() || value.len() > 64 * 1024 || value.as_bytes().contains(&0) {
                return Err(fault("invalid include path string"));
            }
            let parent = self
                .roots
                .get(&from.root)
                .ok_or_else(|| fault("include caller source authority is absent"))?;
            let file = raw_file
                .map(Path::to_owned)
                .unwrap_or_else(|| parent.path.join(from.path.as_str()));
            if file
                .to_str()
                .is_none_or(|s| s.len() > 64 * 1024 || s.contains('\0'))
                || !file.is_absolute()
            {
                return Err(fault("invalid included raw file base"));
            }
            let original_target = if Path::new(value).is_absolute() {
                PathBuf::from(value)
            } else {
                file.parent()
                    .ok_or_else(|| fault("include caller directory is absent"))?
                    .join(value)
            };
            let target = normalize_absolute(&original_target)?;
            let root = self
                .roots
                .values()
                .filter(|r| target.starts_with(&r.path))
                .max_by_key(|r| r.path.components().count())
                .ok_or_else(|| fault("include target is outside source authority"))?;
            let path = SourcePath::new(
                target
                    .strip_prefix(&root.path)
                    .map_err(|_| fault("include authority prefix mismatch"))?,
            )?;
            let mut e = StableEncoder::new("lrr/v2/generated-source-request-raw-base");
            e.digest(self.fingerprint);
            e.digest(producer);
            e.digest(from.root);
            e.string(from.path.as_str());
            e.bytes(&from.origin.0);
            e.bytes(&include_origin.0);
            e.string(value);
            e.string(
                original_target
                    .to_str()
                    .ok_or_else(|| fault("include target encoding"))?,
            );
            e.digest(root.identity);
            e.string(path.as_str());
            Ok(GeneratedSourceRequest {
                authority: self.fingerprint,
                identity: e.finish(),
                unit: self.unit,
                generation: self.generation,
                root: root.identity,
                root_kind: root.kind,
                path,
                from: from.clone(),
                include_origin,
                kind,
                original_target,
            })
        };
        route().map_err(|error| {
            fault(format!(
                "{}; caller {} (origin {}), include {:?} (origin {})",
                error.message,
                from.path.as_str().chars().take(256).collect::<String>(),
                Digest(from.origin.0),
                value.chars().take(256).collect::<String>(),
                Digest(include_origin.0)
            ))
        })
    }
}
