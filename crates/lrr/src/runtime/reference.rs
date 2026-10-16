//! Run-owned location leases. Shared access never grants mutation permission.
pub use super::cell::RefId;
use super::{
    place::{
        CellStamp, EnclosingObject, ResolvedPlace, SourceProjectionStep, checked_chain_bytes,
        checked_source_projection_bytes,
    },
    shape::TypeId,
};
use crate::core::diagnostic::Diagnostic;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Access {
    Shared,
    Mutable,
}
/// Fields stay private to the checked store; callers cannot forge a lease.
pub struct ReferenceRecord {
    pub(crate) access: Access,
    // Shared leases are reusable; a mutable reference value has one transfer owner.
    pub(super) owner: Option<super::cell::ValueOwner>,
    pub(crate) target: TypeId,
    pub(crate) root: CellStamp,
    pub(crate) enclosing: Vec<EnclosingObject>,
    pub(crate) final_cell: CellStamp,
    pub(crate) source_projections: Vec<SourceProjectionStep>,
}
impl ReferenceRecord {
    pub(crate) fn from_place(place: ResolvedPlace, access: Access) -> Self {
        Self {
            access,
            owner: None,
            target: place.ty,
            root: place.root,
            enclosing: place.enclosing,
            final_cell: place.target,
            source_projections: place.source_projections,
        }
    }
    pub(crate) fn owned_bytes(&self) -> Result<usize, Diagnostic> {
        // GuestArena accounts for the record itself; charge only its owned buffer.
        checked_chain_bytes(self.enclosing.capacity())?
            .checked_add(checked_source_projection_bytes(
                self.source_projections.capacity(),
            )?)
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(super::arena::resource)
    }
}
pub(crate) fn access_fault() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-REFERENCE-ACCESS",
        "reference access requires a writable location and mutable access".into(),
    )
}
pub(crate) fn lease_fault() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-STALE-HANDLE",
        "reference location lease is stale".into(),
    )
}
