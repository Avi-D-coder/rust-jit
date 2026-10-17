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
/// `&[T]`/`&mut [T]`: a contiguous element window `start..start + len` of
/// one array location. The base lease names the array place exactly (its
/// whole ownership chain); every resolution rechecks the window against the
/// array's own retained length, so a stale or forged window cannot address
/// outside the borrowed elements.
pub(crate) struct ElementsLease {
    pub(crate) base: LocationLease,
    pub(crate) start: u32,
    pub(crate) len: u32,
}
pub(crate) enum ReferenceLease {
    Location(LocationLease),
    Buffer(BufferLease),
    Elements(ElementsLease),
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
        let base = LocationLease {
            root: place.root,
            enclosing: place.enclosing,
            final_cell: place.target,
            source_projections: place.source_projections,
        };
        Self {
            access,
            owner: None,
            target: place.ty,
            // A resolved element window (`&a[1..3]`, `&*s`) borrows exactly
            // those elements of its array location.
            lease: match place.window {
                Some((start, len)) => ReferenceLease::Elements(ElementsLease { base, start, len }),
                None => ReferenceLease::Location(base),
            },
        }
    }
    /// The unsizing coercion `&[T; N]` -> `&[T]` (and `&mut`): the same
    /// array location viewed as its complete element window.
    pub(crate) fn unsized_array(
        &self,
        access: Access,
        slice: TypeId,
        len: u32,
    ) -> Result<Self, Diagnostic> {
        let location = self.location()?;
        let mut enclosing = Vec::new();
        enclosing
            .try_reserve_exact(location.enclosing.len())
            .map_err(|_| super::arena::resource())?;
        enclosing.extend_from_slice(&location.enclosing);
        let mut source_projections = Vec::new();
        source_projections
            .try_reserve_exact(location.source_projections.len())
            .map_err(|_| super::arena::resource())?;
        source_projections.extend_from_slice(&location.source_projections);
        Ok(Self {
            access,
            owner: None,
            target: slice,
            lease: ReferenceLease::Elements(ElementsLease {
                base: LocationLease {
                    root: location.root,
                    enclosing,
                    final_cell: location.final_cell,
                    source_projections,
                },
                start: 0,
                len,
            }),
        })
    }
    /// The array location a slice reference windows.
    pub(crate) fn elements(&self) -> Option<&ElementsLease> {
        match &self.lease {
            ReferenceLease::Elements(elements) => Some(elements),
            _ => None,
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
            ReferenceLease::Elements(_) => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "slice reference requires its element-window operation".into(),
            )),
        }
    }
    #[cfg(test)]
    pub(crate) fn location_mut(&mut self) -> Result<&mut LocationLease, Diagnostic> {
        match &mut self.lease {
            ReferenceLease::Location(location) => Ok(location),
            ReferenceLease::Buffer(_) | ReferenceLease::Elements(_) => Err(access_fault()),
        }
    }
    pub(crate) fn owned_bytes(&self) -> Result<usize, Diagnostic> {
        // GuestArena accounts for the record including its fixed-size enum.
        match &self.lease {
            ReferenceLease::Location(location)
            | ReferenceLease::Elements(ElementsLease { base: location, .. }) => {
                checked_chain_bytes(location.enclosing.capacity())?
                    .checked_add(checked_source_projection_bytes(
                        location.source_projections.capacity(),
                    )?)
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
