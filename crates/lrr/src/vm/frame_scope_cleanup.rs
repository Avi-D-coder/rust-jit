//! Reached scope/discard/replacement cleanup, bound to its exact opcode.
use super::*;
use crate::{
    ir::op::Op,
    vm::cleanup::{CleanupMachine, CleanupProgress},
};

/// Scope-exit cursors count down through the retained per-scope owner groups
/// of the indexed body (`BodyIndex::scope_slots`/`scope_nodes`, ascending
/// ordinals): `Guards`, `Slots` and `Bindings` hold how many group members
/// remain, so the member visited next is `group[next - 1]`. A scope exit
/// therefore visits exactly its own owners, in the same descending ordinal
/// order the former whole-frame scan produced. `Guard`, `DiscardSlot` and
/// `Parameters` hold frame slot or parameter ordinals directly.
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
    /// The exact operation this activation is bound to (compared directly:
    /// equal keys, not a hash of them, bind the continuation).
    pc: PcKey,
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
            || active.pc != self.pc
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
            next: self.body.scope_slots(scope).len(),
            selected: None,
        })
    }
    fn first_cleanup_storage(&self, scope: ScopeId) -> Result<Stage, Diagnostic> {
        Ok(if self.bindings_first(scope)? {
            Stage::Bindings(self.body.scope_nodes(scope).len())
        } else {
            Stage::Slots(self.body.scope_slots(scope).len())
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
            pc: self.pc.clone(),
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
                if active.pc != self.pc
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
                    pc: self.pc.clone(),
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
                if active.pc != self.pc || !matches!(active.boundary, Boundary::Panic)
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
                    pc: self.pc.clone(),
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
                        // No side effects: walk the scope's whole slot group
                        // once, selecting the construction with the latest guard.
                        let group = self.body.scope_slots(active.scope);
                        if next > group.len() {
                            return Err(invalid("cleanup guard cursor invalid"));
                        }
                        for &ordinal in group[..next].iter().rev() {
                            let index = ordinal as usize;
                            let slot = self
                                .slots
                                .get(index)
                                .ok_or_else(|| invalid("cleanup guard cursor invalid"))?;
                            if let Some(construction) = &slot.construction {
                                let latest = selected
                                    .and_then(|index| self.slots[index].construction.as_ref());
                                if latest.is_none_or(|latest| {
                                    latest.guard.bits() < construction.guard.bits()
                                }) {
                                    selected = Some(index);
                                }
                            }
                        }
                        active.stage = Stage::Guards { next: 0, selected };
                    }
                    Stage::Guard(index) => self.start_cleanup_guard(&mut active, index, store)?,
                    Stage::Slots(0) => {
                        active.stage = if self.bindings_first(active.scope)? {
                            self.after_cleanup_storage(active.scope)
                        } else {
                            Stage::Bindings(self.body.scope_nodes(active.scope).len())
                        };
                    }
                    Stage::Slots(mut next) => {
                        // Slots without a cell are cleared inline (their order
                        // and effects are those of one transition each); the
                        // first slot owning a cell ends this transition.
                        while next > 0 {
                            let index = *self
                                .body
                                .scope_slots(active.scope)
                                .get(next - 1)
                                .ok_or_else(|| invalid("cleanup slot cursor invalid"))?
                                as usize;
                            let slot = self
                                .slots
                                .get(index)
                                .ok_or_else(|| invalid("cleanup slot cursor invalid"))?;
                            active.stage = Stage::Slots(next);
                            let owning = slot.cell;
                            if let Some(cell) = owning {
                                self.start_cleanup_owner(
                                    &mut active,
                                    Owner::Slot(index),
                                    cell,
                                    store,
                                )?;
                            } else {
                                self.clear_cleanup_slot(index)?;
                            }
                            next -= 1;
                            if owning.is_some() {
                                break;
                            }
                        }
                        active.stage = Stage::Slots(next);
                    }
                    Stage::Bindings(0) => {
                        active.stage = if self.bindings_first(active.scope)? {
                            Stage::Slots(self.body.scope_slots(active.scope).len())
                        } else {
                            self.after_cleanup_storage(active.scope)
                        };
                    }
                    Stage::Bindings(mut next) => {
                        // Nodes holding neither a binding nor a source view
                        // need no transition: clearing an absent view is a
                        // no-op. The first node owning a binding cell ends this
                        // transition.
                        while next > 0 {
                            let index = *self
                                .body
                                .scope_nodes(active.scope)
                                .get(next - 1)
                                .ok_or_else(|| invalid("cleanup binding cursor invalid"))?
                                as usize;
                            active.stage = Stage::Bindings(next);
                            let owning = self.bindings.get(index).copied().flatten();
                            if let Some(cell) = owning {
                                self.start_cleanup_owner(
                                    &mut active,
                                    Owner::Binding(index),
                                    cell,
                                    store,
                                )?;
                            } else if self
                                .binding_sources
                                .get(index)
                                .is_none_or(|source| source.is_some())
                            {
                                self.set_binding_source_view(
                                    NodeId(u32::try_from(index).map_err(|_| {
                                        invalid("binding source cleanup index overflow")
                                    })?),
                                    None,
                                )?;
                            }
                            next -= 1;
                            if owning.is_some() {
                                break;
                            }
                        }
                        active.stage = Stage::Bindings(next);
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
                if active.pc != self.pc
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
                    pc: self.pc.clone(),
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

/// Differential control: the retained per-scope owner groups a scope exit
/// walks must reproduce, for every scope of every body, the exact descending
/// sequence the former whole-frame scans visited, and the frame's slot homes
/// must equal the former per-call node walk. The references below are the
/// removed production scans, kept only here.
#[cfg(test)]
mod owner_group_tests {
    use super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        ir::pc::Phase,
        source::snapshot::{CfgSet, SourceBlob},
        syntax::{
            body_index::{ExpressionKind, NodeRole},
            parse::ParsedFile,
        },
    };

    /// The removed `Frame::new` node walk, verbatim in effect.
    fn reference_slot_homes(body: &BodyIndex) -> Vec<(ScopeId, Option<NodeId>)> {
        let mut homes = vec![(ScopeId(0), None); body.slot_count() as usize];
        for node in body.nodes() {
            let result_scope = if matches!(
                node.role,
                NodeRole::Expression(
                    ExpressionKind::Block
                        | ExpressionKind::Const
                        | ExpressionKind::While
                        | ExpressionKind::Loop
                        | ExpressionKind::For
                )
            ) {
                body.scopes()[node.scope.0 as usize].parent.unwrap_or(node.scope)
            } else {
                node.scope
            };
            if let Some(id) = node.value {
                if node
                    .extended_to
                    .as_ref()
                    .is_some_and(|extension| !extension.constructors.is_empty())
                {
                    homes[id.0 as usize].1 = Some(node.id);
                }
                homes[id.0 as usize].0 = if let Some(extension) = node
                    .extended_to
                    .as_ref()
                    .filter(|e| e.constructors.is_empty())
                {
                    extension.scope
                } else if let Some(clause) = node.boolean_scope.or(node.clause_scope) {
                    body.scopes()[clause.0 as usize].parent.unwrap_or(node.scope)
                } else {
                    result_scope
                };
            }
            for (ordinal, id) in node.runtime_temporaries.into_iter().enumerate() {
                let Some(id) = id else { continue };
                let scope = if node.role == NodeRole::Statement {
                    node.statement_scope.unwrap_or(node.scope)
                } else if node.role == NodeRole::Expression(ExpressionKind::For) && ordinal == 1 {
                    node.condition_scope.unwrap_or(node.scope)
                } else if let Some(closure) = node.closure_scope {
                    closure
                } else {
                    node.scope
                };
                homes[id.0 as usize].0 = scope;
                assert_eq!(body.slot_kind(SlotId(id.0 + 1)).unwrap(), SlotKind::Place);
                homes[id.0 as usize + 1].0 = scope;
            }
            let scalar_scope = node.closure_scope.unwrap_or(node.scope);
            for id in &node.runtime_scalar_slots {
                homes[id.0 as usize].0 = scalar_scope;
                assert_eq!(body.slot_kind(SlotId(id.0 + 1)).unwrap(), SlotKind::Place);
                homes[id.0 as usize + 1].0 = scalar_scope;
            }
            if let Some(id) = node.constructor_guard {
                homes[id.0 as usize].0 = node.scope;
            }
            if let Some(id) = node.place {
                homes[id.0 as usize].0 = result_scope;
            }
        }
        homes
    }

    /// The removed `Stage::Guards`/`Stage::Slots` cursor: every frame slot
    /// from the last down, filtered by scope.
    fn scanned_slots(frame: &Frame, scope: ScopeId) -> Vec<usize> {
        (0..frame.slots.len())
            .rev()
            .filter(|&index| frame.slots[index].scope == scope)
            .collect()
    }

    /// The removed `Stage::Bindings` cursor.
    fn scanned_nodes(frame: &Frame, scope: ScopeId) -> Vec<usize> {
        (0..frame.bindings.len())
            .rev()
            .filter(|&index| frame.body.nodes()[index].scope == scope)
            .collect()
    }

    fn check(body: &BodyIndex, label: &str) -> (usize, usize) {
        let mut store = GuestStore::for_epoch(1).unwrap();
        let entry = body.pc(NodeId(0), Phase::Entry).unwrap();
        let frame = Frame::new(body.clone(), entry, &mut store).unwrap();
        let homes = reference_slot_homes(body);
        assert_eq!(frame.slots.len(), homes.len(), "{label}");
        for (index, (scope, extension)) in homes.iter().enumerate() {
            assert_eq!(frame.slots[index].scope, *scope, "{label} slot {index} scope");
            assert_eq!(
                frame.slots[index].conditional_extension, *extension,
                "{label} slot {index} conditional extension"
            );
        }
        let mut slot_total = 0;
        let mut node_total = 0;
        for scope in body.scopes() {
            let slots: Vec<usize> = body
                .scope_slots(scope.id)
                .iter()
                .rev()
                .map(|&ordinal| ordinal as usize)
                .collect();
            assert_eq!(slots, scanned_slots(&frame, scope.id), "{label} slots of {:?}", scope.id);
            let nodes: Vec<usize> = body
                .scope_nodes(scope.id)
                .iter()
                .rev()
                .map(|&ordinal| ordinal as usize)
                .collect();
            assert_eq!(nodes, scanned_nodes(&frame, scope.id), "{label} nodes of {:?}", scope.id);
            slot_total += slots.len();
            node_total += nodes.len();
        }
        // Every slot and node belongs to exactly one scope's group.
        assert_eq!(slot_total, frame.slots.len(), "{label}");
        assert_eq!(node_total, frame.bindings.len(), "{label}");
        // The groups of an absent scope are empty, never a panic.
        assert!(body.scope_slots(ScopeId(u32::MAX)).is_empty());
        assert!(body.scope_nodes(ScopeId(u32::MAX)).is_empty());
        (slot_total, node_total)
    }

    fn bodies_of(source: &str, edition: u16, label: &str) -> (usize, usize, usize) {
        let Ok(blob) = SourceBlob::new(source.as_bytes().to_vec()) else {
            return (0, 0, 0);
        };
        let Ok(parsed) = ParsedFile::parse(
            &blob,
            ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
        ) else {
            return (0, 0, 0);
        };
        let Ok(outline) = parsed.outline(Digest::of_bytes(label.as_bytes()), vec![], &CfgSet::guest_default())
        else {
            return (0, 0, 0);
        };
        let (mut bodies, mut slots, mut nodes) = (0, 0, 0);
        for item in &outline.items {
            let Some(handle) = item.body.as_ref() else { continue };
            let Ok(index) = parsed.index_body(handle, Default::default()) else {
                continue;
            };
            let (s, n) = check(&index, label);
            bodies += 1;
            slots += s;
            nodes += n;
        }
        (bodies, slots, nodes)
    }

    #[test]
    fn scope_owner_groups_reproduce_the_removed_whole_frame_scans() {
        // Hand-written bodies covering loops (`for` iteration scopes), clauses
        // (`if let`/`while let`/`match` guards), closures, let-else, labeled
        // blocks, constructor guards and conditional temporary extension.
        const SOURCES: &[&str] = &[
            "fn main() { let mut s = 0u64; for i in 0..4u64 { let t = i + 1; s += t; } while s < 9 { s += 1; } \
             let v = loop { break s; }; let _ = v; }",
            "struct D(u8); impl Drop for D { fn drop(&mut self) {} } \
             fn main() { let _a = D(1); { let _b = D(2); let _ = D(3); } let (x, y) = (D(4), D(5)); \
             let z = if let Some(d) = Some(D(6)) { d.0 } else { 0 }; match Some(D(7)) { Some(d) if d.0 > 1 => {}, _ => {} } \
             let _ = (x, y, z); }",
            "fn f(v: Option<(u8, u8)>) -> u8 { let Some((a, b)) = v else { return 0 }; \
             let c = |p: (u8, u8)| p.0 + a; 'o: loop { loop { break 'o; } } c((a, b)) }",
            "struct P { a: Vec<u8>, b: String } fn main() { let p = P { a: vec![1], b: String::new() }; \
             let r = &P { a: vec![2], b: String::new() }; let q = &mut [1, 2, 3][..]; let _ = (p, r, q); \
             let t = &(String::new(), 1); let _ = t; let l = 'blk: { if true { break 'blk 1; } 2 }; let _ = l; }",
            "const C: [u8; 2] = [1, 2]; static S: u8 = 3; fn main() { let a = [0u8; 3]; let _ = (C, S, a); let v: Vec<u8> = Vec::new(); \
             let w = v.iter().map(|x| *x).count(); let _ = w; assert_eq!(w, 0); }",
        ];
        let (mut bodies, mut slots, mut nodes) = (0, 0, 0);
        for (ordinal, source) in SOURCES.iter().enumerate() {
            for edition in [2015, 2018, 2021, 2024] {
                let (b, s, n) = bodies_of(source, edition, &format!("owner-groups-{ordinal}-{edition}"));
                bodies += b;
                slots += s;
                nodes += n;
            }
        }
        assert!(bodies >= 15, "hand-written bodies indexed: {bodies}");
        // Unchanged official programs: drop, move, closure, loop and match
        // programs exercise the same scope kinds with real shapes.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout/tests/ui");
        let mut programs = 0;
        for directory in [
            "drop", "moves", "closures", "match", "for-loop-while", "pattern", "block-result", "binding",
            "destructuring-assignment", "let-else", "loops", "nll", "expr", "union", "structs-enums",
        ] {
            let Ok(entries) = std::fs::read_dir(root.join(directory)) else { continue };
            let mut files: Vec<_> = entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
                .collect();
            files.sort();
            for path in files {
                let Ok(source) = std::fs::read_to_string(&path) else { continue };
                let edition = source
                    .lines()
                    .find_map(|line| line.strip_prefix("//@ edition:"))
                    .and_then(|edition| edition.trim().parse::<u16>().ok())
                    .unwrap_or(2021);
                let label = path.display().to_string();
                let (b, s, n) = bodies_of(&source, edition, &label);
                if b > 0 {
                    programs += 1;
                }
                bodies += b;
                slots += s;
                nodes += n;
            }
        }
        assert!(programs >= 30, "official programs indexed: {programs}");
        eprintln!("scope owner groups checked: {bodies} bodies in {programs} official programs, {slots} slots, {nodes} nodes");
        assert!(bodies >= 100 && slots > 500 && nodes > 1000, "{bodies} bodies, {slots} slots, {nodes} nodes");
    }
}
