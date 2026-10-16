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
    IndexOutOfBounds,
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
        .filter(|bytes| *bytes <= 32 * 1024 * 1024)
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
        .filter(|bytes| *bytes <= 32 * 1024 * 1024)
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
/// Conversion follows the bounds check so a huge usize is a bounds fault.
pub fn checked_array_index(value: ValueRepr, length: u32) -> Result<Projection, PlaceFailure> {
    let index = checked_index(value)?;
    if index >= u64::from(length) {
        return Err(PlaceFailure::GuestPanic(PlacePanic::IndexOutOfBounds));
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
}
pub(crate) fn checked_chain_bytes(count: usize) -> Result<usize, Diagnostic> {
    count
        .checked_mul(std::mem::size_of::<EnclosingObject>())
        .filter(|bytes| *bytes <= 32 * 1024 * 1024)
        .ok_or_else(resource)
}
/// Grow in bounded batches. Repeated exact one-element reservations make a deep
/// guest ownership chain quadratic even when every lookup is constant time.
pub(crate) fn reserve_chain_link(chain: &mut Vec<EnclosingObject>) -> Result<(), Diagnostic> {
    let required = chain.len().checked_add(1).ok_or_else(resource)?;
    checked_chain_bytes(required)?;
    if required > chain.capacity() {
        let maximum = (32 * 1024 * 1024) / std::mem::size_of::<EnclosingObject>();
        let capacity = chain
            .capacity()
            .checked_mul(2)
            .ok_or_else(resource)?
            .max(4)
            .max(required)
            .min(maximum);
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
                Err(PlaceFailure::GuestPanic(PlacePanic::IndexOutOfBounds))
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
