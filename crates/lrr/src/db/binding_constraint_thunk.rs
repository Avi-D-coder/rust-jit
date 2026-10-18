//! Cold shared source-constraint selection. Installed execution consumes the
//! ordinary verified recipe and its retained relevant-input guard only.
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
    frontend::binding_constraints::BindingConstraintState,
    ir::pc::{NodeId, PcKey},
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
        source_constraints::{FactKind, SourceOperator},
    },
};
use ra_ap_syntax::{AstNode, AstToken, ast};
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
    let Some(role) = index.nodes().get(pc.node as usize).map(|node| node.role) else {
        return QueryPoll::Ready(Ok(None));
    };
    // Only a `let`, a compound assignment to a local and a bare unsuffixed
    // numeric literal consume a source-constraint goal here. Test the
    // already-parsed syntax first so no other recipe gains a
    // source-constraint or environment dependency.
    let consumer = match role {
        NodeRole::Statement => Consumer::Statement,
        NodeRole::Expression(kind) => {
            let syntax = match parsed.entered_node_syntax(body, index, NodeId(pc.node)) {
                Ok(Some(syntax)) => syntax,
                Ok(None) => return QueryPoll::Ready(Ok(None)),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            if kind == ExpressionKind::Literal && unsuffixed_numeric(&syntax) {
                Consumer::Literal(NodeId(pc.node))
            } else if let Some(literal) = negated_literal(&syntax) {
                // `-literal` lowers as one negative literal at the negation.
                let range = crate::syntax::ast_adapter::range(literal.syntax().text_range());
                let (Some(start), Some(end)) = (
                    range.start.checked_sub(body.range.start),
                    range.end.checked_sub(body.range.start),
                ) else {
                    return QueryPoll::Ready(Ok(None));
                };
                let relative = crate::syntax::spans::SourceRange { start, end };
                match index
                    .nodes_at_range(relative)
                    .find(|node| node.role == NodeRole::Expression(ExpressionKind::Literal))
                {
                    Some(node) => Consumer::Literal(node.id),
                    None => return QueryPoll::Ready(Ok(None)),
                }
            } else if matches!(
                syntax.kind(),
                ra_ap_syntax::SyntaxKind::PATH_EXPR
                    | ra_ap_syntax::SyntaxKind::CALL_EXPR
                    | ra_ap_syntax::SyntaxKind::METHOD_CALL_EXPR
            ) {
                // A call or fieldless constructor whose type arguments later
                // code decides: its own result Type, stated at its Entry, and
                // the callee's decided generic arguments, read by whichever
                // phase prepares the call.
                let entry = crate::ir::pc::Phase::Entry.code().ok() == Some(pc.phase);
                if !entry && syntax.kind() == ra_ap_syntax::SyntaxKind::PATH_EXPR {
                    return QueryPoll::Ready(Ok(None));
                }
                Consumer::Open { entry }
            } else if ast::BinExpr::cast(syntax)
                .and_then(|binary| binary.op_kind())
                .is_some_and(|op| matches!(op, ast::BinaryOp::Assignment { op: Some(_) }))
            {
                Consumer::Compound
            } else {
                return QueryPoll::Ready(Ok(None));
            }
        }
        _ => return QueryPoll::Ready(Ok(None)),
    };
    // The legacy unsealed entry is explicitly held. A real publication selects
    // the common source services for original and retained syntax identically.
    let environment = db.input(&InputKey::BodyEnvironment(pc.body.clone()));
    let Some(InputValue::BodyEnvironment(_)) = environment.as_deref() else {
        return QueryPoll::Ready(Ok(None));
    };
    match consumer {
        Consumer::Statement => statement(db, parsed, body, index, pc),
        Consumer::Compound => compound_assignment(db, parsed, body, index, pc),
        Consumer::Literal(node) => literal(db, parsed, body, index, pc, node),
        Consumer::Open { entry } => open_expression(db, parsed, body, index, pc, entry),
    }
}
enum Consumer {
    Statement,
    Compound,
    Open { entry: bool },
    // The literal's own goal node (itself, or the literal a negation wraps).
    Literal(NodeId),
}
/// `-literal` (through parentheses) over an unsuffixed integer literal.
fn negated_literal(syntax: &ra_ap_syntax::SyntaxNode) -> Option<ast::Literal> {
    let prefix = ast::PrefixExpr::cast(syntax.clone())?;
    if prefix.op_kind() != Some(ast::UnaryOp::Neg) {
        return None;
    }
    let mut operand = prefix.expr()?;
    while let ast::Expr::ParenExpr(group) = operand {
        operand = group.expr()?;
    }
    let ast::Expr::Literal(literal) = operand else {
        return None;
    };
    (literal.token().kind() == ra_ap_syntax::SyntaxKind::INT_NUMBER
        && unsuffixed_numeric(literal.syntax()))
    .then_some(literal)
}
fn unsuffixed_numeric(syntax: &ra_ap_syntax::SyntaxNode) -> bool {
    let Some(literal) = ast::Literal::cast(syntax.clone()) else {
        return false;
    };
    let token = literal.token();
    match token.kind() {
        ra_ap_syntax::SyntaxKind::INT_NUMBER => {
            ast::IntNumber::cast(token).is_some_and(|number| number.suffix().is_none())
        }
        ra_ap_syntax::SyntaxKind::FLOAT_NUMBER => {
            ast::FloatNumber::cast(token).is_some_and(|number| number.suffix().is_none())
        }
        _ => false,
    }
}
/// A bare unsuffixed literal whose own inference variable the common source
/// constraints resolved. Anything unresolved keeps the existing route (the
/// reached parent hint); a pending relation is never an error here.
fn literal(
    db: &mut Database,
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    pc: &PcKey,
    node: NodeId,
) -> QueryPoll<Option<Provided>> {
    let key = PcKey {
        body: pc.body.clone(),
        node: node.0,
        phase: 0,
    };
    let prepared = ready!(db.fetch(&SyntaxProvider, QueryKey::PreparedBindingPlan(key.clone())));
    let QueryValue::PreparedBindingPlan(plan) = prepared.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("binding constraint query variant differs")));
    };
    let proof = plan.constraints();
    if !matches!(proof.state(), BindingConstraintState::Resolved(_)) {
        return QueryPoll::Ready(Ok(None));
    }
    let Some(guard) = plan.guard().filter(|_| prepared.cacheable) else {
        return QueryPoll::Ready(Ok(None));
    };
    let recipe = match parsed.initial_thunk_with_literal_constraints(body, index, pc, &key, proof) {
        Ok(recipe) => recipe,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    QueryPoll::Ready(
        recipe
            .with_shared_prepared_inputs(std::sync::Arc::clone(guard))
            .map(|recipe| {
                Some(Provided {
                    value: QueryValue::Thunk(Box::new(recipe)),
                    cacheable: true,
                })
            }),
    )
}
/// A call or fieldless constructor whose open type arguments the common
/// source constraints decided from its later uses. The ordinary recipe is
/// kept; only the reached result Type is stated first, as a parent's
/// `SetHint` would. Anything unresolved keeps the ordinary route untouched.
fn open_expression(
    db: &mut Database,
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    pc: &PcKey,
    entry: bool,
) -> QueryPoll<Option<Provided>> {
    let key = PcKey {
        body: pc.body.clone(),
        node: pc.node,
        phase: 0,
    };
    let prepared = ready!(db.fetch(&SyntaxProvider, QueryKey::PreparedBindingPlan(key)));
    let QueryValue::PreparedBindingPlan(plan) = prepared.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("binding constraint query variant differs")));
    };
    let proof = plan.constraints();
    if !matches!(proof.state(), BindingConstraintState::ResolvedType) {
        return QueryPoll::Ready(Ok(None));
    }
    let Some(guard) = plan.guard().filter(|_| prepared.cacheable) else {
        return QueryPoll::Ready(Ok(None));
    };
    // The result Type may be only partly stated (the projection of a decided
    // parameter is not): the decided arguments are delivered either way.
    let hint = match proof.hint_for(index, pc) {
        Ok(hint) => hint.filter(|_| entry),
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    if hint.is_none() && proof.inferred_arguments().is_none() {
        return QueryPoll::Ready(Ok(None));
    }
    // Only a node with a value of its own states a result Type.
    if index
        .nodes()
        .get(pc.node as usize)
        .is_none_or(|node| node.value.is_none())
    {
        return QueryPoll::Ready(Ok(None));
    }
    let recipe = match parsed.initial_thunk(body, index, pc) {
        Ok(recipe) => recipe,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    if matches!(
        recipe.recipe().exit,
        crate::ir::recipe::RecipeExit::Fault(_)
    ) {
        return QueryPoll::Ready(Ok(None));
    }
    let recipe = match hint {
        Some(hint) => match recipe.with_entry_hint(index, hint) {
            Ok(recipe) => recipe,
            Err(error) => return QueryPoll::Ready(Err(error)),
        },
        None => recipe,
    };
    let recipe = match proof.inferred_arguments() {
        Some(arguments) => recipe.with_inferred_arguments(std::sync::Arc::clone(arguments)),
        None => recipe,
    };
    QueryPoll::Ready(
        recipe
            .with_shared_prepared_inputs(std::sync::Arc::clone(guard))
            .map(|recipe| {
                Some(Provided {
                    value: QueryValue::Thunk(Box::new(recipe)),
                    cacheable: true,
                })
            }),
    )
}
/// A `let` statement's own binding goal.
fn statement(
    db: &mut Database,
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    pc: &PcKey,
) -> QueryPoll<Option<Provided>> {
    let key = QueryKey::PreparedBindingPlan(PcKey {
        body: pc.body.clone(),
        node: pc.node,
        phase: 0,
    });
    let prepared = ready!(db.fetch(&SyntaxProvider, key.clone()));
    let QueryValue::PreparedBindingPlan(plan) = prepared.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("binding constraint query variant differs")));
    };
    let proof = plan.constraints();
    match proof.state() {
        // A `let` statement never carries an open expression's answer.
        BindingConstraintState::NoNumericDemand
        | BindingConstraintState::Unconstrained
        | BindingConstraintState::ResolvedType => {
            return QueryPoll::Ready(Ok(None));
        }
        BindingConstraintState::Pending { node, demand } => {
            return QueryPoll::Ready(Err(fault(&format!(
                "numeric source constraint requires a missing common source goal: {demand:?} at source node {}",
                node.0
            ))));
        }
        BindingConstraintState::Resolved(_) => {}
    }
    if !prepared.cacheable {
        return QueryPoll::Ready(Err(fault(
            "numeric source constraint has unretained dependencies",
        )));
    }
    let recipe = match parsed.initial_thunk_with_source_constraints(body, index, pc, proof) {
        Ok(recipe) => recipe,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let Some(guard) = plan.guard() else {
        return QueryPoll::Ready(Err(fault(
            "prepared numeric source has no retained currentness guard",
        )));
    };
    QueryPoll::Ready(
        recipe
            .with_shared_prepared_inputs(std::sync::Arc::clone(guard))
            .map(|recipe| {
                Some(Provided {
                    value: QueryValue::Thunk(Box::new(recipe)),
                    cacheable: true,
                })
            }),
    )
}
/// `local op= value` where `local` is a plain `let` binding whose Type the
/// common source constraints resolved. Every phase shares the declaring
/// statement's one prepared plan and guard; any other place keeps its
/// existing route.
fn compound_assignment(
    db: &mut Database,
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    pc: &PcKey,
) -> QueryPoll<Option<Provided>> {
    let facts = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::SourceConstraints(pc.body.clone())
    ));
    let QueryValue::SourceConstraints(facts) = facts.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("compound source facts variant differs")));
    };
    if facts.body() != &pc.body || facts.structure() != index.semantic_identity() {
        return QueryPoll::Ready(Err(fault("compound source structure differs")));
    }
    let Some(fact) = facts.fact(NodeId(pc.node)) else {
        return QueryPoll::Ready(Ok(None));
    };
    if fact.kind() != FactKind::Binary
        || !matches!(
            fact.operation(),
            Some(SourceOperator::CompoundArithmetic | SourceOperator::CompoundShift)
        )
    {
        return QueryPoll::Ready(Ok(None));
    }
    let Some(place) = fact.operand(0) else {
        return QueryPoll::Ready(Ok(None));
    };
    let use_key = PcKey {
        body: pc.body.clone(),
        node: place.0,
        phase: 0,
    };
    let Some(statement) = ready!(crate::frontend::binding_constraints::declaring_let(
        db, &use_key
    )) else {
        return QueryPoll::Ready(Ok(None));
    };
    let prepared = ready!(db.fetch(&SyntaxProvider, QueryKey::PreparedBindingPlan(statement)));
    let QueryValue::PreparedBindingPlan(plan) = prepared.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("binding constraint query variant differs")));
    };
    let proof = plan.constraints();
    if !matches!(proof.state(), BindingConstraintState::Resolved(_)) {
        return QueryPoll::Ready(Ok(None));
    }
    let Some(guard) = plan.guard().filter(|_| prepared.cacheable) else {
        return QueryPoll::Ready(Err(fault(
            "prepared numeric source has no retained currentness guard",
        )));
    };
    let recipe = match parsed.initial_thunk_with_local_constraints(body, index, pc, proof) {
        Ok(recipe) => recipe,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    QueryPoll::Ready(
        recipe
            .with_shared_prepared_inputs(std::sync::Arc::clone(guard))
            .map(|recipe| {
                Some(Provided {
                    value: QueryValue::Thunk(Box::new(recipe)),
                    cacheable: true,
                })
            }),
    )
}
