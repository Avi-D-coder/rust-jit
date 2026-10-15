use lrr::{
    core::digest::{Digest, StableEncode},
    project::cargo_units::{Selection, TargetKind, UnitContext, UnitGraph, UnitRole},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
const LIB: &str = include_str!("../../../fixtures/macros/cargo-unit-graphs/lib-tests.json");
const INTEGRATION: &str =
    include_str!("../../../fixtures/macros/cargo-unit-graphs/integration.json");
fn context(graph: &Value, integration: bool) -> UnitContext {
    let root = graph["roots"][0].as_u64().unwrap() as usize;
    UnitContext {
        package: graph["units"][root]["pkg_id"].as_str().unwrap().into(),
        selection: if integration {
            Selection::IntegrationTest("integration".into())
        } else {
            Selection::Library {
                test: true,
                name: "lrr_tool_probe_app".into(),
            }
        },
        ordinary_role: UnitRole::Live,
        host: "aarch64-apple-darwin".into(),
        target: "x86_64-unknown-linux-gnu".into(),
        cfg_by_target: BTreeMap::from([
            (
                "aarch64-apple-darwin".into(),
                Digest::of_bytes(b"captured-host-cfg-fixture"),
            ),
            (
                "x86_64-unknown-linux-gnu".into(),
                Digest::of_bytes(b"captured-target-cfg-fixture"),
            ),
        ]),
        rustc_fingerprint: Digest::of_bytes(b"pinned-compiler-fixture"),
    }
}
fn parse(graph: &Value, integration: bool) -> UnitGraph {
    UnitGraph::parse(
        &serde_json::to_vec(graph).unwrap(),
        &context(graph, integration),
    )
    .unwrap()
}
#[test]
fn cargo_array_reordering_preserves_unit_and_edge_identities() {
    let old: Value = serde_json::from_str(INTEGRATION).unwrap();
    let before = parse(&old, true);
    let mut new = old.clone();
    let count = new["units"].as_array().unwrap().len();
    new["units"].as_array_mut().unwrap().reverse();
    for unit in new["units"].as_array_mut().unwrap() {
        for edge in unit["dependencies"].as_array_mut().unwrap() {
            edge["index"] = json!(count - 1 - edge["index"].as_u64().unwrap() as usize);
        }
        unit["dependencies"].as_array_mut().unwrap().reverse();
    }
    for root in new["roots"].as_array_mut().unwrap() {
        *root = json!(count - 1 - root.as_u64().unwrap() as usize);
    }
    let after = parse(&new, true);
    assert_eq!(before.roots, after.roots);
    assert_eq!(
        before.units.keys().collect::<Vec<_>>(),
        after.units.keys().collect::<Vec<_>>()
    );
    for (key, unit) in before.units {
        assert_eq!(unit.dependencies, after.units[&key].dependencies);
    }
}

#[test]
fn resolved_named_strip_profiles_are_checked_and_enter_unit_identity() {
    let mut graph: Value = serde_json::from_str(LIB).unwrap();
    for unit in graph["units"].as_array_mut().unwrap() {
        unit["profile"]["strip"] = json!({"resolved":{"Named":"debuginfo"}});
    }
    let debuginfo = parse(&graph, false);
    for unit in graph["units"].as_array_mut().unwrap() {
        unit["profile"]["strip"] = json!({"resolved":{"Named":"symbols"}});
    }
    assert_ne!(debuginfo.roots, parse(&graph, false).roots);
    for invalid in [
        json!({"resolved":{"Named":"unknown"}}),
        json!({"resolved":{"Named":"debuginfo","extra":0}}),
        json!({"resolved":{"Named":"debuginfo"},"extra":0}),
        json!({"resolved":"debuginfo"}),
    ] {
        graph["units"][0]["profile"]["strip"] = invalid;
        assert!(
            UnitGraph::parse(
                &serde_json::to_vec(&graph).unwrap(),
                &context(&graph, false)
            )
            .unwrap_err()
            .message
            .contains("strip")
        );
    }
}
#[test]
fn host_worlds_and_test_intent_are_not_package_wide() {
    let graph: Value = serde_json::from_str(INTEGRATION).unwrap();
    let normalized = parse(&graph, true);
    let root = normalized
        .units
        .get(normalized.roots.first().unwrap())
        .unwrap();
    assert!(root.key.test_intent);
    assert_eq!(root.key.target_kind, TargetKind::IntegrationTest);
    let library = normalized
        .units
        .values()
        .find(|u| u.key.target_kind == TargetKind::Library)
        .unwrap();
    assert!(!library.key.test_intent);
    let script = normalized
        .units
        .values()
        .find(|u| u.key.target_kind == TargetKind::BuildScript)
        .unwrap();
    assert_eq!(script.key.package, library.key.package);
    assert_eq!(script.key.role, UnitRole::BuildHost);
    assert_ne!(script.key.fingerprint(), library.key.fingerprint());
    for unit in normalized.units.values() {
        if matches!(unit.key.role, UnitRole::BuildHost | UnitRole::ProcMacroHost) {
            assert!(!unit.key.test_intent);
            assert_eq!(unit.key.compile_target, "aarch64-apple-darwin");
        }
    }
}
#[test]
fn changed_macro_feature_invalidates_dependents_but_not_unrelated_build_unit() {
    let old: Value = serde_json::from_str(LIB).unwrap();
    let before = parse(&old, false);
    let mut new = old.clone();
    let index = new["units"]
        .as_array()
        .unwrap()
        .iter()
        .position(|u| u["target"]["kind"] == json!(["proc-macro"]))
        .unwrap();
    new["units"][index]["features"]
        .as_array_mut()
        .unwrap()
        .push(json!("additional-feature"));
    let after = parse(&new, false);
    assert_ne!(before.roots, after.roots);
    for (key, unit) in before.units {
        if unit.key.target_kind == TargetKind::BuildScript {
            assert!(after.units.contains_key(&key));
        }
    }
}
#[test]
fn unsupported_unit_schema_and_graph_cycles_fail_before_publication() {
    let original: Value = serde_json::from_str(LIB).unwrap();
    for mutated in [
        {
            let mut v = original.clone();
            v["version"] = json!(2);
            v
        },
        {
            let mut v = original.clone();
            v["units"][0]["unknown_required_field"] = json!(true);
            v
        },
        {
            let mut v = original.clone();
            v["units"][0]["mode"] = json!("future-check-mode");
            v
        },
        {
            let mut v = original.clone();
            v["units"][0]["profile"]["unknown_profile_semantics"] = json!(true);
            v
        },
        {
            let mut v = original.clone();
            v["units"][0]["profile"]["lto"] = json!("guess-next-compiler-rule");
            v
        },
        {
            let mut v = original.clone();
            v["units"][0]["dependencies"].as_array_mut().unwrap().push(json!({"index":0,"extern_crate_name":"recursive","public":false,"noprelude":false,"nounused":false}));
            v
        },
    ] {
        let error = UnitGraph::parse(
            &serde_json::to_vec(&mutated).unwrap(),
            &context(&original, false),
        )
        .unwrap_err();
        assert_eq!(error.code, "LRR-E-UNIT-GRAPH");
    }
}
#[test]
fn all_official_selected_root_editions_are_preserved() {
    let original: Value = serde_json::from_str(LIB).unwrap();
    let mut root_old = original.clone();
    let mut identities = std::collections::BTreeSet::new();
    for edition in [2015, 2018, 2021, 2024] {
        root_old["units"][0]["target"]["edition"] = json!(edition.to_string());
        let graph = UnitGraph::parse(
            &serde_json::to_vec(&root_old).unwrap(),
            &context(&original, false),
        )
        .unwrap();
        let key = *graph.roots.iter().next().unwrap();
        assert_eq!(graph.units[&key].key.edition, edition);
        assert!(identities.insert(key));
    }
    root_old["units"][0]["target"]["edition"] = json!("2099");
    assert_eq!(
        UnitGraph::parse(
            &serde_json::to_vec(&root_old).unwrap(),
            &context(&original, false)
        )
        .unwrap_err()
        .code,
        "LRR-E-LIVE-EDITION"
    );
    let mut old_dependency = original;
    let index = old_dependency["units"]
        .as_array()
        .unwrap()
        .iter()
        .position(|u| u["target"]["kind"] == json!(["proc-macro"]))
        .unwrap();
    old_dependency["units"][index]["target"]["edition"] = json!("2018");
    let normalized = parse(&old_dependency, false);
    assert!(
        normalized
            .units
            .values()
            .any(|u| u.key.role == UnitRole::ProcMacroHost && u.key.edition == 2018)
    );
}
