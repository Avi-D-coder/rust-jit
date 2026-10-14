//! Original stock constructor operands; no synthetic type/default substitutes.
use super::*;

#[test]
fn rustc_original_tuple_operand_return_enters_guard_without_inventing_field_type() {
    const SOURCE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/expr/return-in-block-tuple.rs"
    );
    let (mut db, mut run) = borrows::tests::start_stock_body(SOURCE, "main");
    let type_bytes = run.store.type_metadata_bytes();
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("unchanged stock tuple operand early return did not complete")
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert!(run.frame.active_scopes().is_empty());
    assert!(run.returning.is_none());
    assert_eq!(report.opcode_counters.started_by_opcode[27], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[28], 0);
    assert_eq!(report.opcode_counters.started_by_opcode[29], 0);
    assert_eq!(run.store.type_metadata_bytes(), type_bytes);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert!(report.opcode_counters.return_scopes_cleaned > 0);
    assert!(db.trace.events.iter().all(|event| match &event.key {
        QueryKey::BodyStructure(body) => *body == run.root_body,
        QueryKey::Thunk(pc) => pc.body == run.root_body,
        _ => true,
    }));
    db.finish_run().unwrap();
    // Whole unchanged original main executes under the Rust2024 profile.
    // Official compiletest harness/directive acceptance remains separate.
}
