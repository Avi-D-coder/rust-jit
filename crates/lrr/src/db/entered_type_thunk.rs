//! Cold selection of an actual indexed Type goal for ordinary expression lowering.
//! Installed recipes retain the relevant immutable input closure; they do not
//! reconstruct source plans or enter a second expression evaluator.
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
    ir::pc::{NodeId, PcKey},
    syntax::{
        body_index::{BodyIndex, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
        source_constraints::FactKind,
        thunks_let_annotations::{AnnotationShape, LetAnnotation},
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;
/// Whether a written Type contains a trait object.
fn object(shape: &AnnotationShape) -> bool {
    match shape {
        AnnotationShape::Dyn(_) => true,
        AnnotationShape::Leaf(_) | AnnotationShape::Infer => false,
        AnnotationShape::Tuple(members) => members.iter().any(object),
        AnnotationShape::Generic(_, arguments) => arguments.iter().any(object),
        AnnotationShape::Array(inner, _) | AnnotationShape::Slice(inner) => object(inner),
        AnnotationShape::Ref { target, .. } => object(target),
    }
}
/// A generic target with an unsized slice argument (`Box<[T]>`) or a
/// reference to a slice (`&[u8]`, `&W<[T]>`): rustc's unsizing coercion cast
/// (`CoerceUnsized`), like a trait-object target.
fn unsizing(shape: &AnnotationShape) -> bool {
    match shape {
        AnnotationShape::Generic(_, arguments) => arguments
            .iter()
            .any(|argument| matches!(argument, AnnotationShape::Slice(_)) || unsizing(argument)),
        AnnotationShape::Ref { target, .. } => {
            matches!(target.as_ref(), AnnotationShape::Slice(_)) || unsizing(target)
        }
        _ => false,
    }
}
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
    // The written structure of the target, as a `let` annotation's: a
    // trait-object target is a coercion cast; a named target is a primitive
    // cast; any other structure needs its own cast integration.
    let written = match parsed
        .entered_node_syntax(body, index, NodeId(pc.node))
        .map(|syntax| syntax.and_then(ast::CastExpr::cast).and_then(|cast| cast.ty()))
    {
        Ok(Some(written)) => written,
        Ok(None) => return QueryPoll::Ready(Err(fault("cast actual Type syntax absent"))),
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let shape = match parsed.annotation_shape(body, index, pc.body.definition, &written) {
        Ok(shape) => shape,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    match &shape {
        // A named (possibly parenthesized) target: its entered Type goal.
        Some(AnnotationShape::Leaf(_)) => {}
        Some(shape) if object(shape) || unsizing(shape) => {
            let mut guards = Vec::new();
            let selected = ready!(super::let_annotation_thunk::select(db, pc, shape, &mut guards));
            let guard = {
                let borrowed: Vec<&PreparedInputGuard> = guards.iter().map(Arc::as_ref).collect();
                match PreparedInputGuard::join(&borrowed) {
                    Ok(guard) => guard,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
            let coercion = LetAnnotation {
                range: crate::syntax::ast_adapter::range(written.syntax().text_range()),
                written: Arc::new(selected),
            };
            let recipe = match parsed.initial_thunk_with_cast_coercion(body, index, pc, &coercion) {
                Ok(recipe) => recipe,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            return QueryPoll::Ready(recipe.with_shared_prepared_inputs(Arc::new(guard)).map(
                |recipe| {
                    Some(Provided {
                        value: QueryValue::Thunk(Box::new(recipe)),
                        cacheable: true,
                    })
                },
            ));
        }
        _ => {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                format!(
                    "cast to `{}` requires its pointer, function-pointer or reference cast integration",
                    written.syntax().text()
                ),
            )));
        }
    }
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
