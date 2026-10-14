//! Concrete primitive operations. No literal defaulting, guest cleanup or host panic.
pub use crate::ir::op::{NumericBinary as Binary, NumericUnary as Unary};
use crate::{
    core::diagnostic::Diagnostic,
    runtime::{
        shape::{Primitive, TypeId, primitive},
        value::ValueRepr,
    },
};
#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PanicKind {
    Overflow,
    DivisionByZero,
    InvalidShift,
    Explicit,
    Assertion,
    IndexOutOfBounds,
}
impl PanicKind {
    pub fn description(self) -> &'static str {
        match self {
            Self::Overflow => "arithmetic overflow",
            Self::DivisionByZero => "division by zero",
            Self::InvalidShift => "invalid shift",
            Self::Explicit => "explicit panic",
            Self::Assertion => "assertion failed",
            Self::IndexOutOfBounds => "index out of bounds",
        }
    }
}
#[derive(Debug)]
pub enum NumericFailure {
    GuestPanic(PanicKind),
    RuntimeFault(Diagnostic),
}
impl From<Diagnostic> for NumericFailure {
    fn from(error: Diagnostic) -> Self {
        Self::RuntimeFault(error)
    }
}
type Result<T> = std::result::Result<T, NumericFailure>;
fn op_type() -> NumericFailure {
    NumericFailure::RuntimeFault(Diagnostic::new(
        "LRR-E-OP-TYPE",
        "operator requires supported concrete primitive operand descriptors".into(),
    ))
}
fn check(value: ValueRepr) -> Result<()> {
    if value.kind == 11 || value.kind == 12 {
        return Err(NumericFailure::RuntimeFault(Diagnostic::new(
            "LRR-E-TYPE-CONTEXT",
            "numeric operation requires an explicit suffix or reached concrete context".into(),
        )));
    }
    if !matches!(value.kind, 1..=7) {
        return Err(op_type());
    }
    value.validate_scalar().map_err(Into::into)
}
/// Read-only primitive forwarding for the core `forward_ref_*op` implementations.
/// One shared lease is validated and read by Copy; owners, epochs and source
/// descriptors stay unchanged. Comparisons and casts do not call this adjustment.
pub(crate) fn borrowed_operand(
    store: &crate::runtime::cell::GuestStore,
    cell: crate::runtime::cell::CellId,
) -> std::result::Result<Option<ValueRepr>, Diagnostic> {
    use crate::runtime::{cell::CellState, shape::TypeKey};
    let CellState::Live(value) = store.state(cell)? else {
        return Err(crate::runtime::value::invalid());
    };
    if value.kind != 9 {
        return Ok(None);
    }
    let TypeKey::Ref {
        mutable: false,
        target,
    } = store.type_descriptor(TypeId(value.type_id))?.key.as_ref()
    else {
        return Err(Diagnostic::new(
            "LRR-E-OP-TYPE",
            "primitive operator requires one shared primitive reference".into(),
        ));
    };
    if !matches!(
        primitive(*target),
        Ok(Primitive::Bool
            | Primitive::Signed(_)
            | Primitive::Unsigned(_)
            | Primitive::F32
            | Primitive::F64)
    ) {
        return Err(Diagnostic::new(
            "LRR-E-OP-TYPE",
            "primitive reference forwarding requires a primitive operator target".into(),
        ));
    }
    let reference = store.reference_from_cell(cell)?;
    let scalar = store.read_reference(reference)?;
    if scalar.type_id != target.0 {
        return Err(crate::runtime::value::invalid());
    }
    Ok(Some(scalar))
}

fn shift_count(value: ValueRepr, width: u8) -> Result<u32> {
    let count = match value.kind {
        4 => {
            let n = value.payload as i64;
            if n < 0 {
                return Err(NumericFailure::GuestPanic(PanicKind::InvalidShift));
            }
            n as u64
        }
        5 => value.payload,
        _ => return Err(op_type()),
    };
    if count >= u64::from(width) {
        Err(NumericFailure::GuestPanic(PanicKind::InvalidShift))
    } else {
        Ok(count as u32)
    }
}
macro_rules! integer {
    ($operator:expr,$left:expr,$right:expr,$ty:ty,$construct:ident) => {{
        let a = $left.payload as $ty;
        let b = $right.payload as $ty;
        let value = match $operator {
            Binary::Eq => return Ok(ValueRepr::boolean(a == b)),
            Binary::Ne => return Ok(ValueRepr::boolean(a != b)),
            Binary::Lt => return Ok(ValueRepr::boolean(a < b)),
            Binary::Le => return Ok(ValueRepr::boolean(a <= b)),
            Binary::Gt => return Ok(ValueRepr::boolean(a > b)),
            Binary::Ge => return Ok(ValueRepr::boolean(a >= b)),
            Binary::Add => a.checked_add(b),
            Binary::Sub => a.checked_sub(b),
            Binary::Mul => a.checked_mul(b),
            Binary::Div | Binary::Rem => {
                if b == 0 {
                    return Err(NumericFailure::GuestPanic(PanicKind::DivisionByZero));
                }
                if $operator == Binary::Div {
                    a.checked_div(b)
                } else {
                    a.checked_rem(b)
                }
            }
            Binary::Shl => a.checked_shl(shift_count($right, <$ty>::BITS as u8)?),
            Binary::Shr => a.checked_shr(shift_count($right, <$ty>::BITS as u8)?),
            Binary::BitAnd => Some(a & b),
            Binary::BitOr => Some(a | b),
            Binary::BitXor => Some(a ^ b),
        }
        .ok_or(NumericFailure::GuestPanic(PanicKind::Overflow))?;
        ValueRepr::$construct(TypeId($left.type_id), value as _).map_err(Into::into)
    }};
}
macro_rules! float {
    ($operator:expr,$left:expr,$right:expr,$ty:ty,$construct:ident) => {{
        let a = <$ty>::from_bits($left.payload as _);
        let b = <$ty>::from_bits($right.payload as _);
        match $operator {
            Binary::Eq => Ok(ValueRepr::boolean(a == b)),
            Binary::Ne => Ok(ValueRepr::boolean(a != b)),
            Binary::Lt => Ok(ValueRepr::boolean(a < b)),
            Binary::Le => Ok(ValueRepr::boolean(a <= b)),
            Binary::Gt => Ok(ValueRepr::boolean(a > b)),
            Binary::Ge => Ok(ValueRepr::boolean(a >= b)),
            Binary::Add => Ok(ValueRepr::$construct(a + b)),
            Binary::Sub => Ok(ValueRepr::$construct(a - b)),
            Binary::Mul => Ok(ValueRepr::$construct(a * b)),
            Binary::Div => Ok(ValueRepr::$construct(a / b)),
            Binary::Rem => Ok(ValueRepr::$construct(a % b)),
            _ => Err(op_type()),
        }
    }};
}
pub(crate) fn binary_types(operator: Binary, left: TypeId, right: TypeId) -> Result<()> {
    let left_kind = primitive(left).map_err(|_| op_type())?;
    let right_kind = primitive(right).map_err(|_| op_type())?;
    if matches!(operator, Binary::Shl | Binary::Shr) {
        return if matches!(left_kind, Primitive::Signed(_) | Primitive::Unsigned(_))
            && matches!(right_kind, Primitive::Signed(_) | Primitive::Unsigned(_))
        {
            Ok(())
        } else {
            Err(op_type())
        };
    }
    if left != right {
        return Err(op_type());
    }
    let comparison = matches!(
        operator,
        Binary::Eq | Binary::Ne | Binary::Lt | Binary::Le | Binary::Gt | Binary::Ge
    );
    let bitwise = matches!(operator, Binary::BitAnd | Binary::BitOr | Binary::BitXor);
    let supported = match left_kind {
        Primitive::Signed(_) | Primitive::Unsigned(_) => true,
        Primitive::F32 | Primitive::F64 => {
            comparison
                || matches!(
                    operator,
                    Binary::Add | Binary::Sub | Binary::Mul | Binary::Div | Binary::Rem
                )
        }
        Primitive::Bool => comparison || bitwise,
        Primitive::Char | Primitive::Unit => comparison,
        _ => false,
    };
    if supported { Ok(()) } else { Err(op_type()) }
}
pub(crate) fn unary_type(operator: Unary, value: TypeId) -> Result<()> {
    let supported = match primitive(value).map_err(|_| op_type())? {
        Primitive::Signed(_) => true,
        Primitive::Unsigned(_) | Primitive::Bool => operator == Unary::Not,
        Primitive::F32 | Primitive::F64 => operator == Unary::Neg,
        _ => false,
    };
    if supported { Ok(()) } else { Err(op_type()) }
}
pub(crate) fn cast_types(source: TypeId, destination: TypeId) -> Result<()> {
    primitive(source).map_err(|_| op_type())?;
    primitive(destination).map_err(|_| op_type())?;
    if source == destination
        || destination.0 == 3 && source.0 == 9
        || (4..=13).contains(&destination.0) && (2..=15).contains(&source.0)
        || (14..=15).contains(&destination.0) && (4..=15).contains(&source.0)
    {
        Ok(())
    } else {
        Err(op_type())
    }
}
pub fn binary(operator: Binary, left: ValueRepr, right: ValueRepr) -> Result<ValueRepr> {
    check(left)?;
    check(right)?;
    binary_types(operator, TypeId(left.type_id), TypeId(right.type_id))?;
    match (primitive(TypeId(left.type_id))?, left.kind, right.kind) {
        (Primitive::Signed(8), 4, 4 | 5) => integer!(operator, left, right, i8, signed),
        (Primitive::Signed(16), 4, 4 | 5) => integer!(operator, left, right, i16, signed),
        (Primitive::Signed(32), 4, 4 | 5) => integer!(operator, left, right, i32, signed),
        (Primitive::Signed(64), 4, 4 | 5) => integer!(operator, left, right, i64, signed),
        (Primitive::Unsigned(8), 5, 4 | 5) => integer!(operator, left, right, u8, unsigned),
        (Primitive::Unsigned(16), 5, 4 | 5) => integer!(operator, left, right, u16, unsigned),
        (Primitive::Unsigned(32), 5, 4 | 5) => integer!(operator, left, right, u32, unsigned),
        (Primitive::Unsigned(64), 5, 4 | 5) => integer!(operator, left, right, u64, unsigned),
        (Primitive::F32, 6, 6) => float!(operator, left, right, f32, f32),
        (Primitive::F64, 7, 7) => float!(operator, left, right, f64, f64),
        (Primitive::Bool, 2, 2) => {
            let a = left.payload == 1;
            let b = right.payload == 1;
            let result = match operator {
                Binary::BitAnd => a & b,
                Binary::BitOr => a | b,
                Binary::BitXor => a ^ b,
                Binary::Eq => a == b,
                Binary::Ne => a != b,
                Binary::Lt => (!a) & b,
                Binary::Le => (!a) | b,
                Binary::Gt => a & (!b),
                Binary::Ge => a | (!b),
                _ => return Err(op_type()),
            };
            Ok(ValueRepr::boolean(result))
        }
        (Primitive::Char, 3, 3) => {
            let a = left.payload;
            let b = right.payload;
            let result = match operator {
                Binary::Eq => a == b,
                Binary::Ne => a != b,
                Binary::Lt => a < b,
                Binary::Le => a <= b,
                Binary::Gt => a > b,
                Binary::Ge => a >= b,
                _ => return Err(op_type()),
            };
            Ok(ValueRepr::boolean(result))
        }
        (Primitive::Unit, 1, 1) => match operator {
            Binary::Eq | Binary::Le | Binary::Ge => Ok(ValueRepr::boolean(true)),
            Binary::Ne | Binary::Lt | Binary::Gt => Ok(ValueRepr::boolean(false)),
            _ => Err(op_type()),
        },
        _ => Err(op_type()),
    }
}
pub fn unary(operator: Unary, value: ValueRepr) -> Result<ValueRepr> {
    check(value)?;
    unary_type(operator, TypeId(value.type_id))?;
    macro_rules! signed {
        ($ty:ty) => {{
            let n = value.payload as $ty;
            let n = match operator {
                Unary::Neg => n
                    .checked_neg()
                    .ok_or(NumericFailure::GuestPanic(PanicKind::Overflow))?,
                Unary::Not => !n,
            };
            ValueRepr::signed(TypeId(value.type_id), n as i64).map_err(Into::into)
        }};
    }
    match primitive(TypeId(value.type_id))? {
        Primitive::Signed(8) => signed!(i8),
        Primitive::Signed(16) => signed!(i16),
        Primitive::Signed(32) => signed!(i32),
        Primitive::Signed(64) => signed!(i64),
        Primitive::Unsigned(width) if operator == Unary::Not => {
            let mask = if width == 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            ValueRepr::unsigned(TypeId(value.type_id), (!value.payload) & mask).map_err(Into::into)
        }
        Primitive::Bool if operator == Unary::Not => Ok(ValueRepr::boolean(value.payload == 0)),
        Primitive::F32 if operator == Unary::Neg => {
            Ok(ValueRepr::f32(-f32::from_bits(value.payload as u32)))
        }
        Primitive::F64 if operator == Unary::Neg => {
            Ok(ValueRepr::f64(-f64::from_bits(value.payload)))
        }
        _ => Err(op_type()),
    }
}

/// Primitive `as`: source validation precedes conversion; destination identity is retained.
pub fn cast(value: ValueRepr, destination: TypeId) -> Result<ValueRepr> {
    check(value)?;
    cast_types(TypeId(value.type_id), destination)?;
    let target = primitive(destination).map_err(|_| op_type())?;
    if value.type_id == destination.0 {
        return Ok(value);
    }
    if target == Primitive::Char {
        if value.kind == 5 && value.type_id == 9 {
            return Ok(ValueRepr::character(
                char::from_u32(value.payload as u32).expect("checked u8 code point"),
            ));
        }
        return Err(op_type());
    }
    macro_rules! convert {
        ($source:expr,$numeric:expr) => {{
            let source = $source;
            match destination.0 {
                4 => ValueRepr::signed(destination, (source as i8) as i64).map_err(Into::into),
                5 => ValueRepr::signed(destination, (source as i16) as i64).map_err(Into::into),
                6 => ValueRepr::signed(destination, (source as i32) as i64).map_err(Into::into),
                7 | 8 => ValueRepr::signed(destination, source as i64).map_err(Into::into),
                9 => ValueRepr::unsigned(destination, (source as u8) as u64).map_err(Into::into),
                10 => ValueRepr::unsigned(destination, (source as u16) as u64).map_err(Into::into),
                11 => ValueRepr::unsigned(destination, (source as u32) as u64).map_err(Into::into),
                12 | 13 => ValueRepr::unsigned(destination, source as u64).map_err(Into::into),
                14 if $numeric => Ok(ValueRepr::f32(source as f32)),
                15 if $numeric => Ok(ValueRepr::f64(source as f64)),
                _ => Err(op_type()),
            }
        }};
    }
    match value.kind {
        2 | 3 => convert!(value.payload, false),
        4 => convert!(value.payload as i64, true),
        5 => convert!(value.payload, true),
        6 => convert!(f32::from_bits(value.payload as u32), true),
        7 => convert!(f64::from_bits(value.payload), true),
        _ => Err(op_type()),
    }
}

#[cfg(test)]
mod borrowed_operand_tests {
    use super::*;
    use crate::runtime::{
        cell::{CellId, CellOwner, GuestStore, StorageContract},
        place::Place,
        reference::Access,
    };

    fn reference(value: ValueRepr, access: Access) -> (GuestStore, CellId, CellId) {
        let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let target = store
            .new_cell(
                StorageContract::Exact(TypeId(value.type_id)),
                true,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        let ticket = store.ticket(value).unwrap();
        store.initialize(target, ticket).unwrap();
        let lease = store.borrow(&Place::cell(target), access).unwrap();
        let ticket = store.reference_ticket(lease).unwrap();
        let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
        let operand = store
            .new_cell(
                StorageContract::Exact(ty),
                false,
                CellOwner::FrameSlot { frame, slot: 1 },
            )
            .unwrap();
        store.initialize(operand, ticket).unwrap();
        (store, target, operand)
    }

    #[test]
    fn shared_primitive_forwarding_is_a_checked_read_without_storage_effects() {
        for value in [
            ValueRepr::unsigned(TypeId(9), 42).unwrap(),
            ValueRepr::signed(TypeId(6), -7).unwrap(),
            ValueRepr::f32(0.25),
            ValueRepr::boolean(true),
        ] {
            let (store, target, operand) = reference(value, Access::Shared);
            let before = (
                store.state(target).unwrap(),
                store.state(operand).unwrap(),
                store.location_epoch(target).unwrap(),
                store.budget().handles(),
                store.budget().bytes(),
            );
            assert_eq!(borrowed_operand(&store, operand).unwrap(), Some(value));
            assert_eq!(
                before,
                (
                    store.state(target).unwrap(),
                    store.state(operand).unwrap(),
                    store.location_epoch(target).unwrap(),
                    store.budget().handles(),
                    store.budget().bytes()
                )
            );
            assert_eq!(borrowed_operand(&store, target).unwrap(), None);
        }
    }

    #[test]
    fn refused_mutable_nonoperator_and_nested_references_preserve_storage() {
        for (value, access) in [
            (ValueRepr::unsigned(TypeId(9), 42).unwrap(), Access::Mutable),
            (ValueRepr::unit(), Access::Shared),
            (ValueRepr::character('x'), Access::Shared),
        ] {
            let (store, target, operand) = reference(value, access);
            let before = (
                store.state(target).unwrap(),
                store.state(operand).unwrap(),
                store.budget().handles(),
                store.budget().bytes(),
            );
            assert_eq!(
                borrowed_operand(&store, operand).unwrap_err().code,
                "LRR-E-OP-TYPE"
            );
            assert_eq!(
                before,
                (
                    store.state(target).unwrap(),
                    store.state(operand).unwrap(),
                    store.budget().handles(),
                    store.budget().bytes()
                )
            );
        }
        let (mut store, _, operand) =
            reference(ValueRepr::signed(TypeId(6), 1).unwrap(), Access::Shared);
        let lease = store.borrow(&Place::cell(operand), Access::Shared).unwrap();
        let ticket = store.reference_ticket(lease).unwrap();
        let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
        let frame = store.new_frame_anchor().unwrap();
        let nested = store
            .new_cell(
                StorageContract::Exact(ty),
                false,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(nested, ticket).unwrap();
        assert_eq!(
            borrowed_operand(&store, nested).unwrap_err().code,
            "LRR-E-OP-TYPE"
        );
    }

    #[test]
    fn stale_primitive_lease_is_refused_before_forwarding() {
        let (mut store, target, operand) =
            reference(ValueRepr::signed(TypeId(6), 1).unwrap(), Access::Shared);
        let moved = store.move_from(target).unwrap();
        store.discard(moved).unwrap();
        let before = (
            store.state(operand).unwrap(),
            store.budget().handles(),
            store.budget().bytes(),
        );
        assert_eq!(
            borrowed_operand(&store, operand).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(
            before,
            (
                store.state(operand).unwrap(),
                store.budget().handles(),
                store.budget().bytes()
            )
        );
    }
}
