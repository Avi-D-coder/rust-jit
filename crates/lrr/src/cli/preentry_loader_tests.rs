//! Registered pre-entry API controls. No interpreter/main execution occurs here.
use super::preentry_loader::Packet;
use crate::core::digest::Digest;
use std::path::PathBuf;
fn observe(path: &std::path::Path, edition: u16) -> serde_json::Value {
    serde_json::from_slice(
        &Packet::observe(path, edition, Digest([7; 32]))
            .render()
            .unwrap(),
    )
    .unwrap()
}
#[test]
fn unchanged_original_main_metadata_uses_actual_capture_and_index() {
    let root = std::env::var_os("LRR_PREENTRY_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout")
        });
    for case in [
        "tests/ui/codegen/assign-expr-unit-type.rs",
        "tests/ui/numbers-arithmetic/floatlits.rs",
        "tests/ui/numbers-arithmetic/i8-incr.rs",
        "tests/ui/cast/u8-to-char-cast-9918.rs",
        "tests/ui/cast/cast-does-fallback.rs",
        "tests/ui/numeric/type-limit-comparisons-6130.rs",
    ] {
        let path = root.join(case);
        let bytes = std::fs::read(&path).unwrap();
        let packet = observe(&path, 2015);
        assert_eq!(packet["status"], "ready", "{case}: {packet}");
        assert_eq!(packet["snapshot"]["owning_edition"], 2015);
        assert_eq!(packet["entry_body"], packet["entry_selection"]["body"]);
        assert_eq!(packet["entry_body"], packet["structural_index"]["body"]);
        assert!(
            packet["snapshot"]["files"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["content"] == serde_json::to_value(Digest::of_bytes(&bytes)).unwrap())
        );
        assert_eq!(packet["guest_executions"], 0);
        assert_eq!(packet["vm_dispatches"], 0);
        assert_eq!(packet["executable_thunks"], 0);
        assert_eq!(packet["compiler_tool_invocations"], 0);
        assert_eq!(packet["procedural_macro_executions"], 0);
        assert!(
            !packet["discovery"]["current_input_stamps"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(packet["structural_index"]["nodes"].as_u64().unwrap() > 0);
    }
}
fn temporary_source(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lrr-preentry-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("root.rs");
    std::fs::write(&path, text).unwrap();
    path
}
#[test]
fn structural_metadata_never_runs_panic_arithmetic_or_managed_output() {
    for (name, text) in [
        ("panic", "fn main(){panic!(\"metadata must not run\");}"),
        ("division", "fn main(){let x=1/0;assert_eq!(x,7);}"),
        (
            "output",
            "fn main(){println!(\"must not be guest output\");}",
        ),
        ("cold", "fn cold(){panic!(\"cold\");} fn main(){}"),
    ] {
        let path = temporary_source(name, text);
        let packet = observe(&path, 2015);
        assert_eq!(packet["guest_executions"], 0);
        assert_eq!(packet["vm_dispatches"], 0);
        assert_eq!(packet["compiler_tool_invocations"], 0);
        assert_eq!(packet["status"], "ready", "{name}: {packet}");
        assert_ne!(packet["schema"], "lrr/v5/standalone-runtime-capture/v1");
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
#[test]
fn missing_ambiguous_or_generated_main_is_typed_frontier() {
    for (name, text) in [
        ("missing", "fn other(){}"),
        ("duplicate", "fn main(){} fn main(){}"),
        (
            "generated",
            "macro_rules! make {()=>{fn main(){}}} make!();",
        ),
    ] {
        let path = temporary_source(name, text);
        let packet = observe(&path, 2015);
        assert_eq!(packet["status"], "frontier", "{packet}");
        assert!(packet["frontier"]["code"].as_str().is_some());
        assert!(packet["entry_body"].is_null());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
#[test]
fn metadata_context_flags_are_strict_and_separate_from_run() {
    use clap::Parser;
    let context = "07".repeat(32);
    assert!(
        super::args::Args::try_parse_from([
            "lrr",
            "source-world",
            "root.rs",
            "--context",
            &context
        ])
        .is_ok()
    );
    assert!(
        super::args::Args::try_parse_from(["lrr", "run", "root.rs", "--context", &context])
            .is_err()
    );
    for value in ["A".repeat(64), "f".repeat(63), "g".repeat(64)] {
        assert!(
            super::args::Args::try_parse_from([
                "lrr",
                "source-world",
                "root.rs",
                "--context",
                &value
            ])
            .is_err()
        );
    }
}
