//! Cold closure environment selection over admitted ordinary syntax. Captures
//! and entered reads share LexicalValue and its retained relevant dependencies.
use super::{
    dependency::{InputKey, InputValue},
    key::QueryKey,
    memo::{Database, PreparedInputGuard},
    poll::QueryPoll,
    provider::{Provided, SyntaxProvider},
    value::QueryValue,
};
use crate::{
    core::diagnostic::Diagnostic,
    frontend::source_value_goals::LocalValueAccess,
    ir::{
        op::CaptureSource,
        pc::{NodeId, PcKey, Phase},
    },
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
    },
};
use ra_ap_syntax::{AstNode, WalkEvent, ast};
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
fn fault(message: String) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-CAPTURE", message)
}
pub(super) fn thunk(
    db: &mut Database,
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    pc: &PcKey,
) -> QueryPoll<Option<Provided>> {
    if pc.phase != Phase::Entry.code().expect("entry phase")
        || !index
            .nodes()
            .get(pc.node as usize)
            .is_some_and(|node| node.role == NodeRole::Expression(ExpressionKind::Closure))
        || !matches!(
            db.input(&InputKey::BodyEnvironment(pc.body.clone()))
                .as_deref(),
            Some(InputValue::BodyEnvironment(_))
        )
    {
        return QueryPoll::Ready(Ok(None));
    }
    let syntax = ready!(QueryPoll::Ready(parsed.entered_node_syntax(
        body,
        index,
        NodeId(pc.node)
    )));
    let Some(closure) = syntax.and_then(ast::ClosureExpr::cast) else {
        return QueryPoll::Ready(Err(fault("indexed closure syntax absent".into())));
    };
    let Some(closure_body) = closure.body() else {
        return QueryPoll::Ready(Ok(None));
    };
    let mut captures = Vec::new();
    let mut guards = Vec::new();
    let mut pattern_uses = Vec::new();
    // Walk this retained subtree once at creation preparation. Nested closure
    // uses contribute outer free bindings. Like BodyIndex, nested items own
    // independent bodies; neither their initializer nor function body captures
    // a local of this closure. Prune them rather than looking for absent nodes.
    let mut walk = closure_body.syntax().preorder();
    while let Some(event) = walk.next() {
        let WalkEvent::Enter(syntax) = event else {
            continue;
        };
        if ast::MacroCall::can_cast(syntax.kind()) {
            return QueryPoll::Ready(Err(fault(
                "closure capture requires actual expanded source, not macro argument tokens".into(),
            )));
        }
        if ast::Item::can_cast(syntax.kind()) {
            walk.skip_subtree();
            continue;
        }
        let Some(path) = ast::PathExpr::cast(syntax) else {
            continue;
        };
        let Some(written) = path.path() else {
            continue;
        };
        // Qualified paths cannot denote a captured place. Their declaration
        // completion stays cold until ordinary reached execution demands it.
        if written.qualifier().is_some() || written.coloncolon_token().is_some() {
            continue;
        }
        let range = path.syntax().text_range();
        let relative = crate::syntax::spans::SourceRange {
            start: ready!(QueryPoll::Ready(
                u32::from(range.start())
                    .checked_sub(body.range.start)
                    .ok_or_else(|| fault("capture path before owning body".into()))
            )),
            end: ready!(QueryPoll::Ready(
                u32::from(range.end())
                    .checked_sub(body.range.start)
                    .ok_or_else(|| fault("capture path before owning body".into()))
            )),
        };
        let Some(node) = index
            .nodes_at_range(relative)
            .find(|node| node.role == NodeRole::Expression(ExpressionKind::Path))
        else {
            return QueryPoll::Ready(Err(fault(
                "capture path lacks actual indexed occurrence".into(),
            )));
        };
        let at = PcKey {
            body: pc.body.clone(),
            node: node.id.0,
            phase: 0,
        };
        let key = QueryKey::LexicalValue(at.clone());
        let selected = ready!(db.fetch(&SyntaxProvider, key.clone()));
        let QueryValue::LexicalValue(goal) = selected.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("capture lexical query variant differs".into())));
        };
        if let Some(demand) = goal.pending() {
            return QueryPoll::Ready(Err(fault(format!(
                "capture lexical source selection remains pending: {demand:?}"
            ))));
        }
        if !selected.cacheable {
            return QueryPoll::Ready(Err(fault(
                "capture lexical dependencies are not retained".into(),
            )));
        }
        guards.push(ready!(db.prepare_input_guard(
            &SyntaxProvider,
            &key,
            selected.fingerprint
        )));
        let access = ready!(QueryPoll::Ready(goal.indexed_local_access_at(
            index,
            &at,
            NodeId(pc.node)
        )));
        let source = match access {
            Some(LocalValueAccess::Binding(binding)) => {
                let range = index.nodes()[binding.0 as usize].range;
                let start = ready!(QueryPoll::Ready(
                    body.range
                        .start
                        .checked_add(range.start)
                        .ok_or_else(|| fault("capture binding position overflow".into()))
                ));
                if closure.syntax().text_range().contains(start.into()) {
                    continue;
                }
                CaptureSource::Binding(binding)
            }
            Some(LocalValueAccess::Parameter(ordinal)) => CaptureSource::Parameter(ordinal),
            None => continue,
        };
        let mode = match ready!(QueryPoll::Ready(crate::syntax::capture_use(
            body,
            index,
            &closure,
            path.syntax()
        ))) {
            crate::syntax::SourceCaptureUse::Mode(mode) => mode,
            crate::syntax::SourceCaptureUse::Pattern { owner, projections } => {
                let query = QueryKey::PatternPlan(PcKey {
                    body: pc.body.clone(),
                    node: owner.0,
                    phase: 0,
                });
                let fetched = ready!(db.fetch(&SyntaxProvider, query.clone()));
                let QueryValue::PatternPlan(plan) = fetched.value.as_ref() else {
                    return QueryPoll::Ready(Err(fault("capture pattern plan differs".into())));
                };
                guards.push(ready!(db.prepare_input_guard(
                    &SyntaxProvider,
                    &query,
                    fetched.fingerprint
                )));
                pattern_uses.push(crate::ir::op::CapturePatternUse {
                    source,
                    plan: Arc::clone(plan),
                    projections,
                });
                crate::ir::op::CaptureMode::Shared
            }
        };
        crate::syntax::merge_capture(&mut captures, source, mode);
    }
    let refs: Vec<&PreparedInputGuard> = guards.iter().collect();
    let guard = ready!(QueryPoll::Ready(PreparedInputGuard::join(&refs)));
    let recipe = ready!(QueryPoll::Ready(
        parsed.initial_thunk_with_closure_captures(body, index, pc, &captures, &pattern_uses)
    ));
    QueryPoll::Ready(
        recipe
            .with_shared_prepared_inputs(Arc::new(guard))
            .map(|recipe| {
                Some(Provided {
                    value: QueryValue::Thunk(Box::new(recipe)),
                    cacheable: true,
                })
            }),
    )
}
