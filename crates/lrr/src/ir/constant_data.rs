//! Shared immutable constant data. No source parser, query provider or guest execution.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    ir::op::PrimitiveType,
    runtime::{shape::PrimitiveIdentity, value::ValueRepr},
};
use std::sync::Arc;
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
#[derive(Debug, Clone, Eq, PartialEq)]
enum Immutable {
    Scalar(ValueRepr),
    Array(Arc<[ConstValue]>),
}
/// Fields are private: no guest/object/reference/function handle can enter a constant value.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConstValue(Immutable);
impl ConstValue {
    pub fn from_scalar(value: ValueRepr) -> Result<Self, Diagnostic> {
        if !matches!(value.kind, 1..=7) {
            return Err(fault(
                "LRR-E-CONST-TYPE",
                "constant scalar cannot contain a guest handle",
            ));
        }
        value.validate_scalar()?;
        Ok(Self(Immutable::Scalar(value)))
    }
    /// Only the pure evaluator may build an array outside the checked codec.
    /// Every child is already private immutable data. E-R5-006: rustc has no
    /// constant aggregate size, node or depth limit, so none is applied here.
    pub(crate) fn from_elements(values: Vec<Self>) -> Result<Self, Diagnostic> {
        Ok(Self(Immutable::Array(values.into())))
    }
    pub fn scalar(&self) -> Option<ValueRepr> {
        match &self.0 {
            Immutable::Scalar(v) => Some(*v),
            _ => None,
        }
    }
    pub fn elements(&self) -> Option<&[ConstValue]> {
        match &self.0 {
            Immutable::Array(es) => Some(es),
            _ => None,
        }
    }
    pub(crate) fn matches_scalar(&self, ty: PrimitiveType) -> Result<bool, Diagnostic> {
        Ok(self.scalar().is_some_and(|v| {
            Some(v.type_id) == PrimitiveIdentity::from_recipe(ty).ok().map(|id| id.id().0)
        }))
    }
    fn field_bytes(&self) -> Option<usize> {
        match &self.0 {
            Immutable::Scalar(_) => Some(17),
            Immutable::Array(es) => es
                .iter()
                .try_fold(9usize, |n, e| n.checked_add(e.field_bytes()?)),
        }
    }
    pub(crate) fn serialized_bytes(&self) -> Option<usize> {
        self.field_bytes()?
            .checked_add(8)?
            .checked_add(Self::DOMAIN.len())
    }
}
impl StableEncode for ConstValue {
    const DOMAIN: &'static str = "lrr/v1/immutable-constant-value";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match &self.0 {
            Immutable::Scalar(v) => {
                e.u8(1);
                e.u32(v.kind);
                e.u32(v.type_id);
                e.u64(v.payload)
            }
            Immutable::Array(es) => {
                e.u8(2);
                e.sequence(es, |e, v| v.encode_fields(e))
            }
        }
    }
}

#[path = "constant_data_wire.rs"]
pub mod wire;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_array_factory_and_wire_share_nesting_and_handle_boundaries() {
        // LRR-specific wire/ownership boundary, not an upstream language case.
        // E-R5-006: nesting deeper than the former 128-level cap round-trips.
        let mut value = ConstValue::from_scalar(ValueRepr::unit()).unwrap();
        let mut ty = wire::ConstantWireType::Scalar(PrimitiveType::Unit);
        for _ in 0..200 {
            value = ConstValue::from_elements(vec![value]).unwrap();
            ty = wire::ConstantWireType::Array {
                element: Box::new(ty),
                length: 1,
            };
        }
        let bytes = wire::encode_value(&value, &ty).unwrap();
        let replay = wire::decode_value(&bytes, &ty).unwrap();
        assert_eq!(value.fingerprint(), replay.fingerprint());
        assert!(
            ConstValue::from_scalar(ValueRepr {
                kind: 8,
                type_id: 17,
                payload: 1
            })
            .is_err()
        );
    }
}
