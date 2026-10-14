use std::{collections::BTreeSet, fs, path::Path};

fn spec_task_id(line: &str) -> Option<&str> {
    if !line.starts_with("**") {
        return None;
    }
    let id = line.get(2..6)?;
    let bytes = id.as_bytes();
    (matches!(bytes[0], b'T' | b'H' | b'M')
        && bytes[1..].iter().all(u8::is_ascii_digit)
        && line.get(6..)?.starts_with(" — "))
    .then_some(id)
}

fn expanded_fixture_id(id: &str) -> bool {
    id.len() == 5
        && matches!(id.get(..2), Some("MX" | "TR" | "NX" | "CA"))
        && id.as_bytes()[2..].iter().all(u8::is_ascii_digit)
}

/// Normative specification fixture IDs from complete table fields. Registry
/// rows remain separate: a planned matrix contract is not an executed test.
fn spec_fixture_id(line: &str) -> Option<&str> {
    let field = line.strip_prefix("| ")?.split_once('|')?.0.trim();
    expanded_fixture_id(field).then_some(field)
}

fn acceptance_ids<'a>(spec: &'a str, registered: &'a BTreeSet<String>) -> BTreeSet<&'a str> {
    let mut known: BTreeSet<_> = spec.lines().filter_map(spec_fixture_id).collect();
    // Registration cannot manufacture an absent normative matrix contract.
    // Ordinary runtime IDs such as CACHE-* and CARGO-* retain their identity.
    known.extend(
        registered
            .iter()
            .filter(|id| !id.starts_with("MX") && !expanded_fixture_id(id))
            .map(String::as_str),
    );
    known
}

fn validate_fixture_ids(
    id: &str,
    values: &serde_json::Value,
    known: &BTreeSet<String>,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values
        .as_array()
        .ok_or_else(|| format!("{id} has no fixture_ids array"))?
    {
        let fixture = value
            .as_str()
            .ok_or_else(|| format!("{id} has a non-string fixture reference"))?;
        if !known.contains(fixture) {
            return Err(format!("{id} references unknown fixture {fixture}"));
        }
        if !seen.insert(fixture) {
            return Err(format!("{id} references duplicate fixture {fixture}"));
        }
    }
    Ok(())
}

fn validate_integration_completion(
    task: &serde_json::Value,
    known_tasks: &BTreeSet<&str>,
    completed: &BTreeSet<&str>,
    known_acceptance: &BTreeSet<&str>,
) -> Result<(), String> {
    let Some(dependencies) = task.get("integration_dependencies") else {
        return Ok(());
    };
    let id = task["id"].as_str().ok_or("task without id")?;
    if task["release_complete"].as_bool().is_none() {
        return Err(format!("{id} needs an explicit boolean release_complete"));
    }
    if task["completion_scope"] != "component" {
        return Err(format!("{id} integration needs explicit component scope"));
    }
    let mut seen = BTreeSet::new();
    for dependency in dependencies
        .as_array()
        .ok_or("integration_dependencies must be an array")?
    {
        let dependency = dependency
            .as_str()
            .ok_or("non-string integration dependency")?;
        if dependency == id || !known_tasks.contains(dependency) || !seen.insert(dependency) {
            return Err(format!(
                "{id} has invalid integration dependency {dependency}"
            ));
        }
        if task["release_complete"] == true && !completed.contains(dependency) {
            return Err(format!(
                "{id} release requires completed integration {dependency}"
            ));
        }
    }
    let mut fixtures = BTreeSet::new();
    for acceptance in task["pending_acceptance"]
        .as_array()
        .ok_or("integration needs pending_acceptance array")?
    {
        let fixture = acceptance["fixture_id"]
            .as_str()
            .ok_or("missing integration fixture id")?;
        if !known_acceptance.contains(fixture) || !fixtures.insert(fixture) {
            return Err(format!("{id} has invalid integration fixture {fixture}"));
        }
        let status = acceptance["status"]
            .as_str()
            .ok_or("missing integration fixture status")?;
        let evidence = acceptance["evidence_paths"]
            .as_array()
            .ok_or("missing integration evidence paths")?;
        if !matches!(status, "pending" | "passed")
            || evidence.iter().any(|path| path.as_str().is_none())
        {
            return Err(format!("{id} has invalid integration evidence"));
        }
        if status == "passed" && evidence.is_empty() {
            return Err(format!("{fixture} has no final evidence"));
        }
        if task["release_complete"] == true && status != "passed" {
            return Err(format!("{id} release still requires {fixture}"));
        }
    }
    if fixtures.is_empty() || seen.is_empty() {
        return Err(format!("{id} has empty integration contract"));
    }
    Ok(())
}
fn validate_final_observation(fixture: &str, evidence: &serde_json::Value) -> Result<(), String> {
    if evidence["fixture_id"] != fixture
        || evidence["runner"] != "lrr"
        || evidence["scope"] != "full_lrr"
        || evidence["status"] != "passed"
        || evidence["semantics_profile"] != "lrr-2024-v2"
        || evidence["rust_version"] != "1.99.0"
        || !matches!(
            evidence["platform"].as_str(),
            Some("aarch64-apple-darwin" | "x86_64-apple-darwin" | "x86_64-unknown-linux-gnu")
        )
        || evidence["observations"]
            .as_object()
            .is_none_or(|values| values.is_empty())
    {
        return Err(format!("{fixture} lacks full LRR observation evidence"));
    }
    Ok(())
}

pub fn validate(root: &Path) -> Result<(), String> {
    let ledger: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("docs/tasks.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let spec = fs::read_to_string(root.join("lrr_rust2024_macros_traits_implementation_spec.md"))
        .map_err(|e| e.to_string())?;
    let expected: BTreeSet<_> = spec.lines().filter_map(spec_task_id).collect();
    if ledger["spec_revision"].as_str() != Some("5.0") {
        return Err("ledger revision must be 5.0".into());
    }
    let tasks = ledger["tasks"].as_array().ok_or("missing tasks array")?;
    let completed: BTreeSet<_> = tasks
        .iter()
        .filter(|t| t["status"] == "complete")
        .filter_map(|t| t["id"].as_str())
        .collect();
    let registry: toml::Value = toml::from_str(
        &fs::read_to_string(root.join("compat/fixtures.toml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut fixture_ids = BTreeSet::new();
    for fixture in registry["fixture"]
        .as_array()
        .ok_or("fixture registry missing rows")?
    {
        let id = fixture["id"]
            .as_str()
            .ok_or("fixture registry row missing id")?;
        if !fixture_ids.insert(id.to_owned()) {
            return Err(format!("duplicate registered fixture {id}"));
        }
    }
    let acceptance = acceptance_ids(&spec, &fixture_ids);
    let mut actual = BTreeSet::new();
    for task in tasks {
        let id = task["id"].as_str().ok_or("task without id")?;
        if !actual.insert(id) {
            return Err(format!("duplicate task {id}"));
        }
        let status = task["status"].as_str().ok_or("task without status")?;
        if !["not_started", "in_progress", "complete"].contains(&status) {
            return Err(format!("invalid status for {id}"));
        }
        for key in [
            "acceptance_tests",
            "commands",
            "fixture_ids",
            "artifact_paths",
        ] {
            if !task[key].is_array() {
                return Err(format!("{id} has no {key} array"));
            }
        }
        validate_fixture_ids(id, &task["fixture_ids"], &fixture_ids)?;
        validate_integration_completion(task, &expected, &completed, &acceptance)?;
        if task["release_complete"] == true && task.get("integration_dependencies").is_some() {
            for case in task["pending_acceptance"]
                .as_array()
                .ok_or("missing final acceptance")?
            {
                let fixture = case["fixture_id"].as_str().ok_or("missing final fixture")?;
                for path in case["evidence_paths"]
                    .as_array()
                    .ok_or("missing final paths")?
                {
                    let path = Path::new(path.as_str().ok_or("invalid final path")?);
                    if !path.starts_with("artifacts")
                        || path
                            .components()
                            .any(|c| !matches!(c, std::path::Component::Normal(_)))
                    {
                        return Err(format!("{fixture} has invalid artifact path"));
                    }
                    let evidence: serde_json::Value = serde_json::from_str(
                        &fs::read_to_string(root.join(path)).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                    validate_final_observation(fixture, &evidence)?;
                }
            }
        }
        if status == "complete"
            && (task["acceptance_tests"].as_array().unwrap().is_empty()
                || task["artifact_paths"].as_array().unwrap().is_empty())
        {
            return Err(format!(
                "{id} marked complete without test/evidence references"
            ));
        }
    }
    if actual != expected {
        return Err("ledger does not exactly cover specification task cards".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ledger_card_parser_excludes_prose_and_includes_macro_tasks() {
        assert_eq!(
            super::spec_task_id("**M001 — Vendor adapter.** Files: adapter"),
            Some("M001")
        );
        assert_eq!(
            super::spec_task_id("**Meaning of laziness:** cold bodies"),
            None
        );
        assert_eq!(
            super::spec_task_id("**Match.** Enter evaluates scrutinee"),
            None
        );
    }
    #[test]
    fn task_ledger_validates() {
        super::validate(&crate::root()).unwrap();
    }
    #[test]
    fn ledger_fixture_references_reject_unknown_duplicate_and_non_string_entries() {
        let known = std::collections::BTreeSet::from(["MX011".to_owned()]);
        for ids in [
            serde_json::json!(["UNKNOWN"]),
            serde_json::json!(["MX011", "MX011"]),
            serde_json::json!([17]),
        ] {
            assert!(super::validate_fixture_ids("M007", &ids, &known).is_err());
        }
        super::validate_fixture_ids("M007", &serde_json::json!(["MX011"]), &known).unwrap();
        super::validate_fixture_ids("M008", &serde_json::json!([]), &known).unwrap();
    }
    #[test]
    fn registered_runtime_fixture_still_requires_full_integration_evidence() {
        use serde_json::json;
        let known = std::collections::BTreeSet::from(["T022", "T024"]);
        let completed = known.clone();
        let acceptance = std::collections::BTreeSet::from(["T022-DEFERRED-UNSUPPORTED"]);
        let task = json!({"id":"T022","completion_scope":"component","release_complete":false,
            "integration_dependencies":["T024"],"pending_acceptance":[{"fixture_id":"T022-DEFERRED-UNSUPPORTED","status":"pending","evidence_paths":[]}]});
        super::validate_integration_completion(&task, &known, &completed, &acceptance).unwrap();
        let mut premature = task.clone();
        premature["release_complete"] = json!(true);
        assert!(
            super::validate_integration_completion(&premature, &known, &completed, &acceptance)
                .is_err()
        );
        premature["pending_acceptance"][0]["status"] = json!("passed");
        assert!(
            super::validate_integration_completion(&premature, &known, &completed, &acceptance)
                .is_err()
        );
        let mut unknown = task;
        unknown["pending_acceptance"][0]["fixture_id"] = json!("UNKNOWN");
        assert!(
            super::validate_integration_completion(&unknown, &known, &completed, &acceptance)
                .is_err()
        );
    }
    #[test]
    fn component_release_gate_rejects_pending_later_dependencies_and_unproven_fixtures() {
        use serde_json::json;
        let known = std::collections::BTreeSet::from(["M009", "T034", "M015"]);
        let acceptance = std::collections::BTreeSet::from(["MX015"]);
        let task = json!({"id":"M009","status":"complete","completion_scope":"component",
            "release_complete":false,"integration_dependencies":["T034","M015"],
            "pending_acceptance":[{"fixture_id":"MX015","status":"pending","evidence_paths":[]}]});
        let incomplete = std::collections::BTreeSet::from(["M009"]);
        super::validate_integration_completion(&task, &known, &incomplete, &acceptance).unwrap();
        for deps in [json!(["UNKNOWN"]), json!(["M009"]), json!(["T034", "T034"])] {
            let mut invalid = task.clone();
            invalid["integration_dependencies"] = deps;
            assert!(
                super::validate_integration_completion(&invalid, &known, &incomplete, &acceptance)
                    .is_err()
            );
        }
        let mut premature = task.clone();
        premature["release_complete"] = json!(true);
        assert!(
            super::validate_integration_completion(&premature, &known, &incomplete, &acceptance)
                .is_err()
        );
        assert!(
            super::validate_integration_completion(&premature, &known, &known, &acceptance)
                .is_err()
        );
        premature["pending_acceptance"][0]["status"] = json!("passed");
        assert!(
            super::validate_integration_completion(&premature, &known, &known, &acceptance)
                .is_err()
        );
        premature["pending_acceptance"][0]["evidence_paths"] =
            json!(["full-guest-observations.json"]);
        super::validate_integration_completion(&premature, &known, &known, &acceptance).unwrap();
    }
    #[test]
    fn final_integration_evidence_rejects_native_only_component_and_wrong_profile_reports() {
        use serde_json::json;
        let evidence = json!({"fixture_id":"MX015","runner":"lrr","scope":"full_lrr","status":"passed",
            "semantics_profile":"lrr-2024-v2","rust_version":"1.99.0","platform":"aarch64-apple-darwin",
            "observations":{"selected_tests":["generated_one"]}});
        super::validate_final_observation("MX015", &evidence).unwrap();
        for (field, value) in [
            ("runner", json!("native_oracle")),
            ("scope", json!("component")),
            ("semantics_profile", json!("lrr-2024-v1")),
            ("observations", json!({})),
            ("fixture_id", json!("MX016")),
            ("status", json!("pending")),
        ] {
            let mut invalid = evidence.clone();
            invalid[field] = value;
            assert!(super::validate_final_observation("MX015", &invalid).is_err());
        }
    }
    #[test]
    fn workspace_has_exact_members() {
        let doc: toml::Value =
            toml::from_str(&std::fs::read_to_string(crate::root().join("Cargo.toml")).unwrap())
                .unwrap();
        assert_eq!(
            doc["workspace"]["members"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>(),
            ["crates/lrr", "crates/lrr-expand-adapter", "xtask"]
        );
        assert_eq!(doc["workspace"]["resolver"].as_str(), Some("3"));
    }
    #[test]
    fn toolchain_is_pinned() {
        let doc: toml::Value = toml::from_str(
            &std::fs::read_to_string(crate::root().join("rust-toolchain.toml")).unwrap(),
        )
        .unwrap();
        assert_eq!(doc["toolchain"]["channel"].as_str(), Some("1.99.0"));
        assert_eq!(doc["toolchain"]["profile"].as_str(), Some("minimal"));
    }
    #[test]
    fn direct_versions_match_spec() {
        crate::direct_versions().unwrap();
    }
    #[test]
    fn vendor_patch_is_exact() {
        let output = std::process::Command::new("python3")
            .arg(crate::root().join("tools/vendor_audit.py"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }
}

#[cfg(test)]
mod license_tests {
    #[test]
    fn dependency_license_inventory_is_complete() {
        crate::license_inventory().unwrap();
    }
}

#[cfg(test)]
#[path = "tasks_fixture_tests.rs"]
mod fixture_tests;
