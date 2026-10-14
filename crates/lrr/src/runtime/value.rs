//! Host bit-copy representation, never permission to duplicate guest ownership.
use super::shape::{Primitive, TypeId, primitive};
use crate::core::diagnostic::Diagnostic;
#[repr(C)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ValueRepr {
    pub kind: u32,
    pub type_id: u32,
    pub payload: u64,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ValueKind {
    Empty,
    Unit,
    Bool,
    Char,
    Signed,
    Unsigned,
    F32,
    F64,
    Object,
    Reference,
    Function,
    IntegerLiteral,
    FloatLiteral,
    Iterator,
    NativeObject,
    ManagedBox,
    DynView,
}
impl ValueKind {
    pub fn code(self) -> u32 {
        match self {
            Self::Empty => 0,
            Self::Unit => 1,
            Self::Bool => 2,
            Self::Char => 3,
            Self::Signed => 4,
            Self::Unsigned => 5,
            Self::F32 => 6,
            Self::F64 => 7,
            Self::Object => 8,
            Self::Reference => 9,
            Self::Function => 10,
            Self::IntegerLiteral => 11,
            Self::FloatLiteral => 12,
            Self::Iterator => 13,
            Self::NativeObject => 14,
            Self::ManagedBox => 15,
            Self::DynView => 16,
        }
    }
}
pub(crate) fn invalid() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-VALUE-REPR",
        "guest value kind, descriptor, or payload invariant failed".into(),
    )
}
impl ValueRepr {
    pub fn unit() -> Self {
        Self {
            kind: 1,
            type_id: 1,
            payload: 0,
        }
    }
    pub fn boolean(value: bool) -> Self {
        Self {
            kind: 2,
            type_id: 2,
            payload: u64::from(value),
        }
    }
    pub fn character(value: char) -> Self {
        Self {
            kind: 3,
            type_id: 3,
            payload: u64::from(u32::from(value)),
        }
    }
    pub fn signed(ty: TypeId, value: i64) -> Result<Self, Diagnostic> {
        let repr = Self {
            kind: 4,
            type_id: ty.0,
            payload: value as u64,
        };
        repr.validate_scalar()?;
        Ok(repr)
    }
    pub fn unsigned(ty: TypeId, value: u64) -> Result<Self, Diagnostic> {
        let repr = Self {
            kind: 5,
            type_id: ty.0,
            payload: value,
        };
        repr.validate_scalar()?;
        Ok(repr)
    }
    pub fn f32(value: f32) -> Self {
        Self {
            kind: 6,
            type_id: 14,
            payload: u64::from(value.to_bits()),
        }
    }
    pub fn f64(value: f64) -> Self {
        Self {
            kind: 7,
            type_id: 15,
            payload: value.to_bits(),
        }
    }
    pub fn validate_scalar(self) -> Result<(), Diagnostic> {
        let descriptor = primitive(TypeId(self.type_id))?;
        let valid = match (self.kind, descriptor) {
            (1, Primitive::Unit) => self.payload == 0,
            (2, Primitive::Bool) => self.payload <= 1,
            (3, Primitive::Char) => u32::try_from(self.payload)
                .ok()
                .and_then(char::from_u32)
                .is_some(),
            (4, Primitive::Signed(width)) => {
                let value = self.payload as i64;
                width == 64 || (value >= -(1i64 << (width - 1)) && value < (1i64 << (width - 1)))
            }
            (5, Primitive::Unsigned(width)) => width == 64 || self.payload < (1u64 << width),
            (6, Primitive::F32) => self.payload <= u64::from(u32::MAX),
            (7, Primitive::F64) => true,
            _ => false,
        };
        if valid { Ok(()) } else { Err(invalid()) }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn value_layout_and_integer_widths_match_the_fixed_contract() {
        assert_eq!(std::mem::size_of::<ValueRepr>(), 16);
        assert_eq!(std::mem::align_of::<ValueRepr>(), 8);
        assert!(ValueRepr::signed(TypeId(4), -128).is_ok());
        assert!(ValueRepr::signed(TypeId(4), 128).is_err());
        assert!(ValueRepr::unsigned(TypeId(9), 256).is_err());
        assert!(
            ValueRepr {
                kind: 2,
                type_id: 2,
                payload: 2
            }
            .validate_scalar()
            .is_err()
        );
        assert!(
            ValueRepr {
                kind: 3,
                type_id: 3,
                payload: 0xd800
            }
            .validate_scalar()
            .is_err()
        );
    }
}
