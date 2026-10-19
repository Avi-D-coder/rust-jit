//! Parser component evidence from unchanged rustc originals; no diagnostic or
//! full-fixture acceptance is inferred from these structural observations.
use super::*;

const TAIL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/traits/const-traits/const-closure-parse-not-item.rs"
);
const INITIALIZER: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/traits/const-traits/const-closure-fn-ptr-const-item.rs"
);

#[test]
fn unchanged_const_closures_use_ordinary_expression_syntax_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for original in [TAIL, INITIALIZER] {
            let blob = SourceBlob::new(original.as_bytes().to_vec()).unwrap();
            let parsed = ParsedFile::parse(
                &blob,
                ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            assert!(parsed.errors.is_empty(), "{edition}: {:?}", parsed.errors);
            assert!(parsed.parsed.errors().is_empty());
            let closures = parsed
                .parsed
                .tree()
                .syntax()
                .descendants()
                .filter_map(ast::ClosureExpr::cast)
                .collect::<Vec<_>>();
            assert_eq!(closures.len(), 1);
            assert!(closures[0].const_token().is_some());
            assert_eq!(closures[0].move_token().is_some(), original == TAIL);
            assert_eq!(parsed.parsed.tree().syntax().text().to_string(), original);
        }
    }
}

#[test]
fn const_closure_disambiguation_preserves_pinned_item_and_block_grammar() {
    // Existing upstream rust-analyzer grammar witnesses; no LRR substitute language.
    let originals = [
        include_str!("../../../../vendor/ra-ap-parser/test_data/parser/inline/ok/const_item.rs"),
        include_str!("../../../../vendor/ra-ap-parser/test_data/parser/inline/ok/const_closure.rs"),
        include_str!(
            "../../../../vendor/ra-ap-parser/test_data/parser/inline/ok/impl_item_const.rs"
        ),
        include_str!(
            "../../../../vendor/ra-ap-parser/test_data/parser/inline/ok/const_arg_block.rs"
        ),
    ];
    for original in originals {
        let parsed = ast::SourceFile::parse(original, Edition::Edition2024);
        assert!(
            parsed.errors().is_empty(),
            "{original}: {:?}",
            parsed.errors()
        );
    }
}
