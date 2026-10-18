//! Full-width Copy integers behind checked immutable numeric arena records.
//! ValueRepr remains 16 bytes. Copies share bits, never an owning guest object.
use super::*;

impl GuestStore {
    /// Materialize a literal's bit pattern at its selected primitive width.
    /// Rust's overflowing_literals lint is outside runtime execution.
    pub(crate) fn integer_literal_bits(
        negative: bool,
        magnitude: u128,
        ty: TypeId,
    ) -> Result<u128, Diagnostic> {
        use super::super::shape::{Primitive, primitive};
        match primitive(ty)? {
            Primitive::Signed(width) => {
                let bits = if negative {
                    0u128.wrapping_sub(magnitude)
                } else {
                    magnitude
                };
                Ok(if width == 128 {
                    bits
                } else {
                    bits & ((1u128 << width) - 1)
                })
            }
            Primitive::Unsigned(width) => {
                if negative {
                    return Err(Diagnostic::new(
                        "LRR-E-OP-TYPE",
                        "unsigned integer cannot carry folded unary negation".into(),
                    ));
                }
                Ok(if width == 128 {
                    magnitude
                } else {
                    magnitude & ((1u128 << width) - 1)
                })
            }
            _ => Err(invalid()),
        }
    }
    /// Existing unresolved literal storage supplies wide bits without another
    /// allocation. A concrete view must agree with its actual selected binding.
    pub(super) fn wide_literal_value(
        &self,
        literal: ValueRepr,
        ty: TypeId,
    ) -> Result<ValueRepr, Diagnostic> {
        if literal.kind != 11 || !matches!(ty.0, 17 | 18) {
            return Err(invalid());
        }
        let record = self.literals.get(LiteralId::from_bits(literal.payload))?;
        if record.binding.is_some_and(|bound| bound != ty) {
            return Err(ownership());
        }
        let LiteralNumber::Integer {
            negative,
            magnitude,
        } = record.number
        else {
            return Err(invalid());
        };
        Self::integer_literal_bits(negative, magnitude, ty)?;
        Ok(ValueRepr {
            kind: 17,
            type_id: ty.0,
            payload: literal.payload,
        })
    }
    /// Decode only actual same-run arena handles with matching numeric family.
    pub(crate) fn wide_integer_bits(&self, value: ValueRepr) -> Result<u128, Diagnostic> {
        if value.kind != 17 || !matches!(value.type_id, 17 | 18) {
            return Err(invalid());
        }
        let record = self.literals.get(LiteralId::from_bits(value.payload))?;
        if record.binding.is_some_and(|bound| bound.0 != value.type_id) {
            return Err(ownership());
        }
        match record.number {
            LiteralNumber::Integer {
                negative,
                magnitude,
            } => Self::integer_literal_bits(negative, magnitude, TypeId(value.type_id)),
            _ => Err(invalid()),
        }
    }
    /// A typed operation's result, with its full bits and source width retained.
    /// Records live for the run, like existing immutable numeric literal records.
    pub(crate) fn wide_integer_value(
        &mut self,
        ty: TypeId,
        bits: u128,
    ) -> Result<ValueRepr, Diagnostic> {
        if !matches!(ty.0, 17 | 18) {
            return Err(invalid());
        }
        let negative = ty.0 == 17 && bits >> 127 != 0;
        let magnitude = if negative {
            0u128.wrapping_sub(bits)
        } else {
            bits
        };
        self.literals.reserve(1, &mut self.budget)?;
        let id = self.literals.allocate(
            LiteralRecord {
                number: LiteralNumber::Integer {
                    negative,
                    magnitude,
                },
                spelling: String::new(),
                binding: Some(ty),
            },
            0,
            &mut self.budget,
        )?;
        Ok(ValueRepr {
            kind: 17,
            type_id: ty.0,
            payload: id.bits(),
        })
    }
    /// Exact scalar bits for the shared integer operator/cast/format consumers.
    /// A narrow signed operand is sign-extended, a narrow unsigned is zero-extended.
    pub(crate) fn integer_bits(&self, value: ValueRepr) -> Result<u128, Diagnostic> {
        match value.kind {
            4 => {
                value.validate_scalar()?;
                Ok((value.payload as i64 as i128) as u128)
            }
            5 => {
                value.validate_scalar()?;
                Ok(value.payload as u128)
            }
            17 => self.wide_integer_bits(value),
            _ => Err(invalid()),
        }
    }
}
