use std::{
    fs,
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cargo_native_reuse_and_live_edit_isolation_are_observed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let work = root.join(format!(
        "artifacts/probe/host-test-{}-{nonce}",
        std::process::id()
    ));
    let output = Command::new("python3")
        .arg(root.join("tools/cargo-reuse-probe/probe.py"))
        .args(["--allow-native-build", "--trace-fingerprints", "--work-dir"])
        .arg(&work)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(work.join("evidence/report.json")).unwrap()).unwrap();
    assert_eq!(report["passed"], true);
    assert!(
        report["toolchain"]["rustc"]
            .as_str()
            .unwrap()
            .contains("release: 1.99.0\n")
    );
    let stages = report["stages"].as_array().unwrap();
    let live = stages.iter().find(|s| s["name"] == "live-edit").unwrap();
    assert_eq!(live["cargo_invocations"], 0);
    assert_eq!(live["live_code_executed"], false);
    let native_edit = stages
        .iter()
        .find(|s| s["name"] == "native-edit-load")
        .unwrap();
    assert_eq!(
        native_edit["native_output"],
        serde_json::json!({"scale":24,"twice":14})
    );
}
