//! Implicit reborrows at coercion sites.
//!
//! rustc coerces a `&mut T` place used where a reference is expected (a call
//! argument, a typed `let`, a struct field initializer, a method argument) by
//! reborrowing it (`&mut *r`, or `&*r` for a `&T` expectation) instead of
//! moving it, so the original binding stays usable afterwards. The ordinary
//! local read applies this when its destination's retained expectation (the
//! parameter/annotation/field source view, or its expected descriptor) is a
//! reference: the result is a fresh lease of the same referent (for a slice
//! reference, the same element window), formed by the ordinary borrow. A
//! destination without a reference expectation (a generic `T`, an
//! unannotated `let`) still moves the reference, as in rustc.
use super::Frame;
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::{NodeId, SlotId},
    runtime::{
        cell::{CellId, GuestStore},
        place::{Place, PlaceFailure},
        reference::Access,
        shape::TypeKey,
        value::ValueRepr,
    },
    traits::term::TyTerm,
};

fn place_fault(failure: PlaceFailure) -> Diagnostic {
    match failure {
        PlaceFailure::RuntimeFault(error) => error,
        PlaceFailure::GuestPanic(_) => super::invalid("reborrow place panicked"),
    }
}

impl Frame {
    /// The access of the implicit reborrow a `&mut` value takes when it is
    /// read into `destination`, or `None` when the read moves it.
    pub(crate) fn reborrow_access(
        &self,
        destination: SlotId,
        value: ValueRepr,
        store: &GuestStore,
    ) -> Result<Option<Access>, Diagnostic> {
        if value.kind != 9 || !store.reference_is_mutable(value)? {
            return Ok(None);
        }
        self.reborrow_expectation(destination, store)
    }
    /// The access a reference-typed coercion expectation of `destination`
    /// selects for an implicit reborrow, or `None` when it has none.
    fn reborrow_expectation(
        &self,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<Option<Access>, Diagnostic> {
        // A body result is returned (moved), never reborrowed from a local
        // whose frame ends with the return.
        if Some(destination) == self.body.nodes()[0].value {
            return Ok(None);
        }
        let slot = self.slot(destination)?;
        let expected = match slot.source.as_ref() {
            Some(TyTerm::Ref(_, mutable, _)) => Some(*mutable),
            Some(_) => None,
            None => match slot.hint {
                Some(ty) => match store.type_descriptor(ty)?.key.as_ref() {
                    TypeKey::Ref { mutable, .. } => Some(*mutable),
                    _ => None,
                },
                None => slot.expects_reference,
            },
        };
        Ok(expected.map(|mutable| {
            if mutable {
                Access::Mutable
            } else {
                Access::Shared
            }
        }))
    }
    /// Admission of a reborrow read of `cell` without effects.
    pub(crate) fn preflight_reborrow(
        &self,
        cell: CellId,
        destination: SlotId,
        access: Access,
        source: Option<&TyTerm>,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let place = Place::deref(store.reference_from_cell(cell)?);
        store
            .admit_place_borrow(&place, access)
            .map_err(place_fault)?;
        self.preflight_temporary_slot(destination)?;
        let view = Self::reborrow_view(source, access);
        self.preflight_source_view(destination, view.as_ref())
    }
    /// `&mut *r` / `&*r` of the local `binding` (or a parameter, `None`)
    /// into `destination`; the local keeps its own lease.
    pub(crate) fn reborrow_read(
        &mut self,
        binding: Option<NodeId>,
        cell: CellId,
        destination: SlotId,
        access: Access,
        source: Option<TyTerm>,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_reborrow(cell, destination, access, source.as_ref(), store)?;
        let place = Place::deref(store.reference_from_cell(cell)?);
        store
            .reserve_place_borrow_result(&place, self.slot(destination)?.cell.is_none())
            .map_err(place_fault)?;
        self.prepare_write(destination, store)?;
        let ticket = store
            .borrow_ticket(&place, access)
            .map_err(place_fault)?;
        self.write(destination, ticket, store)?;
        if let Some(binding) = binding
            && self.binding_sources[binding.0 as usize].is_none()
        {
            self.set_binding_source_view(binding, self.clone_source(source.as_ref())?)?;
        }
        let view = Self::reborrow_view(source.as_ref(), access);
        self.set_source_view(destination, view)
    }
    /// The reborrow a `&mut` value read from the projected `place` (`w.r`,
    /// `*rr`) takes at `destination`: the place of its referent and the
    /// access. `None` reads the place by value; a place that cannot be
    /// inspected also takes the ordinary read, which reports its fault.
    pub(super) fn place_reborrow(
        &self,
        place: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<Option<(Place, Access)>, PlaceFailure> {
        let executed = self.executed_place(place)?;
        let Ok(ty) = store.live_place_type_id(executed) else {
            return Ok(None);
        };
        if !matches!(
            store.type_descriptor(ty)?.key.as_ref(),
            TypeKey::Ref { mutable: true, .. }
        ) {
            return Ok(None);
        }
        let Some(access) = self.reborrow_expectation(destination, store)? else {
            return Ok(None);
        };
        Ok(Some((store.dereference_place(executed)?, access)))
    }
    /// Admission of a reborrow read of a projected place, or `false` when
    /// the place is read by value.
    pub(crate) fn preflight_place_reborrow(
        &self,
        place: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<bool, PlaceFailure> {
        let Some((referent, access)) = self.place_reborrow(place, destination, store)? else {
            return Ok(false);
        };
        store.admit_place_borrow(&referent, access)?;
        self.preflight_temporary_slot(destination)?;
        let view = Self::reborrow_view(self.place_slot(place)?.source.as_ref(), access);
        self.preflight_source_view(destination, view.as_ref())?;
        Ok(true)
    }
    /// `&mut *place` / `&*place` into `destination`; the place keeps its
    /// reference. `false` when the place is read by value instead.
    pub(crate) fn place_reborrow_read(
        &mut self,
        place: SlotId,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<bool, PlaceFailure> {
        if !self.preflight_place_reborrow(place, destination, store)? {
            return Ok(false);
        }
        let (referent, access) = self
            .place_reborrow(place, destination, store)?
            .ok_or_else(|| super::invalid("admitted place reborrow changed"))?;
        let view = Self::reborrow_view(self.place_slot(place)?.source.as_ref(), access);
        store.reserve_place_borrow_result(&referent, self.slot(destination)?.cell.is_none())?;
        self.prepare_write(destination, store)?;
        let ticket = store.borrow_ticket(&referent, access)?;
        self.write(destination, ticket, store)?;
        self.set_source_view(destination, view)?;
        self.clear_place(place)?;
        Ok(true)
    }
    /// The reborrow's view: the local's `&mut T` view at the reborrow access.
    fn reborrow_view(source: Option<&TyTerm>, access: Access) -> Option<TyTerm> {
        match source {
            Some(TyTerm::Ref(region, _, pointee)) => Some(TyTerm::Ref(
                region.clone(),
                access == Access::Mutable,
                pointee.clone(),
            )),
            _ => None,
        }
    }
}
