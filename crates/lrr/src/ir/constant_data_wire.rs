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
const MAX_BYTES: usize = 1 << 20;
const MAX_NODES: usize = 100_000;
const MAX_DEPTH: usize = 128;
const MAX_HEAP: usize = 32 << 20;
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
        let admitted = admit(self, MAX_BYTES)?;
        let mut encoder =
            crate::core::digest::StableEncoder::new("lrr/v1/pure-constant-result-type");
        encoder.bytes(&admitted.header);
        Ok(encoder.finish())
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.layout(0)?;
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
    fn layout(&self, depth: usize) -> Result<Layout, Diagnostic> {
        if depth >= MAX_DEPTH {
            return Err(invalid("constant wire type depth limit"));
        }
        let result = match self {
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
                let child = element.layout(depth + 1)?;
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
        };
        if result.nodes > MAX_NODES {
            return Err(invalid("constant wire node limit"));
        }
        Ok(result)
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
fn admit(ty: &ConstantWireType, limit: usize) -> Result<Admission, Diagnostic> {
    if limit == 0 || limit > MAX_BYTES {
        return Err(invalid("constant wire byte limit exceeds profile"));
    }
    let layout = ty.layout(0)?;
    let header_bytes = layout
        .shape
        .checked_add(10)
        .ok_or_else(|| invalid("header size overflow"))?;
    let bytes = layout
        .scalars
        .checked_mul(16)
        .and_then(|n| n.checked_add(header_bytes))
        .filter(|n| *n <= limit)
        .ok_or_else(|| invalid("constant wire byte limit"))?;
    // Includes both Vec and Arc payloads during transfer, BTree-free traversal,
    // allocation overhead for every array and the temporary canonical header.
    let heap = layout
        .nodes
        .checked_mul(std::mem::size_of::<ConstValue>())
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| {
            layout
                .arrays
                .checked_mul(128)
                .and_then(|a| n.checked_add(a))
        })
        .and_then(|n| n.checked_add(header_bytes))
        .and_then(|n| n.checked_add(bytes))
        .filter(|n| *n <= MAX_HEAP)
        .ok_or_else(|| invalid("constant wire heap limit"))?;
    let _ = heap;
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
/// Reuse still obeys the requesting payload and immutable-value byte limits.
/// This checks the exact rectangular scalar/array shape without decoding values.
pub(crate) fn admit_result_limits(
    ty: &ConstantWireType,
    wire_limit: usize,
    value_limit: usize,
) -> Result<(), Diagnostic> {
    admit(ty, wire_limit)?;
    let layout = ty.layout(0)?;
    layout
        .scalars
        .checked_mul(17)
        .and_then(|n| layout.arrays.checked_mul(9).and_then(|a| n.checked_add(a)))
        .and_then(|n| {
            n.checked_add(8 + <ConstValue as crate::core::digest::StableEncode>::DOMAIN.len())
        })
        .filter(|n| *n <= value_limit)
        .ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-CONST-LIMIT",
                "constant immutable value byte limit".into(),
            )
        })?;
    Ok(())
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
/// Explicit little-endian schema, never repr(C), serde or Debug transport.
pub fn encode(
    value: &ConstValue,
    expected: &ConstantWireType,
    byte_limit: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let admission = admit(expected, byte_limit)?;
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
/// Decode only after the caller supplies its exact expected concrete result type.
/// Header, expanded node count, depth, wire bytes and heap are checked before
/// allocating any value tree. No decoded payload can contain a guest handle.
pub fn decode(
    bytes: &[u8],
    expected: &ConstantWireType,
    byte_limit: usize,
) -> Result<ConstValue, Diagnostic> {
    let admission = admit(expected, byte_limit)?;
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
