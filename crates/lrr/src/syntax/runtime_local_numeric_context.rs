//! Entered, source-only numeric relations to an admitted definition contract.
//! This does not normalize a type or inspect any runtime descriptor. A relation
//! is useful only when the VM can admit its current parameter/result source.
use super::{BodyHandle, BodyIndex, ParsedFile, id_for, local_binding, parameter_at};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    ir::{
        op::{DefinitionNumericContext, PrimitiveType},
        pc::NodeId,
    },
};
use ra_ap_syntax::{
    AstNode, AstToken, SyntaxKind, SyntaxNode,
    ast::{self, HasGenericArgs},
};

fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-BODY-INDEX-LIMIT",
        "definition numeric source work budget".into(),
    )
}
#[derive(Default)]
struct Budget {
    work: usize,
    reads: usize,
}
impl Budget {
    fn charge(&mut self) -> Result<(), Diagnostic> {
        self.work = self
            .work
            .checked_add(1)
            .filter(|n| *n <= 100_000)
            .ok_or_else(limit)?;
        Ok(())
    }
    fn read(&mut self) -> Result<(), Diagnostic> {
        self.reads = self
            .reads
            .checked_add(1)
            .filter(|n| *n <= 256)
            .ok_or_else(limit)?;
        self.charge()
    }
}
fn numeric_family(primitive: PrimitiveType) -> Option<bool> {
    match primitive {
        PrimitiveType::I8
        | PrimitiveType::I16
        | PrimitiveType::I32
        | PrimitiveType::I64
        | PrimitiveType::Isize
        | PrimitiveType::U8
        | PrimitiveType::U16
        | PrimitiveType::U32
        | PrimitiveType::U64
        | PrimitiveType::Usize => Some(false),
        PrimitiveType::F32 | PrimitiveType::F64 => Some(true),
        _ => None,
    }
}
fn unsuffixed_family(mut expression: ast::Expr) -> Result<Option<bool>, Diagnostic> {
    for _ in 0..256 {
        match expression {
            ast::Expr::ParenExpr(group) => {
                let Some(inner) = group.expr() else {
                    return Ok(None);
                };
                expression = inner;
            }
            ast::Expr::PrefixExpr(prefix) if prefix.op_token().is_some_and(|t| t.text() == "-") => {
                let Some(inner) = prefix.expr() else {
                    return Ok(None);
                };
                expression = inner;
            }
            ast::Expr::Literal(literal) => {
                let token = literal.token();
                return Ok(match token.kind() {
                    SyntaxKind::INT_NUMBER => ast::IntNumber::cast(token)
                        .and_then(|n| n.suffix().is_none().then_some(false)),
                    SyntaxKind::FLOAT_NUMBER => ast::FloatNumber::cast(token)
                        .and_then(|n| n.suffix().is_none().then_some(true)),
                    _ => None,
                });
            }
            _ => return Ok(None),
        }
    }
    Err(limit())
}
fn suffix_primitive(suffix: Option<&str>) -> Option<Option<PrimitiveType>> {
    let primitive = match suffix {
        None => return Some(None),
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
        _ => return None,
    };
    Some(Some(primitive))
}
fn literal_suffix(mut expression: ast::Expr) -> Result<Option<Option<PrimitiveType>>, Diagnostic> {
    for _ in 0..256 {
        match expression {
            ast::Expr::ParenExpr(group) => {
                let Some(inner) = group.expr() else {
                    return Ok(None);
                };
                expression = inner;
            }
            ast::Expr::PrefixExpr(prefix) if prefix.op_token().is_some_and(|t| t.text() == "-") => {
                let Some(inner) = prefix.expr() else {
                    return Ok(None);
                };
                expression = inner;
            }
            ast::Expr::Literal(literal) => {
                let token = literal.token();
                return Ok(match token.kind() {
                    SyntaxKind::INT_NUMBER => {
                        ast::IntNumber::cast(token).and_then(|n| suffix_primitive(n.suffix()))
                    }
                    SyntaxKind::FLOAT_NUMBER => {
                        ast::FloatNumber::cast(token).and_then(|n| suffix_primitive(n.suffix()))
                    }
                    _ => None,
                });
            }
            _ => return Ok(None),
        }
    }
    Err(limit())
}
/// The nested option distinguishes an unsupported expression from an admitted
/// same-source unsuffixed literal. Literal validity is still checked when reached.
pub(super) fn compound_right(
    expression: &ast::Expr,
    shift: bool,
) -> Result<Option<Option<PrimitiveType>>, Diagnostic> {
    Ok(literal_suffix(expression.clone())?.map(|suffix| {
        suffix.or(if shift {
            Some(PrimitiveType::I32)
        } else {
            None
        })
    }))
}
fn path(mut expression: ast::Expr) -> Result<Option<ast::PathExpr>, Diagnostic> {
    for _ in 0..256 {
        match expression {
            ast::Expr::ParenExpr(group) => {
                let Some(inner) = group.expr() else {
                    return Ok(None);
                };
                expression = inner;
            }
            ast::Expr::PathExpr(path) => return Ok(Some(path)),
            _ => return Ok(None),
        }
    }
    Err(limit())
}
fn same_binding(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    expression: ast::Expr,
    binding: NodeId,
    budget: &mut Budget,
) -> Result<bool, Diagnostic> {
    let Some(path) = path(expression)? else {
        return Ok(false);
    };
    if !path
        .path()
        .filter(|p| p.qualifier().is_none())
        .and_then(|p| p.segment())
        .is_some_and(|segment| {
            segment.generic_arg_list().is_none() && segment.coloncolon_token().is_none()
        })
    {
        return Ok(false);
    }
    budget.read()?;
    Ok(local_binding(parsed, index, body, &path)? == Some(binding))
}
fn compatible_numeric_type(
    parsed: &ParsedFile,
    index: &BodyIndex,
    ty: ast::Type,
    wanted_float: bool,
) -> Result<bool, Diagnostic> {
    match super::local_numeric_constraints::source_primitive_annotation(parsed, index, ty) {
        Ok(primitive) => Ok(numeric_family(primitive) == Some(wanted_float)),
        // A cold unresolved/nonnumeric use supplies no numeric evidence. Do not
        // turn its diagnostic contract into a failure of the entered initializer.
        Err(diagnostic) if diagnostic.code == "LRR-E-UNSUPPORTED" => Ok(false),
        // Resource and invalid-source failures retain their actual meaning.
        Err(diagnostic) => Err(diagnostic),
    }
}
fn parameter(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    expression: ast::Expr,
    wanted_float: bool,
    budget: &mut Budget,
) -> Result<Option<u32>, Diagnostic> {
    let Some(path) = path(expression)? else {
        return Ok(None);
    };
    let Some(segment) = path
        .path()
        .filter(|p| p.qualifier().is_none())
        .and_then(|p| p.segment())
        .filter(|s| s.generic_arg_list().is_none() && s.coloncolon_token().is_none())
    else {
        return Ok(None);
    };
    let Some(name) = segment.name_ref() else {
        return Ok(None);
    };
    let name = MacroName::parse(name.text(), parsed.edition)?;
    budget.read()?;
    if local_binding(parsed, index, body, &path)?.is_some() {
        return Ok(None);
    }
    let Some(ordinal) = parameter_at(parsed, path.syntax(), &name)? else {
        return Ok(None);
    };
    let Some(parameters) = path
        .syntax()
        .ancestors()
        .find_map(ast::Fn::cast)
        .and_then(|f| f.param_list())
    else {
        return Ok(None);
    };
    let offset = usize::from(parameters.self_param().is_some());
    let Some(parameter) = usize::try_from(ordinal)
        .ok()
        .and_then(|n| n.checked_sub(offset))
        .and_then(|n| parameters.params().nth(n))
    else {
        return Ok(None);
    };
    let Some(ty) = parameter.ty() else {
        return Ok(None);
    };
    Ok(compatible_numeric_type(parsed, index, ty, wanted_float)?.then_some(ordinal))
}
fn push_children(
    stack: &mut Vec<(SyntaxNode, usize)>,
    node: &SyntaxNode,
    depth: usize,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    let depth = depth
        .checked_add(1)
        .filter(|n| *n <= 256)
        .ok_or_else(limit)?;
    for child in node.children() {
        budget.charge()?;
        if stack.len() >= 100_000
            || stack
                .len()
                .checked_add(1)
                .and_then(|count| count.checked_mul(std::mem::size_of::<(SyntaxNode, usize)>()))
                .is_none_or(|bytes| bytes > 8 * 1024 * 1024)
        {
            return Err(limit());
        }
        stack.try_reserve(1).map_err(|_| limit())?;
        stack.push((child, depth));
    }
    Ok(())
}

pub(super) fn inferred_context(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    statement: &ast::LetStmt,
    pattern: &ast::IdentPat,
) -> Result<Option<DefinitionNumericContext>, Diagnostic> {
    if index.body() != &body.key() || index.profile() != &parsed.profile {
        return Err(super::error(
            "definition numeric context body/profile mismatch",
        ));
    }
    if statement.ty().is_some() || pattern.ref_token().is_some() || pattern.at_token().is_some() {
        return Ok(None);
    }
    let Some(initializer) = statement.initializer() else {
        return Ok(None);
    };
    let Some(wanted_float) = unsuffixed_family(initializer)? else {
        return Ok(None);
    };
    let root = parsed.entered_body_root(body)?;
    if !root
        .text_range()
        .contains_range(statement.syntax().text_range())
        || !statement
            .pat()
            .is_some_and(|actual| actual.syntax() == pattern.syntax())
    {
        return Err(super::error(
            "definition numeric binding source owner mismatch",
        ));
    }
    let Some(function) = root.parent().and_then(ast::Fn::cast) else {
        return Ok(None);
    };
    let Some(block) = ast::BlockExpr::cast(root.clone()) else {
        return Ok(None);
    };
    let binding = id_for(index, body, pattern.syntax())?;
    let mut budget = Budget::default();
    // A strong same-family source proof retains the old Result path. Otherwise
    // retain only the exact owning-result relation: the reached VM, not printed
    // syntax, projects its admitted source or chooses the closed literal default.
    let mut conditional_result = false;
    if let Some(result) = function.ret_type().and_then(|r| r.ty())
        && let Some(tail) = block.tail_expr()
        && same_binding(parsed, index, body, tail, binding, &mut budget)?
    {
        if compatible_numeric_type(parsed, index, result, wanted_float)? {
            return Ok(Some(DefinitionNumericContext::Result));
        }
        conditional_result = true;
    }
    let mut stack = Vec::new();
    push_children(&mut stack, &root, 0, &mut budget)?;
    let mut selected = None;
    let mut conflict = false;
    while let Some((node, depth)) = stack.pop() {
        budget.charge()?;
        if ast::Item::can_cast(node.kind())
            || ast::ClosureExpr::can_cast(node.kind())
            || ast::MacroExpr::can_cast(node.kind())
            || node.kind() == SyntaxKind::TOKEN_TREE
        {
            continue;
        }
        if ast::BlockExpr::cast(node.clone()).is_some_and(|block| {
            block.async_token().is_some()
                || block.const_token().is_some()
                || block.gen_token().is_some()
                || block.try_block_modifier().is_some()
        }) {
            continue;
        }
        let (enabled, attributed) = super::statement_cfg(&node, index.cfg())?;
        if !enabled || attributed {
            continue;
        }
        if let Some(result) = function.ret_type().and_then(|r| r.ty())
            && let Some(returned) = ast::ReturnExpr::cast(node.clone()).and_then(|r| r.expr())
            && same_binding(parsed, index, body, returned, binding, &mut budget)?
        {
            if compatible_numeric_type(parsed, index, result, wanted_float)? {
                return Ok(Some(DefinitionNumericContext::Result));
            }
            conditional_result = true;
        }
        if let Some(binary) = ast::BinExpr::cast(node.clone())
            && binary.op_token().is_some_and(|t| {
                matches!(
                    t.text(),
                    "==" | "!="
                        | "<"
                        | "<="
                        | ">"
                        | ">="
                        | "+"
                        | "-"
                        | "*"
                        | "/"
                        | "%"
                        | "&"
                        | "|"
                        | "^"
                )
            })
            && let (Some(left), Some(right)) = (binary.lhs(), binary.rhs())
        {
            let other = if same_binding(parsed, index, body, left.clone(), binding, &mut budget)? {
                Some(right)
            } else if same_binding(parsed, index, body, right, binding, &mut budget)? {
                Some(left)
            } else {
                None
            };
            if let Some(other) = other
                && let Some(ordinal) =
                    parameter(parsed, index, body, other, wanted_float, &mut budget)?
            {
                let context = DefinitionNumericContext::Parameter(ordinal);
                conflict |= selected.is_some_and(|old| old != context);
                selected = Some(context);
            }
        }
        push_children(&mut stack, &node, depth, &mut budget)?;
    }
    // A unique already-proven compatible parameter is stronger evidence than
    // a conditional result. Conflicting parameter authorities mint no positive
    // parameter claim; the exact result relation may still be projected by VM.
    Ok((if conflict { None } else { selected }).or_else(|| {
        conditional_result.then_some(DefinitionNumericContext::ResultOrDefault {
            floating: wanted_float,
        })
    }))
}

pub(super) fn binding_context(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    binding: NodeId,
) -> Result<Option<DefinitionNumericContext>, Diagnostic> {
    let root = parsed.entered_body_root(body)?;
    let mut stack = Vec::new();
    let mut budget = Budget::default();
    push_children(&mut stack, &root, 0, &mut budget)?;
    while let Some((node, depth)) = stack.pop() {
        budget.charge()?;
        if ast::Item::can_cast(node.kind())
            || ast::ClosureExpr::can_cast(node.kind())
            || ast::MacroExpr::can_cast(node.kind())
            || node.kind() == SyntaxKind::TOKEN_TREE
        {
            continue;
        }
        if ast::BlockExpr::cast(node.clone()).is_some_and(|block| {
            block.async_token().is_some()
                || block.const_token().is_some()
                || block.gen_token().is_some()
                || block.try_block_modifier().is_some()
        }) {
            continue;
        }
        let (enabled, attributed) = super::statement_cfg(&node, index.cfg())?;
        if !enabled || attributed {
            continue;
        }
        if let Some(statement) = ast::LetStmt::cast(node.clone())
            && let Some(ast::Pat::IdentPat(pattern)) = statement.pat()
            && id_for(index, body, pattern.syntax())? == binding
        {
            return inferred_context(parsed, index, body, &statement, &pattern);
        }
        push_children(&mut stack, &node, depth, &mut budget)?;
    }
    Ok(None)
}

#[cfg(test)]
#[path = "runtime_local_numeric_context_tests.rs"]
mod tests;
