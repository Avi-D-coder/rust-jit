//! Dispatcher-owned return continuation. Never publish a caller before cleanup.
use super::*;
use crate::runtime::cell::TicketId;

pub(super) struct ReturnContinuation {
    pc: crate::core::digest::Digest,
    ticket: TicketId,
    depth: usize,
    action: Option<crate::vm::action::ActionId>,
}
impl RunMachine {
    pub(super) fn begin_return_cleanup(
        &mut self,
        db: &mut Database,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        if self.returning.is_some() || self.frame.status != crate::vm::frame::FrameStatus::Returned
        {
            return Err(call_fault(
                "return continuation already active or frame has not returned",
            ));
        }
        let retained = self
            .call_bytes
            .checked_add(std::mem::size_of::<ReturnContinuation>())
            .ok_or_else(crate::runtime::arena::resource)?;
        if retained > self.call_limits.max_metadata_bytes {
            return Err(crate::runtime::arena::resource());
        }
        let action = if let Some(mut suspended) = self.calls.pop() {
            // Borrow the last frame context for validation, restore it on every
            // outcome, and keep its metadata charged throughout cleanup.
            let admitted = (|| {
                if suspended.action.get() > self.frame.actions.snapshot().consumed
                    || suspended.origin.span.body_key != suspended.caller.pc().body
                {
                    return Err(call_fault(
                        "return continuation call action identity mismatch",
                    ));
                }

                let value = self.store.peek_ticket(ticket)?;
                let actual = self.store.effective_type(value)?.ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-TYPE-CONTEXT",
                        "return literal requires reached concrete type".into(),
                    )
                })?;
                if let (
                    Some(crate::traits::term::TyTerm::Param(expected)),
                    Some(crate::traits::term::TyTerm::Param(found)),
                ) = (
                    &suspended.call.contracts.source_result,
                    &self.frame.return_source,
                ) && expected != found
                {
                    return Err(Diagnostic::new(
                        "LRR-E-GENERIC-BINDING",
                        "returned rigid source parameter differs from declared result".into(),
                    ));
                }
                let reference_result = suspended
                    .call
                    .contracts
                    .closed_references
                    .as_ref()
                    .is_some_and(|proof| proof.has_result());
                if reference_result {
                    suspended.call.contracts.admit_reference_return(
                        db,
                        &suspended.call.callee,
                        &self.store,
                        ticket,
                        self.frame.return_source.as_ref(),
                    )?;
                } else if suspended.call.contracts.result_descriptor.is_some()
                    || matches!(
                        suspended.call.contracts.source_result,
                        Some(crate::traits::term::TyTerm::Param(_))
                    )
                {
                    let mut returned = primitive_argument_type(actual)?;
                    let source = match self.frame.return_source.as_ref() {
                        Some(crate::traits::term::TyTerm::Param(key))
                            if suspended.call.contracts.source_result.as_ref()
                                == self.frame.return_source.as_ref() =>
                        {
                            // Body views keep the callee's rigid parameter.
                            // Only this admitted boundary restores its caller
                            // binding; an unbound result is learned on return.
                            suspended.call.contracts.source_substitution.get(key)
                        }
                        source => source,
                    };
                    if let Some(source) = source {
                        returned.source = source.clone();
                    }
                    suspended.call.contracts.bind_return_parameter(&returned)?;
                }
                self.check_source_return(db, &suspended.call, actual)?;
                let source_result = suspended.call.contracts.source_result.as_ref().map(|term| {
                    if let crate::traits::term::TyTerm::Param(key) = term {
                        suspended
                            .call
                            .contracts
                            .source_substitution
                            .get(key)
                            .unwrap_or(term)
                    } else {
                        term
                    }
                });
                suspended
                    .caller
                    .preflight_source_view(suspended.result, source_result)?;
                suspended.caller.reserve_resume_call(
                    &suspended.resume,
                    suspended.result,
                    ticket,
                    &mut self.store,
                )?;
                Ok(())
            })();
            let action = suspended.action;
            self.calls.push(suspended);
            admitted?;
            Some(action)
        } else {
            let value = self.store.peek_ticket(ticket)?;
            if value.kind == 11 || value.kind == 12 {
                return Err(Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "returned numeric literal requires an explicit reached type".into(),
                ));
            }
            // Preserve the current supported root-summary admission before any
            // cleanup; detachment/publication happens only after cleanup.
            summary(value)?;
            None
        };
        self.returning = Some(ReturnContinuation {
            pc: self.frame.pc().fingerprint(),
            ticket,
            depth: self.calls.len(),
            action,
        });
        Ok(())
    }
    /// None means a bounded cleanup chunk or a completed caller resumption.
    /// The current frame status distinguishes them; the loop never reruns Return.
    pub(super) fn advance_return_cleanup(&mut self) -> Result<Option<RunReport>, Diagnostic> {
        let active = self
            .returning
            .as_ref()
            .ok_or_else(|| call_fault("return continuation absent"))?;
        if active.pc != self.frame.pc().fingerprint()
            || active.depth != self.calls.len()
            || active.action != self.calls.last().map(|call| call.action)
        {
            return Err(call_fault("return continuation changed during cleanup"));
        }
        self.store.peek_ticket(active.ticket)?;
        if let Some(proof) = self
            .calls
            .last()
            .and_then(|call| call.call.contracts.closed_references.as_ref())
            .filter(|proof| proof.has_result())
        {
            proof.recheck_result_ticket(&self.store, active.ticket)?;
        }
        if !self.frame.advance_return_cleanup(&mut self.store)? {
            return Ok(None);
        }
        let active = self
            .returning
            .take()
            .ok_or_else(|| call_fault("completed return continuation absent"))?;
        if let Some(mut suspended) = self.calls.pop() {
            // Cleanup may retire locations. Refuse before publishing the caller
            // unless the actual target still matches the admitted input lease.
            if let Some(proof) = suspended
                .call
                .contracts
                .closed_references
                .as_ref()
                .filter(|proof| proof.has_result())
            {
                if let Err(error) = proof.recheck_result_ticket(&self.store, active.ticket) {
                    self.calls.push(suspended);
                    return Err(error);
                }
            }
            let bytes = match self.call_bytes.checked_sub(suspended.call.retained_bytes) {
                Some(bytes) => bytes,
                None => {
                    self.calls.push(suspended);
                    return Err(call_fault("call metadata retirement underflow"));
                }
            };
            // Move the already charged source result out of its retiring call
            // context. Cleanup completion must not clone a fresh type tree.
            let source_result = suspended.call.contracts.source_result.take().map(|term| {
                if let crate::traits::term::TyTerm::Param(key) = &term {
                    suspended
                        .call
                        .contracts
                        .source_substitution
                        .remove(key)
                        .unwrap_or(term)
                } else {
                    term
                }
            });
            if let Err(error) = self
                .collected
                .accumulate(self.frame.counters())
                .and_then(|()| self.retire_pending_frame())
            {
                self.calls.push(suspended);
                return Err(error);
            }
            // The caller remained suspended and owned by the charged stack
            // entry until cleanup actually finished. Transfer exactly once.
            self.call_bytes = bytes;
            self.frame = suspended.caller;
            self.body_lease = suspended.body_lease;
            self.pending = suspended.pending;
            self.frame.resume_call(
                suspended.resume,
                suspended.result,
                active.ticket,
                &mut self.store,
            )?;
            self.frame
                .set_source_view(suspended.result, source_result)?;
            return Ok(None);
        }
        let value = summary(self.store.peek_ticket(active.ticket)?)?;
        self.store.discard(active.ticket)?;
        Ok(Some(RunReport {
            live_edition: self.frame.body.profile().edition,
            semantics_profile: crate::core::profile::SEMANTICS_PROFILE,
            schema: "lrr/v2/development-run",
            status: "completed",
            value,
            opcode_counters: self.all_counters()?,
            installed_recipes: self.installed.len(),
            installed_lrir_bytes: self.installed.metadata_bytes(),
        }))
    }
}

#[cfg(test)]
#[path = "run_reference_result_tests.rs"]
mod reference_result_tests;
