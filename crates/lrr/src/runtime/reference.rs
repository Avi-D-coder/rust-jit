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
/// Exact cell-chain provenance; buffer leases never fabricate cell endpoints.
pub(crate) struct LocationLease {
    pub(crate) root: CellStamp,
    pub(crate) enclosing: Vec<EnclosingObject>,
    pub(crate) final_cell: CellStamp,
    pub(crate) source_projections: Vec<SourceProjectionStep>,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct BufferLease {
    pub(crate) backing: super::cell::str_storage::ImmutableStrId,
    pub(crate) start: usize,
    pub(crate) end: usize,
}
pub(crate) enum ReferenceLease {
    Location(LocationLease),
    Buffer(BufferLease),
}
/// Fields stay private to the checked store; callers cannot forge a lease.
pub struct ReferenceRecord {
    pub(crate) access: Access,
    // Shared leases are reusable; a mutable reference value has one transfer owner.
    pub(super) owner: Option<super::cell::ValueOwner>,
    pub(crate) target: TypeId,
    pub(crate) lease: ReferenceLease,
}
impl ReferenceRecord {
    pub(crate) fn from_place(place: ResolvedPlace, access: Access) -> Self {
        Self {
            access,
            owner: None,
            target: place.ty,
            lease: ReferenceLease::Location(LocationLease {
                root: place.root,
                enclosing: place.enclosing,
                final_cell: place.target,
                source_projections: place.source_projections,
            }),
        }
    }
    pub(crate) fn from_immutable_buffer(lease: BufferLease) -> Self {
        Self {
            access: Access::Shared,
            owner: None,
            target: TypeId(16),
            lease: ReferenceLease::Buffer(lease),
        }
    }
    pub(crate) fn location(&self) -> Result<&LocationLease, Diagnostic> {
        match &self.lease {
            ReferenceLease::Location(location) => Ok(location),
            ReferenceLease::Buffer(_) => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "unsized buffer reference requires its buffer operation/source contract".into(),
            )),
        }
    }
    #[cfg(test)]
    pub(crate) fn location_mut(&mut self) -> Result<&mut LocationLease, Diagnostic> {
        match &mut self.lease {
            ReferenceLease::Location(location) => Ok(location),
            ReferenceLease::Buffer(_) => Err(access_fault()),
        }
    }
    pub(crate) fn owned_bytes(&self) -> Result<usize, Diagnostic> {
        // GuestArena accounts for the record including its fixed-size enum.
        match &self.lease {
            ReferenceLease::Location(location) => {
                checked_chain_bytes(location.enclosing.capacity())?
                    .checked_add(checked_source_projection_bytes(
                        location.source_projections.capacity(),
                    )?)
                    .filter(|bytes| *bytes <= 32 * 1024 * 1024)
                    .ok_or_else(super::arena::resource)
            }
            ReferenceLease::Buffer(_) => Ok(0),
        }
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
