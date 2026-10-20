//! Dispatcher-owned cleanup obligations; no recursive callbacks or source queries.
use super::steps::RunSteps;
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
    fn partial_constructor() -> (GuestStore, ConstructionId, FrameId, u32, String) {
        let source = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/nested-return-drop-order.rs"
        );
        let facts = crate::syntax::scope_fixture_facts::original_nested_return(source);
        let (members, ordinal) =
            crate::syntax::scope_fixture_facts::original_nested_constructor_fields(source);
        let mut store = GuestStore::for_epoch(1).unwrap();
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
        let (mut store, guard, frame, expected_ordinal, expected) = partial_constructor();
        let mut fuel = RunSteps::default();
        let Some((ordinal, ticket, mut machine)) =
            CleanupMachine::begin_constructor_field(guard, frame, 0, &mut store, &mut fuel)
                .unwrap()
        else {
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
            CleanupMachine::begin_constructor_field(guard, frame, 0, &mut store, &mut fuel)
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
            let mut store = GuestStore::for_epoch(1).unwrap();
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
            let mut fuel = RunSteps::default();
            let mut observed = Vec::new();
            for (expected_ordinal, _) in completed.iter().rev() {
                let Some((ordinal, ticket, mut machine)) =
                    CleanupMachine::begin_constructor_field(guard, frame, 0, &mut store, &mut fuel)
                        .unwrap()
                else {
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
                CleanupMachine::begin_constructor_field(guard, frame, 0, &mut store, &mut fuel)
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
    fn rustc_record_cleanup_machine_finishes_in_declaration_order_with_one_charge_per_owner() {
        let (mut store, root, fields, members) = record();
        let mut fuel = RunSteps::default();
        let mut machine = CleanupMachine::begin(root, &mut store, &mut fuel).unwrap();
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
    fn rustc_partial_record_cleanup_skips_the_extracted_field_without_consuming_its_owner() {
        let original = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-partial-move-1.rs"
        );
        let (members, extracted) =
            crate::syntax::scope_fixture_facts::original_partial_record_move(original);
        // Original `p.y`/declaration ordinals; intrinsic String storage replaces
        // S/its destructor. Struct update, generic f/FnMut and derives are cold.
        let mut store = GuestStore::for_epoch(1).unwrap();
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
        let mut fuel = RunSteps::default();
        let mut machine = CleanupMachine::begin(root, &mut store, &mut fuel).unwrap();
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
/// One run owns this continuation. Its task stack is unbounded (E-R5-006).
pub struct CleanupMachine {
    tasks: Vec<Task>,
    completed: u64,
    epoch: u32,
}
/// Prepared metadata, without taking an owner. Consuming this wrapper installs
/// exactly one root task; an empty machine cannot be published as completed.
pub(crate) struct PreparedCleanup {
    machine: CleanupMachine,
    cell: CellId,
    location_epoch: u32,
    expected: crate::runtime::value::ValueRepr,
}
impl PreparedCleanup {
    /// After transactional replace, the old value lives in its explicit cleanup
    /// location. The reserved root may follow only that exact owning value.
    pub(crate) fn relocate_replacement_root(
        mut self,
        receipt: crate::runtime::cell::ReplacementCleanupRoot,
        store: &GuestStore,
    ) -> Result<Self, Diagnostic> {
        if self.machine.epoch != store.budget().epoch() {
            return Err(invalid(
                "replacement cleanup root differs from reserved old owner",
            ));
        }
        let (cell, location_epoch, expected) = receipt.admit(
            self.cell, self.location_epoch, self.expected, store,
        )?;
        self.cell = cell;
        self.location_epoch = location_epoch;
        self.expected = expected;
        Ok(self)
    }

    pub(crate) fn start(
        mut self,
        store: &mut GuestStore,
        steps: &mut RunSteps,
    ) -> Result<CleanupMachine, Diagnostic> {
        if self.machine.epoch != store.budget().epoch() {
            return Err(invalid("prepared cleanup belongs to another run"));
        }
        if store.location_epoch(self.cell)? != self.location_epoch
            || store.admit_cleanup_root(self.cell)? != self.expected {
            return Err(invalid("prepared cleanup root changed before entry"));
        }
        steps.charge();
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
    /// order. Reserve location storage before moving a field; the guard retains
    /// the exact ticket until this machine completes.
    pub(crate) fn begin_constructor_field(
        guard: ConstructionId,
        frame: FrameId,
        slot: u32,
        store: &mut GuestStore,
        steps: &mut RunSteps,
    ) -> Result<Option<(u32, TicketId, Self)>, Diagnostic> {
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
            completed: 0,
            epoch: store.budget().epoch(),
        };
        machine.reserve_task()?;
        steps.charge();
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
        steps: &mut RunSteps,
    ) -> Result<Self, Diagnostic> {
        Self::prepare_root(cell, store)?.start(store, steps)
    }
    pub(crate) fn prepare_root(
        cell: CellId,
        store: &GuestStore,
    ) -> Result<PreparedCleanup, Diagnostic> {
        let expected = store.admit_cleanup_root(cell)?;
        let mut machine = Self {
            tasks: Vec::new(),
            completed: 0,
            epoch: store.budget().epoch(),
        };
        machine.reserve_task()?;
        Ok(PreparedCleanup {
            machine,
            cell,
            location_epoch: store.location_epoch(cell)?,
            expected,
        })
    }
    /// Reserve the next task slot before any owner moves, so a push cannot
    /// fail after a transfer has started.
    fn reserve_task(&mut self) -> Result<(), Diagnostic> {
        self.tasks.try_reserve(1).map_err(|_| resource())
    }
    pub fn completed_owners(&self) -> u64 {
        self.completed
    }
    /// The demanded `Drop::drop` witness for the inspected value returned (or
    /// unwound): its drop glue continues with the value's fields, in
    /// declaration order, from the same location and reserved ticket.
    pub(crate) fn witness_complete(
        &mut self,
        cell: CellId,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let task = self
            .tasks
            .last_mut()
            .ok_or_else(|| invalid("drop witness task absent"))?;
        if task.cell != cell || task.ticket != ticket || !matches!(task.phase, Phase::Inspect) {
            return Err(invalid("drop witness resumed a different cleanup task"));
        }
        task.phase = Phase::Fields;
        Ok(())
    }
    /// Exactly one transition; even a skipped field returns to the dispatcher.
    /// A nominal policy is demanded only when reached. No absent policy is NoDrop.
    pub fn advance(
        &mut self,
        store: &mut GuestStore,
        steps: &mut RunSteps,
    ) -> Result<CleanupProgress, Diagnostic> {
        if store.budget().epoch() != self.epoch {
            return Err(invalid("cleanup continuation belongs to a different run"));
        }
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
                        steps.charge();
                    }
                    CleanupFieldAdmission::Skip => steps.charge(),
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
