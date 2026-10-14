//! Whole unchanged assignment-unit program execution. Rust2024 adaptation is
//! explicit; native compiletest/LLVM lowering are separate acceptance scopes.
use super::*;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/codegen/assign-expr-unit-type.rs"
);
#[test]
fn rustc_original_assignment_unit_program_executes_ordinary_and_compound_assertions() {
    let (assertions, calls, compounds) =
        crate::syntax::scope_fixture_facts::original_assignment_unit_program_counts(ORIGINAL);
    let (mut db, mut run) = borrows::tests::start_stock_body(ORIGINAL, "main");
    run.frame.set_fuel_limit(1_000_000).unwrap();
    let view = run.sources[0].share();
    let macros = crate::engine::standalone_macros::StandaloneRuntimeMacros::new(&view).unwrap();
    let contexts = [macros.context(&view).unwrap()];
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
    assert_eq!(report.opcode_counters.started_by_opcode[26], assertions);
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
