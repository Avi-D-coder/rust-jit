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
// Only these three fixed, scalar-field opcodes can own cleanup. Retain their
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
}
struct Replacement {
    binding: NodeId,
    old: CellId,
    target: CellId,
    unit: TicketId,
    destination: SlotId,
    prepared: Option<crate::vm::cleanup::PreparedCleanup>,
}
pub(super) struct Activation {
    pc: crate::core::digest::Digest,
    boundary: Boundary,
    target: ScopeId,
    scope: ScopeId,
    stage: Stage,
    pending: Option<(Owner, CleanupMachine)>,
    replacement: Option<Replacement>,
}
impl Activation {
    pub(super) fn operation_index(&self) -> Option<usize> {
        match self.boundary {
            Boundary::Operation {
                operation_index, ..
            } => Some(operation_index as usize),
            Boundary::Return | Boundary::Panic => None,
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
        Ok(self
            .body
            .scopes()
            .get(scope.0 as usize)
            .ok_or_else(|| invalid("cleanup scope absent"))?
            .kind
            == crate::syntax::body_index::ScopeKind::Clause)
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
        if scope == ScopeId(0) {
            Stage::Parameters(self.parameters.len())
        } else {
            Stage::Finish
        }
    }
    fn cleanup_metadata_limit(&self) -> Result<usize, Diagnostic> {
        let base = Self::metadata_bytes(&self.body, self.parameter_metadata_count())
            .and_then(|n| n.checked_add(self.constructor_mapping_bytes))
            .and_then(|n| n.checked_add(self.projection_bytes))
            .and_then(|n| n.checked_add(self.source_field_bytes()?))
            .ok_or_else(crate::runtime::arena::resource)?;
        (32_usize * 1024 * 1024)
            .checked_sub(base)
            .ok_or_else(crate::runtime::arena::resource)
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
                if store.owner(old)? != owner {
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
                if !self.active_scopes.contains(&scope) {
                    return Err(invalid("replacement binding scope inactive"));
                }
                let prepared =
                    CleanupMachine::prepare_root(old, store, self.cleanup_metadata_limit()?)?;
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
                let old_ticket = store.replace(old, incoming)?;
                store.initialize_replacement_cleanup_root(cleanup_root, old_ticket)?;
                let prepared = prepared.relocate_replacement_root(cleanup_root, store)?;
                replacement = Some(Replacement {
                    binding: *binding,
                    old: cleanup_root,
                    target: old,
                    unit,
                    destination: *destination,
                    prepared: Some(prepared),
                });
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
                    target: ScopeId(0),
                    scope,
                    stage: self.first_cleanup_stage(scope)?,
                    pending: None,
                    replacement: None,
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
                    target: ScopeId(0),
                    scope,
                    stage: self.first_cleanup_stage(scope)?,
                    pending: None,
                    replacement: None,
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
                *self
                    .binding_sources
                    .get_mut(index)
                    .ok_or_else(|| invalid("cleanup binding source absent"))? = None;
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
        let limit = self.cleanup_metadata_limit()?;
        match CleanupMachine::begin_constructor_field(
            guard,
            self.anchor,
            slot,
            store,
            &mut self.fuel,
            limit,
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
                let limit = self.cleanup_metadata_limit()?;
                let machine = CleanupMachine::begin(cell, store, &mut self.fuel, limit)?;
                active.pending = Some((owner, machine));
            }
            CellState::Uninit | CellState::Moved => {
                store.admit_discard(cell)?;
                let completed = self
                    .counters
                    .cleanup_elements_completed
                    .checked_add(1)
                    .ok_or_else(crate::runtime::arena::resource)?;
                self.fuel.charge()?;
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
            for _ in 0..256 {
                self.fuel.poll()?;
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
                        CleanupProgress::NeedWitness { .. } => {
                            return Err(Diagnostic::new(
                                "LRR-E-UNSUPPORTED",
                                "cleanup requires its source-selected Drop witness".into(),
                            ));
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
                        if self.binding(replacement.binding)? != replacement.target
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
                        if node.scope == active.scope
                            && let Some(cell) = self.bindings.get(index).copied().flatten()
                        {
                            self.start_cleanup_owner(
                                &mut active,
                                Owner::Binding(index),
                                cell,
                                store,
                            )?;
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
                        if active.scope == ScopeId(0) {
                            self.parameter_sources.clear();
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
                        self.active_scopes.pop();
                        if active.scope == active.target {
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
