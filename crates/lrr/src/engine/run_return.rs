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
        let action = if let Some(mut suspended) = self.calls.pop() {
            let site = match &suspended.continuation {
                CallContinuation::Site {
                    action,
                    origin,
                    resume,
                    result,
                } => Some((
                    *action,
                    origin.span.body_key == suspended.caller.pc().body,
                    resume.clone(),
                    *result,
                )),
                CallContinuation::DropWitness(_)
                | CallContinuation::Operator
                | CallContinuation::Format { .. }
                | CallContinuation::Initializer { .. } => None,
            };
            // Borrow the last frame context for validation, restore it on every
            // outcome, and keep its metadata charged throughout cleanup.
            let admitted = match site {
                None if matches!(suspended.continuation, CallContinuation::Operator) => {
                    self.admit_operator_return(db, &mut suspended, ticket)
                }
                None if matches!(suspended.continuation, CallContinuation::Format { .. }) => {
                    self.admit_format_return(db, &suspended, ticket)
                }
                None if matches!(
                    suspended.continuation,
                    CallContinuation::Initializer { .. }
                ) =>
                {
                    self.admit_initializer_return(db, &suspended, ticket)
                }
                None => self.admit_drop_witness_return(db, &suspended, ticket),
                Some((site_action, same_body, resume, result)) => (|| {
                if site_action.get() > self.frame.actions.snapshot().consumed || !same_body
                {
                    return Err(call_fault(
                        "return continuation call action identity mismatch",
                    ));
                }
                self.admit_call_result(db, &mut suspended, ticket, result)?;
                suspended.caller.reserve_resume_call(
                    &resume,
                    result,
                    ticket,
                    &mut self.store,
                )?;
                Ok(())
            })(),
            };
            let action = suspended.action();
            self.calls.push(suspended);
            admitted?;
            action
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
            || active.action != self.calls.last().and_then(SuspendedCall::action)
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
            if matches!(suspended.continuation, CallContinuation::DropWitness(_)) {
                return self
                    .resume_drop_witness(suspended, active.ticket)
                    .map(|()| None);
            }
            if matches!(suspended.continuation, CallContinuation::Operator) {
                return self
                    .resume_operator_call(suspended, active.ticket)
                    .map(|()| None);
            }
            if matches!(suspended.continuation, CallContinuation::Format { .. }) {
                return self
                    .resume_format_call(suspended, active.ticket)
                    .map(|()| None);
            }
            if matches!(suspended.continuation, CallContinuation::Initializer { .. }) {
                return self
                    .resume_initializer(suspended, active.ticket)
                    .map(|()| None);
            }
            let (continuation, source_result) = self.restore_caller(suspended, active.ticket)?;
            let CallContinuation::Site { resume, result, .. } = continuation else {
                return Err(call_fault("return continuation lost its call site"));
            };
            self.frame
                .resume_call(resume, result, active.ticket, &mut self.store)?;
            self.frame.set_source_view(result, source_result)?;
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
            output: std::mem::take(&mut self.output),
        }))
    }
}

impl RunMachine {
    /// A `Drop::drop` witness entered by return cleanup unwound: the frame's
    /// return is abandoned for panic cleanup and its result is discarded.
    pub(super) fn abandon_return(&mut self, returning: ReturnContinuation) -> Result<(), Diagnostic> {
        match self.store.cleanup_ticket_inventory(returning.ticket)? {
            crate::runtime::cell::CleanupInventory::Leaf => self.store.discard(returning.ticket),
            _ => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "unwinding from a Drop witness abandons an owning return value".into(),
            )),
        }
    }
}

impl RunMachine {
    /// Admission of a returned value as its suspended caller's result at
    /// slot `result`, shared by every continuation that publishes a source
    /// call's result (a recipe call site, an overloaded operator): the
    /// caller's reached expectation, rigid result parameters, a reference
    /// result's lease or a parameter result's binding, the declared result,
    /// and the caller's view of it.
    pub(super) fn admit_call_result(
        &self,
        db: &mut Database,
        suspended: &mut SuspendedCall,
        ticket: TicketId,
        result: SlotId,
    ) -> Result<(), Diagnostic> {
        self.admit_call_result_at(db, suspended, ticket, Some(result))
    }
    /// `admit_call_result` for a result with no caller slot (a place
    /// operation's `Deref` step): no slot expectation applies.
    pub(super) fn admit_call_result_at(
        &self,
        db: &mut Database,
        suspended: &mut SuspendedCall,
        ticket: TicketId,
        result: Option<SlotId>,
    ) -> Result<(), Diagnostic> {
        let value = self.store.peek_ticket(ticket)?;
        let actual = self.store.effective_type(value)?.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "return literal requires reached concrete type".into(),
            )
        })?;
        // Some generic results become concrete only on return. The
        // caller's physical expectation remains a constraint even
        // when it could not supply a symbolic source hint at entry.
        if let Some(result) = result
            && suspended
                .caller
                .strict_hint_of(result)?
                .is_some_and(|expected| !self.store.contract_admits(Some(expected), Some(actual)))
        {
            return Err(Diagnostic::new(
                "LRR-E-CALL-RETURN-TYPE",
                "returned descriptor differs from the reached caller result context".into(),
            ));
        }
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
            let returned = reached_argument_type(actual, source.cloned())?;
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
        // An opaque/trait-object result is viewed as the returned
        // value's own concrete type.
        let concrete = source_result
            .filter(|term| crate::frontend::call_contracts::contains_object(term))
            .map(|_| self.store.descriptor_source_term(actual));
        // A closure declares no result Type: its call result is viewed as the
        // returned value's reached closed Type.
        let reached = if source_result.is_none() && suspended.call.closure.is_some() {
            self.store
                .descriptor_source_term(actual)
                .filter(crate::vm::frame::Frame::admissible_parameter_source)
        } else {
            None
        };
        let source_result = match (&concrete, &reached) {
            (Some(view), _) => view.as_ref(),
            (None, Some(view)) => Some(view),
            (None, None) => source_result,
        };
        if let Some(result) = result {
            suspended
                .caller
                .preflight_source_view(result, source_result)?;
        }
        Ok(())
    }
    /// Retire the returned callee frame and restore its suspended caller,
    /// moving the call's source result out of its retiring context (a
    /// parameter result as its binding; an opaque/trait-object result as
    /// the returned value's own type). On refusal the caller stays suspended.
    pub(super) fn restore_caller(
        &mut self,
        mut suspended: SuspendedCall,
        ticket: TicketId,
    ) -> Result<(CallContinuation, Option<crate::traits::term::TyTerm>), Diagnostic> {
        // Cleanup may retire locations. Refuse before publishing the caller
        // unless the actual target still matches the admitted input lease.
        if let Some(proof) = suspended
            .call
            .contracts
            .closed_references
            .as_ref()
            .filter(|proof| proof.has_result())
        {
            if let Err(error) = proof.recheck_result_ticket(&self.store, ticket) {
                self.calls.push(suspended);
                return Err(error);
            }
        }
        // A returned raw pointer takes the call's closed declared pointer
        // type (E-R5-008): the intrinsics behind `null`, `cast` and `as_ptr`
        // return pointers with an erased pointee.
        if let Some(term) = suspended.call.contracts.source_result.as_ref()
            && crate::traits::pointer_terms::as_raw_pointer(term).is_some()
            && crate::frontend::call_contracts::closed_term(term)
        {
            let value = self.store.peek_ticket(ticket)?;
            if value.kind == 9
                && let Some(expected) = self.store.intern_structural_term(term)?
                && expected.0 != value.type_id
                && self.store.pointer_coercion(value, expected)?
            {
                self.store.coerce_unsized_ticket(ticket, expected)?;
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
        // An opaque/trait-object result is viewed as the returned value's
        // own concrete type (read before the ticket is consumed).
        let source_result = match source_result {
            Some(term) if crate::frontend::call_contracts::contains_object(&term) => {
                let value = match self.store.peek_ticket(ticket) {
                    Ok(value) => value,
                    Err(error) => {
                        self.calls.push(suspended);
                        return Err(error);
                    }
                };
                self.store
                    .descriptor_source_term(crate::runtime::shape::TypeId(value.type_id))
            }
            None if suspended.call.closure.is_some() => {
                let value = match self.store.peek_ticket(ticket) {
                    Ok(value) => value,
                    Err(error) => {
                        self.calls.push(suspended);
                        return Err(error);
                    }
                };
                self.store
                    .descriptor_source_term(crate::runtime::shape::TypeId(value.type_id))
                    .filter(crate::vm::frame::Frame::admissible_parameter_source)
            }
            other => other,
        };
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
        Ok((suspended.continuation, source_result))
    }
}

#[cfg(test)]
#[path = "run_reference_result_tests.rs"]
mod reference_result_tests;
