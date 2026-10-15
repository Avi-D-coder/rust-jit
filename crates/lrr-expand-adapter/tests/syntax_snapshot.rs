//! Source-map closure/immutability, not interpreter or source-authority gates.
use lrr_expand_adapter::{
    declaration_output::DeclarationGrammar,
    declarative::{DeclarativeMacro, DeclarativeSite},
    syntax_attachment::{FragmentSlot, ListSlot, SyntaxAttachment},
    token_wire::*,
};
use std::collections::BTreeSet;
#[path = "support/declarative.rs"]
#[allow(dead_code)]
mod fixture;

fn source(text: &str) -> TokenForest {
    parse_source(
        text,
        SourceIdentity {
            unit: Key([91; 32]),
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: "caller.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([91; 32]),
            edition: Edition::E2024,
        },
        Key([92; 32]),
    )
    .unwrap()
}

#[test]
fn snapshot_closes_actual_mixed_edition_hygiene_and_caller_semicolon_origins() {
    let mut syntax = SyntaxAttachment::parse(
        &source("fn main(){selected!(7_u8);}"),
        DeclarationGrammar::Items,
    )
    .unwrap();
    let call = syntax.materialize().unwrap().calls.remove(0);
    let definition = DeclarativeMacro::new(fixture::definition(
        "($value:expr)=>{let gen=$value;gen}",
        Edition::E2015,
    ))
    .unwrap();
    let (result, _) = definition
        .expand_hygienic_with_limits(
            &call.input,
            call.site,
            DeclarativeSite {
                expansion: Key([93; 32]),
                defining_unit: Key([1; 32]),
                definition_edition: Edition::E2015,
            },
            2_000_000,
            32 * 1024 * 1024,
        )
        .unwrap();
    syntax
        .attach_list(
            call.start,
            call.end,
            &result.unwrap().tokens,
            ListSlot::Statements,
        )
        .unwrap();
    let snapshot = syntax.snapshot().unwrap();
    let records = snapshot.origin_records();
    records.validate().unwrap();
    assert!(records.roots().is_empty());
    let used = snapshot
        .syntax()
        .origins
        .iter()
        .map(|o| o.span)
        .collect::<BTreeSet<_>>();
    assert_eq!(used, records.spans().iter().map(SpanRecord::key).collect());
    let contexts = records
        .contexts()
        .iter()
        .map(ContextRecord::key)
        .collect::<BTreeSet<_>>();
    for span in records.spans() {
        assert!(contexts.contains(&span.context));
    }
    for context in records.contexts() {
        if let Some(parent) = context.parent {
            assert!(contexts.contains(&parent));
        }
    }
    assert!(
        records
            .contexts()
            .iter()
            .any(|c| c.expansion == Some(Key([93; 32])))
    );
    for (token, expected) in [("gen", Edition::E2015), ("7_u8", Edition::E2024)] {
        let start = snapshot.syntax().text.find(token).unwrap() as u32;
        let origin = snapshot
            .syntax()
            .origins
            .iter()
            .find(|o| o.start <= start && start < o.end)
            .unwrap();
        let span = records
            .spans()
            .iter()
            .find(|s| s.key() == origin.span)
            .unwrap();
        assert_eq!(span.edition, expected);
        assert_eq!(
            records
                .contexts()
                .iter()
                .find(|c| c.key() == span.context)
                .unwrap()
                .edition,
            expected
        );
    }
    let generated = snapshot.syntax().text.rfind("gen").unwrap() as u32;
    let carried_semi = snapshot.syntax().text[generated as usize..]
        .find(';')
        .unwrap() as u32
        + generated;
    let origin = snapshot
        .syntax()
        .origins
        .iter()
        .find(|o| o.start <= carried_semi && carried_semi < o.end)
        .unwrap();
    let span = records
        .spans()
        .iter()
        .find(|s| s.key() == origin.span)
        .unwrap();
    assert_eq!(span.source.unit, Key([91; 32]));
    assert_eq!(span.edition, Edition::E2024);
}

#[test]
fn owned_snapshot_survives_subsequent_attachments_without_changing() {
    let mut syntax = SyntaxAttachment::parse(
        &source("fn main(){let value=first!()+second!();}"),
        DeclarationGrammar::Items,
    )
    .unwrap();
    let call = syntax.materialize().unwrap().calls.remove(0);
    syntax
        .attach(
            call.start,
            call.end,
            &source("1_i32+2_i32"),
            FragmentSlot::Expression,
        )
        .unwrap();
    let before = syntax.snapshot().unwrap();
    let identity = before.identity();
    let origins = before.origin_records().encode().unwrap();
    assert_eq!(before.syntax().calls.len(), 1);
    let call = syntax.materialize().unwrap().calls.remove(0);
    syntax
        .attach(
            call.start,
            call.end,
            &source("3_i32"),
            FragmentSlot::Expression,
        )
        .unwrap();
    assert_ne!(syntax.snapshot().unwrap().identity(), identity);
    assert_eq!(before.identity(), identity);
    assert_eq!(before.syntax().calls.len(), 1);
    assert_eq!(before.origin_records().encode().unwrap(), origins);
}

#[test]
fn empty_item_expansion_has_an_empty_closed_origin_table() {
    let mut syntax =
        SyntaxAttachment::parse(&source("selected!();"), DeclarationGrammar::Items).unwrap();
    let call = syntax.materialize().unwrap().calls.remove(0);
    syntax
        .attach_list(call.start, call.end, &source(""), ListSlot::Items)
        .unwrap();
    let snapshot = syntax.snapshot().unwrap();
    assert!(snapshot.syntax().text.is_empty());
    assert!(snapshot.syntax().origins.is_empty());
    assert!(snapshot.origin_records().contexts().is_empty());
    assert!(snapshot.origin_records().spans().is_empty());
    snapshot.origin_records().validate().unwrap();
}
