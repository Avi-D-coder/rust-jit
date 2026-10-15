//! Structural edit receipts, not namespace or guest execution acceptance.
use lrr_expand_adapter::{
    declaration_output::DeclarationGrammar,
    syntax_attachment::{FragmentSlot, ListSlot, SyntaxAttachment},
    token_wire::*,
};
fn tokens(text: &str, unit: u8) -> TokenForest {
    parse_source(
        text,
        SourceIdentity {
            unit: Key([unit; 32]),
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: format!("edit-{unit}.rs"),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([unit; 32]),
            edition: Edition::E2024,
        },
        Key([unit + 1; 32]),
    )
    .unwrap()
}
#[test]
fn expression_receipt_maps_surrounding_anchors_and_consumes_original_nodes() {
    let mut syntax = SyntaxAttachment::parse(
        &tokens("fn before(){} fn live(){selected!();} fn after(){}", 1),
        DeclarationGrammar::Items,
    )
    .unwrap();
    let before = syntax.materialize().unwrap();
    let call = &before.calls[0];
    let edit = syntax
        .attach_with_edit(
            call.start,
            call.end,
            &tokens("1_i32+2_i32", 3),
            FragmentSlot::Expression,
        )
        .unwrap();
    let after = syntax.materialize().unwrap();
    assert_eq!(edit.removed(), (call.start, call.end));
    assert_eq!(edit.map_original_range(call.start, call.end).unwrap(), None);
    assert_eq!(
        edit.map_original_range(0, call.start).unwrap(),
        Some((0, call.start))
    );
    let mapped = edit
        .map_original_range(call.end, before.text.len() as u32)
        .unwrap()
        .unwrap();
    assert_eq!(
        &before.text[call.end as usize..],
        &after.text[mapped.0 as usize..mapped.1 as usize]
    );
    assert_eq!(
        edit.map_original_range(0, before.text.len() as u32)
            .unwrap(),
        Some((0, after.text.len() as u32))
    );
    assert!(
        edit.map_original_range(call.start - 1, call.start + 1)
            .is_err()
    );
    assert!(edit.map_original_range(call.start, call.start).is_err());
    assert!(edit.map_original_range(0, u32::MAX).is_err());
}
#[test]
fn statement_receipt_covers_the_actual_caller_semicolon_and_empty_output() {
    for output in ["1_i32", ""] {
        let mut syntax = SyntaxAttachment::parse(
            &tokens("fn live(){selected!();}", 1),
            DeclarationGrammar::Items,
        )
        .unwrap();
        let before = syntax.materialize().unwrap();
        let call = &before.calls[0];
        let edit = syntax
            .attach_list_with_edit(
                call.start,
                call.end,
                &tokens(output, 3),
                ListSlot::Statements,
            )
            .unwrap();
        let after = syntax.materialize().unwrap();
        let removed = &before.text[edit.removed().0 as usize..edit.removed().1 as usize];
        assert_eq!(
            removed
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>(),
            "selected!();"
        );
        let inserted = &after.text[edit.inserted().0 as usize..edit.inserted().1 as usize];
        if output.is_empty() {
            assert!(inserted.is_empty());
        } else {
            assert!(inserted.ends_with(';'));
        }
        assert_eq!(
            edit.map_original_range(0, before.text.len() as u32)
                .unwrap(),
            Some((0, after.text.len() as u32))
        );
    }
}
#[test]
fn rejected_attachment_preserves_tree_and_a_later_valid_receipt() {
    let mut syntax = SyntaxAttachment::parse(
        &tokens("fn live(){selected!();}", 1),
        DeclarationGrammar::Items,
    )
    .unwrap();
    let identity = syntax.syntax_identity().unwrap();
    let before = syntax.materialize().unwrap();
    let call = &before.calls[0];
    assert!(
        syntax
            .attach_with_edit(
                call.start,
                call.end,
                &tokens("let x=1;", 3),
                FragmentSlot::Expression
            )
            .is_err()
    );
    assert_eq!(syntax.syntax_identity().unwrap(), identity);
    let edit = syntax
        .attach_with_edit(
            call.start,
            call.end,
            &tokens("1_i32", 3),
            FragmentSlot::Expression,
        )
        .unwrap();
    assert_eq!(edit.removed(), (call.start, call.end));
}
