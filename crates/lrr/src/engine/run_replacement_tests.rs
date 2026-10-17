//! Whole unchanged assignment-unit program execution. Rust2024 adaptation is
//! explicit; native compiletest/LLVM lowering are separate acceptance scopes.
use super::*;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/codegen/assign-expr-unit-type.rs"
);
#[test]
fn rustc_original_assignment_unit_program_executes_ordinary_and_compound_assertions() {
    let (_assertions, calls, compounds) =
        crate::syntax::scope_fixture_facts::original_assignment_unit_program_counts(ORIGINAL);
    // The library's own `assert_eq!` expands through the ordinary pipeline;
    // its comparisons are ordinary operations, not a legacy Assert opcode.
    let (mut db, mut run, macros) = borrows::tests::start_discovered_body(ORIGINAL, "main", 2024);
    let contexts = macros.contexts().unwrap();
    let report = match run.poll_with_runtime_macros(&mut db, &contexts) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("unchanged original assignment program failed: {error:?}"),
        RunPoll::NeedInput(_) => {
            panic!("unchanged original assignment program requested missing input")
        }
        RunPoll::Panicked(_) => {
            panic!("unchanged original assignment program unexpectedly panicked")
        }
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.opcode_counters.started_by_opcode[40], compounds);
    assert_eq!(run.action_snapshot().published, calls);
    assert_eq!(run.action_snapshot().consumed, calls);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert!(run.calls.is_empty() && run.returning.is_none());
    assert!(run.frame.active_scopes().is_empty());
    assert!(run.terminal);
    db.finish_run().unwrap();
}
