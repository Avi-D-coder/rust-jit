#[test]
fn jit_does_not_resolve_ambient_symbol() {
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("--ambient-import-probe")
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("can't resolve symbol getpid"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
