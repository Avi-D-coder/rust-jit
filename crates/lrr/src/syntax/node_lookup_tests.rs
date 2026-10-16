//! Range-attachment correspondence against unchanged upstream originals. The
//! full scan is a test oracle, never production entered-phase lookup.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    source::snapshot::{CfgSet, SourceBlob},
};
const ORIGINALS: &[&str] = &[
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/return/return-nil.rs"),
    include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    ),
    include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-call.rs"
    ),
    include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/match-range-infer.rs"
    ),
];
#[test]
fn indexed_role_lookup_matches_source_preorder_on_unchanged_originals_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut let_bindings = 0;
        let mut other_patterns = 0;
        for original in ORIGINALS {
            let mut profile = ProfileIdentity::current();
            profile.edition = edition;
            let parsed = ParsedFile::parse(
                &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
                profile,
            )
            .unwrap();
            let cfg = CfgSet::guest_default();
            let outline = parsed.outline(Digest([71; 32]), vec![], &cfg).unwrap();
            let mut checked = 0;
            for body in outline.items.iter().filter_map(|item| item.body.as_ref()) {
                let index = parsed.index_body(body, Default::default()).unwrap();
                for node in index.nodes() {
                    let expected = parsed.parsed.tree().syntax().descendants().find(|syntax| {
                        u32::from(syntax.text_range().start())
                            == body.range.start + node.range.start
                            && u32::from(syntax.text_range().end())
                                == body.range.start + node.range.end
                            && role_matches(syntax, node.role)
                    });
                    let actual = parsed.entered_node_syntax(body, &index, node.id).unwrap();
                    assert_eq!(actual, expected, "edition {edition}, node {:?}", node.id);
                    // Separate structural oracle for let ownership. The normal
                    // executable-pattern lookup intentionally selects only
                    // match patterns; local declaration lookup accepts the
                    // exact indexed pattern and its immediate owning let.
                    let expected_let = (node.role == NodeRole::Pattern)
                        .then(|| {
                            parsed.parsed.tree().syntax().descendants().find(|syntax| {
                                u32::from(syntax.text_range().start())
                                    == body.range.start + node.range.start
                                    && u32::from(syntax.text_range().end())
                                        == body.range.start + node.range.end
                                    && ast::Pat::can_cast(syntax.kind())
                            })
                        })
                        .flatten()
                        .and_then(|pattern| pattern.parent().and_then(ast::LetStmt::cast));
                    let_bindings += usize::from(expected_let.is_some());
                    other_patterns +=
                        usize::from(node.role == NodeRole::Pattern && expected_let.is_none());
                    assert_eq!(
                        parsed.binding_let_statement(body, &index, node.id).unwrap(),
                        expected_let,
                        "edition {edition}, binding node {:?}",
                        node.id
                    );
                    checked += 1;
                }
            }
            assert!(checked > 0);
        }
        assert!(let_bindings > 0 && other_patterns > 0);
    }
}
#[test]
fn direct_lookup_refuses_outside_ranges_and_does_not_return_partial_tokens() {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINALS[0].as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let end = u32::from(parsed.parsed.tree().syntax().text_range().end());
    assert!(
        parsed
            .syntax_at(SourceRange::checked(end, end).unwrap(), |_| true)
            .unwrap()
            .is_none()
    );
    assert!(
        parsed
            .syntax_at(SourceRange::checked(end, end + 1).unwrap(), |_| true)
            .unwrap()
            .is_none()
    );
    let function = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::Fn::cast)
        .unwrap();
    let range = function.syntax().text_range();
    assert_eq!(
        parsed
            .syntax_at(
                SourceRange::checked(range.start().into(), range.end().into()).unwrap(),
                |node| ast::Fn::can_cast(node.kind())
            )
            .unwrap()
            .as_ref(),
        Some(function.syntax())
    );
}
