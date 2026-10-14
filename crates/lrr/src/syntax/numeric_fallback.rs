//! Rust numeric fallback only where source syntax proves the numeric inference
//! island closed. Named values/calls/casts/suffixes require ordinary constraints.
use crate::{
    core::diagnostic::Diagnostic,
    ir::op::{HintRecipe, NumericBinary, Op, PrimitiveType},
    ir::pc::SlotId,
};
use ra_ap_syntax::{AstNode, AstToken, SyntaxKind, ast};

pub(super) fn literal_only(expression: &ast::Expr) -> Result<Option<PrimitiveType>, Diagnostic> {
    // The result of an operator can be primitive even when an operand uses
    // the standard shared-reference forwarding impl. A reference result itself
    // never gets a primitive hint from this closed-syntax check.
    let mut root = expression.clone();
    let mut depth = 0usize;
    while let ast::Expr::ParenExpr(parenthesis) = root {
        depth = depth.checked_add(1).filter(|n| *n <= 256).ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "numeric fallback parenthesis limit".into(),
            )
        })?;
        let Some(inner) = parenthesis.expr() else {
            return Ok(None);
        };
        root = inner;
    }
    let primitive_operator_result =
        matches!(root, ast::Expr::BinExpr(_) | ast::Expr::PrefixExpr(_));
    let mut family = None;
    let mut visited = 0usize;
    for node in expression.syntax().descendants() {
        source_work(&mut visited)?;
        if ast::Attr::can_cast(node.kind())
            || ast::Item::can_cast(node.kind())
            || ast::MacroCall::can_cast(node.kind())
        {
            return Ok(None);
        }
        let Some(expr) = ast::Expr::cast(node) else {
            continue;
        };
        let next = match expr {
            ast::Expr::Literal(literal) => match literal.token().kind() {
                SyntaxKind::INT_NUMBER => {
                    let Some(number) = ast::IntNumber::cast(literal.token()) else {
                        return Ok(None);
                    };
                    if number.suffix().is_some() {
                        return Ok(None);
                    }
                    PrimitiveType::I32
                }
                SyntaxKind::FLOAT_NUMBER => {
                    let Some(number) = ast::FloatNumber::cast(literal.token()) else {
                        return Ok(None);
                    };
                    if number.suffix().is_some() {
                        return Ok(None);
                    }
                    PrimitiveType::F64
                }
                _ => return Ok(None),
            },
            ast::Expr::RefExpr(reference) => {
                if !primitive_operator_result
                    || reference.mut_token().is_some()
                    || reference.raw_token().is_some()
                    || reference.const_token().is_some()
                {
                    return Ok(None);
                }
                let Some(inner) = reference.expr() else {
                    return Ok(None);
                };
                // Both this nested-reference check and the outer traversal
                // share one budget. Multiple borrows cannot multiply uncharged
                // source work, and no guest borrow or dereference occurs here.
                for nested in inner.syntax().descendants() {
                    source_work(&mut visited)?;
                    if ast::RefExpr::can_cast(nested.kind()) {
                        return Ok(None);
                    }
                }
                continue;
            }
            ast::Expr::ParenExpr(_) => continue,
            ast::Expr::PrefixExpr(prefix) => {
                if !matches!(
                    prefix.op_token().as_ref().map(|t| t.text()),
                    Some("-" | "!")
                ) {
                    return Ok(None);
                }
                continue;
            }
            ast::Expr::BinExpr(binary) => {
                let Some(operator) = binary
                    .op_token()
                    .and_then(|t| super::thunks::binary_operator(t.text()))
                else {
                    return Ok(None);
                };
                if matches!(
                    operator,
                    NumericBinary::Eq
                        | NumericBinary::Ne
                        | NumericBinary::Lt
                        | NumericBinary::Le
                        | NumericBinary::Gt
                        | NumericBinary::Ge
                ) {
                    return Ok(None);
                }
                continue;
            }
            _ => return Ok(None),
        };
        if family.is_some_and(|old| old != next) {
            return Ok(None);
        }
        family = Some(next);
    }
    Ok(family)
}

/// A proven primitive cast target is a soft expectation, not the source type
/// of every cast operand. Pinned rustc passes it through Neg/Not to literals;
/// written suffixes and other source constraints retain their own types.
pub(super) fn cast_operand(
    expression: &ast::Expr,
    target: PrimitiveType,
) -> Result<Option<PrimitiveType>, Diagnostic> {
    let mut selected = expression.clone();
    let mut visited = 0usize;
    let mut parentheses = 0usize;
    loop {
        source_work(&mut visited)?;
        if selected
            .syntax()
            .children()
            .any(|node| ast::Attr::can_cast(node.kind()))
        {
            return literal_only(expression);
        }
        match selected {
            ast::Expr::ParenExpr(group) => {
                parentheses = parentheses
                    .checked_add(1)
                    .filter(|n| *n <= 256)
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "LRR-E-METADATA-LIMIT",
                            "numeric cast parenthesis limit".into(),
                        )
                    })?;
                let Some(inner) = group.expr() else {
                    return Ok(None);
                };
                selected = inner;
            }
            ast::Expr::PrefixExpr(prefix)
                if matches!(
                    prefix.op_token().as_ref().map(|token| token.text()),
                    Some("-" | "!")
                ) =>
            {
                let Some(inner) = prefix.expr() else {
                    return Ok(None);
                };
                selected = inner;
            }
            ast::Expr::Literal(literal) => {
                let expected = match literal.token().kind() {
                    SyntaxKind::INT_NUMBER
                        if ast::IntNumber::cast(literal.token())
                            .is_some_and(|number| number.suffix().is_none()) =>
                    {
                        match target {
                            PrimitiveType::I8
                            | PrimitiveType::I16
                            | PrimitiveType::I32
                            | PrimitiveType::I64
                            | PrimitiveType::Isize
                            | PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                            | PrimitiveType::Usize => Some(target),
                            PrimitiveType::Char => Some(PrimitiveType::U8),
                            _ => None,
                        }
                    }
                    SyntaxKind::FLOAT_NUMBER
                        if ast::FloatNumber::cast(literal.token())
                            .is_some_and(|number| number.suffix().is_none()) =>
                    {
                        match target {
                            PrimitiveType::F32 | PrimitiveType::F64 => Some(target),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                return if expected.is_some() {
                    Ok(expected)
                } else {
                    literal_only(expression)
                };
            }
            _ => return literal_only(expression),
        }
    }
}

/// A local literal can receive constraints from later uses of its binding.
/// Keep its unresolved scalar representation until those uses supply context.
/// An operator initializer separately needs a numeric result to run operands.
pub(super) fn operator_initializer(
    expression: &ast::Expr,
) -> Result<Option<PrimitiveType>, Diagnostic> {
    let mut root = expression.clone();
    let mut depth = 0usize;
    while let ast::Expr::ParenExpr(parenthesis) = root {
        depth = depth.checked_add(1).filter(|n| *n <= 256).ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "numeric initializer parenthesis limit".into(),
            )
        })?;
        let Some(inner) = parenthesis.expr() else {
            return Ok(None);
        };
        root = inner;
    }
    if matches!(root, ast::Expr::BinExpr(_) | ast::Expr::PrefixExpr(_)) {
        literal_only(expression)
    } else {
        Ok(None)
    }
}

fn source_work(visited: &mut usize) -> Result<(), Diagnostic> {
    *visited = visited
        .checked_add(1)
        .filter(|n| *n <= 100_000)
        .ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "numeric fallback source work limit".into(),
            )
        })?;
    Ok(())
}

pub(super) fn pair(
    left: &ast::Expr,
    right: &ast::Expr,
) -> Result<Option<PrimitiveType>, Diagnostic> {
    let left = literal_only(left)?;
    let right = literal_only(right)?;
    Ok(left.filter(|_| left == right))
}

pub(super) fn comparison(expression: &ast::BinExpr) -> Result<Option<PrimitiveType>, Diagnostic> {
    let Some(operator) = expression
        .op_token()
        .and_then(|t| super::thunks::binary_operator(t.text()))
    else {
        return Ok(None);
    };
    if !matches!(
        operator,
        NumericBinary::Eq
            | NumericBinary::Ne
            | NumericBinary::Lt
            | NumericBinary::Le
            | NumericBinary::Gt
            | NumericBinary::Ge
    ) {
        return Ok(None);
    }
    let (Some(left), Some(right)) = (expression.lhs(), expression.rhs()) else {
        return Ok(None);
    };
    pair(&left, &right)
}

pub(super) fn operand_hint(
    default: Option<PrimitiveType>,
    parent: SlotId,
    child: SlotId,
    left: Option<SlotId>,
    operator: NumericBinary,
) -> Op {
    match default {
        Some(primitive) => Op::SetHint {
            child,
            hint: HintRecipe::Primitive(primitive),
        },
        None => Op::NumericHint {
            parent,
            child,
            left,
            operator: Some(operator),
        },
    }
}

/// Keep an independently defaulted shared shift count a reference value.
/// Its pointee gets numeric context; the operand never gets a primitive contract.
pub(super) fn right_operand_hint(
    right: &ast::Expr,
    parent: SlotId,
    child: SlotId,
    left: Option<SlotId>,
    operator: NumericBinary,
    default: Option<PrimitiveType>,
    definition: crate::core::digest::Digest,
) -> Result<Op, Diagnostic> {
    if matches!(operator, NumericBinary::Shl | NumericBinary::Shr) && default.is_none() {
        let mut root = right.clone();
        let mut work = 0usize;
        loop {
            source_work(&mut work)?;
            if work > 256 {
                return Err(Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "shared shift count parenthesis limit".into(),
                ));
            }
            if root
                .syntax()
                .children()
                .any(|n| ast::Attr::can_cast(n.kind()))
            {
                break;
            }
            match root {
                ast::Expr::ParenExpr(group) => {
                    let Some(inner) = group.expr() else {
                        break;
                    };
                    root = inner;
                }
                ast::Expr::RefExpr(reference)
                    if reference.mut_token().is_none()
                        && reference.raw_token().is_none()
                        && reference.const_token().is_none() =>
                {
                    if let Some(inner) = reference.expr()
                        && literal_only(&inner)? == Some(PrimitiveType::I32)
                    {
                        let range = reference.syntax().text_range();
                        return Ok(Op::SetHint {
                            child,
                            hint: HintRecipe::Reference {
                                region: crate::traits::term::RegionTerm::Local {
                                    definition,
                                    start: u32::from(range.start()),
                                    end: u32::from(range.end()),
                                },
                                mutable: false,
                                target: PrimitiveType::I32,
                            },
                        });
                    }
                    break;
                }
                _ => break,
            }
        }
    }
    Ok(operand_hint(default, parent, child, left, operator))
}

/// A shift's right operand has an independent numeric inference island. Only
/// closed original syntax permits fallback here; a named operand may still be
/// constrained by later source uses. Suffixes retain their ordinary width.
pub(super) fn right_operand_default(
    right: &ast::Expr,
    operator: NumericBinary,
    comparison_default: Option<PrimitiveType>,
) -> Result<Option<PrimitiveType>, Diagnostic> {
    if matches!(operator, NumericBinary::Shl | NumericBinary::Shr) {
        literal_only(right)
    } else {
        Ok(comparison_default)
    }
}

#[cfg(test)]
mod closed_borrow_tests {
    use super::*;

    fn fallback(text: &str) -> Option<PrimitiveType> {
        let parsed = ast::SourceFile::parse(
            &format!("fn main() {{ let _ = {text}; }}"),
            ra_ap_syntax::Edition::Edition2024,
        );
        assert!(parsed.errors().is_empty(), "{text}: {:?}", parsed.errors());
        let statement = parsed
            .tree()
            .syntax()
            .descendants()
            .find_map(ast::LetStmt::cast)
            .unwrap();
        literal_only(&statement.initializer().unwrap()).unwrap()
    }

    #[test]
    fn closed_shared_operator_fallback() {
        for expression in ["1 / &0", "1 % &0", "(&1 + &2)", "-(&1)"] {
            assert_eq!(
                fallback(expression),
                Some(PrimitiveType::I32),
                "{expression}"
            );
        }
        assert_eq!(fallback("1.0 + &2.0"), Some(PrimitiveType::F64));
    }

    #[test]
    fn borrow_result_and_nested_borrow_are_not_primitive_fallback() {
        for expression in ["&1", "(&1)", "1 + &&2", "1 + &mut 2", "1 + &raw const 2"] {
            assert_eq!(fallback(expression), None, "{expression}");
        }
    }

    #[test]
    fn borrow_does_not_close_names_suffixes_casts_or_producers() {
        for expression in [
            "1 + &x",
            "1 + &f()",
            "1 + &2u8",
            "1 + &(2 as i32)",
            "1 + &value!()",
        ] {
            assert_eq!(fallback(expression), None, "{expression}");
        }
    }
}
