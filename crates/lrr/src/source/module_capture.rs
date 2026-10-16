//! Frozen module candidate requests and presence/absence evidence. No filesystem effects.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    source::{
        include_paths::{GeneratedSourceRequest, IncludeAuthorities, IncludeKind, IncludeLocation},
        included::CapturedInclude,
    },
    syntax::{modules::ModuleRequest, token_wire::Key},
};
use std::path::{Path, PathBuf};
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ModuleCaptureRequest {
    identity: Digest,
    producer: Digest,
    declaration: ModuleRequest,
    candidates: Vec<GeneratedSourceRequest>,
    directory: PathBuf,
}
impl ModuleCaptureRequest {
    pub fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .directory
            .capacity()
            .checked_add(std::mem::size_of::<Self>())
            .and_then(|n| {
                n.checked_add(
                    self.candidates
                        .capacity()
                        .checked_mul(std::mem::size_of::<GeneratedSourceRequest>())?,
                )
            })
            .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "module request metadata overflow"))?;
        bytes = bytes
            .checked_add(self.declaration.name.capacity())
            .and_then(|n| {
                n.checked_add(
                    self.declaration
                        .path_override
                        .as_ref()
                        .map_or(0, String::capacity),
                )
            })
            .and_then(|n| {
                n.checked_add(
                    self.declaration
                        .module_path
                        .capacity()
                        .checked_add(self.declaration.directory.capacity())?
                        .checked_mul(std::mem::size_of::<String>())?,
                )
            })
            .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "module declaration metadata overflow"))?;
        for part in self
            .declaration
            .module_path
            .iter()
            .chain(&self.declaration.directory)
        {
            bytes = bytes
                .checked_add(part.capacity())
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "module declaration metadata limit"))?;
        }
        for candidate in &self.candidates {
            bytes = bytes
                .checked_add(candidate.metadata_bytes().ok_or_else(|| {
                    fault("LRR-E-SOURCE-LIMIT", "module candidate metadata overflow")
                })?)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "module request metadata limit"))?;
        }
        Ok(bytes)
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn producer(&self) -> Digest {
        self.producer
    }
    pub fn candidates(&self) -> &[GeneratedSourceRequest] {
        &self.candidates
    }
    pub fn declaration(&self) -> &ModuleRequest {
        &self.declaration
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}
impl IncludeAuthorities {
    /// Route candidate spellings through the explicit source/build-output authorities.
    pub fn module_request(
        &self,
        from: &IncludeLocation,
        directory: &Path,
        module: ModuleRequest,
        producer: Digest,
        origin: Key,
    ) -> Result<ModuleCaptureRequest, Diagnostic> {
        self.module_request_with_layout(
            from,
            directory,
            module,
            producer,
            origin,
            &Default::default(),
        )
    }
    pub(crate) fn module_request_with_layout(
        &self,
        from: &IncludeLocation,
        directory: &Path,
        module: ModuleRequest,
        producer: Digest,
        origin: Key,
        layout: &crate::syntax::modules::InlineModuleLayout,
    ) -> Result<ModuleCaptureRequest, Diagnostic> {
        if directory.as_os_str().len() > 64 * 1024
            || module
                .path_override
                .as_ref()
                .is_some_and(|p| p.len() > 64 * 1024)
            || module.module_path.len() > 256
            || module.directory.len() > 256
            || module.name.len() > 64 * 1024
            || module
                .module_path
                .iter()
                .chain(&module.directory)
                .any(|s| s.len() > 64 * 1024)
        {
            return Err(fault("LRR-E-SOURCE-LIMIT", "module request metadata limit"));
        }
        let root = self
            .root(from.root)
            .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "module source authority absent"))?;
        let mut effective = module.clone();
        if !layout.directory.is_empty() {
            let count = layout
                .directory
                .len()
                .checked_add(effective.directory.len())
                .filter(|n| *n <= 256)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-SOURCE-LIMIT",
                        "combined inline module directory depth limit",
                    )
                })?;
            layout
                .directory
                .iter()
                .chain(&effective.directory)
                .try_fold(0usize, |n, part| {
                    n.checked_add(part.len()).and_then(|n| n.checked_add(64))
                })
                .filter(|n| *n <= 64 * 1024)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-SOURCE-LIMIT",
                        "combined inline module directory metadata limit",
                    )
                })?;
            let mut combined = Vec::new();
            combined.try_reserve_exact(count).map_err(|_| {
                fault(
                    "LRR-E-SOURCE-LIMIT",
                    "combined inline module directory allocation",
                )
            })?;
            combined.extend(layout.directory.iter().cloned());
            combined.extend(effective.directory);
            effective.directory = combined;
            effective.file_relative = layout.file_relative;
        }
        let (paths, directory) =
            super::graph::module_candidate_spellings(&from.path, directory, effective)?;
        let mut candidates = Vec::with_capacity(paths.len());
        for path in paths {
            let target = root.path.join(path);
            let target = target
                .to_str()
                .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "module path must be UTF-8"))?;
            candidates.push(self.request_kind_with_base(
                IncludeKind::Syntax,
                producer,
                from,
                target,
                origin,
                None,
            )?);
        }
        let mut identity = StableEncoder::new("lrr/v2/module-capture-request");
        identity.digest(self.fingerprint());
        identity.digest(producer);
        identity.sequence(&module.module_path, |e, s| e.string(s));
        identity.sequence(&module.directory, |e, s| e.string(s));
        identity.bool(module.file_relative);
        identity.string(&module.name);
        identity.option(&module.path_override, |e, s| e.string(s));
        identity.sequence(&candidates, |e, c| e.digest(c.identity()));
        identity.string(
            directory
                .to_str()
                .ok_or_else(|| fault("LRR-E-SOURCE-PATH", "module directory must be UTF-8"))?,
        );
        let request = ModuleCaptureRequest {
            identity: identity.finish(),
            producer,
            declaration: module,
            candidates,
            directory,
        };
        request.metadata_bytes()?;
        Ok(request)
    }
}
#[derive(Debug, Clone)]
pub enum ModuleObservation {
    Absent,
    Present(Box<CapturedInclude>),
    NonFile { identity: Digest },
}
#[derive(Debug, Clone)]
pub struct CapturedModule {
    pub(crate) request: Digest,
    pub(crate) identity: Digest,
    pub(crate) observations: Vec<ModuleObservation>,
    pub(crate) directory: PathBuf,
}
impl CapturedModule {
    pub fn validate(&self, request: &ModuleCaptureRequest) -> Result<(), Diagnostic> {
        request.metadata_bytes()?;
        self.metadata_bytes()?;
        if self.request != request.identity()
            || self.directory != request.directory()
            || self.observations.len() != request.candidates().len()
        {
            return Err(fault(
                "LRR-E-SOURCE-PATH",
                "captured module request/layout mismatch",
            ));
        }
        for (candidate, observation) in request.candidates().iter().zip(&self.observations) {
            match observation {
                ModuleObservation::Absent => {}
                ModuleObservation::NonFile { .. } => {}
                ModuleObservation::Present(record) => {
                    if record.request() != candidate.identity()
                        || record.kind() != IncludeKind::Syntax
                        || record.location().unit != candidate.unit()
                    {
                        return Err(fault(
                            "LRR-E-SOURCE-PATH",
                            "captured module candidate mismatch",
                        ));
                    }
                }
            }
        }
        // StableEncoder::sequence prefixes its element count; reproduce that exact format below.
        let mut checked = StableEncoder::new("lrr/v2/captured-module-candidates");
        checked.digest(request.identity());
        checked.sequence(&self.observations, |e, o| match o {
            ModuleObservation::Absent => e.u8(0),
            ModuleObservation::NonFile { identity } => {
                e.u8(2);
                e.digest(*identity);
            }
            ModuleObservation::Present(record) => {
                e.u8(1);
                e.digest(record.identity());
            }
        });
        if checked.finish() != self.identity {
            return Err(fault(
                "LRR-E-SOURCE-PATH",
                "captured module identity mismatch",
            ));
        }
        Ok(())
    }

    pub fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .directory
            .capacity()
            .checked_add(std::mem::size_of::<Self>())
            .and_then(|n| {
                n.checked_add(
                    self.observations
                        .capacity()
                        .checked_mul(std::mem::size_of::<ModuleObservation>())?,
                )
            })
            .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "captured module metadata overflow"))?;
        for observation in &self.observations {
            if let ModuleObservation::Present(record) = observation {
                bytes = bytes
                    .checked_add(record.metadata_bytes().ok_or_else(|| {
                        fault("LRR-E-SOURCE-LIMIT", "module source metadata overflow")
                    })?)
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "captured module metadata limit"))?;
            }
        }
        Ok(bytes)
    }
    pub fn request(&self) -> Digest {
        self.request
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn observations(&self) -> &[ModuleObservation] {
        &self.observations
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn selected(&self) -> Result<&CapturedInclude, Diagnostic> {
        let mut present = self
            .observations
            .iter()
            .filter(|o| !matches!(o, ModuleObservation::Absent));
        let selected = present
            .next()
            .ok_or_else(|| fault("LRR-E-SOURCE-MISSING", "module has no captured source file"))?;
        if present.next().is_some() {
            return Err(fault(
                "LRR-E-MODULE-AMBIGUOUS",
                "both module source candidates exist",
            ));
        }
        match selected {
            ModuleObservation::Present(record) => Ok(record.as_ref()),
            ModuleObservation::NonFile { .. } => Err(fault(
                "LRR-E-SOURCE-IO",
                "module source is not a regular file",
            )),
            ModuleObservation::Absent => {
                Err(fault("LRR-E-SOURCE-MISSING", "module source is absent"))
            }
        }
    }
}

/// Consumer-held immutable input lease keeps all candidate bytes in cache accounting.
pub struct AdoptedModuleCapture {
    input_identity: Digest,
    captured: std::sync::Arc<CapturedModule>,
    _input: std::sync::Arc<crate::db::dependency::InputValue>,
}
impl AdoptedModuleCapture {
    pub(crate) fn new(
        input: std::sync::Arc<crate::db::dependency::InputValue>,
    ) -> Result<Self, Diagnostic> {
        let crate::db::dependency::InputValue::ModuleRecord {
            input: identity,
            captured,
        } = input.as_ref()
        else {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "adopted module input variant differs",
            ));
        };
        Ok(Self {
            input_identity: *identity,
            captured: captured.clone(),
            _input: input,
        })
    }
    pub fn input_identity(&self) -> Digest {
        self.input_identity
    }
    pub fn record(&self) -> &CapturedModule {
        &self.captured
    }
    /// Existing immutable allocations retained by this lease; accounting only.
    pub fn record_root(&self) -> &std::sync::Arc<CapturedModule> {
        &self.captured
    }
    pub fn input_root(&self) -> &std::sync::Arc<crate::db::dependency::InputValue> {
        &self._input
    }
}
