//! Actual unchanged original main plus separate LRR reached/cold flow controls.
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
    },
    engine::{
        run::{RunMachine, RunPoll, ValueSummary},
        standalone_macros::CompletedStandaloneRuntimeMacros,
    },
    expand::{
        source_declarative,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/match/match-on-negative-integer-ranges.rs"
);
fn run(text: &str, edition: u16) -> (Database, RunMachine, RunPoll) {
    run_with_fuel(text, edition, 100_000)
}
fn run_with_fuel(text: &str, edition: u16, fuel: u64) -> (Database, RunMachine, RunPoll) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("tests/ui/match/match-on-negative-integer-ranges.rs").unwrap();
    let unit = Digest::of_bytes(path.as_str().as_bytes());
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let prepared = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile: profile.clone(),
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(source) = prepared.select(&selection).unwrap() else {
        panic!()
    };
    let main = source
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let mut db = Database::default();
    db.trace.detailed = true;
    prepared.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Value,
        Digest::of_bytes(b"LRR scalar control original world"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR scalar control declarative world"),
        source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&profile).unwrap()),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            _ => panic!("actual original assertion preparation remains incomplete"),
        }
    }
    let macros = CompletedStandaloneRuntimeMacros::from_discovery(&source, &discovery).unwrap();
    let contexts = macros.contexts().unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut machine)) =
        RunMachine::start_with_discovery(&mut db, main, fuel, discovery)
    else {
        panic!("actual original startup remains blocked")
    };
    let result = machine.poll_with_runtime_macros(&mut db, &contexts);
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
    assert_eq!(report.opcode_counters.started_by_opcode[26], 2);
    // Two reached bounds + the original equality for each of the two assertions.
    assert_eq!(report.opcode_counters.started_by_opcode[10], 6);
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
        assert_eq!(report.opcode_counters.started_by_opcode[26], 1);
        assert!(matches!(report.value, ValueSummary::Unit));
    }
}
#[test]
fn selected_unknown_scalar_result_remains_the_actual_reached_unsupported_fault() {
    let (_, _, outcome) = run(
        "fn main(){assert_eq!(true,if let 0_i32..=10_i32=50_i32{true}else{loop{}});}",
        2024,
    );
    let RunPoll::Fault(error) = outcome else {
        panic!("selected unsupported result was skipped")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert_eq!(
        error.message,
        "reached assertion operand requires pending non-scalar expression integration"
    );
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
            assert_eq!(
                counters.started_by_opcode[0], 3,
                "first operand/scrutinee/endpoint exactly once"
            );
            assert_eq!(counters.started_by_opcode[10], 1);
            assert_eq!(
                counters.started_by_opcode[26], 0,
                "assertion must stay unentered"
            );
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
            assert_eq!(report.opcode_counters.started_by_opcode[26], 1);
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
    assert_eq!(counters.started_by_opcode[19], 1);
    assert_eq!(counters.started_by_opcode[22], 1);
    assert_eq!(counters.started_by_opcode[26], 0);
    let (_, _, outcome) = run(
        "fn main(){assert_eq!(true,match loop{}{true=>true});}",
        2024,
    );
    let RunPoll::Fault(error) = outcome else {
        panic!("unsupported actual scrutinee was skipped")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    let (_, _, outcome) = run(
        "fn value()->bool{panic!(\"earlier\")}fn main(){assert_eq!(true,match value(){true=>true});}",
        2024,
    );
    let RunPoll::Panicked(report) = outcome else {
        panic!("actual scrutinee panic was replaced by later exhaustion")
    };
    assert_eq!(report.panic.description(), "earlier");
    assert_eq!(report.opcode_counters.started_by_opcode[25], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
}

#[test]
fn exhaustion_exit_fuel_refusal_preserves_the_completed_scrutinee_prefix() {
    let text = "fn main(){assert_eq!(true,match false{true=>true});}";
    let (_, complete, outcome) = run(text, 2024);
    assert!(matches!(outcome,RunPoll::Fault(ref error) if error.code=="LRR-E-MATCH-NO-ARM"));
    let used = complete.fuel_snapshot().used;
    let expected = complete.opcode_counters().unwrap();
    assert!(used > 1);
    let (_, limited, outcome) = run_with_fuel(text, 2024, used - 1);
    let RunPoll::Fault(error) = outcome else {
        panic!("exhaustion exit was not fuel guarded")
    };
    assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(error.message, "guest opcode fuel exhausted");
    let actual = limited.opcode_counters().unwrap();
    assert_eq!(actual.started_by_opcode, expected.started_by_opcode);
    assert_eq!(actual.operations_started, expected.operations_started);
    assert_eq!(actual.operations_completed, expected.operations_completed);
    assert_eq!(actual.started_by_opcode[0], 3);
    assert_eq!(limited.fuel_snapshot().used, used - 1);
}
