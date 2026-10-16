//! Ordinary physical module facts from retained syntax; no capture or generated identity.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    source::{
        changes::PreparedSourceView,
        manifest::{DefKey, ItemKind},
    },
    syntax::{
        modules::ModuleRequest,
        outline::{ProducerKind, ScopeKey, TextualEventKind},
        spans::SourceRange,
    },
};
pub struct SourceModuleDeclaration {
    source: PreparedSourceView,
    definition: DefKey,
    scope: ScopeKey,
    producer: Digest,
    request: ModuleRequest,
    range: SourceRange,
    bytes: usize,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CATALOG-INCOMPLETE", message.into())
}
impl SourceModuleDeclaration {
    pub fn from_captured(source: &PreparedSourceView) -> Result<Vec<Self>, Diagnostic> {
        let declarations = source.captured_external_module_declarations()?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(declarations.len())
            .map_err(|_| fault("source module fact allocation"))?;
        let mut total = 0usize;
        for declaration in declarations {
            let request = declaration.request;
            let mut parent = source.outline().root_scope.module_path.clone();
            parent.extend(
                request.module_path[..request.module_path.len() - 1]
                    .iter()
                    .cloned(),
            );
            let item = source
                .outline()
                .items
                .iter()
                .find(|item| {
                    item.def_key.item_kind == ItemKind::Module
                        && item.scope.module_path == parent
                        && item.def_key.declared_name == request.name
                        && item.declaration_span == declaration.range
                })
                .ok_or_else(|| fault("source module syntax/header binding differs"))?;
            let mut scope = item.scope.clone();
            scope.module_path.push(request.name.clone());
            scope.owner = None;
            if scope.module_path.len() > 256 {
                return Err(fault("ordinary module scope depth limit"));
            }
            let mut encoder = StableEncoder::new("lrr/v2/source-module-input");
            encoder.digest(item.def_key.fingerprint());
            let producer = encoder.finish();
            if !source.outline().textual_events.iter().any(|event| {
                event.scope == scope
                    && matches!(event.kind, TextualEventKind::Producer {
                    identity, kind: ProducerKind::ModuleSource } if identity == producer)
            }) {
                return Err(fault("source module textual producer absent"));
            }
            let bytes = request
                .module_path
                .iter()
                .chain(&request.directory)
                .try_fold(512usize, |n, s| {
                    n.checked_add(s.len()).and_then(|n| n.checked_add(64))
                })
                .and_then(|n| n.checked_add(request.name.len()))
                .and_then(|n| n.checked_add(request.path_override.as_ref().map_or(0, String::len)))
                .and_then(|n| n.checked_add(scope.heap_bytes()))
                .and_then(|n| n.checked_add(item.scope.heap_bytes()))
                .ok_or_else(|| fault("source module fact size overflow"))?;
            total = total
                .checked_add(bytes)
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or_else(|| fault("source module fact metadata limit"))?;
            result.push(Self {
                source: source.share(),
                definition: item.def_key.clone(),
                scope,
                producer,
                request,
                range: declaration.range,
                bytes,
            });
        }
        Ok(result)
    }
    pub fn share(&self) -> Self {
        Self {
            source: self.source.share(),
            definition: self.definition.clone(),
            scope: self.scope.clone(),
            producer: self.producer,
            request: self.request.clone(),
            range: self.range,
            bytes: self.bytes,
        }
    }
    pub fn capture_origin(&self) -> crate::syntax::token_wire::Key {
        let mut e = StableEncoder::new("lrr/v2/ordinary-module-capture-origin");
        e.record(self);
        crate::syntax::token_wire::Key(e.finish().0)
    }
    pub fn capture_request(
        &self,
        authorities: &crate::source::include_paths::IncludeAuthorities,
        directory: &std::path::Path,
        compile_environment: Digest,
    ) -> Result<crate::db::poll::InputRequest, Diagnostic> {
        let from = crate::source::include_paths::IncludeLocation {
            unit: self.scope.crate_key,
            root: authorities.unique_source_root()?,
            path: self.source.selection().path.clone(),
            origin: self.capture_origin(),
        };
        Ok(crate::db::poll::InputRequest::CaptureModule {
            request: Box::new(authorities.module_request(
                &from,
                directory,
                self.request.clone(),
                self.producer,
                self.capture_origin(),
            )?),
            compile_environment,
        })
    }
    pub fn source(&self) -> &PreparedSourceView {
        &self.source
    }
    pub fn definition(&self) -> &DefKey {
        &self.definition
    }
    pub fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub fn producer(&self) -> Digest {
        self.producer
    }
    pub fn request(&self) -> &ModuleRequest {
        &self.request
    }
    pub fn range(&self) -> SourceRange {
        self.range
    }
    /// Actual retained row/heap upper bound, excluding the shared immutable
    /// source-state allocation. It includes both selected/source-location keys.
    pub fn retained_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.source
                    .selection()
                    .metadata_bytes()?
                    .checked_mul(2)
                    .ok_or_else(|| fault("source module retained key overflow"))?,
            )
            .and_then(|n| n.checked_add(self.definition.declared_name.capacity()))
            .and_then(|n| n.checked_add(self.request.name.capacity()))
            .and_then(|n| {
                n.checked_add(
                    self.request
                        .path_override
                        .as_ref()
                        .map_or(0, String::capacity),
                )
            })
            .ok_or_else(|| fault("source module retained metadata overflow"))?;
        for parts in [
            &self.definition.module_path,
            &self.scope.module_path,
            &self.request.module_path,
            &self.request.directory,
        ] {
            bytes = parts
                .capacity()
                .checked_mul(std::mem::size_of::<String>())
                .and_then(|n| bytes.checked_add(n))
                .ok_or_else(|| fault("source module retained vector overflow"))?;
            bytes = parts
                .iter()
                .try_fold(bytes, |n, part| n.checked_add(part.capacity()))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("source module retained metadata limit"))?;
        }
        Ok(bytes)
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
}
impl StableEncode for SourceModuleDeclaration {
    const DOMAIN: &'static str = "lrr/v2/captured-source-module-declaration";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.source.selection().location_identity());
        e.digest(
            crate::db::key::QueryKey::Outline(self.source.selection().outline.clone())
                .fingerprint(),
        );
        e.record(&self.definition);
        e.digest(self.producer);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.sequence(&self.scope.module_path, |e, part| e.string(part));
        e.sequence(&self.request.module_path, |e, part| e.string(part));
        e.sequence(&self.request.directory, |e, part| e.string(part));
        e.bool(self.request.file_relative);
        e.string(&self.request.name);
        e.option(&self.request.path_override, |e, path| e.string(path));
    }
}
