//! Common transaction execution for every ordinary pattern owner. Source
//! planning/verifier select identities; this module never parses guest syntax.
use super::*;
use crate::{ir::op::PatternOperation, vm::pattern::PendingBinding};

impl Frame {
    pub(super) fn pattern_bool(
        &self,
        slot: SlotId,
        store: &GuestStore,
    ) -> Result<bool, Diagnostic> {
        let value = store.read_scalar(self.cell(slot)?)?;
        if value.kind != 2 || value.type_id != 2 {
            return Err(invalid("pattern result is not a concrete Bool"));
        }
        Ok(value.payload == 1)
    }
    fn preflight_pattern_bindings(
        &self,
        owner: NodeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let pending = self.patterns.pending(owner)?;
        for binding in pending {
            self.preflight_bind(binding.binding, binding.source, None, store)?;
            if store.owner(self.cell(binding.source)?)?
                != (CellOwner::FrameSlot {
                    frame: self.anchor,
                    slot: binding.source.0,
                })
            {
                return Err(invalid(
                    "pending pattern source is not its unique frame-slot owner",
                ));
            }
        }
        store.preflight_pattern_bindings(pending.len())
    }
    pub(crate) fn preflight_pattern(
        &self,
        operation: &PatternOperation,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.require_store(store)?;
        match operation {
            PatternOperation::Decision {
                owner,
                plan,
                transition,
            } => self.preflight_pattern_decision(*owner, plan, transition, store),
            PatternOperation::Begin {
                owner,
                alternative,
                source,
                retry,
            } => {
                self.patterns.preflight_begin(*owner)?;
                if retry.body != self.pc.body
                    || !self.active_scopes.contains(&self.slot(*source)?.scope)
                    || self.body.nodes().get(alternative.0 as usize).is_none()
                {
                    return Err(invalid(
                        "pattern transaction source/continuation is not current in this frame",
                    ));
                }
                self.body.pc(
                    NodeId(retry.node),
                    crate::ir::pc::Phase::from_code(retry.phase),
                )?;
                store.admit_move(self.cell(*source)?)?;
                Ok(())
            }
            PatternOperation::StageBinding {
                owner,
                binding,
                source,
                writable,
            } => {
                let active = self.patterns.current(*owner)?;
                if active.decision.is_some() {
                    return Err(Diagnostic::new("LRR-E-UNSUPPORTED", "structural binding publication requires cached occurrence namespace admission".into()));
                }
                if active.alternative != *binding {
                    return Err(invalid(
                        "pending binding is not the selected admitted pattern header",
                    ));
                }
                self.patterns.preflight_stage(
                    *owner,
                    PendingBinding {
                        binding: *binding,
                        source: *source,
                        writable: *writable,
                    },
                )?;
                self.preflight_bind(*binding, *source, None, store)
            }
            PatternOperation::Preview { owner } => {
                let active = self.patterns.current(*owner)?;
                if let Some(decision) = &active.decision
                    && !decision.state.matched()
                {
                    return Err(invalid(
                        "guard preview requires a completed source decision",
                    ));
                }
                if active.decision.is_some() {
                    self.require_decision_source(*owner, store)?;
                }
                if active.previewed {
                    return Err(invalid("pattern guard preview was already committed"));
                }
                if !self.patterns.pending(*owner)?.is_empty() {
                    // The selected match source service already proves the
                    // scalar contract. GuestStore checks actual intrinsic Copy
                    // authority; no owning value is copied to test a pattern.
                    if active.decision.is_none() {
                        store.admit_copy(self.cell(active.source)?)?;
                    } else {
                        store.admit_move(self.cell(active.source)?)?;
                    }
                }
                self.preflight_pattern_bindings(*owner, store)?;
                for pending in self.patterns.pending(*owner)? {
                    store.admit_copy(self.cell(pending.source)?)?;
                }
                Ok(())
            }
            PatternOperation::Finish { owner, matched } => {
                let active = self.patterns.current(*owner)?;
                if active.decision.is_some() {
                    self.require_decision_source(*owner, store)?;
                }
                let matched = matched
                    .map(|slot| self.pattern_bool(slot, store))
                    .transpose()?
                    .unwrap_or(true);
                if let Some(decision) = &active.decision
                    && ((matched && !decision.state.matched())
                        || (!matched && !decision.state.unmatched()))
                {
                    return Err(invalid(
                        "pattern finish differs from source decision outcome",
                    ));
                }
                if matched && !active.previewed {
                    self.preflight_pattern_bindings(*owner, store)?;
                }
                Ok(())
            }
            PatternOperation::Rollback { owner } => {
                let active = self.patterns.current(*owner)?;
                if let Some(decision) = &active.decision {
                    self.require_decision_source(*owner, store)?;
                    if !decision.state.unmatched() || !self.patterns.pending(*owner)?.is_empty() {
                        return Err(invalid(
                            "decision rollback requires completed attempt cleanup",
                        ));
                    }
                    return Ok(());
                }
                if active.previewed
                    || (self.pc.node != owner.0 && self.pc.node != active.alternative.0)
                {
                    return Err(invalid(
                        "pattern rollback is outside its uncommitted owner/alternative",
                    ));
                }
                Ok(())
            }
        }
    }
    fn commit_pattern_bindings(
        &mut self,
        owner: NodeId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_pattern_bindings(owner, store)?;
        let count = self.patterns.pending(owner)?.len();
        store.reserve_pattern_bindings(count)?;
        for index in 0..count {
            let binding = self.patterns.pending(owner)?[index];
            self.bind(
                binding.binding,
                binding.source,
                None,
                binding.writable,
                store,
            )?;
        }
        Ok(())
    }
    fn preview_pattern_bindings(
        &mut self,
        owner: NodeId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let count = self.patterns.pending(owner)?.len();
        store.reserve_pattern_bindings(count)?;
        for index in 0..count {
            let pending = self.patterns.pending(owner)?[index];
            let source = self.cell(pending.source)?;
            let value = store.admit_copy(source)?;
            let source_view = self.source_view(pending.source)?;
            let slot = self
                .body
                .slot_count()
                .checked_add(pending.binding.0)
                .ok_or_else(|| invalid("provisional pattern binding slot overflow"))?;
            let destination = store.new_cell(
                StorageContract::Unbound,
                pending.writable,
                CellOwner::FrameSlot {
                    frame: self.anchor,
                    slot,
                },
            )?;
            store.validate_initialization_value(destination, value)?;
            let ticket = store.copy_from(source)?;
            store.initialize(destination, ticket)?;
            self.bindings[pending.binding.0 as usize] = Some(destination);
            self.set_binding_source_view(pending.binding, source_view)?;
        }
        Ok(())
    }
    pub(crate) fn execute_pattern(
        &mut self,
        operation: &PatternOperation,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_pattern(operation, store)?;
        match operation {
            PatternOperation::Decision {
                owner,
                plan,
                transition,
            } => self.execute_pattern_decision(*owner, plan, transition, store),
            PatternOperation::Begin {
                owner,
                alternative,
                source,
                retry,
            } => {
                let scope = self.slot(*source)?.scope;
                self.patterns
                    .begin(*owner, *alternative, *source, retry.clone(), scope)
            }
            PatternOperation::StageBinding {
                owner,
                binding,
                source,
                writable,
            } => self.patterns.stage(
                *owner,
                PendingBinding {
                    binding: *binding,
                    source: *source,
                    writable: *writable,
                },
            ),
            PatternOperation::Preview { owner } => {
                self.preview_pattern_bindings(*owner, store)?;
                self.patterns.mark_previewed(*owner)
            }
            PatternOperation::Finish { owner, matched } => {
                let matched = matched
                    .map(|slot| self.pattern_bool(slot, store))
                    .transpose()?
                    .unwrap_or(true);
                if matched && !self.patterns.current(*owner)?.previewed {
                    self.commit_pattern_bindings(*owner, store)?;
                }
                self.patterns.finish(*owner)
            }
            PatternOperation::Rollback { owner } => self.patterns.finish(*owner),
        }
    }
    pub(crate) fn preflight_pattern_retry(
        &self,
        owner: NodeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let active = self.patterns.current(owner)?;
        let arm = self
            .body
            .nodes()
            .get(self.pc.node as usize)
            .ok_or_else(|| invalid("pattern guard continuation node absent"))?;
        let alternative = &self.body.nodes()[active.alternative.0 as usize];
        if !active.previewed
            || arm.guard_scope.is_none()
            || active.retry.body != self.pc.body
            || !arm.range.contains(alternative.range)
        {
            return Err(invalid(
                "pattern guard retry belongs to another source/body/arm",
            ));
        }
        self.body.pc(
            NodeId(active.retry.node),
            crate::ir::pc::Phase::from_code(active.retry.phase),
        )?;
        store.admit_move(self.cell(active.source)?)?;
        Ok(())
    }
    pub(crate) fn pattern_retry(
        &mut self,
        owner: NodeId,
        store: &GuestStore,
    ) -> Result<PcKey, Diagnostic> {
        self.preflight_pattern_retry(owner, store)?;
        let retry = self.patterns.current(owner)?.retry.clone();
        self.patterns.finish(owner)?;
        Ok(retry)
    }
}
