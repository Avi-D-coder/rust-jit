//! Cold selection of an actual indexed Type goal for ordinary expression lowering.
//! Installed recipes retain the relevant immutable input closure; they do not
//! reconstruct source plans or enter a second expression evaluator.
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
    ir::pc::{NodeId, PcKey},
    syntax::{
        body_index::{BodyIndex, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
        source_constraints::FactKind,
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
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
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
        .is_some_and(|node| matches!(node.role, NodeRole::Expression(_)))
    {
        return QueryPoll::Ready(Ok(None));
    }
    let environment = db.input(&InputKey::BodyEnvironment(pc.body.clone()));
    let Some(InputValue::BodyEnvironment(_)) = environment.as_deref() else {
        // Historical unsealed component entry remains held, not new authority.
        return QueryPoll::Ready(Ok(None));
    };
    let facts = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::SourceConstraints(pc.body.clone())
    ));
    let QueryValue::SourceConstraints(facts) = facts.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("Type consumer source facts variant differs")));
    };
    if facts.body() != &pc.body || facts.structure() != index.semantic_identity() {
        return QueryPoll::Ready(Err(fault("Type consumer source structure differs")));
    }
    let Some(fact) = facts.fact(NodeId(pc.node)) else {
        return QueryPoll::Ready(Err(fault("Type consumer indexed expression absent")));
    };
    if fact.kind() != FactKind::Cast {
        return QueryPoll::Ready(Ok(None));
    }
    match parsed.entered_cast_requires_type(body, index, pc) {
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
    let Some(annotation) = fact.annotation() else {
        return QueryPoll::Ready(Err(fault("cast actual Type syntax absent")));
    };
    // Exactly the canonical Syntax node used by PreparedBodyTypeOccurrences;
    // the lowerer independently checks this is the selected cast's actual ty().
    let Some(target) = index
        .nodes_at_range(annotation)
        .find(|node| node.role == NodeRole::Syntax)
    else {
        return QueryPoll::Ready(Err(fault("cast Type lacks canonical indexed syntax")));
    };
    let key = PcKey {
        body: pc.body.clone(),
        node: target.id.0,
        phase: 0,
    };
    let prepared = ready!(db.fetch(&SyntaxProvider, QueryKey::PreparedTypePlan(key)));
    let QueryValue::PreparedTypePlan(plan) = prepared.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("Type consumer prepared plan variant differs")));
    };
    if !prepared.cacheable {
        return QueryPoll::Ready(Err(fault("Type consumer dependencies are not retained")));
    }
    let recipe = match parsed.initial_thunk_with_source_type(body, index, pc, plan.goal()) {
        Ok(recipe) => recipe,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    QueryPoll::Ready(
        recipe
            .with_shared_prepared_inputs(Arc::clone(plan.guard()))
            .map(|recipe| {
                Some(Provided {
                    value: QueryValue::Thunk(Box::new(recipe)),
                    cacheable: true,
                })
            }),
    )
}
