//! Dispatcher-owned cleanup obligations; no recursive callbacks or source queries.
use super::fuel::RunFuel;
use crate::{
    core::diagnostic::Diagnostic,
    runtime::{
        arena::resource,
        cell::{
            AggregateCleanupTransfer, CellId, CleanupFieldAdmission, CleanupInventory,
            ConstructionId, FrameId, GuestStore, TicketId,
        },
        shape::TypeId,
    },
};

#[derive(Clone, Copy)]
enum Phase {
    Inspect,
    Fields,
    AwaitChild { cell: CellId, ticket: TicketId },
    Release,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::cell::{
        CellOwner, CellState, StorageContract, tests::stock_drop_record_storage,
    };

    fn record() -> (GuestStore, CellId, Vec<CellId>, usize) {
        let (mut store, ticket, fields, members) = stock_drop_record_storage();
        let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
        let frame = store.new_frame_anchor().unwrap();
        let root = store
            .new_cell(
                StorageContract::Exact(ty),
                true,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(root, ticket).unwrap();
        (store, root, fields, members.len())
    }
    fn partial_constructor(max_handles: u32) -> (GuestStore, ConstructionId, FrameId, u32, String) {
        let source = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/nested-return-drop-order.rs"
        );
        let facts = crate::syntax::scope_fixture_facts::original_nested_return(source);
        let (members, ordinal) =
            crate::syntax::scope_fixture_facts::original_nested_constructor_fields(source);
        let mut store = GuestStore::new(1, max_handles, 1_000_000).unwrap();
        let expected = facts.field_tag.to_string();
        let field = store.string(&expected).unwrap();
        let ty = TypeId(store.peek_ticket(field).unwrap().type_id);
        let mut fields = vec![TypeId(8); members.len()];
        fields[ordinal as usize] = ty;
        let guard = store.begin_construction(&fields).unwrap();
        store.construction_push(guard, ordinal, field).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        (store, guard, frame, ordinal, expected)
    }
    #[test]
    fn rustc_nested_constructor_field_cleanup_preserves_location_through_actual_completion() {
        let (mut store, guard, frame, expected_ordinal, expected) = partial_constructor(1000);
        let mut fuel = RunFuel::new(10).unwrap();
        let Some((ordinal, ticket, mut machine)) = CleanupMachine::begin_constructor_field(
            guard, frame, 0, &mut store, &mut fuel, 1_000_000,
        )
        .unwrap() else {
            panic!("stock completed field absent")
        };
        assert_eq!(ordinal, expected_ordinal);
        let cell = machine.tasks[0].cell;
        assert_eq!(store.state(cell).unwrap(), CellState::Dropping);
        assert_eq!(store.string_contents(ticket).unwrap(), expected);
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
        assert!(matches!(
            machine.advance(&mut store, &mut fuel).unwrap(),
            CleanupProgress::Advanced
        ));
        assert_eq!(store.state(cell).unwrap(), CellState::Dropping);
        assert_eq!(store.string_contents(ticket).unwrap(), expected);
        assert!(matches!(
            machine.advance(&mut store, &mut fuel).unwrap(),
            CleanupProgress::Advanced
        ));
        assert_eq!(store.state(cell).unwrap(), CellState::Dropped);
        assert_eq!(
            store.peek_ticket(ticket).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert!(matches!(
            machine.advance(&mut store, &mut fuel).unwrap(),
            CleanupProgress::Complete
        ));
        assert_eq!(fuel.snapshot().used, 1);
        assert_eq!(machine.completed_owners(), 1);
        store
            .finish_construction_cleanup_field(guard, ordinal, ticket)
            .unwrap();
        assert!(
            CleanupMachine::begin_constructor_field(
                guard, frame, 0, &mut store, &mut fuel, 1_000_000
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(
            store.construction_cleanup_next(guard).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(fuel.snapshot().used, 1);
    }
    #[test]
    fn rustc_interrupted_struct_field_cleanup_uses_reverse_written_order_not_declaration_order() {
        let source = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/rfcs/rfc-1857-stabilize-drop-order/drop-order.rs"
        );
        let facts =
            crate::syntax::scope_fixture_facts::original_partial_construction_orders(source);
        for scenario in facts.scenarios {
            let completed = scenario.completed;
            let expected = scenario.expected;
            let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
            let first = store.string(&completed[0].1).unwrap();
            let ty = TypeId(store.peek_ticket(first).unwrap().type_id);
            let guard = store
                .begin_construction(&vec![ty; facts.members.len()])
                .unwrap();
            store
                .construction_push(guard, completed[0].0, first)
                .unwrap();
            for (ordinal, tag) in completed.iter().skip(1) {
                let ticket = store.string(tag).unwrap();
                store.construction_push(guard, *ordinal, ticket).unwrap();
            }
            let frame = store.new_frame_anchor().unwrap();
            let mut fuel = RunFuel::new(100).unwrap();
            let mut observed = Vec::new();
            for (expected_ordinal, _) in completed.iter().rev() {
                let Some((ordinal, ticket, mut machine)) = CleanupMachine::begin_constructor_field(
                    guard, frame, 0, &mut store, &mut fuel, 1_000_000,
                )
                .unwrap() else {
                    panic!("original completed field lost")
                };
                assert_eq!(ordinal, *expected_ordinal);
                observed.push(store.string_contents(ticket).unwrap().to_owned());
                assert!(matches!(
                    machine.advance(&mut store, &mut fuel).unwrap(),
                    CleanupProgress::Advanced
                ));
                assert!(matches!(
                    machine.advance(&mut store, &mut fuel).unwrap(),
                    CleanupProgress::Advanced
                ));
                assert!(matches!(
                    machine.advance(&mut store, &mut fuel).unwrap(),
                    CleanupProgress::Complete
                ));
                store
                    .finish_construction_cleanup_field(guard, ordinal, ticket)
                    .unwrap();
            }
            assert_eq!(observed, expected);
            assert!(
                CleanupMachine::begin_constructor_field(
                    guard, frame, 0, &mut store, &mut fuel, 1_000_000
                )
                .unwrap()
                .is_none()
            );
            assert_eq!(
                fuel.snapshot().used,
                u64::try_from(completed.len()).unwrap()
            );
        }
        // Original written fields, scalar tags and assertion order are retained.
        // Intrinsic String replaces PushOnDrop; catch_unwind, Rc/RefCell, callbacks
        // and the complete original program remain unexecuted.
    }
    #[test]
    fn rustc_nested_constructor_field_cleanup_admission_refusal_preserves_guard_and_fuel() {
        for (handles, metadata, cancelled, limit) in [
            (5, 1_000_000, false, 10),
            (1000, 0, false, 10),
            (1000, 1_000_000, true, 10),
            (1000, 1_000_000, false, 0),
        ] {
            let (mut store, guard, frame, ordinal, expected) = partial_constructor(handles);
            let mut fuel = RunFuel::new(limit).unwrap();
            if cancelled {
                fuel.cancellation().cancel();
            }
            let before = store.budget().handles();
            let result = CleanupMachine::begin_constructor_field(
                guard, frame, 0, &mut store, &mut fuel, metadata,
            );
            assert!(matches!(result, Err(error) if error.code == "LRR-E-RUNTIME-RESOURCE"));
            assert_eq!(store.budget().handles(), before);
            assert_eq!(fuel.snapshot().used, 0);
            // Each refusal leaves the original completed owner available for an
            // exact later transport; no cursor advance is treated as success.
            let crate::runtime::cell::ConstructionCleanupTransfer::Field {
                ordinal: actual,
                ticket,
            } = store.construction_cleanup_transfer(guard).unwrap()
            else {
                panic!("refused cleanup lost completed field")
            };
            assert_eq!(actual, ordinal);
            assert_eq!(store.string_contents(ticket).unwrap(), expected);
            store.discard(ticket).unwrap();
            store
                .finish_construction_cleanup_field(guard, ordinal, ticket)
                .unwrap();
        }
    }
    #[test]
    fn rustc_record_cleanup_machine_finishes_in_declaration_order_with_one_charge_per_owner() {
        let (mut store, root, fields, members) = record();
        let mut fuel = RunFuel::new(100).unwrap();
        let mut machine = CleanupMachine::begin(root, &mut store, &mut fuel, 1_000_000).unwrap();
        let mut seen = Vec::new();
        let mut complete = false;
        for _ in 0..members * 8 + 10 {
            match machine.advance(&mut store, &mut fuel).unwrap() {
                CleanupProgress::Advanced => {}
                CleanupProgress::Complete => {
                    complete = true;
                    break;
                }
                CleanupProgress::NeedWitness { .. } => {
                    panic!("intrinsic adaptation demanded a nominal witness")
                }
            }
            for (ordinal, field) in fields.iter().enumerate() {
                if store.state(*field).unwrap() == CellState::Dropped && !seen.contains(&ordinal) {
                    seen.push(ordinal);
                }
            }
        }
        assert!(complete, "bounded cleanup never completed");
        assert_eq!(seen, (0..members).collect::<Vec<_>>());
        assert_eq!(
            machine.completed_owners(),
            u64::try_from(members + 1).unwrap()
        );
        assert_eq!(fuel.snapshot().used, u64::try_from(members + 1).unwrap());
        assert_eq!(store.state(root).unwrap(), CellState::Dropped);
        assert!(matches!(
            machine.advance(&mut store, &mut fuel).unwrap(),
            CleanupProgress::Complete
        ));
    }
    #[test]
    fn rustc_record_cleanup_machine_fuel_and_cancel_preserve_unprocessed_owned_fields() {
        let (mut store, root, fields, _) = record();
        let mut fuel = RunFuel::new(1).unwrap();
        let mut machine = CleanupMachine::begin(root, &mut store, &mut fuel, 1_000_000).unwrap();
        assert!(matches!(
            machine.advance(&mut store, &mut fuel).unwrap(),
            CleanupProgress::Advanced
        ));
        let handles = store.budget().handles();
        assert!(
            matches!(machine.advance(&mut store, &mut fuel), Err(error) if error.code == "LRR-E-RUNTIME-RESOURCE")
        );
        assert_eq!(store.budget().handles(), handles);
        assert_eq!(machine.completed_owners(), 0);
        assert_eq!(store.state(root).unwrap(), CellState::Dropping);
        assert!(
            fields
                .iter()
                .all(|field| matches!(store.state(*field).unwrap(), CellState::Live(_)))
        );
        assert!(store.peek_ticket(machine.tasks[0].ticket).is_ok());

        let (mut store, root, fields, _) = record();
        let mut fuel = RunFuel::new(100).unwrap();
        fuel.cancellation().cancel();
        let handles = store.budget().handles();
        assert!(
            matches!(CleanupMachine::begin(root, &mut store, &mut fuel, 1_000_000), Err(error) if error.code == "LRR-E-RUNTIME-RESOURCE")
        );
        assert_eq!(store.budget().handles(), handles);
        assert!(matches!(store.state(root).unwrap(), CellState::Live(_)));
        assert!(
            fields
                .iter()
                .all(|field| matches!(store.state(*field).unwrap(), CellState::Live(_)))
        );
        assert_eq!(fuel.snapshot().used, 0);

        let (mut store, root, fields, _) = record();
        let mut fuel = RunFuel::new(100).unwrap();
        let handles = store.budget().handles();
        assert!(
            matches!(CleanupMachine::begin(root, &mut store, &mut fuel, 0), Err(error) if error.code == "LRR-E-RUNTIME-RESOURCE")
        );
        assert_eq!(store.budget().handles(), handles);
        assert!(matches!(store.state(root).unwrap(), CellState::Live(_)));
        assert!(
            fields
                .iter()
                .all(|field| matches!(store.state(*field).unwrap(), CellState::Live(_)))
        );
        assert_eq!(fuel.snapshot().used, 0);
    }
    #[test]
    fn rustc_partial_record_cleanup_skips_the_extracted_field_without_consuming_its_owner() {
        let original = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-partial-move-1.rs"
        );
        let (members, extracted) =
            crate::syntax::scope_fixture_facts::original_partial_record_move(original);
        // Original `p.y`/declaration ordinals; intrinsic String storage replaces
        // S/its destructor. Struct update, generic f/FnMut and derives are cold.
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let first = store.string(&members[0]).unwrap();
        let element = TypeId(store.peek_ticket(first).unwrap().type_id);
        let types = vec![element; members.len()];
        let ty = store
            .intern_type(crate::runtime::shape::TypeKey::Tuple(types.clone().into()))
            .unwrap();
        let guard = store.begin_construction(&types).unwrap();
        store.construction_push(guard, 0, first).unwrap();
        for (ordinal, member) in members.iter().enumerate().skip(1) {
            let ticket = store.string(member).unwrap();
            store
                .construction_push(guard, u32::try_from(ordinal).unwrap(), ticket)
                .unwrap();
        }
        let ticket = store.commit_construction(guard, ty).unwrap();
        let fields: Vec<_> = (0..members.len())
            .map(|ordinal| {
                store
                    .aggregate_field(ticket, u32::try_from(ordinal).unwrap())
                    .unwrap()
            })
            .collect();
        let frame = store.new_frame_anchor().unwrap();
        let root = store
            .new_cell(
                StorageContract::Exact(ty),
                true,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(root, ticket).unwrap();
        let moved = store.move_from(fields[extracted]).unwrap();
        let moved_value = store.peek_ticket(moved).unwrap();
        let mut fuel = RunFuel::new(100).unwrap();
        let mut machine = CleanupMachine::begin(root, &mut store, &mut fuel, 1_000_000).unwrap();
        let mut complete = false;
        for _ in 0..members.len() * 8 + 10 {
            match machine.advance(&mut store, &mut fuel).unwrap() {
                CleanupProgress::Advanced => {}
                CleanupProgress::Complete => {
                    complete = true;
                    break;
                }
                CleanupProgress::NeedWitness { .. } => {
                    panic!("intrinsic adaptation demanded nominal Drop")
                }
            }
            assert_eq!(store.peek_ticket(moved).unwrap(), moved_value);
        }
        assert!(complete);
        assert_eq!(machine.completed_owners(), members.len() as u64 + 1);
        assert_eq!(fuel.snapshot().used, members.len() as u64 + 1);
        assert!(
            fields
                .iter()
                .all(|field| store.state(*field).unwrap() == CellState::Dropped)
        );
        assert_eq!(store.state(root).unwrap(), CellState::Dropped);
        assert_eq!(store.peek_ticket(moved).unwrap(), moved_value);
        store.discard(moved).unwrap();
        assert_eq!(
            store.peek_ticket(moved).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
    }
}
#[derive(Clone, Copy)]
struct Task {
    cell: CellId,
    ticket: TicketId,
    phase: Phase,
}
/// A demand is owned VM state, never a query result containing guest handles.
pub enum CleanupProgress {
    Advanced,
    NeedWitness {
        cell: CellId,
        ticket: TicketId,
        ty: TypeId,
    },
    Complete,
}
/// One run owns this continuation. Metadata_limit is the caller's remaining
/// frame/engine metadata budget, not an independent extra allocation allowance.
pub struct CleanupMachine {
    tasks: Vec<Task>,
    metadata_limit: usize,
    completed: u64,
    epoch: u32,
}
/// Prepared metadata, without taking an owner. Consuming this wrapper installs
/// exactly one root task; an empty machine cannot be published as completed.
pub(crate) struct PreparedCleanup {
    machine: CleanupMachine,
    cell: CellId,
    expected: crate::runtime::value::ValueRepr,
}
impl PreparedCleanup {
    /// After transactional replace, the old value lives in its explicit cleanup
    /// location. The reserved root may follow only that exact owning value.
    pub(crate) fn relocate_replacement_root(
        mut self,
        cell: CellId,
        store: &GuestStore,
    ) -> Result<Self, Diagnostic> {
        if self.machine.epoch != store.budget().epoch()
            || store.admit_cleanup_root(cell)? != self.expected
        {
            return Err(invalid(
                "replacement cleanup root differs from reserved old owner",
            ));
        }
        self.cell = cell;
        Ok(self)
    }

    pub(crate) fn start(
        mut self,
        store: &mut GuestStore,
        fuel: &mut RunFuel,
    ) -> Result<CleanupMachine, Diagnostic> {
        if self.machine.epoch != store.budget().epoch() {
            return Err(invalid("prepared cleanup belongs to another run"));
        }
        if store.admit_cleanup_root(self.cell)? != self.expected {
            return Err(invalid("prepared cleanup root changed before entry"));
        }
        fuel.poll()?;
        fuel.charge()?;
        let ticket = store.begin_drop(self.cell)?;
        self.machine.tasks.push(Task {
            cell: self.cell,
            ticket,
            phase: Phase::Inspect,
        });
        Ok(self.machine)
    }
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-CLEANUP", message.into())
}
impl CleanupMachine {
    /// A partial constructor contributes completed fields in reverse completion
    /// order. Reserve all metadata/location storage before charging or moving a
    /// field; the guard retains the exact ticket until this machine completes.
    pub(crate) fn begin_constructor_field(
        guard: ConstructionId,
        frame: FrameId,
        slot: u32,
        store: &mut GuestStore,
        fuel: &mut RunFuel,
        metadata_limit: usize,
    ) -> Result<Option<(u32, TicketId, Self)>, Diagnostic> {
        fuel.poll()?;
        if !store.reserve_construction_cleanup_location(guard, frame)? {
            if store
                .construction_cleanup_location(guard, frame, slot)?
                .is_some()
            {
                return Err(invalid(
                    "empty construction acquired a field during admission",
                ));
            }
            return Ok(None);
        }
        let mut machine = Self {
            tasks: Vec::new(),
            metadata_limit: metadata_limit.min(32 * 1024 * 1024),
            completed: 0,
            epoch: store.budget().epoch(),
        };
        machine.reserve_task()?;
        fuel.charge()?;
        let Some((ordinal, cell, ticket)) =
            store.construction_cleanup_location(guard, frame, slot)?
        else {
            return Err(invalid(
                "admitted construction field disappeared before transfer",
            ));
        };
        machine.tasks.push(Task {
            cell,
            ticket,
            phase: Phase::Inspect,
        });
        Ok(Some((ordinal, ticket, machine)))
    }
    pub fn begin(
        cell: CellId,
        store: &mut GuestStore,
        fuel: &mut RunFuel,
        metadata_limit: usize,
    ) -> Result<Self, Diagnostic> {
        Self::prepare_root(cell, store, metadata_limit)?.start(store, fuel)
    }
    pub(crate) fn prepare_root(
        cell: CellId,
        store: &GuestStore,
        metadata_limit: usize,
    ) -> Result<PreparedCleanup, Diagnostic> {
        let expected = store.admit_cleanup_root(cell)?;
        let mut machine = Self {
            tasks: Vec::new(),
            metadata_limit: metadata_limit.min(32 * 1024 * 1024),
            completed: 0,
            epoch: store.budget().epoch(),
        };
        machine.reserve_task()?;
        Ok(PreparedCleanup {
            machine,
            cell,
            expected,
        })
    }
    fn reserve_task(&mut self) -> Result<(), Diagnostic> {
        let count = self.tasks.len().checked_add(1).ok_or_else(resource)?;
        let bytes = count
            .checked_mul(std::mem::size_of::<Task>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(resource)?;
        if bytes > self.metadata_limit {
            return Err(resource());
        }
        if count > self.tasks.capacity() {
            self.tasks.try_reserve_exact(1).map_err(|_| resource())?;
        }
        if self.metadata_bytes()? > self.metadata_limit {
            return Err(resource());
        }
        Ok(())
    }
    pub fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.tasks
            .capacity()
            .checked_mul(std::mem::size_of::<Task>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(resource)
    }
    pub fn completed_owners(&self) -> u64 {
        self.completed
    }
    /// Exactly one transition; even a skipped field returns to the dispatcher.
    /// A nominal policy is demanded only when reached. No absent policy is NoDrop.
    pub fn advance(
        &mut self,
        store: &mut GuestStore,
        fuel: &mut RunFuel,
    ) -> Result<CleanupProgress, Diagnostic> {
        if store.budget().epoch() != self.epoch {
            return Err(invalid("cleanup continuation belongs to a different run"));
        }
        fuel.poll()?;
        let Some(task) = self.tasks.last().copied() else {
            return Ok(CleanupProgress::Complete);
        };
        match task.phase {
            Phase::Inspect => {
                let phase = match store.cleanup_ticket_inventory(task.ticket)? {
                    CleanupInventory::Leaf => Phase::Release,
                    CleanupInventory::Fields => Phase::Fields,
                    CleanupInventory::NeedWitness(ty) => {
                        return Ok(CleanupProgress::NeedWitness {
                            cell: task.cell,
                            ticket: task.ticket,
                            ty,
                        });
                    }
                };
                self.tasks
                    .last_mut()
                    .ok_or_else(|| invalid("cleanup task absent"))?
                    .phase = phase;
            }
            Phase::Fields => {
                let admission = store.cleanup_next_field(task.ticket)?;
                let completed = if matches!(admission, CleanupFieldAdmission::Skip) {
                    self.completed.checked_add(1).ok_or_else(resource)?
                } else {
                    self.completed
                };
                match admission {
                    CleanupFieldAdmission::Live => {
                        self.reserve_task()?;
                        fuel.charge()?;
                    }
                    CleanupFieldAdmission::Skip => fuel.charge()?,
                    CleanupFieldAdmission::Complete => {}
                }
                match store.aggregate_cleanup_transfer(task.ticket)? {
                    AggregateCleanupTransfer::Field { cell, ticket } => {
                        self.tasks
                            .last_mut()
                            .ok_or_else(|| invalid("cleanup parent absent"))?
                            .phase = Phase::AwaitChild { cell, ticket };
                        self.tasks.push(Task {
                            cell,
                            ticket,
                            phase: Phase::Inspect,
                        });
                    }
                    AggregateCleanupTransfer::Skipped => self.completed = completed,
                    AggregateCleanupTransfer::Complete => {
                        self.tasks
                            .last_mut()
                            .ok_or_else(|| invalid("cleanup parent absent"))?
                            .phase = Phase::Release;
                    }
                }
            }
            Phase::AwaitChild { cell, ticket } => {
                store.finish_aggregate_cleanup_field(task.ticket, cell, ticket)?;
                self.tasks
                    .last_mut()
                    .ok_or_else(|| invalid("cleanup parent absent"))?
                    .phase = Phase::Fields;
            }
            Phase::Release => {
                let completed = self.completed.checked_add(1).ok_or_else(resource)?;
                store.discard(task.ticket)?;
                // Child locations belong to the parent's saved continuation.
                // Only the root retires here; the parent acknowledges children.
                if self.tasks.len() == 1 {
                    store.retire_cell(task.cell)?;
                }
                self.tasks.pop();
                self.completed = completed;
            }
        }
        Ok(CleanupProgress::Advanced)
    }
}
