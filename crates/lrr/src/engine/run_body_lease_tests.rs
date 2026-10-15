//! Actual unchanged upstream call/return frames retain their source ownership.
//! These are LRR publication controls, not additional compiletest fixture passes.
use super::*;

#[test]
fn original_caller_and_callee_ownership_follow_actual_return_cleanup() {
    let (mut db, mut run, ticket) = return_tests::original_caller_return_frontier();
    let callee = run.frame.body.body().clone();
    let caller = run.calls[0].caller.body.body().clone();
    assert_ne!(caller.definition, callee.definition);
    assert_eq!(run.body_lease.body(), &callee);
    assert_eq!(run.calls[0].body_lease.body(), &caller);
    for body in [&caller, &callee] {
        assert_eq!(
            db.require_body_unowned(body.definition).unwrap_err().code,
            "LRR-E-BODY-ACTIVE"
        );
    }
    run.begin_return_cleanup(&mut db, ticket).unwrap();
    assert!(run.advance_return_cleanup().unwrap().is_none());
    assert!(run.calls.is_empty());
    assert_eq!(run.body_lease.body(), &caller);
    db.require_body_unowned(callee.definition).unwrap();
    assert_eq!(
        db.require_body_unowned(caller.definition).unwrap_err().code,
        "LRR-E-BODY-ACTIVE"
    );
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("unchanged upstream caller did not complete after lease transfer")
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    // A terminal machine still owns diagnostic/source state until dropped.
    assert!(db.require_body_unowned(caller.definition).is_err());
    drop(run);
    db.require_body_unowned(caller.definition).unwrap();
    db.finish_run().unwrap();
}

#[test]
fn dropping_an_unentered_original_frame_releases_its_definition() {
    const SOURCE: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/return/return-nil.rs");
    let (mut db, run) = borrows::tests::start_stock_body(SOURCE, "main");
    let body = run.root_body.clone();
    assert_eq!(run.frame.counters.operations_started, 0);
    assert_eq!(
        db.require_body_unowned(body.definition).unwrap_err().code,
        "LRR-E-BODY-ACTIVE"
    );
    drop(run);
    db.require_body_unowned(body.definition).unwrap();
    db.finish_run().unwrap();
}
