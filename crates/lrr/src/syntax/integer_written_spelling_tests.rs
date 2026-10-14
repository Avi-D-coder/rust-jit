//! CA027 spelling acceptance only; no full compiletest or runtime pass receipt.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    facade::macros::RuntimeMacroKind,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{body_index::BodyIndexLimits, parse::ParsedFile, spans::SourceRange},
};
const OVERFLOW: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lint/lint-type-overflow.rs"
);
const CONSTANTS: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-contents.rs");
const PRECEDENCE: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/lint/negative_literals.rs");
fn world(text: &str, edition: u16, name: &str) -> (ParsedFile, BodyHandle, BodyIndex) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(
        &blob,
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let outline = parsed
        .outline(
            Digest::of_bytes(b"LRR exact written integer spelling source"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .clone();
    let index = parsed
        .index_body(&body, BodyIndexLimits::default())
        .unwrap();
    (parsed, body, index)
}
fn prepared(
    text: &str,
    edition: u16,
    name: &str,
    marker: &str,
    written: &str,
) -> (BodyHandle, VerifiedRecipe) {
    let (parsed, body, index) = world(text, edition, name);
    let absolute = text.find(marker).unwrap() + marker.find(written).unwrap();
    let relative = u32::try_from(absolute)
        .unwrap()
        .checked_sub(body.range.start)
        .unwrap();
    let node = index
        .nodes()
        .iter()
        .find(|node| {
            node.range.start == relative
                && node.range.end == relative + u32::try_from(written.len()).unwrap()
        })
        .expect("exact original prefix expression node");
    let key = index.pc(node.id, Phase::Entry).unwrap();
    let recipe = parsed.initial_thunk(&body, &index, &key).unwrap();
    (body, recipe)
}
fn assert_written(text: &str, body: &BodyHandle, literal: &LiteralRecipe, written: &str) {
    assert_eq!(literal.source.body_key, body.key());
    let start = body
        .range
        .start
        .checked_add(literal.source.relative_start)
        .unwrap();
    let end = body
        .range
        .start
        .checked_add(literal.source.relative_end)
        .unwrap();
    let mapped = text.get(start as usize..end as usize).unwrap();
    assert_eq!(
        mapped, written,
        "source span must retain complete written prefix"
    );
    assert_eq!(
        literal.spelling, mapped,
        "cached spelling must equal its mapped written source"
    );
}
fn check_official_prefix(
    marker: &str,
    written: &str,
    magnitude: u128,
    suffix: Option<PrimitiveType>,
) {
    for edition in [2015, 2018, 2021, 2024] {
        let (body, recipe) = prepared(OVERFLOW, edition, "main", marker, written);
        let literal = recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| match op {
                Op::Literal { literal, .. } => Some(literal),
                _ => None,
            })
            .unwrap();
        assert_eq!(literal.kind, LiteralKind::NegativeInteger);
        assert_eq!(
            literal.decoded,
            Some(DecodedLiteral::Integer {
                negative: true,
                radix: 10,
                magnitude,
                suffix,
            })
        );
        assert_written(OVERFLOW, &body, literal, written);
    }
    // Compile-fail original: source-recipe control, not successful compiletest.
}
#[test]
fn unchanged_original_parenthesized_negative_minimum_spelling_all_editions() {
    check_official_prefix(
        "let x = -(128_i8);",
        "-(128_i8)",
        128,
        Some(PrimitiveType::I8),
    );
}
#[test]
fn unchanged_original_parenthesized_overflow_spelling_all_editions() {
    check_official_prefix("let x3: i8 = -(129);", "-(129)", 129, None);
}
#[test]
fn unchanged_original_unparenthesized_minimum_spelling_stays_exact_all_editions() {
    check_official_prefix("let x = -128_i8;", "-128_i8", 128, Some(PrimitiveType::I8));
}
#[test]
fn unchanged_static_parenthesized_integer_preserves_source_spelling_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (body, recipe) = prepared(
            CONSTANTS,
            edition,
            "neg",
            "static neg : isize = -(1);",
            "-(1)",
        );
        let literal = recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| match op {
                Op::Literal { literal, .. } => Some(literal),
                _ => None,
            })
            .unwrap();
        assert_written(CONSTANTS, &body, literal, "-(1)");
        assert_eq!(
            literal.decoded,
            Some(DecodedLiteral::Integer {
                negative: true,
                radix: 10,
                magnitude: 1,
                suffix: None,
            })
        );
    }
}
#[test]
fn negative_fold_retains_trivia_radix_and_separators_without_affecting_decoded_payload() {
    let written = "- /* sign gap */ ( (0x8_0_i8) )";
    let text = format!("fn main(){{let value:i8={written};}}");
    for edition in [2015, 2018, 2021, 2024] {
        let (body, recipe) = prepared(&text, edition, "main", &text, written);
        let literal = recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| match op {
                Op::Literal { literal, .. } => Some(literal),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            literal.decoded,
            Some(DecodedLiteral::Integer {
                negative: true,
                radix: 16,
                magnitude: 128,
                suffix: Some(PrimitiveType::I8),
            })
        );
        assert_written(&text, &body, literal, written);
    }
}
#[test]
fn scalar_assertion_component_retains_lifted_original_written_expression_and_trivia() {
    // Explicit scalar component capture. This wrapper is supplemental LRR source,
    // not an unchanged upstream whole fixture or runtime macro authority claim.
    for written in ["-(128_i8)", "-(129)", "- /* sign gap */ ( (0x8_0_i8) )"] {
        let text = format!("fn main(){{assert_eq!({written}, {written});}}");
        for edition in [2015, 2018, 2021, 2024] {
            let (parsed, body, index) = world(&text, edition, "main");
            let node = index
                .nodes()
                .iter()
                .find(|node| {
                    node.role
                        == super::super::body_index::NodeRole::Expression(
                            super::super::body_index::ExpressionKind::Macro,
                        )
                })
                .unwrap();
            let key = index.pc(node.id, Phase::Entry).unwrap();
            let range = SourceRange::checked(
                body.range.start.checked_add(node.range.start).unwrap(),
                body.range.start.checked_add(node.range.end).unwrap(),
            )
            .unwrap();
            let selected = parsed.macro_call_at(range).unwrap();
            let arguments = parsed
                .runtime_macro_arguments(&selected, RuntimeMacroKind::AssertEq)
                .unwrap();
            let ops = parsed
                .runtime_macro_scalar_operations(&body, &index, &key, &arguments)
                .unwrap();
            let literals = ops
                .iter()
                .filter_map(|op| match op {
                    Op::Literal { literal, .. } => Some(literal),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(literals.len(), 2);
            for literal in literals {
                assert_eq!(literal.kind, LiteralKind::NegativeInteger);
                assert_written(&text, &body, literal, written);
            }
        }
    }
}
#[test]
fn unchanged_method_precedence_case_does_not_fold_a_nonliteral_operand() {
    for edition in [2015, 2018, 2021, 2024] {
        let (_, recipe) = prepared(
            PRECEDENCE,
            edition,
            "main",
            "let _ = -(1i32).abs();",
            "-(1i32).abs()",
        );
        assert!(
            !recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::Literal { .. }))
        );
        assert!(
            recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::NumericHint { .. }))
        );
    }
}
