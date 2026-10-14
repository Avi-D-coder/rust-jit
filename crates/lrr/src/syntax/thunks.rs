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
            DecodedLiteral, HintRecipe, LiteralKind, LiteralRecipe, NumericBinary, NumericUnary,
            Op, PrimitiveType,
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
#[path = "literal_match.rs"]
mod literal_match;
#[path = "local_numeric_constraints.rs"]
mod local_numeric_constraints;
#[path = "primitive_cast_target.rs"]
mod primitive_cast_target;

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
        .nodes()
        .iter()
        .find(|node| {
            node.range.start == start
                && node.range.end == end
                && if ast::Expr::can_cast(syntax.kind()) {
                    matches!(node.role, NodeRole::Expression(_))
                } else if ast::Pat::can_cast(syntax.kind()) {
                    node.role == NodeRole::Pattern
                } else if ast::Item::can_cast(syntax.kind()) {
                    node.role == NodeRole::OpaqueItem
                } else {
                    node.role == NodeRole::Statement
                }
        })
        .map(|node| node.id)
        .ok_or_else(|| error("selected syntax has no structural phase identity"))
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
        "isize" => PrimitiveType::Isize,
        "u8" => PrimitiveType::U8,
        "u16" => PrimitiveType::U16,
        "u32" => PrimitiveType::U32,
        "u64" => PrimitiveType::U64,
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
    if let ast::Type::ArrayType(array) = &ty {
        let Some(element) = array.ty() else {
            return Ok(None);
        };
        // One primitive element, resolved in the actual lexical namespace.
        // General symbolic/nested arrays and const expressions belong to M017.
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
        let mut len = 0u32;
        let spelling = token.text().strip_suffix("usize").unwrap_or(token.text());
        if spelling.is_empty() || spelling.len() > 32 {
            return Ok(None);
        }
        for byte in spelling.bytes().filter(|byte| *byte != b'_') {
            if !byte.is_ascii_digit() {
                return Ok(None);
            }
            let Some(next) = len
                .checked_mul(10)
                .and_then(|value| value.checked_add(u32::from(byte - b'0')))
            else {
                return Ok(None);
            };
            len = next;
        }
        return Ok(Some(HintRecipe::ArrayPrimitive { element, len }));
    }
    if let ast::Type::RefType(reference) = &ty {
        // Resolve the pointee in its actual lexical context before accepting a
        // primitive spelling. Abstract/nested pointees require later source tables.
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
        let region = if let Some(lifetime) = reference.lifetime() {
            if lifetime.syntax().text() != "'static" {
                return Ok(None);
            }
            crate::traits::term::RegionTerm::Static
        } else {
            let range = reference.syntax().text_range();
            crate::traits::term::RegionTerm::Local {
                definition,
                start: u32::from(range.start()),
                end: u32::from(range.end()),
            }
        };
        return Ok(Some(HintRecipe::Reference {
            region,
            mutable: reference.mut_token().is_some(),
            target,
        }));
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
    Ok(hint(Some(ty)))
}
fn compound_local_primitive(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    path: &ast::PathExpr,
) -> Result<Option<(NodeId, PrimitiveType)>, Diagnostic> {
    let Some(binding) = local_binding(parsed, index, body, path)? else {
        return Ok(None);
    };
    let root = parsed.entered_body_root(body)?;
    let range = index.nodes()[binding.0 as usize].range;
    let start = body
        .range
        .start
        .checked_add(range.start)
        .ok_or_else(|| error("compound binding source start overflow"))?;
    let end = body
        .range
        .start
        .checked_add(range.end)
        .ok_or_else(|| error("compound binding source end overflow"))?;
    let statement = root
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .find(|statement| {
            statement.pat().is_some_and(|pattern| {
                let range = pattern.syntax().text_range();
                u32::from(range.start()) == start && u32::from(range.end()) == end
            })
        });
    let Some(statement) = statement else {
        return Ok(None);
    };
    let Some(HintRecipe::Primitive(ty)) = annotation_hint(parsed, statement.ty(), body.def_key)?
    else {
        return Ok(None);
    };
    Ok(Some((binding, ty)))
}
fn compound_rhs_primitive(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    mut expression: ast::Expr,
    context: PrimitiveType,
) -> Result<Option<PrimitiveType>, Diagnostic> {
    for _ in 0..256 {
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
                return Ok(compound_local_primitive(parsed, index, body, &path)?.map(|(_, ty)| ty));
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
        || !syntax.parent().is_some_and(|parent| {
            ast::Const::can_cast(parent.kind()) || ast::Static::can_cast(parent.kind())
        })
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

fn return_pc(
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
fn place_context(syntax: &SyntaxNode) -> bool {
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
        if let Some(statement) = ast::LetStmt::cast(parent.clone()) {
            return statement
                .initializer()
                .is_some_and(|initializer| initializer.syntax() == &current)
                && matches!(statement.pat(), Some(ast::Pat::IdentPat(pattern))
                    if pattern.ref_token().is_some() && pattern.at_token().is_none());
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
fn intrinsic_place_receiver(expr: &ast::Expr) -> bool {
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
    let Ok(name) = MacroName::parse(reference.text(), parsed.edition) else {
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
        if metas.len() >= 100_000 {
            return Err(error("statement attribute inventory limit"));
        }
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
        remaining
            .iter()
            .any(|meta| !matches!(meta, ast::Meta::CfgMeta(_))),
    ))
}
fn local_binding_at(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    syntax: &SyntaxNode,
    name: &MacroName,
) -> Result<Option<NodeId>, Diagnostic> {
    for ancestor in syntax.ancestors() {
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
                    if let Some(ast::Pat::IdentPat(pattern)) = statement.pat()
                        && let Some(candidate) = pattern.name()
                        && MacroName::parse(candidate.text(), parsed.edition)
                            .is_ok_and(|candidate| &candidate == name)
                    {
                        binding = Some(id_for(index, body, pattern.syntax())?);
                    }
                }
            }
            if binding.is_some() {
                return Ok(binding);
            }
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
            let indexed = &index.nodes()[pattern.0 as usize];
            let start = body
                .range
                .start
                .checked_add(indexed.range.start)
                .ok_or_else(|| error("condition pattern range overflow"))?;
            let end = body
                .range
                .start
                .checked_add(indexed.range.end)
                .ok_or_else(|| error("condition pattern range overflow"))?;
            let pattern = condition
                .syntax()
                .descendants()
                .filter_map(ast::IdentPat::cast)
                .find(|p| {
                    u32::from(p.syntax().text_range().start()) == start
                        && u32::from(p.syntax().text_range().end()) == end
                });
            if let Some(pattern) = pattern
                && pattern.ref_token().is_none()
                && pattern.at_token().is_none()
                && let Some(candidate) = pattern.name()
                && MacroName::parse(candidate.text(), parsed.edition)
                    .is_ok_and(|candidate| &candidate == name)
            {
                binding = Some(id_for(index, body, pattern.syntax())?);
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
    if let Some(params) = syntax
        .ancestors()
        .find_map(ast::Fn::cast)
        .and_then(|f| f.param_list())
    {
        for (ordinal, param) in params.params().enumerate() {
            if let Some(ast::Pat::IdentPat(pattern)) = param.pat()
                && pattern.ref_token().is_none()
                && pattern.at_token().is_none()
                && let Some(candidate) = pattern.name()
                && MacroName::parse(candidate.text(), parsed.edition)
                    .is_ok_and(|candidate| &candidate == name)
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
fn control_target(
    parsed: &ParsedFile,
    index: &BodyIndex,
    node: &super::body_index::StructuralNode,
    lifetime: Option<ast::Lifetime>,
    continuing: bool,
) -> Result<Option<super::body_index::LabelId>, Diagnostic> {
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
        if wanted.as_ref().map_or(label.is_loop, |wanted| {
            label
                .spelling
                .as_ref()
                .is_some_and(|s| canonical(s).is_some() && canonical(s) == canonical(wanted))
        }) {
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
        SyntaxKind::BYTE_STRING => (LiteralKind::ByteString, None),
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
                    .filter(|n| *n <= 100_000)
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
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::MacroExpr::cast)
            .find(|e| {
                u32::from(e.syntax().text_range().start()) == start
                    && u32::from(e.syntax().text_range().end()) == end
            })
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
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::MacroExpr::cast)
            .find(|e| {
                u32::from(e.syntax().text_range().start()) == start
                    && u32::from(e.syntax().text_range().end()) == end
            })
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
        if !place && let Some(ordinal) = parameter_at(self, expression.syntax(), &name)? {
            return Ok(Op::ReadParameter {
                ordinal,
                destination,
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
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::MacroExpr::cast)
            .find(|e| {
                u32::from(e.syntax().text_range().start()) == start
                    && u32::from(e.syntax().text_range().end()) == end
            })
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
        use super::constructors::{ConstructorKind, ConstructorOperand};
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
        let mut operands: Vec<&ConstructorOperand> = Vec::new();
        let count = match plan.kind() {
            ConstructorKind::Tuple(fields) | ConstructorKind::Array(fields) => fields.len(),
            ConstructorKind::Record {
                fields, rest: None, ..
            } => fields.len(),
            ConstructorKind::Record { rest: Some(_), .. } => {
                return thunks::unsupported(
                    index,
                    key.clone(),
                    "record update requires bound base-field transfer".into(),
                    fallback,
                );
            }
            ConstructorKind::Repeat { .. } => {
                return thunks::unsupported(
                    index,
                    key.clone(),
                    "repeat requires const length and resumable Copy binding".into(),
                    fallback,
                );
            }
        };
        operands
            .try_reserve_exact(count)
            .map_err(|_| error("constructor operand allocation failed"))?;
        match plan.kind() {
            ConstructorKind::Tuple(fields) | ConstructorKind::Array(fields) => {
                operands.extend(fields)
            }
            ConstructorKind::Record { fields, .. } => {
                operands.extend(fields.iter().map(|field| &field.operand))
            }
            _ => return Err(error("constructor operand inventory mismatch")),
        }
        let syntax = self
            .parsed
            .tree()
            .syntax()
            .descendants()
            .find(|syntax| {
                let r = syntax.text_range();
                u32::from(r.start()).checked_sub(body.range.start) == Some(node.range.start)
                    && u32::from(r.end()).checked_sub(body.range.start) == Some(node.range.end)
                    && ast::Expr::can_cast(syntax.kind())
            })
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
            if let Some(operand) = operands.get(written) {
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
                let written = operands
                    .iter()
                    .position(|operand| operand.node == *source_node)
                    .ok_or_else(|| error("constructor phase is not an enabled written field"))?;
                let field = u32::try_from(written)
                    .map_err(|_| error("constructor field ordinal overflow"))?;
                draft(
                    index,
                    key,
                    vec![Op::ConstructField {
                        guard,
                        field,
                        source: operands[written].value,
                    }],
                    RecipeExit::Next(next(written + 1)?),
                )
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
                "constructor exit is not an executable construction phase",
            )),
        }
    }
    pub(crate) fn initial_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<VerifiedRecipe, Diagnostic> {
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
        let start = body
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| error("phase range overflow"))?;
        let end = body
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| error("phase range overflow"))?;
        let syntax = self.parsed.tree().syntax().descendants().find(|syntax| {
            u32::from(syntax.text_range().start()) == start
                && u32::from(syntax.text_range().end()) == end
                && if matches!(node.role, NodeRole::Expression(_)) {
                    ast::Expr::can_cast(syntax.kind())
                } else if node.role == NodeRole::Statement {
                    ast::Stmt::can_cast(syntax.kind())
                } else if node.role == NodeRole::MatchArm {
                    ast::MatchArm::can_cast(syntax.kind())
                } else if node.role == NodeRole::Pattern {
                    matches!(
                        syntax.kind(),
                        SyntaxKind::LITERAL_PAT | SyntaxKind::WILDCARD_PAT
                    ) && syntax
                        .ancestors()
                        .any(|parent| ast::MatchArm::can_cast(parent.kind()))
                } else if node.role == NodeRole::OpaqueItem {
                    ast::Item::can_cast(syntax.kind())
                } else {
                    false
                }
        });
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
        if matches!(
            syntax.kind(),
            SyntaxKind::REF_EXPR
                | SyntaxKind::FIELD_EXPR
                | SyntaxKind::INDEX_EXPR
                | SyntaxKind::PREFIX_EXPR
                | SyntaxKind::PAREN_EXPR
                | SyntaxKind::PATH_EXPR
        ) {
            return self.initial_place_thunk(body, index, key, syntax);
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
            | SyntaxKind::TRAIT => self.initial_declaration_thunk(body, index, key, syntax),
            SyntaxKind::BLOCK_EXPR | SyntaxKind::LET_STMT | SyntaxKind::EXPR_STMT => {
                self.initial_scope_thunk(body, index, key, syntax)
            }
            SyntaxKind::LET_EXPR => self.initial_let_condition_thunk(body, index, key, syntax),
            SyntaxKind::MATCH_EXPR
            | SyntaxKind::MATCH_ARM
            | SyntaxKind::LITERAL_PAT
            | SyntaxKind::WILDCARD_PAT => literal_match::prepare(self, body, index, key, syntax),
            SyntaxKind::CALL_EXPR => self.initial_call_thunk(body, index, key, syntax),
            SyntaxKind::RETURN_EXPR
            | SyntaxKind::LOOP_EXPR
            | SyntaxKind::BREAK_EXPR
            | SyntaxKind::CONTINUE_EXPR => self.initial_jump_thunk(body, index, key, syntax),
            SyntaxKind::WHILE_EXPR | SyntaxKind::IF_EXPR => {
                self.initial_condition_thunk(body, index, key, syntax)
            }
            SyntaxKind::BIN_EXPR => self.initial_binary_thunk(body, index, key, syntax),
            SyntaxKind::CAST_EXPR | SyntaxKind::LITERAL => {
                self.initial_scalar_thunk(body, index, key, syntax)
            }
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
                | SyntaxKind::TRAIT,
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
                if block.async_token().is_some()
                    || block.const_token().is_some()
                    || block.gen_token().is_some()
                    || block.move_token().is_some()
                    || block.unsafe_token().is_some()
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
                let first = block.stmt_list().and_then(|list| {
                    list.syntax().children().find(|node| {
                        ast::Stmt::can_cast(node.kind()) || ast::Expr::can_cast(node.kind())
                    })
                });
                let next = if let Some(first) = first {
                    if ast::Expr::can_cast(first.kind()) && node.tail_scope.is_some() {
                        index.pc(id, Phase::Exit)?
                    } else {
                        index.pc(id_for(index, body, &first)?, Phase::Entry)?
                    }
                } else {
                    index.pc(id, Phase::Commit)?
                };
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

                if id == NodeId(0)
                    && let Some(ty) = block
                        .syntax()
                        .ancestors()
                        .find_map(ast::Fn::cast)
                        .and_then(|function| function.ret_type())
                        .and_then(|ret| ret.ty())
                {
                    let Some(return_hint) = annotation_hint(self, Some(ty), key.body.definition)?
                    else {
                        return unsupported("return type requires symbolic resolution");
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
                draft(index, key, ops, RecipeExit::Next(next))
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
                    // A wildcard does not move a place into a new local. Keep
                    // place handling refused until its type-only check is ready.
                    if intrinsic_place_receiver(&initializer) {
                        return unsupported("wildcard place requires non-moving type integration");
                    }
                    let Some(mut hint) =
                        annotation_hint(self, statement.ty(), key.body.definition)?
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
                // A bare local literal remains unresolved for later written
                // uses. Only an operator initializer needs a closed numeric
                // result before executing its operands. A written type wins.
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
    fn initial_let_condition_thunk(
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
            (SyntaxKind::LET_EXPR, phase) => {
                let expression = ast::LetExpr::cast(syntax).expect("checked condition let");
                if node.clause_scope.is_none() {
                    return unsupported(
                        "let pattern requires a top-level if/while condition clause",
                    );
                }
                let Some(pattern) = expression.pat() else {
                    return unsupported("condition pattern absent");
                };
                if matches!(pattern, ast::Pat::IdentPat(_) | ast::Pat::WildcardPat(_)) {
                    let binding = if let ast::Pat::IdentPat(pattern) = &pattern {
                        use crate::vm::pattern::{BindingModifier, PatternContext};
                        let modifier =
                            match (pattern.ref_token().is_some(), pattern.mut_token().is_some()) {
                                (false, false) => BindingModifier::Plain,
                                (false, true) => BindingModifier::Mut,
                                (true, false) => BindingModifier::Ref,
                                (true, true) => BindingModifier::RefMut,
                            };
                        // The admitted scalar condition path begins in Move;
                        // reference/aggregate peeling belongs to T050/M028.
                        PatternContext::new(self.edition)?.binding_mode(modifier)?;
                        if pattern.ref_token().is_some() || pattern.at_token().is_some() {
                            return unsupported(
                                "condition binding mode awaits pattern integration",
                            );
                        }
                        Some((
                            id_for(index, body, pattern.syntax())?,
                            pattern.mut_token().is_some(),
                        ))
                    } else {
                        None
                    };
                    let Some(scrutinee) = expression.expr() else {
                        return unsupported("condition scrutinee absent");
                    };
                    let scrutinee = id_for(index, body, scrutinee.syntax())?;
                    let source = index.nodes()[scrutinee.0 as usize]
                        .value
                        .ok_or_else(|| error("condition scrutinee slot absent"))?;
                    let destination = node
                        .value
                        .ok_or_else(|| error("condition test result absent"))?;
                    return match phase {
                        Phase::Entry => draft(
                            index,
                            key,
                            vec![
                                Op::ScopeEnter {
                                    scope: node
                                        .clause_scope
                                        .ok_or_else(|| error("condition clause scope absent"))?,
                                },
                                Op::SetHint {
                                    child: source,
                                    hint: HintRecipe::None,
                                },
                            ],
                            RecipeExit::Next(index.pc(scrutinee, Phase::Entry)?),
                        ),
                        Phase::Operand(n)
                            if node.operands.get(usize::from(n)) == Some(&scrutinee) =>
                        {
                            draft(
                                index,
                                key,
                                vec![Op::Literal {
                                    literal: LiteralRecipe {
                                        kind: LiteralKind::Bool,
                                        spelling: "true".into(),
                                        decoded: None,
                                        source: index.span(id)?,
                                    },
                                    hint: HintRecipe::Primitive(PrimitiveType::Bool),
                                    destination,
                                }],
                                RecipeExit::Next(index.pc(id, Phase::Commit)?),
                            )
                        }
                        Phase::Commit => draft(
                            index,
                            key,
                            vec![if let Some((binding, writable)) = binding {
                                Op::BindLocal {
                                    binding,
                                    source,
                                    hint: HintRecipe::None,
                                    writable,
                                }
                            } else {
                                Op::Discard { source }
                            }],
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        ),
                        _ => unsupported("invalid condition binding phase"),
                    };
                }
                let ast::Pat::LiteralPat(pattern) = pattern else {
                    return unsupported(
                        "reached condition pattern awaits ordered pattern integration",
                    );
                };
                let Some(literal) = pattern.literal() else {
                    return unsupported("condition literal pattern absent");
                };
                if pattern.minus_token().is_some()
                    || !matches!(
                        literal.token().kind(),
                        SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW
                    )
                {
                    return unsupported(
                        "reached condition literal pattern awaits scalar pattern integration",
                    );
                }
                let Some(scrutinee) = expression.expr() else {
                    return unsupported("condition scrutinee absent");
                };
                let scrutinee = id_for(index, body, scrutinee.syntax())?;
                let source = index.nodes()[scrutinee.0 as usize]
                    .value
                    .ok_or_else(|| error("condition scrutinee slot absent"))?;
                let expected = node.runtime_temporaries[0]
                    .ok_or_else(|| error("condition pattern temporary absent"))?;
                let destination = node
                    .value
                    .ok_or_else(|| error("condition test result absent"))?;
                match phase {
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![
                            Op::ScopeEnter {
                                scope: node
                                    .clause_scope
                                    .ok_or_else(|| error("condition clause scope absent"))?,
                            },
                            Op::SetHint {
                                child: source,
                                hint: HintRecipe::Primitive(PrimitiveType::Bool),
                            },
                        ],
                        RecipeExit::Next(index.pc(scrutinee, Phase::Entry)?),
                    ),
                    Phase::Operand(n) if node.operands.get(usize::from(n)) == Some(&scrutinee) => {
                        let span = literal.syntax().text_range();
                        let relative_start = u32::from(span.start())
                            .checked_sub(body.range.start)
                            .ok_or_else(|| error("condition literal precedes body"))?;
                        let relative_end = u32::from(span.end())
                            .checked_sub(body.range.start)
                            .ok_or_else(|| error("condition literal precedes body"))?;
                        draft(
                            index,
                            key,
                            vec![
                                Op::Literal {
                                    literal: LiteralRecipe {
                                        kind: LiteralKind::Bool,
                                        spelling: literal.token().text().into(),
                                        decoded: None,
                                        source: super::spans::BodySpan {
                                            body_key: key.body.clone(),
                                            relative_start,
                                            relative_end,
                                        },
                                    },
                                    hint: HintRecipe::Primitive(PrimitiveType::Bool),
                                    destination: expected,
                                },
                                Op::NumericBinary {
                                    left: source,
                                    right: expected,
                                    destination,
                                    operator: NumericBinary::Eq,
                                },
                                Op::Discard { source },
                                Op::Discard { source: expected },
                            ],
                            RecipeExit::Next(return_pc(index, body, expression.syntax())?),
                        )
                    }
                    _ => unsupported("invalid condition pattern phase"),
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
                let Some(ast::Expr::PathExpr(path)) = call.expr() else {
                    return unsupported("callee values await function-value integration");
                };
                if local_binding(self, index, body, &path)?.is_some() {
                    return unsupported("local callee value awaits function-value integration");
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
                    if components.len() >= 256 {
                        return Err(error("source call path depth exceeded"));
                    }
                    let Some(part) = cursor.segment() else {
                        return unsupported("callee path segment absent");
                    };
                    if !components.is_empty() && part.generic_arg_list().is_some() {
                        return unsupported(
                            "generic path owner requires associated-call integration",
                        );
                    }
                    let text = match part.kind() {
                        Some(ast::PathSegmentKind::Name(name)) => name.text().to_string(),
                        Some(ast::PathSegmentKind::CrateKw) => "crate".into(),
                        Some(ast::PathSegmentKind::SelfKw) => "self".into(),
                        Some(ast::PathSegmentKind::SuperKw) => "super".into(),
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
                let relative =
                    !absolute && !matches!(components[0].as_str(), "crate" | "self" | "super");
                let root_name = components[0].clone();
                let mut name = components.join("::");
                if absolute {
                    name.insert_str(0, "::");
                }
                if name.len() > 4096 {
                    return Err(error("source call path text limit exceeded"));
                }

                let explicit = segment
                    .generic_arg_list()
                    .map(|list| {
                        list.generic_args()
                            .map(|arg| arg.syntax().text().to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
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
                let target_name = MacroName::parse(&root_name, self.edition).ok();
                if qualified && relative && path.syntax().ancestors().find_map(ast::Fn::cast)
                    .and_then(|function| function.generic_param_list())
                    .is_some_and(|parameters| parameters.generic_params().any(|parameter| matches!(parameter,
                        ast::GenericParam::TypeParam(parameter) if parameter.name().is_some_and(|name| MacroName::parse(name.text(),self.edition).ok()==target_name)))) {
                    return unsupported("generic source owner requires associated-call integration");
                }
                let local_item = relative
                    && path
                        .syntax()
                        .ancestors()
                        .filter_map(ast::StmtList::cast)
                        .any(|list| {
                            list.statements().any(|statement| match statement {
                                ast::Stmt::Item(ast::Item::Fn(item)) if !qualified => {
                                    item.name().is_some_and(|n| {
                                        MacroName::parse(n.text(), self.edition).ok() == target_name
                                    })
                                }
                                ast::Stmt::Item(ast::Item::Const(item)) if !qualified => {
                                    item.name().is_some_and(|n| {
                                        MacroName::parse(n.text(), self.edition).ok() == target_name
                                    })
                                }
                                ast::Stmt::Item(ast::Item::Static(item)) if !qualified => {
                                    item.name().is_some_and(|n| {
                                        MacroName::parse(n.text(), self.edition).ok() == target_name
                                    })
                                }
                                ast::Stmt::Item(ast::Item::Module(item)) if qualified => {
                                    item.name().is_some_and(|n| {
                                        MacroName::parse(n.text(), self.edition).ok() == target_name
                                    })
                                }
                                ast::Stmt::Item(ast::Item::TypeAlias(item)) if qualified => {
                                    item.name().is_some_and(|n| {
                                        MacroName::parse(n.text(), self.edition).ok() == target_name
                                    })
                                }
                                ast::Stmt::Item(ast::Item::Struct(item)) if qualified => {
                                    item.name().is_some_and(|n| {
                                        MacroName::parse(n.text(), self.edition).ok() == target_name
                                    })
                                }
                                ast::Stmt::Item(ast::Item::Enum(item)) if qualified => {
                                    item.name().is_some_and(|n| {
                                        MacroName::parse(n.text(), self.edition).ok() == target_name
                                    })
                                }
                                ast::Stmt::Item(ast::Item::Union(item)) if qualified => {
                                    item.name().is_some_and(|n| {
                                        MacroName::parse(n.text(), self.edition).ok() == target_name
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
                    && self.generated_origin.is_none()
                    && self.lexical_function_recipe_candidate(&path, index.cfg())?;
                if local_item && !lexical_function
                    || relative
                        && self.enclosing_import_may_bind(path.syntax(), &root_name, index.cfg())?
                {
                    return unsupported("lexical item callee requires body namespace integration");
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
                            parent: index.nodes()[0].value.expect("root result"),
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
                let Some(target) = control_target(self, index, node, expression.lifetime(), false)?
                else {
                    return unsupported("break target is absent");
                };
                let label = &index.labels()[target.0 as usize];
                let target_node = &index.nodes()[label.node.0 as usize];
                if expression.expr().is_some()
                    && target_node.role
                        == NodeRole::Expression(super::body_index::ExpressionKind::While)
                {
                    return unsupported("while does not accept a break value");
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
                    .parsed
                    .tree()
                    .syntax()
                    .descendants()
                    .find(|s| {
                        ast::Expr::can_cast(s.kind())
                            && u32::from(s.text_range().start()) == start
                            && u32::from(s.text_range().end()) == end
                    })
                    .ok_or_else(|| error("break continuation syntax absent"))?;
                let resume = return_pc(index, body, &target_syntax)?;
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
                let Some(target) = control_target(self, index, node, expression.lifetime(), true)?
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
    fn initial_binary_thunk(
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
                            if !matches!(&left, ast::Expr::IndexExpr(_) | ast::Expr::FieldExpr(_))
                                && !matches!(&left, ast::Expr::PrefixExpr(prefix) if prefix.op_token().is_some_and(|token| token.text() == "*"))
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
                let Some(ast::Expr::PathExpr(left)) = expression.lhs() else {
                    return unsupported(
                        "compound projected place requires source-view integration before operands",
                    );
                };
                let Some((binding, left_type)) =
                    compound_local_primitive(self, index, body, &left)?
                else {
                    return Err(Diagnostic::new("LRR-E-TYPE-CONTEXT", "compound assignment requires a primitive source annotation or exact trait member before operands".into()));
                };
                let right = expression
                    .rhs()
                    .ok_or_else(|| error("compound RHS absent"))?;
                let context = if matches!(operator, NumericBinary::Shl | NumericBinary::Shr) {
                    PrimitiveType::I32
                } else {
                    left_type
                };
                let Some(right_type) =
                    compound_rhs_primitive(self, index, body, right.clone(), context)?
                else {
                    return Err(Diagnostic::new(
                        "LRR-E-TYPE-CONTEXT",
                        "compound RHS requires a symbolic source view before operands".into(),
                    ));
                };
                let kind = crate::ir::op::AssignKind::Primitive {
                    left: left_type,
                    right: right_type,
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
                    Phase::Entry => draft(
                        index,
                        key,
                        vec![
                            Op::CompoundHint {
                                binding,
                                child: rhs,
                                kind,
                            },
                            Op::SetHint {
                                child: computed,
                                hint: HintRecipe::Primitive(left_type),
                            },
                        ],
                        RecipeExit::Next(index.pc(right, Phase::Entry)?),
                    ),
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
                let left = id_for(index, body, left.syntax())?;
                let right = id_for(index, body, right.syntax())?;
                let left_slot = index.nodes()[left.0 as usize].value.expect("left value");
                let right_slot = index.nodes()[right.0 as usize].value.expect("right value");
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
                        vec![super::numeric_fallback::right_operand_hint(
                            &right_expression,
                            destination,
                            right_slot,
                            Some(left_slot),
                            operator,
                            right_default,
                            key.body.definition,
                        )?],
                        RecipeExit::Next(index.pc(right, Phase::Entry)?),
                    ),
                    Phase::Operand(1) => draft(
                        index,
                        key,
                        vec![Op::NumericBinary {
                            operator,
                            left: left_slot,
                            right: right_slot,
                            destination,
                        }],
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
                // Admit the target in its original lexical source context.
                // A hoisted local type or generic parameter may shadow a
                // primitive spelling; it requires later symbolic resolution.
                let Some(target) = primitive_cast_target::resolve(self, index, expression.ty())?
                else {
                    return unsupported("cast target requires symbolic type resolution");
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
                    literal_recipe(&literal, index.span(id)?)?,
                    HintRecipe::None,
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
                if !matches!(&operand, ast::Expr::PathExpr(_)) {
                    let operand_is_place = intrinsic_place_receiver(&operand);
                    let child = id_for(index, body, operand.syntax())?;
                    let place = index.nodes()[child.0 as usize]
                        .place
                        .ok_or_else(|| error("borrowed operand place absent"))?;
                    return match phase {
                        Phase::Entry => draft(
                            index,
                            key,
                            if operand_is_place {
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
                        vec![Op::ValuePlace {
                            source: index.nodes()[base.0 as usize]
                                .value
                                .ok_or_else(|| error("temporary field receiver value absent"))?,
                            destination: base_place,
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
                            ops.push(Op::ValuePlace {
                                source: index.nodes()[base.0 as usize].value.ok_or_else(|| {
                                    error("temporary index receiver value absent")
                                })?,
                                destination: base_place,
                            });
                        }
                        ops.push(Op::SetHint {
                            child: index_value,
                            hint: HintRecipe::Primitive(PrimitiveType::Usize),
                        });
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
                            HintRecipe::None,
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
            (SyntaxKind::PATH_EXPR, Phase::Entry) => {
                let path = ast::PathExpr::cast(syntax).expect("checked path");
                let binding = local_binding(self, index, body, &path)?;
                let Some(binding) = binding else {
                    if place_context(path.syntax()) {
                        return unsupported(
                            "non-local receiver requires retained source place resolution",
                        );
                    }
                    let name =
                        MacroName::parse(&path.syntax().text().to_string(), self.edition).ok();
                    if let Some(ordinal) = name
                        .as_ref()
                        .map(|name| parameter_at(self, path.syntax(), name))
                        .transpose()?
                        .flatten()
                    {
                        return draft(
                            index,
                            key,
                            vec![Op::ReadParameter {
                                ordinal,
                                destination: node.value.expect("parameter path value"),
                            }],
                            RecipeExit::Next(return_pc(index, body, path.syntax())?),
                        );
                    }
                    return unsupported("non-local path awaits ordinary namespace resolution");
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
        if self.generated_origin.is_some() || expected_name.contains("::") {
            return Ok(false);
        }
        if body.key() != *index.body() || key.body != body.key() {
            return Err(error("lexical dispatch caller body identity differs"));
        }
        if index.pc(NodeId(key.node), Phase::from_code(key.phase))? != *key {
            return Err(error("lexical dispatch exact PC differs"));
        }
        let node = &index.nodes()[key.node as usize];
        // A verified assertion predicate CallSeal belongs to its Macro PC,
        // whose operands are opaque token trees, not indexed CallExpr nodes.
        // RuntimeMacroPlan already requires the exact original predicate and
        // refuses lexical shadowing before preparing this module-call recipe.
        if node.role == NodeRole::Expression(ExpressionKind::Macro) {
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
        self.lexical_function_recipe_candidate(&path, index.cfg())
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
        for (depth, scope) in path.syntax().ancestors().enumerate() {
            if depth >= 256 {
                return Err(error("lexical function recipe scope depth budget"));
            }
            let Some(list) = ast::StmtList::cast(scope) else {
                continue;
            };
            for statement in list.statements() {
                work = work
                    .checked_add(1)
                    .filter(|n| *n <= 100_000)
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
