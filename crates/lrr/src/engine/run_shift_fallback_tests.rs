//! Unchanged rustc shift.rs, with selected-function entry disclosed.
//! The original static declarations in test_const are not exercised here.
use super::*;

const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/shift.rs"
);

#[test]
fn rustc_original_chained_shift_function_executes_its_original_assertion() {
    let (mut db, mut run) = borrows::tests::start_stock_body(ORIGINAL, "test_misc");
    let view = run.sources[0].share();
    let macros = crate::engine::standalone_macros::StandaloneRuntimeMacros::new(&view).unwrap();
    let contexts = [macros.context(&view).unwrap()];
    let RunPoll::Completed(report) = run.poll_with_runtime_macros(&mut db, &contexts) else {
        panic!("original test_misc did not complete");
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
    assert_eq!(report.opcode_counters.started_by_opcode[26], assertions);
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
    let (mut db, mut run) = borrows::tests::start_stock_body(ORIGINAL, "test_expr");
    run.frame.set_fuel_limit(100_000).unwrap();
    let view = run.sources[0].share();
    let macros = crate::engine::standalone_macros::StandaloneRuntimeMacros::new(&view).unwrap();
    let contexts = [macros.context(&view).unwrap()];
    let RunPoll::Completed(report) = run.poll_with_runtime_macros(&mut db, &contexts) else {
        panic!("original test_expr did not complete");
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
    assert_eq!(report.opcode_counters.started_by_opcode[26], assertions);
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
