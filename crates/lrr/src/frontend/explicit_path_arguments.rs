//! Indexed path argument selection shared by constructor constraints, record
//! preparation and ordinary call entry. Syntax is retained by SourceConstraints;
//! named leaves use the existing entered Type service, never displayed text.
use super::{ExplicitType, fault, select_argument};
use crate::{
    core::diagnostic::Diagnostic,
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::source_locals::PreparedSourceLocalPlan,
    ir::pc::{NodeId, Phase},
    source::manifest::PcKey,
    syntax::{
        source_constraints::{FactKind, PathTypeArguments},
        spans::SourceRange,
    },
};
use std::sync::Arc;

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum PathArgumentPosition {
    Owner,
    Constructor,
}
pub(crate) struct SelectedPathArguments {
    pub(crate) arguments: Vec<Option<ExplicitType>>,
    pub(crate) guards: Vec<Arc<PreparedInputGuard>>,
}
macro_rules! ready {
    ($poll:expr) => {
        match $poll {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
macro_rules! checked {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(error) => return QueryPoll::Ready(Err(error)),
        }
    };
}
pub(crate) fn select_path_arguments(
    db: &mut Database,
    owner: &Arc<PreparedSourceLocalPlan>,
    key: &PcKey,
    position: PathArgumentPosition,
) -> QueryPoll<Option<SelectedPathArguments>> {
    ready!(owner.require_current(db));
    let facts = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::SourceConstraints(key.body.clone())
    ));
    let QueryValue::SourceConstraints(facts) = facts.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("path argument syntax plan differs")));
    };
    let source = owner.context().source();
    if source.index().body() != &key.body || key.phase != 0 {
        return QueryPoll::Ready(Err(fault("path argument body/entry differs")));
    }
    let node = NodeId(key.node);
    let call = facts
        .fact(node)
        .is_some_and(|fact| fact.kind() == FactKind::Call);
    let path = if call {
        facts.fact(node).and_then(|fact| fact.operand(0))
    } else {
        Some(node)
    };
    let Some(arguments) = path.and_then(|node| facts.path_arguments(node)) else {
        return QueryPoll::Ready(Ok(None));
    };
    let selected = match position {
        PathArgumentPosition::Owner if call => arguments.owner_segment.as_ref(),
        PathArgumentPosition::Owner => arguments.final_segment.as_ref(),
        PathArgumentPosition::Constructor => arguments
            .final_segment
            .as_ref()
            .or(arguments.owner_segment.as_ref()),
    };
    let nodes = match selected {
        None => return QueryPoll::Ready(Ok(None)),
        Some(PathTypeArguments::Types(nodes)) => nodes,
        Some(PathTypeArguments::Unsupported) => {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "path const or associated argument requires its typed source goal".into(),
            )));
        }
    };
    let mut result = SelectedPathArguments {
        arguments: Vec::new(),
        guards: Vec::new(),
    };
    for node in nodes {
        let indexed = checked!(
            source
                .index()
                .nodes()
                .get(node.0 as usize)
                .ok_or_else(|| fault("path Type argument node absent"))
        );
        let range = SourceRange {
            start: checked!(
                source
                    .body()
                    .range
                    .start
                    .checked_add(indexed.range.start)
                    .ok_or_else(|| fault("path Type argument range overflow"))
            ),
            end: checked!(
                source
                    .body()
                    .range
                    .start
                    .checked_add(indexed.range.end)
                    .ok_or_else(|| fault("path Type argument range overflow"))
            ),
        };
        let site = checked!(source.index().pc(*node, Phase::Entry));
        let selected = ready!(select_argument(db, owner, key, &site, range));
        result.arguments.push(match selected {
            Some(selected) => {
                result.guards.extend(selected.guards);
                Some(selected.source)
            }
            None => None,
        });
    }
    QueryPoll::Ready(Ok(Some(result)))
}
