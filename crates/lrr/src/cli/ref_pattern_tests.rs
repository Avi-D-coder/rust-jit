//! Whole unchanged stock program under the explicit standalone Rust2024 profile.
use super::*;
const ORIGINAL: &[u8] = include_bytes!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/let-destruct-ref.rs"
);
#[test]
fn rustc_let_destruct_ref_whole_program_executes_original_dereference_assertion() {
    let path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/upstream/rustc-checkout/tests/ui/binding/let-destruct-ref.rs"
    ));
    assert_eq!(std::fs::read(path).unwrap(), ORIGINAL);
    let StandaloneOutcome::Completed(report) = standalone(path, None).unwrap() else {
        panic!("original ref-binding run-pass program did not complete")
    };
    assert_eq!(report.status, "completed");
    assert!(matches!(
        report.value,
        crate::engine::run::ValueSummary::Unit
    ));
    // The library's own `assert_eq!` expands through the ordinary pipeline, so
    // the legacy Assert opcode and its fixed operation counts no longer apply;
    // a failed dereference comparison would panic instead of completing.
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
}
