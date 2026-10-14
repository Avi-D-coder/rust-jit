//! Frozen child declarations and their generated parent. No capture or publication effects.
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    expand::{generated_items::GeneratedItems, identity::GeneratedItemKey},
    frontend::generated_headers::GeneratedModuleSource,
    source::{changes::PreparedSourceView, module_capture::AdoptedModuleCapture},
    syntax::{outline::Outline, parse::ParsedFile, token_wire::SpanRecord},
};
use std::sync::Arc;

pub struct CapturedModuleSourceView {
    capture: Arc<AdoptedModuleCapture>,
    parent: GeneratedItems,
    demand: GeneratedModuleSource,
    caller: PreparedSourceView,
    invocation_base: SpanRecord,
    estimated_bytes: usize,
    parsed: Arc<ParsedFile>,
    outline: Arc<Outline>,
}
impl CapturedModuleSourceView {
    pub(crate) fn new(
        capture: Arc<AdoptedModuleCapture>,
        parent: GeneratedItems,
        demand: GeneratedModuleSource,
        caller: PreparedSourceView,
        invocation_base: SpanRecord,
    ) -> Result<Self, Diagnostic> {
        let fail = || {
            Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "captured child declaration binding or metadata limit".into(),
            )
        };
        if !parent
            .items()
            .iter()
            .any(|header| &header.key == demand.header())
        {
            return Err(fail());
        }
        let selected = capture.record().selected()?;
        let parsed = Arc::new(ParsedFile::parse_dependency(
            selected.blob(),
            caller.selection().outline.parse.profile.clone(),
        )?);
        let cfg = caller.captured_macro_cfg()?;
        let outline = Arc::new(parsed.outline(
            demand.scope().crate_key,
            demand.scope().module_path.clone(),
            cfg,
        )?);
        let estimated_bytes = parsed
            .estimated_bytes()
            .checked_add(outline.estimated_bytes())
            .and_then(|n| n.checked_add(std::mem::size_of::<ParsedFile>()))
            .and_then(|n| n.checked_add(4 * std::mem::size_of::<usize>()))
            .and_then(|n| n.checked_add(demand.metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(parent.metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(caller.selection().metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(invocation_base.source.path.capacity()))
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(fail)?;
        Ok(Self {
            capture,
            parent,
            demand,
            caller,
            invocation_base,
            estimated_bytes,
            parsed,
            outline,
        })
    }
    pub fn outline(&self) -> &Outline {
        &self.outline
    }
    pub fn parent_header(&self) -> &GeneratedItemKey {
        self.demand.header()
    }
    pub fn producer(&self) -> Digest {
        self.demand.producer()
    }
    pub fn capture(&self) -> &AdoptedModuleCapture {
        &self.capture
    }
    pub fn parent(&self) -> &GeneratedItems {
        &self.parent
    }
    pub fn caller(&self) -> &PreparedSourceView {
        &self.caller
    }
    pub fn invocation_base(&self) -> &SpanRecord {
        &self.invocation_base
    }
    pub fn estimated_bytes(&self) -> usize {
        self.estimated_bytes
    }
    pub(crate) fn parsed_handle(&self) -> Arc<ParsedFile> {
        self.parsed.clone()
    }
    pub(crate) fn outline_handle(&self) -> Arc<Outline> {
        self.outline.clone()
    }
    pub fn parsed(&self) -> &ParsedFile {
        &self.parsed
    }
}
