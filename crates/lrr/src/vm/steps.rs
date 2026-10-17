//! Run-wide executed-step observation shared by a caller and its callees.
//!
//! E-R5-006: this is an unlimited counter only. There is no fuel budget,
//! deadline or cancellation; rustc imposes no limit on executed guest steps.
use serde::Serialize;
use std::{cell::Cell, rc::Rc};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub struct StepSnapshot {
    pub used: u64,
}
#[derive(Debug, Default)]
pub struct RunSteps {
    used: Rc<Cell<u64>>,
}
impl RunSteps {
    pub fn snapshot(&self) -> StepSnapshot {
        StepSnapshot {
            used: self.used.get(),
        }
    }
    /// A callee observes and extends its caller's run-wide count.
    pub(crate) fn share(&self) -> Self {
        Self {
            used: Rc::clone(&self.used),
        }
    }
    /// Record one executed guest step. Saturation is unreachable in practice
    /// and never refuses execution.
    pub(crate) fn charge(&mut self) {
        self.used.set(self.used.get().saturating_add(1));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_steps_count_caller_and_callee_without_a_limit() {
        let mut caller = RunSteps::default();
        let mut callee = caller.share();
        caller.charge();
        callee.charge();
        assert_eq!(caller.snapshot(), callee.snapshot());
        assert_eq!(callee.snapshot().used, 2);
    }
}
