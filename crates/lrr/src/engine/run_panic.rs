//! Ordinary guest panic retains its origin and frame stack during bounded cleanup.
use super::*;

pub(super) struct PanicContinuation {
    panic: crate::vm::step::GuestPanic,
    pc: crate::core::digest::Digest,
    depth: usize,
    action: Option<crate::vm::action::ActionId>,
}
fn panic_fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-IR", message.into())
}
impl RunMachine {
    pub(super) fn begin_panic_cleanup(
        &mut self,
        panic: crate::vm::step::GuestPanic,
    ) -> Result<(), Diagnostic> {
        // A `Drop::drop` witness entered by unwinding cleanup suspends that
        // unwinding with its caller; a panic below it is still a second panic.
        if self.unwinding.is_some()
            || self.calls.iter().any(|call| {
                matches!(&call.continuation, CallContinuation::DropWitness(resume)
                    if resume.unwinding.is_some())
            })
        {
            return Err(Diagnostic::new(
                "LRR-E-DOUBLE-PANIC",
                "a second guest panic occurred during unwinding".into(),
            ));
        }
        if self.returning.is_some()
            || self.frame.status != crate::vm::frame::FrameStatus::Panicked
            || panic.pc != *self.frame.pc()
        {
            return Err(panic_fault("panic continuation requires a panicked frame"));
        }
        self.unwinding = Some(PanicContinuation {
            panic,
            pc: self.frame.pc().fingerprint(),
            depth: self.calls.len(),
            action: self.calls.last().and_then(SuspendedCall::action),
        });
        Ok(())
    }
    /// A cleanup chunk never prepares another thunk or resumes a caller recipe.
    pub(super) fn advance_panic_cleanup(&mut self) -> Result<Option<PanicReport>, Diagnostic> {
        let active = self
            .unwinding
            .as_ref()
            .ok_or_else(|| panic_fault("panic continuation absent"))?;
        if active.pc != self.frame.pc().fingerprint()
            || active.depth != self.calls.len()
            || active.action != self.calls.last().and_then(SuspendedCall::action)
        {
            return Err(panic_fault("panic continuation changed during cleanup"));
        }
        if !self.frame.advance_panic_cleanup(&mut self.store)? {
            return Ok(None);
        }
        if let Some(suspended) = self.calls.last() {
            let bytes = self
                .call_bytes
                .checked_sub(suspended.call.retained_bytes)
                .ok_or_else(|| panic_fault("panic call metadata retirement underflow"))?;
            self.collected.accumulate(self.frame.counters())?;
            self.retire_pending_frame()?;
            let suspended = self
                .calls
                .pop()
                .ok_or_else(|| panic_fault("panic caller disappeared"))?;
            self.call_bytes = bytes;
            self.frame = suspended.caller;
            self.body_lease = suspended.body_lease;
            self.pending = suspended.pending;
            match suspended.continuation {
                // `Drop::drop` unwound: the dropped value's fields and the
                // dropping frame's remaining owners continue as panic cleanup.
                CallContinuation::DropWitness(resume) => {
                    self.frame.unwind_drop_witness(&mut self.store)?;
                    if let Some(returning) = resume.returning {
                        self.abandon_return(returning)?;
                    }
                }
                CallContinuation::Site { .. } => {
                    self.frame.status = crate::vm::frame::FrameStatus::Panicked;
                }
                // The operator's own operation becomes the panicking one.
                CallContinuation::Operator => {
                    self.frame.unwind_operator_call(&mut self.store)?;
                }
                // The formatting operation becomes the panicking one.
                CallContinuation::Format { sink } => {
                    self.unwind_format_call(sink)?;
                }
                // The demanding `ValuePath` becomes the panicking operation.
                CallContinuation::Initializer { .. } => {
                    self.frame.status = crate::vm::frame::FrameStatus::Panicked;
                }
            }
            let active = self
                .unwinding
                .as_mut()
                .ok_or_else(|| panic_fault("panic continuation disappeared"))?;
            active.pc = self.frame.pc().fingerprint();
            active.depth = self.calls.len();
            active.action = self.calls.last().and_then(SuspendedCall::action);
            return Ok(None);
        }
        let counters = self.all_counters()?;
        // Root selected calls can remain pending when an argument panics. Keep
        // their metadata until every guest owner is cleaned, then retire them
        // before publishing the terminal panic.
        self.retire_pending_frame()?;
        if self.pending_bytes != 0 || self.call_bytes != 0 {
            return Err(panic_fault("completed panic retains call metadata"));
        }
        let active = self
            .unwinding
            .take()
            .ok_or_else(|| panic_fault("completed panic continuation absent"))?;
        Ok(Some(PanicReport {
            live_edition: self.frame.body.profile().edition,
            semantics_profile: crate::core::profile::SEMANTICS_PROFILE,
            schema: "lrr/v2/development-run",
            status: "guest_panic",
            panic: active.panic,
            opcode_counters: counters,
            output: std::mem::take(&mut self.output),
        }))
    }
}
