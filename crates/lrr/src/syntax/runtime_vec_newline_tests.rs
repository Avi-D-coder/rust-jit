//! Disclosed vec adaptations of unchanged upstream CRLF byte tokens. Capturing
//! argument ranges is not execution of Vec or a full upstream fixture pass.
use super::*;
use crate::{core::profile::ProfileIdentity, source::snapshot::SourceBlob};
use ra_ap_syntax::AstToken;

const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lexer/crlf-in-byte-string-literal.rs"
);

#[test]
fn upstream_crlf_byte_tokens_keep_original_vec_list_and_repeat_ranges() {
    let tokens = ast::SourceFile::parse(ORIGINAL, Edition::Edition2024)
        .tree()
        .syntax()
        .descendants_with_tokens()
        .filter_map(|item| item.into_token())
        .filter_map(ast::ByteString::cast)
        .filter(|token| token.text().contains("\r\n"))
        .collect::<Vec<_>>();
    assert_eq!(tokens.len(), 2);
    for edition in [2015, 2018, 2021, 2024] {
        for token in &tokens {
            for (separator, form, trailing) in [
                (",", VecForm::List, ",\r\n"),
                (";", VecForm::Repeat, "\r\n"),
            ] {
                let second = if form == VecForm::List {
                    token.text()
                } else {
                    "2usize"
                };
                let prefix = "// 🦀\r\nfn main(){let v=vec![\r\n";
                let text = format!(
                    "{prefix}{}\r\n{separator}\r\n{second}{trailing}];}}",
                    token.text()
                );
                let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
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
                    .runtime_macro_arguments(&selected, RuntimeMacroKind::Vec)
                    .unwrap();
                assert_eq!(args.vec_form(), Some(form));
                assert_eq!(args.operands().len(), 2);
                assert_eq!(args.operands(), args.declaration_arguments());
                let first = args.operands()[0];
                assert_eq!(first.start as usize, prefix.len());
                assert_eq!(first.end as usize, prefix.len() + token.text().len());
                let slices = args
                    .operands()
                    .iter()
                    .map(|span| &text[span.start as usize..span.end as usize])
                    .collect::<Vec<_>>();
                assert_eq!(slices, [token.text(), second]);
                assert_eq!(args.source(), &parsed.key());
                assert_eq!(blob.bytes(), text.as_bytes());
            }
        }
    }
}
