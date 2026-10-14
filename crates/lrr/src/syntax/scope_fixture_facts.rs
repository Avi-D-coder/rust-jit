//! Owned facts from an unchanged rustc drop-order test; test-only adaptation.
use super::{parse::ParsedFile, spans::SourceRange};
use crate::{core::profile::ProfileIdentity, source::snapshot::SourceBlob};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasArgList, HasName, HasVisibility},
};
use std::collections::BTreeSet;

pub(crate) struct ScopeOrderFacts {
    /// Original closure body wrapped as a function only for structural indexing.
    pub indexed_source: String,
    pub primary_bindings: Vec<(String, SourceRange)>,
    pub initializations: Vec<(String, u64)>,
    pub expected_tags: Vec<u64>,
    pub tag_type: crate::ir::op::PrimitiveType,
}

pub(crate) fn original_deferred_binding_order(source: &str) -> ScopeOrderFacts {
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    assert!(parsed.errors.is_empty());
    let log = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Struct::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == "LogDrop"))
        .unwrap();
    let tag = log
        .syntax()
        .descendants()
        .filter_map(ast::TupleField::cast)
        .nth(1)
        .unwrap()
        .ty()
        .unwrap();
    assert!(
        tag.syntax().text() == "u32",
        "written original log tag type differs"
    );
    let tag_type = crate::ir::op::PrimitiveType::U32;
    let main = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().is_some_and(|name| name.text() == "main"))
        .unwrap();
    let call = main
        .body()
        .unwrap()
        .syntax()
        .descendants()
        .filter_map(ast::CallExpr::cast)
        .find(|call| {
            call.expr()
                .is_some_and(|callee| callee.syntax().text() == "assert_drop_order")
        })
        .unwrap();
    let args: Vec<_> = call.arg_list().unwrap().args().collect();
    assert_eq!(args.len(), 2);
    let range = args[0].syntax().text().to_string();
    let (start, end) = range.split_once("..=").unwrap();
    let (start, end) = (start.parse::<u64>().unwrap(), end.parse::<u64>().unwrap());
    assert!(start <= end && end - start < 32);
    let expected_tags = (start..=end).collect();
    let ast::Expr::ClosureExpr(closure) = &args[1] else {
        panic!("original order oracle closure absent")
    };
    let ast::Expr::BlockExpr(body) = closure.body().unwrap() else {
        panic!("original order oracle block absent")
    };
    let origin = u32::from(body.syntax().text_range().start());
    let declaration = body
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .find(|statement| statement.initializer().is_none())
        .unwrap();
    let pattern = declaration.pat().unwrap();
    let mut names = BTreeSet::new();
    let mut primary_bindings = Vec::new();
    for binding in pattern
        .syntax()
        .descendants()
        .filter_map(ast::IdentPat::cast)
    {
        let name = binding.name().unwrap().text().to_string();
        if names.insert(name.clone()) {
            let range = binding.syntax().text_range();
            primary_bindings.push((
                name,
                SourceRange::checked(
                    u32::from(range.start()).checked_sub(origin).unwrap(),
                    u32::from(range.end()).checked_sub(origin).unwrap(),
                )
                .unwrap(),
            ));
        }
    }
    let initializations = body
        .syntax()
        .descendants()
        .filter_map(ast::BinExpr::cast)
        .filter(|expression| {
            expression
                .op_token()
                .is_some_and(|token| token.text() == "=")
        })
        .map(|assignment| {
            let name = assignment.lhs().unwrap().syntax().text().to_string();
            assert!(names.contains(&name));
            let Some(ast::Expr::CallExpr(value)) = assignment.rhs() else {
                panic!("original LogDrop initializer absent")
            };
            assert!(value.expr().unwrap().syntax().text() == "LogDrop");
            let tag = value
                .arg_list()
                .unwrap()
                .args()
                .nth(1)
                .unwrap()
                .syntax()
                .text()
                .to_string()
                .parse()
                .unwrap();
            (name, tag)
        })
        .collect::<Vec<_>>();
    assert_eq!(initializations.len(), primary_bindings.len());
    ScopeOrderFacts {
        indexed_source: format!("fn lrr_scope_order_fixture() {}", body.syntax().text()),
        primary_bindings,
        initializations,
        expected_tags,
        tag_type,
    }
}

/// The first complete let initializer in an unchanged stock function.
pub(crate) struct TemporaryBorrowFacts {
    pub statement: SourceRange,
    pub binding: SourceRange,
    pub borrow: SourceRange,
    pub operand: SourceRange,
}
pub(crate) fn original_temporary_borrow(source: &str, function: &str) -> TemporaryBorrowFacts {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let function = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == function))
        .unwrap();
    let body = function.body().unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    let statement = body
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .next()
        .unwrap();
    let pattern = statement.pat().unwrap();
    let ast::Expr::RefExpr(borrow) = statement.initializer().unwrap() else {
        panic!("original temporary borrow initializer absent")
    };
    assert!(borrow.raw_token().is_none());
    let operand = borrow.expr().unwrap();
    let ast::Expr::Literal(_) = &operand else {
        panic!("original computed operand is not a literal")
    };
    let relative = |syntax: &ra_ap_syntax::SyntaxNode| {
        let range = syntax.text_range();
        SourceRange::checked(
            u32::from(range.start()).checked_sub(origin).unwrap(),
            u32::from(range.end()).checked_sub(origin).unwrap(),
        )
        .unwrap()
    };
    TemporaryBorrowFacts {
        statement: relative(statement.syntax()),
        binding: relative(pattern.syntax()),
        borrow: relative(borrow.syntax()),
        operand: relative(operand.syntax()),
    }
}

/// Exact source ranges for a stock let whose first argument is a borrow of a call.
pub(crate) struct CallScopeFacts {
    pub outer: SourceRange,
    pub borrow: SourceRange,
    pub operand: SourceRange,
    pub outer_name: String,
    pub operand_name: String,
}
pub(crate) fn original_call_scope(source: &str, binding: &str) -> CallScopeFacts {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let main = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == "main"))
        .unwrap();
    let body = main.body().unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    let statement = body
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .find(|statement| {
            statement
                .pat()
                .unwrap()
                .syntax()
                .descendants()
                .filter_map(ast::IdentPat::cast)
                .any(|pat| pat.name().is_some_and(|name| name.text() == binding))
        })
        .unwrap();
    let ast::Expr::CallExpr(outer) = statement.initializer().unwrap() else {
        panic!("stock ordinary/constructor frontier absent")
    };
    let ast::Expr::RefExpr(borrow) = outer.arg_list().unwrap().args().next().unwrap() else {
        panic!("stock borrowed argument absent")
    };
    let ast::Expr::CallExpr(operand) = borrow.expr().unwrap() else {
        panic!("stock computed borrowed call absent")
    };
    let relative = |node: &ra_ap_syntax::SyntaxNode| {
        let range = node.text_range();
        SourceRange::checked(
            u32::from(range.start()).checked_sub(origin).unwrap(),
            u32::from(range.end()).checked_sub(origin).unwrap(),
        )
        .unwrap()
    };
    CallScopeFacts {
        outer: relative(outer.syntax()),
        borrow: relative(borrow.syntax()),
        operand: relative(operand.syntax()),
        outer_name: outer.expr().unwrap().syntax().text().to_string(),
        operand_name: operand.expr().unwrap().syntax().text().to_string(),
    }
}

/// Test-only declared SDK facet. The declaration bytes come from a copied stock
/// minicore-style fixture; compiler lang attributes and surrounding unsafe test
/// code are outside the selected range. No Drop body or trait witness is supplied.
pub(crate) fn original_drop_declaration_facet(source: &str) -> (String, SourceRange) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let declaration = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Trait::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == "Drop"))
        .unwrap();
    let visibility = declaration.visibility().unwrap();
    let range = SourceRange::checked(
        u32::from(visibility.syntax().text_range().start()),
        u32::from(declaration.syntax().text_range().end()),
    )
    .unwrap();
    let original = source
        .get(range.start as usize..range.end as usize)
        .unwrap();
    (format!("pub mod ops {{ {original} }}"), range)
}

/// Exact initialized reference bindings from a stock borrow-checker error case.
/// The error-producing remainder is deliberately outside these component controls.
pub(crate) struct ReferencePatternFacts {
    pub statement: SourceRange,
    pub binding: SourceRange,
    pub initializer: SourceRange,
    pub mutable: bool,
    pub owner_binding: Option<SourceRange>,
    pub owner_statement: Option<SourceRange>,
}
pub(crate) fn original_reference_patterns(source: &str) -> Vec<ReferencePatternFacts> {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let function = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == "main"))
        .unwrap();
    let body = function.body().unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    let relative = |syntax: &ra_ap_syntax::SyntaxNode| {
        let range = syntax.text_range();
        SourceRange::checked(
            u32::from(range.start()).checked_sub(origin).unwrap(),
            u32::from(range.end()).checked_sub(origin).unwrap(),
        )
        .unwrap()
    };
    body.syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .filter_map(|statement| {
            let ast::Pat::IdentPat(pattern) = statement.pat()? else {
                return None;
            };
            pattern.ref_token()?;
            assert!(pattern.at_token().is_none());
            let initializer = statement.initializer().unwrap();
            let owner = match &initializer {
                ast::Expr::Literal(_) => None,
                ast::Expr::PathExpr(path) => {
                    let name = path.syntax().text().to_string();
                    Some(
                        body.syntax()
                            .descendants()
                            .filter_map(ast::LetStmt::cast)
                            .find(|previous| {
                                previous.syntax().text_range().end()
                                    <= statement.syntax().text_range().start()
                                    && matches!(previous.pat(), Some(ast::Pat::IdentPat(binding))
                                    if binding.name().is_some_and(|binding| binding.text() == name))
                            })
                            .unwrap(),
                    )
                }
                _ => panic!("original ref initializer requires another fixture adaptation"),
            };
            Some(ReferencePatternFacts {
                statement: relative(statement.syntax()),
                binding: relative(pattern.syntax()),
                initializer: relative(initializer.syntax()),
                mutable: pattern.mut_token().is_some(),
                owner_binding: owner
                    .as_ref()
                    .map(|statement| relative(statement.pat().unwrap().syntax())),
                owner_statement: owner.as_ref().map(|statement| relative(statement.syntax())),
            })
        })
        .collect()
}

/// Original first three statements prove a cast creates a distinct temporary.
/// Pointer comparison and nominal Drop in the remainder remain unexecuted.
pub(crate) struct CastBorrowFacts {
    pub statement: SourceRange,
    pub original_binding: SourceRange,
    pub borrowed_binding: SourceRange,
    pub temporary: SourceRange,
    pub stop: SourceRange,
    pub original: i64,
    pub assigned: i64,
}
pub(crate) fn original_cast_borrow(source: &str) -> CastBorrowFacts {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let main = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == "main"))
        .unwrap();
    let body = main.body().unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    let statements: Vec<_> = body.stmt_list().unwrap().statements().collect();
    let ast::Stmt::LetStmt(first) = &statements[0] else {
        panic!("original owner let absent")
    };
    let ast::Stmt::LetStmt(second) = &statements[1] else {
        panic!("original cast borrow let absent")
    };
    let ast::Stmt::ExprStmt(third) = &statements[2] else {
        panic!("original assignment absent")
    };
    let cast = second
        .initializer()
        .unwrap()
        .syntax()
        .descendants()
        .find_map(ast::CastExpr::cast)
        .unwrap();
    assert!(cast.ty().unwrap().syntax().text() == "i32");
    let ast::Expr::Literal(initial) = first.initializer().unwrap() else {
        panic!("original scalar initializer absent")
    };
    let ast::Expr::BinExpr(assignment) = third.expr().unwrap() else {
        panic!("original assignment absent")
    };
    assert!(
        assignment
            .op_token()
            .is_some_and(|token| token.text() == "=")
    );
    let ast::Expr::Literal(assigned) = assignment.rhs().unwrap() else {
        panic!("original assigned scalar absent")
    };
    let relative = |syntax: &ra_ap_syntax::SyntaxNode| {
        let r = syntax.text_range();
        SourceRange::checked(
            u32::from(r.start()).checked_sub(origin).unwrap(),
            u32::from(r.end()).checked_sub(origin).unwrap(),
        )
        .unwrap()
    };
    CastBorrowFacts {
        statement: relative(first.syntax()),
        original_binding: relative(first.pat().unwrap().syntax()),
        borrowed_binding: relative(second.pat().unwrap().syntax()),
        temporary: relative(
            second
                .initializer()
                .unwrap()
                .syntax()
                .children()
                .find_map(ast::Expr::cast)
                .unwrap()
                .syntax(),
        ),
        stop: relative(statements[3].syntax()),
        original: initial
            .syntax()
            .text()
            .to_string()
            .strip_suffix("i32")
            .unwrap()
            .parse()
            .unwrap(),
        assigned: assigned.syntax().text().to_string().parse().unwrap(),
    }
}

/// LRR guard adaptation: put the original cast under a hoisted stock alias.
/// Both original files remain unchanged; this is a source-identity refusal,
/// not execution or acceptance of the entire shadow-primitives program.
pub(crate) struct GuardedCastFacts {
    pub source: String,
    pub cast: SourceRange,
}
pub(crate) fn original_cast_with_stock_shadow(casts: &str, shadows: &str) -> GuardedCastFacts {
    original_cast_with_stock_declaration(casts, shadows, false)
}
pub(crate) fn original_cast_with_stock_value_function(
    casts: &str,
    shadows: &str,
) -> GuardedCastFacts {
    original_cast_with_stock_declaration(casts, shadows, true)
}
fn original_cast_with_stock_declaration(
    casts: &str,
    shadows: &str,
    value_function: bool,
) -> GuardedCastFacts {
    let parse = |text: &str| {
        ParsedFile::parse(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap()
    };
    let casts = parse(casts);
    let cast = casts
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
        .filter(|call| {
            call.path()
                .is_some_and(|path| path.syntax().text() == "assert_eq")
        })
        .find_map(|call| {
            // The original u8 casts are inside assert_eq token trees, not the
            // outer AST. Reparse the unchanged argument tokens in a disclosed
            // test-only call wrapper; this never executes the assertion.
            let tokens = call.token_tree()?.syntax().text().to_string();
            let inside = tokens.strip_prefix('(')?.strip_suffix(')')?;
            let arguments = parse(&format!(
                "fn lrr_stock_arguments() {{ lrr_capture({inside}); }}"
            ));
            arguments
                .parsed
                .tree()
                .syntax()
                .descendants()
                .filter_map(ast::CastExpr::cast)
                .find(|cast| cast.ty().is_some_and(|ty| ty.syntax().text() == "u8"))
        })
        .unwrap();
    let shadows = parse(shadows);
    let declaration = if value_function {
        shadows
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::Fn::cast)
            .find(|function| function.name().is_some_and(|name| name.text() == "u8"))
            .unwrap()
            .syntax()
            .clone()
    } else {
        shadows
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::TypeAlias::cast)
            .find(|alias| alias.name().is_some_and(|name| name.text() == "u8"))
            .unwrap()
            .syntax()
            .clone()
    };
    // Written after the cast to exercise the type declaration's hoisted scope.
    let source = format!(
        "fn lrr_cast_guard() {{ let guard = {}; {} }}",
        cast.syntax().text(),
        declaration.text()
    );
    let indexed = parse(&source);
    let function = indexed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::Fn::cast)
        .unwrap();
    let body = function.body().unwrap();
    let cast = body
        .syntax()
        .descendants()
        .find_map(ast::CastExpr::cast)
        .unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    GuardedCastFacts {
        source,
        cast: SourceRange::checked(
            u32::from(cast.syntax().text_range().start())
                .checked_sub(origin)
                .unwrap(),
            u32::from(cast.syntax().text_range().end())
                .checked_sub(origin)
                .unwrap(),
        )
        .unwrap(),
    }
}

pub(crate) struct GuardedLetFacts {
    pub source: String,
    pub statement: SourceRange,
}
/// LRR namespace adaptation: constrain the original boolean initializer with
/// the original trait's name and put its original declaration in a local rib.
/// Original source bytes are retained unchanged; this is not a stock run pass.
pub(crate) fn original_let_with_stock_trait(shadows: &str) -> GuardedLetFacts {
    let parse = |text: &str| {
        ParsedFile::parse(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap()
    };
    let parsed = parse(shadows);
    let root = parsed.parsed.tree();
    let declaration = root
        .syntax()
        .descendants()
        .filter_map(ast::Trait::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == "bool"))
        .unwrap();
    let statement = root
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .find(|item| {
            item.pat()
                .is_some_and(|pattern| pattern.syntax().text() == "bool")
        })
        .unwrap();
    let source = format!(
        "fn lrr_let_trait_guard() {{ let {}: {} = {}; {} }}",
        statement.pat().unwrap().syntax().text(),
        declaration.name().unwrap().text(),
        statement.initializer().unwrap().syntax().text(),
        declaration.syntax().text()
    );
    let adapted = parse(&source);
    assert!(adapted.errors.is_empty());
    let function = adapted
        .parsed
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::Fn::cast)
        .unwrap();
    let body = function.body().unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    let statement = body
        .syntax()
        .descendants()
        .find_map(ast::LetStmt::cast)
        .unwrap();
    let range = statement.syntax().text_range();
    GuardedLetFacts {
        source,
        statement: SourceRange::checked(
            u32::from(range.start()).checked_sub(origin).unwrap(),
            u32::from(range.end()).checked_sub(origin).unwrap(),
        )
        .unwrap(),
    }
}

pub(crate) struct ReferenceContextFacts {
    pub statement: SourceRange,
    pub binding: SourceRange,
    pub borrow: SourceRange,
    pub operand: SourceRange,
    pub mutable: bool,
    pub value: u64,
}
/// Exact explicitly typed scalar borrow lets. Array, pointer, trait and closure
/// cases from the same source are retained but outside these component controls.
pub(crate) fn original_reference_contexts(source: &str) -> Vec<ReferenceContextFacts> {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let main = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == "main"))
        .unwrap();
    let body = main.body().unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    let relative = |syntax: &ra_ap_syntax::SyntaxNode| {
        let r = syntax.text_range();
        SourceRange::checked(
            u32::from(r.start()).checked_sub(origin).unwrap(),
            u32::from(r.end()).checked_sub(origin).unwrap(),
        )
        .unwrap()
    };
    body.syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .filter_map(|statement| {
            let ast::Type::RefType(reference) = statement.ty()? else {
                return None;
            };
            if reference.ty()?.syntax().text() != "u32" {
                return None;
            }
            let ast::Expr::RefExpr(borrow) = statement.initializer()? else {
                return None;
            };
            let operand = borrow.expr()?;
            if !matches!(operand, ast::Expr::Literal(_)) {
                return None;
            }
            assert!(borrow.raw_token().is_none());
            assert_eq!(
                reference.mut_token().is_some(),
                borrow.mut_token().is_some()
            );
            Some(ReferenceContextFacts {
                statement: relative(statement.syntax()),
                binding: relative(statement.pat()?.syntax()),
                borrow: relative(borrow.syntax()),
                operand: relative(operand.syntax()),
                mutable: borrow.mut_token().is_some(),
                value: operand.syntax().text().to_string().parse().unwrap(),
            })
        })
        .collect()
}

/// Owned declaration order from the unchanged stock NoDropImpl record. Runtime
/// cleanup tests adapt these fields to intrinsic String storage; no Rc/Cell,
/// source-selected nominal Drop or original output execution is claimed.
pub(crate) fn original_no_drop_record_members(source: &str) -> Vec<String> {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let item = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Struct::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == "NoDropImpl"))
        .unwrap();
    let ast::FieldList::RecordFieldList(fields) = item.field_list().unwrap() else {
        panic!("stock NoDropImpl record absent")
    };
    fields
        .fields()
        .map(|field| field.name().unwrap().text().to_string())
        .collect()
}

/// Exact first explicit drop statement and its argument in the original main.
pub(crate) fn original_record_drop_statement(source: &str) -> (SourceRange, SourceRange) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let function = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().is_some_and(|name| name.text() == "main"))
        .unwrap();
    let body = function.body().unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    let statement = body
        .syntax()
        .descendants()
        .filter_map(ast::ExprStmt::cast)
        .find(|statement| {
            statement
                .expr()
                .is_some_and(|expression| expression.syntax().text() == "drop(noImpl)")
        })
        .unwrap();
    let Some(ast::Expr::CallExpr(call)) = statement.expr() else {
        panic!("stock explicit drop absent")
    };
    let argument = call.arg_list().unwrap().args().next().unwrap();
    let range = |node: &ra_ap_syntax::SyntaxNode| {
        let range = node.text_range();
        SourceRange::checked(
            u32::from(range.start()).checked_sub(origin).unwrap(),
            u32::from(range.end()).checked_sub(origin).unwrap(),
        )
        .unwrap()
    };
    (range(statement.syntax()), range(argument.syntax()))
}

/// Original declaration order and the explicit `p.y` move before struct update.
pub(crate) fn original_partial_record_move(source: &str) -> (Vec<String>, usize) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let tree = parsed.parsed.tree();
    let record = tree
        .syntax()
        .descendants()
        .filter_map(ast::Struct::cast)
        .find(|record| record.name().is_some_and(|name| name.text() == "Partial"))
        .unwrap();
    let ast::FieldList::RecordFieldList(fields) = record.field_list().unwrap() else {
        panic!("stock Partial record absent")
    };
    let members: Vec<_> = fields
        .fields()
        .map(|field| field.name().unwrap().text().to_string())
        .collect();
    let function = tree
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().is_some_and(|name| name.text() == "f"))
        .unwrap();
    let moved = function
        .body()
        .unwrap()
        .syntax()
        .descendants()
        .filter_map(ast::FieldExpr::cast)
        .find(|field| field.expr().is_some_and(|base| base.syntax().text() == "p"))
        .unwrap();
    let name = moved.name_ref().unwrap().text().to_string();
    let ordinal = members.iter().position(|member| member == &name).unwrap();
    (members, ordinal)
}

pub(crate) struct NestedReturnFacts {
    pub returned: SourceRange,
    pub prepared_owner: SourceRange,
    pub returned_tag: String,
    pub field_tag: i64,
}
/// Original enclosing Foo declaration and the Bar field completed before the
/// nested return interrupts the following initializer. No nominal admission.
pub(crate) fn original_nested_constructor_fields(source: &str) -> (Vec<String>, u32) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let tree = parsed.parsed.tree();
    let declaration = tree
        .syntax()
        .descendants()
        .filter_map(ast::Struct::cast)
        .find(|record| record.name().is_some_and(|name| name.text() == "Foo"))
        .unwrap();
    let ast::FieldList::RecordFieldList(fields) = declaration.field_list().unwrap() else {
        panic!("stock Foo record fields absent")
    };
    let members: Vec<_> = fields
        .fields()
        .map(|field| field.name().unwrap().text().to_string())
        .collect();
    let function = tree
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().is_some_and(|name| name.text() == "foo"))
        .unwrap();
    let outer = function
        .body()
        .unwrap()
        .syntax()
        .descendants()
        .filter_map(ast::RecordExpr::cast)
        .find(|record| {
            record
                .path()
                .is_some_and(|path| path.syntax().text() == "Foo")
        })
        .unwrap();
    let initializers: Vec<_> = outer.record_expr_field_list().unwrap().fields().collect();
    let completed = initializers
        .iter()
        .position(|field| {
            matches!(field.expr(), Some(ast::Expr::RecordExpr(record))
            if record.path().is_some_and(|path| path.syntax().text() == "Bar"))
        })
        .unwrap();
    let interrupted = initializers
        .iter()
        .position(|field| matches!(field.expr(), Some(ast::Expr::ReturnExpr(_))))
        .unwrap();
    assert!(completed < interrupted);
    assert_eq!(
        completed, 0,
        "fixture has additional completed fields to retain"
    );
    let name = initializers[completed]
        .name_ref()
        .unwrap()
        .text()
        .to_string();
    let ordinal = members.iter().position(|member| *member == name).unwrap();
    (members, u32::try_from(ordinal).unwrap())
}
/// Exact nested `return Err(32)` after the completed `Bar { x: 22 }` field.
pub(crate) fn original_nested_return(source: &str) -> NestedReturnFacts {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let function = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().is_some_and(|name| name.text() == "foo"))
        .unwrap();
    let body = function.body().unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    let returned = body
        .syntax()
        .descendants()
        .filter_map(ast::ReturnExpr::cast)
        .find(|returned| {
            returned
                .expr()
                .is_some_and(|value| value.syntax().text() == "Err(32)")
        })
        .unwrap();
    let Some(ast::Expr::CallExpr(call)) = returned.expr() else {
        panic!("stock Err call absent")
    };
    let returned_tag = call
        .arg_list()
        .unwrap()
        .args()
        .next()
        .unwrap()
        .syntax()
        .text()
        .to_string();
    let record = body
        .syntax()
        .descendants()
        .filter_map(ast::RecordExpr::cast)
        .find(|record| {
            record
                .path()
                .is_some_and(|path| path.syntax().text() == "Bar")
        })
        .unwrap();
    let field = record
        .record_expr_field_list()
        .unwrap()
        .fields()
        .next()
        .unwrap();
    assert!(field.name_ref().is_some_and(|name| name.text() == "x"));
    let field_tag = field
        .expr()
        .unwrap()
        .syntax()
        .text()
        .to_string()
        .parse()
        .unwrap();
    let range = |node: &ra_ap_syntax::SyntaxNode| {
        let range = node.text_range();
        SourceRange::checked(
            u32::from(range.start()).checked_sub(origin).unwrap(),
            u32::from(range.end()).checked_sub(origin).unwrap(),
        )
        .unwrap()
    };
    NestedReturnFacts {
        returned: range(returned.syntax()),
        prepared_owner: range(record.syntax()),
        returned_tag,
        field_tag,
    }
}

pub(crate) struct PartialConstructionOrderFacts {
    pub members: Vec<String>,
    pub scenarios: Vec<PartialConstructionScenario>,
}
pub(crate) struct PartialConstructionScenario {
    pub completed: Vec<(u32, String)>,
    pub expected: Vec<String>,
}
/// Exact written fields and following original assertions from RFC1857's two
/// interrupted TestStruct constructors. No Rc/RefCell/panic/Drop is executed.
pub(crate) fn original_partial_construction_orders(source: &str) -> PartialConstructionOrderFacts {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let tree = parsed.parsed.tree();
    let declaration = tree
        .syntax()
        .descendants()
        .filter_map(ast::Struct::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == "TestStruct"))
        .unwrap();
    let members: Vec<_> = declaration
        .syntax()
        .descendants()
        .filter_map(ast::RecordField::cast)
        .map(|field| field.name().unwrap().text().to_string())
        .collect();
    let function = tree
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| {
            function
                .name()
                .is_some_and(|name| name.text() == "test_drop_struct")
        })
        .unwrap();
    let body = function.body().unwrap();
    let mut scenarios = Vec::new();
    for record in body
        .syntax()
        .descendants()
        .filter_map(ast::RecordExpr::cast)
    {
        if !record
            .path()
            .is_some_and(|path| path.syntax().text() == "TestStruct")
        {
            continue;
        }
        let fields: Vec<_> = record.record_expr_field_list().unwrap().fields().collect();
        let Some(ast::Expr::MacroExpr(interruption)) = fields.last().unwrap().expr() else {
            continue;
        };
        assert!(
            interruption
                .macro_call()
                .unwrap()
                .path()
                .unwrap()
                .syntax()
                .text()
                == "panic"
        );
        let completed: Vec<_> = fields[..fields.len() - 1]
            .iter()
            .map(|field| {
                let name = field.name_ref().unwrap().text().to_string();
                let ordinal =
                    u32::try_from(members.iter().position(|member| *member == name).unwrap())
                        .unwrap();
                let ast::Expr::CallExpr(call) = field.expr().unwrap() else {
                    panic!("stock completed field call absent")
                };
                assert!(call.expr().unwrap().syntax().text() == "PushOnDrop::new");
                let tag = call
                    .arg_list()
                    .unwrap()
                    .args()
                    .next()
                    .unwrap()
                    .syntax()
                    .text()
                    .to_string();
                tag.parse::<u32>().unwrap();
                (ordinal, tag)
            })
            .collect();
        let after = record.syntax().text_range().end();
        let oracle = body
            .syntax()
            .descendants()
            .filter_map(ast::MacroCall::cast)
            .filter(|call| call.syntax().text_range().start() > after)
            .find(|call| {
                call.path()
                    .is_some_and(|path| path.syntax().text() == "assert_eq")
            })
            .unwrap();
        let tokens = oracle.token_tree().unwrap().syntax().text().to_string();
        let start = tokens.find("&[").unwrap() + 2;
        let end = tokens[start..].find(']').unwrap() + start;
        let expected: Vec<_> = tokens[start..end]
            .split(',')
            .map(|tag| {
                let tag = tag.trim().to_owned();
                tag.parse::<u32>().unwrap();
                tag
            })
            .collect();
        assert_eq!(completed.len(), expected.len());
        scenarios.push(PartialConstructionScenario {
            completed,
            expected,
        });
    }
    assert_eq!(
        scenarios.len(),
        2,
        "both original written field orders must be retained"
    );
    assert_ne!(scenarios[0].completed[0].0, scenarios[1].completed[0].0);
    PartialConstructionOrderFacts { members, scenarios }
}

/// Exact original let range relative to the selected function body. Consumers
/// receive owned coordinates; parser nodes stay inside the syntax boundary.
pub(crate) fn original_first_let_body_range(source: &str, name: &str) -> SourceRange {
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    assert!(parsed.errors.is_empty());
    let function = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| {
            function
                .name()
                .is_some_and(|written| written.text() == name)
        })
        .unwrap();
    let body = function.body().unwrap();
    let binding = body
        .syntax()
        .descendants()
        .find_map(ast::LetStmt::cast)
        .unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    let range = binding.syntax().text_range();
    SourceRange::checked(
        u32::from(range.start()).checked_sub(origin).unwrap(),
        u32::from(range.end()).checked_sub(origin).unwrap(),
    )
    .unwrap()
}

/// The unique written scalar literal in a selected unchanged stock function.
pub(crate) fn original_single_return_literal(source: &str, name: &str) -> String {
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    assert!(parsed.errors.is_empty());
    let function = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| {
            function
                .name()
                .is_some_and(|written| written.text() == name)
        })
        .unwrap();
    let literals: Vec<_> = function
        .body()
        .unwrap()
        .syntax()
        .descendants()
        .filter_map(ast::Literal::cast)
        .collect();
    let [literal] = literals.as_slice() else {
        panic!("stock return literal is not unique")
    };
    literal
        .syntax()
        .text()
        .to_string()
        .parse::<i64>()
        .unwrap()
        .to_string()
}

/// The first original all-unit pair statement. Lint errors elsewhere in the
/// upstream file stay cold; only owned relative ranges leave the syntax layer.
pub(crate) fn original_unit_pair_statement(source: &str) -> (SourceRange, SourceRange) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let function = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().is_some_and(|name| name.text() == "main"))
        .unwrap();
    let body = function.body().unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    let statement = body
        .syntax()
        .descendants()
        .filter_map(ast::ExprStmt::cast)
        .find(|statement| {
            let Some(ast::Expr::TupleExpr(pair)) = statement.expr() else {
                return false;
            };
            let fields: Vec<_> = pair.fields().collect();
            fields.len() == 2 && fields.iter().all(|field| {
                matches!(field, ast::Expr::TupleExpr(unit) if unit.fields().next().is_none())
            })
        })
        .unwrap();
    let expression = statement.expr().unwrap();
    let relative = |node: &ra_ap_syntax::SyntaxNode| {
        let range = node.text_range();
        SourceRange::checked(
            u32::from(range.start()).checked_sub(origin).unwrap(),
            u32::from(range.end()).checked_sub(origin).unwrap(),
        )
        .unwrap()
    };
    (relative(statement.syntax()), relative(expression.syntax()))
}

/// Original local array replacement, before and after the intervening partial
/// pattern. Only owned ranges/count leave syntax; allocators/patterns stay cold.
pub(crate) struct LocalArrayReassignment {
    pub binding: SourceRange,
    pub initializer: SourceRange,
    pub assignment: SourceRange,
    pub rhs: SourceRange,
    pub count: u32,
    pub moved_field: u32,
}
pub(crate) fn original_array_reassignment_profile(source: &str) -> ProfileIdentity {
    assert!(source.lines().any(|line| line == "//@ edition:2015..2021"));
    ProfileIdentity {
        edition: 2021,
        ..ProfileIdentity::current()
    }
}
pub(crate) fn original_array_reassignment(source: &str) -> LocalArrayReassignment {
    let parsed = ParsedFile::parse_dependency(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        original_array_reassignment_profile(source),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let function = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|f| {
            f.name()
                .is_some_and(|name| name.text() == "slice_pattern_reassign")
        })
        .unwrap();
    let body = function.body().unwrap();
    let origin = u32::from(body.syntax().text_range().start());
    let declaration = body
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .find(|l| l.pat().is_some_and(|pat| pat.syntax().text() == "mut ar"))
        .unwrap();
    let assignment = body
        .syntax()
        .descendants()
        .filter_map(ast::BinExpr::cast)
        .find(|b| {
            b.op_token().is_some_and(|t| t.text() == "=")
                && b.lhs().is_some_and(|lhs| lhs.syntax().text() == "ar")
        })
        .unwrap();
    let initial = declaration.initializer().unwrap();
    let incoming = assignment.rhs().unwrap();
    let ast::Expr::ArrayExpr(array) = &incoming else {
        panic!("original array RHS absent")
    };
    let count = u32::try_from(array.exprs().count()).unwrap();
    let partial = body
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .find(|statement| {
            statement.syntax().text_range().end() < assignment.syntax().text_range().start()
                && statement
                    .initializer()
                    .is_some_and(|expr| expr.syntax().text() == "ar")
        })
        .unwrap();
    let ast::Pat::SlicePat(pattern) = partial.pat().unwrap() else {
        panic!("original pre-reassignment slice pattern absent")
    };
    assert_eq!(u32::try_from(pattern.pats().count()).unwrap(), count);
    let moved_field = u32::try_from(
        pattern
            .pats()
            .enumerate()
            .find_map(|(index, pat)| {
                let ast::Pat::IdentPat(binding) = pat else {
                    return None;
                };
                binding
                    .name()
                    .filter(|name| name.text() == "_x")
                    .map(|_| index)
            })
            .unwrap(),
    )
    .unwrap();
    let relative = |node: &ra_ap_syntax::SyntaxNode| {
        let range = node.text_range();
        SourceRange::checked(
            u32::from(range.start()).checked_sub(origin).unwrap(),
            u32::from(range.end()).checked_sub(origin).unwrap(),
        )
        .unwrap()
    };
    LocalArrayReassignment {
        binding: relative(declaration.pat().unwrap().syntax()),
        initializer: relative(initial.syntax()),
        assignment: relative(assignment.syntax()),
        rhs: relative(incoming.syntax()),
        count,
        moved_field,
    }
}

/// All assertion invocations and direct calls in the unchanged assignment-unit
/// regression. The source has no loops/revisions/conditional assertion paths.
pub(crate) fn original_assignment_unit_program_counts(source: &str) -> (u64, u64, u64) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let tree = parsed.parsed.tree();
    let assertions = tree
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
        .filter(|call| {
            call.path()
                .is_some_and(|path| path.syntax().text() == "assert_eq")
        })
        .count();
    let main = tree
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().is_some_and(|name| name.text() == "main"))
        .unwrap();
    let calls = main
        .body()
        .unwrap()
        .syntax()
        .descendants()
        .filter_map(ast::CallExpr::cast)
        .count();
    let compounds = tree
        .syntax()
        .descendants()
        .filter_map(ast::BinExpr::cast)
        .filter(|binary| binary.op_token().is_some_and(|token| token.text() == "+="))
        .count();
    assert!(assertions > 0 && calls > 0 && compounds > 0);
    (
        u64::try_from(assertions).unwrap(),
        u64::try_from(calls).unwrap(),
        u64::try_from(compounds).unwrap(),
    )
}
