//! Reached ordinary pattern phases select an existing pure memo result.
use super::{
    key::QueryKey,
    memo::Database,
    poll::QueryPoll,
    provider::{Provided, SyntaxProvider},
    value::QueryValue,
};
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::PcKey,
    syntax::{
        body_index::BodyIndex, outline::BodyHandle, parse::ParsedFile,
        pattern_plan::PatternSourcePlan,
    },
};
use std::sync::Arc;

pub(super) fn thunk(
    provider: &SyntaxProvider,
    db: &mut Database,
    parsed: &Arc<ParsedFile>,
    handle: &BodyHandle,
    index: &Arc<BodyIndex>,
    pc: &PcKey,
) -> QueryPoll<Option<Provided>> {
    let owner = match PatternSourcePlan::owner_for(index, pc) {
        Ok(Some(owner)) => owner,
        Ok(None) => return QueryPoll::Ready(Ok(None)),
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let retained = match db.fetch(provider, QueryKey::PatternPlan(owner)) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    };
    let QueryValue::PatternPlan(plan) = retained.value.as_ref() else {
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "pattern source-plan query variant differs".into(),
        )));
    };
    // The dependency-checked query owns immutable roots. An equivalent memo
    // replacement need not have the same allocation address; entry performs
    // only these constant-size identity checks, never a whole-index comparison.
    if handle.key() != pc.body
        || plan.parsed().content != parsed.content
        || plan.parsed().profile != parsed.profile
        || plan.index().body() != index.body()
        || plan.index().profile() != index.profile()
    {
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "pattern source-plan roots differ".into(),
        )));
    }
    let binding = plan.requires_binding_admission(pc);
    let recipe = if binding {
        SyntaxProvider::scalar_binding_admission(db, index, pc).and_then(|proof| {
            plan.lower(pc, Some(&proof))
                .and_then(|recipe| recipe.with_namespace_witness(db))
        })
    } else {
        plan.lower(pc, None)
    };
    QueryPoll::Ready(Ok(Some(Provided {
        value: match recipe {
            Ok(recipe) => QueryValue::Thunk(Box::new(recipe)),
            Err(error) => QueryValue::Error(error),
        },
        // Existing reached namespace absence/currentness admission is reminted
        // independently; immutable structural source preparation remains cached.
        cacheable: !binding,
    })))
}

#[cfg(test)]
#[path = "pattern_plan_provider_tests.rs"]
mod tests;
