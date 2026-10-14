//! Run-owned fuel and interruption state; no source queries or guest evaluation.
use crate::core::diagnostic::Diagnostic;
use serde::Serialize;
use std::{
    cell::Cell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};
pub const DEFAULT_FUEL: u64 = 10_000_000;
pub const MAX_FUEL: u64 = 1_000_000_000;
#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<RunSignal>);
#[derive(Debug, Default)]
struct RunSignal {
    cancelled: AtomicBool,
    charged: AtomicU64,
}
impl CancellationToken {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
    }
    pub fn fuel_charged(&self) -> u64 {
        self.0.charged.load(Ordering::Acquire)
    }
    fn requested(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FuelStop {
    Exhausted,
    Cancelled,
    Deadline,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub struct FuelSnapshot {
    pub limit: u64,
    pub used: u64,
    pub remaining: u64,
    pub polls: u64,
    pub stop: Option<FuelStop>,
}
pub struct RunFuel {
    state: Rc<FuelState>,
}
struct FuelState {
    snapshot: Cell<FuelSnapshot>,
    cancellation: CancellationToken,
    deadline: Cell<Option<Instant>>,
}
impl RunFuel {
    pub fn new(limit: u64) -> Result<Self, Diagnostic> {
        if limit > MAX_FUEL {
            return Err(Self::fault(FuelStop::Exhausted));
        }
        Ok(Self {
            state: Rc::new(FuelState {
                snapshot: Cell::new(FuelSnapshot {
                    limit,
                    used: 0,
                    remaining: limit,
                    polls: 0,
                    stop: None,
                }),
                cancellation: CancellationToken::default(),
                deadline: Cell::new(None),
            }),
        })
    }
    pub fn snapshot(&self) -> FuelSnapshot {
        self.state.snapshot.get()
    }
    pub(crate) fn share(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
    pub(crate) fn is_shared(&self) -> bool {
        Rc::strong_count(&self.state) != 1
    }
    pub fn cancellation(&self) -> CancellationToken {
        self.state.cancellation.clone()
    }
    pub fn set_deadline(&mut self, deadline: Instant) {
        self.state.deadline.set(Some(deadline));
    }
    fn fault(stop: FuelStop) -> Diagnostic {
        Diagnostic::new(
            "LRR-E-RUNTIME-RESOURCE",
            match stop {
                FuelStop::Exhausted => "guest opcode fuel exhausted",
                FuelStop::Cancelled => "guest run cancellation requested",
                FuelStop::Deadline => "guest run deadline expired",
            }
            .into(),
        )
    }
    fn stop(&mut self, stop: FuelStop) -> Diagnostic {
        let mut snapshot = self.snapshot();
        snapshot.stop = Some(stop);
        self.state.snapshot.set(snapshot);
        Self::fault(stop)
    }
    /// Dispatch/cleanup boundaries poll without charging an opcode.
    pub fn poll(&mut self) -> Result<(), Diagnostic> {
        let mut snapshot = self.snapshot();
        if let Some(stop) = snapshot.stop {
            return Err(Self::fault(stop));
        }
        snapshot.polls = match snapshot.polls.checked_add(1) {
            Some(count) => count,
            None => return Err(self.stop(FuelStop::Exhausted)),
        };
        self.state.snapshot.set(snapshot);
        if self.state.cancellation.requested() {
            return Err(self.stop(FuelStop::Cancelled));
        }
        if self
            .state
            .deadline
            .get()
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(self.stop(FuelStop::Deadline));
        }
        Ok(())
    }
    pub fn require_credit(&mut self) -> Result<(), Diagnostic> {
        self.poll()?;
        if self.snapshot().remaining == 0 {
            return Err(self.stop(FuelStop::Exhausted));
        }
        Ok(())
    }
    /// One shared charge operation for interpreter and future native helpers.
    pub fn charge(&mut self) -> Result<(), Diagnostic> {
        self.require_credit()?;
        let mut snapshot = self.snapshot();
        let used = snapshot
            .used
            .checked_add(1)
            .ok_or_else(|| self.stop(FuelStop::Exhausted))?;
        snapshot.remaining -= 1;
        snapshot.used = used;
        self.state.snapshot.set(snapshot);
        self.state
            .cancellation
            .0
            .charged
            .store(used, Ordering::Release);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_run_fuel_preserves_cancellation_deadline_and_terminal_stop() {
        for deadline in [false, true] {
            let mut caller = RunFuel::new(10).unwrap();
            let mut callee = caller.share();
            caller.charge().unwrap();
            callee.charge().unwrap();
            let token = caller.cancellation();
            assert_eq!(token.fuel_charged(), 2);
            if deadline {
                callee.set_deadline(Instant::now());
            } else {
                std::thread::spawn(move || token.cancel()).join().unwrap();
            }
            assert!(caller.charge().is_err());
            assert!(callee.charge().is_err());
            assert_eq!(caller.snapshot(), callee.snapshot());
            assert_eq!(callee.snapshot().used, 2);
            assert_eq!(
                callee.snapshot().stop,
                Some(if deadline {
                    FuelStop::Deadline
                } else {
                    FuelStop::Cancelled
                })
            );
        }
    }
    #[test]
    fn fuel_charges_exact_units_and_terminal_failure_cannot_restart() {
        let mut fuel = RunFuel::new(2).unwrap();
        fuel.poll().unwrap();
        assert_eq!(fuel.snapshot().used, 0);
        fuel.charge().unwrap();
        fuel.charge().unwrap();
        assert!(fuel.charge().is_err());
        assert_eq!(fuel.snapshot().used, 2);
        assert_eq!(fuel.snapshot().remaining, 0);
        assert_eq!(fuel.snapshot().stop, Some(FuelStop::Exhausted));
        assert!(fuel.poll().is_err());
        assert!(RunFuel::new(MAX_FUEL + 1).is_err());
    }
    #[test]
    fn cancellation_from_another_thread_and_expired_deadline_stop_before_charge() {
        let mut fuel = RunFuel::new(10).unwrap();
        let token = fuel.cancellation();
        std::thread::spawn(move || token.cancel()).join().unwrap();
        assert!(fuel.charge().is_err());
        assert_eq!(fuel.snapshot().stop, Some(FuelStop::Cancelled));
        assert_eq!(fuel.snapshot().used, 0);
        let mut fuel = RunFuel::new(10).unwrap();
        fuel.set_deadline(Instant::now());
        assert!(fuel.charge().is_err());
        assert_eq!(fuel.snapshot().stop, Some(FuelStop::Deadline));
        assert_eq!(fuel.snapshot().used, 0);
    }
}
