//! Run-owned expansion accounting shared by coordinator discovery domains.
use super::record::{ExpansionRecord, ExpansionUsage};
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncode},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct SharedExpansionBudget(Arc<Budget>);
struct Budget {
    request: Digest,
    graph: Digest,
    epoch: u64,
    state: Mutex<State>,
}
struct State {
    epoch: u64,
    frozen: bool,
    tokens: usize,
    wire_bytes: usize,
    outputs: BTreeMap<Digest, Digest>,
    invocation_timeout: Duration,
    cumulative_timeout: Duration,
    deadline: Option<Instant>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpansionTotals {
    pub tokens: usize,
    pub wire_bytes: usize,
    pub outputs: usize,
}
struct Charge {
    tokens: usize,
    wire_bytes: usize,
    outputs: Vec<(Digest, Digest)>,
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
impl SharedExpansionBudget {
    pub(crate) fn shares_ledger(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn new(
        request: Digest,
        graph: Digest,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        if epoch == 0 {
            return Err(fault(
                "LRR-E-DISCOVERY-LIMIT",
                "expansion budget epoch is invalid",
            ));
        }
        Ok(Self(Arc::new(Budget {
            request,
            graph,
            epoch,
            state: Mutex::new(State {
                epoch,
                frozen: false,
                tokens: 0,
                wire_bytes: 0,
                outputs: BTreeMap::new(),
                invocation_timeout: lrr_expand_adapter::NO_TIME_LIMIT,
                cumulative_timeout: lrr_expand_adapter::NO_TIME_LIMIT,
                deadline: None,
            }),
        })))
    }
    pub(crate) fn request_identity(&self) -> Digest {
        self.0.request
    }
    pub(crate) fn matches(&self, request: Digest, graph: Digest, epoch: u64) -> bool {
        self.0.request == request && self.0.graph == graph && self.0.epoch == epoch
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>, Diagnostic> {
        self.0
            .state
            .lock()
            .map_err(|_| fault("LRR-E-INTERNAL-SNAPSHOT", "expansion budget was poisoned"))
    }
    pub(crate) fn freeze(&self) -> Result<(), Diagnostic> {
        self.lock()?.start()
    }
    pub fn with_deadlines(
        self,
        invocation: Duration,
        cumulative: Duration,
    ) -> Result<Self, Diagnostic> {
        {
            let mut state = self.lock()?;
            if state.frozen || invocation.is_zero() || cumulative.is_zero() {
                return Err(fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "expansion deadlines are invalid or configured late",
                ));
            }
            state.invocation_timeout = invocation;
            state.cumulative_timeout = cumulative;
        }
        Ok(self)
    }
    pub(crate) fn check_deadline(&self) -> Result<(), Diagnostic> {
        self.lock()?.check_deadline()
    }
    pub(crate) fn invocation_deadline(&self) -> Result<Instant, Diagnostic> {
        let mut state = self.lock()?;
        state.start()?;
        let individual = Instant::now()
            .checked_add(state.invocation_timeout)
            .ok_or_else(|| fault("LRR-E-DISCOVERY-LIMIT", "invocation deadline overflow"))?;
        Ok(individual.min(state.deadline.expect("started expansion deadline")))
    }
    pub fn totals(&self) -> Result<ExpansionTotals, Diagnostic> {
        Ok(self.lock()?.totals())
    }
    /// Account a validated output; execution/adoption authority is handled separately.
    pub fn record_output(&self, record: &ExpansionRecord) -> Result<(), Diagnostic> {
        self.observe(&BTreeMap::from([(
            record.key().fingerprint(),
            record.usage(),
        )]))
    }
    pub(crate) fn check_record(&self, record: &ExpansionRecord) -> Result<(), Diagnostic> {
        self.lock()?.plan(&BTreeMap::from([(
            record.key().fingerprint(),
            record.usage(),
        )]))?;
        Ok(())
    }
    pub(crate) fn observe(
        &self,
        usage: &BTreeMap<Digest, ExpansionUsage>,
    ) -> Result<(), Diagnostic> {
        self.transaction(usage, || Ok(()))
    }
    /// Keep planning, source admission and accounting commit in one serialized transition.
    pub(crate) fn transaction<T>(
        &self,
        usage: &BTreeMap<Digest, ExpansionUsage>,
        work: impl FnOnce() -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let mut state = self.lock()?;
        state.start()?;
        let charge = state.plan(usage)?;
        let value = work()?;
        state.check_deadline()?;
        state.frozen = true;
        state.tokens = charge.tokens;
        state.wire_bytes = charge.wire_bytes;
        state.outputs.extend(charge.outputs);
        Ok(value)
    }
}
impl State {
    fn start(&mut self) -> Result<(), Diagnostic> {
        self.frozen = true;
        if self.deadline.is_none() {
            self.deadline = Some(
                Instant::now()
                    .checked_add(self.cumulative_timeout)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-DISCOVERY-LIMIT",
                            "cumulative expansion deadline overflow",
                        )
                    })?,
            );
        }
        self.check_deadline()
    }
    /// E-R5-006: rustc imposes no expansion time limit (only its recursion
    /// limit), so the cumulative deadline is observed but never enforced.
    fn check_deadline(&self) -> Result<(), Diagnostic> {
        Ok(())
    }
    fn totals(&self) -> ExpansionTotals {
        ExpansionTotals {
            tokens: self.tokens,
            wire_bytes: self.wire_bytes,
            outputs: self.outputs.len(),
        }
    }
    fn plan(&self, usage: &BTreeMap<Digest, ExpansionUsage>) -> Result<Charge, Diagnostic> {
        self.check_deadline()?;
        let limit = |message: &str| fault("LRR-E-DISCOVERY-LIMIT", message);
        usage
            .len()
            .checked_mul(std::mem::size_of::<(Digest, Digest)>())
            .ok_or_else(|| limit("expansion accounting metadata limit"))?;
        let mut outputs = Vec::new();
        outputs
            .try_reserve_exact(usage.len())
            .map_err(|_| limit("expansion accounting allocation"))?;
        let mut tokens = self.tokens;
        let mut wire_bytes = self.wire_bytes;
        for (identity, observed) in usage {
            if observed
                .procedural_epoch
                .is_some_and(|epoch| epoch != self.epoch)
            {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "accounted procedural output belongs to another epoch",
                ));
            }
            if let Some(old) = self.outputs.get(identity) {
                if *old != observed.fingerprint {
                    return Err(fault(
                        "LRR-E-EXPANSION-CONFLICT",
                        "accounted expansion output changed",
                    ));
                }
                continue;
            }
            self.outputs
                .len()
                .checked_add(outputs.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| limit("request expansion invocation count limit"))?;
            tokens = tokens
                .checked_add(observed.tokens)
                .ok_or_else(|| limit("cumulative generated token limit"))?;
            wire_bytes = wire_bytes
                .checked_add(observed.wire_bytes)
                .ok_or_else(|| limit("request cumulative generated wire limit"))?;
            outputs.push((*identity, observed.fingerprint));
        }
        Ok(Charge {
            tokens,
            wire_bytes,
            outputs,
        })
    }
}
