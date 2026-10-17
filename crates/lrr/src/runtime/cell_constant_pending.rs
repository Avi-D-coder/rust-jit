//! Unpublished constant owner. Only actual construction can create this token.
use super::*;
#[path = "cell_constant_chunks.rs"]
pub(super) mod chunks;
/// No public fields, raw-ticket constructor, Clone or stable encoding.
pub(crate) struct PendingConstantResult {
    session: StoreSession,
    epoch: u32,
    ticket: Option<TicketId>,
}
impl GuestStore {
    #[cfg(test)]
    pub(crate) fn instantiate_pending_constant(
        &mut self,
        plan: PreparedConstantInstantiation,
        steps: &mut RunSteps,
    ) -> Result<PendingConstantResult, Diagnostic> {
        let ticket = self.instantiate_constant_admitted(plan, steps)?;
        Ok(PendingConstantResult {
            session: self.pattern_binding_session(),
            epoch: self.budget.epoch(),
            ticket: Some(ticket),
        })
    }
    fn validate_pending_constant(
        &self,
        pending: &PendingConstantResult,
    ) -> Result<ValueRepr, Diagnostic> {
        if pending.session != self.pattern_binding_session() || pending.epoch != self.budget.epoch()
        {
            return Err(stale());
        }
        let ticket = pending.ticket.ok_or_else(stale)?;
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        self.require_initialized_value(value)?;
        Ok(value)
    }
    /// Failure leaves the exact opaque owner in place. Successful publication
    /// consumes its private ticket once, without allocating an error wrapper.
    pub(crate) fn commit_pending_constant(
        &mut self,
        pending: &mut PendingConstantResult,
        destination: CellId,
    ) -> Result<(), Diagnostic> {
        let value = self.validate_pending_constant(pending)?;
        if !matches!(
            self.cells.get(destination)?.owner,
            CellOwner::FrameSlot { .. }
        ) {
            return Err(ownership());
        }
        self.validate_initialization_value(destination, value)?;
        let ticket = pending.ticket.ok_or_else(stale)?;
        // Exact FrameSlot initialization cannot invoke callbacks or allocate.
        // The token retains ownership through every rejected preflight.
        self.initialize(destination, ticket)?;
        pending.ticket = None;
        Ok(())
    }
    /// Closed unpublished copies only: never a general aggregate cleanup bypass.
    #[cfg(test)]
    pub(crate) fn abort_pending_constant(
        &mut self,
        pending: PendingConstantResult,
    ) -> Result<(), Diagnostic> {
        self.validate_pending_constant(&pending)?;
        self.abort_constant_ticket(pending.ticket.ok_or_else(stale)?)
    }
}
#[cfg(test)]
#[path = "constant_pending_tests.rs"]
mod tests;
