//! Unchanged stock expressions; auxiliary entry is a component observation.
use super::*;

fn observe_original(source: &str, function: &str) {
    let crate::syntax::discarded_numeric_fixture_facts::Facts {
        source_values,
        expected_binary,
        sum,
    } = crate::syntax::discarded_numeric_fixture_facts::capture(source, function).unwrap();
    let literals = source_values.len();
    // The syntax adapter checked the original operator is addition; the
    // mathematical oracle uses written literals without hard-coding a result.
    let (mut db, mut run) = borrows::tests::start_stock_body(source, function);
    let expected =
        crate::runtime::shape::PrimitiveIdentity::from_recipe(crate::ir::op::PrimitiveType::I32)
            .unwrap()
            .id();
    let mut literal_results = 0usize;
    let mut binary_results = 0usize;
    for _ in 0..1_000 {
        let outcome = borrows::tests::next(&mut db, &mut run);
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("stock expression thunk absent");
        };
        let results: Vec<_> = recipe
            .recipe()
            .operations
            .iter()
            .filter_map(|op| match op {
                Op::Literal { destination, .. } => Some((*destination, false)),
                Op::NumericBinary { destination, .. } => Some((*destination, true)),
                _ => None,
            })
            .collect();
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        for (destination, binary) in results {
            let crate::runtime::cell::CellState::Live(value) = run
                .store
                .state(run.frame.cell(destination).unwrap())
                .unwrap()
            else {
                panic!("stock numeric result is not live");
            };
            assert_eq!(value.type_id, expected.0);
            assert_eq!(value.kind, crate::runtime::value::ValueKind::Signed.code());
            if binary {
                assert_eq!(value.payload, sum);
                binary_results += 1;
            } else {
                assert_eq!(value.payload, source_values[literal_results]);
                literal_results += 1;
            }
        }
        if literal_results == literals && binary_results == expected_binary {
            break;
        }
    }
    assert_eq!(literal_results, literals);
    assert_eq!(binary_results, expected_binary);
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("original discarded expression body did not complete");
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(
        report.opcode_counters.started_by_opcode[0],
        u64::try_from(literals).unwrap()
    );
    assert_eq!(
        report.opcode_counters.started_by_opcode[10],
        u64::try_from(expected_binary).unwrap()
    );
    assert_eq!(report.opcode_counters.started_by_opcode[9], 1);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert!(run.frame.active_scopes().is_empty());
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
    db.finish_run().unwrap();
}

#[test]
fn rustc_original_doc_comment_program_materializes_discarded_integer_fallback() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/parser/doc-comment-parsing.rs"
    );
    observe_original(ORIGINAL, "main");
    // Whole original body, without full compiletest/edition acceptance.
}

#[test]
fn rustc_original_reexport_auxiliary_arithmetic_statement_uses_integer_fallback() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/cross-crate/auxiliary/aux-9906.rs"
    );
    observe_original(ORIGINAL, "foo");
    // Selected original nested auxiliary function. Parent crate/re-export/
    // construction/linking observations remain unexecuted, not fixture passes.
}
