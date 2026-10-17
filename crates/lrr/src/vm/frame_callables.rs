//! Frame access for calls through a callee value (`TypeKey::Callable`).
//!
//! The callee operand of a value call executes as a place (`MakePlace`,
//! `ParameterPlace` or a temporary `ValuePlace`). The dispatcher inspects its
//! reached type to select the callee and retires the place when the call is
//! sealed; the place never becomes an argument.
use super::{Frame, invalid};
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::SlotId,
    runtime::{cell::GuestStore, place::Place, shape::TypeId},
};

impl Frame {
    /// The value callee operand's place slot for the call at `site`.
    pub(crate) fn value_callee_place(&self, site: u32) -> Result<SlotId, Diagnostic> {
        let call = self
            .body
            .nodes()
            .get(site as usize)
            .ok_or_else(|| invalid("value call node absent"))?;
        let callee = call
            .operands
            .first()
            .and_then(|id| self.body.nodes().get(id.0 as usize))
            .ok_or_else(|| invalid("value call callee operand absent"))?;
        callee
            .place
            .ok_or_else(|| invalid("value callee place slot absent"))
    }
    /// The executed callee place and its reached descriptor.
    pub(crate) fn value_callee(
        &self,
        site: u32,
        store: &GuestStore,
    ) -> Result<(Place, TypeId), Diagnostic> {
        let slot = self.value_callee_place(site)?;
        let ty = self.place_type(slot, store)?;
        let place = self.executed_place(slot)?;
        let mut projections = Vec::new();
        projections
            .try_reserve_exact(place.projections.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        projections.extend_from_slice(&place.projections);
        Ok((
            Place {
                root: place.root,
                projections: projections.into_boxed_slice(),
            },
            ty,
        ))
    }
    /// Retire the callee place once the call is sealed.
    pub(crate) fn retire_value_callee(&mut self, site: u32) -> Result<(), Diagnostic> {
        let slot = self.value_callee_place(site)?;
        if self.place_slot(slot)?.place.is_some() {
            self.clear_place(slot)?;
        }
        Ok(())
    }
}
