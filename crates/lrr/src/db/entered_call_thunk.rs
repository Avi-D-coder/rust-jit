//! Select the retained source call proof for the ordinary CallExpr lowerer.
//! This cold query adapter never executes, publishes or prepares a callee body.
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
    frontend::entered_call_goals::EnteredCallSelection,
    ir::pc::PcKey,
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
    },
};
use std::sync::Arc;
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALL-CONTEXT", message.into())
}
pub(super) fn thunk(
    db: &mut Database,
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    pc: &PcKey,
) -> QueryPoll<Option<Provided>> {
    if !index
        .nodes()
        .get(pc.node as usize)
        .is_some_and(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
        || !matches!(
            db.input(&InputKey::BodyEnvironment(pc.body.clone()))
                .as_deref(),
            Some(InputValue::BodyEnvironment(_))
        )
    {
        return QueryPoll::Ready(Ok(None));
    }
    match parsed.entered_call_requires_goal(body, index, pc) {
        Ok(true) => {}
        Ok(false) => {
            return QueryPoll::Ready(parsed.initial_thunk(body, index, pc).map(|recipe| {
                Some(Provided {
                    value: QueryValue::Thunk(Box::new(recipe)),
                    cacheable: true,
                })
            }));
        }
        Err(error) => return QueryPoll::Ready(Err(error)),
    }
    let mut entry = pc.clone();
    entry.phase = 0;
    let prepared = ready!(db.fetch(&SyntaxProvider, QueryKey::EnteredCall(entry)));
    let QueryValue::EnteredCall(goal) = prepared.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("source call prepared goal variant differs")));
    };
    if !prepared.cacheable {
        return QueryPoll::Ready(Err(fault("source call dependencies are not retained")));
    }
    // Module/qualified/value paths keep their existing safety checks until their
    // exact source-selection receipts join this route. No Local may fall back.
    if !matches!(goal.selection(), EnteredCallSelection::Local(_)) {
        return QueryPoll::Ready(Ok(None));
    }
    if let Err(error) = goal.require_current(db) {
        return QueryPoll::Ready(Err(error));
    }
    let recipe = match parsed.initial_thunk_with_source_call(body, index, pc, goal) {
        Ok(recipe) => recipe,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    QueryPoll::Ready(
        recipe
            .with_shared_prepared_inputs(Arc::clone(goal.guard()))
            .map(|recipe| {
                Some(Provided {
                    value: QueryValue::Thunk(Box::new(recipe)),
                    cacheable: true,
                })
            }),
    )
}
