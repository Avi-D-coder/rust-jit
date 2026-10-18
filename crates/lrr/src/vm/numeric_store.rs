//! The ordinary primitive numeric operations with run-local full-width backing.
//! Narrow primitives keep their existing immediate path. No source is inspected.
use super::*;
use crate::runtime::cell::GuestStore;

pub(crate) fn binary_in(
    store: &mut GuestStore,
    op: Binary,
    left: ValueRepr,
    right: ValueRepr,
) -> Result<ValueRepr> {
    if left.kind != 17 && right.kind != 17 {
        return binary(op, left, right);
    }
    binary_types(op, TypeId(left.type_id), TypeId(right.type_id))?;
    let a = store.integer_bits(left)?;
    let b = store.integer_bits(right)?;
    let shift = matches!(op, Binary::Shl | Binary::Shr);
    let count = if shift {
        let width = match primitive(TypeId(left.type_id))? {
            Primitive::Signed(w) | Primitive::Unsigned(w) => w,
            _ => return Err(op_type()),
        };
        if b >= u128::from(width) {
            return Err(NumericFailure::GuestPanic(PanicKind::InvalidShift));
        }
        b as u32
    } else {
        0
    };
    if left.kind != 17 {
        // The only mixed-width operation is a shift. Its exact, checked count
        // is representable in u32; the left operand's width stays unchanged.
        return binary(op, left, ValueRepr::unsigned(TypeId(11), u64::from(count))?);
    }
    let comparison = |result| Ok(ValueRepr::boolean(result));
    let bits = if left.type_id == 17 {
        let a = a as i128;
        let b = b as i128;
        match op {
            Binary::Eq => return comparison(a == b),
            Binary::Ne => return comparison(a != b),
            Binary::Lt => return comparison(a < b),
            Binary::Le => return comparison(a <= b),
            Binary::Gt => return comparison(a > b),
            Binary::Ge => return comparison(a >= b),
            Binary::Add => a.checked_add(b),
            Binary::Sub => a.checked_sub(b),
            Binary::Mul => a.checked_mul(b),
            Binary::Div | Binary::Rem => {
                if b == 0 {
                    return Err(NumericFailure::GuestPanic(PanicKind::DivisionByZero));
                }
                if op == Binary::Div {
                    a.checked_div(b)
                } else {
                    a.checked_rem(b)
                }
            }
            Binary::Shl => a.checked_shl(count),
            Binary::Shr => a.checked_shr(count),
            Binary::BitAnd => Some(a & b),
            Binary::BitOr => Some(a | b),
            Binary::BitXor => Some(a ^ b),
        }
        .ok_or(NumericFailure::GuestPanic(PanicKind::Overflow))? as u128
    } else {
        match op {
            Binary::Eq => return comparison(a == b),
            Binary::Ne => return comparison(a != b),
            Binary::Lt => return comparison(a < b),
            Binary::Le => return comparison(a <= b),
            Binary::Gt => return comparison(a > b),
            Binary::Ge => return comparison(a >= b),
            Binary::Add => a.checked_add(b),
            Binary::Sub => a.checked_sub(b),
            Binary::Mul => a.checked_mul(b),
            Binary::Div | Binary::Rem => {
                if b == 0 {
                    return Err(NumericFailure::GuestPanic(PanicKind::DivisionByZero));
                }
                if op == Binary::Div {
                    a.checked_div(b)
                } else {
                    a.checked_rem(b)
                }
            }
            Binary::Shl => a.checked_shl(count),
            Binary::Shr => a.checked_shr(count),
            Binary::BitAnd => Some(a & b),
            Binary::BitOr => Some(a | b),
            Binary::BitXor => Some(a ^ b),
        }
        .ok_or(NumericFailure::GuestPanic(PanicKind::Overflow))?
    };
    store
        .wide_integer_value(TypeId(left.type_id), bits)
        .map_err(Into::into)
}
pub(crate) fn unary_in(store: &mut GuestStore, op: Unary, value: ValueRepr) -> Result<ValueRepr> {
    if value.kind != 17 {
        return unary(op, value);
    }
    unary_type(op, TypeId(value.type_id))?;
    let bits = store.wide_integer_bits(value)?;
    let result = match op {
        Unary::Not => !bits,
        Unary::Neg => (bits as i128)
            .checked_neg()
            .ok_or(NumericFailure::GuestPanic(PanicKind::Overflow))? as u128,
    };
    store
        .wide_integer_value(TypeId(value.type_id), result)
        .map_err(Into::into)
}
pub(crate) fn cast_in(
    store: &mut GuestStore,
    value: ValueRepr,
    target: TypeId,
) -> Result<ValueRepr> {
    if value.kind != 17 && !matches!(target.0, 17 | 18) {
        return cast(value, target);
    }
    cast_types(TypeId(value.type_id), target)?;
    // Rust's built-in casts define truncation, sign extension and float-to-int
    // saturation. The host i128/u128 types have those exact scalar semantics.
    macro_rules! convert {
        ($value:expr) => {{
            let source = $value;
            match target.0 {
                4 => ValueRepr::signed(target, source as i8 as i64)?,
                5 => ValueRepr::signed(target, source as i16 as i64)?,
                6 => ValueRepr::signed(target, source as i32 as i64)?,
                7 | 8 => ValueRepr::signed(target, source as i64)?,
                9 => ValueRepr::unsigned(target, source as u8 as u64)?,
                10 => ValueRepr::unsigned(target, source as u16 as u64)?,
                11 => ValueRepr::unsigned(target, source as u32 as u64)?,
                12 | 13 => ValueRepr::unsigned(target, source as u64)?,
                14 => ValueRepr::f32(source as f32),
                15 => ValueRepr::f64(source as f64),
                17 => store.wide_integer_value(target, source as i128 as u128)?,
                18 => store.wide_integer_value(target, source as u128)?,
                _ => return Err(op_type()),
            }
        }};
    }
    Ok(match value.kind {
        17 if value.type_id == 17 => convert!(store.wide_integer_bits(value)? as i128),
        17 => convert!(store.wide_integer_bits(value)?),
        4 => {
            check(value)?;
            convert!(value.payload as i64)
        }
        5 | 2 | 3 => {
            check(value)?;
            convert!(value.payload)
        }
        6 => {
            check(value)?;
            convert!(f32::from_bits(value.payload as u32))
        }
        7 => {
            check(value)?;
            convert!(f64::from_bits(value.payload))
        }
        _ => return Err(op_type()),
    })
}
