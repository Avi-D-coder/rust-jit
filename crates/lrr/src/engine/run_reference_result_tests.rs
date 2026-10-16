//! The original f body remains byte-for-byte unchanged; LRR probe is external.
use super::*;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-addr-of-ret.rs"
);

#[test]
fn unchanged_stock_reference_result_body_reborrows_and_resumes_the_actual_caller() {
    // Keep stock main (including println) intact and cold. This supplemental
    // entry exercises the actual unchanged f, not a stock whole-main pass.
    let source = format!("{ORIGINAL}\nfn lrr_probe()->isize{{let three=&3_isize;*f(three)}}");
    let (mut db, mut run) = super::super::borrows::tests::start_stock_body(&source, "lrr_probe");
    let cold_main = run.sources[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let report = match run.poll(&mut db) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("unchanged reference-result body fault: {error:?}"),
        RunPoll::NeedInput(input) => panic!("unchanged reference-result body suspended: {input:?}"),
        RunPoll::Panicked(report) => panic!("unchanged reference-result body panic: {report:?}"),
    };
    assert!(matches!(report.value, ValueSummary::Signed(ref value) if value == "3"));
    assert!(run.calls.is_empty());
    assert!(run.returning.is_none());
    assert_eq!(run.call_bytes, 0);
    assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
    assert!(report.opcode_counters.started_by_opcode[31] >= 2);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert!(report.opcode_counters.return_scopes_cleaned >= 2);
    assert!(run.frame.active_scopes().is_empty());
    assert!(!db.trace.events.iter().any(|event| matches!(&event.key,
        QueryKey::BodyStructure(body) if body == &cold_main)));
}

#[test]
fn reference_result_cannot_escape_an_equal_typed_callee_local_before_cleanup() {
    let (mut db, mut run) = super::super::borrows::tests::start_stock_body(
        "fn f(x:&isize)->&isize{let wrong:isize=3;&wrong}fn main()->isize{let actual:isize=3;*f(&actual)}",
        "main",
    );
    let RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("callee-local reference target was published to caller")
    };
    assert!(matches!(
        error.code,
        "LRR-E-OWNERSHIP" | "LRR-E-CALL-CONTEXT"
    ));
    assert_eq!(run.calls.len(), 1);
    assert!(run.returning.is_none());
    assert_eq!(run.frame.status, crate::vm::frame::FrameStatus::Faulted);
    assert_eq!(run.frame.counters.return_scopes_cleaned, 0);
    // The actual returned reference's live location, not borrow-checker
    // diagnostics, prevents the wrong target crossing this runtime boundary.
}

#[test]
fn ambiguous_elided_reference_result_never_selects_an_arbitrary_input() {
    let (mut db, mut run) = super::super::borrows::tests::start_stock_body(
        "fn f(x:&isize,y:&isize)->&isize{x}fn main(){f(&3_isize,&3_isize);}",
        "main",
    );
    let RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("ambiguous result region was admitted")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert!(error.message.contains("unambiguous declared input region"));
    assert!(run.calls.is_empty());
    assert_eq!(run.peak_guest_frames(), 1);
}
