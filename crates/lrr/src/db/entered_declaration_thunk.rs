//! Ordinary declaration lowering after the actual annotation/source preparation.
use super::{dependency::InputKey, key::QueryKey, memo::Database, poll::QueryPoll,
    provider::{Provided, SyntaxProvider}, value::QueryValue};
use crate::{ir::pc::PcKey, syntax::{body_index::{BodyIndex, NodeRole}, outline::BodyHandle, parse::ParsedFile}};

pub(super) fn thunk(db: &mut Database, parsed: &ParsedFile, body: &BodyHandle,
    index: &BodyIndex, key: &PcKey) -> QueryPoll<Option<Provided>> {
    if !index.nodes().get(key.node as usize).is_some_and(|node| node.role == NodeRole::OpaqueItem) {
        return QueryPoll::Ready(Ok(None));
    }
    // Absence is observed; plain unprepared declarations keep their ordinary route.
    if db.input(&InputKey::BodyEnvironment(key.body.clone())).is_none() { return QueryPoll::Ready(Ok(None)); }
    let plan = match db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(key.body.clone())) {
        QueryPoll::Ready(Ok(value)) => value.value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::SourceLocalPlan(plan) = plan.as_ref() else {
        return QueryPoll::Ready(Err(crate::core::diagnostic::Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT", "declaration source plan variant differs".into())));
    };
    let admission = match plan.declaration_name_completions().admit_node(plan.context(), index, key) {
        Ok(Some(admission)) => admission,
        Ok(None) => return QueryPoll::Ready(Ok(None)),
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    if let Err(error) = admission.require_published(db) { return QueryPoll::Ready(Err(error)); }
    match parsed.initial_thunk_with_declaration(body, index, key, &admission) {
        Ok(recipe) => QueryPoll::Ready(Ok(Some(Provided { value: QueryValue::Thunk(Box::new(recipe)), cacheable: true }))),
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
