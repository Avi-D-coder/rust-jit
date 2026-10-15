//! Original rustc sources and disclosed literal-placement controls. Structural
//! parsing is not execution of the original main or a diagnostic fixture pass.
use super::*;
use crate::{
    core::profile::ProfileIdentity, source::snapshot::SourceBlob, syntax::parse::ParsedFile,
};

const STRINGS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lexer/lexer-crlf-line-endings-string-literal-doc-comment.rs"
);
const BYTES: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lexer/crlf-in-byte-string-literal.rs"
);

fn parse(original: &str, edition: u16) -> ParsedFile {
    let blob = SourceBlob::new(original.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(
        &blob,
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    assert_eq!(parsed.content, blob.content());
    assert_eq!(parsed.parsed.tree().syntax().text().to_string(), original);
    assert_eq!(blob.bytes(), original.as_bytes());
    parsed
}

#[test]
fn unchanged_crlf_sources_admit_original_expression_and_macro_literal_ranges_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for original in [STRINGS, BYTES] {
            let parsed = parse(original, edition);
            assert!(parsed.errors.is_empty(), "{edition}: {:?}", parsed.errors);
            assert!(!parsed.parsed.errors().is_empty());
        }
    }
}

#[test]
fn upstream_crlf_tokens_in_ordinary_expressions_preserve_genuine_literal_errors() {
    let tokens = [STRINGS, BYTES]
        .into_iter()
        .flat_map(|source| {
            ast::SourceFile::parse(source, Edition::Edition2024)
                .tree()
                .syntax()
                .descendants_with_tokens()
                .filter_map(|item| item.into_token())
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
            let original = token.text();
            let unknown = format!("{}\\q\"", &original[..original.len() - 1]);
            let bare_cr = format!("{}x\r\"", &original[..original.len() - 1]);
            for (spelling, valid) in [
                (original, true),
                (unknown.as_str(), false),
                (bare_cr.as_str(), false),
            ] {
                let source = format!("// 🦀\r\nfn main(){{let s={spelling};}}");
                let parsed = parse(&source, edition);
                assert_eq!(
                    parsed.errors.is_empty(),
                    valid,
                    "{edition}: {spelling:?}: {:?}",
                    parsed.errors
                );
            }
        }
    }
}
