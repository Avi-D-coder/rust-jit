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
        if self.unwinding.is_some() {
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
        let heap = panic
            .origin
            .expansion_ancestry
            .capacity()
            .checked_mul(std::mem::size_of::<crate::core::digest::Digest>())
            .and_then(|n| n.checked_add(panic.message.as_ref().map_or(0, String::capacity)))
            .ok_or_else(crate::runtime::arena::resource)?;
        let retained = self
            .call_bytes
            .checked_add(self.pending_bytes)
            .and_then(|n| n.checked_add(std::mem::size_of::<PanicContinuation>()))
            .and_then(|n| n.checked_add(heap))
            .ok_or_else(crate::runtime::arena::resource)?;
        if retained > self.call_limits.max_metadata_bytes {
            return Err(crate::runtime::arena::resource());
        }
        self.unwinding = Some(PanicContinuation {
            panic,
            pc: self.frame.pc().fingerprint(),
            depth: self.calls.len(),
            action: self.calls.last().map(|call| call.action),
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
            || active.action != self.calls.last().map(|call| call.action)
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
            self.frame.status = crate::vm::frame::FrameStatus::Panicked;
            let active = self
                .unwinding
                .as_mut()
                .ok_or_else(|| panic_fault("panic continuation disappeared"))?;
            active.pc = self.frame.pc().fingerprint();
            active.depth = self.calls.len();
            active.action = self.calls.last().map(|call| call.action);
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
        }))
    }
}
