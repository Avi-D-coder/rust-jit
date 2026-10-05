use std::{collections::BTreeSet, fs, path::Path};

pub fn validate(root: &Path) -> Result<(), String> {
    let ledger: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("docs/tasks.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let spec = fs::read_to_string(root.join("lrr_rust2024_hybrid_cargo_implementation_spec.md"))
        .map_err(|e| e.to_string())?;
    let expected: BTreeSet<_> = spec
        .lines()
        .filter(|line| line.starts_with("**T") || line.starts_with("**H"))
        .filter_map(|line| line.get(2..6))
        .collect();
    let tasks = ledger["tasks"].as_array().ok_or("missing tasks array")?;
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
    fn task_ledger_validates() {
        super::validate(&crate::root()).unwrap();
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
            ["crates/lrr", "xtask"]
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
        let status = std::process::Command::new("python3")
            .arg(crate::root().join("tools/vendor_audit.py"))
            .status()
            .unwrap();
        assert!(status.success());
    }
}

#[cfg(test)]
mod license_tests {
    #[test]
    fn dependency_license_inventory_is_complete() {
        crate::license_inventory().unwrap();
    }
}
