//! Complete unchanged rustc float-literal body; directives remain separate.
use super::*;

#[test]
fn rustc_original_float_literal_wildcards_apply_written_type_without_binding() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/parser/float-literals.rs"
    );
    let (mut db, mut run) = borrows::tests::start_stock(ORIGINAL);
    let initializers = u64::try_from(ORIGINAL.matches("let _: f64 =").count()).unwrap();
    let expected =
        crate::runtime::shape::PrimitiveIdentity::from_recipe(crate::ir::op::PrimitiveType::F64)
            .unwrap()
            .id();
    let mut observed = 0;
    // Observe actual original-source literal recipes before statement cleanup.
    // Completion alone would not prove an unused literal was materialized.
    for _ in 0..1_000 {
        let outcome = borrows::tests::next(&mut db, &mut run);
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("original wildcard thunk absent");
        };
        let literal = recipe.recipe().operations.iter().find_map(|op| match op {
            Op::Literal { destination, .. } => Some(*destination),
            _ => None,
        });
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        if let Some(destination) = literal {
            let crate::runtime::cell::CellState::Live(value) = run
                .store
                .state(run.frame.cell(destination).unwrap())
                .unwrap()
            else {
                panic!("original literal result is not live");
            };
            assert_eq!(value.type_id, expected.0);
            assert_eq!(value.kind, crate::runtime::value::ValueKind::F64.code());
            observed += 1;
            if observed == initializers {
                break;
            }
        }
    }
    assert_eq!(observed, initializers);
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("unchanged float-literals.rs did not complete");
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.opcode_counters.started_by_opcode[0], initializers);
    assert_eq!(report.opcode_counters.started_by_opcode[9], initializers);
    assert_eq!(report.opcode_counters.started_by_opcode[7], 0);
    assert_eq!(report.opcode_counters.started_by_opcode[14], 0);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert!(run.frame.active_scopes().is_empty());
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
    db.finish_run().unwrap();
    // build-pass compiletest/lint/edition acceptance is not established here.
}
