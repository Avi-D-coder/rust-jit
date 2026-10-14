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
    assert_eq!(report.opcode_counters.started_by_opcode[0], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[7], 2);
    assert_eq!(report.opcode_counters.started_by_opcode[31], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[26], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[32], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[33], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[8], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[38], 0);
    assert_eq!(report.opcode_counters.return_scopes_cleaned, 1);
}
