//! The original f body remains byte-for-byte unchanged; LRR probe is external.
use super::*;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-addr-of-ret.rs"
);

fn require_completed(outcome: RunPoll, case: &str) -> RunReport {
    match outcome {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("{case} faulted: {error:?}"),
        RunPoll::NeedInput(input) => panic!("{case} suspended: {input:?}"),
        RunPoll::Panicked(report) => panic!("{case} panicked: {report:?}"),
    }
}

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
    // rustc rejects this signature statically (E0106); LRR runs programs
    // that already pass `cargo check`, so it only type-checks the returned
    // reference at the return boundary. The result is the body's actual
    // reference, never one chosen among the inputs by the interpreter.
    let (mut db, mut run) = super::super::borrows::tests::start_stock_body(
        "fn f(x:&isize,y:&isize)->&isize{x}fn lrr_probe()->isize{let a=3_isize;let b=4_isize;*f(&a,&b)}",
        "lrr_probe",
    );
    let report = require_completed(run.poll(&mut db), "ambiguous elided reference result");
    assert!(matches!(report.value,ValueSummary::Signed(ref value) if value=="3"));
    assert!(run.calls.is_empty());
}

const NAMED_ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-borrow-scope.rs"
);
#[test]
fn named_source_reference_identity_returns_the_actual_target_then_cleans_the_callee() {
    // The actual imported Point declaration is unchanged; stock Box/assert main
    // and x_coord stay cold. This is an external common-route identity control.
    let source = format!(
        "{NAMED_ORIGINAL}\nfn lrr_identity(p:&Point)->&Point{{p}}\nfn lrr_probe()->isize{{let p=Point{{x:3_isize,y:4_isize}};let q=lrr_identity(&p);q.x}}"
    );
    let (mut db, mut run) = super::super::borrows::tests::start_stock_body(&source, "lrr_probe");
    let cold: Vec<_> = run.sources[0]
        .outline()
        .items
        .iter()
        .filter(|item| matches!(item.def_key.declared_name.as_str(), "main" | "x_coord"))
        .filter_map(|item| item.body.as_ref().map(|body| body.key()))
        .collect();
    let report = require_completed(
        run.poll(&mut db),
        "source-certified nominal reference identity",
    );
    assert!(matches!(report.value,ValueSummary::Signed(ref value) if value=="3"));
    assert!(run.calls.is_empty());
    assert!(run.returning.is_none());
    assert_eq!(run.call_bytes, 0);
    assert!(run.frame.active_scopes().is_empty());
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert!(report.opcode_counters.return_scopes_cleaned >= 2);
    assert!(
        !db.trace
            .events
            .iter()
            .any(|event| matches!(&event.key,QueryKey::BodyStructure(body) if cold.contains(body)))
    );
}
#[test]
fn nominal_reference_result_cannot_return_an_equal_schema_callee_local() {
    let source = format!(
        "{NAMED_ORIGINAL}\nfn lrr_wrong(p:&Point)->&Point{{let wrong=Point{{x:3_isize,y:4_isize}};&wrong}}\nfn lrr_probe(){{let p=Point{{x:3_isize,y:4_isize}};lrr_wrong(&p);}}"
    );
    let (mut db, mut run) = super::super::borrows::tests::start_stock_body(&source, "lrr_probe");
    let RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("callee-local nominal target escaped")
    };
    assert!(
        matches!(error.code, "LRR-E-OWNERSHIP" | "LRR-E-CALL-CONTEXT"),
        "{error:?}"
    );
    assert_eq!(run.calls.len(), 1);
    assert!(run.returning.is_none());
    assert_eq!(run.frame.status, crate::vm::frame::FrameStatus::Faulted);
}
#[test]
fn unchanged_point_interior_return_keeps_source_projection_and_caller_region() {
    let source = format!(
        "{NAMED_ORIGINAL}\nfn lrr_probe()->isize{{let p=Point{{x:3_isize,y:4_isize}};*x_coord(&p)}}"
    );
    let (mut db, mut run) = super::super::borrows::tests::start_stock_body(&source, "lrr_probe");
    let callee = run.sources[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "x_coord")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
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
    let report = require_completed(
        run.poll(&mut db),
        "unchanged source-certified interior result",
    );
    assert!(matches!(report.value, ValueSummary::Signed(ref value) if value=="3"));
    assert!(run.calls.is_empty());
    assert!(run.returning.is_none());
    assert_eq!(run.call_bytes, 0);
    assert!(run.frame.active_scopes().is_empty());
    assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert!(report.opcode_counters.return_scopes_cleaned >= 2);
    assert!(
        db.trace
            .events
            .iter()
            .any(|event| matches!(&event.key,QueryKey::BodyStructure(body) if body==&callee))
    );
    assert!(
        !db.trace
            .events
            .iter()
            .any(|event| matches!(&event.key,QueryKey::BodyStructure(body) if body==&cold_main))
    );
    // Source-only external unchanged-body control, never a stock Box/assert
    // full-main pass. Sole validator must execute this candidate.
}

#[test]
fn derived_reference_result_cannot_escape_a_callee_local_field() {
    let source = format!(
        "{NAMED_ORIGINAL}\nfn lrr_wrong(p:&Point)->&isize{{let wrong=Point{{x:3_isize,y:4_isize}};&wrong.x}}\nfn lrr_probe(){{let p=Point{{x:3_isize,y:4_isize}};lrr_wrong(&p);}}"
    );
    let (mut db, mut run) = super::super::borrows::tests::start_stock_body(&source, "lrr_probe");
    let RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("callee-local selected field escaped")
    };
    assert!(
        matches!(error.code, "LRR-E-OWNERSHIP" | "LRR-E-CALL-CONTEXT"),
        "{error:?}"
    );
    assert_eq!(run.calls.len(), 1);
    assert!(run.returning.is_none());
    assert_eq!(run.frame.counters.return_scopes_cleaned, 0);
}

#[test]
fn named_declared_lifetime_uses_the_same_source_selected_projection_return() {
    let source = format!(
        "{NAMED_ORIGINAL}\nfn lrr_named<'a>(p:&'a Point)->&'a isize{{&p.x}}\nfn lrr_probe()->isize{{let p=Point{{x:3_isize,y:4_isize}};*lrr_named(&p)}}"
    );
    let (mut db, mut run) = super::super::borrows::tests::start_stock_body(&source, "lrr_probe");
    let report = require_completed(run.poll(&mut db), "named lifetime selected-field return");
    assert!(matches!(report.value,ValueSummary::Signed(ref value) if value=="3"));
    assert!(run.calls.is_empty());
    assert!(run.returning.is_none());
    assert_eq!(run.call_bytes, 0);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
}
