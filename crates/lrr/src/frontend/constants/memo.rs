//! Immutable typed payload for the T054 query-value migration.
//! Content/source-selection binding is not a current database seal or execution authority.
use super::{
    ConstEvaluation, ConstValue,
    wire::{self, ConstantWireType},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    syntax::pure_constants::ConstantPlan,
};
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-MEMO", message.into())
}
/// Only canonical scalar/array bytes can enter; fields remain private.
/// SourceDb publication must additionally replay current source, namespace seals,
/// epochs, concrete substitutions and every dependency before caching this value.
#[derive(Debug, Clone)]
pub struct ConstMemoPayload {
    definition: Digest,
    selection: Digest,
    source: Digest,
    cfg: Digest,
    profile: ProfileIdentity,
    value_type: ConstantWireType,
    type_identity: Digest,
    dependencies: Digest,
    // Cloning a payload makes an independent bounded byte allocation. Only the
    // outer cached QueryValue is shared, so raw inner clones cannot evade cache
    // retirement accounting by keeping an unregistered Arc byte allocation alive.
    bytes: Box<[u8]>,
}
impl ConstMemoPayload {
    /// Canonical content construction for trusted pure providers. Identity data
    /// is never a current source, macro, native or execution admission proof.
    pub fn capture_content(
        identity: &super::query_identity::ConstQueryIdentity,
        value: &ConstValue,
        value_type: &ConstantWireType,
        dependencies: Digest,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        identity.validate()?;
        let type_identity = value_type.fingerprint_checked()?;
        if type_identity != identity.result_type() {
            return Err(Diagnostic::new(
                "LRR-E-CONST-TYPE",
                "constant content type differs from query".into(),
            ));
        }
        value_type.metadata_bytes()?;
        let bytes = wire::encode(value, value_type, byte_limit)?;
        Ok(Self {
            definition: identity.definition(),
            selection: identity.selection(),
            source: identity.source(),
            cfg: identity.cfg(),
            profile: identity.profile().clone(),
            value_type: value_type.clone(),
            type_identity,
            dependencies,
            bytes: bytes.into(),
        })
    }
    pub fn capture(
        plan: &ConstantPlan,
        evaluation: &ConstEvaluation,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        // Refuse stale/foreign selection before encoding or cloning a result.
        if evaluation.plan() != plan.identity()
            || evaluation.source() != plan.source().source()
            || evaluation.cfg() != plan.source().cfg()
        {
            return Err(invalid(
                "constant evaluation belongs to another source/cfg/selection",
            ));
        }
        plan.profile().validate_source()?;
        if byte_limit == 0 {
            return Err(invalid("constant payload byte budget exceeds profile"));
        }
        let value_type = evaluation.value_type();
        let type_identity = value_type.fingerprint_checked()?;
        value_type.metadata_bytes()?;
        let bytes = wire::encode(evaluation.value(), value_type, byte_limit)?;
        let mut dependencies = StableEncoder::new("lrr/v1/pure-constant-memo-dependencies");
        dependencies.sequence(evaluation.dependencies(), |encoder, dependency| {
            encoder.record(dependency)
        });
        Ok(Self {
            definition: plan.owner_definition(),
            selection: plan.identity(),
            source: evaluation.source(),
            cfg: evaluation.cfg(),
            profile: plan.profile().clone(),
            value_type: value_type.clone(),
            type_identity,
            dependencies: dependencies.finish(),
            bytes: bytes.into(),
        })
    }
    pub fn definition(&self) -> Digest {
        self.definition
    }
    pub fn selection(&self) -> Digest {
        self.selection
    }
    pub fn source(&self) -> Digest {
        self.source
    }
    pub fn cfg(&self) -> Digest {
        self.cfg
    }
    pub fn profile(&self) -> &ProfileIdentity {
        &self.profile
    }
    pub fn type_identity(&self) -> Digest {
        self.type_identity
    }
    pub fn value_type(&self) -> &ConstantWireType {
        &self.value_type
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Fresh immutable decoding only. Owning guest arrays still require the runtime
    /// construction/cleanup transaction; this method creates no GuestStore cells.
    pub fn decode_value(&self) -> Result<ConstValue, Diagnostic> {
        wire::decode(&self.bytes, &self.value_type, usize::MAX)
    }
    pub fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(128)
            .and_then(|n| n.checked_add(self.bytes.len()))
            .and_then(|n| n.checked_add(self.profile.semantics.capacity()))
            .and_then(|n| n.checked_add(self.value_type.metadata_bytes().ok()?))
            .ok_or_else(|| invalid("constant payload metadata overflow"))
    }
}
impl StableEncode for ConstMemoPayload {
    const DOMAIN: &'static str = "lrr/v1/typed-pure-constant-memo-payload";
    fn encode_fields(&self, encoder: &mut StableEncoder) {
        encoder.digest(self.definition);
        encoder.digest(self.selection);
        encoder.digest(self.source);
        encoder.digest(self.cfg);
        encoder.record(&self.profile);
        encoder.digest(self.type_identity);
        encoder.digest(self.dependencies);
        encoder.bytes(&self.bytes);
    }
}
