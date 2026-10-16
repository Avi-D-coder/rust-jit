//! Source-graph controls. No guest evaluator or runtime-fixture receipt.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    source::snapshot::{CfgSet, SourceBlob},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/or-patterns/search-via-bindings.rs"
);

fn prepared(
    source: &str,
    function: &str,
    edition: u16,
) -> (std::sync::Arc<BodyIndex>, PatternProductPlan) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([88; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == function)
        .unwrap()
        .body
        .clone()
        .unwrap();
    let index = std::sync::Arc::new(parsed.index_body(&body, Default::default()).unwrap());
    let pattern = parsed
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
    let range = pattern.syntax().text_range();
    let relative = SourceRange {
        start: u32::from(range.start()) - body.range.start,
        end: u32::from(range.end()) - body.range.start,
    };
    let root = index
        .nodes_at_range(relative)
        .find(|node| node.role == NodeRole::Pattern)
        .unwrap()
        .id;
    let plan = PatternProductPlan::prepare(&parsed, &body, &index, root).unwrap();
    (index, plan)
}

// Enumerate at most the original's eight tiny source combinations to compare
// structural edge order. Production never constructs Cartesian alternatives;
// these are binding-path facts, not guard execution or fixture output.
fn binding_paths(
    plan: &PatternProductPlan,
    index: &BodyIndex,
    node: NodeId,
) -> Vec<Vec<(String, Vec<u32>)>> {
    let row = plan.row(node).unwrap();
    let product = |left: Vec<Vec<(String, Vec<u32>)>>, right: Vec<Vec<(String, Vec<u32>)>>| {
        let mut out = Vec::new();
        for prefix in left {
            for suffix in &right {
                let mut path = prefix.clone();
                path.extend(suffix.iter().cloned());
                out.push(path);
            }
        }
        out
    };
    match &row.kind {
        DecisionKind::Wildcard | DecisionKind::Scalar(_) => vec![vec![]],
        DecisionKind::Unsupported(_) => panic!("source graph expected a supported structural edge"),
        DecisionKind::Group(inner) => binding_paths(plan, index, *inner),
        DecisionKind::Or(children) => children
            .iter()
            .flat_map(|child| binding_paths(plan, index, *child))
            .collect(),
        DecisionKind::Tuple(children) => children.iter().fold(vec![vec![]], |paths, child| {
            product(paths, binding_paths(plan, index, *child))
        }),
        DecisionKind::Binding(intent) => {
            let group = index.pattern_bindings().group_for(node).unwrap();
            assert_eq!(intent.canonical, group.canonical_candidate());
            let paths = vec![vec![(group.name().to_owned(), row.source_path.to_vec())]];
            match intent.subpattern {
                Some(inner) => product(paths, binding_paths(plan, index, inner)),
                None => paths,
            }
        }
    }
}

#[test]
fn unchanged_stock_nested_and_spelled_out_products_retain_identical_source_order_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (index, nested) = prepared(ORIGINAL, "search", edition);
        let (old_index, spelled_out) = prepared(ORIGINAL, "search_old_style", edition);
        let paths = binding_paths(&nested, &index, nested.root());
        assert_eq!(paths.len(), 8);
        assert_eq!(
            paths,
            binding_paths(&spelled_out, &old_index, spelled_out.root())
        );
        for (ordinal, paths) in paths.iter().enumerate() {
            assert_eq!(
                paths
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>(),
                ["a", "b", "c"]
            );
            assert_eq!(paths[0].1, [0, u32::try_from(ordinal / 4).unwrap()]);
            assert_eq!(paths[1].1, [1, u32::try_from((ordinal / 2) % 2).unwrap()]);
            assert_eq!(paths[2].1, [2, u32::try_from(ordinal % 2).unwrap()]);
        }
        assert_eq!(
            nested
                .rows()
                .iter()
                .filter(|row| matches!(row.kind, DecisionKind::Or(_)))
                .count(),
            3
        );
        assert!(nested.metadata_bytes() < 32 * 1024 * 1024);
        let (_, repeated) = prepared(ORIGINAL, "search", edition);
        assert_eq!(nested.fingerprint(), repeated.fingerprint());
    }
}

#[test]
fn original_dummy_wildcard_alternatives_remain_distinct_source_choices() {
    let (index, plan) = prepared(ORIGINAL, "search_with_dummy", 2024);
    let paths = binding_paths(&plan, &index, plan.root());
    assert_eq!(paths.len(), 8);
    for repeated in paths.chunks_exact(2) {
        assert_eq!(repeated[0], repeated[1]);
    }
    let wildcard_or=plan.rows().iter().filter(|row|matches!(&row.kind,DecisionKind::Or(children)
        if children.iter().all(|child|matches!(plan.row(*child).unwrap().kind,DecisionKind::Wildcard)))).count();
    assert_eq!(wildcard_or, 1);
}

#[test]
fn source_metadata_is_linear_even_when_logical_product_exceeds_a_trillion_paths() {
    // LRR allocation control outside originals, not a substitute language suite.
    let source = format!(
        "fn main(){{match ({}){{({})=>(),_=>()}}}}",
        vec!["false"; 40].join(","),
        vec!["false | true"; 40].join(",")
    );
    let (_, plan) = prepared(&source, "main", 2024);
    assert_eq!(
        plan.rows()
            .iter()
            .filter(|row| matches!(row.kind, DecisionKind::Or(_)))
            .count(),
        40
    );
    assert!(plan.rows().len() < 200);
    assert!(plan.metadata_bytes() < 128 * 1024);
}

#[test]
fn cold_source_graph_capture_does_not_decode_scalar_upper_bounds_or_prepare_result_calls() {
    // The invalid reached bound remains a deferred scalar decoding obligation;
    // this metadata check is not a valid Rust/runtime-contract pass.
    let source = "fn cold()->(){loop{}} fn main(){match 1u8{0..=9999999999999999999999999999999999999999=>cold(),_=>()}}";
    let (_, plan) = prepared(source, "main", 2024);
    assert_eq!(plan.rows().len(), 1);
    assert!(matches!(
        plan.rows()[0].kind,
        DecisionKind::Scalar(ScalarTestPlan::Range { upper: Some(_), .. })
    ));
}
