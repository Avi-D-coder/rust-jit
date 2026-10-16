//! Read-only source-boundary receipts for actual reference targets and tickets.
use super::*;

/// Captured only from a live owning reference cell before callee entry. A new
/// reborrow lease may name the same checked location without reusing its ID.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ReferenceTarget {
    session: StoreSession,
    root: CellStamp,
    final_cell: CellStamp,
    target: TypeId,
    access: Access,
    enclosing: Vec<EnclosingObject>,
}
impl ReferenceTarget {
    pub(crate) fn owned_bytes(&self) -> Result<usize, Diagnostic> {
        checked_chain_bytes(self.enclosing.capacity())
    }
}
impl GuestStore {
    pub(crate) fn reference_target_owned_bytes(&self, cell: CellId) -> Result<usize, Diagnostic> {
        let reference = self.reference_from_cell(cell)?;
        checked_chain_bytes(self.references.get(reference)?.enclosing.len())
    }
    pub(crate) fn reference_target(&self, cell: CellId) -> Result<ReferenceTarget, Diagnostic> {
        let reference = self.reference_from_cell(cell)?;
        let record = self.references.get(reference)?;
        checked_chain_bytes(record.enclosing.len())?;
        let mut enclosing = Vec::new();
        enclosing
            .try_reserve_exact(record.enclosing.len())
            .map_err(|_| resource())?;
        enclosing.extend_from_slice(&record.enclosing);
        checked_chain_bytes(enclosing.capacity())?;
        Ok(ReferenceTarget {
            session: self.pattern_binding_session(),
            root: record.root,
            final_cell: record.final_cell,
            target: record.target,
            access: record.access,
            enclosing,
        })
    }
    /// Validate the ticket's actual value/owner and its live target, without
    /// moving it or allocating a cell. Source proof supplies pointee/access.
    pub(crate) fn require_reference_return(
        &self,
        ticket: TicketId,
        expected: &ReferenceTarget,
        mutable: bool,
        pointee: TypeId,
    ) -> Result<TypeId, Diagnostic> {
        if expected.session != self.pattern_binding_session() {
            return Err(ownership());
        }
        self.validate_reference_target(
            expected.root,
            &expected.enclosing,
            expected.final_cell,
            expected.target,
        )?;
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if value.kind != 9 {
            return Err(invalid());
        }
        let record = self.references.get(RefId::from_bits(value.payload))?;
        self.validate_reference_record(record)?;
        if record.root != expected.root
            || record.final_cell != expected.final_cell
            || record.enclosing != expected.enclosing
            || record.target != expected.target
            || record.target != pointee
            || (record.access == Access::Mutable) != mutable
            || mutable && expected.access != Access::Mutable
        {
            return Err(ownership());
        }
        // validate_value checks the real record's descriptor, not the reverse.
        Ok(TypeId(value.type_id))
    }

    /// A changed pointee requires a source-selected member edge from the
    /// original input location. Runtime shape and equal endpoints are not a
    /// replacement for that edge's source certificate.
    pub(crate) fn require_reference_projection_return(
        &self,
        ticket: TicketId,
        expected: &ReferenceTarget,
        mutable: bool,
        origin: TypeId,
        pointee: TypeId,
    ) -> Result<TypeId, Diagnostic> {
        if expected.session != self.pattern_binding_session() || expected.target != origin {
            return Err(ownership());
        }
        self.validate_reference_target(
            expected.root,
            &expected.enclosing,
            expected.final_cell,
            expected.target,
        )?;
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if value.kind != 9 {
            return Err(invalid());
        }
        let record = self.references.get(RefId::from_bits(value.payload))?;
        self.validate_reference_record(record)?;
        let prefix = expected.enclosing.len();
        if record.root != expected.root
            || record.enclosing.len() != prefix.checked_add(1).ok_or_else(resource)?
            || record.enclosing.get(..prefix) != Some(expected.enclosing.as_slice())
            || record.target != pointee
            || (record.access == Access::Mutable) != mutable
            || mutable && expected.access != Access::Mutable
        {
            return Err(ownership());
        }
        let edge = &record.enclosing[prefix];
        if edge.parent != expected.final_cell || edge.child != record.final_cell.cell {
            return Err(ownership());
        }
        let index = u32::try_from(prefix).map_err(|_| resource())?;
        let source = record
            .source_projections
            .binary_search_by_key(&index, |step| step.chain_index)
            .ok()
            .and_then(|index| record.source_projections.get(index))
            .ok_or_else(ownership)?;
        Self::require_source_projection_parent(
            &source.member,
            self.types.descriptor(origin)?.key.as_ref(),
        )?;
        if crate::runtime::shape::PrimitiveIdentity::from_recipe(source.member.primitive)?.id()
            != pointee
        {
            return Err(ownership());
        }
        // Current member admission proves primitive fields. Further structural
        // paths need the same common source-member proof before admission.
        Ok(TypeId(value.type_id))
    }
}

#[cfg(test)]
#[path = "cell_reference_return_tests.rs"]
mod tests;
