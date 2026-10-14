//! Exact typed immutable constant query result; no guest handles or authority.
use super::{memo::ConstMemoPayload, query_identity::ConstQueryIdentity};
use crate::core::{
    diagnostic::Diagnostic,
    digest::{StableEncode, StableEncoder},
};
#[derive(Debug, Clone)]
pub struct ConstantQueryValue {
    key: ConstQueryIdentity,
    payload: ConstMemoPayload,
}
impl ConstantQueryValue {
    pub fn capture(
        key: &ConstQueryIdentity,
        payload: ConstMemoPayload,
    ) -> Result<Self, Diagnostic> {
        key.validate()?;
        if key.result_type() != payload.type_identity() {
            return Err(Diagnostic::new(
                "LRR-E-CONST-TYPE",
                "constant result concrete type differs".into(),
            ));
        }
        if !key.matches_payload(&payload) {
            return Err(Diagnostic::new(
                "LRR-E-CONST-SOURCE",
                "constant result selected source differs".into(),
            ));
        }
        key.metadata_bytes()?
            .checked_add(payload.metadata_bytes()?)
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() + 128))
            .filter(|n| *n <= 32 << 20)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "constant query result metadata limit".into(),
                )
            })?;
        Ok(Self {
            key: key.clone(),
            payload,
        })
    }
    pub fn key(&self) -> &ConstQueryIdentity {
        &self.key
    }
    pub fn payload(&self) -> &ConstMemoPayload {
        &self.payload
    }
    pub fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.key.heap_bytes()?)
            .and_then(|n| n.checked_add(self.payload.metadata_bytes().ok()?))
            .filter(|n| *n <= 32 << 20)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "constant query result metadata limit".into(),
                )
            })
    }
}
impl StableEncode for ConstantQueryValue {
    const DOMAIN: &'static str = "lrr/v3/typed-constant-query-value";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(&self.payload);
    }
}
