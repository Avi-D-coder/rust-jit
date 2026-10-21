//! Unchanged rustc witnesses drive structural transport. No guest execution or
//! diagnostic/full-fixture acceptance is claimed by these adapter controls.
use lrr_expand_adapter::{
    declaration_output::{CallCapture, DeclarationGrammar, DeclarationSyntax},
    declarative::DeclarativeMacro,
    item_output::SyntaxEvent,
    syntax_attachment::{FragmentSlot, ListSlot, SyntaxAttachment},
    token_wire::*,
};
use syntax::{
    AstNode, SyntaxNode, SyntaxTreeBuilder,
    ast::{self, HasName},
};

const EDITIONS: [Edition; 4] = [
    Edition::E2015,
    Edition::E2018,
    Edition::E2021,
    Edition::E2024,
];
fn input(text: &str, edition: Edition) -> TokenForest {
    parse_source(
        text,
        SourceIdentity {
            unit: Key([91; 32]),
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: "upstream-list.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([91; 32]),
            edition,
        },
        Key([92; 32]),
    )
    .unwrap()
}
fn tree(output: &DeclarationSyntax) -> SyntaxNode {
    let mut builder = SyntaxTreeBuilder::default();
    for event in &output.syntax {
        match event {
            SyntaxEvent::Start(kind) => builder.start_node(syntax::SyntaxKind::from(*kind)),
            SyntaxEvent::Token { kind, text } => {
                builder.token(syntax::SyntaxKind::from(*kind), text)
            }
            SyntaxEvent::Finish => builder.finish_node(),
        }
    }
    builder.finish().syntax_node()
}
fn calls_named(source: &SyntaxAttachment, name: &str) -> Vec<CallCapture> {
    source
        .materialize()
        .unwrap()
        .calls
        .into_iter()
        .filter(|call| {
            let path = lrr_expand_adapter::declaration_output::materialize(
                &call.path,
                DeclarationGrammar::Expression,
            )
            .unwrap();
            path.text.trim() == name
        })
        .collect()
}
// Component-only selection of an actual captured rule definition. Namespace,
// visibility and runtime admission belong to the production source resolver.
fn expand(source: &SyntaxAttachment, name: &str, call: &CallCapture) -> TokenForest {
    let captured = source.materialize().unwrap();
    let root = tree(&captured);
    let declaration = root
        .descendants()
        .find_map(|node| {
            ast::MacroRules::cast(node).filter(|d| d.name().is_some_and(|n| n.text() == name))
        })
        .unwrap();
    let range = declaration.syntax().text_range();
    let definition = captured
        .definitions
        .iter()
        .find(|d| d.start == u32::from(range.start()) && d.end == u32::from(range.end()))
        .unwrap();
    DeclarativeMacro::new(definition.body.clone())
        .unwrap()
        .expand(&call.input, call.site)
        .unwrap()
        .unwrap()
        .tokens
}
fn expand_list(source: &mut SyntaxAttachment, name: &str, slot: ListSlot) {
    let call = calls_named(source, name).remove(0);
    let output = expand(source, name, &call);
    source
        .attach_list(call.start, call.end, &output, slot)
        .unwrap();
}

#[test]
fn upstream_nested_statement_expansions_join_the_same_caller_scope() {
    let original = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    );
    for edition in EDITIONS {
        let mut source =
            SyntaxAttachment::parse(&input(original, edition), DeclarationGrammar::Items).unwrap();
        expand_list(&mut source, "grault", ListSlot::Statements);
        expand_list(&mut source, "foo", ListSlot::Statements);
        let output = source.materialize().unwrap();
        assert!(output.calls.is_empty());
        let root = tree(&output);
        let main = root
            .descendants()
            .filter_map(ast::Fn::cast)
            .find(|f| f.name().is_some_and(|n| n.text() == "main"))
            .unwrap();
        let statements = main.body().unwrap().stmt_list().unwrap();
        let names = statements
            .syntax()
            .children()
            .filter_map(ast::Struct::cast)
            .map(|s| s.name().unwrap().text().to_string())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Bar", "Baz", "Xyzzy"]);
        assert_eq!(
            root.descendants()
                .filter(|n| ast::BlockExpr::can_cast(n.kind()))
                .count(),
            2
        );
        assert_eq!(
            statements
                .syntax()
                .descendants()
                .filter(|n| ast::CallExpr::can_cast(n.kind()))
                .count(),
            3
        );
        assert_eq!(
            output.origins.last().unwrap().end as usize,
            output.text.len()
        );
    }
}

#[test]
fn upstream_function_bodies_attach_in_item_trait_and_impl_lists() {
    let original = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-as-fn-body.rs"
    );
    for edition in EDITIONS {
        let mut source =
            SyntaxAttachment::parse(&input(original, edition), DeclarationGrammar::Items).unwrap();
        for slot in [ListSlot::TraitItems, ListSlot::ImplItems, ListSlot::Items] {
            expand_list(&mut source, "def_fn", slot);
        }
        assert!(calls_named(&source, "def_fn").is_empty());
        // println bodies remain real unresolved macro calls, never evaluated
        // by a transport test or replaced with a manufactured successful body.
        assert_eq!(calls_named(&source, "println").len(), 3);
        let root = tree(&source.materialize().unwrap());
        let functions = root
            .descendants()
            .filter_map(ast::Fn::cast)
            .filter(|f| f.name().is_some_and(|n| n.text() == "bar"))
            .collect::<Vec<_>>();
        assert_eq!(functions.len(), 3);
        assert!(functions.iter().all(|f| f.body().is_some()));
    }
}

#[test]
fn upstream_statement_macros_retain_local_bindings_and_generated_functions() {
    let original =
        include_str!("../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-stmt.rs");
    for edition in EDITIONS {
        let mut source =
            SyntaxAttachment::parse(&input(original, edition), DeclarationGrammar::Items).unwrap();
        expand_list(&mut source, "myfn", ListSlot::Items);
        expand_list(&mut source, "mylet", ListSlot::Statements);
        expand_list(&mut source, "myfn", ListSlot::Statements);
        let root = tree(&source.materialize().unwrap());
        let names = root
            .descendants()
            .filter_map(ast::Fn::cast)
            .map(|f| f.name().unwrap().text().to_string())
            .collect::<Vec<_>>();
        assert_eq!(names, ["add", "main", "mult"]);
        let y = root.descendants().find_map(ast::LetStmt::cast).unwrap();
        // The pinned transcriber intentionally groups a non-atomic $expr
        // (mbe/expander/transcriber.rs, Fragment::Expr). Preserve that syntax.
        let grouped = ast::ParenExpr::cast(y.initializer().unwrap().syntax().clone()).unwrap();
        let product = ast::BinExpr::cast(grouped.expr().unwrap().syntax().clone()).unwrap();
        assert_eq!(product.lhs().unwrap().syntax().text().to_string(), "8");
        assert_eq!(product.rhs().unwrap().syntax().text().to_string(), "2");
        assert_eq!(product.op_token().unwrap().text(), "*");
        assert!(
            y.syntax()
                .parent()
                .is_some_and(|n| ast::StmtList::can_cast(n.kind()))
        );
        assert_eq!(calls_named(&source, "assert_eq").len(), 3);
    }
}

#[test]
fn upstream_statement_fragment_forwarding_keeps_the_caller_semicolon() {
    let original =
        include_str!("../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-stmt-2.rs");
    for edition in EDITIONS {
        let mut source =
            SyntaxAttachment::parse(&input(original, edition), DeclarationGrammar::Items).unwrap();
        expand_list(&mut source, "as_stmt", ListSlot::Statements);
        expand_list(&mut source, "declare", ListSlot::Statements);
        let output = source.materialize().unwrap();
        assert!(output.calls.is_empty());
        let root = tree(&output);
        let y = root.descendants().find_map(ast::LetStmt::cast).unwrap();
        assert_eq!(y.pat().unwrap().syntax().text().to_string(), "_x");
        assert!(y.semicolon_token().is_some());
    }
}

#[test]
fn upstream_empty_statement_does_not_promote_the_preceding_expression_to_tail() {
    let original = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/empty-trailing-stmt.rs"
    );
    for edition in EDITIONS {
        let mut source =
            SyntaxAttachment::parse(&input(original, edition), DeclarationGrammar::Items).unwrap();
        expand_list(&mut source, "empty", ListSlot::Statements);
        let root = tree(&source.materialize().unwrap());
        let foo = root
            .descendants()
            .filter_map(ast::Fn::cast)
            .find(|f| f.name().is_some_and(|n| n.text() == "foo"))
            .unwrap();
        let statements = foo.body().unwrap().stmt_list().unwrap();
        assert!(statements.tail_expr().is_none());
        assert_eq!(statements.statements().count(), 1);
    }
}

#[test]
fn upstream_expression_macro_retains_its_real_block_around_inner_statement_items() {
    let original = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-stmt_macro_in_expr_macro.rs"
    );
    for edition in EDITIONS {
        let mut source =
            SyntaxAttachment::parse(&input(original, edition), DeclarationGrammar::Items).unwrap();
        let call = calls_named(&source, "grault").remove(0);
        let output = expand(&source, "grault", &call);
        source
            .attach(call.start, call.end, &output, FragmentSlot::Expression)
            .unwrap();
        expand_list(&mut source, "foo", ListSlot::Statements);
        let root = tree(&source.materialize().unwrap());
        let initializer = root
            .descendants()
            .find_map(ast::LetStmt::cast)
            .unwrap()
            .initializer()
            .unwrap();
        let block = ast::BlockExpr::cast(initializer.syntax().clone()).unwrap();
        let statements = block.stmt_list().unwrap();
        assert_eq!(
            statements
                .syntax()
                .children()
                .filter(|n| ast::Struct::can_cast(n.kind()))
                .count(),
            3
        );
        assert!(ast::Literal::can_cast(
            statements.tail_expr().unwrap().syntax().kind()
        ));
    }
}

#[test]
fn caller_semicolon_converts_the_final_output_expression_into_an_ordinary_statement() {
    for output in [
        "1_i32",
        "let first=0_i32;1_i32",
        "if true{}",
        "",
        "let first=0_i32;",
    ] {
        let caller = "fn main(){selected!();let after=2_i32;}";
        let mut source =
            SyntaxAttachment::parse(&input(caller, Edition::E2024), DeclarationGrammar::Items)
                .unwrap();
        let call = calls_named(&source, "selected").remove(0);
        let before = source.materialize().unwrap();
        let semi_offset = before.text[call.end as usize..].find(';').unwrap() + call.end as usize;
        let caller_origin = before
            .origins
            .iter()
            .find(|o| o.start <= semi_offset as u32 && (semi_offset as u32) < o.end)
            .unwrap()
            .span;
        source
            .attach_list(
                call.start,
                call.end,
                &input(output, Edition::E2015),
                ListSlot::Statements,
            )
            .unwrap();
        let attached = source.materialize().unwrap();
        let root = tree(&attached);
        for stmt in root.descendants().filter_map(ast::ExprStmt::cast) {
            assert!(stmt.semicolon_token().is_some());
            let semi = stmt.semicolon_token().unwrap().text_range();
            let origin = attached
                .origins
                .iter()
                .find(|o| o.start <= u32::from(semi.start()) && u32::from(semi.end()) <= o.end)
                .unwrap();
            assert_eq!(origin.span, caller_origin);
        }
        let main = root.descendants().find_map(ast::Fn::cast).unwrap();
        assert!(
            main.body()
                .unwrap()
                .stmt_list()
                .unwrap()
                .tail_expr()
                .is_none()
        );
    }
}

#[test]
fn wrong_list_slots_or_malformed_outputs_leave_syntax_and_origins_unchanged() {
    for (caller, output, slot) in [
        ("fn main(){selected!();}", "struct S;", ListSlot::Items),
        ("trait T{selected!();}", "fn f(){}", ListSlot::ImplItems),
        ("impl S{selected!();}", "fn f(){}", ListSlot::TraitItems),
        (
            "fn main(){selected!()}",
            "let first=0;",
            ListSlot::Statements,
        ),
        ("selected!();", "1_i32", ListSlot::Items),
        ("trait T{selected!();}", "struct S;", ListSlot::TraitItems),
        (
            "fn main(){selected!();}",
            "let first =;",
            ListSlot::Statements,
        ),
    ] {
        let mut source =
            SyntaxAttachment::parse(&input(caller, Edition::E2024), DeclarationGrammar::Items)
                .unwrap();
        let call = calls_named(&source, "selected").remove(0);
        let before = source.materialize().unwrap();
        let identity = source.syntax_identity().unwrap();
        assert!(
            source
                .attach_list(call.start, call.end, &input(output, Edition::E2024), slot)
                .is_err()
        );
        let after = source.materialize().unwrap();
        assert_eq!(before.text, after.text);
        assert_eq!(before.origins, after.origins);
        assert_eq!(identity, source.syntax_identity().unwrap());
    }
}

// rustc drops builtin attributes on a macro statement once `cfg`/`cfg_attr`
// are evaluated (rustc_expand `take_mac_call`/`check_attributes`). Attachment
// shares one rule for every origin of the statement; it never inherits the
// attributes into the expansion.
#[test]
fn inert_outer_attributes_leave_with_their_statement_macro() {
    for caller in [
        "fn main(){#[allow(unused)] selected!();let after=2_i32;}",
        "fn main(){#[allow(unused)]#[cfg(all())]#[cfg_attr(all(),allow(dead_code))] selected!();let after=2_i32;}",
        "fn main(){#[allow(unused)] selected!{}let after=2_i32;}",
        "fn main(){/// doc\n#[warn(unused)] selected!();let after=2_i32;}",
        "fn main(){#[rustfmt::skip] selected!();let after=2_i32;}",
    ] {
        for edition in EDITIONS {
            let mut source =
                SyntaxAttachment::parse(&input(caller, edition), DeclarationGrammar::Items)
                    .unwrap();
            let call = calls_named(&source, "selected").remove(0);
            source
                .attach_list(
                    call.start,
                    call.end,
                    &input("let first=0_i32;", edition),
                    ListSlot::Statements,
                )
                .unwrap();
            let root = tree(&source.materialize().unwrap());
            assert!(!root.descendants().any(|n| ast::Attr::can_cast(n.kind())));
            let lets = root
                .descendants()
                .filter_map(ast::LetStmt::cast)
                .map(|s| s.pat().unwrap().syntax().text().to_string())
                .collect::<Vec<_>>();
            assert_eq!(lets, ["first", "after"], "{caller}");
        }
    }
}

#[test]
fn non_inert_attribute_on_a_statement_macro_is_never_discarded() {
    for caller in [
        "fn main(){#[unknown_attr] selected!();}",
        "fn main(){#[allow(unused)]#[unknown_attr] selected!();}",
        "fn main(){#[derive(Debug)] selected!();}",
    ] {
        let mut source =
            SyntaxAttachment::parse(&input(caller, Edition::E2024), DeclarationGrammar::Items)
                .unwrap();
        let call = calls_named(&source, "selected").remove(0);
        let before = source.materialize().unwrap();
        let identity = source.syntax_identity().unwrap();
        assert!(
            source
                .attach_list(
                    call.start,
                    call.end,
                    &input("let first=0_i32;", Edition::E2024),
                    ListSlot::Statements,
                )
                .is_err(),
            "{caller}"
        );
        assert_eq!(before.text, source.materialize().unwrap().text);
        assert_eq!(identity, source.syntax_identity().unwrap());
    }
}

#[test]
fn expression_macros_share_the_inert_attribute_rule_of_statement_macros() {
    for edition in EDITIONS {
        let caller = "fn f()->i32{#[allow(unused)]#[cfg(all())] selected!()}";
        let mut source =
            SyntaxAttachment::parse(&input(caller, edition), DeclarationGrammar::Items).unwrap();
        let call = calls_named(&source, "selected").remove(0);
        source
            .attach(
                call.start,
                call.end,
                &input("1_i32", edition),
                FragmentSlot::Expression,
            )
            .unwrap();
        let output = source.materialize().unwrap();
        assert!(output.text.ends_with("{1_i32}") && !output.text.contains("allow"));
        let root = tree(&output);
        assert!(!root.descendants().any(|n| ast::Attr::can_cast(n.kind())));
        assert_eq!(
            output.origins.last().unwrap().end as usize,
            output.text.len()
        );

        let caller = "fn f()->i32{#[unknown_attr] selected!()}";
        let mut source =
            SyntaxAttachment::parse(&input(caller, edition), DeclarationGrammar::Items).unwrap();
        let call = calls_named(&source, "selected").remove(0);
        let identity = source.syntax_identity().unwrap();
        assert!(
            source
                .attach(
                    call.start,
                    call.end,
                    &input("1_i32", edition),
                    FragmentSlot::Expression,
                )
                .is_err()
        );
        assert_eq!(identity, source.syntax_identity().unwrap());
    }
}
