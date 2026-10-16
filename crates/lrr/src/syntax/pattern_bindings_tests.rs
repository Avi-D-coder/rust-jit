//! Source facts and negative authority controls outside unchanged originals.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    source::snapshot::{CfgSet, SourceBlob},
};
use ra_ap_syntax::ast;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/or-patterns/search-via-bindings.rs"
);

fn entered(
    source: &str,
    edition: u16,
    function: &str,
    missing_context: bool,
) -> (ParsedFile, BodyHandle, BodyIndex, PatternBindings, NodeId) {
    let mut parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    if missing_context {
        parsed.generated_origin = Some(Digest([87; 32]));
    }
    let outline = parsed
        .outline(Digest([86; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == function)
        .unwrap()
        .body
        .clone()
        .unwrap();
    let index = parsed.index_body(&body, Default::default()).unwrap();
    let root = parsed
        .entered_body_root(&body)
        .unwrap()
        .descendants()
        .find_map(ast::MatchExpr::cast)
        .unwrap()
        .match_arm_list()
        .unwrap()
        .arms()
        .next()
        .unwrap()
        .pat()
        .unwrap();
    let root_id = identity(&index, &body, root.syntax()).unwrap();
    let facts = PatternBindings::prepare(&parsed, &body, &index, 32 * 1024 * 1024).unwrap();
    (parsed, body, index, facts, root_id)
}

#[test]
fn unchanged_stock_nested_and_expanded_or_forms_retain_same_canonical_candidates_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (function, names, count) in [
            ("search", vec!["a", "b", "c"], 2),
            ("search_old_style", vec!["a", "b", "c"], 8),
            ("search_with_dummy", vec!["a", "b"], 2),
        ] {
            let (parsed, body, index, facts, root) = entered(ORIGINAL, edition, function, false);
            let groups = facts.groups(root);
            assert_eq!(
                groups.iter().map(BindingGroup::name).collect::<Vec<_>>(),
                names
            );
            for group in groups {
                assert_eq!(group.occurrences().len(), count);
                assert!(group.conflicts().is_resolved());
                let canonical = group.canonical_candidate().unwrap();
                for occurrence in group.occurrences() {
                    assert_eq!(
                        facts
                            .group_for(occurrence.pattern)
                            .unwrap()
                            .canonical_candidate(),
                        Some(canonical)
                    );
                    assert_eq!(occurrence.edition, Some(edition));
                }
                assert_eq!(group.context(), BindingContext::Original);
                // Unadorned a/b are still possible constant names. @ makes
                // the syntactic binding explicit; neither facts table is a
                // namespace certificate or permission to publish a cell.
                assert_eq!(
                    group.requires_namespace(),
                    group.name() == "a" || function == "search_with_dummy"
                );
            }
            assert!(facts.metadata_bytes().unwrap() < 32 * 1024 * 1024);
            let repeated =
                PatternBindings::prepare(&parsed, &body, &index, 32 * 1024 * 1024).unwrap();
            assert_eq!(facts, repeated);
            assert_eq!(facts.fingerprint(), repeated.fingerprint());
        }
    }
}

#[test]
fn binding_conflicts_are_unresolved_source_facts_not_static_acceptance_or_binding_authority() {
    for (pattern, duplicate, missing, mode) in [
        ("(a,a)", true, false, false),
        ("(a,_)|(_,_)", false, true, false),
        ("(ref a,_)|(_,a)", false, false, true),
        ("a @ (a,_)", true, false, false),
    ] {
        let source = format!("fn main(){{match (true,false){{{pattern}=>(),_=>()}}}}");
        let (_, _, _, facts, root) = entered(&source, 2024, "main", false);
        let group = &facts.groups(root)[0];
        assert_eq!(group.name(), "a");
        assert_eq!(group.conflicts().conjunction_duplicate, duplicate);
        assert_eq!(group.conflicts().alternative_missing, missing);
        assert_eq!(group.conflicts().written_mode, mode);
        assert!(group.canonical_candidate().is_none());
    }
}

#[test]
fn structural_wrapper_nodes_do_not_create_duplicate_pattern_roots() {
    let source = "struct S{field:(bool,bool)} fn main(){match (S{field:(true,false)}){S{field:(a,_)}=>{let _=a;}}}";
    let (_, _, index, facts, root) = entered(source, 2024, "main", false);
    assert_eq!(facts.groups(root).len(), 1);
    let group = &facts.groups(root)[0];
    assert_eq!(group.name(), "a");
    assert_eq!(group.occurrences().len(), 1);
    let occurrence = group.occurrences()[0].pattern;
    assert!(index.nodes()[occurrence.0 as usize].parent.is_some());
    assert_eq!(
        facts.group_for(occurrence).unwrap().canonical_candidate(),
        Some(occurrence)
    );
}

#[test]
fn missing_actual_hygiene_never_merges_same_spelled_alternative_candidates() {
    // Deliberately incomplete provenance is a negative source-authority control,
    // not an accepted generated Rust contract or macro compatibility result.
    let (_, _, _, facts, root) = entered(
        "fn main(){match (true,false){(a,_)|(_,a)=>(),_=>()}}",
        2024,
        "main",
        true,
    );
    assert_eq!(facts.groups(root).len(), 2);
    for group in facts.groups(root) {
        assert_eq!(group.name(), "a");
        assert!(matches!(group.context(), BindingContext::Unresolved(_)));
        assert!(group.conflicts().context_unresolved);
        assert!(group.canonical_candidate().is_none());
    }
}
