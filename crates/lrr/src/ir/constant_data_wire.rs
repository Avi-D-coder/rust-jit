//! T054 immutable constant payload transport. This codec grants no source authority.
//! Memo publication must independently bind current source/seals and the query key.
use super::{ConstValue, Immutable};
use crate::{
    core::diagnostic::Diagnostic,
    ir::op::PrimitiveType,
    runtime::{shape::PrimitiveIdentity, value::ValueRepr},
};
use std::sync::Arc;
const MAGIC: &[u8; 8] = b"LRRCNST\0";
const VERSION: u16 = 1;
/// Exact concrete result shape, including the element type of an empty array.
/// This describes the current scalar/array evaluator surface, not a runtime TypeId
/// admission certificate. Source-selected aliases/generics are resolved separately.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ConstantWireType {
    Scalar(PrimitiveType),
    Array { element: Box<Self>, length: u64 },
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-WIRE", message.into())
}
#[derive(Clone, Copy)]
struct Layout {
    shape: usize,
    nodes: usize,
    scalars: usize,
    arrays: usize,
}
impl ConstantWireType {
    /// Checked stable type identity; includes empty-array element types and lengths.
    pub fn fingerprint_checked(&self) -> Result<crate::core::digest::Digest, Diagnostic> {
        let admitted = admit(self)?;
        let mut encoder =
            crate::core::digest::StableEncoder::new("lrr/v1/pure-constant-result-type");
        encoder.bytes(&admitted.header);
        Ok(encoder.finish())
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.layout()?;
        let mut shape = self;
        let mut bytes = 0usize;
        loop {
            bytes = bytes
                .checked_add(std::mem::size_of::<Self>() + 32)
                .ok_or_else(|| invalid("constant type metadata overflow"))?;
            match shape {
                Self::Scalar(_) => return Ok(bytes),
                Self::Array { element, .. } => shape = element,
            }
        }
    }
    fn layout(&self) -> Result<Layout, Diagnostic> {
        Ok(match self {
            Self::Scalar(ty) => {
                PrimitiveIdentity::from_recipe(*ty)?;
                Layout {
                    shape: 5,
                    nodes: 1,
                    scalars: 1,
                    arrays: 0,
                }
            }
            Self::Array { element, length } => {
                let child = element.layout()?;
                let count = usize::try_from(*length)
                    .map_err(|_| invalid("array count exceeds host width"))?;
                Layout {
                    shape: child
                        .shape
                        .checked_add(9)
                        .ok_or_else(|| invalid("shape size overflow"))?,
                    nodes: count
                        .checked_mul(child.nodes)
                        .and_then(|n| n.checked_add(1))
                        .ok_or_else(|| invalid("value node count overflow"))?,
                    scalars: count
                        .checked_mul(child.scalars)
                        .ok_or_else(|| invalid("scalar count overflow"))?,
                    arrays: count
                        .checked_mul(child.arrays)
                        .and_then(|n| n.checked_add(1))
                        .ok_or_else(|| invalid("array count overflow"))?,
                }
            }
        })
    }
    fn write_shape(&self, bytes: &mut Vec<u8>) -> Result<(), Diagnostic> {
        match self {
            Self::Scalar(ty) => {
                bytes.push(0);
                bytes.extend_from_slice(&ty.tag().to_le_bytes());
            }
            Self::Array { element, length } => {
                bytes.push(1);
                bytes.extend_from_slice(&length.to_le_bytes());
                element.write_shape(bytes)?;
            }
        }
        Ok(())
    }
}
struct Admission {
    header: Vec<u8>,
    bytes: usize,
}
/// Exact wire size and canonical header for a concrete constant type. E-R5-006:
/// no byte, node or depth ceiling; only arithmetic overflow is refused.
fn admit(ty: &ConstantWireType) -> Result<Admission, Diagnostic> {
    let layout = ty.layout()?;
    let header_bytes = layout
        .shape
        .checked_add(10)
        .ok_or_else(|| invalid("header size overflow"))?;
    let bytes = layout
        .scalars
        .checked_mul(16)
        .and_then(|n| n.checked_add(header_bytes))
        .ok_or_else(|| invalid("constant wire size overflow"))?;
    let mut header = Vec::new();
    header
        .try_reserve_exact(header_bytes)
        .map_err(|_| invalid("constant header allocation"))?;
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&VERSION.to_le_bytes());
    ty.write_shape(&mut header)?;
    if header.len() != header_bytes {
        return Err(invalid("constant header layout differs"));
    }
    Ok(Admission { header, bytes })
}
/// Checks the exact rectangular scalar/array shape without decoding values.
// E-R5-006: ignored, remove at callers (`_wire_limit`, `_value_limit`).
pub(crate) fn admit_result_limits(
    ty: &ConstantWireType,
    _wire_limit: usize,
    _value_limit: usize,
) -> Result<(), Diagnostic> {
    admit(ty).map(|_| ())
}
fn scalar(value: &ConstValue, ty: PrimitiveType) -> Result<ValueRepr, Diagnostic> {
    let value = value
        .scalar()
        .ok_or_else(|| invalid("expected scalar constant"))?;
    value
        .validate_scalar()
        .map_err(|_| invalid("noncanonical constant scalar"))?;
    if value.type_id != PrimitiveIdentity::from_recipe(ty)?.id().0 {
        return Err(invalid("constant scalar type differs"));
    }
    Ok(value)
}
fn write(value: &ConstValue, ty: &ConstantWireType, bytes: &mut Vec<u8>) -> Result<(), Diagnostic> {
    match ty {
        ConstantWireType::Scalar(ty) => {
            let v = scalar(value, *ty)?;
            bytes.extend_from_slice(&v.kind.to_le_bytes());
            bytes.extend_from_slice(&v.type_id.to_le_bytes());
            bytes.extend_from_slice(&v.payload.to_le_bytes());
        }
        ConstantWireType::Array { element, length } => {
            let values = value
                .elements()
                .ok_or_else(|| invalid("expected array constant"))?;
            let count =
                usize::try_from(*length).map_err(|_| invalid("array count exceeds host width"))?;
            if values.len() != count {
                return Err(invalid("constant array length differs"));
            }
            for value in values {
                write(value, element, bytes)?;
            }
        }
    }
    Ok(())
}
// E-R5-006: ignored, remove at callers (`_byte_limit`); use `encode_value`.
pub fn encode(
    value: &ConstValue,
    expected: &ConstantWireType,
    _byte_limit: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_value(value, expected)
}
/// Explicit little-endian schema, never repr(C), serde or Debug transport.
pub fn encode_value(
    value: &ConstValue,
    expected: &ConstantWireType,
) -> Result<Vec<u8>, Diagnostic> {
    let admission = admit(expected)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(admission.bytes)
        .map_err(|_| invalid("constant wire allocation"))?;
    bytes.extend_from_slice(&admission.header);
    write(value, expected, &mut bytes)?;
    if bytes.len() != admission.bytes {
        return Err(invalid("constant wire layout differs"));
    }
    Ok(bytes)
}
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], Diagnostic> {
        let end = self
            .position
            .checked_add(N)
            .ok_or_else(|| invalid("constant wire cursor overflow"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| invalid("truncated constant scalar"))?;
        self.position = end;
        bytes
            .try_into()
            .map_err(|_| invalid("constant scalar width differs"))
    }
    fn value(&mut self, ty: &ConstantWireType) -> Result<ConstValue, Diagnostic> {
        match ty {
            ConstantWireType::Scalar(ty) => {
                let repr = ValueRepr {
                    kind: u32::from_le_bytes(self.take()?),
                    type_id: u32::from_le_bytes(self.take()?),
                    payload: u64::from_le_bytes(self.take()?),
                };
                let value = ConstValue::from_scalar(repr)
                    .map_err(|_| invalid("invalid immutable scalar or guest handle"))?;
                scalar(&value, *ty)?;
                Ok(value)
            }
            ConstantWireType::Array { element, length } => {
                let count = usize::try_from(*length)
                    .map_err(|_| invalid("constant count exceeds host width"))?;
                let mut values = Vec::new();
                values
                    .try_reserve_exact(count)
                    .map_err(|_| invalid("constant array allocation"))?;
                for _ in 0..count {
                    values.push(self.value(element)?);
                }
                Ok(ConstValue(Immutable::Array(Arc::from(values))))
            }
        }
    }
}
// E-R5-006: ignored, remove at callers (`_byte_limit`); use `decode_value`.
pub fn decode(
    bytes: &[u8],
    expected: &ConstantWireType,
    _byte_limit: usize,
) -> Result<ConstValue, Diagnostic> {
    decode_value(bytes, expected)
}
/// Decode only after the caller supplies its exact expected concrete result type.
/// The exact header and wire length are checked before allocating any value
/// tree. No decoded payload can contain a guest handle.
pub fn decode_value(bytes: &[u8], expected: &ConstantWireType) -> Result<ConstValue, Diagnostic> {
    let admission = admit(expected)?;
    if bytes.len() != admission.bytes || !bytes.starts_with(&admission.header) {
        return Err(invalid(
            "constant wire schema, type or exact length differs",
        ));
    }
    let mut reader = Reader {
        bytes,
        position: admission.header.len(),
    };
    let value = reader.value(expected)?;
    if reader.position != bytes.len() {
        return Err(invalid("trailing constant wire bytes"));
    }
    Ok(value)
}
