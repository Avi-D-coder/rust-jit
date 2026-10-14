//! Selected fixture success must be observed, not inferred from Cargo's status.
pub fn unpassed<'a>(names: &'a [String], stdout: &str) -> Vec<&'a str> {
    let passed: Vec<_> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("test ")?.strip_suffix(" ... ok"))
        .collect();
    names
        .iter()
        .filter(|name| {
            !passed.iter().any(|actual| {
                *actual == name.as_str()
                    || actual
                        .strip_suffix(name.as_str())
                        .is_some_and(|prefix| prefix.ends_with("::"))
            })
        })
        .map(String::as_str)
        .collect()
}

/// Gate02 requires every registered fixture from the first execution milestone.
/// A successful process status alone is insufficient evidence.
pub fn scalar_execution(fixtures: &toml::Value, stdout: &str) -> Result<serde_json::Value, String> {
    let names = task_execution(
        fixtures,
        stdout,
        &["T020", "T021", "T022", "T023", "T024", "T025"],
    )?;
    Ok(
        serde_json::json!({"passed_tests": names, "scope": "original-source scalar component; generated/trait/JIT integrations remain pending"}),
    )
}
/// Section57 provisional acceptance only. No pending behavior is a passed test.
pub fn control_flow_calls(
    fixtures: &toml::Value,
    stdout: &str,
) -> Result<serde_json::Value, String> {
    let names = task_execution(fixtures, stdout, &["T030", "T031", "T032", "T033", "T034"])?;
    Ok(serde_json::json!({
        "passed_tests": names, "completion_scope": "component", "release_complete": false,
        "full_acceptance_passed": false,
        "scope": "original-source scalar control/call/test components; Section57 later integration remains mandatory",
        "pending_requirements": [
            {"requirement": "ordered broker output and destructor transcripts", "dependencies": ["T060","T061","T063","T065"], "status": "pending"},
            {"requirement": "generated operands, complete discovery and origin mapping", "dependencies": ["M013","M014","M015","M016"], "status": "pending"},
            {"requirement": "actual auto/force tier equivalence and native observations", "dependencies": ["T080","T084","M035","M037"], "status": "pending"},
            {"requirement": "complete Section28 and revision4 fixture matrix", "dependencies": ["M043","T104"], "status": "pending"}
        ]
    }))
}
fn task_execution(
    fixtures: &toml::Value,
    stdout: &str,
    tasks: &[&str],
) -> Result<Vec<String>, String> {
    let inventory = fixtures["fixture"]
        .as_array()
        .ok_or("missing fixture inventory")?;
    let mut names = Vec::new();
    for task in tasks {
        let prefix = format!("{task}-");
        let selected: Vec<_> = inventory
            .iter()
            .filter(|fixture| {
                fixture["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with(&prefix))
            })
            .collect();
        if selected.is_empty() {
            return Err(format!("missing {task} execution acceptance inventory"));
        }
        for fixture in selected {
            names.push(
                fixture["test"]
                    .as_str()
                    .ok_or("execution fixture without test name")?
                    .to_owned(),
            );
        }
    }
    names.sort();
    names.dedup();
    let missing = unpassed(&names, stdout);
    if !missing.is_empty() {
        return Err(format!("execution fixtures did not pass: {missing:?}"));
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn scalar_gate_requires_each_task_inventory_and_each_actual_pass() {
        let text: String = (20..=25)
            .map(|task| {
                format!("[[fixture]]\nid = \"T0{task}-CONTROL\"\ntest = \"control_{task}\"\n")
            })
            .collect();
        let inventory: toml::Value = toml::from_str(&text).unwrap();
        let stdout: String = (20..=25)
            .map(|task| format!("test control_{task} ... ok\n"))
            .collect();
        assert!(scalar_execution(&inventory, &stdout).is_ok());
        assert!(
            scalar_execution(
                &inventory,
                &stdout.replace("control_25 ... ok", "control_25 ... ignored")
            )
            .is_err()
        );
        let empty: toml::Value = toml::from_str("fixture = []").unwrap();
        assert!(scalar_execution(&empty, &stdout).is_err());
    }

    #[test]
    fn control_call_gate_requires_each_actual_task_pass_and_retains_full_acceptance_pending() {
        let inventory: toml::Value = toml::from_str(
            &(30..=34)
                .map(|task| {
                    format!("[[fixture]]\nid = \"T0{task}-CONTROL\"\ntest = \"control_{task}\"\n")
                })
                .collect::<String>(),
        )
        .unwrap();
        let stdout = (30..=34)
            .map(|task| format!("test control_{task} ... ok\n"))
            .collect::<String>();
        let report = control_flow_calls(&inventory, &stdout).unwrap();
        assert_eq!(report["completion_scope"], "component");
        assert_eq!(report["release_complete"], false);
        assert_eq!(report["full_acceptance_passed"], false);
        assert_eq!(report["passed_tests"].as_array().unwrap().len(), 5);
        assert!(
            report["pending_requirements"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["status"] == "pending")
        );
        for task in 30..=34 {
            assert!(
                control_flow_calls(
                    &inventory,
                    &stdout.replace(
                        &format!("control_{task} ... ok"),
                        &format!("control_{task} ... ignored")
                    )
                )
                .is_err()
            );
        }
        let empty: toml::Value = toml::from_str("fixture=[]").unwrap();
        assert!(control_flow_calls(&empty, &stdout).is_err());
    }

    #[test]
    fn successful_cargo_status_cannot_hide_a_disabled_selected_fixture() {
        let directory = std::env::temp_dir().join(format!(
            "lrr-gate-ignored-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        fs::create_dir(directory.join("src")).unwrap();
        fs::write(directory.join("Cargo.toml"),
            "[package]\nname = \"lrr_gate_ignored_fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n").unwrap();
        fs::write(
            directory.join("src/lib.rs"),
            "#[test]\nfn enabled_witness() {}\n#[test]\n#[ignore]\nfn selected_but_disabled() {}\n",
        )
        .unwrap();
        let result = Command::new("cargo")
            .args(["+1.99.0", "test", "--offline"])
            .current_dir(&directory)
            .env("CARGO_TARGET_DIR", directory.join("target"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let stdout = String::from_utf8(result.stdout).unwrap();
        assert!(stdout.contains("selected_but_disabled ... ignored"));
        let names = vec![
            "enabled_witness".into(),
            "selected_but_disabled".into(),
            "missing_fixture".into(),
        ];
        assert_eq!(
            unpassed(&names, &stdout),
            ["selected_but_disabled", "missing_fixture"]
        );
    }
}
