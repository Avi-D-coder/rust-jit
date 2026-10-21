//! Sized sub-array places: the `rest @ ..` of an array pattern (rustc MIR's
//! `Subslice` projection on an array). The place stays an in-place element
//! window of its array (`Projection::Subarray`, resolved with the interned
//! `[T; K]` Type). A reference borrows it as `&[T; K]` (an `Elements` lease
//! whose target is the array Type); a by-value binding builds a fresh `[T; K]`
//! from the window's elements by the ordinary per-element owning reads.
use super::*;

/// A fresh `[T; K]` being assembled from a sub-array window's elements.
pub(crate) struct SubarrayBuild {
    guard: ConstructionId,
    ty: TypeId,
}

impl Place {
    /// The element count of a place that ends in a sized sub-array window.
    pub(crate) fn subarray_len(&self) -> Option<u32> {
        match self.projections.last() {
            Some(Projection::Subarray { start, end }) => Some(end - start),
            _ => None,
        }
    }
    fn without_last_projection(&self) -> Place {
        let count = self.projections.len().saturating_sub(1);
        Place {
            root: self.root,
            projections: self.projections[..count].into(),
        }
    }
}

impl GuestStore {
    /// `base[start..end]` as a sized `[T; end - start]` window of an array
    /// place or of a sized window. The bounds are the pattern's own
    /// arithmetic over the scrutinee's actual length.
    pub(crate) fn project_subarray(
        &self,
        base: &Place,
        start: u32,
        end: u32,
    ) -> Result<Place, PlaceFailure> {
        let resolved = self.resolve_place_windowed(base, PlaceUse::Address)?;
        let available = match resolved.window {
            Some((_, len)) => len,
            None => match self.types.descriptor(resolved.ty)?.key.as_ref() {
                TypeKey::Array { len, .. } => *len,
                _ => {
                    return Err(Diagnostic::new(
                        "LRR-E-OP-TYPE",
                        "a sub-array projection requires an array place".into(),
                    )
                    .into());
                }
            },
        };
        if start > end || end > available {
            return Err(PlaceFailure::GuestPanic(PlacePanic::RangeEndOutOfBounds {
                end: u64::from(end),
                len: u64::from(available),
            }));
        }
        Ok(base.projected(Projection::Subarray { start, end })?)
    }
    /// Intern the `[T; K]` Type of a place that ends in a sized sub-array
    /// window, so the pure place resolution (a search, a guard preflight) can
    /// name it. Any other place is unchanged.
    pub(crate) fn ensure_subarray_type(&mut self, place: &Place) -> Result<(), PlaceFailure> {
        let Some(len) = place.subarray_len() else {
            return Ok(());
        };
        let resolved = self.resolve_place_windowed(&place.without_last_projection(), PlaceUse::Address)?;
        let array = self.address_type(resolved.target.cell)?;
        let Some((element, _)) = self.sequence_shape(resolved.target.cell, array)? else {
            return Err(invalid().into());
        };
        self.intern_type(TypeKey::Array { element, len })?;
        Ok(())
    }
    /// Start assembling the value of a sub-array place. Elements are pushed in
    /// order with `subarray_push`; `subarray_finish` publishes the array.
    pub(crate) fn begin_subarray_build(
        &mut self,
        place: &Place,
    ) -> Result<SubarrayBuild, PlaceFailure> {
        let len = place.subarray_len().ok_or_else(invalid)?;
        let resolved = self.resolve_place_windowed(&place.without_last_projection(), PlaceUse::Address)?;
        let array = self.address_type(resolved.target.cell)?;
        let Some((element, _)) = self.sequence_shape(resolved.target.cell, array)? else {
            return Err(invalid().into());
        };
        let ty = self.intern_type(TypeKey::Array { element, len })?;
        let members = vec![element; len as usize];
        let guard = self.begin_construction(&members)?;
        Ok(SubarrayBuild { guard, ty })
    }
    pub(crate) fn subarray_push(
        &mut self,
        build: &SubarrayBuild,
        index: u32,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        self.construction_push(build.guard, index, ticket)
    }
    pub(crate) fn subarray_finish(&mut self, build: SubarrayBuild) -> Result<TicketId, Diagnostic> {
        self.commit_construction(build.guard, build.ty)
    }
}
