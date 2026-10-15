//! Source-context/coldness regressions for the ordinary indexed match route.
//! The unchanged rustc binding/expr-match.rs remains the primary language bank.
//! These supplemental review witnesses exercise the missing preparation boundary.
use super::*;

#[test]
fn surrounding_pattern_context_keeps_unselected_nested_results_cold() {
    for cold in ["cold()", "{ let dead = cold(); dead }", "undefined()"] {
        let source = format!(
            "fn cold()->i32{{2_i32}} fn value()->i32{{match (match true{{true=>1_i32,false=>{cold}}}){{1_i32=>7_i32,_=>8_i32}}}} fn main(){{}}"
        );
        let (mut db, mut run) = borrows::tests::start_stock_body(&source, "value");
        let body = run.root_body.clone();
        // Bind the cold ranges through the actual ordinary BodyIndex, not
        // expected text output or a synthesized executable wrapper.
        let cold_nodes: std::collections::BTreeSet<_> = run
            .frame
            .body
            .nodes()
            .iter()
            .filter(|node| {
                node.role
                    == crate::syntax::body_index::NodeRole::Expression(
                        crate::syntax::body_index::ExpressionKind::Call,
                    )
            })
            .map(|node| node.id.0)
            .collect();
        assert!(!cold_nodes.is_empty());
        let RunPoll::Completed(report) = run.poll(&mut db) else {
            panic!("nested match with cold result {cold:?} did not complete")
        };
        assert!(matches!(report.value, ValueSummary::Signed(ref value) if value == "7"));
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert_eq!(run.peak_guest_frames(), 1);
        assert!(db.trace.events.iter().all(|event| match &event.key {
            QueryKey::Thunk(pc) if pc.body == body => !cold_nodes.contains(&pc.node),
            QueryKey::BodyStructure(other) => *other == body,
            _ => true,
        }));
        assert_eq!(
            report.opcode_counters.started_by_opcode[22], 0,
            "no cold CallPrepare"
        );
        drop(run);
        db.finish_run().unwrap();
    }
}

#[test]
fn nested_entry_preserves_the_surrounding_constraint_without_preparing_a_result() {
    let source = "fn value()->i32{match (match true{true=>1_u8,false=>2_u8}) {1_i32=>7_i32,_=>8_i32}} fn main(){}";
    let (mut db, run) = borrows::tests::start_stock_body(source, "value");
    let outer = run
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| {
            node.role
                == crate::syntax::body_index::NodeRole::Expression(
                    crate::syntax::body_index::ExpressionKind::Match,
                )
        })
        .unwrap();
    let pc = run.frame.body.pc(outer.id, Phase::Entry).unwrap();
    let trace_start = db.trace.events.len();
    let outcome = db
        .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("match entry absent")
    };
    let source_slot = run.frame.body.nodes()[outer.operands[0].0 as usize]
        .value
        .unwrap();
    assert!(recipe.recipe().operations.iter().any(|op| matches!(op,
        Op::SetHint {
            child,
            hint: crate::ir::op::HintRecipe::Primitive(crate::ir::op::PrimitiveType::I32),
        } if *child == source_slot
    )));
    assert!(!recipe.recipe().operations.iter().any(|op| matches!(
        op,
        Op::Literal { .. }
            | Op::ReadLocal { .. }
            | Op::ReadParameter { .. }
            | Op::CallPrepare { .. }
    )));
    assert!(
        db.trace.events[trace_start..]
            .iter()
            .all(|event| match &event.key {
                QueryKey::Thunk(actual) => actual == &pc,
                _ => true,
            })
    );
    assert_eq!(run.frame.counters.operations_started, 0);
    // This is a source-constraint/coldness control, not acceptance of the
    // inconsistent program or its separate compiler diagnostic contract.
    drop(run);
    db.finish_run().unwrap();
}
