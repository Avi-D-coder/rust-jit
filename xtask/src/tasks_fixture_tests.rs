//! LRR ledger guards for the specification's full revision-four fixture matrix.
use super::*;

#[test]
fn revision_four_fixture_matrix_retains_macro_trait_native_and_cache_contracts() {
    let spec =
        fs::read_to_string(crate::root().join("lrr_rust2024_macros_traits_implementation_spec.md"))
            .unwrap();
    let registered = BTreeSet::from([
        "CACHE-SNAPSHOT-METADATA".to_owned(),
        "CARGO-NATIVE-REUSE".to_owned(),
        "MX999".to_owned(),
        "TR999".to_owned(),
        "NX999".to_owned(),
        "CA999".to_owned(),
    ]);
    let known = acceptance_ids(&spec, &registered);
    assert!(known.contains("CACHE-SNAPSHOT-METADATA"));
    assert!(known.contains("CARGO-NATIVE-REUSE"));
    assert!(!known.contains("MX999"));
    for required in [
        "MX014", "TR001", "TR002", "TR028", "NX005", "CA001", "CA003",
    ] {
        assert!(
            known.contains(required),
            "lost normative fixture {required}"
        );
    }
    assert!(!known.contains("TR999"));
    assert!(!known.contains("NX999"));
    assert!(!known.contains("CA999"));
}

#[test]
fn revision_four_fixture_parser_rejects_other_tables_and_malformed_identifiers() {
    for line in [
        "| TR | text |",
        "| TR1 | text |",
        "| TR0001 | text |",
        "| TR00x | text |",
        "| BAD01 | text |",
        "| D15 | decision |",
        "| TR001 extra | text |",
        "TR001",
        "**TR001 — body**",
        "| fixture ID | Required setup |",
    ] {
        assert_eq!(spec_fixture_id(line), None, "accepted {line}");
    }
    for family in ["MX", "TR", "NX", "CA"] {
        let line = format!("| {family}001 | observation | ");
        assert_eq!(spec_fixture_id(&line), Some(&line[2..7]));
    }
}

#[test]
fn pending_trait_fixture_does_not_become_release_success_from_component_completion() {
    let spec =
        fs::read_to_string(crate::root().join("lrr_rust2024_macros_traits_implementation_spec.md"))
            .unwrap();
    let acceptance = spec.lines().filter_map(spec_fixture_id).collect();
    let known = BTreeSet::from(["T044", "M029"]);
    let completed = known.clone();
    let mut task = serde_json::json!({
        "id":"T044", "status":"complete", "completion_scope":"component",
        "release_complete":false, "integration_dependencies":["M029"],
        "pending_acceptance":[{"fixture_id":"TR028","status":"pending","evidence_paths":[]}]
    });
    validate_integration_completion(&task, &known, &completed, &acceptance).unwrap();
    task["release_complete"] = serde_json::json!(true);
    assert!(validate_integration_completion(&task, &known, &completed, &acceptance).is_err());
    task["pending_acceptance"][0]["status"] = serde_json::json!("passed");
    assert!(validate_integration_completion(&task, &known, &completed, &acceptance).is_err());
    let full = serde_json::json!({
        "fixture_id":"TR028", "runner":"lrr", "scope":"full_lrr", "status":"passed",
        "semantics_profile":"lrr-2024-v2", "rust_version":"1.99.0",
        "platform":"aarch64-apple-darwin", "observations":{"metadata_control":true}
    });
    // Synthetic validator input, not an artifact or an actual TR028 pass.
    validate_final_observation("TR028", &full).unwrap();
    for (field, value) in [
        ("runner", "rustc"),
        ("scope", "native_oracle"),
        ("scope", "component"),
    ] {
        let mut evidence = full.clone();
        evidence[field] = serde_json::json!(value);
        assert!(validate_final_observation("TR028", &evidence).is_err());
    }
    task["release_complete"] = serde_json::json!(false);
    task["pending_acceptance"][0] =
        serde_json::json!({"fixture_id":"TR999","status":"pending","evidence_paths":[]});
    assert!(validate_integration_completion(&task, &known, &completed, &acceptance).is_err());
}
