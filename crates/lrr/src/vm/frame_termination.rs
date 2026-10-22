//! The program entry's `Termination` report, frame side. The finished entry
//! frame lends a cell holding the value `main` returned as the report call's
//! one argument, and the report's frame shares this run's step and action
//! observation, like any callee entered from this frame.
use super::*;
use crate::runtime::cell::{CellId, CellOwner, StorageContract, TicketId};

impl Frame {
    /// A cell owned by this (finished) frame's cleanup locations that takes
    /// the returned value's ticket.
    pub(crate) fn lend_termination_value(
        &mut self,
        ticket: TicketId,
        store: &mut GuestStore,
    ) -> Result<CellId, Diagnostic> {
        self.require_store(store)?;
        let ty = crate::runtime::shape::TypeId(store.peek_ticket(ticket)?.type_id);
        let slot = self.owned_location_base()?;
        let cell = store.new_cell(
            StorageContract::Exact(ty),
            false,
            CellOwner::FrameSlot {
                frame: self.anchor,
                slot,
            },
        )?;
        store.initialize(cell, ticket)?;
        Ok(cell)
    }
    /// Enter the report: its frame shares this frame's step and actions.
    pub(crate) fn enter_termination(
        &mut self,
        body: Arc<BodyIndex>,
        entry: PcKey,
        store: &mut GuestStore,
    ) -> Result<Self, Diagnostic> {
        self.require_store(store)?;
        let mut frame = Self::new(body, entry, store)?;
        frame.fuel = self.fuel.share();
        frame.actions = self.actions.share();
        Ok(frame)
    }
}
