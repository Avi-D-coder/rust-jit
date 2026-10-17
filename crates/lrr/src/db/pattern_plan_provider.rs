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
    let retained = match db.fetch(provider, QueryKey::PatternPlan(owner.clone())) {
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
    // A destructuring `let`'s composite annotation is the entered Type
    // service's selection (the same `let_annotations` route an ordinary
    // `let` uses); its joined input guard stays with the installed recipe.
    if pc == &owner
        && plan.let_value_owner()
        && matches!(
            db.input(&super::dependency::InputKey::BodyEnvironment(pc.body.clone()))
                .as_deref(),
            Some(super::dependency::InputValue::BodyEnvironment(_))
        )
    {
        let site = match parsed.let_annotation_site(handle, index, pc) {
            Ok(site) => site,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if let Some(site) = site {
            let mut guards = Vec::new();
            let written =
                match super::let_annotation_thunk::select(db, pc, &site.shape, &mut guards) {
                    QueryPoll::Ready(Ok(written)) => written,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
            let borrowed: Vec<&super::memo::PreparedInputGuard> =
                guards.iter().map(Arc::as_ref).collect();
            let guard = match super::memo::PreparedInputGuard::join(&borrowed) {
                Ok(guard) => guard,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let recipe = plan
                .lower_let_annotated(
                    pc,
                    crate::ir::op::HintRecipe::Written(Arc::new(written)),
                )
                .and_then(|recipe| recipe.with_shared_prepared_inputs(Arc::new(guard)));
            return QueryPoll::Ready(Ok(Some(Provided {
                value: match recipe {
                    Ok(recipe) => QueryValue::Thunk(Box::new(recipe)),
                    Err(error) => QueryValue::Error(error),
                },
                cacheable: true,
            })));
        }
    }
    // Identifier classification is ordinary Value-namespace resolution at the
    // reached dispatcher; the immutable lowering is fully cacheable.
    QueryPoll::Ready(Ok(Some(Provided {
        value: match plan.lower(pc) {
            Ok(recipe) => QueryValue::Thunk(Box::new(recipe)),
            Err(error) => QueryValue::Error(error),
        },
        cacheable: true,
    })))
}

