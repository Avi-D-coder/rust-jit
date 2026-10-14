//! Partial-constructor cleanup keeps a precise child continuation. Uninitialized
//! fields and the uncommitted enclosing value have no destructor to dispatch.
use super::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ConstructionCleanupTransfer {
    Field { ordinal: u32, ticket: TicketId },
    Complete,
}
impl GuestStore {
    /// Reserve the complete field-to-cleanup-location transaction. A physical
    /// frame anchor outlives the child ticket, so the location remains usable as
    /// `self` even after a nested destructor consumes that ticket.
    pub(crate) fn reserve_construction_cleanup_location(
        &mut self,
        guard: ConstructionId,
        frame: FrameId,
    ) -> Result<bool, Diagnostic> {
        self.frames.get(frame)?;
        let record = self.constructions.get(guard)?;
        if record.pending_cleanup.is_some() {
            return Err(ownership());
        }
        let Some(field) = record.completed.last().copied() else {
            return Ok(false);
        };
        let value = record.fields[field as usize].ok_or_else(ownership)?;
        self.validate_value(value)?;
        self.object_owner(value, ValueOwner::Construction { guard, field })?;
        self.budget.check(2, 0)?;
        self.tickets.reserve(1, &mut self.budget)?;
        self.cells.reserve(1, &mut self.budget)?;
        Ok(true)
    }
    /// The VM calls this only after metadata, arena and fuel admission. Keep
    /// the original transport ticket; do not move it through a second ticket.
    pub(crate) fn construction_cleanup_location(
        &mut self,
        guard: ConstructionId,
        frame: FrameId,
        slot: u32,
    ) -> Result<Option<(u32, CellId, TicketId)>, Diagnostic> {
        self.reserve_construction_cleanup_location(guard, frame)?;
        let ConstructionCleanupTransfer::Field { ordinal, ticket } =
            self.construction_cleanup_transfer(guard)?
        else {
            return Ok(None);
        };
        let value = self.peek_ticket(ticket)?;
        let cell = self.cells.allocate(
            Cell {
                state: CellState::Dropping,
                declared: StorageContract::Exact(TypeId(value.type_id)),
                writable: true,
                location_epoch: 1,
                owner: CellOwner::FrameSlot { frame, slot },
                retired: false,
                numeric_record: None,
            },
            0,
            &mut self.budget,
        )?;
        Ok(Some((ordinal, CellId::from(cell), ticket)))
    }
    /// Transport a single completed field and retain its exact ordinal/ticket.
    /// Reservation and source-owner validation precede cursor mutation. This
    /// does not select or execute Drop and is not proof of a NoDrop policy.
    pub fn construction_cleanup_transfer(
        &mut self,
        guard: ConstructionId,
    ) -> Result<ConstructionCleanupTransfer, Diagnostic> {
        let record = self.constructions.get(guard)?;
        if record.pending_cleanup.is_some() {
            return Err(ownership());
        }
        let field = record.completed.last().copied();
        let ticket = self.construction_cleanup_next(guard)?;
        match (field, ticket) {
            (Some(ordinal), Some(ticket)) => {
                let value = self.peek_ticket(ticket)?;
                self.constructions.get_mut(guard)?.pending_cleanup = Some((ordinal, ticket, value));
                Ok(ConstructionCleanupTransfer::Field { ordinal, ticket })
            }
            (None, None) => Ok(ConstructionCleanupTransfer::Complete),
            _ => Err(ownership()),
        }
    }
    /// Only the exact saved child may acknowledge completion. A live ticket,
    /// unrelated stale ticket or repeated acknowledgement cannot release a guard.
    pub fn finish_construction_cleanup_field(
        &mut self,
        guard: ConstructionId,
        ordinal: u32,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let record = self.constructions.get(guard)?;
        let Some((expected_ordinal, expected_ticket, value)) = record.pending_cleanup else {
            return Err(ownership());
        };
        if record.state != ConstructionState::Cleaning
            || expected_ordinal != ordinal
            || expected_ticket != ticket
            || record
                .fields
                .get(ordinal as usize)
                .is_none_or(Option::is_some)
        {
            return Err(ownership());
        }
        match self.tickets.get(ticket) {
            Err(error) if error.code == "LRR-E-STALE-HANDLE" => {}
            Ok(_) | Err(_) => return Err(ownership()),
        }
        // Consuming a transport ticket into another live owner is not cleanup.
        // Owning object and mutable-reference lease retirement must be real.
        if value.kind == 8 {
            match self.objects.get(ObjectId::from_bits(value.payload)) {
                Err(error) if error.code == "LRR-E-STALE-HANDLE" => {}
                Ok(_) | Err(_) => return Err(ownership()),
            }
        } else if value.kind == 9
            && matches!(
                self.types.descriptor(TypeId(value.type_id))?.key.as_ref(),
                TypeKey::Ref { mutable: true, .. }
            )
        {
            match self.references.get(RefId::from_bits(value.payload)) {
                Err(error) if error.code == "LRR-E-STALE-HANDLE" => {}
                Ok(_) | Err(_) => return Err(ownership()),
            }
        }
        self.constructions.get_mut(guard)?.pending_cleanup = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/nested-return-drop-order.rs"
    );
    fn partial(max_handles: u32) -> (GuestStore, ConstructionId, u32, String) {
        let facts = crate::syntax::scope_fixture_facts::original_nested_return(ORIGINAL);
        let (members, completed) =
            crate::syntax::scope_fixture_facts::original_nested_constructor_fields(ORIGINAL);
        let mut store = GuestStore::new(1, max_handles, 1_000_000).unwrap();
        // Completed Bar is adapted to intrinsic String tagged by its written x;
        // original Foo/Bar construction and Bar::drop assertions are unexecuted.
        let expected = facts.field_tag.to_string();
        let ticket = store.string(&expected).unwrap();
        let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
        let mut types = vec![TypeId(8); members.len()];
        types[completed as usize] = ty;
        let guard = store.begin_construction(&types).unwrap();
        store.construction_push(guard, completed, ticket).unwrap();
        (store, guard, completed, expected)
    }
    #[test]
    fn rustc_nested_constructor_cleanup_keeps_guard_until_exact_child_completion() {
        let (mut store, guard, completed, expected) = partial(1000);
        let ConstructionCleanupTransfer::Field { ordinal, ticket } =
            store.construction_cleanup_transfer(guard).unwrap()
        else {
            panic!("completed original field absent")
        };
        assert_eq!(ordinal, completed);
        assert_eq!(store.string_contents(ticket).unwrap(), expected);
        let value = store.peek_ticket(ticket).unwrap();
        let handles = store.budget().handles();
        for _ in 0..2 {
            assert_eq!(
                store.construction_cleanup_transfer(guard).unwrap_err().code,
                "LRR-E-OWNERSHIP"
            );
            assert_eq!(
                store.construction_cleanup_next(guard).unwrap_err().code,
                "LRR-E-OWNERSHIP"
            );
            assert_eq!(
                store
                    .finish_construction_cleanup_field(guard, ordinal, ticket)
                    .unwrap_err()
                    .code,
                "LRR-E-OWNERSHIP"
            );
            assert_eq!(
                store.constructions.get(guard).unwrap().pending_cleanup,
                Some((ordinal, ticket, value))
            );
            assert_eq!(store.budget().handles(), handles);
        }
        let other = store.ticket(ValueRepr::unit()).unwrap();
        store.discard(other).unwrap();
        assert_eq!(
            store
                .finish_construction_cleanup_field(guard, ordinal, other)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        // A consumed ticket moved to a live location must not masquerade as
        // completed cleanup. Retire the actual object before acknowledging it.
        let frame = store.new_frame_anchor().unwrap();
        let destination = store
            .new_cell(
                StorageContract::Exact(TypeId(value.type_id)),
                true,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(destination, ticket).unwrap();
        assert_eq!(
            store.peek_ticket(ticket).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(
            store
                .finish_construction_cleanup_field(guard, ordinal, ticket)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        let cleanup = store.begin_drop(destination).unwrap();
        assert_eq!(
            store
                .finish_construction_cleanup_field(guard, ordinal, ticket)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        store.discard(cleanup).unwrap();
        store.retire_cell(destination).unwrap();
        assert_eq!(
            store
                .finish_construction_cleanup_field(guard, ordinal.checked_add(1).unwrap(), ticket)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        store
            .finish_construction_cleanup_field(guard, ordinal, ticket)
            .unwrap();
        assert_eq!(
            store
                .finish_construction_cleanup_field(guard, ordinal, ticket)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(
            store.construction_cleanup_transfer(guard).unwrap(),
            ConstructionCleanupTransfer::Complete
        );
        assert_eq!(
            store.constructions.get(guard).err().unwrap().code,
            "LRR-E-STALE-HANDLE"
        );
    }
    #[test]
    fn rustc_nested_constructor_cleanup_reservation_refusal_preserves_guarded_field() {
        let (mut store, guard, completed, _) = partial(3);
        let value = store.constructions.get(guard).unwrap().fields[completed as usize].unwrap();
        let handles = store.budget().handles();
        assert_eq!(
            store.construction_cleanup_transfer(guard).unwrap_err().code,
            "LRR-E-RUNTIME-RESOURCE"
        );
        let record = store.constructions.get(guard).unwrap();
        assert_eq!(record.state, ConstructionState::Building);
        assert_eq!(record.pending_cleanup, None);
        assert_eq!(record.completed, [completed]);
        assert_eq!(record.fields[completed as usize], Some(value));
        assert_eq!(store.budget().handles(), handles);
        store
            .object_owner(
                value,
                ValueOwner::Construction {
                    guard,
                    field: completed,
                },
            )
            .unwrap();
    }
}
