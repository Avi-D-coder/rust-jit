//! Reached scope/discard/replacement cleanup, bound to its exact opcode.
use super::*;
use crate::{
    ir::op::Op,
    vm::cleanup::{CleanupMachine, CleanupProgress},
};

#[derive(Clone, Copy)]
enum Stage {
    Guards {
        next: usize,
        selected: Option<usize>,
    },
    Guard(usize),
    Slots(usize),
    Bindings(usize),
    Parameters(usize),
    Finish,
    DiscardSlot(usize),
    FinishDiscard,
    Replacement,
    FinishReplacement,
}
#[derive(Clone, Copy)]
enum Owner {
    Guard {
        index: usize,
        ordinal: u32,
        ticket: TicketId,
    },
    Slot(usize),
    Binding(usize),
    Parameter(usize),
    Replacement,
}
// Only these four fixed, scalar-field opcodes can own cleanup. Retain their
// exact fields without cloning unrelated large opcode payloads or allocating.
#[derive(Clone, Copy, Eq, PartialEq)]
enum CleanupOperation {
    ScopeLeave {
        scope: ScopeId,
    },
    Discard {
        source: SlotId,
    },
    AssignLocal {
        binding: NodeId,
        source: SlotId,
        destination: SlotId,
    },
    AssignPlace {
        place: SlotId,
        source: SlotId,
        destination: SlotId,
    },
}
impl CleanupOperation {
    fn from_op(op: &Op) -> Option<Self> {
        match op {
            Op::ScopeLeave { scope } => Some(Self::ScopeLeave { scope: *scope }),
            Op::Discard { source } => Some(Self::Discard { source: *source }),
            Op::AssignLocal {
                binding,
                source,
                destination,
            } => Some(Self::AssignLocal {
                binding: *binding,
                source: *source,
                destination: *destination,
            }),
            Op::AssignPlace {
                place,
                source,
                destination,
            } => Some(Self::AssignPlace {
                place: *place,
                source: *source,
                destination: *destination,
            }),
            _ => None,
        }
    }
}
enum Boundary {
    Operation {
        operation_index: u32,
        expected: CleanupOperation,
    },
    Return,
    Panic,
    /// A `break`/`continue` exit leaving every scope through its label's.
    Control(ControlExit),
}
/// A `break` value moved before cleanup and written after it; none for
/// `continue`.
struct ControlExit {
    destination: Option<SlotId>,
    ticket: Option<TicketId>,
    source: Option<crate::traits::term::TyTerm>,
}
struct Replacement {
    /// The assigned local, or none for an assigned place (field, element or
    /// dereference), whose old value the same machine drops.
    binding: Option<NodeId>,
    old: CellId,
    target: CellId,
    unit: TicketId,
    destination: SlotId,
    prepared: Option<crate::vm::cleanup::PreparedCleanup>,
}
/// The pending owner reached a value whose nominal type has a user `Drop`
/// impl. The dispatcher selects `<T as Drop>::drop` and enters it as an
/// ordinary call; its return (or unwind) resumes this activation, whose drop
/// glue then continues with the value's fields.
struct Witness {
    /// The Dropping location and reserved ticket the cleanup machine reported.
    location: CellId,
    ticket: TicketId,
    ty: TypeId,
    /// The lent value's location and the `&mut` receiver argument location.
    lent: Option<(CellId, CellId)>,
    entered: bool,
}
pub(super) struct Activation {
    pc: crate::core::digest::Digest,
    boundary: Boundary,
    target: ScopeId,
    scope: ScopeId,
    stage: Stage,
    pending: Option<(Owner, CleanupMachine)>,
    replacement: Option<Replacement>,
    witness: Option<Witness>,
}
impl Activation {
    pub(super) fn operation_index(&self) -> Option<usize> {
        match self.boundary {
            Boundary::Operation {
                operation_index, ..
            } => Some(operation_index as usize),
            Boundary::Return | Boundary::Panic => None,
            // A control exit follows every operation of its recipe.
            Boundary::Control(_) => Some(usize::MAX),
        }
    }
}
impl Frame {
    pub(crate) fn has_cleanup_activation(&self) -> bool {
        self.cleanup_activation.is_some()
    }
    pub(crate) fn check_cleanup_activation(&self, index: usize, op: &Op) -> Result<(), Diagnostic> {
        let active = self
            .cleanup_activation
            .as_ref()
            .ok_or_else(|| invalid("cleanup activation absent"))?;
        let Boundary::Operation {
            operation_index,
            expected,
        } = &active.boundary
        else {
            return Err(invalid("control cleanup cannot resume as an opcode"));
        };
        if self.copy_activation.is_some()
            || active.pc != self.pc.fingerprint()
            || *operation_index as usize != index
            || Some(*expected) != CleanupOperation::from_op(op)
        {
            return Err(invalid("cleanup resumed at a different bound operation"));
        }
        Ok(())
    }
    fn bindings_first(&self, scope: ScopeId) -> Result<bool, Diagnostic> {
        // A pattern clause's scrutinee temporary, and a pattern function's
        // staged arguments, outlive the bindings taken from them.
        Ok(matches!(
            self.body
                .scopes()
                .get(scope.0 as usize)
                .ok_or_else(|| invalid("cleanup scope absent"))?
                .kind,
            crate::syntax::body_index::ScopeKind::Clause
                | crate::syntax::body_index::ScopeKind::Parameters
        ))
    }
    fn first_cleanup_stage(&self, scope: ScopeId) -> Result<Stage, Diagnostic> {
        self.bindings_first(scope)?;
        Ok(Stage::Guards {
            next: self.slots.len(),
            selected: None,
        })
    }
    fn first_cleanup_storage(&self, scope: ScopeId) -> Result<Stage, Diagnostic> {
        Ok(if self.bindings_first(scope)? {
            Stage::Bindings(self.bindings.len())
        } else {
            Stage::Slots(self.slots.len())
        })
    }
    fn after_cleanup_storage(&self, scope: ScopeId) -> Stage {
        if scope == self.root_scope() {
            Stage::Parameters(self.parameters.len())
        } else {
            Stage::Finish
        }
    }
    /// Install only after the reached opcode entry charge. All owning storage
    /// is visited through bounded transitions, with no eager type/body query.
    pub(crate) fn begin_scope_cleanup(
        &mut self,
        index: usize,
        op: &Op,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        let operation_index =
            u32::try_from(index).map_err(|_| invalid("cleanup opcode index overflow"))?;
        let Some(expected) = CleanupOperation::from_op(op) else {
            return Ok(false);
        };
        let mut replacement = None;
        let (target, scope, stage) = match op {
            Op::ScopeLeave { scope } => {
                self.preflight_cleanup_scope(*scope)?;
                let current = self
                    .active_scopes
                    .last()
                    .copied()
                    .ok_or_else(|| invalid("cleanup scope absent"))?;
                (*scope, current, self.first_cleanup_stage(current)?)
            }
            Op::Discard { source } if self.requires_iterative_discard(*source, store)? => {
                self.preflight_discard(*source, store)?;
                let scope = self.slot(*source)?.scope;
                (scope, scope, Stage::DiscardSlot(source.0 as usize))
            }
            Op::AssignLocal {
                binding,
                source,
                destination,
            } if Self::requires_structural_cleanup(self.binding(*binding)?, store)? => {
                self.preflight_assign(*binding, *source, *destination, store)?;
                if self.copy_activation.is_some() || self.cleanup_activation.is_some() {
                    return Err(invalid("another operation activation is already active"));
                }
                let old = self.binding(*binding)?;
                let owner = CellOwner::FrameSlot {
                    frame: self.anchor,
                    slot: self
                        .body
                        .slot_count()
                        .checked_add(binding.0)
                        .ok_or_else(|| invalid("replacement binding slot overflow"))?,
                };
                if store.owner(old)? != owner && !self.captured.contains(binding) {
                    return Err(invalid(
                        "replacement local location belongs to another frame slot",
                    ));
                }
                let scope = self
                    .body
                    .nodes()
                    .get(binding.0 as usize)
                    .ok_or_else(|| invalid("replacement binding node absent"))?
                    .scope;
                if !self.active_scopes.contains(&scope) && !self.captured.contains(binding) {
                    return Err(invalid("replacement binding scope inactive"));
                }
                let prepared = CleanupMachine::prepare_root(old, store)?;
                store.reserve_bounded_local_replacement(self.slot(*destination)?.cell.is_none())?;
                let cleanup_root = store.new_cell(store.declared(old)?, true, owner)?;
                store.validate_replacement_cleanup_root(
                    cleanup_root,
                    store.admit_cleanup_root(old)?,
                )?;
                self.prepare_write(*destination, store)?;
                let unit = store.ticket(ValueRepr::unit())?;
                let incoming = store.move_from(self.cell(*source)?)?;
                // Commit the new value at the original location before any
                // cleanup transition or callback. Its epoch stays unchanged.
                let receipt = store.replace_for_cleanup(old, incoming, cleanup_root)?;
                let prepared = prepared.relocate_replacement_root(receipt, store)?;
                replacement = Some(Replacement {
                    binding: Some(*binding),
                    old: cleanup_root,
                    target: old,
                    unit,
                    destination: *destination,
                    prepared: Some(prepared),
                });
                (scope, scope, Stage::Replacement)
            }
            Op::AssignPlace {
                place,
                source,
                destination,
            } if let Some((ty, target)) = self.owned_place_replacement(*place, *source, store) => {
                if self.copy_activation.is_some() || self.cleanup_activation.is_some() {
                    return Err(invalid("another operation activation is already active"));
                }
                if self.slot(*destination)?.cell == Some(target) {
                    return Err(invalid("assignment result aliases its reached target"));
                }
                let prepared = CleanupMachine::prepare_root(target, store)?;
                store.reserve_bounded_local_replacement(self.slot(*destination)?.cell.is_none())?;
                let owner = CellOwner::FrameSlot {
                    frame: self.anchor,
                    slot: self
                        .owned_location_base()?
                        .checked_add(2)
                        .ok_or_else(|| invalid("replacement location overflow"))?,
                };
                let cleanup_root = store.new_cell(StorageContract::Exact(ty), true, owner)?;
                store.validate_replacement_cleanup_root(
                    cleanup_root,
                    store.admit_cleanup_root(target)?,
                )?;
                self.prepare_write(*destination, store)?;
                let unit = store.ticket(ValueRepr::unit())?;
                let incoming = store.move_from(self.cell(*source)?)?;
                // Commit the new value at the place before the old value's
                // drop glue (and any `Drop` witness) runs from its own root.
                let receipt = store
                    .replace_place_for_cleanup(self.executed_place(*place)?, incoming, cleanup_root)
                    .map_err(|failure| match failure {
                        crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                        crate::runtime::place::PlaceFailure::GuestPanic(_) => {
                            invalid("admitted place replacement panicked")
                        }
                    })?;
                let prepared = prepared.relocate_replacement_root(receipt, store)?;
                self.clear_place(*place)?;
                replacement = Some(Replacement {
                    binding: None,
                    old: cleanup_root,
                    target,
                    unit,
                    destination: *destination,
                    prepared: Some(prepared),
                });
                let scope = self
                    .active_scopes
                    .last()
                    .copied()
                    .ok_or_else(|| invalid("replacement scope absent"))?;
                (scope, scope, Stage::Replacement)
            }
            _ => return Ok(false),
        };
        if self.copy_activation.is_some() || self.cleanup_activation.is_some() {
            return Err(invalid("another operation activation is already active"));
        }
        self.cleanup_activation = Some(Activation {
            pc: self.pc.fingerprint(),
            boundary: Boundary::Operation {
                operation_index,
                expected,
            },
            target,
            scope,
            stage,
            pending: None,
            replacement,
            witness: None,
        });
        Ok(true)
    }
    /// A Returned frame retains its external return ticket in the dispatcher.
    /// This boundary performs no opcode/exit charge and never executes a thunk.
    pub fn advance_return_cleanup(&mut self, store: &mut GuestStore) -> Result<bool, Diagnostic> {
        let result = (|| {
            if self.status != FrameStatus::Returned || self.copy_activation.is_some() {
                return Err(invalid("return cleanup requires an actual returned frame"));
            }
            if let Some(active) = &self.cleanup_activation {
                if active.pc != self.pc.fingerprint()
                    || !matches!(active.boundary, Boundary::Return)
                {
                    return Err(invalid("return cleanup boundary changed during suspension"));
                }
            } else {
                let Some(scope) = self.active_scopes.last().copied() else {
                    return Ok(true);
                };
                self.preflight_cleanup_scope(scope)?;
                self.cleanup_activation = Some(Activation {
                    pc: self.pc.fingerprint(),
                    boundary: Boundary::Return,
                    target: self.root_scope(),
                    scope,
                    stage: self.first_cleanup_stage(scope)?,
                    pending: None,
                    replacement: None,
                    witness: None,
                });
            }
            self.advance_scope_cleanup(store)
        })();
        if result.is_err() {
            self.status = FrameStatus::Faulted;
        }
        result
    }
    /// Retain an ordinary guest panic while draining the same bounded owner
    /// machine. A fatal cleanup error stops execution with its obligation intact.
    pub fn advance_panic_cleanup(&mut self, store: &mut GuestStore) -> Result<bool, Diagnostic> {
        let result = (|| {
            if self.status != FrameStatus::Panicked || self.copy_activation.is_some() {
                return Err(invalid("panic cleanup requires an actual guest panic"));
            }
            if let Some(active) = &self.cleanup_activation {
                if active.pc != self.pc.fingerprint() || !matches!(active.boundary, Boundary::Panic)
                {
                    return Err(invalid("panic cleanup boundary changed during suspension"));
                }
            } else {
                let Some(scope) = self.active_scopes.last().copied() else {
                    self.status = FrameStatus::PanicCleaned;
                    return Ok(true);
                };
                self.preflight_cleanup_scope(scope)?;
                self.cleanup_activation = Some(Activation {
                    pc: self.pc.fingerprint(),
                    boundary: Boundary::Panic,
                    target: self.root_scope(),
                    scope,
                    stage: self.first_cleanup_stage(scope)?,
                    pending: None,
                    replacement: None,
                    witness: None,
                });
            }
            let complete = self.advance_scope_cleanup(store)?;
            if complete {
                self.status = FrameStatus::PanicCleaned;
            }
            Ok(complete)
        })();
        if result.is_err() {
            self.status = FrameStatus::Faulted;
        }
        result
    }
    fn clear_cleanup_slot(&mut self, index: usize) -> Result<(), Diagnostic> {
        let slot = self
            .slots
            .get_mut(index)
            .ok_or_else(|| invalid("cleanup slot absent"))?;
        let bytes = slot
            .place
            .as_ref()
            .map_or(Ok(0), |place| place.projection_bytes())?;
        self.projection_bytes = self
            .projection_bytes
            .checked_sub(bytes)
            .ok_or_else(|| invalid("cleanup place accounting underflow"))?;
        slot.cell = None;
        slot.source = None;
        slot.place = None;
        slot.hint = None;
        slot.strict_hint = true;
        Ok(())
    }
    fn clear_cleanup_owner(&mut self, owner: Owner) -> Result<(), Diagnostic> {
        match owner {
            Owner::Guard { .. } => {
                return Err(invalid("guard cleanup requires child acknowledgement"));
            }
            Owner::Replacement => return Err(invalid("replacement owns its final publication")),
            Owner::Slot(index) => self.clear_cleanup_slot(index)?,
            Owner::Binding(index) => {
                *self
                    .bindings
                    .get_mut(index)
                    .ok_or_else(|| invalid("cleanup binding absent"))? = None;
                self.set_binding_source_view(
                    NodeId(
                        u32::try_from(index)
                            .map_err(|_| invalid("binding source index overflow"))?,
                    ),
                    None,
                )?;
            }
            Owner::Parameter(index) => {
                *self
                    .parameters
                    .get_mut(index)
                    .ok_or_else(|| invalid("cleanup parameter absent"))? = None;
            }
        }
        Ok(())
    }
    fn finish_cleanup_owner(
        &mut self,
        owner: Owner,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        if let Owner::Guard {
            index,
            ordinal,
            ticket,
        } = owner
        {
            let guard = self
                .slots
                .get(index)
                .and_then(|slot| slot.construction.as_ref())
                .ok_or_else(|| invalid("pending construction guard absent"))?
                .guard;
            store.finish_construction_cleanup_field(guard, ordinal, ticket)
        } else if matches!(owner, Owner::Replacement) {
            Ok(())
        } else {
            self.clear_cleanup_owner(owner)
        }
    }
    fn start_cleanup_guard(
        &mut self,
        active: &mut Activation,
        index: usize,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let construction = self
            .slots
            .get(index)
            .and_then(|slot| slot.construction.as_ref())
            .ok_or_else(|| invalid("selected construction guard absent"))?;
        let guard = construction.guard;
        let mapping = construction
            .mapping_bytes()
            .ok_or_else(crate::runtime::arena::resource)?;
        let remaining = self
            .constructor_mapping_bytes
            .checked_sub(mapping)
            .ok_or_else(|| invalid("constructor mapping charge mismatch"))?;
        let slot = u32::try_from(index).map_err(|_| invalid("guard slot ordinal overflow"))?;
        match CleanupMachine::begin_constructor_field(
            guard,
            self.anchor,
            slot,
            store,
            &mut self.fuel,
        )? {
            Some((ordinal, ticket, machine)) => {
                self.slots[index]
                    .construction
                    .as_mut()
                    .ok_or_else(|| invalid("transferred construction guard absent"))?
                    .cleaning = true;
                active.pending = Some((
                    Owner::Guard {
                        index,
                        ordinal,
                        ticket,
                    },
                    machine,
                ));
            }
            None => {
                self.slots[index].construction = None;
                self.constructor_mapping_bytes = remaining;
                active.stage = self.first_cleanup_stage(active.scope)?;
            }
        }
        Ok(())
    }
    fn start_cleanup_owner(
        &mut self,
        active: &mut Activation,
        owner: Owner,
        cell: CellId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        match store.state(cell)? {
            CellState::Live(_) => {
                let machine = CleanupMachine::begin(cell, store, &mut self.fuel)?;
                active.pending = Some((owner, machine));
            }
            CellState::Uninit | CellState::Moved => {
                store.admit_discard(cell)?;
                let completed = self
                    .counters
                    .cleanup_elements_completed
                    .checked_add(1)
                    .ok_or_else(crate::runtime::arena::resource)?;
                self.count_step();
                store.retire_cell(cell)?;
                self.clear_cleanup_owner(owner)?;
                self.counters.cleanup_elements_completed = completed;
            }
            _ => return Err(invalid("cleanup owner already retired or dropping")),
        }
        Ok(())
    }
    /// At most256 metadata/owner transitions, then return Dispatch at the same
    /// opcode. Errors retain the exact activation for the fault diagnostic.
    pub(crate) fn advance_scope_cleanup(
        &mut self,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        let mut active = self
            .cleanup_activation
            .take()
            .ok_or_else(|| invalid("cleanup activation absent"))?;
        let result = (|| {
            if active.witness.is_some() {
                return Err(invalid("cleanup advanced while its Drop witness is pending"));
            }
            for _ in 0..256 {
                if let Some((owner, machine)) = active.pending.as_mut() {
                    match machine.advance(store, &mut self.fuel)? {
                        CleanupProgress::Advanced => {}
                        CleanupProgress::Complete => {
                            let completed = self
                                .counters
                                .cleanup_elements_completed
                                .checked_add(machine.completed_owners())
                                .ok_or_else(crate::runtime::arena::resource)?;
                            self.finish_cleanup_owner(*owner, store)?;
                            self.counters.cleanup_elements_completed = completed;
                            active.pending = None;
                        }
                        CleanupProgress::NeedWitness { cell, ticket, ty } => {
                            active.witness = Some(Witness {
                                location: cell,
                                ticket,
                                ty,
                                lent: None,
                                entered: false,
                            });
                            return Ok(false);
                        }
                    }
                    continue;
                }
                match active.stage {
                    Stage::Replacement => {
                        let replacement = active
                            .replacement
                            .as_mut()
                            .ok_or_else(|| invalid("replacement continuation absent"))?;
                        let prepared = replacement
                            .prepared
                            .take()
                            .ok_or_else(|| invalid("replacement cleanup was already entered"))?;
                        let machine = prepared.start(store, &mut self.fuel)?;
                        active.pending = Some((Owner::Replacement, machine));
                        active.stage = Stage::FinishReplacement;
                    }
                    Stage::FinishReplacement => {
                        let replacement = active
                            .replacement
                            .as_ref()
                            .ok_or_else(|| invalid("replacement continuation absent"))?;
                        let target = match replacement.binding {
                            Some(binding) => self.binding(binding)?,
                            None => replacement.target,
                        };
                        if target != replacement.target
                            || store.state(replacement.old)? != CellState::Dropped
                        {
                            return Err(invalid("replacement location changed during cleanup"));
                        }
                        // The replacement is already committed. Only assignment's
                        // Unit result waits for the old drop obligation to finish.
                        self.write(replacement.destination, replacement.unit, store)?;
                        return Ok(true);
                    }
                    Stage::DiscardSlot(index) => {
                        let cell = self
                            .slots
                            .get(index)
                            .and_then(|slot| slot.cell)
                            .ok_or_else(|| invalid("discard owner vanished before cleanup"))?;
                        self.start_cleanup_owner(&mut active, Owner::Slot(index), cell, store)?;
                        active.stage = Stage::FinishDiscard;
                    }
                    // Unlike ScopeLeave, the reached discard never scans other
                    // owners or retires a lexical scope. Its original recipe
                    // executes any following ScopeLeave only after completion.
                    Stage::FinishDiscard => return Ok(true),
                    Stage::Guards { next: 0, selected } => {
                        active.stage = if let Some(index) = selected {
                            Stage::Guard(index)
                        } else {
                            self.first_cleanup_storage(active.scope)?
                        };
                    }
                    Stage::Guards { next, mut selected } => {
                        let index = next - 1;
                        let slot = self
                            .slots
                            .get(index)
                            .ok_or_else(|| invalid("cleanup guard cursor invalid"))?;
                        if slot.scope == active.scope
                            && let Some(construction) = &slot.construction
                        {
                            let latest =
                                selected.and_then(|index| self.slots[index].construction.as_ref());
                            if latest.is_none_or(|latest| {
                                latest.guard.bits() < construction.guard.bits()
                            }) {
                                selected = Some(index);
                            }
                        }
                        active.stage = Stage::Guards {
                            next: index,
                            selected,
                        };
                    }
                    Stage::Guard(index) => self.start_cleanup_guard(&mut active, index, store)?,
                    Stage::Slots(0) => {
                        active.stage = if self.bindings_first(active.scope)? {
                            self.after_cleanup_storage(active.scope)
                        } else {
                            Stage::Bindings(self.bindings.len())
                        };
                    }
                    Stage::Slots(next) => {
                        let index = next - 1;
                        let slot = self
                            .slots
                            .get(index)
                            .ok_or_else(|| invalid("cleanup slot cursor invalid"))?;
                        if slot.scope == active.scope {
                            if let Some(cell) = slot.cell {
                                self.start_cleanup_owner(
                                    &mut active,
                                    Owner::Slot(index),
                                    cell,
                                    store,
                                )?;
                            } else {
                                self.clear_cleanup_slot(index)?;
                            }
                        }
                        active.stage = Stage::Slots(index);
                    }
                    Stage::Bindings(0) => {
                        active.stage = if self.bindings_first(active.scope)? {
                            Stage::Slots(self.slots.len())
                        } else {
                            self.after_cleanup_storage(active.scope)
                        };
                    }
                    Stage::Bindings(next) => {
                        let index = next - 1;
                        let node = self
                            .body
                            .nodes()
                            .get(index)
                            .ok_or_else(|| invalid("cleanup binding cursor invalid"))?;
                        if node.scope == active.scope {
                            if let Some(cell) = self.bindings.get(index).copied().flatten() {
                                self.start_cleanup_owner(
                                    &mut active,
                                    Owner::Binding(index),
                                    cell,
                                    store,
                                )?;
                            } else {
                                self.set_binding_source_view(
                                    NodeId(u32::try_from(index).map_err(|_| {
                                        invalid("binding source cleanup index overflow")
                                    })?),
                                    None,
                                )?;
                            }
                        }
                        active.stage = Stage::Bindings(index);
                    }
                    Stage::Parameters(0) => active.stage = Stage::Finish,
                    Stage::Parameters(next) => {
                        let index = next - 1;
                        if let Some(cell) = self.parameters.get(index).copied().flatten() {
                            self.start_cleanup_owner(
                                &mut active,
                                Owner::Parameter(index),
                                cell,
                                store,
                            )?;
                        }
                        active.stage = Stage::Parameters(index);
                    }
                    Stage::Finish => {
                        if self.active_scopes.last() != Some(&active.scope) {
                            return Err(invalid("cleanup scope changed during suspension"));
                        }
                        self.preflight_leave(active.scope)?;
                        if active.scope == self.root_scope() {
                            self.clear_parameter_source_views()?;
                        }
                        if matches!(active.boundary, Boundary::Return) {
                            self.counters.return_scopes_cleaned = self
                                .counters
                                .return_scopes_cleaned
                                .checked_add(1)
                                .ok_or_else(crate::runtime::arena::resource)?;
                        }
                        if matches!(active.boundary, Boundary::Panic) {
                            self.counters.panic_scopes_cleaned = self
                                .counters
                                .panic_scopes_cleaned
                                .checked_add(1)
                                .ok_or_else(crate::runtime::arena::resource)?;
                        }
                        self.open_patterns.retire_scope(active.scope);
                        self.active_scopes.pop();
                        if active.scope == active.target {
                            if let Boundary::Control(exit) = &mut active.boundary
                                && let (Some(destination), Some(ticket)) =
                                    (exit.destination, exit.ticket.take())
                            {
                                self.write(destination, ticket, store)?;
                                self.set_source_view(destination, exit.source.take())?;
                            }
                            return Ok(true);
                        }
                        active.scope = self
                            .active_scopes
                            .last()
                            .copied()
                            .ok_or_else(|| invalid("cleanup prefix lost target"))?;
                        self.preflight_cleanup_scope(active.scope)?;
                        active.stage = self.first_cleanup_stage(active.scope)?;
                    }
                }
            }
            Ok(false)
        })();
        self.counters.fuel_used = self.fuel.snapshot().used;
        if !matches!(result, Ok(true)) {
            self.cleanup_activation = Some(active);
        }
        result
    }
    pub(crate) fn has_control_cleanup(&self) -> bool {
        self.cleanup_activation
            .as_ref()
            .is_some_and(|active| matches!(active.boundary, Boundary::Control(_)))
    }
    /// A `break`/`continue` exit: move its value, then leave every scope
    /// through the label's with the same bounded owner machine as scope exit
    /// (so drop glue and `Drop` witnesses run), then write the value. False
    /// means the exit resumes later at this PC, after its exit preflight.
    pub(crate) fn control_transfer(
        &mut self,
        label: crate::syntax::body_index::LabelId,
        value: Option<SlotId>,
        continuing: bool,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        match &self.cleanup_activation {
            Some(active) => {
                if active.pc != self.pc.fingerprint()
                    || !matches!(active.boundary, Boundary::Control(_))
                {
                    return Err(invalid("control cleanup resumed at a different exit"));
                }
            }
            None => {
                self.preflight_control(label, value, continuing, store)?;
                if self.copy_activation.is_some() {
                    return Err(invalid("another operation activation is already active"));
                }
                let target = self
                    .body
                    .labels()
                    .get(label.0 as usize)
                    .cloned()
                    .ok_or_else(|| invalid("control target absent"))?;
                if !self.active_scopes.contains(&target.scope) || continuing && !target.is_loop
                {
                    return Err(invalid("control target inactive or not a loop"));
                }
                let destination = if continuing {
                    None
                } else {
                    Some(
                        self.body.nodes()[target.node.0 as usize]
                            .value
                            .ok_or_else(|| invalid("control target result absent"))?,
                    )
                };
                let source = value
                    .map(|slot| self.source_view(slot))
                    .transpose()?
                    .flatten();
                let ticket = if continuing {
                    None
                } else if let Some(source) = value {
                    Some(store.move_from(self.cell(source)?)?)
                } else {
                    Some(store.ticket(ValueRepr::unit())?)
                };
                let scope = self
                    .active_scopes
                    .last()
                    .copied()
                    .ok_or_else(|| invalid("control cleanup lost target"))?;
                self.preflight_cleanup_scope(scope)?;
                self.cleanup_activation = Some(Activation {
                    pc: self.pc.fingerprint(),
                    boundary: Boundary::Control(ControlExit {
                        destination,
                        ticket,
                        source,
                    }),
                    target: target.scope,
                    scope,
                    stage: self.first_cleanup_stage(scope)?,
                    pending: None,
                    replacement: None,
                    witness: None,
                });
            }
        }
        self.advance_scope_cleanup(store)
    }
    /// Cleanup-owned locations of this frame (a Drop witness's receiver and
    /// argument, a replaced place's old value) follow its slot, binding and
    /// parameter locations; each lives only for its activation.
    pub(super) fn owned_location_base(&self) -> Result<u32, Diagnostic> {
        u32::try_from(self.parameters.len())
            .ok()
            .and_then(|parameters| {
                self.body.slot_count().checked_add(
                    u32::try_from(self.body.nodes().len())
                        .ok()?
                        .checked_add(parameters)?,
                )
            })
            .ok_or_else(|| invalid("cleanup location overflow"))
    }
    /// An executed place assignment whose old value owns drop glue.
    fn owned_place_replacement(
        &self,
        place: SlotId,
        source: SlotId,
        store: &GuestStore,
    ) -> Option<(TypeId, CellId)> {
        match store.preview_intrinsic_place_assignment(
            self.executed_place(place).ok()?,
            self.cell(source).ok()?,
        ) {
            Ok(crate::runtime::cell::IntrinsicAssignment::ReplaceOwned { ty, target }) => {
                Some((ty, target))
            }
            _ => None,
        }
    }
    /// The value type whose selected `Drop::drop` this frame's pending
    /// cleanup owner awaits, until the dispatcher enters that call.
    pub(crate) fn drop_witness_demand(&self) -> Option<TypeId> {
        self.cleanup_activation
            .as_ref()?
            .witness
            .as_ref()
            .filter(|witness| !witness.entered)
            .map(|witness| witness.ty)
    }
    fn witness_mut(&mut self) -> Result<&mut Witness, Diagnostic> {
        self.cleanup_activation
            .as_mut()
            .and_then(|active| active.witness.as_mut())
            .ok_or_else(|| invalid("drop witness absent"))
    }
    /// Lend the value being dropped to a fresh writable location of this
    /// frame and form its `&mut` receiver in a fresh argument location, as an
    /// ordinary borrow of that place. Repeating before entry reuses both.
    /// Returns the argument location, its source view and its descriptor.
    pub(crate) fn lend_drop_witness(
        &mut self,
        store: &mut GuestStore,
    ) -> Result<(CellId, crate::traits::term::TyTerm, TypeId), Diagnostic> {
        use crate::{
            runtime::{place::Place, reference::Access},
            traits::term::{RegionTerm, TyTerm},
        };
        self.require_store(store)?;
        let range = self
            .body
            .nodes()
            .get(self.pc.node as usize)
            .ok_or_else(|| invalid("drop witness node absent"))?
            .range;
        let definition = self.body.body().definition;
        let anchor = self.anchor;
        let base = self.owned_location_base()?;
        let witness = self.witness_mut()?;
        if witness.entered {
            return Err(invalid("drop witness already entered"));
        }
        let pointee = store.descriptor_source_term(witness.ty).ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "Drop receiver type requires its closed source descriptor".into(),
            )
        })?;
        let source = TyTerm::Ref(
            RegionTerm::Local {
                definition,
                start: range.start,
                end: range.end,
            },
            true,
            Box::new(pointee),
        );
        let argument = match witness.lent {
            Some((_, argument)) => argument,
            None => {
                let receiver = store.new_cell(
                    StorageContract::Exact(witness.ty),
                    true,
                    CellOwner::FrameSlot {
                        frame: anchor,
                        slot: base,
                    },
                )?;
                store.lend_cleanup_ticket(witness.ticket, receiver)?;
                let reference = store
                    .borrow_ticket(&Place::cell(receiver), Access::Mutable)
                    .map_err(|failure| match failure {
                        crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                        crate::runtime::place::PlaceFailure::GuestPanic(_) => {
                            invalid("drop receiver borrow panicked")
                        }
                    })?;
                let ty = TypeId(store.peek_ticket(reference)?.type_id);
                let argument = store.new_cell(
                    StorageContract::Exact(ty),
                    false,
                    CellOwner::FrameSlot {
                        frame: anchor,
                        slot: base
                            .checked_add(1)
                            .ok_or_else(|| invalid("drop witness location overflow"))?,
                    },
                )?;
                store.initialize(argument, reference)?;
                witness.lent = Some((receiver, argument));
                argument
            }
        };
        let descriptor = TypeId(store.admit_move(argument)?.type_id);
        Ok((argument, source, descriptor))
    }
    /// Enter the lent witness: the callee frame shares this run's step and
    /// action observation, like any callee entered from this frame.
    pub(crate) fn enter_drop_witness(
        &mut self,
        body: Arc<BodyIndex>,
        entry: PcKey,
        store: &mut GuestStore,
    ) -> Result<Self, Diagnostic> {
        self.require_store(store)?;
        let witness = self.witness_mut()?;
        if witness.entered || witness.lent.is_none() {
            return Err(invalid("drop witness entry requires its lent receiver"));
        }
        let mut frame = Self::new(body, entry, store)?;
        frame.fuel = self.fuel.share();
        frame.actions = self.actions.share();
        self.witness_mut()?.entered = true;
        Ok(frame)
    }
    /// The witness returned: reclaim the (possibly reassigned) value into its
    /// reserved ticket and continue that value's drop glue with its fields.
    pub(crate) fn finish_drop_witness(&mut self, store: &mut GuestStore) -> Result<(), Diagnostic> {
        let active = self
            .cleanup_activation
            .as_mut()
            .ok_or_else(|| invalid("drop witness activation absent"))?;
        let witness = active
            .witness
            .take()
            .ok_or_else(|| invalid("drop witness absent"))?;
        let (true, Some((receiver, argument))) = (witness.entered, witness.lent) else {
            return Err(invalid("drop witness resumed before entry"));
        };
        store.reclaim_cleanup_ticket(witness.ticket, receiver)?;
        store.retire_cell(argument)?;
        let (_, machine) = active
            .pending
            .as_mut()
            .ok_or_else(|| invalid("drop witness owner absent"))?;
        machine.witness_complete(witness.location, witness.ticket)
    }
    /// The witness unwound: finish it as above, then continue this frame's
    /// cleanup as panic cleanup (the value's fields, then every remaining
    /// owner of every active scope), as rustc's unwind path does.
    pub(crate) fn unwind_drop_witness(&mut self, store: &mut GuestStore) -> Result<(), Diagnostic> {
        self.finish_drop_witness(store)?;
        let scope = self
            .active_scopes
            .last()
            .copied()
            .ok_or_else(|| invalid("unwinding cleanup scope absent"))?;
        self.preflight_cleanup_scope(scope)?;
        let first = self.first_cleanup_stage(scope)?;
        let root = self.root_scope();
        let active = self
            .cleanup_activation
            .as_mut()
            .ok_or_else(|| invalid("drop witness activation absent"))?;
        if matches!(active.boundary, Boundary::Panic) {
            return Err(Diagnostic::new(
                "LRR-E-DOUBLE-PANIC",
                "a Drop witness panicked during unwinding".into(),
            ));
        }
        // Assignment already committed its new value; its unit result is
        // never published on the unwind path, nor is a pending `break` value.
        if let Some(replacement) = active.replacement.take() {
            store.discard(replacement.unit)?;
        }
        if let Boundary::Control(exit) = &mut active.boundary
            && let Some(ticket) = exit.ticket.take()
        {
            match store.cleanup_ticket_inventory(ticket)? {
                crate::runtime::cell::CleanupInventory::Leaf => store.discard(ticket)?,
                _ => {
                    return Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "unwinding from a Drop witness abandons an owning break value".into(),
                    ));
                }
            }
        }
        if active.scope != scope
            || matches!(
                active.stage,
                Stage::DiscardSlot(_)
                    | Stage::FinishDiscard
                    | Stage::Replacement
                    | Stage::FinishReplacement
            )
        {
            active.stage = first;
        }
        active.scope = scope;
        active.target = root;
        active.boundary = Boundary::Panic;
        self.status = FrameStatus::Panicked;
        Ok(())
    }
}

#[cfg(test)]
impl Frame {
    pub(super) fn replacement_frontier_for_test(&self) -> (CellId, CellId, TicketId) {
        let replacement = self
            .cleanup_activation
            .as_ref()
            .unwrap()
            .replacement
            .as_ref()
            .unwrap();
        (replacement.target, replacement.old, replacement.unit)
    }
}
