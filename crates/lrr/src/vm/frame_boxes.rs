//! Builtin box dereference, frame side (see `runtime/cell_boxes.rs`).
//!
//! `Op::DerefPlace` selects the builtin dereference of its operand place by
//! the place's reached descriptor: a reference place dereferences its lease,
//! a `Box<T>` place projects the box's heap cell. The projected place's
//! source view is the box view's `T`, exactly as a reference's pointee view.
use super::*;
use crate::{
    runtime::place::PlaceFailure,
    traits::term::{GenericArg, TyTerm},
};

impl Frame {
    /// The contents view `T` of a retained `Box<T>` source view.
    pub(crate) fn box_contents_source<'a>(
        source: &'a TyTerm,
        store: &GuestStore,
    ) -> Option<&'a TyTerm> {
        match source {
            TyTerm::Nominal(definition, args) if store.owned_box() == Some(*definition) => {
                let mut types = args.iter().filter_map(|argument| match argument {
                    GenericArg::Type(ty) => Some(ty),
                    _ => None,
                });
                match (types.next(), types.next()) {
                    (Some(contents), None) => Some(contents),
                    _ => None,
                }
            }
            _ => None,
        }
    }
    /// Whether a place of reached type `ty` autoderefs to a box: a box, or
    /// a reference chain whose final referent is one.
    fn reaches_box(mut ty: TypeId, store: &GuestStore) -> Result<bool, Diagnostic> {
        loop {
            match store.type_descriptor(ty)?.key.as_ref() {
                crate::runtime::shape::TypeKey::Ref { target, .. } => ty = *target,
                _ => return Ok(store.box_contents_type(ty)?.is_some()),
            }
        }
    }
    /// rustc's field-access autoderef through boxes: replace a named member
    /// base place (`b.f`, `r.f` with `r: &Box<S>`) by its dereferenced place
    /// until no box remains on the path to the record, carrying each step's
    /// pointee view. A base that reaches no box is unchanged (the ordinary
    /// single-reference member route still applies), so repeating the step
    /// for the same operation is a no-op.
    pub(crate) fn autoderef_box_member_base(
        &mut self,
        base: SlotId,
        store: &GuestStore,
    ) -> Result<(), PlaceFailure> {
        loop {
            let place = self.executed_place(base)?;
            if !Self::reaches_box(store.place_type_id(place)?, store)? {
                return Ok(());
            }
            let dereferenced = store.dereference_place(place)?;
            let released = place.projection_bytes()?;
            let source = match self.place_slot(base)?.source.as_ref() {
                Some(TyTerm::Ref(_, _, pointee)) => self.clone_source(Some(pointee.as_ref()))?,
                Some(source) => self.clone_source(Self::box_contents_source(source, store))?,
                None => None,
            };
            self.projection_bytes = self
                .projection_bytes
                .checked_sub(released)
                .and_then(|bytes| bytes.checked_add(dereferenced.projection_bytes().ok()?))
                .ok_or_else(|| invalid("place projection accounting underflow"))?;
            self.slots[base.0 as usize].place = Some(dereferenced);
            self.set_source_view(base, source)?;
        }
    }
    /// rustc's deref coercion at a borrow coercion site: `&b` (`b: Box<T>`,
    /// or a box of a box) whose result is expected as `&T` borrows `&*b`.
    /// The borrow's expected pointee comes from its retained source view,
    /// typed storage contract or reached hint (a call argument's parameter
    /// hint); without one, or when the box itself is the
    /// expected pointee, the place is unchanged. A place not yet executed
    /// (formed later in the same operation list) is left to the borrow.
    pub(crate) fn box_deref_coercion(
        &mut self,
        place: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        use crate::runtime::{cell::StorageContract, shape::TypeKey};
        let fault = |failure: PlaceFailure| match failure {
            PlaceFailure::RuntimeFault(error) => error,
            PlaceFailure::GuestPanic(panic) => invalid(&panic.message()),
        };
        // Only an executed place in an active scope can be coerced here;
        // anything else is left to the borrow's own checks.
        let executed = |frame: &Self| {
            frame.body.slot_kind(place).is_ok_and(|kind| kind == SlotKind::Place)
                && frame.slots.get(place.0 as usize).is_some_and(|slot| {
                    slot.place.is_some() && frame.active_scopes.contains(&slot.scope)
                })
        };
        // Only a box place is adjusted; any other borrow (and any place
        // whose Type cannot be inspected here) keeps its own admission.
        let boxed = |frame: &Self| {
            executed(frame)
                && frame.executed_place(place).is_ok_and(|current| {
                    store
                        .place_type_id(current)
                        .ok()
                        .is_some_and(|ty| store.box_contents_type(ty).ok().flatten().is_some())
                })
        };
        if store.owned_box().is_none() || !boxed(self) {
            return Ok(());
        }
        let Ok(result) = self.slot(destination) else {
            return Ok(());
        };
        let source = match result.source.as_ref() {
            Some(TyTerm::Ref(_, _, pointee)) => Some(pointee.as_ref().clone()),
            _ => None,
        };
        // A call argument's parameter hint is its coercion expectation too.
        let contract = match result.cell {
            Some(cell) => match store.declared(cell) {
                Ok(StorageContract::Exact(ty)) => Some(ty),
                _ => None,
            },
            None => result.hint,
        };
        let expected = contract
            .and_then(|ty| store.type_descriptor(ty).ok())
            .and_then(|descriptor| match descriptor.key.as_ref() {
                TypeKey::Ref { target, .. } => Some(*target),
                _ => None,
            });
        if source.is_none() && expected.is_none() {
            return Ok(());
        }
        loop {
            if !boxed(self) {
                return Ok(());
            }
            let current = self.executed_place(place)?;
            let ty = store.place_type_id(current).map_err(fault)?;
            if expected == Some(ty)
                || expected.is_some_and(|expected| store.dyn_admits(expected, ty))
                || source.as_ref().is_some_and(|pointee| {
                    Self::require_borrow_source_target(Some(pointee), ty, store).is_ok()
                })
            {
                return Ok(());
            }
            let dereferenced = store.dereference_place(current).map_err(fault)?;
            let released = current.projection_bytes()?;
            let view = match self.place_slot(place)?.source.as_ref() {
                Some(view) => self.clone_source(Self::box_contents_source(view, store))?,
                None => None,
            };
            self.projection_bytes = self
                .projection_bytes
                .checked_sub(released)
                .and_then(|bytes| bytes.checked_add(dereferenced.projection_bytes().ok()?))
                .ok_or_else(|| invalid("place projection accounting underflow"))?;
            self.slots[place.0 as usize].place = Some(dereferenced);
            self.set_source_view(place, view)?;
        }
    }
    /// Admission of `*b` for a box place whose heap cell holds `contents`:
    /// the retained box view (if any) must name the reached contents.
    pub(super) fn preflight_box_deref(
        &self,
        boxed: SlotId,
        destination: SlotId,
        contents: TypeId,
        store: &GuestStore,
    ) -> Result<(), PlaceFailure> {
        let pointee = match self.place_slot(boxed)?.source.as_ref() {
            None => None,
            Some(source) => Some(Self::box_contents_source(source, store).ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "box dereference requires its retained box source view".into(),
                )
            })?),
        };
        Self::require_borrow_source_target(pointee, contents, store)?;
        self.preflight_source_view(destination, pointee)?;
        if let Some(expected) = self.place_slot(destination)?.source.as_ref() {
            Self::require_borrow_source_target(Some(expected), contents, store)?;
        }
        Ok(())
    }
}
