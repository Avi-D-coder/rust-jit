//! Retained cold binding plan and actual input closure, shared by all phases.
use super::binding_constraints::{BindingConstraintState, PreparedBindingConstraints};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::pc::PcKey,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PreparedBindingPlan {
    constraints: PreparedBindingConstraints,
    guard: Option<Arc<PreparedInputGuard>>,
    guard_fingerprint: Option<Digest>,
}
impl PreparedBindingPlan {
    pub(crate) fn constraints(&self) -> &PreparedBindingConstraints {
        &self.constraints
    }
    pub(crate) fn guard(&self) -> Option<&Arc<PreparedInputGuard>> {
        self.guard.as_ref()
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        // Conservative per-query ownership, including a shared guard's payload.
        std::mem::size_of::<Self>()
            .checked_add(
                self.guard
                    .as_ref()
                    .map_or(0, |guard| guard.metadata_bytes()),
            )
            .and_then(|n| {
                n.checked_add(usize::from(self.guard.is_some()) * 2 * std::mem::size_of::<usize>())
            })
            .unwrap_or(usize::MAX)
    }
}
pub(crate) fn prepare(db: &mut Database, key: &PcKey) -> QueryPoll<PreparedBindingPlan> {
    let demand = QueryKey::BindingConstraints(key.clone());
    let value = match db.fetch(&SyntaxProvider, demand.clone()) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::BindingConstraints(constraints) = value.value.as_ref() else {
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "prepared binding source query variant differs".into(),
        )));
    };
    let guard = if matches!(constraints.state(), BindingConstraintState::Resolved(_)) {
        if !value.cacheable {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "numeric source constraint has unretained dependencies".into(),
            )));
        }
        match db.prepare_input_guard(&SyntaxProvider, &demand, value.fingerprint) {
            QueryPoll::Ready(Ok(guard)) => Some(Arc::new(guard)),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    } else {
        None
    };
    let guard_fingerprint = guard.as_ref().map(|guard| guard.fingerprint());
    QueryPoll::Ready(Ok(PreparedBindingPlan {
        constraints: constraints.as_ref().clone(),
        guard,
        guard_fingerprint,
    }))
}
impl StableEncode for PreparedBindingPlan {
    const DOMAIN: &'static str = "lrr/v1/prepared-binding-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.constraints);
        e.option(&self.guard_fingerprint, |e, fingerprint| {
            e.digest(*fingerprint)
        });
    }
}
