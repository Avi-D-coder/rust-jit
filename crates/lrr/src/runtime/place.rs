//! Checked address recipes. They contain guest identities, never host pointers.
use super::{
    arena::resource,
    cell::{CellId, ObjectId, RefId},
    shape::TypeId,
    value::ValueRepr,
};
use crate::core::diagnostic::Diagnostic;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PlacePanic {
    /// rustc: `index out of bounds: the len is {len} but the index is {index}`.
    IndexOutOfBounds { len: u64, index: u64 },
    /// rustc's slice range checks (`core::slice::index`):
    /// `range end index {end} out of range for slice of length {len}`.
    RangeEndOutOfBounds { end: u64, len: u64 },
    /// `range start index {start} out of range for slice of length {len}`.
    RangeStartOutOfBounds { start: u64, len: u64 },
    /// `slice index starts at {start} but ends at {end}`.
    RangeOrder { start: u64, end: u64 },
}
impl PlacePanic {
    /// The guest panic message rustc's bounds checks format.
    pub fn message(self) -> String {
        match self {
            Self::IndexOutOfBounds { len, index } => {
                format!("index out of bounds: the len is {len} but the index is {index}")
            }
            Self::RangeEndOutOfBounds { end, len } => {
                format!("range end index {end} out of range for slice of length {len}")
            }
            Self::RangeStartOutOfBounds { start, len } => {
                format!("range start index {start} out of range for slice of length {len}")
            }
            Self::RangeOrder { start, end } => {
                format!("slice index starts at {start} but ends at {end}")
            }
        }
    }
}
#[derive(Debug)]
pub enum PlaceFailure {
    RuntimeFault(Diagnostic),
    GuestPanic(PlacePanic),
}
impl From<Diagnostic> for PlaceFailure {
    fn from(error: Diagnostic) -> Self {
        Self::RuntimeFault(error)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PlaceRoot {
    Cell(CellId),
    Deref(RefId),
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Projection {
    Field(u32),
    SourceField(SourceFieldProjection),
    Tuple(u32),
    Index(u32),
    /// Element window `start..end` of an array place or of a slice window
    /// (`a[1..3]`, `s[..2]`). The projected place is unsized (`[T]`): it can
    /// be borrowed or indexed further, never read or assigned as a value.
    Range { start: u32, end: u32 },
    /// `*b` of a `Box<T>` place (rustc MIR's `Deref` projection on a box):
    /// the box object's single owned heap cell holding its `T`.
    BoxContents,
}
/// Opaque selected nominal member facts, never derived from a runtime layout.
/// The checked source-member projection operation is the only constructor.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SourceFieldProjection {
    pub(super) definition: crate::core::digest::Digest,
    pub(super) schema: crate::core::digest::Digest,
    pub(super) variant: u32,
    pub(super) field: u32,
    /// Worker-local exact descriptor admitted from the source field proof.
    /// It is never a persistent recipe identity or a source lookup result.
    pub(super) ty: TypeId,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct SourceProjectionStep {
    pub chain_index: u32,
    pub member: SourceFieldProjection,
}
pub(crate) fn checked_source_projection_bytes(count: usize) -> Result<usize, Diagnostic> {
    count
        .checked_mul(std::mem::size_of::<SourceProjectionStep>())
        .ok_or_else(resource)
}
pub struct Place {
    pub root: PlaceRoot,
    pub projections: Box<[Projection]>,
}
impl Place {
    pub(crate) fn projection_bytes(&self) -> Result<usize, Diagnostic> {
        checked_projection_bytes(self.projections.len())
    }
    /// Copy only bounded guest address facts. No resolved host address is retained.
    pub(crate) fn projected(&self, projection: Projection) -> Result<Self, Diagnostic> {
        let count = self.projections.len().checked_add(1).ok_or_else(resource)?;
        checked_projection_bytes(count)?;
        let mut projections = Vec::new();
        projections
            .try_reserve_exact(count)
            .map_err(|_| resource())?;
        projections.extend_from_slice(&self.projections);
        projections.push(projection);
        checked_projection_bytes(projections.capacity())?;
        Ok(Self {
            root: self.root,
            projections: projections.into_boxed_slice(),
        })
    }
    pub fn cell(cell: CellId) -> Self {
        Self {
            root: PlaceRoot::Cell(cell),
            projections: Box::new([]),
        }
    }
    pub fn deref(reference: RefId) -> Self {
        Self {
            root: PlaceRoot::Deref(reference),
            projections: Box::new([]),
        }
    }
}
fn checked_projection_bytes(count: usize) -> Result<usize, Diagnostic> {
    count
        .checked_mul(std::mem::size_of::<Projection>())
        .ok_or_else(resource)
}
/// Freeze an evaluated index only after checking its actual usize identity.
/// The source evaluator remains responsible for operand order and bounds.
pub fn checked_index(value: ValueRepr) -> Result<u64, Diagnostic> {
    if value.kind != 5 || value.type_id != 13 {
        return Err(Diagnostic::new(
            "LRR-E-INDEX-TYPE",
            "place index requires usize".into(),
        ));
    }
    value.validate_scalar()?;
    Ok(value.payload)
}
/// An evaluated `core::ops::Range*` index operand in rustc's `SliceIndex`
/// form: `end: None` is the slice length (`a..`, `..`), `inclusive` marks
/// `a..=b` / `..=b`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SliceRange {
    pub start: u64,
    pub end: Option<u64>,
    pub inclusive: bool,
}
/// rustc's `slice_index_fail` (library/core/src/slice/index.rs): the first
/// violated bound selects the message, start before end before order.
fn slice_index_fail(start: u64, end: u64, len: u64) -> PlacePanic {
    if start > len {
        PlacePanic::RangeStartOutOfBounds { start, len }
    } else if end > len {
        PlacePanic::RangeEndOutOfBounds { end, len }
    } else if start > end {
        PlacePanic::RangeOrder { start, end }
    } else {
        // Only an inclusive range with `end == len` reaches this.
        PlacePanic::RangeEndOutOfBounds { end, len }
    }
}
/// The checked element window `(start, end)` a range selects from a slice of
/// `len` elements, with rustc's `SliceIndex<[T]>` bounds semantics.
pub fn checked_slice_range(range: SliceRange, len: u64) -> Result<(u64, u64), PlacePanic> {
    let SliceRange {
        start,
        end,
        inclusive,
    } = range;
    match (end, inclusive) {
        (None, _) => {
            if start > len {
                return Err(slice_index_fail(start, len, len));
            }
            Ok((start, len))
        }
        (Some(end), false) => {
            if start <= end && end <= len {
                Ok((start, end))
            } else {
                Err(slice_index_fail(start, end, len))
            }
        }
        (Some(end), true) => {
            if end < len {
                let end = end + 1;
                if start <= end {
                    return Ok((start, end));
                }
                return Err(slice_index_fail(start, end, len));
            }
            Err(slice_index_fail(start, end, len))
        }
    }
}
/// Conversion follows the bounds check so a huge usize is a bounds fault.
pub fn checked_array_index(value: ValueRepr, length: u32) -> Result<Projection, PlaceFailure> {
    let index = checked_index(value)?;
    if index >= u64::from(length) {
        return Err(PlaceFailure::GuestPanic(PlacePanic::IndexOutOfBounds {
            len: u64::from(length),
            index,
        }));
    }
    Ok(Projection::Index(
        u32::try_from(index).map_err(|_| resource())?,
    ))
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct CellStamp {
    pub cell: CellId,
    pub epoch: u32,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct EnclosingObject {
    pub parent: CellStamp,
    pub object: ObjectId,
    pub field: u32,
    pub child: CellId,
    pub variant: Option<u32>,
}
pub(crate) struct ResolvedPlace {
    pub root: CellStamp,
    pub enclosing: Vec<EnclosingObject>,
    pub target: CellStamp,
    pub ty: TypeId,
    pub writable: bool,
    pub source_projections: Vec<SourceProjectionStep>,
    /// `Some((start, len))`: the place is the unsized element window of the
    /// array at `target`, and `ty` is its `[T]` descriptor.
    pub window: Option<(u32, u32)>,
    /// The place was reached through an interior-mutable `&mut` (see
    /// `ReferenceRecord::interior`): reborrows of it keep that writability.
    pub interior: bool,
}
pub(crate) fn checked_chain_bytes(count: usize) -> Result<usize, Diagnostic> {
    count
        .checked_mul(std::mem::size_of::<EnclosingObject>())
        .ok_or_else(resource)
}
/// Grow geometrically. Repeated exact one-element reservations make a deep
/// guest ownership chain quadratic even when every lookup is constant time.
pub(crate) fn reserve_chain_link(chain: &mut Vec<EnclosingObject>) -> Result<(), Diagnostic> {
    let required = chain.len().checked_add(1).ok_or_else(resource)?;
    checked_chain_bytes(required)?;
    if required > chain.capacity() {
        let capacity = chain
            .capacity()
            .checked_mul(2)
            .ok_or_else(resource)?
            .max(4)
            .max(required);
        checked_chain_bytes(capacity)?;
        chain
            .try_reserve_exact(capacity.checked_sub(chain.len()).ok_or_else(resource)?)
            .map_err(|_| resource())?;
        checked_chain_bytes(chain.capacity())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const STOCK_EMPTY_INDEX: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/array_const_index-1.rs"
    );
    #[test]
    fn rustc_empty_array_index_checks_usize_identity_and_bounds_before_conversion() {
        let (length, index) = crate::syntax::place_fixture_facts::empty_index(STOCK_EMPTY_INDEX);
        for value in [
            ValueRepr::unsigned(TypeId(13), index).unwrap(),
            ValueRepr::unsigned(TypeId(13), u64::MAX).unwrap(),
        ] {
            assert!(matches!(
                checked_array_index(value, length),
                Err(PlaceFailure::GuestPanic(PlacePanic::IndexOutOfBounds { len, .. }))
                    if len == u64::from(length)
            ));
        }
        for value in [
            ValueRepr::unsigned(TypeId(12), index).unwrap(),
            ValueRepr::signed(TypeId(8), i64::try_from(index).unwrap()).unwrap(),
        ] {
            let Err(PlaceFailure::RuntimeFault(error)) = checked_array_index(value, length) else {
                panic!("wrong index identity must fault")
            };
            assert_eq!(error.code, "LRR-E-INDEX-TYPE");
        }
        // Literal metadata is exercised; A/B evaluation/main remain unexecuted.
    }
}
