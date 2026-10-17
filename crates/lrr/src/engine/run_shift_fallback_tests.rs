//! Unchanged rustc shift.rs, with selected-function entry disclosed.
//! The original static declarations in test_const are not exercised here.
use super::*;

const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/shift.rs"
);

#[test]
fn rustc_original_chained_shift_function_executes_its_original_assertion() {
    let (mut db, mut run, macros) =
        borrows::tests::start_discovered_body(ORIGINAL, "test_misc", 2024);
    let view = run.sources[0].share();
    let contexts = macros.contexts().unwrap();
    let report = match run.poll_with_runtime_macros(&mut db, &contexts) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("original test_misc faulted: {error:?}"),
        RunPoll::Panicked(report) => panic!("original test_misc panicked: {report:?}"),
        RunPoll::NeedInput(input) => panic!("original test_misc suspended: {input:?}"),
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    let handle = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "test_misc")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let body = &ORIGINAL[handle.range.start as usize..handle.range.end as usize];
    let assertions = u64::try_from(body.matches("assert_eq!(").count()).unwrap();
    let shifts = u64::try_from(body.matches("<<").count()).unwrap();
    // Each expanded `assert_eq!` compares with one ordinary binary operation.
    assert_eq!(
        report.opcode_counters.started_by_opcode[10],
        assertions + shifts
    );
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
    db.finish_run().unwrap();
}

#[test]
fn rustc_original_shift_expression_function_keeps_written_count_types() {
    let (mut db, mut run, macros) =
        borrows::tests::start_discovered_body(ORIGINAL, "test_expr", 2024);
    let view = run.sources[0].share();
    let contexts = macros.contexts().unwrap();
    let report = match run.poll_with_runtime_macros(&mut db, &contexts) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("original test_expr faulted: {error:?}"),
        RunPoll::Panicked(report) => panic!("original test_expr panicked: {report:?}"),
        RunPoll::NeedInput(input) => panic!("original test_expr suspended: {input:?}"),
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    let handle = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "test_expr")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let body = &ORIGINAL[handle.range.start as usize..handle.range.end as usize];
    let assertions = u64::try_from(body.matches("assert_eq!(").count()).unwrap();
    let shifts = u64::try_from(body.matches("<<").count() + body.matches(">>").count()).unwrap();
    // Each expanded `assert_eq!` compares with one ordinary binary operation.
    assert_eq!(
        report.opcode_counters.started_by_opcode[10],
        assertions + shifts
    );
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
    db.finish_run().unwrap();
}
