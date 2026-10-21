//! rustc's destructuring-assignment grammar (`rustc_ast_lowering`
//! `lower_expr_assign` / `destructure_assign`).
//!
//! An assignment whose left operand is a tuple, array, record or tuple-struct
//! expression, `_`, or a parenthesized such expression is not a place
//! assignment. rustc lowers `lhs = rhs` to
//! `{ let <pattern of lhs> = rhs; place0 = tmp0; place1 = tmp1; .. }`: the
//! left operand is read as a pattern whose leaves (any other expression) bind
//! temporaries that are then assigned, in order, to the leaf places.
//!
//! This module is the single syntactic definition of that shape. The body
//! index, the retained pattern tree, the shared pattern-owner lowering and the
//! place-context rule all ask it, so handwritten, macro-emitted and nested
//! forms take the same path.
use ra_ap_syntax::{
    AstNode, SyntaxKind, SyntaxNode,
    ast::{self, HasArgList, RangeItem},
};

/// One node of an assignee expression read as a pattern.
#[derive(Debug, Clone)]
pub(crate) enum Assignee {
    /// `_`: discards its part.
    Wildcard,
    /// `..` inside a tuple, array or tuple-struct list.
    Rest,
    /// Any other expression: one assigned place.
    Leaf(ast::Expr),
    Tuple(Vec<Assignee>),
    Array(Vec<Assignee>),
    /// `Path(a, ..)`: the callee names a tuple struct or variant.
    TupleStruct {
        callee: ast::PathExpr,
        items: Vec<Assignee>,
    },
    /// `Path { f, g: p, .. }`.
    Record {
        expression: ast::RecordExpr,
        fields: Vec<(String, Assignee)>,
        rest: bool,
    },
}

/// Whether `range` is the bare `..` rest marker (`(a, .., b)`, `(..)`).
fn bare_rest(range: &ast::RangeExpr) -> bool {
    range.start().is_none()
        && range.end().is_none()
        && range.op_kind() == Some(ast::RangeOp::Exclusive)
}

/// Whether an expression makes an assignment whose left operand it is a
/// destructuring one (rustc's `!is_ordinary`).
fn destructuring_root(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::TupleExpr(_)
        | ast::Expr::ArrayExpr(_)
        | ast::Expr::RecordExpr(_)
        | ast::Expr::UnderscoreExpr(_) => true,
        ast::Expr::CallExpr(call) => matches!(call.expr(), Some(ast::Expr::PathExpr(_))),
        ast::Expr::ParenExpr(group) => match group.expr() {
            // `(..)` is special-cased for consistency with patterns.
            Some(ast::Expr::RangeExpr(range)) if bare_rest(&range) => true,
            Some(inner) => destructuring_root(&inner),
            None => false,
        },
        _ => false,
    }
}

/// The left operand of a destructuring assignment, if `binary` is one.
pub(crate) fn destructuring_lhs(binary: &ast::BinExpr) -> Option<ast::Expr> {
    if !binary.op_token().is_some_and(|token| token.text() == "=") {
        return None;
    }
    let lhs = binary.lhs()?;
    destructuring_root(&lhs).then_some(lhs)
}

/// Read an assignee expression as a pattern.
pub(crate) fn assignee(expr: &ast::Expr) -> Assignee {
    match expr {
        ast::Expr::UnderscoreExpr(_) => Assignee::Wildcard,
        ast::Expr::ParenExpr(group) => match group.expr() {
            Some(ast::Expr::RangeExpr(range)) if bare_rest(&range) => {
                Assignee::Tuple(vec![Assignee::Rest])
            }
            Some(inner) if destructuring_root(&inner) => assignee(&inner),
            _ => Assignee::Leaf(expr.clone()),
        },
        ast::Expr::TupleExpr(tuple) => Assignee::Tuple(items(tuple.fields())),
        ast::Expr::ArrayExpr(array) => match array.kind() {
            ast::ArrayExprKind::ElementList(elements) => Assignee::Array(items(elements)),
            ast::ArrayExprKind::Repeat { .. } => Assignee::Leaf(expr.clone()),
        },
        ast::Expr::CallExpr(call) => match (call.expr(), call.arg_list()) {
            (Some(ast::Expr::PathExpr(callee)), Some(arguments)) => Assignee::TupleStruct {
                callee,
                items: items(arguments.args()),
            },
            _ => Assignee::Leaf(expr.clone()),
        },
        ast::Expr::RecordExpr(record) => {
            let Some(list) = record.record_expr_field_list() else {
                return Assignee::Leaf(expr.clone());
            };
            let mut fields = Vec::new();
            for field in list.fields() {
                let (Some(name), Some(value)) = (field.field_name(), field.expr()) else {
                    return Assignee::Leaf(expr.clone());
                };
                fields.push((name.to_string(), item(&value)));
            }
            Assignee::Record {
                expression: record.clone(),
                fields,
                rest: list.dotdot_token().is_some() && list.spread().is_none(),
            }
        }
        other => Assignee::Leaf(other.clone()),
    }
}
/// A component of a composite: the same grammar, plus `..`.
fn item(expr: &ast::Expr) -> Assignee {
    match expr {
        ast::Expr::RangeExpr(range) if bare_rest(range) => Assignee::Rest,
        other => assignee(other),
    }
}
fn items(list: impl Iterator<Item = ast::Expr>) -> Vec<Assignee> {
    list.map(|component| item(&component)).collect()
}

/// Every expression node of a destructuring left operand in source order,
/// each with whether it is a leaf place (`true`) or pattern shape (`false`:
/// tuple, array, record, tuple-struct, `_`, `..`, parentheses). These are the
/// nodes the body index registers as operands of the assignment itself.
pub(crate) fn region(lhs: &ast::Expr) -> Vec<(SyntaxNode, bool)> {
    let mut out = Vec::new();
    walk(lhs, &mut out);
    out
}
fn walk(expr: &ast::Expr, out: &mut Vec<(SyntaxNode, bool)>) {
    match expr {
        ast::Expr::UnderscoreExpr(_) => out.push((expr.syntax().clone(), false)),
        ast::Expr::ParenExpr(group) => match group.expr() {
            Some(inner)
                if destructuring_root(&inner)
                    || matches!(&inner, ast::Expr::RangeExpr(range) if bare_rest(range)) =>
            {
                out.push((expr.syntax().clone(), false));
                walk_item(&inner, out);
            }
            _ => out.push((expr.syntax().clone(), true)),
        },
        ast::Expr::TupleExpr(tuple) => {
            out.push((expr.syntax().clone(), false));
            for field in tuple.fields() {
                walk_item(&field, out);
            }
        }
        ast::Expr::ArrayExpr(array) => match array.kind() {
            ast::ArrayExprKind::ElementList(elements) => {
                out.push((expr.syntax().clone(), false));
                for element in elements {
                    walk_item(&element, out);
                }
            }
            ast::ArrayExprKind::Repeat { .. } => out.push((expr.syntax().clone(), true)),
        },
        ast::Expr::CallExpr(call) if destructuring_root(expr) => {
            out.push((expr.syntax().clone(), false));
            // The callee path names a constructor; it is shape, not a place.
            if let Some(callee) = call.expr() {
                out.push((callee.syntax().clone(), false));
            }
            if let Some(arguments) = call.arg_list() {
                for argument in arguments.args() {
                    walk_item(&argument, out);
                }
            }
        }
        ast::Expr::RecordExpr(record) => {
            out.push((expr.syntax().clone(), false));
            if let Some(list) = record.record_expr_field_list() {
                for field in list.fields() {
                    if let Some(value) = field.expr() {
                        walk_item(&value, out);
                    }
                }
            }
        }
        other => out.push((other.syntax().clone(), true)),
    }
}
fn walk_item(expr: &ast::Expr, out: &mut Vec<(SyntaxNode, bool)>) {
    match expr {
        ast::Expr::RangeExpr(range) if bare_rest(range) => {
            out.push((expr.syntax().clone(), false));
        }
        other => walk(other, out),
    }
}

/// The destructuring assignment whose left operand contains `syntax`, and
/// whether `syntax` is one of its leaf places (`false`: pattern shape).
/// `None` for any expression outside such a region.
pub(crate) fn enclosing(syntax: &SyntaxNode) -> Option<(ast::BinExpr, bool)> {
    let mut current = syntax.clone();
    loop {
        let parent = current.parent()?;
        if let Some(binary) = ast::BinExpr::cast(parent.clone()) {
            let lhs = destructuring_lhs(&binary)?;
            if lhs.syntax() != &current {
                return None;
            }
            let leaf = region(&lhs)
                .into_iter()
                .find(|(node, _)| node == syntax)?
                .1;
            return Some((binary, leaf));
        }
        if !matches!(
            parent.kind(),
            SyntaxKind::PAREN_EXPR
                | SyntaxKind::TUPLE_EXPR
                | SyntaxKind::ARRAY_EXPR
                | SyntaxKind::RECORD_EXPR
                | SyntaxKind::RECORD_EXPR_FIELD_LIST
                | SyntaxKind::RECORD_EXPR_FIELD
                | SyntaxKind::ARG_LIST
                | SyntaxKind::CALL_EXPR
        ) {
            return None;
        }
        current = parent;
    }
}

/// Whether `expr` is the bare `..` rest marker of an assignee list.
pub(crate) fn is_rest(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::RangeExpr(range) if bare_rest(range))
}

/// The expression a tuple or array assignee shape is matched against: the
/// right operand, descended in parallel with the left operand's tuple/array
/// structure (parentheses are transparent on both sides).
fn counterpart(binary: &ast::BinExpr, shape: &SyntaxNode) -> Option<ast::Expr> {
    fn peel(mut expr: ast::Expr) -> ast::Expr {
        while let ast::Expr::ParenExpr(group) = &expr {
            match group.expr() {
                Some(inner) => expr = inner,
                None => break,
            }
        }
        expr
    }
    fn elements(expr: &ast::Expr) -> Option<Vec<ast::Expr>> {
        match peel(expr.clone()) {
            ast::Expr::TupleExpr(tuple) => Some(tuple.fields().collect()),
            ast::Expr::ArrayExpr(array) => match array.kind() {
                ast::ArrayExprKind::ElementList(list) => Some(list.collect()),
                ast::ArrayExprKind::Repeat { .. } => None,
            },
            _ => None,
        }
    }
    fn descend(lhs: &ast::Expr, rhs: &ast::Expr, shape: &SyntaxNode) -> Option<ast::Expr> {
        let mut lhs = lhs.clone();
        loop {
            if lhs.syntax() == shape {
                return Some(peel(rhs.clone()));
            }
            match &lhs {
                ast::Expr::ParenExpr(group) => lhs = group.expr()?,
                _ => break,
            }
        }
        let (left, right) = (elements(&lhs)?, elements(rhs)?);
        let rest = left.iter().position(is_rest);
        for (position, item) in left.iter().enumerate() {
            let counterpart = match rest {
                Some(rest) if position > rest => {
                    right.get(right.len().checked_sub(left.len() - position)?)
                }
                Some(rest) if position == rest => continue,
                _ => right.get(position),
            };
            if let Some(found) = descend(item, counterpart?, shape) {
                return Some(found);
            }
        }
        None
    }
    descend(&destructuring_lhs(binary)?, &binary.rhs()?, shape)
}

/// The operands a tuple or array assignee shape contributes to the source
/// constraint facts, aligned with the value it is matched against. `_` stays a
/// component (a wildcard fact: a fresh, unconstrained variable); `..` expands
/// to as many such components as the matched tuple has unwritten positions
/// (none for an array, whose elements share one Type). `None` when a tuple's
/// `..` cannot be aligned because the matched value is not a written tuple.
pub(crate) fn fact_operands(shape: &ast::Expr) -> Option<Vec<ast::Expr>> {
    let items: Vec<ast::Expr> = match shape {
        ast::Expr::TupleExpr(tuple) => tuple.fields().collect(),
        ast::Expr::ArrayExpr(array) => match array.kind() {
            ast::ArrayExprKind::ElementList(list) => list.collect(),
            ast::ArrayExprKind::Repeat { .. } => return None,
        },
        _ => return None,
    };
    let Some(rest) = items.iter().position(is_rest) else {
        return Some(items);
    };
    if matches!(shape, ast::Expr::ArrayExpr(_)) {
        return Some(items.into_iter().filter(|item| !is_rest(item)).collect());
    }
    let (binary, _) = enclosing(shape.syntax())?;
    let matched = match counterpart(&binary, shape.syntax())? {
        ast::Expr::TupleExpr(tuple) => tuple.fields().count(),
        _ => return None,
    };
    let fill = matched.checked_sub(items.len() - 1)?;
    let marker = items[rest].clone();
    let mut aligned = Vec::with_capacity(matched);
    aligned.extend(items[..rest].iter().cloned());
    aligned.extend(std::iter::repeat_n(marker, fill));
    aligned.extend(items[rest + 1..].iter().cloned());
    Some(aligned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ra_ap_syntax::{Edition, SourceFile};

    fn assignment(text: &str) -> ast::BinExpr {
        let parse = SourceFile::parse(&format!("fn f() {{ {text}; }}"), Edition::Edition2024);
        assert!(parse.errors().is_empty(), "{:?}", parse.errors());
        parse
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::BinExpr::cast)
            .find(|binary| binary.op_token().is_some_and(|token| token.text() == "="))
            .expect("assignment")
    }

    #[test]
    fn only_tuple_array_record_call_and_underscore_assignees_destructure() {
        for destructures in [
            "(a, b) = r",
            "((a, b)) = r",
            "[a, b] = r",
            "S { a, b } = r",
            "T(a, b) = r",
            "E::V(a) = r",
            "_ = r",
            "(..) = r",
            "() = r",
        ] {
            assert!(
                destructuring_lhs(&assignment(destructures)).is_some(),
                "{destructures}"
            );
        }
        for ordinary in ["a = r", "(a) = r", "*p = r", "v[i] = r", "s.f = r", "s.0 = r"] {
            assert!(destructuring_lhs(&assignment(ordinary)).is_none(), "{ordinary}");
        }
        // A compound assignment is never a destructuring one.
        let compound = SourceFile::parse("fn f() { (a, b) += r; }", Edition::Edition2024);
        let binary = compound
            .tree()
            .syntax()
            .descendants()
            .find_map(ast::BinExpr::cast)
            .expect("binary");
        assert!(destructuring_lhs(&binary).is_none());
    }

    #[test]
    fn region_lists_shape_and_leaf_places_in_source_order() {
        let binary = assignment("((a, b), v[i], _, S { f: s.g, .. }, ..) = r");
        let lhs = destructuring_lhs(&binary).expect("destructuring");
        let kinds: Vec<(String, bool)> = region(&lhs)
            .into_iter()
            .map(|(node, leaf)| (node.text().to_string(), leaf))
            .collect();
        let leaves: Vec<&str> = kinds
            .iter()
            .filter(|(_, leaf)| *leaf)
            .map(|(text, _)| text.as_str())
            .collect();
        assert_eq!(leaves, ["a", "b", "v[i]", "s.g"]);
        // Shape: both tuples, `_`, the record, `..`; never an index operand.
        assert_eq!(kinds.iter().filter(|(_, leaf)| !*leaf).count(), 5);
        assert!(!kinds.iter().any(|(text, _)| text == "i"));
        // Every region node (and nothing beneath a leaf) resolves back to it.
        for (node, leaf) in region(&lhs) {
            let (owner, found) = enclosing(&node).expect("region member");
            assert_eq!(owner.syntax(), binary.syntax());
            assert_eq!(found, leaf);
        }
        let inner = binary
            .syntax()
            .descendants()
            .filter_map(ast::PathExpr::cast)
            .find(|path| path.syntax().text() == "i")
            .expect("index operand");
        assert!(enclosing(inner.syntax()).is_none());
        // The right operand is outside the assignee.
        let rhs = binary.rhs().expect("rhs");
        assert!(enclosing(rhs.syntax()).is_none());
    }

    #[test]
    fn rest_spans_the_matched_tuple_but_names_no_array_element() {
        let operands = |text: &str| {
            let binary = assignment(text);
            let lhs = destructuring_lhs(&binary).expect("destructuring");
            fact_operands(&lhs).map(|list| {
                list.iter()
                    .map(|operand| operand.syntax().text().to_string())
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(
            operands("(a, .., b) = (1, 2, 3, 4)").expect("aligned"),
            ["a", "..", "..", "b"]
        );
        assert_eq!(
            operands("(.., b) = (1, 2)").expect("aligned"),
            ["..", "b"]
        );
        assert_eq!(operands("(a, ..) = ((1, 2), 3)").expect("aligned"), ["a", ".."]);
        assert_eq!(
            operands("[a, .., b] = [1, 2, 3, 4]").expect("aligned"),
            ["a", "b"]
        );
        assert_eq!(operands("(a, _) = (1, 2)").expect("aligned"), ["a", "_"]);
        // A tuple whose matched value is not written as a tuple cannot place `..`.
        assert!(operands("(a, .., b) = f()").is_none());
    }
}
