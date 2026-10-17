//! Actual unchanged original main plus separate LRR reached/cold flow controls.
use crate::{
    db::{key::QueryKey, memo::Database},
    engine::run::{RunMachine, RunPoll, ValueSummary},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/match/match-on-negative-integer-ranges.rs"
);
fn run(text: &str, edition: u16) -> (Database, RunMachine, RunPoll) {
    // The production route: sealed discovery expands the library's own
    // `assert_eq!`, and the retained entry body runs the expanded source.
    let (mut db, mut machine, macros) =
        crate::engine::run::start_discovered_body(text, "main", edition);
    let contexts = macros.contexts().unwrap();
    let result = machine.poll_with_runtime_macros(&mut db, &contexts);
    drop(contexts);
    db.finish_run().unwrap();
    (db, machine, result)
}
fn original_main(edition: u16) {
    let (db, _machine, outcome) = run(ORIGINAL, edition);
    let report = match outcome {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("edition {edition} actual original fault: {error}"),
        RunPoll::NeedInput(input) => {
            panic!("edition {edition} actual original needs {input:?}")
        }
        RunPoll::Panicked(report) => {
            panic!("edition {edition} actual original panic: {report:?}")
        }
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.live_edition, edition);
    // The library's own `assert_eq!` compares with one ordinary equality per
    // assertion; the range bounds are tested by the shared pattern engine.
    assert_eq!(report.opcode_counters.started_by_opcode[10], 2);
    assert_eq!(report.opcode_counters.started_by_opcode[25], 0);
    assert_eq!(report.opcode_counters.started_by_opcode[19], 0);
    assert_eq!(report.opcode_counters.started_by_opcode[22], 0);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    let bodies = db
        .trace
        .events
        .iter()
        .filter_map(|e| match &e.key {
            QueryKey::BodyStructure(body) => Some(body),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        bodies.len(),
        1,
        "only the unchanged original main may execute"
    );
}
#[test]
fn unchanged_negative_integer_range_entire_main_edition_2015() {
    original_main(2015);
}
#[test]
fn unchanged_negative_integer_range_entire_main_edition_2018() {
    original_main(2018);
}
#[test]
fn unchanged_negative_integer_range_entire_main_edition_2021() {
    original_main(2021);
}
#[test]
fn unchanged_negative_integer_range_entire_main_edition_2024() {
    original_main(2024);
}
#[test]
fn selected_scalar_control_result_keeps_unknown_future_arm_and_upper_cold() {
    for text in [
        "fn main(){assert_eq!(true,match 1_i32{1_i32=>true,_=>loop{}});}",
        "fn main(){assert_eq!(false,match 9_i32{20_i32..='x'=>true,_=>false});}",
        "fn main(){assert_eq!(false,match 9_i32{20_i32..=340282366920938463463374607431768211456i32=>true,_=>false});}",
        "fn main(){assert_eq!(false,match 9_i32{20_i32..=9.0bad=>true,_=>false});}",
        "fn main(){assert_eq!(true,match 1_i32{1_i32=>true,_=>340282366920938463463374607431768211456i32});}",
        "fn main(){assert_eq!(false,match 9_i32{20_i32..=UNKNOWN=>true,_=>false});}",
        "fn main(){assert_eq!(true,if let 0_i32..=10_i32=5_i32{true}else{loop{}});}",
    ] {
        let (_, _, outcome) = run(text, 2024);
        let RunPoll::Completed(report) = outcome else {
            panic!("unreached unsupported scalar subtree was prepared/executed")
        };
        assert!(matches!(report.value, ValueSummary::Unit));
    }
}

/// LRR reached-interpreter controls, deliberately outside upstream. Nonexhaustive
/// patterns are not rustc run-pass fixtures or a static exhaustiveness claim.
#[test]
fn exhausted_opaque_match_faults_after_its_actual_scrutinee_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "fn main(){assert_eq!(true,match false{true=>true});}",
            "fn main(){assert_eq!(true,match 2_i32{1_i32=>true});}",
        ] {
            let (_, machine, outcome) = run(text, edition);
            let RunPoll::Fault(error) = outcome else {
                panic!("actual source match did not exhaust")
            };
            assert_eq!(error.code, "LRR-E-MATCH-NO-ARM");
            assert_eq!(error.message, "no match arm accepted the reached scrutinee");
            let counters = machine.opcode_counters().unwrap();
            assert_eq!(counters.operations_started, counters.operations_completed);
        }
    }
}
#[test]
fn exhaustive_opaque_bool_matches_and_an_unselected_exhaustion_complete_normally() {
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "fn main(){assert_eq!(true,match false{true=>false,false=>true});}",
            "fn main(){assert_eq!(true,match true{true=>true,false=>match false{true=>false}});}",
        ] {
            let (_, _, outcome) = run(text, edition);
            let RunPoll::Completed(report) = outcome else {
                panic!("matched source arm reached exhaustion")
            };
            assert!(matches!(report.value, ValueSummary::Unit));
        }
    }
}
#[test]
fn opaque_match_scrutinee_call_or_reached_fault_is_not_replayed_or_replaced_by_exhaustion() {
    let (_, machine, outcome) = run(
        "fn value()->bool{false}fn main(){assert_eq!(true,match value(){true=>true});}",
        2024,
    );
    let RunPoll::Fault(error) = outcome else {
        panic!("actual call scrutinee did not exhaust")
    };
    assert_eq!(error.code, "LRR-E-MATCH-NO-ARM");
    let counters = machine.opcode_counters().unwrap();
    // The scrutinee call is entered exactly once.
    assert_eq!(counters.started_by_opcode[19], 1);
    // (A `match loop {}` scrutinee now loops forever, as in Rust; it is not
    // a reached refusal of a separate assertion operand route.)
    let (_, _, outcome) = run(
        "fn value()->bool{panic!(\"earlier\")}fn main(){assert_eq!(true,match value(){true=>true});}",
        2024,
    );
    let RunPoll::Panicked(report) = outcome else {
        panic!("actual scrutinee panic was replaced by later exhaustion")
    };
    assert_eq!(report.panic.description(), "earlier");
}

