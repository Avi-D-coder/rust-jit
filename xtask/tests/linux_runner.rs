use std::{path::Path, process::Command};

#[test]
fn linux_runner_requires_authority_before_docker_or_evidence_creation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let evidence = root.join(format!(
        "artifacts/linux-bootstrap/permission-negative-{}",
        std::process::id()
    ));
    assert!(!evidence.exists());
    let result = Command::new("python3")
        .arg(root.join("tools/linux-bootstrap.py"))
        .arg("--evidence-dir")
        .arg(&evidence)
        .current_dir(root)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("--allow-build-tools is required before Docker or compiler invocation")
    );
    assert!(!evidence.exists());
}
