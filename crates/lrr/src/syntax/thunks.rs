//! Pure entered-phase lowering. No guest state or child thunk queries.
use super::{
    body_index::{BodyIndex, ExpressionKind, NodeRole},
    outline::BodyHandle,
    parse::ParsedFile,
};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    frontend::thunks,
    ir::{
        op::{
            DecodedLiteral, DefinitionNumericContext, HintRecipe, LiteralKind, LiteralRecipe,
            NumericBinary, NumericUnary, Op, PrimitiveType,
        },
        pc::{NodeId, PcKey, Phase},
        recipe::{ControlRecipe, Origin, RecipeDraft, RecipeExit, VerifiedRecipe},
        validate::verify,
    },
};
use ra_ap_syntax::{
    AstNode, AstToken, SyntaxKind, SyntaxNode,
    ast::{self, HasArgList, HasGenericArgs, HasGenericParams, HasLoopBody, HasName},
};
#[path = "closure_thunk.rs"]
pub(crate) mod closure_thunk;
#[path = "format_args_thunk.rs"]
mod format_args_thunk;
#[path = "lang_call_thunk.rs"]
mod lang_call_thunk;
#[path = "let_annotations.rs"]
pub(crate) mod let_annotations;
#[path = "local_numeric_constraints.rs"]
mod local_numeric_constraints;
#[path = "method_call_thunk.rs"]
pub(crate) mod method_call_thunk;
#[path = "pattern_plan.rs"]
pub mod pattern_plan;
#[path = "pattern_tree.rs"]
pub mod pattern_tree;
#[path = "primitive_cast_target.rs"]
mod primitive_cast_target;
#[path = "runtime_local_numeric_context.rs"]
mod runtime_local_numeric_context;
#[path = "runtime_macro_numeric_context.rs"]
pub(crate) mod runtime_macro_numeric_context;
#[path = "string_literal.rs"]
pub mod string_literal;
#[path = "value_call_thunk.rs"]
pub(crate) mod value_call_thunk;
#[path = "value_paths.rs"]
mod value_paths;

#[derive(Default, Clone, Copy)]
struct EnteredAdmissions<'a> {
    numeric_admission:
        Option<&'a crate::frontend::runtime_macro_numeric::RuntimeMacroNumericAdmission>,
    type_name: Option<&'a crate::frontend::primitive_type_names::PrimitiveTypeNameAdmission>,
    source_constraints:
        Option<&'a crate::frontend::binding_constraints::PreparedBindingConstraints>,
    type_use: Option<&'a crate::frontend::entered_type_goals::EnteredTypeGoal>,
    call: Option<&'a crate::frontend::entered_call_goals::EnteredCallGoal>,
    let_annotation: Option<&'a let_annotations::LetAnnotation>,
    value_goal: Option<&'a crate::frontend::source_value_goals::EnteredValueGoal>,
    closure_captures: Option<&'a [crate::ir::op::ClosureCapture]>,
}

fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-SNAPSHOT", message.into())
}
fn id_for(index: &BodyIndex, body: &BodyHandle, syntax: &SyntaxNode) -> Result<NodeId, Diagnostic> {
    let range = syntax.text_range();
    let start = u32::from(range.start())
        .checked_sub(body.range.start)
        .ok_or_else(|| error("phase node before selected body"))?;
    let end = u32::from(range.end())
        .checked_sub(body.range.start)
        .ok_or_else(|| error("phase node before selected body"))?;
    index
        .nodes_at_range(super::spans::SourceRange { start, end })
        .find(|node| {
            if ast::Expr::can_cast(syntax.kind()) {
                matches!(node.role, NodeRole::Expression(_))
            } else if ast::Pat::can_cast(syntax.kind()) {
                node.role == NodeRole::Pattern
            } else if ast::Item::can_cast(syntax.kind()) {
                node.role == NodeRole::OpaqueItem
            } else if ast::MatchArm::can_cast(syntax.kind()) {
                node.role == NodeRole::MatchArm
            } else {
                node.role == NodeRole::Statement
            }
        })
        .map(|node| node.id)
        .ok_or_else(|| error("selected syntax has no structural phase identity"))
}
/// The PC a block's `Entry` continues to once its scope is entered: its first
/// statement or tail expression, else its commit.
fn block_first_pc(
    index: &BodyIndex,
    body: &BodyHandle,
    block: &ast::BlockExpr,
) -> Result<PcKey, Diagnostic> {
    let id = id_for(index, body, block.syntax())?;
    let first = block.stmt_list().and_then(|list| {
        list.syntax()
            .children()
            .find(|node| ast::Stmt::can_cast(node.kind()) || ast::Expr::can_cast(node.kind()))
    });
    if let Some(first) = first {
        if ast::Expr::can_cast(first.kind()) && index.nodes()[id.0 as usize].tail_scope.is_some() {
            index.pc(id, Phase::Exit)
        } else {
            index.pc(id_for(index, body, &first)?, Phase::Entry)
        }
    } else {
        index.pc(id, Phase::Commit)
    }
}
fn draft(
    index: &BodyIndex,
    key: &PcKey,
    ops: Vec<Op>,
    exit: RecipeExit,
) -> Result<VerifiedRecipe, Diagnostic> {
    verify(
        thunks::with_temporary_scope_entry(
            index,
            RecipeDraft {
                key: key.clone(),
                origin: Origin {
                    span: index.span(NodeId(key.node))?,
                    expansion_ancestry: vec![],
                },
                operations: ops,
                exits: vec![exit],
                dependencies: vec![],
            },
        )?,
        index,
    )
}
pub(super) fn hint(ty: Option<ast::Type>) -> Option<HintRecipe> {
    let Some(ty) = ty else {
        return Some(HintRecipe::None);
    };
    let p = match ty.syntax().text().to_string().trim() {
        "()" => PrimitiveType::Unit,
        "i8" => PrimitiveType::I8,
        "i16" => PrimitiveType::I16,
        "i32" => PrimitiveType::I32,
        "i64" => PrimitiveType::I64,
        "i128" => PrimitiveType::I128,
        "isize" => PrimitiveType::Isize,
        "u8" => PrimitiveType::U8,
        "u16" => PrimitiveType::U16,
        "u32" => PrimitiveType::U32,
        "u64" => PrimitiveType::U64,
        "u128" => PrimitiveType::U128,
        "usize" => PrimitiveType::Usize,
        "bool" => PrimitiveType::Bool,
        "char" => PrimitiveType::Char,
        "f32" => PrimitiveType::F32,
        "f64" => PrimitiveType::F64,
        _ => return None,
    };
    Some(HintRecipe::Primitive(p))
}
fn annotation_hint(
    parsed: &ParsedFile,
    ty: Option<ast::Type>,
    definition: crate::core::digest::Digest,
) -> Result<Option<HintRecipe>, Diagnostic> {
    let Some(ty) = ty else {
        return Ok(Some(HintRecipe::None));
    };
    // `_` alone is the absent annotation.
    if matches!(ty, ast::Type::InferType(_)) {
        return Ok(Some(HintRecipe::None));
    }
    // Pure lowering (no entered Type service) admits only a primitive
    // element/pointee selected like a bare primitive annotation, and states it
    // as the same written-Type hint the entered Type service selects for every
    // other composite annotation (`let_annotations`): one runtime route.
    if let ast::Type::ArrayType(array) = &ty {
        let Some(element) = array.ty() else {
            return Ok(None);
        };
        if !matches!(&element, ast::Type::PathType(_) | ast::Type::TupleType(_)) {
            return Ok(None);
        }
        let Some(HintRecipe::Primitive(element)) =
            annotation_hint(parsed, Some(element), definition)?
        else {
            return Ok(None);
        };
        let Some(ast::Expr::Literal(length)) = array.const_arg().and_then(|arg| arg.expr()) else {
            return Ok(None);
        };
        let Some(token) = length
            .syntax()
            .children_with_tokens()
            .filter_map(|item| item.into_token())
            .find(|token| token.kind() == SyntaxKind::INT_NUMBER)
        else {
            return Ok(None);
        };
        let Some(number) = ast::IntNumber::cast(token) else {
            return Ok(None);
        };
        if !matches!(number.suffix(), None | Some("usize")) {
            return Ok(None);
        }
        let Some(len) = number.value().ok().and_then(|len| u32::try_from(len).ok()) else {
            return Ok(None);
        };
        return Ok(Some(HintRecipe::Written(std::sync::Arc::new(
            crate::ir::written_type::WrittenType::Array(
                Box::new(crate::ir::written_type::WrittenType::Primitive(element)),
                len,
            ),
        ))));
    }
    if let ast::Type::RefType(reference) = &ty {
        let Some(pointee) = reference.ty() else {
            return Ok(None);
        };
        if matches!(pointee, ast::Type::RefType(_)) {
            return Ok(None);
        }
        let Some(HintRecipe::Primitive(target)) =
            annotation_hint(parsed, Some(pointee), definition)?
        else {
            return Ok(None);
        };
        let region = match reference.lifetime() {
            Some(lifetime) if lifetime.syntax().text() == "'static" => {
                crate::traits::term::RegionTerm::Static
            }
            Some(lifetime) if lifetime.syntax().text() != "'_" => return Ok(None),
            _ => {
                let range = reference.syntax().text_range();
                crate::traits::term::RegionTerm::Local {
                    definition,
                    start: u32::from(range.start()),
                    end: u32::from(range.end()),
                }
            }
        };
        return Ok(Some(HintRecipe::Written(std::sync::Arc::new(
            crate::ir::written_type::WrittenType::Ref {
                region,
                mutable: reference.mut_token().is_some(),
                target: Box::new(crate::ir::written_type::WrittenType::Primitive(target)),
            },
        ))));
    }
    let wanted = MacroName::parse(&ty.syntax().text().to_string(), parsed.edition).ok();
    // An annotation inside a body must not ignore a hoisted local type declaration.
    if let Some(wanted) = &wanted {
        for list in ty.syntax().ancestors().filter_map(ast::StmtList::cast) {
            for statement in list.statements() {
                let declared = match statement {
                    ast::Stmt::Item(ast::Item::TypeAlias(item)) => item.name(),
                    ast::Stmt::Item(ast::Item::Struct(item)) => item.name(),
                    ast::Stmt::Item(ast::Item::Enum(item)) => item.name(),
                    ast::Stmt::Item(ast::Item::Union(item)) => item.name(),
                    ast::Stmt::Item(ast::Item::Trait(item)) => item.name(),
                    _ => None,
                };
                if declared.is_some_and(|name| {
                    MacroName::parse(name.text(), parsed.edition)
                        .is_ok_and(|local| local == *wanted)
                }) {
                    return Ok(None);
                }
            }
        }
    }
    if let Some(parameters) = ty
        .syntax()
        .ancestors()
        .find_map(ast::Fn::cast)
        .and_then(|function| function.generic_param_list())
    {
        for (ordinal, parameter) in parameters.generic_params().enumerate() {
            if let ast::GenericParam::TypeParam(parameter) = parameter
                && let Some(name) = parameter.name()
                && wanted.is_some()
                && MacroName::parse(name.text(), parsed.edition).ok() == wanted
            {
                return Ok(Some(HintRecipe::Parameter(crate::traits::term::ParamKey {
                    definition,
                    ordinal: u32::try_from(ordinal)
                        .map_err(|_| error("annotation generic ordinal overflow"))?,
                })));
            }
        }
    }
    // An impl or trait member's inherited owner parameter, numbered after the
    // function's own exactly as the common signature capture numbers it.
    if let Some(wanted) = &wanted
        && wanted.as_str() != "Self"
        && let Some(function) = ty.syntax().ancestors().find_map(ast::Fn::cast)
    {
        let own = crate::syntax::signatures::capture_generics(function.generic_param_list())?;
        let own_count = own.len();
        let combined = crate::syntax::signatures::owner_generics(&function, own)?;
        if let Some(parameter) = combined.iter().skip(own_count).find(|parameter| {
            parameter.kind == crate::syntax::signatures::GenericKind::Type
                && parameter.name == wanted.as_str()
        }) {
            return Ok(Some(HintRecipe::Parameter(crate::traits::term::ParamKey {
                definition,
                ordinal: parameter.ordinal,
            })));
        }
    }
    Ok(hint(Some(ty)))
}
/// The entered Type service's selection for this exact statement's annotation.
fn selected_annotation(
    statement: &ast::LetStmt,
    annotation: Option<&let_annotations::LetAnnotation>,
) -> Option<HintRecipe> {
    let annotation = annotation?;
    let ty = statement.ty()?;
    (crate::syntax::ast_adapter::range(ty.syntax().text_range()) == annotation.range)
        .then(|| HintRecipe::Written(std::sync::Arc::clone(&annotation.written)))
}
fn compound_local_primitive(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    path: &ast::PathExpr,
    admitted: Option<&crate::frontend::binding_constraints::PreparedBindingConstraints>,
) -> Result<Option<(NodeId, PrimitiveType)>, Diagnostic> {
    let Some(binding) = local_binding(parsed, index, body, path)? else {
        return Ok(None);
    };
    // An inferred numeric local's Type is the common source-constraint
    // result for its own declaring `let`, admitted by the thunk provider.
    if let Some(proof) = admitted
        && let Some(ty) = proof.local_primitive(index, binding)?
    {
        return Ok(Some((binding, ty)));
    }
    // The binding's own written pattern at its retained indexed location: a
    // whole `let`, `if let`/`while let` or match-arm identifier pattern takes
    // its written annotation or its initializer/scrutinee's literal suffix.
    let Some(pattern) = parsed
        .entered_node_syntax(body, index, binding)?
        .filter(|node| ast::IdentPat::can_cast(node.kind()))
    else {
        return Ok(None);
    };
    let owner = pattern.parent();
    let (annotation, initializer) =
        if let Some(statement) = owner.clone().and_then(ast::LetStmt::cast) {
            (statement.ty(), statement.initializer())
        } else if let Some(condition) = owner.clone().and_then(ast::LetExpr::cast) {
            (None, condition.expr())
        } else if let Some(arm) = owner.and_then(ast::MatchArm::cast) {
            (
                None,
                arm.syntax()
                    .ancestors()
                    .find_map(ast::MatchExpr::cast)
                    .and_then(|expression| expression.expr()),
            )
        } else {
            return Ok(None);
        };
    let ty = if let Some(annotation) = annotation {
        let Some(HintRecipe::Primitive(ty)) =
            annotation_hint(parsed, Some(annotation), body.def_key)?
        else {
            return Ok(None);
        };
        ty
    } else {
        let Some(initializer) = initializer else {
            return Ok(None);
        };
        // A suffix belongs to the literal syntax even when an identically
        // spelled type name is shadowed. Use this exact binding's source;
        // neither a runtime descriptor nor a later use supplies it.
        let Some(Some(ty)) = runtime_local_numeric_context::literal_suffix(initializer)? else {
            return Ok(None);
        };
        ty
    };
    Ok(Some((binding, ty)))
}
fn compound_local_definition_context(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    path: &ast::PathExpr,
) -> Result<Option<(NodeId, DefinitionNumericContext)>, Diagnostic> {
    let Some(binding) = local_binding(parsed, index, body, path)? else {
        return Ok(None);
    };
    runtime_local_numeric_context::binding_context(parsed, index, body, binding)
        .map(|context| context.map(|context| (binding, context)))
}
/// Retain the actual outer shared borrow; nested or modified borrows are not
/// primitive forwarding. This never reads or executes the borrowed expression.
/// A constant expression rustc promotes under a shared borrow
/// (`rustc_const_eval`'s promotion of rvalues): literals, and parentheses,
/// negation, arrays (listed or repeated by a literal count), tuples and
/// record literals of them. No call or path appears, so evaluating it is
/// pure and one shared `'static` location is unobservable; a record's
/// declaration may still have drop glue, which `Frame::promoted_place`
/// checks on the completed value.
fn promotable_constant(expression: &ast::Expr) -> bool {
    match expression {
        ast::Expr::Literal(_) => true,
        ast::Expr::ParenExpr(inner) => inner
            .expr()
            .is_some_and(|inner| promotable_constant(&inner)),
        ast::Expr::PrefixExpr(prefix) => {
            matches!(
                prefix.op_kind(),
                Some(ast::UnaryOp::Neg | ast::UnaryOp::Not)
            ) && prefix
                .expr()
                .is_some_and(|inner| promotable_constant(&inner))
        }
        ast::Expr::TupleExpr(tuple) => tuple.fields().all(|field| promotable_constant(&field)),
        // A record literal of promotable fields (`P { x: 3 }`); the runtime
        // refuses one with drop glue or interior mutability.
        ast::Expr::RecordExpr(record) => record.record_expr_field_list().is_some_and(|list| {
            list.spread().is_none()
                && list.fields().all(|field| {
                    field
                        .expr()
                        .is_some_and(|value| promotable_constant(&value))
                })
        }),
        ast::Expr::ArrayExpr(array) => match array.kind() {
            ast::ArrayExprKind::ElementList(mut elements) => {
                elements.all(|element| promotable_constant(&element))
            }
            ast::ArrayExprKind::Repeat {
                initializer,
                repeat,
            } => {
                initializer.is_some_and(|initializer| promotable_constant(&initializer))
                    && matches!(repeat, Some(ast::Expr::Literal(_)))
            }
        },
        _ => false,
    }
}
/// A temporary of a `const`/`static` initializer whose lifetime extends to
/// the item's own root scope: the item's value borrows it, and rustc gives it
/// `'static` storage (const evaluation interns the final value's
/// allocations; nothing drops them).
fn extended_to_initializer_root(index: &BodyIndex, operand: NodeId) -> bool {
    index
        .scopes()
        .first()
        .is_some_and(|scope| scope.kind == super::body_index::ScopeKind::Initializer)
        && index.nodes()[operand.0 as usize]
            .extended_to
            .as_ref()
            .is_some_and(|target| target.scope == super::body_index::ScopeId(0))
}
fn compound_shared_rhs(mut expression: ast::Expr) -> Result<Option<ast::RefExpr>, Diagnostic> {
    for _ in 0usize.. {
        if expression
            .syntax()
            .children()
            .any(|n| ast::Attr::can_cast(n.kind()))
        {
            return Ok(None);
        }
        match expression {
            ast::Expr::ParenExpr(group) => {
                let Some(inner) = group.expr() else {
                    return Ok(None);
                };
                expression = inner;
            }
            ast::Expr::RefExpr(reference)
                if reference.mut_token().is_none()
                    && reference.raw_token().is_none()
                    && reference.const_token().is_none() =>
            {
                return Ok(Some(reference));
            }
            _ => return Ok(None),
        }
    }
    Err(Diagnostic::new(
        "LRR-E-BODY-INDEX-LIMIT",
        "compound shared RHS source nesting budget".into(),
    ))
}
fn compound_rhs_primitive(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    mut expression: ast::Expr,
    context: PrimitiveType,
    admitted: Option<&crate::frontend::binding_constraints::PreparedBindingConstraints>,
) -> Result<Option<PrimitiveType>, Diagnostic> {
    if let Some(reference) = compound_shared_rhs(expression.clone())? {
        let Some(inner) = reference.expr() else {
            return Ok(None);
        };
        expression = inner;
    }
    for _ in 0usize.. {
        if expression
            .syntax()
            .children()
            .any(|n| ast::Attr::can_cast(n.kind()))
        {
            return Ok(None);
        }
        match expression {
            ast::Expr::ParenExpr(parenthesis) => {
                let Some(inner) = parenthesis.expr() else {
                    return Ok(None);
                };
                expression = inner;
            }
            ast::Expr::PrefixExpr(prefix)
                if prefix.op_token().is_some_and(|token| token.text() == "-") =>
            {
                let Some(inner) = prefix.expr() else {
                    return Ok(None);
                };
                expression = inner;
            }
            ast::Expr::PathExpr(path) => {
                return Ok(
                    compound_local_primitive(parsed, index, body, &path, admitted)?
                        .map(|(_, ty)| ty),
                );
            }
            ast::Expr::Literal(literal) => {
                let token = literal.token();
                let suffix = match token.kind() {
                    SyntaxKind::INT_NUMBER => ast::IntNumber::cast(token)
                        .and_then(|number| number.suffix().map(str::to_owned)),
                    SyntaxKind::FLOAT_NUMBER => ast::FloatNumber::cast(token)
                        .and_then(|number| number.suffix().map(str::to_owned)),
                    SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW => {
                        return Ok(Some(PrimitiveType::Bool));
                    }
                    _ => return Ok(None),
                };
                let ty = match suffix.as_deref() {
                    None => context,
                    Some("i8") => PrimitiveType::I8,
                    Some("i16") => PrimitiveType::I16,
                    Some("i32") => PrimitiveType::I32,
                    Some("i64") => PrimitiveType::I64,
                    Some("isize") => PrimitiveType::Isize,
                    Some("u8") => PrimitiveType::U8,
                    Some("u16") => PrimitiveType::U16,
                    Some("u32") => PrimitiveType::U32,
                    Some("u64") => PrimitiveType::U64,
                    Some("usize") => PrimitiveType::Usize,
                    Some("f32") => PrimitiveType::F32,
                    Some("f64") => PrimitiveType::F64,
                    _ => return Ok(None),
                };
                return Ok(Some(ty));
            }
            _ => return Ok(None),
        }
    }
    Err(Diagnostic::new(
        "LRR-E-BODY-INDEX-LIMIT",
        "compound source view nesting budget".into(),
    ))
}
fn successor(
    index: &BodyIndex,
    body: &BodyHandle,
    syntax: &SyntaxNode,
) -> Result<PcKey, Diagnostic> {
    let list = syntax
        .parent()
        .and_then(ast::StmtList::cast)
        .ok_or_else(|| error("statement phase lacks its block list"))?;
    let next = list
        .syntax()
        .children()
        .filter(|node| ast::Stmt::can_cast(node.kind()) || ast::Expr::can_cast(node.kind()))
        .find(|node| node.text_range().start() >= syntax.text_range().end());
    if let Some(next) = next {
        if ast::Expr::can_cast(next.kind()) {
            let block = list
                .syntax()
                .parent()
                .ok_or_else(|| error("tail continuation lacks block"))?;
            let owner = id_for(index, body, &block)?;
            if index.nodes()[owner.0 as usize].tail_scope.is_some() {
                return index.pc(owner, Phase::Exit);
            }
        }
        return index.pc(id_for(index, body, &next)?, Phase::Entry);
    }
    let block = list
        .syntax()
        .parent()
        .ok_or_else(|| error("statement list lacks block"))?;
    index.pc(id_for(index, body, &block)?, Phase::Commit)
}
/// Pure original-source initializer boundary. A nested expression cannot claim
/// the root result or its final PC merely by sharing a type or source spelling.
fn initializer_result(
    index: &BodyIndex,
    body: &BodyHandle,
    syntax: &SyntaxNode,
) -> Result<Option<crate::ir::pc::SlotId>, Diagnostic> {
    if super::ast_adapter::range(syntax.text_range()) != body.range
        || ast::BlockExpr::can_cast(syntax.kind())
        || !syntax
            .parent()
            .is_some_and(|parent| super::body_index::is_initializer_owner(&parent))
    {
        return Ok(None);
    }
    if index.body() != &body.key()
        || index
            .scopes()
            .first()
            .is_none_or(|scope| scope.kind != super::body_index::ScopeKind::Initializer)
    {
        return Err(error("initializer continuation body/scope differs"));
    }
    let root = index
        .nodes()
        .first()
        .ok_or_else(|| error("initializer continuation root absent"))?;
    if root.id != NodeId(0)
        || root.range.start != 0
        || Some(root.range.end) != body.range.end.checked_sub(body.range.start)
    {
        return Err(error("initializer continuation range differs"));
    }
    root.value
        .map(Some)
        .ok_or_else(|| error("initializer result slot absent"))
}

pub(super) fn return_pc(
    index: &BodyIndex,
    body: &BodyHandle,
    syntax: &SyntaxNode,
) -> Result<PcKey, Diagnostic> {
    if initializer_result(index, body, syntax)?.is_some() {
        return index
            .initializer_return_pc()?
            .ok_or_else(|| error("initializer return PC absent"));
    }
    let parent = syntax
        .parent()
        .ok_or_else(|| error("expression has no structural parent"))?;
    if ast::LetStmt::can_cast(parent.kind()) || ast::ExprStmt::can_cast(parent.kind()) {
        return index.pc(id_for(index, body, &parent)?, Phase::Commit);
    }
    if let Some(list) = ast::StmtList::cast(parent) {
        let block = list
            .syntax()
            .parent()
            .ok_or_else(|| error("tail list has no block"))?;
        return index.pc(id_for(index, body, &block)?, Phase::Commit);
    }
    let id = id_for(index, body, syntax)?;
    let parent = index
        .nodes()
        .iter()
        .find(|node| node.operands.contains(&id))
        .ok_or_else(|| error("expression continuation absent"))?;
    let n = u16::try_from(
        parent
            .operands
            .iter()
            .position(|node| *node == id)
            .expect("checked operand"),
    )
    .map_err(|_| error("operand continuation overflow"))?;
    index.pc(parent.id, Phase::Operand(n))
}
/// Intrinsic location contexts enter a path's place, never its owning value.
/// Parenthesized/computed receivers require their own checked continuations.
/// Whether `syntax` (through parentheses) is the left operand of a plain or
/// compound assignment.
fn compound_assignment_lhs(syntax: &SyntaxNode) -> bool {
    let mut current = syntax.clone();
    while let Some(parent) = current.parent() {
        if ast::ParenExpr::can_cast(parent.kind()) {
            current = parent;
            continue;
        }
        return ast::BinExpr::cast(parent).is_some_and(|binary| {
            binary.op_token().is_some_and(|token| {
                matches!(
                    token.text(),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "<<=" | ">>=" | "&=" | "|=" | "^="
                )
            }) && binary.lhs().is_some_and(|left| left.syntax() == &current)
        });
    }
    false
}
/// A comparison operator's direct operand, or a direct `format_args!`
/// argument, that is itself a place expression (a path, field, index or
/// dereference). rustc autorefs comparison operands (`a == b` is
/// `PartialEq::eq(&a, &b)`) and its FormatArgs lowering borrows every
/// argument (`Argument::new_display(&x)`): such an operand is evaluated as
/// its place, never moved; `Op::ComparisonOperand` then reads a builtin
/// value or borrows it. Other operands are value expressions whose
/// temporaries the operation borrows in place.
fn comparison_place_operand(syntax: &SyntaxNode) -> bool {
    let Some(parent) = syntax.parent() else {
        return false;
    };
    let autoref = match ast::BinExpr::cast(parent.clone()) {
        Some(binary) => {
            binary
                .op_token()
                .is_some_and(|token| matches!(token.text(), "==" | "!=" | "<" | "<=" | ">" | ">="))
                && (binary.lhs().is_some_and(|left| left.syntax() == syntax)
                    || binary.rhs().is_some_and(|right| right.syntax() == syntax))
        }
        None => ast::FormatArgsArg::can_cast(parent.kind()),
    };
    autoref
        && ast::Expr::cast(syntax.clone()).is_some_and(|operand| match operand {
            ast::Expr::PathExpr(_) | ast::Expr::FieldExpr(_) | ast::Expr::IndexExpr(_) => true,
            ast::Expr::PrefixExpr(prefix) => {
                prefix.op_token().is_some_and(|token| token.text() == "*")
            }
            _ => false,
        })
}
fn place_context(syntax: &SyntaxNode) -> bool {
    if comparison_place_operand(syntax) {
        return true;
    }
    let mut current = syntax.clone();
    loop {
        let Some(parent) = current.parent() else {
            return false;
        };
        if ast::ParenExpr::can_cast(parent.kind()) {
            current = parent;
            continue;
        }
        if let Some(prefix) = ast::PrefixExpr::cast(parent.clone()) {
            return prefix.op_token().is_some_and(|token| token.text() == "*");
        }
        if let Some(field) = ast::FieldExpr::cast(parent.clone()) {
            return field.expr().is_some_and(|base| base.syntax() == &current);
        }
        if let Some(index) = ast::IndexExpr::cast(parent.clone()) {
            return index.base().is_some_and(|base| base.syntax() == &current);
        }
        if let Some(reference) = ast::RefExpr::cast(parent.clone()) {
            return reference
                .expr()
                .is_some_and(|operand| operand.syntax() == &current);
        }
        // The left operand of a compound assignment is the place it updates.
        if let Some(binary) = ast::BinExpr::cast(parent.clone())
            && binary.op_token().is_some_and(|token| {
                matches!(
                    token.text(),
                    "+=" | "-=" | "*=" | "/=" | "%=" | "<<=" | ">>=" | "&=" | "|=" | "^="
                )
            })
        {
            // A path LHS keeps the local compound route (read/write binding).
            return binary.lhs().is_some_and(|left| {
                left.syntax() == &current && !matches!(left, ast::Expr::PathExpr(_))
            });
        }
        // A value callee is a place: the call borrows or consumes it through
        // its callable type, like a method receiver.
        if let Some(call) = ast::CallExpr::cast(parent.clone()) {
            // Only an intrinsic place callee is addressed in place; any other
            // callee value gets a temporary place at the call.
            return call.expr().is_some_and(|callee| {
                callee.syntax() == &current && intrinsic_place_receiver(&callee)
            });
        }
        // A method receiver is a place: autoref borrows it, by-value reads it.
        if let Some(call) = ast::MethodCallExpr::cast(parent.clone()) {
            return call
                .receiver()
                .is_some_and(|receiver| receiver.syntax() == &current);
        }
        if let Some(statement) = ast::LetStmt::cast(parent.clone()) {
            return statement
                .initializer()
                .is_some_and(|initializer| initializer.syntax() == &current)
                && (matches!(statement.pat(), Some(ast::Pat::IdentPat(pattern))
                    if pattern.ref_token().is_some() && pattern.at_token().is_none())
                    || super::body_index::pattern_statement(&statement));
        }
        // Record update reads only omitted fields from the original base.
        if let Some(list) = ast::RecordExprFieldList::cast(parent.clone()) {
            return list.spread().is_some_and(|base| base.syntax() == &current);
        }
        // A pattern scrutinee is a place expression: the shared transaction
        // tests it in place and moves/borrows only the bound parts.
        if let Some(expression) = ast::MatchExpr::cast(parent.clone()) {
            return expression
                .expr()
                .is_some_and(|scrutinee| scrutinee.syntax() == &current);
        }
        if let Some(expression) = ast::LetExpr::cast(parent.clone()) {
            return expression
                .expr()
                .is_some_and(|scrutinee| scrutinee.syntax() == &current);
        }
        if let Some(assignment) = ast::BinExpr::cast(parent) {
            return assignment
                .op_token()
                .is_some_and(|token| token.text() == "=")
                && assignment.lhs().is_some_and(|lhs| lhs.syntax() == &current);
        }
        return false;
    }
}
pub(super) fn intrinsic_place_receiver(expr: &ast::Expr) -> bool {
    let mut current = expr.clone();
    loop {
        match &current {
            ast::Expr::ParenExpr(paren) => {
                let Some(child) = paren.expr() else {
                    return false;
                };
                current = child;
            }
            ast::Expr::PathExpr(_) | ast::Expr::FieldExpr(_) | ast::Expr::IndexExpr(_) => {
                return true;
            }
            ast::Expr::PrefixExpr(prefix) => {
                return prefix.op_token().is_some_and(|token| token.text() == "*");
            }
            _ => return false,
        }
    }
}

/// Whether a non-local path operand names a value item rather than one of
/// the executing frame's parameters (which publishes its own place).
fn value_path_operand(parsed: &ParsedFile, path: &ast::PathExpr) -> Result<bool, Diagnostic> {
    let Some(source_path) = path.path() else {
        return Ok(false);
    };
    if source_path.qualifier().is_some() {
        return Ok(true);
    }
    let Some(segment) = source_path.segment() else {
        return Ok(false);
    };
    let Some(ast::PathSegmentKind::Name(reference)) = segment.kind() else {
        return Ok(false);
    };
    let Ok(name) = MacroName::parse(
        &reference.text(),
        parsed.token_edition(super::ast_adapter::range(reference.syntax().text_range()))?,
    ) else {
        return Ok(false);
    };
    Ok(parameter_at(parsed, path.syntax(), &name)?.is_none())
}
fn local_binding(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    path: &ast::PathExpr,
) -> Result<Option<NodeId>, Diagnostic> {
    let Some(source_path) = path.path() else {
        return Ok(None);
    };
    if source_path.qualifier().is_some() {
        return Ok(None);
    }
    let Some(reference) = source_path.segment().and_then(|segment| segment.name_ref()) else {
        return Ok(None);
    };
    let Ok(name) = MacroName::parse(
        reference.text(),
        parsed.token_edition(super::ast_adapter::range(reference.syntax().text_range()))?,
    ) else {
        return Ok(None);
    };
    local_binding_at(parsed, index, body, path.syntax(), &name)
}
/// Pure statement attribute preprocessing in the captured owning cfg world.
fn statement_cfg(
    syntax: &SyntaxNode,
    cfg: &crate::source::snapshot::CfgSet,
) -> Result<(bool, bool), Diagnostic> {
    let mut metas = Vec::new();
    for attribute in syntax.children().filter_map(ast::Attr::cast) {
        let meta = attribute
            .meta()
            .ok_or_else(|| error("statement attribute syntax absent"))?;
        metas
            .try_reserve(1)
            .map_err(|_| error("statement attribute allocation limit"))?;
        metas.push(meta);
    }
    let (enabled, remaining) = super::attributes::effective_metas(metas, cfg)?;
    Ok((
        enabled,
        remaining.iter().any(|meta| {
            if matches!(meta, ast::Meta::CfgMeta(_)) {
                return false;
            }
            // Builtin inert and tool attributes (`#[allow(..)]`, `#[rustfmt::skip]`)
            // never transform the statement or expression they annotate.
            let path = meta
                .path()
                .map(|path| path.syntax().text().to_string())
                .unwrap_or_default();
            !(super::outline::is_builtin_inert_attribute(&path)
                || super::outline::is_tool_attribute(&path))
        }),
    ))
}
/// A lexical spelling is only a candidate. For retained/generated syntax the
/// actual NameRef and binding Name must also share the pinned local-name class.
fn binding_name_matches(
    parsed: &ParsedFile,
    reference: &SyntaxNode,
    wanted: &MacroName,
    candidate: &ast::Name,
) -> Result<bool, Diagnostic> {
    let binding_range = super::ast_adapter::range(candidate.syntax().text_range());
    if !MacroName::parse(candidate.text(), parsed.token_edition(binding_range)?)
        .is_ok_and(|name| &name == wanted)
    {
        return Ok(false);
    }
    if parsed.generated_origin.is_none() {
        return Ok(true);
    }
    let reference = ast::PathExpr::cast(reference.clone())
        .and_then(|path| path.path())
        .filter(|path| path.qualifier().is_none())
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref())
        .ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "lexical binding requires its actual hygienic identifier occurrence".into(),
            )
        })?;
    let reference_range = super::ast_adapter::range(reference.syntax().text_range());
    let actual = MacroName::parse(reference.text(), parsed.token_edition(reference_range)?)?;
    if &actual != wanted {
        return Err(error(
            "lexical reference spelling differs from its source occurrence",
        ));
    }
    parsed.same_local_name_context(reference_range, binding_range)
}
/// Select a retained syntactic candidate. Namespace admission and actual live
/// binding publication remain with the reached shared pattern transaction.
fn pattern_binding_at(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    reference: &SyntaxNode,
    name: &MacroName,
    pattern: &ast::Pat,
) -> Result<Option<NodeId>, Diagnostic> {
    let root = id_for(index, body, pattern.syntax())?;
    pattern_binding_group_at(parsed, index, reference, name, root)
}
fn pattern_binding_group_at(
    parsed: &ParsedFile,
    index: &BodyIndex,
    reference: &SyntaxNode,
    name: &MacroName,
    root: NodeId,
) -> Result<Option<NodeId>, Diagnostic> {
    for group in index.pattern_bindings().groups(root) {
        if group.name() != name.as_str() {
            continue;
        }
        let candidate = parsed
            .syntax_at(group.canonical_name_range(), |node| {
                ast::Name::can_cast(node.kind())
            })?
            .and_then(ast::Name::cast)
            .ok_or_else(|| error("retained pattern candidate name attachment absent"))?;
        if binding_name_matches(parsed, reference, name, &candidate)? {
            return group.canonical_candidate().map(Some).ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "pattern binding candidates have unresolved alternative, mode or hygiene facts"
                        .into(),
                )
            });
        }
    }
    Ok(None)
}
fn local_binding_at(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    syntax: &SyntaxNode,
    name: &MacroName,
) -> Result<Option<NodeId>, Diagnostic> {
    for ancestor in syntax.ancestors() {
        let range = ancestor.text_range();
        // A pattern function's parameter patterns bind for its whole block
        // (E-R5-010), shadowed by any inner binding found before this point.
        if let Some(function) = ast::Fn::cast(ancestor.clone())
            && let Some(block) = function.body()
            && block
                .syntax()
                .text_range()
                .contains_range(syntax.text_range())
            && let Some(list) = super::fn_parameters::pattern_list(block.syntax())
            && super::fn_parameters::body_range(block.syntax()) == body.range
        {
            for parameter in list.params() {
                if let Some(pattern) = parameter.pat()
                    && let Some(candidate) =
                        pattern_binding_at(parsed, index, body, syntax, name, &pattern)?
                {
                    return Ok(Some(candidate));
                }
            }
            break;
        }
        if u32::from(range.start()) < body.range.start
            || u32::from(range.end()) > body.range.end
            || ast::Fn::can_cast(ancestor.kind())
        {
            break;
        }
        // A closure parameter is a binding of the closure's own frame,
        // bound from its call argument at the closure's entry; it shadows
        // every enclosing binding of its name.
        if let Some(closure) = ast::ClosureExpr::cast(ancestor.clone())
            && closure.body().is_some_and(|body| {
                body.syntax()
                    .text_range()
                    .contains_range(syntax.text_range())
            })
            && let Some(parameters) = closure.param_list()
        {
            for parameter in parameters.params() {
                if let Some(pattern) = parameter.pat()
                    && let Some(candidate) =
                        pattern_binding_at(parsed, index, body, syntax, name, &pattern)?
                {
                    return Ok(Some(candidate));
                }
            }
        }
        if let Some(list) = ast::StmtList::cast(ancestor.clone()) {
            let mut binding = None;
            for statement in list.statements() {
                if let ast::Stmt::LetStmt(statement) = statement {
                    if !statement_cfg(statement.syntax(), index.cfg())?.0 {
                        continue;
                    }
                    if statement.syntax().text_range().end() > syntax.text_range().start() {
                        continue;
                    }
                    if let Some(pattern) = statement.pat()
                        && let Some(candidate) =
                            pattern_binding_at(parsed, index, body, syntax, name, &pattern)?
                    {
                        binding = Some(candidate);
                    }
                }
            }
            if binding.is_some() {
                return Ok(binding);
            }
        }
        if let Some(arm) = ast::MatchArm::cast(ancestor.clone()) {
            let here = syntax.text_range();
            let in_guard = arm
                .guard()
                .and_then(|g| g.condition())
                .is_some_and(|guard| guard.syntax().text_range().contains_range(here));
            let in_result = arm
                .expr()
                .is_some_and(|result| result.syntax().text_range().contains_range(here));
            if (in_guard || in_result)
                && let Some(pattern) = arm.pat()
                && let Some(candidate) =
                    pattern_binding_at(parsed, index, body, syntax, name, &pattern)?
            {
                // This is the exact lexical binding location, not a proof that
                // the pattern name denotes a binding. The reached pattern must
                // obtain that private namespace admission before entering either
                // continuation; ReadLocal still requires its actual live cell.
                return Ok(Some(candidate));
            }
        }
        // A `for` pattern binds in its loop body only.
        if let Some(expression) = ast::ForExpr::cast(ancestor.clone())
            && expression.loop_body().is_some_and(|body| {
                body.syntax()
                    .text_range()
                    .contains_range(syntax.text_range())
            })
            && let Some(pattern) = expression.pat()
            && let Some(candidate) =
                pattern_binding_at(parsed, index, body, syntax, name, &pattern)?
        {
            return Ok(Some(candidate));
        }
        let (condition, consequent) = if let Some(expression) = ast::IfExpr::cast(ancestor.clone())
        {
            (expression.condition(), expression.then_branch())
        } else if let Some(expression) = ast::WhileExpr::cast(ancestor.clone()) {
            (expression.condition(), expression.loop_body())
        } else {
            continue;
        };
        let Some(condition) = condition else { continue };
        let here = syntax.text_range();
        let in_condition = condition.syntax().text_range().contains_range(here);
        let in_consequent =
            consequent.is_some_and(|branch| branch.syntax().text_range().contains_range(here));
        if !in_condition && !in_consequent {
            continue;
        }
        let plan = parsed.condition_plan(
            body,
            index,
            id_for(index, body, &ancestor)?,
            Default::default(),
        )?;
        let mut binding = None;
        for clause in plan.clauses() {
            let super::conditions::ConditionClause::Pattern { node, pattern, .. } = clause else {
                continue;
            };
            if in_condition {
                let end = body
                    .range
                    .start
                    .checked_add(index.nodes()[node.0 as usize].range.end)
                    .ok_or_else(|| error("condition binding range overflow"))?;
                if end > u32::from(here.start()) {
                    continue;
                }
            }
            if let Some(candidate) =
                pattern_binding_group_at(parsed, index, syntax, name, *pattern)?
            {
                binding = Some(candidate);
            }
        }
        if binding.is_some() {
            return Ok(binding);
        }
    }
    Ok(None)
}
fn parameter_at(
    parsed: &ParsedFile,
    syntax: &SyntaxNode,
    name: &MacroName,
) -> Result<Option<u32>, Diagnostic> {
    parameter_from(parsed, syntax, syntax, name)
}
/// `parameter_at` for the name written at `reference`, as seen from the
/// frame executing `position` (the reference, or a closure creating it).
fn parameter_from(
    parsed: &ParsedFile,
    position: &SyntaxNode,
    reference: &SyntaxNode,
    name: &MacroName,
) -> Result<Option<u32>, Diagnostic> {
    let syntax = position;
    // Inside a closure its own parameters are bindings; a parameter of the
    // frame creating the closure is captured into the closure frame's
    // parameters after its arguments, at that creating frame's ordinal.
    if let Some(closure) = closure_thunk::enclosing_closure(syntax) {
        if closure_thunk::closure_parameter(parsed, &closure, reference, name)?.is_some() {
            return Ok(None);
        }
        let arity = closure.param_list().map_or(0, |list| list.params().count());
        return match parameter_from(parsed, closure.syntax(), reference, name)? {
            Some(outer) => Ok(Some(
                u32::try_from(arity)
                    .ok()
                    .and_then(|arity| arity.checked_add(outer))
                    .ok_or_else(|| error("closure parameter capture ordinal overflow"))?,
            )),
            None => Ok(None),
        };
    }
    if let Some(params) = syntax
        .ancestors()
        .find_map(ast::Fn::cast)
        .and_then(|f| f.param_list())
    {
        // The receiver is parameter 0.
        if name.as_str() == "self" && params.self_param().is_some() {
            return Ok(Some(0));
        }
        // A pattern function binds every written parameter as a local at
        // entry (E-R5-010): none is read as a frame parameter.
        if super::fn_parameters::runs_patterns(&params) {
            return Ok(None);
        }
        for (ordinal, param) in params.params().enumerate() {
            if let Some(ast::Pat::IdentPat(pattern)) = param.pat()
                && pattern.ref_token().is_none()
                && pattern.at_token().is_none()
                && let Some(candidate) = pattern.name()
                && binding_name_matches(parsed, reference, name, &candidate)?
            {
                let ordinal = ordinal
                    .checked_add(usize::from(params.self_param().is_some()))
                    .ok_or_else(|| error("parameter ordinal overflow"))?;
                return Ok(Some(
                    u32::try_from(ordinal).map_err(|_| error("parameter ordinal overflow"))?,
                ));
            }
        }
    }
    Ok(None)
}
/// The lifetime token range of the label of the loop or block at `node`.
fn label_lifetime_range(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    node: NodeId,
) -> Result<Option<crate::syntax::spans::SourceRange>, Diagnostic> {
    let structural = &index.nodes()[node.0 as usize];
    let start = body
        .range
        .start
        .checked_add(structural.range.start)
        .ok_or_else(|| error("label source start overflow"))?;
    let end = body
        .range
        .start
        .checked_add(structural.range.end)
        .ok_or_else(|| error("label source end overflow"))?;
    let root = parsed.entered_body_root(body)?;
    let labeled = root
        .covering_element(ra_ap_syntax::TextRange::new(start.into(), end.into()))
        .ancestors()
        .find(|syntax| {
            let range = syntax.text_range();
            u32::from(range.start()) == start && u32::from(range.end()) == end
        });
    let lifetime = labeled.and_then(|syntax| match ast::Expr::cast(syntax)? {
        ast::Expr::LoopExpr(e) => e.label(),
        ast::Expr::WhileExpr(e) => e.label(),
        ast::Expr::ForExpr(e) => e.label(),
        ast::Expr::BlockExpr(e) => e.label(),
        _ => None,
    });
    Ok(lifetime
        .and_then(|label| label.lifetime())
        .map(|lifetime| super::ast_adapter::range(lifetime.syntax().text_range())))
}
fn control_target(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    node: &super::body_index::StructuralNode,
    lifetime: Option<ast::Lifetime>,
    continuing: bool,
) -> Result<Option<super::body_index::LabelId>, Diagnostic> {
    // Labels are hygienic like locals: the written label must share the
    // label occurrence's local-name class (a macro's own `'x` is not the
    // caller's `'x`).
    let wanted_range = lifetime
        .as_ref()
        .map(|l| super::ast_adapter::range(l.syntax().text_range()));
    let wanted = lifetime
        .and_then(|l| l.lifetime_ident_token())
        .map(|t| t.text().to_string());
    let canonical =
        |text: &str| MacroName::parse(text.trim_start_matches('\''), parsed.edition).ok();
    let mut target = node.enclosing_syntax_target;
    while let Some(id) = target {
        let label = index
            .labels()
            .get(id.0 as usize)
            .ok_or_else(|| error("control label ancestry absent"))?;
        let named = wanted.as_ref().map_or(label.is_loop, |wanted| {
            label
                .spelling
                .as_ref()
                .is_some_and(|s| canonical(s).is_some() && canonical(s) == canonical(wanted))
        });
        let hygienic = named
            && match (wanted_range, wanted.is_some()) {
                (Some(wanted_range), true) => {
                    match label_lifetime_range(parsed, index, body, label.node)? {
                        Some(label_range) => {
                            parsed.same_local_name_context(wanted_range, label_range)?
                        }
                        None => true,
                    }
                }
                _ => true,
            };
        if hygienic {
            return Ok((!continuing || label.is_loop).then_some(id));
        }
        target = label.parent;
    }
    Ok(None)
}
pub(super) fn literal_recipe(
    literal: &ast::Literal,
    source: crate::syntax::spans::BodySpan,
) -> Result<LiteralRecipe, Diagnostic> {
    let token = literal.token();
    let (kind, decoded) = match token.kind() {
        SyntaxKind::INT_NUMBER => {
            let number =
                ast::IntNumber::cast(token).ok_or_else(|| error("integer token cast failed"))?;
            match number.suffix() {
                Some("f32" | "f64") if number.radix() == ast::Radix::Decimal => {
                    let suffix = if number.suffix() == Some("f32") {
                        PrimitiveType::F32
                    } else {
                        PrimitiveType::F64
                    };
                    (
                        LiteralKind::Float,
                        Some(DecodedLiteral::Float {
                            decimal: number.value_string(),
                            suffix: Some(suffix),
                        }),
                    )
                }
                _ => (
                    LiteralKind::Integer,
                    Some(super::literal_decode::integer(&number, false)),
                ),
            }
        }
        SyntaxKind::FLOAT_NUMBER => {
            let number =
                ast::FloatNumber::cast(token).ok_or_else(|| error("float token cast failed"))?;
            let suffix = match number.suffix() {
                None => None,
                Some("f32") => Some(PrimitiveType::F32),
                Some("f64") => Some(PrimitiveType::F64),
                _ => {
                    return Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "unsupported float suffix".into(),
                    ));
                }
            };
            (
                LiteralKind::Float,
                Some(DecodedLiteral::Float {
                    decimal: number.value_string(),
                    suffix,
                }),
            )
        }
        SyntaxKind::CHAR => {
            let token = ast::Char::cast(token).ok_or_else(|| error("char token cast failed"))?;
            let Ok(value) = token.value() else {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "invalid char escape".into(),
                ));
            };
            (LiteralKind::Char, Some(DecodedLiteral::Char(value)))
        }
        SyntaxKind::BYTE => {
            let token = ast::Byte::cast(token).ok_or_else(|| error("byte token cast failed"))?;
            let Ok(value) = token.value() else {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "invalid byte escape".into(),
                ));
            };
            (LiteralKind::Byte, Some(DecodedLiteral::Byte(value)))
        }
        SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW => (LiteralKind::Bool, None),
        SyntaxKind::STRING => (LiteralKind::String, None),
        SyntaxKind::BYTE_STRING => {
            let token = ast::ByteString::cast(token)
                .ok_or_else(|| error("byte string token cast failed"))?;
            let Ok(value) = token.value() else {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "invalid byte string escape".into(),
                ));
            };
            (
                LiteralKind::ByteString,
                Some(DecodedLiteral::ByteString(std::sync::Arc::from(
                    value.as_ref(),
                ))),
            )
        }
        _ => {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "literal token kind is outside the live profile".into(),
            ));
        }
    };
    Ok(LiteralRecipe {
        decoded,
        kind,
        spelling: literal.syntax().text().to_string(),
        source,
    })
}
pub(super) fn binary_operator(text: &str) -> Option<NumericBinary> {
    use NumericBinary as B;
    Some(match text {
        "+" => B::Add,
        "-" => B::Sub,
        "*" => B::Mul,
        "/" => B::Div,
        "%" => B::Rem,
        "<<" => B::Shl,
        ">>" => B::Shr,
        "&" => B::BitAnd,
        "|" => B::BitOr,
        "^" => B::BitXor,
        "==" => B::Eq,
        "!=" => B::Ne,
        "<" => B::Lt,
        "<=" => B::Le,
        ">" => B::Gt,
        ">=" => B::Ge,
        _ => return None,
    })
}
fn next_phase(
    index: &BodyIndex,
    body: &BodyHandle,
    syntax: &SyntaxNode,
    op: Op,
) -> Result<(Vec<Op>, PcKey), Diagnostic> {
    next_phase_operations(index, body, syntax, vec![op])
}
fn next_phase_operations(
    index: &BodyIndex,
    body: &BodyHandle,
    syntax: &SyntaxNode,
    mut ops: Vec<Op>,
) -> Result<(Vec<Op>, PcKey), Diagnostic> {
    let next = successor(index, body, syntax)?;
    if let Some(scope) = index.nodes()[id_for(index, body, syntax)?.0 as usize].statement_scope {
        ops.push(Op::ScopeLeave { scope });
    }
    if next.phase == Phase::Entry.code()?
        && next.node != 0
        && let Some(child) = index.nodes()[next.node as usize].value
    {
        ops.push(Op::NumericHint {
            parent: index.nodes()[id_for(
                index,
                body,
                &syntax
                    .parent()
                    .and_then(|list| list.parent())
                    .ok_or_else(|| error("statement block absent"))?,
            )?
            .0 as usize]
                .value
                .expect("containing block value"),
            child,
            left: None,
            operator: None,
        });
    }
    Ok((ops, next))
}
impl ParsedFile {
    fn enclosing_import_may_bind(
        &self,
        syntax: &SyntaxNode,
        name: &str,
        cfg: &crate::source::snapshot::CfgSet,
    ) -> Result<bool, Diagnostic> {
        let wanted = MacroName::parse(name, self.edition)?;
        let mut budget = super::imports::ImportBudget::default();
        let mut visited = 0usize;
        for list in syntax.ancestors().filter_map(ast::StmtList::cast) {
            for statement in list.statements() {
                visited = visited
                    .checked_add(1)
                    .ok_or_else(|| error("lexical import inspection work limit exceeded"))?;
                if !statement_cfg(statement.syntax(), cfg)?.0 {
                    continue;
                }
                if let ast::Stmt::Item(ast::Item::Use(item)) = statement
                    && super::imports::may_bind_name(&item, &wanted, self.edition, &mut budget)?
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub(crate) fn require_module_predicate_call(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        name: &str,
    ) -> Result<(), Diagnostic> {
        use ast::HasName;
        let unsupported = || {
            Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "assertion predicate requires lexical callable context integration".into(),
            )
        };
        if body.key() != key.body || *index.body() != key.body {
            return Err(error("assertion predicate source body differs"));
        }
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| error("assertion predicate macro absent"))?;
        let start = body
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| error("assertion predicate offset overflow"))?;
        let end = body
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| error("assertion predicate offset overflow"))?;
        let expression = self
            .syntax_at(
                crate::syntax::spans::SourceRange::checked(start, end)?,
                |syntax| ast::MacroExpr::can_cast(syntax.kind()),
            )?
            .and_then(ast::MacroExpr::cast)
            .ok_or_else(|| error("assertion predicate original macro differs"))?;
        let wanted = MacroName::parse(name, self.edition)?;
        if self.enclosing_import_may_bind(expression.syntax(), name, index.cfg())? {
            return Err(unsupported());
        }
        let matches = |name: ast::Name| {
            MacroName::parse(name.text(), self.edition).is_ok_and(|name| name == wanted)
        };
        for list in expression
            .syntax()
            .ancestors()
            .filter_map(ast::StmtList::cast)
        {
            for statement in list.statements() {
                match statement {
                    ast::Stmt::LetStmt(binding)
                        if u32::from(binding.syntax().text_range().end()) <= start =>
                    {
                        if binding.pat().is_some_and(|pat| {
                            pat.syntax()
                                .descendants()
                                .filter_map(ast::IdentPat::cast)
                                .filter_map(|p| p.name())
                                .any(&matches)
                        }) {
                            return Err(unsupported());
                        }
                    }
                    ast::Stmt::Item(item) => {
                        let declared = match item {
                            ast::Item::Fn(i) => i.name(),
                            ast::Item::Const(i) => i.name(),
                            ast::Item::Static(i) => i.name(),
                            ast::Item::Struct(i) => i.name(),
                            ast::Item::Enum(i) => i.name(),
                            ast::Item::Union(i) => i.name(),
                            _ => None,
                        };
                        if declared.is_some_and(matches) {
                            return Err(unsupported());
                        }
                    }
                    _ => {}
                }
            }
        }
        if expression
            .syntax()
            .ancestors()
            .find_map(ast::Fn::cast)
            .and_then(|f| f.param_list())
            .is_some_and(|parameters| {
                parameters.params().filter_map(|p| p.pat()).any(|pat| {
                    pat.syntax()
                        .descendants()
                        .filter_map(ast::IdentPat::cast)
                        .filter_map(|p| p.name())
                        .any(&matches)
                })
            })
        {
            return Err(unsupported());
        }
        Ok(())
    }
    pub(crate) fn require_runtime_macro_call(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        call: &super::runtime_macro_scalars::RuntimeMacroCall,
    ) -> Result<(), Diagnostic> {
        use super::runtime_macro_scalars::RuntimeMacroCallPhase;
        call.require(&body.key(), key, RuntimeMacroCallPhase::Prepare)?;
        if self.key() != *call.source()
            || body.key() != *index.body()
            || index.pc(NodeId(key.node), Phase::from_code(key.phase))? != *key
        {
            return Err(error("assertion call current source/body/PC differs"));
        }
        if self.generated_origin.is_some() {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "generated assertion call requires origin namespace integration".into(),
            ));
        }
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| error("assertion call macro node absent"))?;
        let invocation = call.invocation();
        let span = call.span();
        let start = body
            .range
            .start
            .checked_add(span.relative_start)
            .ok_or_else(|| error("assertion call source start overflow"))?;
        let end = body
            .range
            .start
            .checked_add(span.relative_end)
            .ok_or_else(|| error("assertion call source end overflow"))?;
        if node.role != NodeRole::Expression(ExpressionKind::Macro)
            || body.range.start.checked_add(node.range.start) != Some(invocation.start)
            || body.range.start.checked_add(node.range.end) != Some(invocation.end)
            || span.body_key != body.key()
            || start < invocation.start
            || end > invocation.end
            || start >= end
        {
            return Err(error("assertion call original source range differs"));
        }
        // The compiler-owned plan certifies the exact inner CallExpr. Lookup
        // still uses the real outer source scope and never synthetic ancestors.
        // Qualified/local type owners await the same entered namespace producer,
        // rather than selecting a module member from a printed name.
        if call.name().contains("::") {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "qualified assertion call requires source namespace integration".into(),
            ));
        }
        self.require_module_predicate_call(body, index, key, call.name())?;
        let original = self.parsed.tree();
        let macro_expression = original
            .syntax()
            .covering_element(ra_ap_syntax::TextRange::new(
                invocation.start.into(),
                invocation.end.into(),
            ))
            .ancestors()
            .filter_map(ast::MacroExpr::cast)
            .find(|expression| {
                super::ast_adapter::range(expression.syntax().text_range()) == invocation
            })
            .ok_or_else(|| error("assertion call owning macro absent"))?;
        let mut work = 0usize;
        for list in macro_expression
            .syntax()
            .ancestors()
            .filter_map(ast::StmtList::cast)
        {
            for statement in list.statements() {
                work = work
                    .checked_add(1)
                    .ok_or_else(|| error("assertion call lexical work limit"))?;
                let declared = match statement {
                    ast::Stmt::Item(ast::Item::TypeAlias(item)) => item.name(),
                    ast::Stmt::Item(ast::Item::Struct(item)) => item.name(),
                    ast::Stmt::Item(ast::Item::Enum(item)) => item.name(),
                    ast::Stmt::Item(ast::Item::Union(item)) => item.name(),
                    _ => None,
                };
                if declared.is_some_and(|name| {
                    call.explicit().iter().any(|argument| {
                        MacroName::parse(argument, self.edition)
                            .ok()
                            .is_some_and(|argument| {
                                MacroName::parse(name.text(), self.edition).ok() == Some(argument)
                            })
                    })
                }) {
                    return Err(Diagnostic::new("LRR-E-UNSUPPORTED", "assertion body-local explicit type requires lexical type namespace integration".into()));
                }
            }
        }
        Ok(())
    }
    pub(crate) fn runtime_macro_binding_read(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        name: &str,
        destination: crate::ir::pc::SlotId,
    ) -> Result<Op, Diagnostic> {
        self.runtime_macro_binding_operation(body, index, key, name, destination, false)
    }
    pub(crate) fn runtime_macro_binding_place(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        name: &str,
        destination: crate::ir::pc::SlotId,
    ) -> Result<Op, Diagnostic> {
        self.runtime_macro_binding_operation(body, index, key, name, destination, true)
    }
    fn runtime_macro_binding_operation(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        name: &str,
        destination: crate::ir::pc::SlotId,
        place: bool,
    ) -> Result<Op, Diagnostic> {
        if body.key() != key.body || *index.body() != key.body {
            return Err(error("runtime binding operand body differs"));
        }
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| error("runtime binding node absent"))?;
        let value = if place {
            crate::ir::pc::SlotId(
                destination
                    .0
                    .checked_sub(1)
                    .ok_or_else(|| error("runtime binding place companion absent"))?,
            )
        } else {
            destination
        };
        if node.role != NodeRole::Expression(super::body_index::ExpressionKind::Macro)
            || (node.runtime_temporaries[0] != Some(value)
                && !node.runtime_scalar_slots.contains(&value))
            || index.slot_kind(destination)?
                != if place {
                    crate::ir::pc::SlotKind::Place
                } else {
                    crate::ir::pc::SlotKind::Value
                }
        {
            return Err(error(
                "runtime binding operand requires exact macro temporary",
            ));
        }
        let start = body
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| error("runtime binding source overflow"))?;
        let end = body
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| error("runtime binding source overflow"))?;
        let expression = self
            .syntax_at(
                crate::syntax::spans::SourceRange::checked(start, end)?,
                |syntax| ast::MacroExpr::can_cast(syntax.kind()),
            )?
            .and_then(ast::MacroExpr::cast)
            .ok_or_else(|| error("runtime binding invocation source differs"))?;
        let name = MacroName::parse(name, self.edition)?;
        if let Some(binding) = local_binding_at(self, index, body, expression.syntax(), &name)? {
            return Ok(if place {
                Op::MakePlace {
                    binding,
                    destination,
                }
            } else {
                Op::ReadLocal {
                    binding,
                    destination,
                }
            });
        }
        if let Some(ordinal) = parameter_at(self, expression.syntax(), &name)? {
            return Ok(if place {
                Op::ParameterPlace {
                    ordinal,
                    destination,
                }
            } else {
                Op::ReadParameter {
                    ordinal,
                    destination,
                }
            });
        }
        Err(Diagnostic::new(
            "LRR-E-UNSUPPORTED",
            "assertion non-local binding awaits ordinary namespace resolution".into(),
        ))
    }
    pub(crate) fn runtime_macro_successor(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<PcKey, Diagnostic> {
        if body.key() != key.body || *index.body() != key.body {
            return Err(error("runtime macro continuation body differs"));
        }
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| error("runtime macro continuation node absent"))?;
        if node.role != NodeRole::Expression(super::body_index::ExpressionKind::Macro) {
            return Err(error(
                "runtime macro continuation requires macro expression",
            ));
        }
        let start = body
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| error("runtime macro continuation start overflow"))?;
        let end = body
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| error("runtime macro continuation end overflow"))?;
        let expression = self
            .syntax_at(
                crate::syntax::spans::SourceRange::checked(start, end)?,
                |syntax| ast::MacroExpr::can_cast(syntax.kind()),
            )?
            .and_then(ast::MacroExpr::cast)
            .ok_or_else(|| error("runtime macro continuation source differs"))?;
        return_pc(index, body, expression.syntax())
    }
    /// Revalidate and lower the reached final initializer phase. The real root
    /// result remains distinct from the mandatory administrative Unit operation.
    fn initializer_exit(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<Option<VerifiedRecipe>, Diagnostic> {
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| error("initializer phase node absent"))?;
        if node.role != NodeRole::InitializerReturn {
            return Ok(None);
        }
        if body.key() != *index.body()
            || key.body != body.key()
            || self.profile != *index.profile()
            || index.initializer_return_pc()?.as_ref() != Some(key)
        {
            return Err(error("initializer return body/profile/PC differs"));
        }
        let root = self.entered_body_root(body)?;
        let value = initializer_result(index, body, &root)?
            .ok_or_else(|| error("initializer return source boundary absent"))?;
        let unit = node.runtime_temporaries[0]
            .ok_or_else(|| error("initializer return Unit slot absent"))?;
        if unit == value {
            return Err(error("initializer return Unit aliases result"));
        }
        draft(
            index,
            key,
            vec![Op::Unit { destination: unit }],
            RecipeExit::Control(ControlRecipe::Return { value }),
        )
        .map(Some)
    }
    /// Lower only the reached constructor phase. Child expressions stay lazy;
    /// descriptor binding remains a dispatcher action before ConstructStart.
    pub(crate) fn constructor_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        plan: &super::constructors::ConstructorPlan,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        self.constructor_thunk_policy(body, index, key, plan, None)
    }
    pub(crate) fn constructor_thunk_with_repeat(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        plan: &super::constructors::ConstructorPlan,
        prepared: &crate::frontend::array_repeats::PreparedArrayRepeat,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        use crate::core::digest::StableEncode;
        if prepared.plan().key().body != key.body
            || prepared.plan().key().node != key.node
            || prepared.plan().syntax().fingerprint() != plan.fingerprint()
        {
            return Err(error("repeat prepared source identity differs"));
        }
        self.constructor_thunk_policy(body, index, key, plan, prepared.plan().written())
    }
    fn constructor_thunk_policy(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        plan: &super::constructors::ConstructorPlan,
        repeated: Option<&std::sync::Arc<crate::ir::written_type::WrittenType>>,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        use super::constructors::ConstructorKind;
        if body.key() != *index.body()
            || key.body != body.key()
            || plan.origin().body_key != body.key()
            || plan.node() != NodeId(key.node)
            || plan.profile() != index.profile()
            || plan.profile() != &self.profile
        {
            return Err(error("constructor thunk/source plan mismatch"));
        }
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| error("constructor node absent"))?;
        index.pc(node.id, Phase::from_code(key.phase))?;
        let guard = node
            .constructor_guard
            .ok_or_else(|| error("constructor guard absent"))?;
        let fallback = index.pc(node.id, Phase::Exit)?;
        if let ConstructorKind::Repeat {
            element,
            count,
            continuation,
            ..
        } = plan.kind()
        {
            return self.repeat_thunk(index, key, node, (element, count), continuation, repeated);
        }
        let start = body
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| error("constructor syntax range overflow"))?;
        let end = body
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| error("constructor syntax range overflow"))?;
        let syntax = self
            .syntax_at(
                crate::syntax::spans::SourceRange::checked(start, end)?,
                |syntax| ast::Expr::can_cast(syntax.kind()),
            )?
            .ok_or_else(|| error("constructor syntax boundary absent"))?;
        if syntax
            .children()
            .any(|child| ast::Attr::can_cast(child.kind()))
        {
            return thunks::unsupported(
                index,
                key.clone(),
                "entered statement/expression attributes require cfg/macro integration".into(),
                fallback,
            );
        }
        let next = |written: usize| -> Result<PcKey, Diagnostic> {
            if let Some(operand) = plan.operand(written) {
                index.pc(operand.node, Phase::Entry)
            } else {
                index.pc(node.id, Phase::Commit)
            }
        };
        match Phase::from_code(key.phase) {
            Phase::Entry => draft(
                index,
                key,
                vec![Op::ConstructStart { guard }],
                RecipeExit::Next(next(0)?),
            ),
            Phase::Operand(raw) => {
                let source_node = node
                    .operands
                    .get(usize::from(raw))
                    .ok_or_else(|| error("constructor operand phase absent"))?;
                let written = plan
                    .written_ordinal(*source_node)
                    .ok_or_else(|| error("constructor phase is not an enabled written field"))?;
                let operand = plan
                    .operand(written)
                    .ok_or_else(|| error("constructor selected operand absent"))?;
                let field = u32::try_from(written)
                    .map_err(|_| error("constructor field ordinal overflow"))?;
                let operations = if matches!(plan.kind(),
                    ConstructorKind::Record { fields, rest: Some(_), .. } if written == fields.len())
                {
                    let base = index.nodes()[source_node.0 as usize]
                        .place
                        .ok_or_else(|| error("record update base place absent"))?;
                    let mut ops = Vec::new();
                    if !plan.base_is_place() {
                        ops.push(Op::ValuePlace {
                            source: operand.value,
                            destination: base,
                        });
                    }
                    ops.push(Op::ConstructRest { guard, base });
                    ops
                } else {
                    vec![Op::ConstructField {
                        guard,
                        field,
                        source: operand.value,
                    }]
                };
                draft(index, key, operations, RecipeExit::Next(next(written + 1)?))
            }
            Phase::Commit => {
                let destination = node
                    .value
                    .ok_or_else(|| error("constructor result slot absent"))?;
                draft(
                    index,
                    key,
                    vec![Op::ConstructFinish { guard, destination }],
                    RecipeExit::Next(return_pc(index, body, &syntax)?),
                )
            }
            Phase::Exit => Err(error(
                "constructor exit/cleanup is not an executable construction phase",
            )),
        }
    }
    /// `[element; count]`: the element, then the count (a `usize` constant
    /// expression evaluated as ordinary syntax), then one `ArrayRepeat`.
    fn repeat_thunk(
        &self,
        index: &BodyIndex,
        key: &PcKey,
        node: &crate::syntax::body_index::StructuralNode,
        operands: (
            &super::constructors::ConstructorOperand,
            &super::constructors::ConstructorOperand,
        ),
        continuation: &PcKey,
        repeated: Option<&std::sync::Arc<crate::ir::written_type::WrittenType>>,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let (element, count) = operands;
        let (element_node, element_value) = (element.node, element.value);
        let (count_node, count) = (count.node, count.value);
        let destination = node.value.ok_or_else(|| error("repeat result absent"))?;
        if let Some(written) = repeated {
            let more =
                node.runtime_temporaries[0].ok_or_else(|| error("repeat branch test absent"))?;
            let guard = node
                .constructor_guard
                .ok_or_else(|| error("repeat constructor guard absent"))?;
            let element = index.pc(element_node, Phase::Entry)?;
            let finish = index.pc(node.id, Phase::Commit)?;
            let branch = RecipeExit::Branch {
                condition: more,
                yes: element,
                no: finish,
            };
            return match Phase::from_code(key.phase) {
                Phase::Entry => draft(
                    index,
                    key,
                    vec![
                        Op::SetHint {
                            child: count,
                            hint: HintRecipe::Primitive(PrimitiveType::Usize),
                        },
                        Op::SetHint {
                            child: element_value,
                            hint: HintRecipe::Written(std::sync::Arc::clone(written)),
                        },
                    ],
                    RecipeExit::Next(index.pc(count_node, Phase::Entry)?),
                ),
                Phase::Operand(n) if node.operands.get(usize::from(n)) == Some(&count_node) => {
                    draft(
                        index,
                        key,
                        vec![Op::ArrayRepeatElement {
                            guard,
                            source: element_value,
                            count,
                            more,
                            initialize: true,
                        }],
                        branch,
                    )
                }
                Phase::Operand(n) if node.operands.get(usize::from(n)) == Some(&element_node) => {
                    draft(
                        index,
                        key,
                        vec![
                            Op::Discard { source: more },
                            Op::ArrayRepeatElement {
                                guard,
                                source: element_value,
                                count,
                                more,
                                initialize: false,
                            },
                        ],
                        branch,
                    )
                }
                Phase::Commit => draft(
                    index,
                    key,
                    vec![
                        Op::Discard { source: more },
                        Op::ConstructFinish { guard, destination },
                    ],
                    RecipeExit::Next(continuation.clone()),
                ),
                _ => Err(error("constant repeat phase differs")),
            };
        }
        match Phase::from_code(key.phase) {
            Phase::Entry => draft(
                index,
                key,
                vec![Op::SetHint {
                    child: count,
                    hint: HintRecipe::Primitive(PrimitiveType::Usize),
                }],
                RecipeExit::Next(index.pc(element_node, Phase::Entry)?),
            ),
            Phase::Operand(n) if node.operands.get(usize::from(n)) == Some(&element_node) => draft(
                index,
                key,
                vec![],
                RecipeExit::Next(index.pc(count_node, Phase::Entry)?),
            ),
            Phase::Operand(n) if node.operands.get(usize::from(n)) == Some(&count_node) => draft(
                index,
                key,
                vec![Op::ArrayRepeat {
                    source: element_value,
                    count,
                    destination,
                }],
                RecipeExit::Next(continuation.clone()),
            ),
            _ => Err(error("invalid array repeat phase")),
        }
    }
    pub(crate) fn initial_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        self.initial_thunk_admission(body, index, key, EnteredAdmissions::default())
    }
    pub(crate) fn initial_thunk_with_closure_captures(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        captures: &[crate::ir::op::ClosureCapture],
    ) -> Result<VerifiedRecipe, Diagnostic> {
        self.initial_thunk_admission(body, index, key, EnteredAdmissions {
            closure_captures: Some(captures), ..Default::default()
        })
    }
    pub(crate) fn initial_thunk_with_value_goal(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        goal: &crate::frontend::source_value_goals::EnteredValueGoal,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        if goal.key() != key || goal.retained_plan().context().source().parsed().key() != self.key()
        {
            return Err(error("Value access source/PC differs"));
        }
        self.initial_thunk_admission(
            body,
            index,
            key,
            EnteredAdmissions {
                value_goal: Some(goal),
                ..Default::default()
            },
        )
    }
    pub(crate) fn runtime_macro_numeric_request(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<Option<runtime_macro_numeric_context::MacroNumericSite>, Diagnostic> {
        runtime_macro_numeric_context::request(self, body, index, key)
    }
    pub(crate) fn initial_thunk_with_runtime_macro_numeric(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        proof: &crate::frontend::runtime_macro_numeric::RuntimeMacroNumericAdmission,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        proof.context(self, body, index, key)?;
        let recipe = self.initial_thunk_admission(
            body,
            index,
            key,
            EnteredAdmissions {
                numeric_admission: Some(proof),
                ..Default::default()
            },
        )?;
        proof.attach(recipe, index)
    }
    pub(crate) fn initial_thunk_with_primitive_name(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        proof: &crate::frontend::primitive_type_names::PrimitiveTypeNameAdmission,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        self.initial_thunk_admission(
            body,
            index,
            key,
            EnteredAdmissions {
                type_name: Some(proof),
                ..Default::default()
            },
        )
    }
    pub(crate) fn initial_thunk_with_source_constraints(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        proof: &crate::frontend::binding_constraints::PreparedBindingConstraints,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        proof.hint_for(index, key)?;
        self.initial_thunk_admission(
            body,
            index,
            key,
            EnteredAdmissions {
                source_constraints: Some(proof),
                ..Default::default()
            },
        )
    }
    /// A bare unsuffixed literal whose own inference variable the common
    /// source constraints resolved.
    /// `literal` is the literal's own goal key: `key` itself, or the
    /// literal under the negation that `key` lowers as one negative literal.
    pub(crate) fn initial_thunk_with_literal_constraints(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        literal: &PcKey,
        proof: &crate::frontend::binding_constraints::PreparedBindingConstraints,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        if proof.literal_hint(index, literal)?.is_none() {
            return Err(error("literal constraint admission is not resolved"));
        }
        self.initial_thunk_admission(
            body,
            index,
            key,
            EnteredAdmissions {
                source_constraints: Some(proof),
                ..Default::default()
            },
        )
    }
    /// A compound assignment whose place is an inferred local. The proof is
    /// the local's own declaring-`let` constraint result; lowering checks
    /// that it names the actual selected binding before using it.
    pub(crate) fn initial_thunk_with_local_constraints(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        proof: &crate::frontend::binding_constraints::PreparedBindingConstraints,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        self.initial_thunk_admission(
            body,
            index,
            key,
            EnteredAdmissions {
                source_constraints: Some(proof),
                ..Default::default()
            },
        )
    }
    /// Source Type demand follows the ordinary cfg/attribute admission. A
    /// disabled expression executes its existing skip recipe without resolving
    /// a Type that no reached Rust operation uses.
    pub(crate) fn entered_cast_requires_type(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<bool, Diagnostic> {
        let syntax = self
            .entered_node_syntax(body, index, NodeId(key.node))?
            .filter(|syntax| ast::CastExpr::can_cast(syntax.kind()))
            .ok_or_else(|| error("source Type consumer is not its actual cast"))?;
        let (enabled, other_attributes) = statement_cfg(&syntax, index.cfg())?;
        Ok(enabled && !other_attributes)
    }
    pub(crate) fn entered_call_requires_goal(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<bool, Diagnostic> {
        let syntax = self
            .entered_node_syntax(body, index, NodeId(key.node))?
            .filter(|syntax| ast::CallExpr::can_cast(syntax.kind()))
            .ok_or_else(|| error("source call consumer is not its actual call"))?;
        let (enabled, other_attributes) = statement_cfg(&syntax, index.cfg())?;
        Ok(enabled && !other_attributes)
    }
    pub(crate) fn initial_thunk_with_source_call(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        goal: &crate::frontend::entered_call_goals::EnteredCallGoal,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        goal.indexed_source_name(index, key)?
            .ok_or_else(|| error("source call consumer lacks indexed target proof"))?;
        self.initial_thunk_admission(
            body,
            index,
            key,
            EnteredAdmissions {
                call: Some(goal),
                ..Default::default()
            },
        )
    }
    /// The actual indexed CallExpr uses the same once-evaluated value-callee
    /// lowering as local bindings and arbitrary expression callees.
    pub(crate) fn initial_thunk_with_value_call_goal(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        goal: &crate::frontend::entered_call_goals::EnteredCallGoal,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        goal.require_value_call_site(index, key)?;
        let call = self
            .entered_node_syntax(body, index, NodeId(key.node))?
            .and_then(ast::CallExpr::cast)
            .ok_or_else(|| error("value call consumer is not its actual call"))?;
        let callee = call
            .expr()
            .ok_or_else(|| error("value call callee absent"))?;
        self.value_call_thunk(body, index, key, &call, &callee)
    }
    /// Lower a cast whose target the entered Type service selected as a
    /// written trait-object Type (`&x as &dyn T`, `Box::new(x) as Box<dyn
    /// T>`): a coercion cast, the same coercion a `let` annotation of that
    /// Type applies. A trait-object place keeps its value's concrete Type.
    pub(crate) fn initial_thunk_with_cast_coercion(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        coercion: &let_annotations::LetAnnotation,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let cast = self
            .entered_node_syntax(body, index, NodeId(key.node))?
            .and_then(ast::CastExpr::cast)
            .ok_or_else(|| error("cast coercion consumer is not its actual cast"))?;
        let ty = cast
            .ty()
            .ok_or_else(|| error("cast coercion target absent"))?;
        if crate::syntax::ast_adapter::range(ty.syntax().text_range()) != coercion.range {
            return Err(error("cast coercion Type is not this cast's target"));
        }
        self.initial_thunk_admission(
            body,
            index,
            key,
            EnteredAdmissions {
                let_annotation: Some(coercion),
                ..Default::default()
            },
        )
    }
    pub(crate) fn initial_thunk_with_source_type(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        proof: &crate::frontend::entered_type_goals::EnteredTypeGoal,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let cast = self
            .entered_node_syntax(body, index, NodeId(key.node))?
            .and_then(ast::CastExpr::cast)
            .ok_or_else(|| error("source Type consumer is not its actual cast"))?;
        let ty = cast
            .ty()
            .ok_or_else(|| error("source Type cast target absent"))?;
        proof.primitive_for(
            index,
            key,
            crate::syntax::ast_adapter::range(ty.syntax().text_range()),
        )?;
        self.initial_thunk_admission(
            body,
            index,
            key,
            EnteredAdmissions {
                type_use: Some(proof),
                ..Default::default()
            },
        )
    }
    /// Lower a `let` whose non-primitive annotation the entered Type service
    /// selected; the statement checks it is its own actual annotation.
    pub(crate) fn initial_thunk_with_let_annotation(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        annotation: &let_annotations::LetAnnotation,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        self.initial_thunk_admission(
            body,
            index,
            key,
            EnteredAdmissions {
                let_annotation: Some(annotation),
                ..Default::default()
            },
        )
    }
    fn initial_thunk_admission(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        admissions: EnteredAdmissions<'_>,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let EnteredAdmissions {
            numeric_admission,
            type_name,
            source_constraints,
            type_use,
            call,
            let_annotation,
            value_goal,
            closure_captures,
        } = admissions;
        if body.key() != *index.body() || key.body != body.key() {
            return Err(error("selected thunk body identity changed"));
        }
        let id = NodeId(key.node);
        index.pc(id, Phase::from_code(key.phase))?;
        if let Some(recipe) = self.initializer_exit(body, index, key)? {
            return Ok(recipe);
        }
        let node = &index.nodes()[key.node as usize];
        if Phase::from_code(key.phase) == Phase::Entry
            && node.boolean_scope.is_some_and(|scope| {
                index.scopes()[scope.0 as usize].kind == super::body_index::ScopeKind::MatchGuard
            })
        {
            self.require_boolean_match_guard(body, index, id)?;
        }
        if Phase::from_code(key.phase) == Phase::Entry
            && matches!(
                node.role,
                NodeRole::Expression(
                    super::body_index::ExpressionKind::If
                        | super::body_index::ExpressionKind::While
                )
            )
        {
            self.condition_plan(
                body,
                index,
                id,
                super::conditions::ConditionLimits::default(),
            )?;
        }
        let syntax = self.entered_node_syntax(body, index, id)?;
        let fallback = index.pc(id, Phase::Exit)?;
        let Some(syntax) = syntax else {
            return thunks::unsupported(
                index,
                key.clone(),
                "entered syntax phase is outside the initial source inventory".into(),
                fallback,
            );
        };
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        let (enabled, other_attributes) = statement_cfg(&syntax, index.cfg())?;
        if !enabled {
            if Phase::from_code(key.phase) != Phase::Entry {
                return unsupported("disabled statement has no evaluated continuation");
            }
            if syntax
                .parent()
                .is_some_and(|parent| ast::StmtList::can_cast(parent.kind()))
                && ast::Stmt::can_cast(syntax.kind())
            {
                let (ops, next) = next_phase_operations(index, body, &syntax, Vec::new())?;
                return draft(index, key, ops, RecipeExit::Next(next));
            }
            if ast::Expr::can_cast(syntax.kind())
                && syntax.parent().is_some_and(|parent| {
                    ast::ExprStmt::can_cast(parent.kind()) || ast::StmtList::can_cast(parent.kind())
                })
            {
                let value = node
                    .value
                    .ok_or_else(|| error("disabled expression unit destination absent"))?;
                return draft(
                    index,
                    key,
                    vec![Op::Unit { destination: value }],
                    RecipeExit::Next(return_pc(index, body, &syntax)?),
                );
            }
            return unsupported("disabled operand requires cfg expression-list integration");
        }
        if other_attributes {
            return unsupported(
                "entered statement/expression attributes require macro integration",
            );
        }
        // Every pattern owner (match, matched let, let-else, if/while let) and
        // its arms/root patterns lower through the one shared pattern plan.
        if let Some(recipe) = self.initial_pattern_thunk(body, index, key)? {
            return Ok(recipe);
        }
        if matches!(
            syntax.kind(),
            SyntaxKind::REF_EXPR
                | SyntaxKind::FIELD_EXPR
                | SyntaxKind::INDEX_EXPR
                | SyntaxKind::PREFIX_EXPR
                | SyntaxKind::PAREN_EXPR
                | SyntaxKind::PATH_EXPR
        ) {
            return self.initial_place_thunk(
                body,
                index,
                key,
                syntax,
                source_constraints,
                value_goal,
            );
        }
        match syntax.kind() {
            SyntaxKind::FN
            | SyntaxKind::MODULE
            | SyntaxKind::TYPE_ALIAS
            | SyntaxKind::STRUCT
            | SyntaxKind::ENUM
            | SyntaxKind::UNION
            | SyntaxKind::USE
            | SyntaxKind::CONST
            | SyntaxKind::STATIC
            | SyntaxKind::IMPL
            | SyntaxKind::TRAIT
            | SyntaxKind::MACRO_RULES
            | SyntaxKind::MACRO_DEF => self.initial_declaration_thunk(body, index, key, syntax),
            SyntaxKind::BLOCK_EXPR | SyntaxKind::LET_STMT | SyntaxKind::EXPR_STMT => self
                .initial_scope_thunk(
                    body,
                    index,
                    key,
                    syntax,
                    numeric_admission,
                    source_constraints,
                    let_annotation,
                ),
            SyntaxKind::LET_EXPR => {
                unsupported("let pattern requires a top-level if/while condition clause")
            }
            SyntaxKind::CALL_EXPR => self.initial_call_thunk(body, index, key, syntax, call),
            SyntaxKind::CLOSURE_EXPR => self.closure_thunk(body, index, key, syntax, closure_captures),
            SyntaxKind::FORMAT_ARGS_EXPR => {
                self.initial_format_args_thunk(body, index, key, syntax)
            }
            SyntaxKind::METHOD_CALL_EXPR => {
                self.initial_method_call_thunk(body, index, key, syntax)
            }
            // `a..=b` is `RangeInclusive::new(a, b)`: an ordinary call of the
            // language-item function.
            SyntaxKind::RANGE_EXPR => self.initial_lang_call_thunk(body, index, key, syntax),
            SyntaxKind::RETURN_EXPR
            | SyntaxKind::LOOP_EXPR
            | SyntaxKind::BREAK_EXPR
            | SyntaxKind::CONTINUE_EXPR => self.initial_jump_thunk(body, index, key, syntax),
            SyntaxKind::WHILE_EXPR | SyntaxKind::IF_EXPR => {
                self.initial_condition_thunk(body, index, key, syntax)
            }
            SyntaxKind::BIN_EXPR => self.initial_binary_thunk(
                body,
                index,
                key,
                syntax,
                numeric_admission,
                source_constraints,
            ),
            SyntaxKind::CAST_EXPR | SyntaxKind::LITERAL => self.initial_scalar_thunk(
                body,
                index,
                key,
                syntax,
                type_name,
                type_use,
                let_annotation,
                source_constraints,
            ),
            _ => unsupported("entered node phase awaits its ordered lowering task"),
        }
    }

    /// Pure lowering of one validated captured PC; separate fixed host stack frame.
    #[inline(never)]
    fn initial_declaration_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        syntax: SyntaxNode,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let id = NodeId(key.node);
        let node = &index.nodes()[key.node as usize];
        let fallback = index.pc(id, Phase::Exit)?;
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        match (syntax.kind(), Phase::from_code(key.phase)) {
            (
                SyntaxKind::FN
                | SyntaxKind::MODULE
                | SyntaxKind::TYPE_ALIAS
                | SyntaxKind::STRUCT
                | SyntaxKind::ENUM
                | SyntaxKind::UNION
                | SyntaxKind::USE
                | SyntaxKind::CONST
                | SyntaxKind::STATIC
                | SyntaxKind::IMPL
                | SyntaxKind::TRAIT
                | SyntaxKind::MACRO_RULES
                | SyntaxKind::MACRO_DEF,
                Phase::Entry,
            ) => {
                if node.role != NodeRole::OpaqueItem {
                    return Err(error("declaration phase lacks opaque item identity"));
                }
                draft(
                    index,
                    key,
                    vec![Op::Declaration],
                    RecipeExit::Next(successor(index, body, &syntax)?),
                )
            }
            _ => unsupported("entered node phase awaits its ordered lowering task"),
        }
    }

    /// Pure lowering of one validated captured PC; separate fixed host stack frame.
    #[inline(never)]
    fn initial_scope_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        syntax: SyntaxNode,
        numeric_admission: Option<
            &crate::frontend::runtime_macro_numeric::RuntimeMacroNumericAdmission,
        >,
        source_constraints: Option<
            &crate::frontend::binding_constraints::PreparedBindingConstraints,
        >,
        let_annotation: Option<&let_annotations::LetAnnotation>,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let id = NodeId(key.node);
        let node = &index.nodes()[key.node as usize];
        let fallback = index.pc(id, Phase::Exit)?;
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        match (syntax.kind(), Phase::from_code(key.phase)) {
            (SyntaxKind::BLOCK_EXPR, Phase::Entry) => {
                let block = ast::BlockExpr::cast(syntax).expect("checked block");
                // `unsafe { .. }` evaluates exactly as an ordinary block; the
                // unsafety obligation is a static check (cargo check), not a
                // runtime semantic. Other modifiers change evaluation.
                if block.async_token().is_some()
                    || block.const_token().is_some()
                    || block.gen_token().is_some()
                    || block.move_token().is_some()
                    || block.try_block_modifier().is_some()
                {
                    return unsupported("modified block requires its ordered semantics");
                }
                if block.stmt_list().is_some_and(|list| {
                    list.syntax()
                        .children()
                        .any(|child| ast::Attr::can_cast(child.kind()))
                }) {
                    return unsupported("block attributes require cfg/macro integration");
                }
                let next = block_first_pc(index, body, &block)?;
                let mut ops = vec![Op::ScopeEnter { scope: node.scope }];
                if id == NodeId(0)
                    && let Some(params) = block
                        .syntax()
                        .ancestors()
                        .find_map(ast::Fn::cast)
                        .and_then(|f| f.param_list())
                {
                    let count =
                        params.params().count() + usize::from(params.self_param().is_some());
                    if count != 0 {
                        ops.insert(
                            0,
                            Op::CheckParameters {
                                count: u32::try_from(count)
                                    .map_err(|_| error("parameter count overflow"))?,
                            },
                        );
                    }
                }
                // A pattern function (E-R5-010) enters its parameter scope
                // (the block's parent) first and binds its pattern parameters
                // through the shared transaction before the block's first PC.
                let parameter_owner = if id == NodeId(0) {
                    index.parameter_owner()
                } else {
                    None
                };
                if parameter_owner.is_some() {
                    let at = usize::from(matches!(ops.first(), Some(Op::CheckParameters { .. })));
                    ops.insert(
                        at,
                        Op::ScopeEnter {
                            scope: crate::syntax::body_index::ScopeId(0),
                        },
                    );
                }

                if id == NodeId(0)
                    && let Some(ty) = block
                        .syntax()
                        .ancestors()
                        .find_map(ast::Fn::cast)
                        .and_then(|function| function.ret_type())
                        .and_then(|ret| ret.ty())
                {
                    let return_hint = if matches!(ty, ast::Type::RefType(_))
                        || (matches!(ty, ast::Type::TupleType(_) | ast::Type::ArrayType(_))
                            && annotation_hint(self, Some(ty.clone()), key.body.definition)?
                                .is_none())
                    {
                        // Region identity comes from the admitted definition's
                        // actual caller/result certificate, never a header-local
                        // elision or a region invented while lowering syntax.
                        HintRecipe::DefinitionResult
                    } else {
                        // A written name not resolvable while lowering (a
                        // nominal/alias result) takes the declared result its
                        // call contract closed (`-> Option<u32>`), if any; the
                        // call boundary still checks the returned value.
                        let path = matches!(ty, ast::Type::PathType(_));
                        annotation_hint(self, Some(ty), key.body.definition)?.unwrap_or(if path {
                            HintRecipe::DefinitionWrittenResult
                        } else {
                            HintRecipe::None
                        })
                    };
                    ops.push(Op::SetHint {
                        child: node.value.expect("root value"),
                        hint: return_hint,
                    });
                }
                if next.phase == Phase::Entry.code()?
                    && next.node != id.0
                    && let Some(child) = index.nodes()[next.node as usize].value
                {
                    ops.push(Op::NumericHint {
                        parent: node.value.expect("root value"),
                        child,
                        left: None,
                        operator: None,
                    });
                }
                let exit = match parameter_owner {
                    Some(owner) => index.pc(owner, Phase::Entry)?,
                    None => next,
                };
                draft(index, key, ops, RecipeExit::Next(exit))
            }
            (SyntaxKind::BLOCK_EXPR, Phase::Exit) => {
                let block = ast::BlockExpr::cast(syntax).expect("checked tail block");
                let scope = node
                    .tail_scope
                    .ok_or_else(|| error("tail entry scope absent"))?;
                let tail = block
                    .tail_expr()
                    .ok_or_else(|| error("tail entry expression absent"))?;
                let child = id_for(index, body, tail.syntax())?;
                draft(
                    index,
                    key,
                    vec![
                        Op::ScopeEnter { scope },
                        Op::NumericHint {
                            parent: node
                                .value
                                .ok_or_else(|| error("tail result destination absent"))?,
                            child: index.nodes()[child.0 as usize]
                                .value
                                .ok_or_else(|| error("tail expression value absent"))?,
                            left: None,
                            operator: None,
                        },
                    ],
                    RecipeExit::Next(index.pc(child, Phase::Entry)?),
                )
            }
            (SyntaxKind::BLOCK_EXPR, Phase::Commit) => {
                let block = ast::BlockExpr::cast(syntax).expect("checked block");
                let destination = node.value.expect("block value slot");
                let op = if let Some(tail) = block.tail_expr() {
                    let child = id_for(index, body, tail.syntax())?;
                    Op::ResultTransfer {
                        source: index.nodes()[child.0 as usize].value.expect("tail value"),
                        destination,
                    }
                } else {
                    Op::Unit { destination }
                };
                let mut ops = vec![op];
                if let Some(scope) = node.tail_scope {
                    ops.push(Op::ScopeLeave { scope });
                }
                if id == NodeId(0) {
                    draft(
                        index,
                        key,
                        ops,
                        RecipeExit::Control(ControlRecipe::Return { value: destination }),
                    )
                } else {
                    draft(
                        index,
                        key,
                        {
                            ops.push(Op::ScopeLeave { scope: node.scope });
                            ops
                        },
                        RecipeExit::Next(return_pc(index, body, block.syntax())?),
                    )
                }
            }
            (SyntaxKind::LET_STMT, phase) => {
                let statement = ast::LetStmt::cast(syntax).expect("checked let");
                if statement.let_else().is_some() {
                    return unsupported("let-else awaits pattern/control integration");
                }
                if matches!(statement.pat(), Some(ast::Pat::WildcardPat(_))) {
                    let Some(initializer) = statement.initializer() else {
                        return unsupported("wildcard declaration requires an initializer");
                    };
                    // A wildcard does not move a place into a new local. A local
                    // path or a field chain of one has no runtime effect; a
                    // non-local path (constant, unit struct, fn item) is a
                    // value created and dropped. Other places stay refused.
                    if intrinsic_place_receiver(&initializer) {
                        let mut base = initializer.clone();
                        loop {
                            match &base {
                                ast::Expr::ParenExpr(paren) => match paren.expr() {
                                    Some(inner) => base = inner,
                                    None => break,
                                },
                                ast::Expr::FieldExpr(field)
                                    if matches!(initializer, ast::Expr::FieldExpr(_)) =>
                                {
                                    match field.expr() {
                                        Some(inner) => base = inner,
                                        None => break,
                                    }
                                }
                                _ => break,
                            }
                        }
                        let local = match &base {
                            ast::Expr::PathExpr(path) => {
                                local_binding(self, index, body, path)?.is_some()
                                    || MacroName::local(
                                        &path.syntax().text().to_string(),
                                        self.edition,
                                    )
                                    .ok()
                                    .map(|name| parameter_at(self, path.syntax(), &name))
                                    .transpose()?
                                    .flatten()
                                    .is_some()
                            }
                            _ => false,
                        };
                        let value_path = matches!(&initializer, ast::Expr::PathExpr(_)) && !local;
                        if local {
                            if phase != Phase::Entry {
                                return unsupported("invalid wildcard declaration phase");
                            }
                            let (ops, next) =
                                next_phase_operations(index, body, statement.syntax(), vec![])?;
                            return draft(index, key, ops, RecipeExit::Next(next));
                        }
                        if !value_path {
                            return unsupported(
                                "wildcard place requires non-moving type integration",
                            );
                        }
                    }
                    let Some(mut hint) =
                        annotation_hint(self, statement.ty(), key.body.definition)?
                            .or_else(|| selected_annotation(&statement, let_annotation))
                    else {
                        return unsupported("wildcard type requires symbolic type resolution");
                    };
                    if hint == HintRecipe::None
                        && let Some(primitive) =
                            super::numeric_fallback::literal_only(&initializer)?
                    {
                        hint = HintRecipe::Primitive(primitive);
                    }
                    let initializer = id_for(index, body, initializer.syntax())?;
                    let source = index.nodes()[initializer.0 as usize]
                        .value
                        .ok_or_else(|| error("wildcard initializer value absent"))?;
                    return match phase {
                        Phase::Entry => draft(
                            index,
                            key,
                            vec![Op::SetHint {
                                child: source,
                                hint,
                            }],
                            RecipeExit::Next(index.pc(initializer, Phase::Entry)?),
                        ),
                        Phase::Commit => {
                            let (ops, next) = next_phase(
                                index,
                                body,
                                statement.syntax(),
                                Op::Discard { source },
                            )?;
                            draft(index, key, ops, RecipeExit::Next(next))
                        }
                        _ => unsupported("invalid wildcard declaration phase"),
                    };
                }
                let Some(ast::Pat::IdentPat(pattern)) = statement.pat() else {
                    return unsupported("binding pattern awaits pattern integration");
                };
                if pattern.at_token().is_some() {
                    return unsupported("binding mode awaits pattern integration");
                }
                let by_reference = pattern.ref_token().is_some();
                let Some(mut hint) = annotation_hint(self, statement.ty(), key.body.definition)?
                    .or_else(|| selected_annotation(&statement, let_annotation))
                else {
                    return unsupported("local type requires later symbolic type resolution");
                };
                let Some(initializer) = statement.initializer() else {
                    if by_reference {
                        return unsupported(
                            "uninitialized reference binding awaits pattern integration",
                        );
                    }
                    if phase != Phase::Entry {
                        return unsupported("invalid declaration phase");
                    }
                    let (ops, next) = next_phase(
                        index,
                        body,
                        statement.syntax(),
                        Op::DeclareLocal {
                            binding: id_for(index, body, pattern.syntax())?,
                            hint,
                            writable: pattern.mut_token().is_some(),
                        },
                    )?;
                    return draft(index, key, ops, RecipeExit::Next(next));
                };
                if hint == HintRecipe::None
                    && !by_reference
                    && let Some(proof) = source_constraints
                    && let Some(source_hint) = proof.hint_for(index, key)?
                {
                    hint = source_hint;
                }
                // An exact owning parameter/result source relation may constrain
                // a bare numeric local before its first read. The hint carries
                // the definition relation, never a guessed primitive descriptor.
                if hint == HintRecipe::None
                    && !by_reference
                    && let Some(context) = runtime_local_numeric_context::inferred_context(
                        self, index, body, &statement, &pattern,
                    )?
                {
                    hint = HintRecipe::DefinitionNumeric(context);
                }
                if hint == HintRecipe::None
                    && !by_reference
                    && let Some(proof) = numeric_admission
                {
                    let (binding, context) = proof.context(self, body, index, key)?;
                    if binding != id_for(index, body, pattern.syntax())? {
                        return Err(error("numeric initializer proof names another binding"));
                    }
                    hint = HintRecipe::DefinitionNumeric(context);
                }
                // Default only a closed local inference variable. This source
                // proof treats casts/discarded reads as unconstraining uses;
                // annotations, calls, patterns and opaque uses remain deferred.
                if hint == HintRecipe::None
                    && !by_reference
                    && let Some(primitive) = local_numeric_constraints::unconstrained_literal(
                        self,
                        index,
                        body,
                        &statement,
                        &pattern,
                        &initializer,
                    )?
                {
                    hint = HintRecipe::Primitive(primitive);
                }
                // Otherwise preserve the existing closed-operator fallback.
                // A written type always wins.
                if hint == HintRecipe::None
                    && let Some(primitive) =
                        super::numeric_fallback::operator_initializer(&initializer)?
                {
                    hint = HintRecipe::Primitive(local_numeric_constraints::operator_type(
                        self, index, body, &statement, &pattern, primitive,
                    )?);
                }
                let initializer_is_place = by_reference && intrinsic_place_receiver(&initializer);
                let reference_hint = if by_reference {
                    match &hint {
                        HintRecipe::None => HintRecipe::None,
                        HintRecipe::Primitive(target) => HintRecipe::Reference {
                            region: crate::traits::term::RegionTerm::Local {
                                definition: key.body.definition,
                                start: node.range.start,
                                end: node.range.end,
                            },
                            mutable: pattern.mut_token().is_some(),
                            target: *target,
                        },
                        _ => {
                            return unsupported(
                                "reference binding annotation requires symbolic target integration",
                            );
                        }
                    }
                } else {
                    HintRecipe::None
                };
                let initializer = id_for(index, body, initializer.syntax())?;
                let source = index.nodes()[initializer.0 as usize]
                    .value
                    .expect("initializer value");
                match phase {
                    Phase::Entry => {
                        let mut ops = vec![];
                        if !initializer_is_place {
                            ops.push(Op::SetHint {
                                child: source,
                                hint,
                            });
                        }
                        if by_reference && reference_hint != HintRecipe::None {
                            ops.push(Op::SetHint {
                                child: node.runtime_temporaries[0]
                                    .ok_or_else(|| error("reference binding temporary absent"))?,
                                hint: reference_hint,
                            });
                        }
                        draft(
                            index,
                            key,
                            ops,
                            RecipeExit::Next(index.pc(initializer, Phase::Entry)?),
                        )
                    }
                    Phase::Commit => {
                        let binding_source = if by_reference {
                            node.runtime_temporaries[0]
                                .ok_or_else(|| error("reference binding temporary absent"))?
                        } else {
                            source
                        };
                        let (mut ops, next) = next_phase(
                            index,
                            body,
                            statement.syntax(),
                            Op::BindLocal {
                                binding: id_for(index, body, pattern.syntax())?,
                                source: binding_source,
                                // A written let type constrains the initializer;
                                // the reference carries the checked target type.
                                hint: if by_reference { reference_hint } else { hint },
                                writable: !by_reference && pattern.mut_token().is_some(),
                            },
                        )?;
                        if by_reference {
                            let place = index.nodes()[initializer.0 as usize]
                                .place
                                .ok_or_else(|| error("reference initializer place absent"))?;
                            // Address the owning result without reading or
                            // moving it. Only the new reference moves into the
                            // binding before statement cleanup.
                            ops.insert(
                                0,
                                Op::Borrow {
                                    place,
                                    access: if pattern.mut_token().is_some() {
                                        crate::ir::op::BorrowAccess::Mutable
                                    } else {
                                        crate::ir::op::BorrowAccess::Shared
                                    },
                                    destination: binding_source,
                                },
                            );
                            if !initializer_is_place {
                                ops.insert(
                                    0,
                                    Op::ValuePlace {
                                        source,
                                        destination: place,
                                    },
                                );
                            }
                        }
                        draft(index, key, ops, RecipeExit::Next(next))
                    }
                    _ => unsupported("invalid reached let phase"),
                }
            }
            (SyntaxKind::EXPR_STMT, phase) => {
                let statement = ast::ExprStmt::cast(syntax).expect("checked expression statement");
                let Some(expression) = statement.expr() else {
                    return unsupported("empty expression statement");
                };
                let child = id_for(index, body, expression.syntax())?;
                let source = index.nodes()[child.0 as usize]
                    .value
                    .expect("statement child value");
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![Op::SetHint {
                            child: source,
                            // This result is discarded and cannot constrain a
                            // later binding. Only closed original numeric syntax
                            // permits Rust fallback; names/calls remain ordinary
                            // source-constraint work, never VM-wide defaults.
                            hint: super::numeric_fallback::literal_only(&expression)?
                                .map_or(HintRecipe::None, HintRecipe::Primitive),
                        }],
                        RecipeExit::Next(index.pc(child, Phase::Entry)?),
                    ),
                    Phase::Commit => {
                        let (ops, next) =
                            next_phase(index, body, statement.syntax(), Op::Discard { source })?;
                        draft(index, key, ops, RecipeExit::Next(next))
                    }
                    _ => unsupported("invalid reached statement phase"),
                }
            }
            _ => unsupported("entered node phase awaits its ordered lowering task"),
        }
    }

    /// Pure lowering of one validated captured PC; separate fixed host stack frame.
    #[inline(never)]
    fn initial_call_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        syntax: SyntaxNode,
        source_goal: Option<&crate::frontend::entered_call_goals::EnteredCallGoal>,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let id = NodeId(key.node);
        let node = &index.nodes()[key.node as usize];
        let fallback = index.pc(id, Phase::Exit)?;
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        match (syntax.kind(), Phase::from_code(key.phase)) {
            (SyntaxKind::CALL_EXPR, phase) => {
                let call = ast::CallExpr::cast(syntax).expect("checked call");
                let Some(list) = call.arg_list() else {
                    return unsupported("call argument list absent");
                };
                let arguments = list
                    .args()
                    .map(|arg| id_for(index, body, arg.syntax()))
                    .collect::<Result<Vec<_>, _>>()?;
                let slots: Vec<_> = arguments
                    .iter()
                    .map(|id| index.nodes()[id.0 as usize].value.expect("argument value"))
                    .collect();
                // A callee that is a value (any non-path expression, or a
                // path naming a local binding or parameter) is called through
                // its reached callable type, never by its spelling.
                let Some(callee) = call.expr() else {
                    return unsupported("callee absent");
                };
                let ast::Expr::PathExpr(path) = &callee else {
                    return self.value_call_thunk(body, index, key, &call, &callee);
                };
                let path = path.clone();
                if source_goal.is_none() && local_binding(self, index, body, &path)?.is_some() {
                    return self.value_call_thunk(body, index, key, &call, &callee);
                }
                if source_goal.is_none()
                    && let Some(source_path) = path.path()
                    && source_path.qualifier().is_none()
                    && let Some(reference) =
                        source_path.segment().and_then(|segment| segment.name_ref())
                    && let Ok(name) = MacroName::local(&reference.text(), self.edition)
                    && parameter_at(self, path.syntax(), &name)?.is_some()
                {
                    return self.value_call_thunk(body, index, key, &call, &callee);
                }
                let Some(source_path) = path.path() else {
                    return unsupported("callee path absent");
                };
                let Some(segment) = source_path.segment() else {
                    return unsupported("callee segment absent");
                };
                let Some(reference) = segment.name_ref() else {
                    return unsupported("callee name absent");
                };
                let mut components = Vec::new();
                let mut cursor = source_path.clone();
                let absolute;
                loop {
                    let Some(part) = cursor.segment() else {
                        return unsupported("callee path segment absent");
                    };
                    // Type arguments written on the owner segment
                    // (`Vec::<T>::new()`) are selected at their indexed Type
                    // occurrences by the dispatcher (`engine/run_owner_arguments.rs`)
                    // and bind the owner impl's parameters. No other segment
                    // of a path takes arguments in a program rustc accepts.
                    let text = match part.kind() {
                        Some(ast::PathSegmentKind::Name(name)) if name.text() == "$crate" => {
                            // `$crate` is the macro's defining crate. Within the
                            // same crate it is exactly `crate`.
                            // rustc resolves `$crate` to the defining crate's
                            // root, never through the caller's names. The chain
                            // root of a definition token is the defining source,
                            // so locality cannot be read from it; always name
                            // the defining root explicitly.
                            match self.dollar_crate_root(crate::syntax::ast_adapter::range(
                                name.syntax().text_range(),
                            ))? {
                                Some((unit, _)) => {
                                    crate::frontend::callables::dollar_crate_segment(unit)
                                }
                                None => {
                                    return Err(error("`$crate` outside generated syntax"));
                                }
                            }
                        }
                        Some(ast::PathSegmentKind::Name(name)) => name.text().to_string(),
                        Some(ast::PathSegmentKind::CrateKw) => "crate".into(),
                        Some(ast::PathSegmentKind::SelfKw) => "self".into(),
                        Some(ast::PathSegmentKind::SuperKw) => "super".into(),
                        // `Self::item`: resolved by associated-item lookup.
                        Some(ast::PathSegmentKind::SelfTypeKw) => "Self".into(),
                        // `<Type as Trait>::name` / `<Type>::name`: the
                        // dispatcher selects the impl of the anchor type (an
                        // enclosing generic parameter reads its reached
                        // binding), through the value-path anchor spelling.
                        Some(ast::PathSegmentKind::Type {
                            type_ref,
                            trait_ref,
                        }) if cursor.qualifier().is_none() => {
                            let Some(ty) = type_ref.and_then(|ty| match ty {
                                ast::Type::PathType(path) => path.path(),
                                _ => None,
                            }) else {
                                return unsupported(
                                    "qualified callee anchor requires structural type integration",
                                );
                            };
                            let Some(ty) =
                                self.anchor_spelling(&ty, path.syntax(), index.cfg(), true)?
                            else {
                                return unsupported(
                                    "qualified callee anchor requires further type integration",
                                );
                            };
                            match trait_ref.and_then(|tr| tr.path()) {
                                Some(tr) => match self.anchor_spelling(
                                    &tr,
                                    path.syntax(),
                                    index.cfg(),
                                    false,
                                )? {
                                    Some(tr) => format!("<{ty} as {tr}>"),
                                    None => {
                                        return unsupported(
                                            "qualified callee trait requires further integration",
                                        );
                                    }
                                },
                                None => format!("<{ty}>"),
                            }
                        }
                        _ => {
                            return unsupported("callee path requires associated-call integration");
                        }
                    };
                    components.push(text);
                    if let Some(parent) = cursor.qualifier() {
                        cursor = parent;
                    } else {
                        absolute = part.coloncolon_token().is_some();
                        break;
                    }
                }
                components.reverse();
                let qualified = components.len() > 1 || absolute;
                // A `$crate` root names the defining crate root explicitly.
                let relative = !absolute
                    && !matches!(components[0].as_str(), "crate" | "self" | "super")
                    && !components[0].starts_with("$crate@")
                    && !components[0].starts_with('<')
                    && components[0] != "Self";
                let root_name = components[0].clone();
                let mut name = components.join("::");
                if absolute {
                    name.insert_str(0, "::");
                }

                let explicit = segment
                    .generic_arg_list()
                    .map(|list| {
                        list.generic_args()
                            .map(|arg| arg.syntax().text().to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if let Some(goal) = source_goal {
                    let target = goal.indexed_source_name(index, key)?.ok_or_else(|| {
                        error("source call goal did not admit this indexed target")
                    })?;
                    if target.node() != id_for(index, body, path.syntax())?
                        || target.range()
                            != crate::syntax::ast_adapter::range(path.syntax().text_range())
                        || target.name() != name
                        || target.explicit() != explicit
                    {
                        return Err(error("source call lowering written target differs"));
                    }
                } else {
                    let local_type = path
                        .syntax()
                        .ancestors()
                        .filter_map(ast::StmtList::cast)
                        .any(|list| {
                            list.statements().any(|statement| {
                                let declared = match statement {
                                    ast::Stmt::Item(ast::Item::TypeAlias(item)) => item.name(),
                                    ast::Stmt::Item(ast::Item::Struct(item)) => item.name(),
                                    ast::Stmt::Item(ast::Item::Enum(item)) => item.name(),
                                    ast::Stmt::Item(ast::Item::Union(item)) => item.name(),
                                    _ => None,
                                };
                                declared.is_some_and(|name| {
                                    explicit.iter().any(|ty| {
                                        MacroName::parse(ty, self.edition).ok().is_some_and(
                                            |argument| {
                                                MacroName::parse(name.text(), self.edition)
                                                    .is_ok_and(|local| local == argument)
                                            },
                                        )
                                    })
                                })
                            })
                        });
                    if local_type {
                        return unsupported(
                            "body-local explicit type requires lexical type namespace integration",
                        );
                    }
                    // A generic parameter owner (`T::name`) is selected by
                    // the dispatcher from the parameter's reached binding,
                    // before any same-named module type.
                    let target_name = MacroName::parse(&root_name, self.edition).ok();
                    let local_item = relative
                        && path
                            .syntax()
                            .ancestors()
                            .filter_map(ast::StmtList::cast)
                            .any(|list| {
                                list.statements().any(|statement| match statement {
                                    ast::Stmt::Item(ast::Item::Fn(item)) if !qualified => {
                                        item.name().is_some_and(|n| {
                                            MacroName::parse(n.text(), self.edition).ok()
                                                == target_name
                                        })
                                    }
                                    ast::Stmt::Item(ast::Item::Const(item)) if !qualified => {
                                        item.name().is_some_and(|n| {
                                            MacroName::parse(n.text(), self.edition).ok()
                                                == target_name
                                        })
                                    }
                                    ast::Stmt::Item(ast::Item::Static(item)) if !qualified => {
                                        item.name().is_some_and(|n| {
                                            MacroName::parse(n.text(), self.edition).ok()
                                                == target_name
                                        })
                                    }
                                    ast::Stmt::Item(ast::Item::Module(item)) if qualified => {
                                        item.name().is_some_and(|n| {
                                            MacroName::parse(n.text(), self.edition).ok()
                                                == target_name
                                        })
                                    }
                                    ast::Stmt::Item(ast::Item::TypeAlias(item)) if qualified => {
                                        item.name().is_some_and(|n| {
                                            MacroName::parse(n.text(), self.edition).ok()
                                                == target_name
                                        })
                                    }
                                    _ => false,
                                })
                            });
                    // This probe only permits an ordinary call recipe. The
                    // dispatcher must resolve the exact entered lexical definition
                    // before any argument or call opcode; it is not a name proof.
                    let lexical_function = relative
                        && !qualified
                        && self.lexical_function_recipe_candidate(&path, index.cfg())?;
                    // A callee (or two-segment path root) a block-level import
                    // or item binds is selected by the dispatcher's entered
                    // lexical Value goal, which refuses what it cannot select
                    // rather than falling back to a same-named module item.
                    if local_item && !lexical_function
                        || relative
                            && components.len() > 2
                            && self.enclosing_import_may_bind(
                                path.syntax(),
                                &root_name,
                                index.cfg(),
                            )?
                    {
                        return unsupported(
                            "lexical item callee requires body namespace integration",
                        );
                    }
                }

                if !qualified && MacroName::parse(reference.text(), self.edition).is_err() {
                    return unsupported(
                        "qualified or explicitly generic call awaits call-path integration",
                    );
                }
                let site = crate::ir::recipe::CallSiteId(id.0);
                let seal = |ops: Vec<Op>| {
                    draft(
                        index,
                        key,
                        ops,
                        RecipeExit::Call {
                            site,
                            resume: return_pc(index, body, call.syntax())?,
                            result: node.value.expect("call result"),
                        },
                    )
                };
                match phase {
                    Phase::Entry if arguments.is_empty() => seal(vec![Op::CallSeal {
                        site,
                        name,
                        explicit,
                        arguments: slots,
                    }]),
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![Op::CallPrepare {
                            site,
                            name,
                            explicit,
                            arguments: slots,
                        }],
                        RecipeExit::Next(index.pc(arguments[0], Phase::Entry)?),
                    ),
                    Phase::Operand(n) => {
                        let child = node
                            .operands
                            .get(n as usize)
                            .ok_or_else(|| error("call operand absent"))?;
                        let ordinal = arguments
                            .iter()
                            .position(|arg| arg == child)
                            .ok_or_else(|| error("call continuation is not a written argument"))?;
                        let advance = Op::CallAdvance {
                            site,
                            argument: u32::try_from(ordinal)
                                .map_err(|_| error("argument ordinal overflow"))?,
                        };
                        if ordinal + 1 == arguments.len() {
                            seal(vec![
                                advance,
                                Op::CallSeal {
                                    site,
                                    name,
                                    explicit,
                                    arguments: slots,
                                },
                            ])
                        } else {
                            draft(
                                index,
                                key,
                                vec![advance],
                                RecipeExit::Next(index.pc(arguments[ordinal + 1], Phase::Entry)?),
                            )
                        }
                    }
                    _ => unsupported("invalid call argument phase"),
                }
            }

            _ => unsupported("entered node phase awaits its ordered lowering task"),
        }
    }

    /// Pure lowering of one validated captured PC; separate fixed host stack frame.
    #[inline(never)]
    fn initial_jump_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        syntax: SyntaxNode,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let id = NodeId(key.node);
        let node = &index.nodes()[key.node as usize];
        let fallback = index.pc(id, Phase::Exit)?;
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        match (syntax.kind(), Phase::from_code(key.phase)) {
            (SyntaxKind::RETURN_EXPR, phase) => {
                let syntax_root = syntax.clone();
                let expression = ast::ReturnExpr::cast(syntax).expect("checked return");
                let destination = node.value.expect("return value");
                let Some(child) = expression.expr() else {
                    return draft(
                        index,
                        key,
                        vec![Op::Unit { destination }],
                        RecipeExit::Control(ControlRecipe::Return { value: destination }),
                    );
                };
                let child = id_for(index, body, child.syntax())?;
                let source = index.nodes()[child.0 as usize]
                    .value
                    .expect("return child value");
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![Op::NumericHint {
                            // A `return` in a closure returns the closure body's value.
                            parent: match closure_thunk::enclosing_closure(&syntax_root) {
                                Some(closure) => closure
                                    .body()
                                    .map(|closure_body| id_for(index, body, closure_body.syntax()))
                                    .transpose()?
                                    .and_then(|id| index.nodes()[id.0 as usize].value)
                                    .ok_or_else(|| error("closure body value absent"))?,
                                None => index.nodes()[0].value.expect("root result"),
                            },
                            child: source,
                            left: None,
                            operator: None,
                        }],
                        RecipeExit::Next(index.pc(child, Phase::Entry)?),
                    ),
                    Phase::Operand(0) => draft(
                        index,
                        key,
                        vec![Op::ResultTransfer {
                            source,
                            destination,
                        }],
                        RecipeExit::Control(ControlRecipe::Return { value: destination }),
                    ),
                    _ => unsupported("invalid return phase"),
                }
            }
            (SyntaxKind::LOOP_EXPR, phase) => {
                let expression = ast::LoopExpr::cast(syntax).expect("checked loop");
                let Some(loop_body) = expression
                    .syntax()
                    .children()
                    .find_map(ast::BlockExpr::cast)
                else {
                    return unsupported("loop body absent");
                };
                let child = id_for(index, body, loop_body.syntax())?;
                let body_slot = index.nodes()[child.0 as usize]
                    .value
                    .expect("loop body value");
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![Op::ScopeEnter { scope: node.scope }],
                        RecipeExit::Next(index.pc(child, Phase::Entry)?),
                    ),
                    Phase::Operand(0) => draft(
                        index,
                        key,
                        vec![
                            Op::CheckUnit { source: body_slot },
                            Op::ScopeLeave { scope: node.scope },
                        ],
                        RecipeExit::Backedge(index.pc(id, Phase::Entry)?),
                    ),
                    _ => unsupported("invalid loop phase"),
                }
            }
            (SyntaxKind::BREAK_EXPR, phase) => {
                let expression = ast::BreakExpr::cast(syntax).expect("checked break");
                let Some(target) =
                    control_target(self, index, body, node, expression.lifetime(), false)?
                else {
                    return unsupported("break target is absent");
                };
                let label = &index.labels()[target.0 as usize];
                let target_node = &index.nodes()[label.node.0 as usize];
                if expression.expr().is_some()
                    && matches!(
                        target_node.role,
                        NodeRole::Expression(
                            super::body_index::ExpressionKind::While
                                | super::body_index::ExpressionKind::For
                        )
                    )
                {
                    return unsupported("while/for does not accept a break value");
                }
                let start = body
                    .range
                    .start
                    .checked_add(target_node.range.start)
                    .ok_or_else(|| error("break target range overflow"))?;
                let end = body
                    .range
                    .start
                    .checked_add(target_node.range.end)
                    .ok_or_else(|| error("break target range overflow"))?;
                let target_syntax = self
                    .syntax_at(
                        crate::syntax::spans::SourceRange::checked(start, end)?,
                        |s| ast::Expr::can_cast(s.kind()),
                    )?
                    .ok_or_else(|| error("break continuation syntax absent"))?;
                // `break` out of a `for` leaves one iteration; the loop's
                // exit then drops its iterator.
                let resume = if target_node.role
                    == NodeRole::Expression(super::body_index::ExpressionKind::For)
                {
                    index.pc(label.node, Phase::Exit)?
                } else {
                    return_pc(index, body, &target_syntax)?
                };
                let destination = node.value.expect("break result");
                if let Some(child) = expression.expr() {
                    let child = id_for(index, body, child.syntax())?;
                    let source = index.nodes()[child.0 as usize].value.expect("break child");
                    match phase {
                        Phase::Entry => draft(
                            index,
                            key,
                            vec![Op::NumericHint {
                                parent: target_node.value.expect("target result"),
                                child: source,
                                left: None,
                                operator: None,
                            }],
                            RecipeExit::Next(index.pc(child, Phase::Entry)?),
                        ),
                        Phase::Operand(0) => draft(
                            index,
                            key,
                            vec![Op::ResultTransfer {
                                source,
                                destination,
                            }],
                            RecipeExit::Control(ControlRecipe::Break {
                                target,
                                value: Some(destination),
                                resume,
                            }),
                        ),
                        _ => unsupported("invalid break value phase"),
                    }
                } else {
                    draft(
                        index,
                        key,
                        vec![Op::Unit { destination }],
                        RecipeExit::Control(ControlRecipe::Break {
                            target,
                            value: Some(destination),
                            resume,
                        }),
                    )
                }
            }
            (SyntaxKind::CONTINUE_EXPR, Phase::Entry) => {
                let expression = ast::ContinueExpr::cast(syntax).expect("checked continue");
                let Some(target) =
                    control_target(self, index, body, node, expression.lifetime(), true)?
                else {
                    return unsupported("continue target is absent or not a loop");
                };
                draft(
                    index,
                    key,
                    vec![Op::Unit {
                        destination: node.value.expect("continue value"),
                    }],
                    RecipeExit::Control(ControlRecipe::Continue { target }),
                )
            }
            _ => unsupported("entered node phase awaits its ordered lowering task"),
        }
    }

    /// Pure lowering of one validated captured PC; separate fixed host stack frame.
    #[inline(never)]
    fn initial_condition_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        syntax: SyntaxNode,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let id = NodeId(key.node);
        let node = &index.nodes()[key.node as usize];
        let fallback = index.pc(id, Phase::Exit)?;
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        match (syntax.kind(), Phase::from_code(key.phase)) {
            (SyntaxKind::WHILE_EXPR, phase) => {
                let expression = ast::WhileExpr::cast(syntax).expect("checked while");
                let mut operands = expression.syntax().children().filter_map(ast::Expr::cast);
                let Some(condition) = operands.next() else {
                    return unsupported("while condition absent");
                };
                let Some(ast::Expr::BlockExpr(loop_body)) = operands.next() else {
                    return unsupported("while body absent");
                };
                let condition = id_for(index, body, condition.syntax())?;
                let loop_body = id_for(index, body, loop_body.syntax())?;
                let condition_slot = index.nodes()[condition.0 as usize]
                    .value
                    .expect("while condition");
                let body_slot = index.nodes()[loop_body.0 as usize]
                    .value
                    .expect("while body value");
                let destination = node.value.expect("while value");
                let condition_scope = node
                    .condition_scope
                    .ok_or_else(|| error("while condition scope absent"))?;
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![
                            Op::ScopeEnter { scope: node.scope },
                            Op::ScopeEnter {
                                scope: condition_scope,
                            },
                            Op::SetHint {
                                child: condition_slot,
                                hint: HintRecipe::Primitive(PrimitiveType::Bool),
                            },
                        ],
                        RecipeExit::Next(index.pc(condition, Phase::Entry)?),
                    ),
                    Phase::Operand(0) => {
                        let mut ops = vec![Op::BranchHint {
                            parent: destination,
                            condition: condition_slot,
                            yes: None,
                            no: None,
                            boolean: false,
                        }];
                        if let Some(scope) = index.nodes()[condition.0 as usize].boolean_scope {
                            ops.push(Op::ScopeLeave { scope });
                        }
                        draft(
                            index,
                            key,
                            ops,
                            RecipeExit::Branch {
                                condition: condition_slot,
                                yes: index.pc(loop_body, Phase::Entry)?,
                                no: index.pc(id, Phase::Commit)?,
                            },
                        )
                    }
                    Phase::Operand(1) => draft(
                        index,
                        key,
                        vec![
                            Op::CheckUnit { source: body_slot },
                            Op::ScopeLeave {
                                scope: condition_scope,
                            },
                            Op::ScopeLeave { scope: node.scope },
                        ],
                        RecipeExit::Backedge(index.pc(id, Phase::Entry)?),
                    ),
                    Phase::Commit => draft(
                        index,
                        key,
                        vec![
                            Op::ScopeLeave {
                                scope: condition_scope,
                            },
                            Op::ScopeLeave { scope: node.scope },
                            Op::Unit { destination },
                        ],
                        RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                    ),
                    _ => unsupported("invalid while phase"),
                }
            }
            (SyntaxKind::IF_EXPR, phase) => {
                let expression = ast::IfExpr::cast(syntax).expect("checked if");
                let Some(condition) = expression.condition() else {
                    return unsupported("if condition absent");
                };
                let Some(yes) = expression.then_branch() else {
                    return unsupported("if then branch absent");
                };
                let condition = id_for(index, body, condition.syntax())?;
                let yes = id_for(index, body, yes.syntax())?;
                let no = expression
                    .else_branch()
                    .map(|branch| id_for(index, body, branch.syntax()))
                    .transpose()?;
                let condition_slot = index.nodes()[condition.0 as usize]
                    .value
                    .expect("condition value");
                let yes_slot = index.nodes()[yes.0 as usize].value.expect("then value");
                let no_slot = no.map(|no| index.nodes()[no.0 as usize].value.expect("else value"));
                let destination = node.value.expect("if value");
                let condition_scope = node
                    .condition_scope
                    .ok_or_else(|| error("if condition scope absent"))?;
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![
                            Op::ScopeEnter {
                                scope: condition_scope,
                            },
                            Op::SetHint {
                                child: condition_slot,
                                hint: HintRecipe::Primitive(PrimitiveType::Bool),
                            },
                        ],
                        RecipeExit::Next(index.pc(condition, Phase::Entry)?),
                    ),
                    Phase::Operand(0) => {
                        let mut ops = vec![Op::BranchHint {
                            parent: destination,
                            condition: condition_slot,
                            yes: Some(yes_slot),
                            no: no_slot,
                            boolean: false,
                        }];
                        if let Some(scope) = index.nodes()[condition.0 as usize].boolean_scope {
                            ops.push(Op::ScopeLeave { scope });
                        }
                        draft(
                            index,
                            key,
                            ops,
                            RecipeExit::Branch {
                                condition: condition_slot,
                                yes: index.pc(yes, Phase::Entry)?,
                                no: index.pc(id, Phase::Exit)?,
                            },
                        )
                    }
                    Phase::Operand(1) | Phase::Operand(2) => {
                        let source = if phase == Phase::Operand(1) {
                            yes_slot
                        } else {
                            let Some(no) = no_slot else {
                                return unsupported("else result absent");
                            };
                            no
                        };
                        let mut ops = Vec::new();
                        if no.is_none() {
                            ops.push(Op::CheckUnit { source });
                        }
                        ops.push(Op::ResultTransfer {
                            source,
                            destination,
                        });
                        if phase == Phase::Operand(1) {
                            ops.push(Op::ScopeLeave {
                                scope: condition_scope,
                            });
                        }
                        draft(
                            index,
                            key,
                            ops,
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        )
                    }
                    Phase::Exit => draft(
                        index,
                        key,
                        vec![Op::ScopeLeave {
                            scope: condition_scope,
                        }],
                        RecipeExit::Next(if let Some(no) = no {
                            index.pc(no, Phase::Entry)?
                        } else {
                            index.pc(id, Phase::Commit)?
                        }),
                    ),
                    Phase::Commit if no.is_none() => draft(
                        index,
                        key,
                        vec![Op::Unit { destination }],
                        RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                    ),
                    _ => unsupported("invalid if phase"),
                }
            }

            _ => unsupported("entered node phase awaits its ordered lowering task"),
        }
    }

    /// Pure lowering of one validated captured PC; separate fixed host stack frame.
    #[inline(never)]
    /// How a comparison's place operand reads its builtin value, or `None`
    /// for a value operand (including a non-local value path).
    fn comparison_operand_read(
        &self,
        index: &BodyIndex,
        body: &BodyHandle,
        operand: &ast::Expr,
    ) -> Result<Option<crate::ir::op::OperandRead>, Diagnostic> {
        use crate::ir::op::OperandRead;
        if !comparison_place_operand(operand.syntax()) {
            return Ok(None);
        }
        let ast::Expr::PathExpr(path) = operand else {
            return Ok(Some(OperandRead::Place));
        };
        if let Some(binding) = local_binding(self, index, body, path)? {
            return Ok(Some(OperandRead::Local(binding)));
        }
        let name = MacroName::local(&path.syntax().text().to_string(), self.edition).ok();
        Ok(name
            .as_ref()
            .map(|name| parameter_at(self, path.syntax(), name))
            .transpose()?
            .flatten()
            .map(OperandRead::Parameter))
    }
    fn initial_binary_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        syntax: SyntaxNode,
        numeric_admission: Option<
            &crate::frontend::runtime_macro_numeric::RuntimeMacroNumericAdmission,
        >,
        source_constraints: Option<
            &crate::frontend::binding_constraints::PreparedBindingConstraints,
        >,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let id = NodeId(key.node);
        let node = &index.nodes()[key.node as usize];
        let fallback = index.pc(id, Phase::Exit)?;
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        match (syntax.kind(), Phase::from_code(key.phase)) {
            (SyntaxKind::BIN_EXPR, phase)
                if ast::BinExpr::cast(syntax.clone())
                    .and_then(|e| e.op_token())
                    .is_some_and(|t| matches!(t.text(), "&&" | "||")) =>
            {
                let expression = ast::BinExpr::cast(syntax).expect("checked short circuit");
                let and = expression.op_token().is_some_and(|t| t.text() == "&&");
                let Some(left) = expression.lhs() else {
                    return unsupported("boolean lhs absent");
                };
                let Some(right) = expression.rhs() else {
                    return unsupported("boolean rhs absent");
                };
                let left = id_for(index, body, left.syntax())?;
                let right = id_for(index, body, right.syntax())?;
                let lhs = index.nodes()[left.0 as usize].value.expect("boolean lhs");
                let rhs = index.nodes()[right.0 as usize].value.expect("boolean rhs");
                let destination = node.value.expect("boolean result");
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![Op::SetHint {
                            child: lhs,
                            hint: HintRecipe::Primitive(PrimitiveType::Bool),
                        }],
                        RecipeExit::Next(index.pc(left, Phase::Entry)?),
                    ),
                    Phase::Operand(0) => {
                        let enter = index.pc(right, Phase::Entry)?;
                        let skip = index.pc(id, Phase::Commit)?;
                        let mut ops = vec![Op::BranchHint {
                            parent: destination,
                            condition: lhs,
                            yes: and.then_some(rhs),
                            no: (!and).then_some(rhs),
                            boolean: true,
                        }];
                        if let Some(scope) = index.nodes()[left.0 as usize].boolean_scope {
                            ops.push(Op::ScopeLeave { scope });
                        }
                        draft(
                            index,
                            key,
                            ops,
                            RecipeExit::Branch {
                                condition: lhs,
                                yes: if and { enter.clone() } else { skip.clone() },
                                no: if and { skip } else { enter },
                            },
                        )
                    }
                    Phase::Operand(1) | Phase::Commit => {
                        let mut ops = Vec::new();
                        if phase == Phase::Operand(1)
                            && let Some(scope) = index.nodes()[right.0 as usize].boolean_scope
                        {
                            ops.push(Op::ScopeLeave { scope });
                        }
                        ops.extend([
                            Op::ResultTransfer {
                                source: if phase == Phase::Commit { lhs } else { rhs },
                                destination,
                            },
                            Op::Discard { source: lhs },
                        ]);
                        draft(
                            index,
                            key,
                            ops,
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        )
                    }
                    _ => unsupported("invalid short circuit phase"),
                }
            }
            (SyntaxKind::BIN_EXPR, phase)
                if ast::BinExpr::cast(syntax.clone())
                    .and_then(|e| e.op_token())
                    .is_some_and(|t| t.text() == "=") =>
            {
                let expression = ast::BinExpr::cast(syntax).expect("checked assignment");
                let Some(right) = expression.rhs() else {
                    return unsupported("assignment RHS absent");
                };
                let right = id_for(index, body, right.syntax())?;
                let source = index.nodes()[right.0 as usize]
                    .value
                    .expect("assignment RHS value");
                let binding = if let Some(ast::Expr::PathExpr(path)) = expression.lhs() {
                    local_binding(self, index, body, &path)?
                } else {
                    None
                };
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![match binding {
                            Some(binding) => Op::LocalHint {
                                binding,
                                child: source,
                            },
                            None => Op::SetHint {
                                child: source,
                                hint: HintRecipe::None,
                            },
                        }],
                        RecipeExit::Next(index.pc(right, Phase::Entry)?),
                    ),
                    Phase::Operand(1) => {
                        let Some(binding) = binding else {
                            let Some(left) = expression.lhs() else {
                                return unsupported(
                                    "assignment destination requires place/namespace integration",
                                );
                            };
                            // A non-local path (a static) is a place, like a
                            // projection; its Exit forms the place.
                            if !matches!(
                                &left,
                                ast::Expr::IndexExpr(_)
                                    | ast::Expr::FieldExpr(_)
                                    | ast::Expr::PathExpr(_)
                            ) && !matches!(&left, ast::Expr::PrefixExpr(prefix) if prefix.op_token().is_some_and(|token| token.text() == "*"))
                            {
                                return unsupported(
                                    "assignment destination requires projected-place integration",
                                );
                            }
                            return draft(
                                index,
                                key,
                                vec![],
                                RecipeExit::Next(
                                    index.pc(id_for(index, body, left.syntax())?, Phase::Entry)?,
                                ),
                            );
                        };
                        draft(
                            index,
                            key,
                            vec![Op::AssignLocal {
                                binding,
                                source,
                                destination: node.value.expect("assignment result"),
                            }],
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        )
                    }
                    Phase::Operand(0) if binding.is_none() => {
                        let Some(left) = expression.lhs() else {
                            return unsupported("assignment LHS absent");
                        };
                        let left = id_for(index, body, left.syntax())?;
                        let place = index.nodes()[left.0 as usize]
                            .place
                            .ok_or_else(|| error("assignment LHS place absent"))?;
                        draft(
                            index,
                            key,
                            vec![Op::AssignPlace {
                                place,
                                source,
                                destination: node
                                    .value
                                    .ok_or_else(|| error("assignment result absent"))?,
                            }],
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        )
                    }
                    _ => unsupported("invalid assignment phase"),
                }
            }
            (SyntaxKind::BIN_EXPR, phase)
                if ast::BinExpr::cast(syntax.clone())
                    .and_then(|expr| expr.op_token())
                    .is_some_and(|token| {
                        matches!(
                            token.text(),
                            "+=" | "-=" | "*=" | "/=" | "%=" | "<<=" | ">>=" | "&=" | "|=" | "^="
                        )
                    }) =>
            {
                let expression = ast::BinExpr::cast(syntax).expect("checked compound assignment");
                let token = expression
                    .op_token()
                    .ok_or_else(|| error("compound operator absent"))?;
                let operator = binary_operator(
                    token
                        .text()
                        .strip_suffix('=')
                        .ok_or_else(|| error("compound operator suffix absent"))?,
                )
                .ok_or_else(|| error("compound numeric operator absent"))?;
                // A local or parameter path keeps the local compound route; any
                // other path (a static) is a place like a projection.
                let local_left = match expression.lhs() {
                    Some(ast::Expr::PathExpr(left)) => {
                        let local = local_binding(self, index, body, &left)?.is_some()
                            || MacroName::local(&left.syntax().text().to_string(), self.edition)
                                .ok()
                                .map(|name| parameter_at(self, left.syntax(), &name))
                                .transpose()?
                                .flatten()
                                .is_some();
                        local.then_some(left)
                    }
                    _ => None,
                };
                // A local keeps the local numeric route only with a primitive
                // or exact definition-numeric proof for both operands. Any
                // other local (a `for` binding, a call result, a nominal value
                // whose `AddAssign` impl the reached operator selects) is the
                // updated place, through the same route as any other place,
                // typed by the reached values.
                let untyped_local = local_left.clone();
                let local_left = match local_left {
                    Some(left) => {
                        let shift = matches!(operator, NumericBinary::Shl | NumericBinary::Shr);
                        let numeric = match (
                            compound_local_primitive(self, index, body, &left, source_constraints)?,
                            expression.rhs(),
                        ) {
                            (Some((_, left_type)), Some(right)) => compound_rhs_primitive(
                                self,
                                index,
                                body,
                                right,
                                if shift { PrimitiveType::I32 } else { left_type },
                                source_constraints,
                            )?
                            .is_some(),
                            (Some(_), None) => true,
                            (None, right) => {
                                let context =
                                    compound_local_definition_context(self, index, body, &left)?
                                        .is_some()
                                        || numeric_admission.is_some();
                                context
                                    && match right {
                                        Some(right) => {
                                            runtime_local_numeric_context::compound_right(
                                                &right, shift,
                                            )?
                                            .is_some()
                                        }
                                        None => true,
                                    }
                            }
                        };
                        numeric.then_some(left)
                    }
                    None => None,
                };
                let untyped_local = untyped_local.filter(|_| local_left.is_none());
                let Some(left) = local_left else {
                    // A projected place (`*r += 1`, `s.f -= 2`, `a[i] *= 3`):
                    // as for primitive operands, the right operand evaluates
                    // first, then the place; the operation reads and writes it.
                    let Some(left) = expression.lhs() else {
                        return unsupported("compound LHS absent");
                    };
                    let Some(right) = expression.rhs() else {
                        return unsupported("compound RHS absent");
                    };
                    let left = id_for(index, body, left.syntax())?;
                    let right = id_for(index, body, right.syntax())?;
                    return match phase {
                        Phase::Entry => draft(
                            index,
                            key,
                            vec![],
                            RecipeExit::Next(index.pc(right, Phase::Entry)?),
                        ),
                        // An untyped local is addressed by the compound
                        // operation itself (its binding's place), never moved.
                        Phase::Operand(n)
                            if node.operands.get(usize::from(n)) == Some(&right)
                                && untyped_local.is_some() =>
                        {
                            let path = untyped_local.as_ref().expect("checked untyped local");
                            let place = index.nodes()[left.0 as usize]
                                .place
                                .ok_or_else(|| error("compound place absent"))?;
                            let address = match local_binding(self, index, body, path)? {
                                Some(binding) => Op::MakePlace {
                                    binding,
                                    destination: place,
                                },
                                None => {
                                    let name = MacroName::local(
                                        &path.syntax().text().to_string(),
                                        self.edition,
                                    )?;
                                    let ordinal = parameter_at(self, path.syntax(), &name)?
                                        .ok_or_else(|| error("compound local absent"))?;
                                    Op::ParameterPlace {
                                        ordinal,
                                        destination: place,
                                    }
                                }
                            };
                            draft(
                                index,
                                key,
                                vec![
                                    address,
                                    Op::CompoundAssignPlace {
                                        operator,
                                        place,
                                        right: index.nodes()[right.0 as usize]
                                            .value
                                            .ok_or_else(|| error("compound operand absent"))?,
                                        destination: node
                                            .value
                                            .ok_or_else(|| error("compound result absent"))?,
                                    },
                                ],
                                RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                            )
                        }
                        Phase::Operand(n) if node.operands.get(usize::from(n)) == Some(&right) => {
                            draft(
                                index,
                                key,
                                vec![],
                                RecipeExit::Next(index.pc(left, Phase::Entry)?),
                            )
                        }
                        Phase::Operand(n) if node.operands.get(usize::from(n)) == Some(&left) => {
                            draft(
                                index,
                                key,
                                vec![Op::CompoundAssignPlace {
                                    operator,
                                    place: index.nodes()[left.0 as usize]
                                        .place
                                        .ok_or_else(|| error("compound place absent"))?,
                                    right: index.nodes()[right.0 as usize]
                                        .value
                                        .ok_or_else(|| error("compound operand absent"))?,
                                    destination: node
                                        .value
                                        .ok_or_else(|| error("compound result absent"))?,
                                }],
                                RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                            )
                        }
                        _ => unsupported("invalid compound place phase"),
                    };
                };
                let right = expression
                    .rhs()
                    .ok_or_else(|| error("compound RHS absent"))?;
                let shift = matches!(operator, NumericBinary::Shl | NumericBinary::Shr);
                let (binding, kind, computed_hint) = if let Some((binding, left_type)) =
                    compound_local_primitive(self, index, body, &left, source_constraints)?
                {
                    let context = if shift { PrimitiveType::I32 } else { left_type };
                    let Some(right_type) = compound_rhs_primitive(
                        self,
                        index,
                        body,
                        right.clone(),
                        context,
                        source_constraints,
                    )?
                    else {
                        return Err(Diagnostic::new(
                            "LRR-E-TYPE-CONTEXT",
                            "compound RHS requires a symbolic source view before operands".into(),
                        ));
                    };
                    (
                        binding,
                        crate::ir::op::AssignKind::Primitive {
                            left: left_type,
                            right: right_type,
                        },
                        HintRecipe::Primitive(left_type),
                    )
                } else if let Some((binding, context)) = compound_local_definition_context(
                    self, index, body, &left,
                )?
                .or(numeric_admission
                    .map(|proof| proof.context(self, body, index, key))
                    .transpose()?)
                {
                    let Some(right) = runtime_local_numeric_context::compound_right(&right, shift)?
                    else {
                        return Err(Diagnostic::new(
                                "LRR-E-TYPE-CONTEXT",
                                "compound RHS requires an exact definition numeric source before operands".into(),
                            ));
                    };
                    (
                        binding,
                        crate::ir::op::AssignKind::DefinitionNumeric { context, right },
                        HintRecipe::DefinitionNumeric(context),
                    )
                } else {
                    return Err(error(
                        "compound local route lost its numeric proof after selection",
                    ));
                };
                let shared_rhs = compound_shared_rhs(right.clone())?;
                let right_reference_hint = match (&kind, shared_rhs) {
                    (
                        crate::ir::op::AssignKind::Primitive { right: target, .. },
                        Some(reference),
                    ) => {
                        let range = reference.syntax().text_range();
                        Some(HintRecipe::Reference {
                            region: crate::traits::term::RegionTerm::Local {
                                definition: body.def_key,
                                start: u32::from(range.start()),
                                end: u32::from(range.end()),
                            },
                            mutable: false,
                            target: *target,
                        })
                    }
                    _ => None,
                };
                let left = id_for(index, body, left.syntax())?;
                let right = id_for(index, body, right.syntax())?;
                let lhs = index.nodes()[left.0 as usize]
                    .value
                    .ok_or_else(|| error("compound LHS value absent"))?;
                let rhs = index.nodes()[right.0 as usize]
                    .value
                    .ok_or_else(|| error("compound RHS value absent"))?;
                let computed = node.runtime_temporaries[0]
                    .ok_or_else(|| error("compound result temporary absent"))?;
                let result = node
                    .value
                    .ok_or_else(|| error("compound Unit result absent"))?;
                match phase {
                    Phase::Entry => {
                        // Admit the local source before evaluating either
                        // operand. An actual shared RHS then gets its reference
                        // expectation before entry; BorrowTarget forwards only
                        // the pointee context to its evaluated expression.
                        let mut operations = vec![Op::CompoundHint {
                            binding,
                            child: rhs,
                            kind,
                        }];
                        if let Some(hint) = right_reference_hint {
                            operations.push(Op::SetHint { child: rhs, hint });
                        }
                        operations.push(Op::SetHint {
                            child: computed,
                            hint: computed_hint,
                        });
                        draft(
                            index,
                            key,
                            operations,
                            RecipeExit::Next(index.pc(right, Phase::Entry)?),
                        )
                    }
                    Phase::Operand(1) => draft(
                        index,
                        key,
                        vec![],
                        RecipeExit::Next(index.pc(left, Phase::Entry)?),
                    ),
                    Phase::Operand(0) => draft(
                        index,
                        key,
                        vec![Op::NumericBinary {
                            operator,
                            left: lhs,
                            right: rhs,
                            destination: computed,
                        }],
                        RecipeExit::Next(index.pc(id, Phase::Commit)?),
                    ),
                    Phase::Commit => draft(
                        index,
                        key,
                        vec![
                            Op::AssignLocal {
                                binding,
                                source: computed,
                                destination: result,
                            },
                            Op::Discard { source: lhs },
                            Op::Discard { source: rhs },
                        ],
                        RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                    ),
                    _ => unsupported("invalid compound assignment phase"),
                }
            }
            (SyntaxKind::BIN_EXPR, phase) => {
                let expression = ast::BinExpr::cast(syntax).expect("checked binary");
                let default = super::numeric_fallback::comparison(&expression)?;
                let Some(operator) = expression
                    .op_token()
                    .and_then(|token| binary_operator(token.text()))
                else {
                    return unsupported("operator requires control/assignment integration");
                };
                let Some(left) = expression.lhs() else {
                    return unsupported("binary left operand absent");
                };
                let Some(right) = expression.rhs() else {
                    return unsupported("binary right operand absent");
                };
                let right_default =
                    super::numeric_fallback::right_operand_default(&right, operator, default)?;
                let right_expression = right.clone();
                // Place operands of a comparison (see `comparison_place_operand`).
                let left_read = self.comparison_operand_read(index, body, &left)?;
                let right_read = self.comparison_operand_read(index, body, &right)?;
                let left = id_for(index, body, left.syntax())?;
                let right = id_for(index, body, right.syntax())?;
                let left_slot = index.nodes()[left.0 as usize].value.expect("left value");
                let right_slot = index.nodes()[right.0 as usize].value.expect("right value");
                let operand = |id: NodeId, read: Option<crate::ir::op::OperandRead>| {
                    read.map(|read| {
                        let node = &index.nodes()[id.0 as usize];
                        Ok::<_, Diagnostic>(Op::ComparisonOperand {
                            place: node
                                .place
                                .ok_or_else(|| error("comparison operand place absent"))?,
                            read,
                            destination: node
                                .value
                                .ok_or_else(|| error("comparison operand value absent"))?,
                        })
                    })
                    .transpose()
                };
                let left_operand = operand(left, left_read)?;
                let right_operand = operand(right, right_read)?;
                let destination = node.value.expect("binary value");
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![super::numeric_fallback::operand_hint(
                            default,
                            destination,
                            left_slot,
                            None,
                            operator,
                        )],
                        RecipeExit::Next(index.pc(left, Phase::Entry)?),
                    ),
                    Phase::Operand(0) => draft(
                        index,
                        key,
                        left_operand
                            .into_iter()
                            .chain([super::numeric_fallback::right_operand_hint(
                                &right_expression,
                                destination,
                                right_slot,
                                Some(left_slot),
                                operator,
                                right_default,
                                key.body.definition,
                            )?])
                            .collect(),
                        RecipeExit::Next(index.pc(right, Phase::Entry)?),
                    ),
                    Phase::Operand(1) => draft(
                        index,
                        key,
                        right_operand
                            .into_iter()
                            .chain([Op::NumericBinary {
                                operator,
                                left: left_slot,
                                right: right_slot,
                                destination,
                            }])
                            .collect(),
                        RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                    ),
                    _ => unsupported("invalid binary phase"),
                }
            }

            _ => unsupported("entered node phase awaits its ordered lowering task"),
        }
    }

    /// Pure lowering of one validated captured PC; separate fixed host stack frame.
    #[inline(never)]
    fn initial_scalar_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        syntax: SyntaxNode,
        type_name: Option<&crate::frontend::primitive_type_names::PrimitiveTypeNameAdmission>,
        type_use: Option<&crate::frontend::entered_type_goals::EnteredTypeGoal>,
        cast_coercion: Option<&let_annotations::LetAnnotation>,
        source_constraints: Option<
            &crate::frontend::binding_constraints::PreparedBindingConstraints,
        >,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let id = NodeId(key.node);
        let node = &index.nodes()[key.node as usize];
        let fallback = index.pc(id, Phase::Exit)?;
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        match (syntax.kind(), Phase::from_code(key.phase)) {
            (SyntaxKind::CAST_EXPR, phase) => {
                let expression = ast::CastExpr::cast(syntax).expect("checked cast");
                let Some(child) = expression.expr() else {
                    return unsupported("cast operand absent");
                };
                // A coercion cast: the operand is evaluated under the written
                // target Type, as a `let` initializer under that annotation,
                // and its value is the cast's value.
                if let Some(coercion) = cast_coercion
                    && expression.ty().is_some_and(|ty| {
                        crate::syntax::ast_adapter::range(ty.syntax().text_range())
                            == coercion.range
                    })
                {
                    // E-R5-008: a written raw-pointer or fn-pointer target. An
                    // unsuffixed integer literal operand is `usize` (rustc's
                    // `0 as *const T`); every other operand is evaluated
                    // as written and then converted by `Op::PointerCast`.
                    let pointer_default = if coercion.written.is_pointer() {
                        super::numeric_fallback::cast_operand(
                            &child,
                            crate::ir::op::PrimitiveType::Usize,
                        )?
                    } else {
                        None
                    };
                    let child = id_for(index, body, child.syntax())?;
                    let source = index.nodes()[child.0 as usize]
                        .value
                        .ok_or_else(|| error("cast operand value absent"))?;
                    let destination = node.value.ok_or_else(|| error("cast value absent"))?;
                    if coercion.written.is_pointer() {
                        return match phase {
                            Phase::Entry => draft(
                                index,
                                key,
                                vec![Op::SetHint {
                                    child: source,
                                    hint: pointer_default
                                        .map(HintRecipe::Primitive)
                                        .unwrap_or(HintRecipe::None),
                                }],
                                RecipeExit::Next(index.pc(child, Phase::Entry)?),
                            ),
                            Phase::Operand(0) => draft(
                                index,
                                key,
                                vec![Op::PointerCast {
                                    source,
                                    target: std::sync::Arc::clone(&coercion.written),
                                    destination,
                                }],
                                RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                            ),
                            _ => unsupported("invalid pointer cast phase"),
                        };
                    }
                    return match phase {
                        Phase::Entry => draft(
                            index,
                            key,
                            vec![Op::SetHint {
                                child: source,
                                hint: HintRecipe::Written(std::sync::Arc::clone(&coercion.written)),
                            }],
                            RecipeExit::Next(index.pc(child, Phase::Entry)?),
                        ),
                        Phase::Operand(0) => draft(
                            index,
                            key,
                            vec![Op::ResultTransfer {
                                source,
                                destination,
                            }],
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        ),
                        _ => unsupported("invalid cast coercion phase"),
                    };
                }
                // The common source Type goal owns currentness/name selection.
                // The historical unsealed route retains its old safety refusals.
                let target = if let Some(proof) = type_use {
                    let ty = expression
                        .ty()
                        .ok_or_else(|| error("cast Type syntax absent"))?;
                    proof.primitive_for(
                        index,
                        key,
                        crate::syntax::ast_adapter::range(ty.syntax().text_range()),
                    )?
                } else {
                    let Some(target) = primitive_cast_target::resolve(
                        self,
                        index,
                        expression.ty(),
                        type_name.map(|proof| (key, proof)),
                    )?
                    else {
                        return unsupported("cast target requires symbolic type resolution");
                    };
                    target
                };
                let default = super::numeric_fallback::cast_operand(&child, target)?;
                let child = id_for(index, body, child.syntax())?;
                let source = index.nodes()[child.0 as usize]
                    .value
                    .expect("cast operand value");
                let destination = node.value.expect("cast value");
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![Op::SetHint {
                            child: source,
                            hint: default
                                .map(HintRecipe::Primitive)
                                .unwrap_or(HintRecipe::None),
                        }],
                        RecipeExit::Next(index.pc(child, Phase::Entry)?),
                    ),
                    Phase::Operand(0) => draft(
                        index,
                        key,
                        vec![Op::PrimitiveCast {
                            source,
                            target,
                            destination,
                        }],
                        RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                    ),
                    _ => unsupported("invalid cast phase"),
                }
            }

            (SyntaxKind::LITERAL, Phase::Entry) => {
                let literal = ast::Literal::cast(syntax).expect("checked literal");
                thunks::literal(
                    index,
                    key.clone(),
                    {
                        let mut recipe = literal_recipe(&literal, index.span(id)?)?;
                        if recipe.kind == LiteralKind::String {
                            recipe.decoded = Some(DecodedLiteral::String(
                                string_literal::PreparedStringLiteral::capture(
                                    self,
                                    body,
                                    index,
                                    key,
                                    &literal,
                                    node.value.expect("literal value"),
                                )?,
                            ));
                        }
                        recipe
                    },
                    // The literal's own inference variable, resolved by the
                    // common source constraints; otherwise its reached parent
                    // hint alone selects the representation.
                    match source_constraints {
                        Some(proof) => proof.literal_hint(index, key)?.unwrap_or(HintRecipe::None),
                        None => HintRecipe::None,
                    },
                    node.value.expect("literal value"),
                    return_pc(index, body, literal.syntax())?,
                )
            }

            _ => unsupported("entered node phase awaits its ordered lowering task"),
        }
    }

    /// Keep reached place lowering outside the general phase dispatcher's fixed
    /// stack frame. This is still pure lowering of one exact captured source PC.
    #[inline(never)]
    fn initial_place_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        syntax: SyntaxNode,
        source_constraints: Option<
            &crate::frontend::binding_constraints::PreparedBindingConstraints,
        >,
        value_goal: Option<&crate::frontend::source_value_goals::EnteredValueGoal>,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let id = NodeId(key.node);
        let node = &index.nodes()[key.node as usize];
        let fallback = index.pc(id, Phase::Exit)?;
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        match (syntax.kind(), Phase::from_code(key.phase)) {
            (SyntaxKind::REF_EXPR, phase) => {
                let expression = ast::RefExpr::cast(syntax).expect("checked borrow");
                if expression.raw_token().is_some() {
                    return unsupported("raw pointer borrow is outside the live profile");
                }
                let Some(operand) = expression.expr() else {
                    return unsupported("borrow operand absent");
                };
                let access = if expression.mut_token().is_some() {
                    crate::ir::op::BorrowAccess::Mutable
                } else {
                    crate::ir::op::BorrowAccess::Shared
                };
                // A parameter or non-local value path (unit value, constant)
                // publishes its own place (ParameterPlace, or a temporary at
                // the path's Exit); only a local binding is borrowed directly.
                let local_path = match &operand {
                    ast::Expr::PathExpr(path) => local_binding(self, index, body, path)?.is_some(),
                    _ => false,
                };
                if !local_path {
                    let operand_is_place = intrinsic_place_receiver(&operand);
                    // A non-local value path that is not a parameter (a unit
                    // variant, a constant) evaluates under the borrow's
                    // target context like any other rvalue operand.
                    let hinted = !operand_is_place
                        || match &operand {
                            ast::Expr::PathExpr(path) => value_path_operand(self, path)?,
                            _ => false,
                        };
                    let child = id_for(index, body, operand.syntax())?;
                    let place = index.nodes()[child.0 as usize]
                        .place
                        .ok_or_else(|| error("borrowed operand place absent"))?;
                    return match phase {
                        Phase::Entry => draft(
                            index,
                            key,
                            if !hinted {
                                vec![]
                            } else {
                                vec![Op::SetHint {
                                    child: index.nodes()[child.0 as usize]
                                        .value
                                        .ok_or_else(|| error("borrow operand value absent"))?,
                                    hint: HintRecipe::BorrowTarget {
                                        reference: node
                                            .value
                                            .ok_or_else(|| error("borrow result absent"))?,
                                    },
                                }]
                            },
                            RecipeExit::Next(index.pc(child, Phase::Entry)?),
                        ),
                        // rustc promotes a shared borrow of a constant
                        // expression to a `'static` place; any other
                        // temporary keeps its one owning cell in its exact
                        // annotated scope.
                        Phase::Operand(0)
                            if !operand_is_place
                                && matches!(access, crate::ir::op::BorrowAccess::Shared)
                                && (promotable_constant(&operand)
                                    || extended_to_initializer_root(index, child)) =>
                        {
                            draft(
                                index,
                                key,
                                vec![Op::PromotedPlace {
                                    source: index.nodes()[child.0 as usize]
                                        .value
                                        .ok_or_else(|| error("temporary borrow value absent"))?,
                                    destination: place,
                                }],
                                RecipeExit::Next(index.pc(node.id, Phase::Commit)?),
                            )
                        }
                        // Preserve the completed operand's one owning cell.
                        // The annotation pass already assigns its exact scope;
                        // forming this checked address changes no ownership.
                        Phase::Operand(0) if !operand_is_place => draft(
                            index,
                            key,
                            vec![Op::ValuePlace {
                                source: index.nodes()[child.0 as usize]
                                    .value
                                    .ok_or_else(|| error("temporary borrow value absent"))?,
                                destination: place,
                            }],
                            RecipeExit::Next(index.pc(node.id, Phase::Commit)?),
                        ),
                        Phase::Operand(0) if operand_is_place => draft(
                            index,
                            key,
                            vec![Op::Borrow {
                                place,
                                access,
                                destination: node
                                    .value
                                    .ok_or_else(|| error("projected reference result absent"))?,
                            }],
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        ),
                        Phase::Commit if !operand_is_place => draft(
                            index,
                            key,
                            vec![Op::Borrow {
                                place,
                                access,
                                destination: node
                                    .value
                                    .ok_or_else(|| error("temporary reference result absent"))?,
                            }],
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        ),
                        _ => unsupported("invalid projected or temporary borrow phase"),
                    };
                }
                if phase != Phase::Entry {
                    return unsupported("invalid local borrow phase");
                }
                let ast::Expr::PathExpr(path) = operand else {
                    return Err(error("local borrow discriminator changed"));
                };
                let Some(binding) = local_binding(self, index, body, &path)? else {
                    return unsupported("borrow requires reached local place resolution");
                };
                let place = node
                    .place
                    .ok_or_else(|| error("borrow place slot absent"))?;
                let access = if expression.mut_token().is_some() {
                    crate::ir::op::BorrowAccess::Mutable
                } else {
                    crate::ir::op::BorrowAccess::Shared
                };
                draft(
                    index,
                    key,
                    vec![
                        Op::MakePlace {
                            binding,
                            destination: place,
                        },
                        Op::Borrow {
                            place,
                            access,
                            destination: node
                                .value
                                .ok_or_else(|| error("borrow value slot absent"))?,
                        },
                    ],
                    RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                )
            }
            (SyntaxKind::FIELD_EXPR, phase) => {
                let expression = ast::FieldExpr::cast(syntax).expect("checked field");
                let Some(base) = expression.expr() else {
                    return unsupported("field base absent");
                };
                let base_is_place = intrinsic_place_receiver(&base);
                let base_expression = base.clone();
                let Some(member) = expression.name_ref() else {
                    return unsupported("field selector absent");
                };
                let field = member.text().parse::<u32>().ok();
                if field.is_none() && member.text().bytes().all(|byte| byte.is_ascii_digit()) {
                    return unsupported("tuple field ordinal exceeds u32");
                }
                let base = id_for(index, body, base.syntax())?;
                let base_place = index.nodes()[base.0 as usize]
                    .place
                    .ok_or_else(|| error("tuple base place absent"))?;
                let place = node.place.ok_or_else(|| error("tuple place absent"))?;
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![],
                        RecipeExit::Next(index.pc(base, Phase::Entry)?),
                    ),
                    Phase::Operand(0) if !base_is_place => draft(
                        index,
                        key,
                        vec![{
                            let source = index.nodes()[base.0 as usize]
                                .value
                                .ok_or_else(|| error("temporary field receiver value absent"))?;
                            if promotable_constant(&base_expression)
                                && index.shared_borrow_owner(base).is_some()
                            {
                                Op::PromotedPlace {
                                    source,
                                    destination: base_place,
                                }
                            } else {
                                Op::ValuePlace {
                                    source,
                                    destination: base_place,
                                }
                            }
                        }],
                        RecipeExit::Next(index.pc(node.id, Phase::Commit)?),
                    ),
                    Phase::Operand(0) | Phase::Commit => {
                        let mut ops = vec![match field {
                            Some(field) => Op::ProjectTuple {
                                base: base_place,
                                field,
                                destination: place,
                            },
                            None => Op::ProjectField {
                                base: base_place,
                                destination: place,
                            },
                        }];
                        if !place_context(expression.syntax()) {
                            ops.push(Op::ReadPlace {
                                place,
                                destination: node
                                    .value
                                    .ok_or_else(|| error("tuple result absent"))?,
                            });
                        }
                        draft(
                            index,
                            key,
                            ops,
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        )
                    }
                    _ => unsupported("invalid tuple projection phase"),
                }
            }
            (SyntaxKind::INDEX_EXPR, phase) => {
                let expression = ast::IndexExpr::cast(syntax).expect("checked index");
                let (Some(base), Some(evaluated)) = (expression.base(), expression.index()) else {
                    return unsupported("index operand absent");
                };
                let base_is_place = intrinsic_place_receiver(&base);
                let base_expression = base.clone();
                // `base[a..b]` selects `Index<Range*<usize>>`: the range's
                // endpoints, not the range value, take the usize context.
                let range_bounds = match &evaluated {
                    ast::Expr::RangeExpr(range) => {
                        use ast::RangeItem;
                        Some(
                            [range.start(), range.end()]
                                .into_iter()
                                .flatten()
                                .map(|bound| id_for(index, body, bound.syntax()))
                                .collect::<Result<Vec<_>, _>>()?,
                        )
                    }
                    _ => None,
                };
                let base = id_for(index, body, base.syntax())?;
                let evaluated = id_for(index, body, evaluated.syntax())?;
                let base_place = index.nodes()[base.0 as usize]
                    .place
                    .ok_or_else(|| error("index base place absent"))?;
                let index_value = index.nodes()[evaluated.0 as usize]
                    .value
                    .ok_or_else(|| error("index value absent"))?;
                let place = node.place.ok_or_else(|| error("index place absent"))?;
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![],
                        RecipeExit::Next(index.pc(base, Phase::Entry)?),
                    ),
                    Phase::Operand(0) => {
                        let mut ops = Vec::new();
                        if !base_is_place {
                            let source = index.nodes()[base.0 as usize]
                                .value
                                .ok_or_else(|| error("temporary index receiver value absent"))?;
                            ops.push(
                                if promotable_constant(&base_expression)
                                    && index.shared_borrow_owner(base).is_some()
                                {
                                    Op::PromotedPlace {
                                        source,
                                        destination: base_place,
                                    }
                                } else {
                                    Op::ValuePlace {
                                        source,
                                        destination: base_place,
                                    }
                                },
                            );
                        }
                        match &range_bounds {
                            Some(bounds) => {
                                for bound in bounds {
                                    ops.push(Op::SetHint {
                                        child: index.nodes()[bound.0 as usize].value.ok_or_else(
                                            || error("range index bound value absent"),
                                        )?,
                                        hint: HintRecipe::Primitive(PrimitiveType::Usize),
                                    });
                                }
                            }
                            None => ops.push(Op::SetHint {
                                child: index_value,
                                hint: HintRecipe::IndexOperand { base: base_place },
                            }),
                        }
                        draft(
                            index,
                            key,
                            ops,
                            RecipeExit::Next(index.pc(evaluated, Phase::Entry)?),
                        )
                    }
                    Phase::Operand(1) => {
                        let mut ops = vec![Op::ProjectIndex {
                            base: base_place,
                            index: index_value,
                            destination: place,
                        }];
                        if !place_context(expression.syntax()) {
                            ops.push(Op::ReadPlace {
                                place,
                                destination: node
                                    .value
                                    .ok_or_else(|| error("index result absent"))?,
                            });
                        }
                        draft(
                            index,
                            key,
                            ops,
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        )
                    }
                    _ => unsupported("invalid index projection phase"),
                }
            }
            (SyntaxKind::PREFIX_EXPR, phase) => {
                let expression = ast::PrefixExpr::cast(syntax).expect("checked prefix");
                let Some(child) = expression.expr() else {
                    return unsupported("prefix operand absent");
                };
                if expression
                    .op_token()
                    .is_some_and(|token| token.text() == "*")
                {
                    let child_is_place = intrinsic_place_receiver(&child);
                    let child = id_for(index, body, child.syntax())?;
                    let reference = index.nodes()[child.0 as usize]
                        .place
                        .ok_or_else(|| error("reference receiver place absent"))?;
                    let place = node
                        .place
                        .ok_or_else(|| error("dereference place absent"))?;
                    return match phase {
                        Phase::Entry => draft(
                            index,
                            key,
                            vec![],
                            RecipeExit::Next(index.pc(child, Phase::Entry)?),
                        ),
                        Phase::Operand(0) if !child_is_place => draft(
                            index,
                            key,
                            vec![Op::ValuePlace {
                                source: index.nodes()[child.0 as usize]
                                    .value
                                    .ok_or_else(|| error("computed reference value absent"))?,
                                destination: reference,
                            }],
                            RecipeExit::Next(index.pc(id, Phase::Commit)?),
                        ),
                        Phase::Operand(0) | Phase::Commit
                            if (phase == Phase::Operand(0)) == child_is_place =>
                        {
                            let mut ops = vec![Op::DerefPlace {
                                reference,
                                destination: place,
                            }];
                            if !place_context(expression.syntax()) {
                                ops.push(Op::ReadPlace {
                                    place,
                                    destination: node
                                        .value
                                        .ok_or_else(|| error("dereference result absent"))?,
                                });
                            }
                            draft(
                                index,
                                key,
                                ops,
                                RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                            )
                        }
                        _ => unsupported("invalid dereference phase"),
                    };
                }
                let operator = match expression
                    .op_token()
                    .map(|token| token.text().to_owned())
                    .as_deref()
                {
                    Some("-") => NumericUnary::Neg,
                    Some("!") => NumericUnary::Not,
                    _ => return unsupported("prefix operator requires place integration"),
                };
                if phase == Phase::Entry && operator == NumericUnary::Neg {
                    let mut selected = child.clone();
                    while let ast::Expr::ParenExpr(group) = selected {
                        let Some(inner) = group.expr() else {
                            return unsupported("empty negative literal group");
                        };
                        selected = inner;
                    }
                    if let ast::Expr::Literal(literal) = selected
                        && !["u8", "u16", "u32", "u64", "usize", "u128", "f32", "f64"]
                            .iter()
                            .any(|suffix| literal.syntax().text().to_string().ends_with(suffix))
                        && literal
                            .syntax()
                            .children_with_tokens()
                            .any(|token| token.kind() == SyntaxKind::INT_NUMBER)
                    {
                        return thunks::literal(
                            index,
                            key.clone(),
                            LiteralRecipe {
                                decoded: Some(super::literal_decode::integer(
                                    &ast::IntNumber::cast(literal.token()).ok_or_else(|| {
                                        error("negative integer token cast failed")
                                    })?,
                                    true,
                                )),
                                kind: LiteralKind::NegativeInteger,
                                spelling: expression.syntax().text().to_string(),
                                source: index.span(id)?,
                            },
                            // The negated literal's own inference variable
                            // (negation keeps its Type), when resolved.
                            match source_constraints {
                                Some(proof) => {
                                    let literal = PcKey {
                                        body: key.body.clone(),
                                        node: id_for(index, body, literal.syntax())?.0,
                                        phase: 0,
                                    };
                                    proof
                                        .literal_hint(index, &literal)?
                                        .unwrap_or(HintRecipe::None)
                                }
                                None => HintRecipe::None,
                            },
                            node.value.expect("negative literal value"),
                            return_pc(index, body, expression.syntax())?,
                        );
                    }
                }
                let child = id_for(index, body, child.syntax())?;
                let source = index.nodes()[child.0 as usize]
                    .value
                    .expect("prefix operand value");
                let destination = node.value.expect("prefix value");
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![Op::NumericHint {
                            parent: destination,
                            child: source,
                            left: None,
                            operator: None,
                        }],
                        RecipeExit::Next(index.pc(child, Phase::Entry)?),
                    ),
                    Phase::Operand(0) => draft(
                        index,
                        key,
                        vec![Op::NumericUnary {
                            operator,
                            source,
                            destination,
                        }],
                        RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                    ),
                    _ => unsupported("invalid prefix phase"),
                }
            }
            (SyntaxKind::PAREN_EXPR, phase) => {
                let expression = ast::ParenExpr::cast(syntax).expect("checked group");
                let Some(child) = expression.expr() else {
                    return unsupported("empty grouped expression");
                };
                let carries_place =
                    place_context(expression.syntax()) && intrinsic_place_receiver(&child);
                let child = id_for(index, body, child.syntax())?;
                if carries_place {
                    return match phase {
                        Phase::Entry => draft(
                            index,
                            key,
                            vec![],
                            RecipeExit::Next(index.pc(child, Phase::Entry)?),
                        ),
                        Phase::Operand(0) => draft(
                            index,
                            key,
                            vec![Op::PlaceTransfer {
                                source: index.nodes()[child.0 as usize]
                                    .place
                                    .ok_or_else(|| error("parenthesized receiver place absent"))?,
                                destination: node
                                    .place
                                    .ok_or_else(|| error("parenthesized place absent"))?,
                            }],
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        ),
                        _ => unsupported("invalid parenthesized place phase"),
                    };
                }
                let source = index.nodes()[child.0 as usize]
                    .value
                    .expect("group child value");
                let destination = node.value.expect("group value");
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![Op::NumericHint {
                            parent: destination,
                            child: source,
                            left: None,
                            operator: None,
                        }],
                        RecipeExit::Next(index.pc(child, Phase::Entry)?),
                    ),
                    Phase::Operand(0) => draft(
                        index,
                        key,
                        vec![Op::ResultTransfer {
                            source,
                            destination,
                        }],
                        RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                    ),
                    _ => unsupported("invalid group phase"),
                }
            }
            // SourceConstant publishes at the path's own Exit PC. Continue
            // through the same original AST parent/operand relation as other
            // path reads; this phase performs no name lookup or value synthesis.
            (SyntaxKind::PATH_EXPR, Phase::Exit) => {
                // Only non-local value paths reach this Exit; one that a
                // compound assignment updates (a static) is addressed in place.
                // A comparison reads a non-local value path's fresh value as
                // an ordinary operand (`OperandRead` covers locals and places).
                let operations = if (place_context(&syntax) && !comparison_place_operand(&syntax))
                    || compound_assignment_lhs(&syntax)
                {
                    // The source constant already owns a fresh ordinary value
                    // cell. Form its location without reading/copying/moving it;
                    // indexing/borrowing retains the original cleanup owner.
                    vec![Op::ValuePlace {
                        source: node
                            .value
                            .ok_or_else(|| error("constant receiver value absent"))?,
                        destination: node
                            .place
                            .ok_or_else(|| error("constant receiver place absent"))?,
                    }]
                } else {
                    Vec::new()
                };
                draft(
                    index,
                    key,
                    operations,
                    RecipeExit::Next(return_pc(index, body, &syntax)?),
                )
            }
            (SyntaxKind::PATH_EXPR, Phase::Entry) => {
                let path = ast::PathExpr::cast(syntax).expect("checked path");
                use crate::frontend::source_value_goals::LocalValueAccess;
                let access = if let Some(goal) = value_goal {
                    goal.indexed_local_access(index, key)?
                } else if let Some(binding) = local_binding(self, index, body, &path)? {
                    Some(LocalValueAccess::Binding(binding))
                } else {
                    // Held unsealed route: published source uses only its
                    // retained lexical selection above, including parameters.
                    let name =
                        MacroName::local(&path.syntax().text().to_string(), self.edition).ok();
                    name.as_ref()
                        .map(|name| parameter_at(self, path.syntax(), name))
                        .transpose()?
                        .flatten()
                        .map(LocalValueAccess::Parameter)
                };
                if let Some(LocalValueAccess::Parameter(ordinal)) = access {
                    return draft(
                        index,
                        key,
                        vec![if place_context(path.syntax()) {
                            Op::ParameterPlace {
                                ordinal,
                                destination: node
                                    .place
                                    .ok_or_else(|| error("parameter receiver place absent"))?,
                            }
                        } else {
                            Op::ReadParameter {
                                ordinal,
                                destination: node
                                    .value
                                    .ok_or_else(|| error("parameter path value absent"))?,
                            }
                        }],
                        RecipeExit::Next(return_pc(index, body, path.syntax())?),
                    );
                }
                let Some(LocalValueAccess::Binding(binding)) = access else {
                    // A non-local value path is selected by the dispatcher
                    // through the ordinary Value namespace (constants were
                    // already routed to SourceConstant). Its value continues
                    // at this path's own Exit PC, exactly like a constant: a
                    // place context addresses that fresh value there.
                    return match self.written_value_path(&path, index.cfg())? {
                        value_paths::WrittenValuePath::Spelled(spelled) => draft(
                            index,
                            key,
                            vec![Op::ValuePath {
                                path: spelled,
                                destination: node
                                    .value
                                    .ok_or_else(|| error("value path result absent"))?,
                            }],
                            RecipeExit::Next(index.pc(id, Phase::Exit)?),
                        ),
                        value_paths::WrittenValuePath::Refused(reason) => unsupported(reason),
                    };
                };
                draft(
                    index,
                    key,
                    vec![if place_context(path.syntax()) {
                        Op::MakePlace {
                            binding,
                            destination: node
                                .place
                                .ok_or_else(|| error("local receiver place absent"))?,
                        }
                    } else {
                        Op::ReadLocal {
                            binding,
                            destination: node.value.expect("path value"),
                        }
                    }],
                    RecipeExit::Next(return_pc(index, body, path.syntax())?),
                )
            }
            _ => unsupported("entered node phase awaits its ordered lowering task"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        runtime::cell::{CellState, GuestStore},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::body_index::{BodyIndexLimits, ScopeId},
        vm::{
            frame::{Frame, FrameStatus},
            step::{StepOutcome, step},
        },
    };
    #[test]
    fn rustc_const_if_else_phase_cannot_be_replaced_by_initializer_return() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/control-flow/basics.rs"
        );
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(Digest([156; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let declaration = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "ABS_DIFF")
            .unwrap();
        let handle = declaration.body.as_ref().unwrap();
        let body = parsed.index_body(handle, Default::default()).unwrap();
        let else_phase = body.pc(NodeId(0), Phase::Exit).unwrap();
        let returned = body.initializer_return_pc().unwrap().unwrap();
        assert_ne!(else_phase, returned);
        let branch = parsed.initial_thunk(handle, &body, &else_phase).unwrap();
        assert!(
            matches!(branch.recipe().operations.as_ref(), [Op::ScopeLeave {scope}] if Some(*scope) == body.nodes()[0].condition_scope)
        );
        let RecipeExit::Next(else_entry) = &branch.recipe().exit else {
            panic!("original else phase must select its original else block")
        };
        assert_ne!(else_entry, &returned);
        let else_node = &body.nodes()[else_entry.node as usize];
        let else_text = &ORIGINAL[(handle.range.start + else_node.range.start) as usize
            ..(handle.range.start + else_node.range.end) as usize];
        assert_eq!(else_text.trim(), "{\n    X - Y\n}");
        let root_return = parsed.initial_thunk(handle, &body, &returned).unwrap();
        assert!(
            matches!(root_return.recipe().exit, RecipeExit::Control(ControlRecipe::Return {value}) if Some(value) == body.nodes()[0].value)
        );
        // Exact original conditional phases only: no X/Y evaluation, branch
        // execution, declared const type fulfillment or full stock run-pass.
    }

    #[test]
    fn rustc_const_literal_root_continuation_keeps_unresolved_context_before_return() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-as-fn.rs"
        );
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(Digest([155; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let declaration = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "FOO")
            .unwrap();
        let handle = declaration.body.as_ref().unwrap();
        let body = parsed.index_body(handle, Default::default()).unwrap();
        let entry = body.pc(NodeId(0), Phase::Entry).unwrap();
        let final_pc = body.initializer_return_pc().unwrap().unwrap();
        assert_eq!(
            body.nodes().last().unwrap().role,
            NodeRole::InitializerReturn
        );
        assert_eq!(body.nodes()[final_pc.node as usize].parent, Some(NodeId(0)));
        assert!(body.pc(NodeId(final_pc.node), Phase::Exit).is_err());
        let node_limit = BodyIndexLimits {
            nodes: u32::try_from(body.nodes().len() - 1).unwrap(),
            ..Default::default()
        };
        let node_error = parsed.index_body(handle, node_limit).unwrap_err();
        assert_eq!(node_error.code, "LRR-E-BODY-INDEX-LIMIT");
        assert_eq!(node_error.message, "initializer return node budget");
        let slot_limit = BodyIndexLimits {
            slots: body.slot_count() - 2,
            ..Default::default()
        };
        let slot_error = parsed.index_body(handle, slot_limit).unwrap_err();
        assert_eq!(slot_error.code, "LRR-E-BODY-INDEX-LIMIT");
        assert_eq!(slot_error.message, "initializer return slot budget");

        let literal = parsed.initial_thunk(handle, &body, &entry).unwrap();
        assert!(matches!(&literal.recipe().exit, RecipeExit::Next(pc) if pc == &final_pc));
        let returned = parsed.initial_thunk(handle, &body, &final_pc).unwrap();
        assert!(
            matches!(returned.recipe().operations.as_ref(), [Op::Unit { destination }] if Some(*destination) == body.nodes()[final_pc.node as usize].runtime_temporaries[0])
        );
        assert!(
            matches!(returned.recipe().exit, RecipeExit::Control(ControlRecipe::Return {value}) if Some(value) == body.nodes()[0].value)
        );
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut frame = Frame::new(body.clone(), entry, &mut store).unwrap();
        // Explicit component setup only. Source const evaluation and the usize
        // declaration context have not been bound by a public evaluator.
        frame.enter(ScopeId(0)).unwrap();
        assert!(
            matches!(step(&mut frame, &mut store, &literal).unwrap(), StepOutcome::Next(pc) if pc == final_pc)
        );
        let cell = frame.cell(body.nodes()[0].value.unwrap()).unwrap();
        let before = store.state(cell).unwrap();
        assert!(matches!(before, CellState::Live(value) if value.kind == 11 && value.type_id == 0));
        let fuel = frame.fuel.snapshot().used;
        let error = step(&mut frame, &mut store, &returned).unwrap_err();
        assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
        assert_eq!(frame.status(), FrameStatus::Faulted);
        // The final phase executes its mandatory Unit opcode, then refuses
        // the Return exit before charging that exit or moving the literal.
        assert_eq!(frame.fuel.snapshot().used, fuel + 1);
        assert_eq!(store.state(cell).unwrap(), before);
        assert_eq!(frame.counters.operations_started, 2);
        assert_eq!(frame.counters.operations_completed, 2);
        // No numeric default, invented usize witness, public constant evaluator,
        // execution of invalid FOO(), or complete compile-fail acceptance.
    }
}

#[cfg(test)]
#[path = "integer_written_spelling_tests.rs"]
mod integer_written_spelling_tests;

impl ParsedFile {
    /// Require the exact entered namespace context whenever lowering admitted a
    /// lexical function spelling. This guard cannot select a callable itself.
    pub(crate) fn entered_call_requires_lexical_context(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        expected_name: &str,
    ) -> Result<bool, Diagnostic> {
        // The probe is a conservative syntactic superset (any same-named
        // block item or import "may bind"; the entered goal decides exactly),
        // so it applies equally to assembled or macro-generated syntax:
        // answering `false` there would hand the name to the module route.
        if body.key() != *index.body() || key.body != body.key() {
            return Err(error("lexical dispatch caller body identity differs"));
        }
        if index.pc(NodeId(key.node), Phase::from_code(key.phase))? != *key {
            return Err(error("lexical dispatch exact PC differs"));
        }
        let node = &index.nodes()[key.node as usize];
        // A qualified callee that is no ordinary call (a lowered range or
        // other language call) is never a block-lexical path.
        if expected_name.contains("::") && node.role != NodeRole::Expression(ExpressionKind::Call) {
            return Ok(false);
        }
        // A verified assertion predicate CallSeal belongs to its Macro PC,
        // whose operands are opaque token trees, not indexed CallExpr nodes.
        // RuntimeMacroPlan already requires the exact original predicate and
        // refuses lexical shadowing before preparing this module-call recipe.
        if node.role == NodeRole::Expression(ExpressionKind::Macro) {
            if expected_name.contains("::") {
                return Ok(false);
            }
            self.require_module_predicate_call(body, index, key, expected_name)?;
            return Ok(false);
        }
        if node.role != NodeRole::Expression(ExpressionKind::Call) {
            return Err(error("lexical dispatch requires an ordinary call node"));
        }
        let start = body
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| error("lexical dispatch source start overflow"))?;
        let end = body
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| error("lexical dispatch source end overflow"))?;
        let root = self.entered_body_root(body)?;
        let call = root
            .covering_element(ra_ap_syntax::TextRange::new(start.into(), end.into()))
            .ancestors()
            .find_map(|syntax| {
                let call = ast::CallExpr::cast(syntax)?;
                let range = call.syntax().text_range();
                (u32::from(range.start()) == start && u32::from(range.end()) == end).then_some(call)
            })
            .ok_or_else(|| error("lexical dispatch exact call syntax absent"))?;
        let Some(ast::Expr::PathExpr(path)) = call.expr() else {
            return Err(error("lexical dispatch direct source path absent"));
        };
        // `Root::member`: lexical only if a block item or import may bind
        // its root; the entered goal then decides whether it actually does.
        if expected_name.contains("::") {
            let Some(written) = path.path() else {
                return Ok(false);
            };
            return Ok(match Self::lexical_root_name(&written) {
                Some((root, Some(member)))
                    if format!("{}::{}", root.text(), member.text()) == expected_name =>
                {
                    self.lexical_binder_may_bind(&path, index.cfg())?
                }
                _ => false,
            });
        }
        let name = path
            .path()
            .and_then(|path| path.segment())
            .and_then(|segment| segment.name_ref())
            .ok_or_else(|| error("lexical dispatch source name absent"))?;
        if MacroName::parse(name.text(), self.edition)?
            != MacroName::parse(expected_name, self.edition)?
        {
            return Err(error("lexical dispatch recipe name differs from source"));
        }
        // Any block item or block-level import that may bind the callee's
        // name: functions, tuple-struct and variant constructors, imported
        // functions. The dispatcher selects it through its lexical Value goal.
        Ok(self.lexical_function_recipe_candidate(&path, index.cfg())?
            || self.lexical_binder_may_bind(&path, index.cfg())?)
    }
    fn lexical_function_recipe_candidate(
        &self,
        path: &ast::PathExpr,
        cfg: &crate::source::snapshot::CfgSet,
    ) -> Result<bool, Diagnostic> {
        let Some(p) = path.path() else {
            return Ok(false);
        };
        if p.qualifier().is_some() {
            return Ok(false);
        }
        let Some(name) = p.segment().and_then(|part| part.name_ref()) else {
            return Ok(false);
        };
        let wanted = MacroName::parse(name.text(), self.edition)?;
        let mut work = 0usize;
        for scope in path.syntax().ancestors() {
            let Some(list) = ast::StmtList::cast(scope) else {
                continue;
            };
            for statement in list.statements() {
                work = work
                    .checked_add(1)
                    .ok_or_else(|| error("lexical function recipe source work budget"))?;
                let ast::Stmt::Item(ast::Item::Fn(function)) = statement else {
                    continue;
                };
                let (enabled, attributed) = statement_cfg(function.syntax(), cfg)?;
                if enabled
                    && !attributed
                    && function.name().is_some_and(|name| {
                        MacroName::parse(name.text(), self.edition).is_ok_and(|name| name == wanted)
                    })
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
#[path = "scalar_pattern_owner_scope_tests.rs"]
mod scalar_pattern_owner_scope_tests;
