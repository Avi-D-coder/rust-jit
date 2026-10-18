//! One cold bridge from the shared Value selection to ordinary path lowering.
use super::{
    dependency::{InputKey, InputValue},
    key::QueryKey,
    memo::Database,
    poll::QueryPoll,
    provider::{Provided, SyntaxProvider},
    value::QueryValue,
};
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::{PcKey, Phase},
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
    },
};
use std::sync::Arc;
macro_rules! ready {
    ($e:expr) => {
        match $e {
            QueryPoll::Ready(Ok(v)) => v,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        }
    };
}
pub(super) fn thunk(
    db: &mut Database,
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    pc: &PcKey,
) -> QueryPoll<Option<Provided>> {
    if pc.phase != Phase::Entry.code().expect("entry encoding")
        || !index
            .nodes()
            .get(pc.node as usize)
            .is_some_and(|node| node.role == NodeRole::Expression(ExpressionKind::Path))
        || !matches!(
            db.input(&InputKey::BodyEnvironment(pc.body.clone()))
                .as_deref(),
            Some(InputValue::BodyEnvironment(_))
        )
    {
        return QueryPoll::Ready(Ok(None));
    }
    let key = QueryKey::EnteredValue(pc.clone());
    let selected = ready!(db.fetch(&SyntaxProvider, key.clone()));
    let QueryValue::EnteredValue(goal) = selected.value.as_ref() else {
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "Value access query variant differs".into(),
        )));
    };
    if !selected.cacheable {
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "Value access dependencies are not retained".into(),
        )));
    }
    let guard = Arc::new(ready!(db.prepare_input_guard(
        &SyntaxProvider,
        &key,
        selected.fingerprint
    )));
    let recipe = ready!(QueryPoll::Ready(
        parsed.initial_thunk_with_value_goal(body, index, pc, goal)
    ));
    // Preserve the same open result/argument constraints on the selected
    // read or item operation, with one joined retained input guard.
    if let Some(open) = ready!(super::binding_constraint_thunk::prepare_open_expression(
        db, index, pc, true
    )) {
        return QueryPoll::Ready(open.apply_with_inputs(index, recipe, guard).map(Some));
    }
    QueryPoll::Ready(recipe.with_shared_prepared_inputs(guard).map(|recipe| {
        Some(Provided {
            value: QueryValue::Thunk(Box::new(recipe)),
            cacheable: true,
        })
    }))
}
