//! Owned stock-fixture facts for runtime tests. No parser identities cross layers.
use super::{parse::ParsedFile, spans::SourceRange};
use crate::{core::profile::ProfileIdentity, source::snapshot::SourceBlob};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasArgList, HasName},
};

pub(crate) struct BorrowFacts {
    pub mutable: bool,
    pub raw: bool,
    pub target: String,
}
pub(crate) enum InitializerFacts {
    Unit(usize),
    Integer(i64),
    Borrow(BorrowFacts),
}
pub(crate) struct BindingFacts {
    pub writable: bool,
    pub annotation: Option<String>,
    pub initializer: InitializerFacts,
}
fn parsed(source: &str) -> ParsedFile {
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    assert!(parsed.errors.is_empty());
    parsed
}
pub(crate) fn temporary_record_field(source: &str) -> (String, i64) {
    let parsed = parsed(source);
    let field = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::FieldExpr::cast)
        .find(|field| matches!(field.expr(), Some(ast::Expr::RecordExpr(_))))
        .expect("original temporary record field absent");
    let member = field.name_ref().unwrap().text().to_string();
    let Some(ast::Expr::RecordExpr(record)) = field.expr() else {
        unreachable!()
    };
    let initializer = record
        .record_expr_field_list()
        .unwrap()
        .fields()
        .find(|field| {
            field
                .name_ref()
                .is_some_and(|name| name.text() == member.as_str())
        })
        .unwrap()
        .expr()
        .unwrap();
    (
        member,
        initializer.syntax().text().to_string().parse().unwrap(),
    )
}
pub(crate) fn parenthesized_field_ranges(source: &str, name: &str) -> Vec<SourceRange> {
    let function = function(source, name);
    let body = function.body().unwrap();
    let start = u32::from(body.syntax().text_range().start());
    body.syntax()
        .descendants()
        .filter_map(ast::ParenExpr::cast)
        .filter(|paren| matches!(paren.expr(), Some(ast::Expr::FieldExpr(_))))
        .map(|paren| SourceRange {
            start: u32::from(paren.syntax().text_range().start())
                .checked_sub(start)
                .unwrap(),
            end: u32::from(paren.syntax().text_range().end())
                .checked_sub(start)
                .unwrap(),
        })
        .collect()
}
pub(crate) struct ComputedDerefFacts {
    pub binding_statement: SourceRange,
    pub binding_pattern: SourceRange,
    pub target: crate::ir::op::PrimitiveType,
    pub assigned: u64,
}
/// Disclosed source-bound context for the original `f` expression: its later
/// `x.1 = a` constrains `a` to the written second tuple element type. This fact
/// reader does not perform general inference or execute const qualification.
pub(crate) fn computed_deref_facts(source: &str) -> ComputedDerefFacts {
    let body = function(source, "f").body().unwrap();
    let start = u32::from(body.syntax().text_range().start());
    let statements: Vec<_> = body
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .collect();
    let x = statements.iter().find(|statement| matches!(statement.pat(), Some(ast::Pat::IdentPat(pat)) if pat.name().is_some_and(|name| name.text() == "x"))).unwrap();
    let Some(ast::Type::TupleType(tuple)) = x.ty() else {
        panic!("original x tuple annotation absent")
    };
    let element = tuple.fields().nth(1).unwrap();
    let target = match element.syntax().text().to_string().as_str() {
        "u32" => crate::ir::op::PrimitiveType::U32,
        _ => panic!("original computed-reference target differs from its written context"),
    };
    assert!(
        body.syntax()
            .descendants()
            .filter_map(ast::BinExpr::cast)
            .any(|assignment| {
                assignment.op_token().is_some_and(|op| op.text() == "=")
                    && matches!(assignment.lhs(), Some(ast::Expr::FieldExpr(field))
                if field.name_ref().is_some_and(|name| name.text() == "1")
                && matches!(field.expr(), Some(ast::Expr::PathExpr(path))
                    if path.syntax().text() == "x"))
                    && matches!(assignment.rhs(), Some(ast::Expr::PathExpr(path))
                if path.syntax().text() == "a")
            }),
        "original later x.1=a source constraint absent"
    );
    let binding = statements.iter().find(|statement| matches!(statement.pat(), Some(ast::Pat::IdentPat(pat)) if pat.name().is_some_and(|name| name.text() == "a"))).unwrap();
    let pattern = binding.pat().unwrap();
    let range = |syntax: &ra_ap_syntax::SyntaxNode| {
        SourceRange::checked(
            u32::from(syntax.text_range().start())
                .checked_sub(start)
                .unwrap(),
            u32::from(syntax.text_range().end())
                .checked_sub(start)
                .unwrap(),
        )
        .unwrap()
    };
    let assignment = body.syntax().descendants().filter_map(ast::BinExpr::cast)
        .find(|assignment| assignment.op_token().is_some_and(|op| op.text() == "=")
            && matches!(assignment.lhs(),Some(ast::Expr::PrefixExpr(prefix)) if prefix.op_token().is_some_and(|op| op.text() == "*"))).unwrap();
    ComputedDerefFacts {
        binding_statement: range(binding.syntax()),
        binding_pattern: range(pattern.syntax()),
        target,
        assigned: assignment
            .rhs()
            .unwrap()
            .syntax()
            .text()
            .to_string()
            .parse()
            .unwrap(),
    }
}
fn function(source: &str, name: &str) -> ast::Fn {
    parsed(source)
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().is_some_and(|found| found.text() == name))
        .unwrap()
}
pub(crate) struct LocalAssignmentFacts {
    pub pattern: SourceRange,
    pub assignment: SourceRange,
    pub writable: bool,
    pub annotation: Option<String>,
    pub initial_text: Option<String>,
    pub incoming_text: Option<String>,
    pub incoming_integer: Option<i64>,
    pub incoming_unit: bool,
}
/// Select the unchanged local assignment and its written initializer values.
/// Tests supply completed RHS values; closures, methods, diagnostics and the
/// remaining source operations are separate pending integration requirements.
pub(crate) fn local_assignment(source: &str, local: &str, ordinal: usize) -> LocalAssignmentFacts {
    let block = function(source, "main").body().unwrap();
    let start = u32::from(block.syntax().text_range().start());
    let statement = block.syntax().descendants().filter_map(ast::LetStmt::cast)
        .find(|statement| matches!(statement.pat(), Some(ast::Pat::IdentPat(pat)) if pat.name().is_some_and(|name| name.text() == local))).unwrap();
    let ast::Pat::IdentPat(pattern) = statement.pat().unwrap() else {
        panic!("original assignment binding absent")
    };
    let assignment = block.syntax().descendants().filter_map(ast::BinExpr::cast)
        .filter(|assignment| assignment.op_token().is_some_and(|op| op.text() == "=")
            && matches!(assignment.lhs(), Some(ast::Expr::PathExpr(path)) if path.syntax().text() == local))
        .nth(ordinal).unwrap();
    let range = |node: &ra_ap_syntax::SyntaxNode| {
        SourceRange::checked(
            u32::from(node.text_range().start())
                .checked_sub(start)
                .unwrap(),
            u32::from(node.text_range().end())
                .checked_sub(start)
                .unwrap(),
        )
        .unwrap()
    };
    let text_value = |expression: ast::Expr| match expression {
        ast::Expr::MethodCallExpr(call) => {
            assert_eq!(call.name_ref().unwrap().text(), "to_string");
            assert_eq!(call.arg_list().unwrap().args().count(), 0);
            let ast::Expr::Literal(literal) = call.receiver().unwrap() else {
                panic!("original text receiver absent")
            };
            let ast::LiteralKind::String(text) = literal.kind() else {
                panic!("original receiver is not a string literal")
            };
            Some(text.value().unwrap().into_owned())
        }
        ast::Expr::CallExpr(call) => {
            assert_eq!(call.expr().unwrap().syntax().text(), "String::new");
            assert_eq!(call.arg_list().unwrap().args().count(), 0);
            Some(String::new())
        }
        ast::Expr::Literal(_) => None,
        ast::Expr::TupleExpr(tuple) if tuple.fields().count() == 0 => None,
        _ => panic!("original RHS is outside the selected assignment facts"),
    };
    let rhs = assignment.rhs().unwrap();
    let incoming_integer = if matches!(&rhs, ast::Expr::Literal(_)) {
        Some(rhs.syntax().text().to_string().parse().unwrap())
    } else {
        None
    };
    let incoming_unit = matches!(&rhs, ast::Expr::TupleExpr(tuple) if tuple.fields().count() == 0);
    let initial_text = if incoming_unit {
        assert_eq!(statement.ty().unwrap().syntax().text(), "()");
        let ast::Expr::CallExpr(call) = statement.initializer().unwrap() else {
            panic!("stock Unit initializer call absent")
        };
        assert_eq!(call.expr().unwrap().syntax().text(), "f");
        let arguments: Vec<_> = call.arg_list().unwrap().args().collect();
        assert_eq!(arguments.len(), 1);
        assert_eq!(arguments[0].syntax().text(), "u1");
        None
    } else {
        statement.initializer().and_then(text_value)
    };
    LocalAssignmentFacts {
        pattern: range(pattern.syntax()),
        assignment: range(assignment.syntax()),
        writable: pattern.mut_token().is_some(),
        annotation: statement.ty().map(|ty| ty.syntax().text().to_string()),
        initial_text,
        incoming_text: text_value(rhs),
        incoming_integer,
        incoming_unit,
    }
}
fn borrow_facts(borrow: ast::RefExpr) -> BorrowFacts {
    BorrowFacts {
        mutable: borrow.mut_token().is_some(),
        raw: borrow.raw_token().is_some(),
        target: borrow.expr().unwrap().syntax().text().to_string(),
    }
}
pub(crate) fn binding(source: &str, name: &str) -> BindingFacts {
    let main = function(source, "main");
    let statement = main.body().unwrap().syntax().descendants().filter_map(ast::LetStmt::cast)
        .find(|statement| matches!(statement.pat(), Some(ast::Pat::IdentPat(pat)) if pat.name().is_some_and(|found| found.text() == name))).unwrap();
    let ast::Pat::IdentPat(pattern) = statement.pat().unwrap() else {
        panic!("stock binding pattern absent")
    };
    let initializer = match statement.initializer().unwrap() {
        ast::Expr::TupleExpr(unit) => InitializerFacts::Unit(unit.fields().count()),
        ast::Expr::RefExpr(borrow) => InitializerFacts::Borrow(borrow_facts(borrow)),
        ast::Expr::Literal(literal) => {
            InitializerFacts::Integer(literal.syntax().text().to_string().parse().unwrap())
        }
        _ => panic!("stock binding initializer is outside this fixture-fact reader"),
    };
    BindingFacts {
        writable: pattern.mut_token().is_some(),
        annotation: statement.ty().map(|ty| ty.syntax().text().to_string()),
        initializer,
    }
}
pub(crate) fn borrows_of(source: &str, target: &str) -> Vec<BorrowFacts> {
    function(source, "main")
        .body()
        .unwrap()
        .syntax()
        .descendants()
        .filter_map(ast::RefExpr::cast)
        .filter(|borrow| {
            borrow
                .expr()
                .is_some_and(|expr| expr.syntax().text() == target)
        })
        .map(borrow_facts)
        .collect()
}
pub(crate) fn point(source: &str, name: &str) -> (bool, Vec<i64>) {
    let function = function(source, name);
    let body = function.body().unwrap();
    let statement = body
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .next()
        .unwrap();
    let ast::Pat::IdentPat(pattern) = statement.pat().unwrap() else {
        panic!("stock Point binding absent")
    };
    assert_eq!(pattern.name().unwrap().text(), "p");
    let record = body
        .syntax()
        .descendants()
        .filter_map(ast::RecordExpr::cast)
        .next()
        .unwrap();
    assert_eq!(record.path().unwrap().syntax().text().to_string(), "Point");
    (pattern.mut_token().is_some(), record_values(record))
}
fn record_values(record: ast::RecordExpr) -> Vec<i64> {
    assert_eq!(record.path().unwrap().syntax().text().to_string(), "Point");
    let list = record.record_expr_field_list().unwrap();
    assert!(list.spread().is_none());
    let fields: Vec<_> = list.fields().collect();
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name_ref().unwrap().text().to_string())
            .collect::<Vec<_>>(),
        ["x", "y"]
    );
    fields
        .into_iter()
        .map(|field| {
            field
                .expr()
                .unwrap()
                .syntax()
                .text()
                .to_string()
                .parse()
                .unwrap()
        })
        .collect()
}
pub(crate) fn point_replacement(source: &str, name: &str) -> Vec<i64> {
    let body = function(source, name).body().unwrap();
    let assignment = body
        .syntax()
        .descendants()
        .filter_map(ast::BinExpr::cast)
        .find(|expression| {
            expression
                .op_token()
                .is_some_and(|token| token.text() == "=")
                && expression
                    .lhs()
                    .is_some_and(|lhs| lhs.syntax().text() == "p")
        })
        .unwrap();
    let ast::Expr::RecordExpr(record) = assignment.rhs().unwrap() else {
        panic!("original Point replacement absent")
    };
    record_values(record)
}
pub(crate) fn assignment(source: &str, name: &str) -> (String, i64) {
    let body = function(source, name).body().unwrap();
    let assignment = body
        .syntax()
        .descendants()
        .filter_map(ast::BinExpr::cast)
        .next()
        .unwrap();
    assert_eq!(assignment.op_token().unwrap().text(), "=");
    (
        assignment.lhs().unwrap().syntax().text().to_string(),
        assignment
            .rhs()
            .unwrap()
            .syntax()
            .text()
            .to_string()
            .parse()
            .unwrap(),
    )
}
pub(crate) fn empty_index(source: &str) -> (u32, u64) {
    let parsed = parsed(source);
    let tree = parsed.parsed.tree();
    let array = tree
        .syntax()
        .descendants()
        .filter_map(ast::ArrayType::cast)
        .next()
        .unwrap();
    let index = tree
        .syntax()
        .descendants()
        .filter_map(ast::IndexExpr::cast)
        .next()
        .unwrap();
    (
        array
            .const_arg()
            .unwrap()
            .syntax()
            .text()
            .to_string()
            .parse()
            .unwrap(),
        index
            .index()
            .unwrap()
            .syntax()
            .text()
            .to_string()
            .parse()
            .unwrap(),
    )
}
pub(crate) fn array_binding(source: &str) -> Vec<i64> {
    let main = function(source, "main");
    let statement = main
        .body()
        .unwrap()
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .next()
        .unwrap();
    let ast::Expr::ArrayExpr(array) = statement.initializer().unwrap() else {
        panic!("stock array initializer absent")
    };
    array
        .exprs()
        .map(|element| element.syntax().text().to_string().parse().unwrap())
        .collect()
}
pub(crate) struct AggregateCopyFacts {
    pub pattern: SourceRange,
    pub read: SourceRange,
    pub values: Vec<i64>,
    pub tuple: bool,
}
/// The exact owning read from the original tuple/array Copy test. Runtime
/// harnesses disclose concrete element context instead of claiming inference.
pub(crate) fn aggregate_copy_read(source: &str, binding: &str, copied: &str) -> AggregateCopyFacts {
    let main = function(source, "main");
    let body = main.body().unwrap();
    let start = u32::from(body.syntax().text_range().start());
    let statements: Vec<_> = body
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .collect();
    let selected = |name: &str| {
        statements
            .iter()
            .find(|statement| {
                statement
                    .pat()
                    .is_some_and(|pat| pat.syntax().text() == name)
            })
            .unwrap()
    };
    let original = selected(binding);
    let copy = selected(copied);
    let initializer = original.initializer().unwrap();
    let (tuple, values) = match initializer {
        ast::Expr::TupleExpr(tuple) => {
            let alias = parsed(source)
                .parsed
                .tree()
                .syntax()
                .descendants()
                .filter_map(ast::TypeAlias::cast)
                .find(|alias| alias.name().is_some_and(|name| name.text() == "point"))
                .unwrap();
            let ast::Type::TupleType(fields) = alias.ty().unwrap() else {
                panic!("original point tuple alias absent")
            };
            assert!(fields.fields().all(|ty| ty.syntax().text() == "isize"));
            assert_eq!(original.ty().unwrap().syntax().text().to_string(), "point");
            (
                true,
                tuple
                    .fields()
                    .map(|field| field.syntax().text().to_string().parse().unwrap())
                    .collect(),
            )
        }
        ast::Expr::ArrayExpr(array) => (
            false,
            array
                .exprs()
                .map(|field| field.syntax().text().to_string().parse().unwrap())
                .collect(),
        ),
        _ => panic!("original aggregate initializer absent"),
    };
    let read = copy.initializer().unwrap();
    assert_eq!(read.syntax().text().to_string(), binding);
    let range = |syntax: &ra_ap_syntax::SyntaxNode| SourceRange {
        start: u32::from(syntax.text_range().start())
            .checked_sub(start)
            .unwrap(),
        end: u32::from(syntax.text_range().end())
            .checked_sub(start)
            .unwrap(),
    };
    AggregateCopyFacts {
        pattern: range(original.pat().unwrap().syntax()),
        read: range(read.syntax()),
        values,
        tuple,
    }
}
/// Owned exact ranges let an outside harness enter an original expression while
/// explicitly leaving earlier unsupported patterns/borrows unexercised.
pub(crate) fn index_ranges(source: &str) -> Vec<SourceRange> {
    let main = function(source, "main");
    let body = main.body().unwrap();
    let start = u32::from(body.syntax().text_range().start());
    body.syntax()
        .descendants()
        .filter_map(ast::IndexExpr::cast)
        .map(|expr| {
            let range = expr.syntax().text_range();
            SourceRange::checked(
                u32::from(range.start()) - start,
                u32::from(range.end()) - start,
            )
            .unwrap()
        })
        .collect()
}
pub(crate) fn borrow_ranges_of(source: &str, name: &str) -> Vec<SourceRange> {
    let main = function(source, "main");
    let body = main.body().unwrap();
    let start = u32::from(body.syntax().text_range().start());
    body.syntax()
        .descendants()
        .filter_map(ast::RefExpr::cast)
        .filter(|expr| {
            expr.expr()
                .is_some_and(|child| child.syntax().text() == name)
        })
        .map(|expr| {
            let range = expr.syntax().text_range();
            SourceRange::checked(
                u32::from(range.start()).checked_sub(start).unwrap(),
                u32::from(range.end()).checked_sub(start).unwrap(),
            )
            .unwrap()
        })
        .collect()
}
pub(crate) fn tuple_selector_ranges(source: &str) -> Vec<(SourceRange, Vec<u32>)> {
    let main = function(source, "main");
    let body = main.body().unwrap();
    let start = u32::from(body.syntax().text_range().start());
    body.syntax()
        .descendants()
        .filter_map(ast::FieldExpr::cast)
        .filter(|expr| {
            expr.syntax()
                .parent()
                .and_then(ast::FieldExpr::cast)
                .is_none()
        })
        .map(|expr| {
            let range = expr.syntax().text_range();
            let mut selectors = Vec::new();
            let mut current = expr;
            loop {
                selectors.push(current.name_ref().unwrap().text().parse().unwrap());
                let Some(ast::Expr::FieldExpr(base)) = current.expr() else {
                    break;
                };
                current = base;
            }
            selectors.reverse();
            (
                SourceRange::checked(
                    u32::from(range.start()) - start,
                    u32::from(range.end()) - start,
                )
                .unwrap(),
                selectors,
            )
        })
        .collect()
}
pub(crate) fn nested_unit_tuple_depth(source: &str) -> usize {
    let main = function(source, "main");
    let mut initializer = main
        .body()
        .unwrap()
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .next()
        .unwrap()
        .initializer()
        .unwrap();
    let mut depth = 0usize;
    loop {
        let ast::Expr::TupleExpr(tuple) = initializer else {
            panic!("expected original nested tuple")
        };
        let fields: Vec<_> = tuple.fields().collect();
        if fields.is_empty() {
            return depth;
        }
        assert_eq!(fields.len(), 1);
        depth += 1;
        initializer = fields[0].clone();
    }
}
pub(crate) fn first_main_boolean(source: &str) -> bool {
    let main = function(source, "main");
    let token = main
        .body()
        .unwrap()
        .syntax()
        .descendants_with_tokens()
        .filter_map(|item| item.into_token())
        .find(|token| {
            matches!(
                token.kind(),
                ra_ap_syntax::SyntaxKind::TRUE_KW | ra_ap_syntax::SyntaxKind::FALSE_KW
            )
        })
        .unwrap();
    token.kind() == ra_ap_syntax::SyntaxKind::TRUE_KW
}

pub(crate) fn dereference_ranges(source: &str) -> Vec<super::spans::SourceRange> {
    let main = function(source, "main");
    let body = main.body().unwrap();
    let start = u32::from(body.syntax().text_range().start());
    body.syntax()
        .descendants()
        .filter_map(ast::PrefixExpr::cast)
        .filter(|expr| expr.op_token().is_some_and(|token| token.text() == "*"))
        .map(|expr| {
            let range = expr.syntax().text_range();
            super::spans::SourceRange::checked(
                u32::from(range.start()) - start,
                u32::from(range.end()) - start,
            )
            .unwrap()
        })
        .collect()
}

/// Decode only the unchanged two-let String move witness; no guest call runs.
pub(crate) fn string_local_move(source: &str) -> (String, bool, bool) {
    let main = function(source, "main");
    let statements = main
        .body()
        .unwrap()
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .collect::<Vec<_>>();
    assert_eq!(statements.len(), 2);
    let ast::Pat::IdentPat(first) = statements[0].pat().unwrap() else {
        panic!("original source binding absent")
    };
    let ast::Pat::IdentPat(second) = statements[1].pat().unwrap() else {
        panic!("original destination binding absent")
    };
    let ast::Expr::MethodCallExpr(call) = statements[0].initializer().unwrap() else {
        panic!("original String construction call absent")
    };
    assert_eq!(call.name_ref().unwrap().text(), "to_string");
    let ast::Expr::Literal(literal) = call.receiver().unwrap() else {
        panic!("original String literal absent")
    };
    let ast::LiteralKind::String(text) = literal.kind() else {
        panic!("original literal is not text")
    };
    let ast::Expr::PathExpr(moved) = statements[1].initializer().unwrap() else {
        panic!("original owning local read absent")
    };
    assert_eq!(
        moved.syntax().text().to_string(),
        first.name().unwrap().text()
    );
    assert_ne!(first.name().unwrap().text(), second.name().unwrap().text());
    (
        text.value().unwrap().into_owned(),
        first.mut_token().is_some(),
        second.mut_token().is_some(),
    )
}

pub(crate) fn string_local_move_ranges(source: &str) -> (SourceRange, SourceRange) {
    let main = function(source, "main");
    let block = main.body().unwrap();
    let start = u32::from(block.syntax().text_range().start());
    let statements = block
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .collect::<Vec<_>>();
    assert_eq!(statements.len(), 2);
    let pattern = statements[0].pat().unwrap().syntax().text_range();
    let moved = statements[1].initializer().unwrap().syntax().text_range();
    let range = |r: ra_ap_syntax::TextRange| {
        SourceRange::checked(
            u32::from(r.start()).checked_sub(start).unwrap(),
            u32::from(r.end()).checked_sub(start).unwrap(),
        )
        .unwrap()
    };
    (range(pattern), range(moved))
}

pub(crate) fn scalar_copy_binding(source: &str) -> i64 {
    let selected = function(source, "test");
    let parameter = selected.param_list().unwrap().params().next().unwrap();
    assert_eq!(parameter.ty().unwrap().syntax().text().to_string(), "isize");
    let InitializerFacts::Integer(value) = binding(source, "x").initializer else {
        panic!("original primitive copy initializer absent")
    };
    value
}
pub(crate) fn mutable_parameter_move(source: &str) -> SourceRange {
    let selected = function(source, "foo");
    let parameter = selected.param_list().unwrap().params().next().unwrap();
    assert_eq!(parameter.pat().unwrap().syntax().text().to_string(), "t0");
    assert_eq!(
        parameter.ty().unwrap().syntax().text().to_string(),
        "&mut isize"
    );
    let body = selected.body().unwrap();
    let statement = body
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .find(|statement| {
            statement
                .pat()
                .is_some_and(|pat| pat.syntax().text() == "t1")
        })
        .unwrap();
    let ast::Expr::PathExpr(path) = statement.initializer().unwrap() else {
        panic!("original mutable parameter owning use absent")
    };
    assert_eq!(path.syntax().text().to_string(), "t0");
    let start = u32::from(body.syntax().text_range().start());
    let range = path.syntax().text_range();
    SourceRange::checked(
        u32::from(range.start()).checked_sub(start).unwrap(),
        u32::from(range.end()).checked_sub(start).unwrap(),
    )
    .unwrap()
}

/// Exact source header and selector for the stock tuple/Drop regression.
pub(crate) fn tuple_reference_move_selector(source: &str) -> u32 {
    let function = function(source, "consume");
    let parameter = function.param_list().unwrap().params().next().unwrap();
    let ast::Type::TupleType(tuple) = parameter.ty().unwrap() else {
        panic!("original tuple parameter absent")
    };
    let mut fields = tuple.fields();
    let ast::Type::RefType(reference) = fields.next().unwrap() else {
        panic!("original mutable reference field absent")
    };
    assert!(reference.mut_token().is_some());
    assert_eq!(reference.ty().unwrap().syntax().text().to_string(), "()");
    assert_eq!(
        fields.next().unwrap().syntax().text().to_string(),
        "WithDrop"
    );
    assert!(fields.next().is_none());
    let ast::Expr::FieldExpr(field) = function.body().unwrap().tail_expr().unwrap() else {
        panic!("original tuple field tail absent")
    };
    assert_eq!(field.expr().unwrap().syntax().text().to_string(), "x");
    field.name_ref().unwrap().text().parse().unwrap()
}

pub(crate) fn tuple_reference_move_range(source: &str) -> SourceRange {
    let function = function(source, "consume");
    let body = function.body().unwrap();
    let range = body.tail_expr().unwrap().syntax().text_range();
    let start = u32::from(body.syntax().text_range().start());
    SourceRange {
        start: u32::from(range.start()).checked_sub(start).unwrap(),
        end: u32::from(range.end()).checked_sub(start).unwrap(),
    }
}

/// Facts from original projected assignments, without substituting source text.
pub(crate) struct ProjectedAssignmentFacts {
    pub assignment: SourceRange,
    pub field: u32,
    pub lhs_binding: String,
    pub initial_text: Option<String>,
    pub incoming_text: Option<String>,
    pub incoming_binding: Option<String>,
}
pub(crate) fn projected_assignment(
    source: &str,
    function_name: &str,
    ordinal: usize,
) -> ProjectedAssignmentFacts {
    let body = function(source, function_name).body().unwrap();
    let start = u32::from(body.syntax().text_range().start());
    let assignment = body
        .syntax()
        .descendants()
        .filter_map(ast::BinExpr::cast)
        .filter(|expression| expression.op_token().is_some_and(|op| op.text() == "="))
        .nth(ordinal)
        .unwrap();
    let ast::Expr::FieldExpr(field) = assignment.lhs().unwrap() else {
        panic!("original assignment field absent")
    };
    let ast::Expr::PathExpr(base) = field.expr().unwrap() else {
        panic!("original projected assignment binding absent")
    };
    let ordinal = field
        .name_ref()
        .unwrap()
        .text()
        .to_string()
        .parse()
        .unwrap();
    let (incoming_text, incoming_binding) = match assignment.rhs().unwrap() {
        ast::Expr::PathExpr(path) => (None, Some(path.syntax().text().to_string())),
        ast::Expr::CallExpr(call) => {
            assert_eq!(call.expr().unwrap().syntax().text(), "String::from");
            let mut args = call.arg_list().unwrap().args();
            let ast::Expr::Literal(literal) = args.next().unwrap() else {
                panic!("original String argument absent")
            };
            assert!(args.next().is_none());
            let ast::LiteralKind::String(text) = literal.kind() else {
                panic!("original String literal absent")
            };
            (Some(text.value().unwrap().into_owned()), None)
        }
        _ => panic!("unselected projected RHS"),
    };
    ProjectedAssignmentFacts {
        assignment: SourceRange::checked(
            u32::from(assignment.syntax().text_range().start())
                .checked_sub(start)
                .unwrap(),
            u32::from(assignment.syntax().text_range().end())
                .checked_sub(start)
                .unwrap(),
        )
        .unwrap(),
        field: ordinal,
        lhs_binding: base.syntax().text().to_string(),
        initial_text: body
            .syntax()
            .descendants()
            .filter_map(ast::Literal::cast)
            .find_map(|literal| match literal.kind() {
                ast::LiteralKind::String(text) => Some(text.value().unwrap().into_owned()),
                _ => None,
            }),
        incoming_text,
        incoming_binding,
    }
}
