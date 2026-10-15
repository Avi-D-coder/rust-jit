//! Exact upstream tokens in disclosed assertion-operand placements. These are
//! source-plan checks, not execution or full-fixture passes of the originals.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::pc::{NodeId, Phase},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::spans::SourceRange,
};

const STRINGS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lexer/lexer-crlf-line-endings-string-literal-doc-comment.rs"
);
const BYTES: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lexer/crlf-in-byte-string-literal.rs"
);

#[test]
fn upstream_crlf_assertion_operand_plans_keep_original_literal_spelling_and_offsets_all_editions() {
    let tokens = [STRINGS, BYTES]
        .into_iter()
        .flat_map(|source| {
            ast::SourceFile::parse(source, Edition::Edition2024)
                .tree()
                .syntax()
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .filter(|token| {
                    matches!(token.kind(), SyntaxKind::STRING | SyntaxKind::BYTE_STRING)
                        && (token.text().starts_with('"') || token.text().starts_with("b\""))
                        && token.text().contains("\r\n")
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(tokens.len(), 4);
    for edition in [2015, 2018, 2021, 2024] {
        for token in &tokens {
            let spelling = token.text();
            let source = format!("// 🦀\r\nfn main(){{assert_eq!(({spelling}),({spelling}));}}");
            let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
            let parsed = ParsedFile::parse(
                &blob,
                ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let outline = parsed
                .outline(Digest([191; 32]), vec![], &CfgSet::guest_default())
                .unwrap();
            let body = outline
                .items
                .iter()
                .find_map(|item| item.body.clone())
                .unwrap();
            let index = parsed.index_body(&body, Default::default()).unwrap();
            let (ordinal, node) = index
                .nodes()
                .iter()
                .enumerate()
                .find(|(_, node)| node.role == NodeRole::Expression(ExpressionKind::Macro))
                .unwrap();
            let invocation = parsed
                .macro_call_at(SourceRange {
                    start: body.range.start + node.range.start,
                    end: body.range.start + node.range.end,
                })
                .unwrap();
            let arguments = parsed
                .runtime_macro_arguments(&invocation, RuntimeMacroKind::AssertEq)
                .unwrap();
            let key = index
                .pc(NodeId(ordinal.try_into().unwrap()), Phase::Entry)
                .unwrap();
            let plan = parsed
                .runtime_macro_scalar_plan(&body, &index, &key, &arguments)
                .unwrap();
            assert!(
                plan.steps()
                    .iter()
                    .all(|step| !matches!(step, RuntimeScalarStep::Fault(_)))
            );
            let literals = plan
                .steps()
                .iter()
                .filter_map(|step| match step {
                    RuntimeScalarStep::Operation(Op::Literal { literal, .. }) => Some(literal),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(literals.len(), 2);
            for literal in literals {
                assert_eq!(literal.spelling, spelling);
                let span = &literal.source;
                assert_eq!(
                    &source[(body.range.start + span.relative_start) as usize
                        ..(body.range.start + span.relative_end) as usize],
                    spelling
                );
            }
            assert_eq!(parsed.content, blob.content());
            assert_eq!(parsed.parsed.tree().syntax().text().to_string(), source);
        }
    }
}
