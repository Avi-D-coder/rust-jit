//! Pure memo adapter: consume real retained structural inputs, never expand or
//! publish. The caller coordinator must register the opaque completed plan.
use super::{
    dependency::{InputKey, InputValue},
    key::PreparedBodySyntaxKey,
    memo::{Database, source_syntax_inputs::PreparedBodySyntax},
    poll::QueryPoll,
    provider::{Provided, SyntaxProvider},
    value::QueryValue,
};
pub(super) fn prepare(
    _provider: &SyntaxProvider,
    db: &mut Database,
    key: &PreparedBodySyntaxKey,
) -> QueryPoll<Provided> {
    if let Err(error) = key.selection() {
        return QueryPoll::Ready(Err(error));
    }
    let Some(input) = db.input(&InputKey::CompletedSourceSyntax(key.plan)) else {
        return Provided::error(
            "LRR-E-EXPANSION-INPUT",
            "body syntax actual retained plan input absent",
        );
    };
    let InputValue::CompletedSourceSyntax(input) = input.as_ref() else {
        return Provided::error(
            "LRR-E-INTERNAL-SNAPSHOT",
            "body syntax retained input variant differs",
        );
    };
    match PreparedBodySyntax::prepare(db, key, input) {
        QueryPoll::Ready(Ok(result)) => Provided::pure(QueryValue::PreparedBodySyntax(result)),
        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => QueryPoll::NeedInput(request),
    }
}
