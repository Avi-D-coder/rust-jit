use std::{fs, path::Path, process::Command};

/// Whole-source interpreter evidence remains distinct from compiletest/JIT acceptance.
#[test]
fn unchanged_rustc_empty_derive_runs_through_cli_with_native_metadata_reference() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let output = Command::new("python3")
        .arg(root.join("tools/rustc_checkout_interpreter.py"))
        .arg("--lrr")
        .arg(env!("CARGO_BIN_EXE_lrr"))
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "upstream CLI/reference comparison failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let path = String::from_utf8(output.stdout).unwrap();
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(path.trim())).unwrap()).unwrap();
    let selected = serde_json::json!(["tests/ui/derives/deriving-meta-empty-trait-list.rs"]);
    assert_eq!(report["native_check_cases_passed"], selected);
    assert_eq!(report["interpreter_cases_passed"], selected);
    assert_eq!(report["selected_scope_passed"], true);
    assert_eq!(report["guest_jit_acceptance"], false);
    assert_eq!(report["lrr_full_cases_passed"], serde_json::json!([]));
    assert_eq!(
        report["full_compiletest_cases_passed"],
        serde_json::json!([])
    );
}
