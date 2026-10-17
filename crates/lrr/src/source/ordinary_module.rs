//! Immutable ordinary module capture projection; no generated header or expansion record.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        poll::InputRequest,
    },
    frontend::source_module_declarations::SourceModuleDeclaration,
    source::{changes::PreparedSourceView, module_capture::AdoptedModuleCapture},
    syntax::{outline::Outline, parse::ParsedFile},
};
use std::{collections::BTreeMap, sync::Arc};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CATALOG-INCOMPLETE", message.into())
}
pub struct OrdinaryModuleSourceView {
    capture: Arc<AdoptedModuleCapture>,
    declaration: SourceModuleDeclaration,
    compile_environment: Digest,
    parsed: Arc<ParsedFile>,
    outline: Arc<Outline>,
    bytes: usize,
}
impl OrdinaryModuleSourceView {
    pub fn new(
        declaration: &SourceModuleDeclaration,
        input: &InputRequest,
        capture: Arc<AdoptedModuleCapture>,
    ) -> Result<Self, Diagnostic> {
        input.validate()?;
        let InputRequest::CaptureModule {
            request,
            compile_environment,
        } = input
        else {
            return Err(fault("ordinary module requires exact module capture input"));
        };
        if capture.input_identity() != input.fingerprint()
            || request.producer() != declaration.producer()
            || request.declaration() != declaration.request()
            || request.candidates().is_empty()
            || request.candidates().iter().any(|candidate| {
                candidate.from().unit != declaration.scope().crate_key
                    || candidate.from().path != declaration.source().selection().path
                    || candidate.from().origin != declaration.capture_origin()
                    || candidate.include_origin() != declaration.capture_origin()
            })
        {
            return Err(fault(
                "ordinary module capture/declaration provenance differs",
            ));
        }
        capture.record().validate(request)?;
        let selected = capture.record().selected()?;
        let parsed = Arc::new(ParsedFile::parse_dependency(
            selected.blob(),
            declaration
                .source()
                .selection()
                .outline
                .parse
                .profile
                .clone(),
        )?);
        let outline = Arc::new(parsed.outline(
            declaration.scope().crate_key,
            declaration.scope().module_path.clone(),
            declaration.source().captured_macro_cfg()?,
        )?);
        let bytes = parsed
            .estimated_bytes()
            .checked_add(outline.estimated_bytes())
            .and_then(|n| n.checked_add(declaration.metadata_bytes()))
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(|| fault("ordinary module projection metadata limit"))?;
        Ok(Self {
            capture,
            declaration: declaration.share(),
            compile_environment: *compile_environment,
            parsed,
            outline,
            bytes,
        })
    }
    pub fn capture(&self) -> &AdoptedModuleCapture {
        &self.capture
    }
    pub fn declaration(&self) -> &SourceModuleDeclaration {
        &self.declaration
    }
    pub fn compile_environment(&self) -> Digest {
        self.compile_environment
    }
    pub(crate) fn parsed_handle(&self) -> Arc<ParsedFile> {
        self.parsed.clone()
    }
    pub(crate) fn outline_handle(&self) -> Arc<Outline> {
        self.outline.clone()
    }
}
pub struct OrdinaryModuleSyntaxInput {
    identity: Digest,
    origin: Arc<OrdinaryModuleSourceView>,
    prepared: PreparedSourceView,
    bytes: usize,
}
impl OrdinaryModuleSyntaxInput {
    pub fn prepare(origin: Arc<OrdinaryModuleSourceView>) -> Result<Self, Diagnostic> {
        let prepared = PreparedSourceView::from_ordinary_module(&origin)?;
        let mut bytes = origin
            .bytes
            .checked_add(prepared.prepared_metadata_bytes()?)
            .and_then(|n| n.checked_add(prepared.selection().metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(|| fault("ordinary module syntax size overflow"))?;
        for (key, value) in prepared.prepared_inputs() {
            bytes = bytes
                .checked_add(key.heap_bytes())
                .and_then(|n| n.checked_add(value.estimated_bytes().ok()?))
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| fault("ordinary module syntax metadata limit"))?;
        }
        let mut e = StableEncoder::new("lrr/v2/ordinary-module-syntax-projection");
        e.record(origin.declaration());
        e.digest(origin.capture().input_identity());
        e.digest(origin.capture().record().identity());
        e.digest(origin.compile_environment());
        e.digest(InputValue::CapturedSource(prepared.selection().clone()).fingerprint());
        e.u64(
            u64::try_from(prepared.prepared_inputs().len())
                .map_err(|_| fault("ordinary module input count overflow"))?,
        );
        for (key, value) in prepared.prepared_inputs() {
            e.record(key);
            e.digest(value.fingerprint());
        }
        Ok(Self {
            identity: e.finish(),
            origin,
            prepared,
            bytes,
        })
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn origin(&self) -> &OrdinaryModuleSourceView {
        &self.origin
    }
    pub fn prepared_source(&self) -> &PreparedSourceView {
        &self.prepared
    }
    pub fn inputs(&self) -> &BTreeMap<InputKey, InputValue> {
        self.prepared.prepared_inputs()
    }
    pub fn metadata_bytes(&self) -> usize {
        self.bytes
    }
}
