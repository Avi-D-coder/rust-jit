//! Pure memoized structural source facts; no call/body/type execution demand.
use super::{
    dependency::{InputKey, InputValue},
    key::QueryKey,
    memo::Database,
    poll::QueryPoll,
    provider::{Provided, SyntaxProvider},
    value::QueryValue,
};
use crate::{source::manifest::BodyKey, syntax::source_constraints::SourceConstraintPlan};

pub(super) fn prepare(
    provider: &SyntaxProvider,
    db: &mut Database,
    body: &BodyKey,
) -> QueryPoll<Provided> {
    let structure = match db.fetch(provider, QueryKey::BodyStructure(body.clone())) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
        return Provided::error(
            "LRR-E-INTERNAL-SNAPSHOT",
            "source constraint index variant differs",
        );
    };
    let handle = db.input(&InputKey::DefinitionBody(body.definition));
    let source = db.input(&InputKey::DefinitionSource(body.definition));
    let (Some(InputValue::DefinitionBody(handle)), Some(InputValue::DefinitionSource(key))) =
        (handle.as_deref(), source.as_deref())
    else {
        return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "source constraint inputs absent");
    };
    let parsed = match db.fetch(provider, QueryKey::ParseFile(key.clone())) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        return Provided::error(
            "LRR-E-INTERNAL-SNAPSHOT",
            "source constraint parse variant differs",
        );
    };
    match SourceConstraintPlan::prepare(parsed, handle, index) {
        Ok(plan) => Provided::pure(QueryValue::SourceConstraints(Box::new(plan))),
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}

#[cfg(test)]
#[path = "source_constraint_tests.rs"]
mod tests;
