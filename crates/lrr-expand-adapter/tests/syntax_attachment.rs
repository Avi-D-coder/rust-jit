//! Syntax/hygiene transport controls, not interpreter or full-fixture passes.
use lrr_expand_adapter::{
    declaration_output::{DeclarationGrammar, DeclarationSyntax},
    declarative::DeclarativeMacro,
    item_output::SyntaxEvent,
    syntax_attachment::{FragmentSlot, SyntaxAttachment},
    token_wire::*,
};
use syntax::{AstNode, SyntaxNode, SyntaxTreeBuilder, ast};

fn tokens(text: &str, edition: Edition, unit: u8) -> TokenForest {
    parse_source(
        text,
        SourceIdentity {
            unit: Key([unit; 32]),
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: format!("attachment-{unit}.rs"),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([unit; 32]),
            edition,
        },
        Key([unit + 1; 32]),
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

fn expand_first(source: &SyntaxAttachment) -> TokenForest {
    let captured = source.materialize().unwrap();
    let definition = DeclarativeMacro::new(captured.definitions[0].body.clone()).unwrap();
    let call = &captured.calls[0];
    definition
        .expand(&call.input, call.site)
        .unwrap()
        .unwrap()
        .tokens
}

#[test]
fn expansion_retains_expression_precedence_without_a_synthetic_scope_or_call() {
    for edition in [
        Edition::E2015,
        Edition::E2018,
        Edition::E2021,
        Edition::E2024,
    ] {
        let original = tokens(
            "macro_rules! arbitrary {()=>{1_i32+2_i32}} fn main(){let value=arbitrary!()*3_i32;}",
            edition,
            10,
        );
        let mut source = SyntaxAttachment::parse(&original, DeclarationGrammar::Items).unwrap();
        let call = source.materialize().unwrap().calls.remove(0);
        let output = expand_first(&source);
        source
            .attach(call.start, call.end, &output, FragmentSlot::Expression)
            .unwrap();
        let attached = source.materialize().unwrap();
        assert!(attached.calls.is_empty());
        let root = tree(&attached);
        let binding = root.descendants().find_map(ast::LetStmt::cast).unwrap();
        let outer = ast::BinExpr::cast(binding.initializer().unwrap().syntax().clone()).unwrap();
        let inner = ast::BinExpr::cast(outer.lhs().unwrap().syntax().clone()).unwrap();
        assert_eq!(
            inner.syntax().text().to_string().replace(' ', ""),
            "1_i32+2_i32"
        );
        assert_eq!(
            root.descendants()
                .filter(|n| ast::Fn::can_cast(n.kind()))
                .count(),
            1
        );
        assert!(
            !root
                .descendants()
                .any(|n| ast::CallExpr::can_cast(n.kind()))
        );
        assert_eq!(
            attached.origins.last().unwrap().end as usize,
            attached.text.len()
        );
    }
}

#[test]
fn successive_attachments_preserve_the_previously_attached_tree() {
    let original = tokens(
        "fn main(){let value=first!()*second!();}",
        Edition::E2024,
        20,
    );
    let mut source = SyntaxAttachment::parse(&original, DeclarationGrammar::Items).unwrap();
    for output in ["1_i32+2_i32", "3_i32+4_i32"] {
        let call = source.materialize().unwrap().calls.remove(0);
        source
            .attach(
                call.start,
                call.end,
                &tokens(output, Edition::E2018, 30),
                FragmentSlot::Expression,
            )
            .unwrap();
    }
    let attached = source.materialize().unwrap();
    assert!(attached.calls.is_empty());
    let root = tree(&attached);
    let binding = root.descendants().find_map(ast::LetStmt::cast).unwrap();
    let outer = ast::BinExpr::cast(binding.initializer().unwrap().syntax().clone()).unwrap();
    assert!(ast::BinExpr::can_cast(outer.lhs().unwrap().syntax().kind()));
    assert!(ast::BinExpr::can_cast(outer.rhs().unwrap().syntax().kind()));
}

#[test]
fn pattern_type_and_control_fragments_become_ordinary_nodes() {
    for (caller, output, slot, expected) in [
        (
            "fn main(){let selected!()=value;}",
            "(left,right)",
            FragmentSlot::Pattern,
            syntax::SyntaxKind::TUPLE_PAT,
        ),
        (
            "fn main(){let value:selected!();}",
            "(i32,bool)",
            FragmentSlot::Type,
            syntax::SyntaxKind::TUPLE_TYPE,
        ),
        (
            "fn main(){'caller:loop{selected!();}}",
            "break 'caller 7_i32",
            FragmentSlot::Expression,
            syntax::SyntaxKind::BREAK_EXPR,
        ),
    ] {
        let input = tokens(caller, Edition::E2024, 40);
        let mut source = SyntaxAttachment::parse(&input, DeclarationGrammar::Items).unwrap();
        let call = source.materialize().unwrap().calls.remove(0);
        source
            .attach(
                call.start,
                call.end,
                &tokens(output, Edition::E2015, 50),
                slot,
            )
            .unwrap();
        let attached = source.materialize().unwrap();
        let root = tree(&attached);
        assert!(attached.calls.is_empty());
        assert_eq!(
            root.descendants().filter(|n| n.kind() == expected).count(),
            1
        );
        assert_eq!(
            root.descendants()
                .filter(|n| ast::Fn::can_cast(n.kind()))
                .count(),
            1
        );
    }
}

#[test]
fn failed_attachment_does_not_change_the_caller_or_its_origins() {
    let input = tokens("fn main(){let value=selected!();}", Edition::E2024, 60);
    let mut source = SyntaxAttachment::parse(&input, DeclarationGrammar::Items).unwrap();
    let before = source.materialize().unwrap();
    let before_identity = source.syntax_identity().unwrap();
    let call = &before.calls[0];
    for (start, end, output, slot) in [
        (call.start + 1, call.end, "1_i32", FragmentSlot::Expression),
        (call.start, u32::MAX, "1_i32", FragmentSlot::Expression),
        (call.start, call.end, "(left,right)", FragmentSlot::Pattern),
        (
            call.start,
            call.end,
            "1_i32 2_i32",
            FragmentSlot::Expression,
        ),
        (call.start, call.end, "", FragmentSlot::Expression),
    ] {
        assert!(
            source
                .attach(start, end, &tokens(output, Edition::E2018, 70), slot)
                .is_err()
        );
        let after = source.materialize().unwrap();
        assert_eq!(after.text, before.text);
        assert_eq!(after.origins, before.origins);
        assert_eq!(source.syntax_identity().unwrap(), before_identity);
        assert_eq!(
            format!("{:?}", after.syntax),
            format!("{:?}", before.syntax)
        );
    }
}

#[test]
fn upstream_borrow_diagnostic_witness_attaches_as_an_ordinary_block() {
    // Unchanged licensed original, already pinned in the upstream manifest.
    // This only checks expansion/tree transport. Its static borrow diagnostic
    // and interpreter execution have separate acceptance scopes.
    let original = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/span-covering-argument-1.rs"
    );
    for edition in [
        Edition::E2015,
        Edition::E2018,
        Edition::E2021,
        Edition::E2024,
    ] {
        let input = tokens(original, edition, 80);
        let mut source = SyntaxAttachment::parse(&input, DeclarationGrammar::Items).unwrap();
        let call = source.materialize().unwrap().calls.remove(0);
        let output = expand_first(&source);
        source
            .attach(call.start, call.end, &output, FragmentSlot::Expression)
            .unwrap();
        let attached = source.materialize().unwrap();
        assert!(attached.calls.is_empty());
        let root = tree(&attached);
        let function = root.descendants().find_map(ast::Fn::cast).unwrap();
        let nested = function
            .body()
            .unwrap()
            .syntax()
            .children()
            .flat_map(|n| n.children())
            .find_map(ast::ExprStmt::cast)
            .unwrap();
        assert!(ast::BlockExpr::can_cast(
            nested.expr().unwrap().syntax().kind()
        ));
        assert_eq!(
            root.descendants()
                .filter(|n| ast::Fn::can_cast(n.kind()))
                .count(),
            1
        );
        assert!(root.descendants().any(|n| ast::RefExpr::can_cast(n.kind())));
    }
}
