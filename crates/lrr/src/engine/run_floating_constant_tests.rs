//! Unchanged float-nan runtime contract and LRR retained-source authority controls.
use super::*;
use crate::engine::standalone_macros::CompletedStandaloneRuntimeMacros;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/float-nan.rs"
);
/// The production standalone route (sealed discovery that expands the
/// library's own assertion macros, then the retained entry body).
fn start(edition: u16) -> (Database, RunMachine, CompletedStandaloneRuntimeMacros) {
    borrows::tests::start_discovered_body(ORIGINAL, "main", edition)
}
#[test]
fn unchanged_float_nan_whole_main_executes_all_assertions_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run, macros) = start(edition);
        let contexts = macros.contexts().unwrap();
        let report = match run.poll_with_runtime_macros(&mut db, &contexts) {
            RunPoll::Completed(report) => report,
            RunPoll::Fault(error) => panic!(
                "unchanged float-nan.rs edition {edition}, pc {:?}: {error:?}",
                run.frame.pc(),
            ),
            RunPoll::Panicked(report) => {
                panic!("unchanged float-nan.rs edition {edition} panicked: {report:?}")
            }
            RunPoll::NeedInput(request) => {
                panic!("unchanged float-nan.rs edition {edition} suspended: {request:?}")
            }
        };
        // The library's own `assert!`/`assert_eq!` expand through the ordinary
        // pipeline; `f64::NAN` and `is_nan` are ordinary paths and methods.
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        assert!(run.frame.active_scopes().is_empty());
        assert!(run.calls.is_empty() && run.pending.is_empty());
        assert!(run.returning.is_none() && run.unwinding.is_none());
        assert!(run.terminal);
        db.finish_run().unwrap();
    }
}
