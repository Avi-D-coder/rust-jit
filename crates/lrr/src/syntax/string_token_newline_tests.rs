//! Text-decoder components from unchanged upstream sources, not full runtime or
//! diagnostic fixture passes. Original source bytes never become normalized.
use super::*;
use ra_ap_syntax::AstToken;
const CRLF: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lexer/lexer-crlf-line-endings-string-literal-doc-comment.rs"
);
const WARNINGS: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/str/str-escape.rs");
const BARE_CR: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lexer/lex-bare-cr-string-literal-doc-comment.rs"
);

fn tokens(original: &str, edition: Edition) -> Vec<ast::String> {
    // Structural token extraction does not claim RA's original unnormalized
    // parse diagnostics match rustc's normalization and diagnostic pipeline.
    ast::SourceFile::parse(original, edition)
        .tree()
        .syntax()
        .descendants_with_tokens()
        .filter_map(|item| item.into_token())
        .filter_map(ast::String::cast)
        .collect()
}

#[test]
fn unchanged_string_tokens_keep_rust_newline_values_and_original_source_all_editions() {
    let cases: [(&str, &[&str]); 2] = [
        (
            CRLF,
            &[
                "string\nliteral",
                "string\nliteral",
                "literal with escaped newline",
                "literal with escaped newline",
                "string\nliteral",
                "string\nliteral",
                "byte string\nliteral",
                "lexer-crlf-line-endings-string-literal-doc-comment.rs",
                "string\r\nliteral",
            ],
        ),
        (WARNINGS, &["", "", "ab", "ab", "ab", "ab"]),
    ];
    for (year, edition) in [
        (2015, Edition::Edition2015),
        (2018, Edition::Edition2018),
        (2021, Edition::Edition2021),
        (2024, Edition::Edition2024),
    ] {
        for (original, expected) in cases {
            let source =
                crate::source::snapshot::SourceBlob::new(original.as_bytes().to_vec()).unwrap();
            let before = source.content();
            let literals = tokens(original, edition);
            assert_eq!(literals.len(), expected.len());
            for (token, expected) in literals.iter().zip(expected) {
                let range = token.syntax().text_range();
                let start = u32::from(range.start()) as usize;
                let end = u32::from(range.end()) as usize;
                assert_eq!(token.text(), &original[start..end]);
                assert_eq!(string_value(token.text(), year).as_deref(), Some(*expected));
            }
            assert_eq!(source.content(), before);
            assert_eq!(source.bytes(), original.as_bytes());
        }
    }
    // This is the original include_str witness: escaped CR/LF in a VALUE must
    // survive decoding even though actual token CRLF is normalized for lexing.
    assert!(CRLF.contains("string\r\nliteral"));
    assert_eq!(
        string_value(r#""string\r\nliteral""#, 2024).as_deref(),
        Some("string\r\nliteral")
    );
}

#[test]
fn unchanged_bare_cr_tokens_refuse_and_exact_token_boundary_remains_required() {
    let invalid = tokens(BARE_CR, Edition::Edition2024);
    assert_eq!(invalid.len(), 3);
    for token in invalid {
        assert!(token.text().contains('\r'));
        assert!(string_value(token.text(), 2024).is_none());
    }
    let literal = tokens(CRLF, Edition::Edition2024).remove(0);
    assert!(string_value(literal.text(), 2027).is_none());
    let injected = format!("{};const EXTRA:&str=\"extra\"", literal.text());
    assert!(string_value(&injected, 2024).is_none());
}

#[test]
fn unchanged_upstream_crlf_macro_arguments_retain_exact_original_operand_ranges() {
    use crate::{
        core::profile::ProfileIdentity, facade::macros::RuntimeMacroKind,
        source::snapshot::SourceBlob, syntax::parse::ParsedFile,
    };
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/lexer/crlf-in-byte-string-literal.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let blob = SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
        let parsed = ParsedFile::parse(
            &blob,
            ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
        )
        .unwrap();
        let calls = parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::MacroCall::cast)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        for call in calls {
            let selected = parsed
                .macro_call_at(range(call.syntax().text_range()))
                .unwrap();
            let args = parsed
                .runtime_macro_arguments(&selected, RuntimeMacroKind::AssertEq)
                .unwrap();
            let original_operands = call
                .syntax()
                .descendants_with_tokens()
                .filter_map(|item| item.into_token())
                .filter_map(ast::ByteString::cast)
                .map(|token| range(token.syntax().text_range()))
                .collect::<Vec<_>>();
            assert_eq!(original_operands.len(), 2);
            assert_eq!(args.operands(), original_operands);
            assert_eq!(args.declaration_arguments(), original_operands);
            assert_eq!(args.source(), &parsed.key());
            assert!(args.message().is_none());
            for operand in args.operands() {
                assert!(
                    ORIGINAL
                        .get(operand.start as usize..operand.end as usize)
                        .unwrap()
                        .starts_with("b\"")
                );
            }
        }
        assert_eq!(blob.bytes(), ORIGINAL.as_bytes());
    }
}

#[test]
fn stock_message_tokens_keep_offsets_after_crlf_and_lazy_format_tail() {
    use crate::{
        core::profile::ProfileIdentity, facade::macros::RuntimeMacroKind,
        source::snapshot::SourceBlob, syntax::parse::ParsedFile,
    };
    let original_tokens = tokens(CRLF, Edition::Edition2024);
    for edition in [2015, 2018, 2021, 2024] {
        for (ordinal, expected) in [
            (0, "string\nliteral"),
            (2, "literal with escaped newline"),
            (4, "string\nliteral"),
        ] {
            let token = original_tokens[ordinal].text();
            // LRR integration adaptation, outside unchanged originals: exercise
            // offset mapping and lazy message-tail capture with that exact token.
            let mut source = String::from("// 🦀\r\nfn main(){assert!(\r\ntrue\r\n,\r\n");
            let message_start = source.len();
            source.push_str(token);
            let message_end = source.len();
            source.push_str(",\r\nmissing(),\r\n);}");
            let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
            let parsed = ParsedFile::parse(
                &blob,
                ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            let call = parsed
                .parsed
                .tree()
                .syntax()
                .descendants()
                .find_map(ast::MacroCall::cast)
                .unwrap();
            let selected = parsed
                .macro_call_at(range(call.syntax().text_range()))
                .unwrap();
            let args = parsed
                .runtime_macro_arguments(&selected, RuntimeMacroKind::Assert)
                .unwrap();
            let slices = args
                .declaration_arguments()
                .iter()
                .map(|r| &source[r.start as usize..r.end as usize])
                .collect::<Vec<_>>();
            assert_eq!(slices, ["true", token, "missing()"]);
            let message = args.declaration_arguments()[1];
            assert_eq!(
                (message.start as usize, message.end as usize),
                (message_start, message_end)
            );
            assert_eq!(args.operands(), &args.declaration_arguments()[..1]);
            assert_eq!(args.literal_predicate(), Some("true"));
            assert_eq!(args.literal_message(), Some(expected));
            assert_eq!(args.message_argument_count(), 2);
            let tail = args.message().unwrap();
            assert_eq!(
                &source[tail.start as usize..tail.end as usize],
                format!("{token},\r\nmissing(),\r\n")
            );
            assert_eq!(args.source(), &parsed.key());
            assert_eq!(blob.bytes(), source.as_bytes());
            let changed = ParsedFile::parse(
                &SourceBlob::new(source.replace("true", "false").into_bytes()).unwrap(),
                ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            assert!(args.require_source(&changed.key()).is_err());
        }
    }
}

#[test]
fn stock_crlf_continuations_do_not_mask_other_original_macro_token_errors() {
    use crate::{
        core::profile::ProfileIdentity, source::snapshot::SourceBlob, syntax::parse::ParsedFile,
    };
    const BYTE_SOURCE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/lexer/crlf-in-byte-string-literal.rs"
    );
    let string = tokens(CRLF, Edition::Edition2024).remove(2);
    let byte = ast::SourceFile::parse(BYTE_SOURCE, Edition::Edition2024)
        .tree()
        .syntax()
        .descendants_with_tokens()
        .filter_map(|item| item.into_token())
        .filter_map(ast::ByteString::cast)
        .find(|token| token.text().contains("\\\r\n"))
        .unwrap();
    for edition in [2015, 2018, 2021, 2024] {
        for original in [string.text(), byte.text()] {
            assert!(original.contains("\\\r\n"));
            // Adaptations outside the originals check the admission boundary:
            // an invalid escape in this token, one in another token, and a
            // bare CR after ordinary text must not disappear with the continuation.
            let unknown = format!("{}\\q\"", &original[..original.len() - 1]);
            let bare_cr = format!("{}x\r\"", &original[..original.len() - 1]);
            // Rust skips CR among ASCII whitespace after a continuation. It
            // becomes invalid only after non-whitespace text resumes.
            let skipped_cr = format!("{}\\\r\n\t\r\"", &original[..original.len() - 1]);
            for (left, right, valid) in [
                (original, original, true),
                (skipped_cr.as_str(), original, true),
                (unknown.as_str(), original, false),
                (original, r#""\q""#, false),
                (bare_cr.as_str(), original, false),
            ] {
                let text = format!("// 🦀\r\nfn main(){{assert_eq!({left},{right});}}");
                let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
                let parsed = ParsedFile::parse(
                    &blob,
                    ProfileIdentity {
                        edition,
                        ..ProfileIdentity::current()
                    },
                )
                .unwrap();
                // The retained parser tree still has original CRLF diagnostics;
                // the admitted diagnostics are independently revalidated.
                assert!(!parsed.parsed.errors().is_empty());
                assert_eq!(parsed.parsed.tree().syntax().text().to_string(), text);
                assert_eq!(blob.bytes(), text.as_bytes());
                assert_eq!(parsed.errors.is_empty(), valid, "{edition}: {left:?}");
                let call = parsed
                    .parsed
                    .tree()
                    .syntax()
                    .descendants()
                    .find_map(ast::MacroCall::cast)
                    .unwrap();
                assert_eq!(
                    parsed
                        .macro_call_at(range(call.syntax().text_range()))
                        .is_ok(),
                    valid,
                    "{edition}: {left:?}"
                );
            }
        }
    }
}
