//! Adopted physical-module syntax projection. No publication or executable lowering.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    db::dependency::{InputKey, InputValue},
    source::{changes::PreparedSourceView, module_view::CapturedModuleSourceView},
};
use std::{collections::BTreeMap, sync::Arc};

pub struct ModuleSyntaxInput {
    identity: Digest,
    origin: Arc<CapturedModuleSourceView>,
    prepared: PreparedSourceView,
    metadata_bytes: usize,
}
impl ModuleSyntaxInput {
    pub(crate) fn prepare(origin: Arc<CapturedModuleSourceView>) -> Result<Self, Diagnostic> {
        let fault = || {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "module syntax projection metadata limit".into(),
            )
        };
        let prepared = PreparedSourceView::from_module(&origin)?;
        let mut metadata_bytes = origin
            .estimated_bytes()
            .checked_add(prepared.prepared_metadata_bytes()?)
            .and_then(|n| n.checked_add(prepared.selection().metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(fault)?;
        for (key, value) in prepared.prepared_inputs() {
            metadata_bytes = metadata_bytes
                .checked_add(key.heap_bytes())
                .and_then(|n| n.checked_add(value.estimated_bytes().ok()?))
                .and_then(|n| n.checked_add(128))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(fault)?;
        }
        let mut identity = StableEncoder::new("lrr/v2/module-syntax-projection");
        identity.digest(origin.capture().input_identity());
        identity.digest(origin.capture().record().identity());
        identity.digest(origin.producer());
        identity.record(origin.parent_header());
        identity.record(&origin.parent().record().descriptor());
        identity.string(prepared.selection().path.as_str());
        identity.digest(InputValue::CapturedSource(prepared.selection().clone()).fingerprint());
        identity.string(origin.caller().selection().path.as_str());
        identity
            .digest(InputValue::CapturedSource(origin.caller().selection().clone()).fingerprint());
        identity.bytes(&origin.invocation_base().key().0);
        identity.u64(u64::try_from(prepared.prepared_inputs().len()).map_err(|_| fault())?);
        for (key, value) in prepared.prepared_inputs() {
            identity.record(key);
            identity.digest(value.fingerprint());
        }
        Ok(Self {
            identity: identity.finish(),
            origin,
            prepared,
            metadata_bytes,
        })
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn origin(&self) -> &CapturedModuleSourceView {
        &self.origin
    }
    pub fn prepared_source(&self) -> &PreparedSourceView {
        &self.prepared
    }
    pub fn inputs(&self) -> &BTreeMap<InputKey, InputValue> {
        self.prepared.prepared_inputs()
    }
    pub fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
}
