//! Retained source views, separate from guest ownership and source admission.
//! E-R5-006: source cloning and publication perform no resource accounting.
use super::*;
use crate::traits::term::TyTerm;

impl Frame {
    /// The source Type a call argument reaches the boundary with: the slot's
    /// retained view, or the value's own closed runtime descriptor where the
    /// slot has none or its view names a declared `dyn Trait` position (a
    /// value keeps its concrete Type).
    pub(crate) fn argument_source_view(
        &self,
        slot: SlotId,
        descriptor: crate::runtime::shape::TypeId,
        store: &crate::runtime::cell::GuestStore,
    ) -> Result<Option<TyTerm>, Diagnostic> {
        let reached = || {
            store
                .descriptor_source_term(descriptor)
                .filter(|term| !term.contains_dyn())
        };
        Ok(match self.source_view(slot)? {
            Some(view) if view.contains_dyn() => reached().or(Some(view)),
            Some(view) => Some(view),
            None => store.descriptor_source_term(descriptor),
        })
    }
    pub(super) fn clone_source(
        &self,
        source: Option<&TyTerm>,
    ) -> Result<Option<TyTerm>, Diagnostic> {
        Ok(source.cloned())
    }
    /// Validate the entire publication before CallPrepare changes ownership.
    pub(super) fn preflight_call_source_views(
        &self,
        hints: &[(SlotId, Option<TypeId>, Option<TyTerm>)],
    ) -> Result<(), Diagnostic> {
        for (slot, _, _) in hints {
            self.slot(*slot)?;
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(hints.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        slots.extend(hints.iter().map(|(slot, _, _)| slot.0));
        slots.sort_unstable();
        if slots.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid("call source arguments alias a temporary slot"));
        }
        Ok(())
    }
    pub(super) fn preflight_binding_source_view(
        &self,
        binding: NodeId,
        _source: Option<&TyTerm>,
    ) -> Result<(), Diagnostic> {
        self.binding_sources
            .get(binding.0 as usize)
            .ok_or_else(|| invalid("binding source absent"))?;
        Ok(())
    }
    pub(super) fn set_binding_source_view(
        &mut self,
        binding: NodeId,
        source: Option<TyTerm>,
    ) -> Result<(), Diagnostic> {
        self.preflight_binding_source_view(binding, source.as_ref())?;
        self.binding_sources[binding.0 as usize] = source;
        Ok(())
    }
    pub(super) fn preflight_borrow_source_view(
        &self,
        place: SlotId,
        destination: SlotId,
    ) -> Result<(), Diagnostic> {
        if let Some(written) = self.source_view_ref(destination)? {
            return self.preflight_source_view(destination, Some(written));
        }
        self.place_slot(place)?;
        Ok(())
    }
    pub(super) fn clear_parameter_source_views(&mut self) -> Result<(), Diagnostic> {
        self.parameter_sources.clear();
        Ok(())
    }
    pub(crate) fn set_return_source_view(
        &mut self,
        source: Option<TyTerm>,
    ) -> Result<(), Diagnostic> {
        self.return_source = source;
        Ok(())
    }
}
