//! Allocation/lifetime controls on unchanged upstream entry and call/return.
//! These prove LRR sharing invariants, not additional full-fixture acceptance.
use super::*;

#[test]
fn original_root_frame_shares_cached_body_without_sharing_execution_state() {
    const SOURCE: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/return/return-nil.rs");
    let (mut db, mut first) = borrows::tests::start_stock_body(SOURCE, "main");
    let mut second = RunMachine::start_with_sources(
        &mut db,
        first.root_body.clone(),
        1000,
        first
            .sources
            .iter()
            .map(PreparedSourceView::share)
            .collect(),
    )
    .into_ready()
    .unwrap()
    .unwrap();
    let cached = db
        .fetch(
            &SyntaxProvider,
            QueryKey::BodyStructure(first.root_body.clone()),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(body) = cached.value.as_ref() else {
        panic!("body absent")
    };
    assert!(Arc::ptr_eq(body, &first.frame.body));
    assert!(Arc::ptr_eq(body, &second.frame.body));
    assert_eq!(first.frame.counters.operations_started, 0);
    assert_eq!(second.frame.counters.operations_started, 0);
    let RunPoll::Completed(report) = first.poll(&mut db) else {
        panic!("first original main did not complete")
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert!(first.frame.counters.operations_started > 0);
    assert_eq!(second.frame.counters.operations_started, 0);
    assert_eq!(second.frame.status, crate::vm::frame::FrameStatus::Running);
    drop(cached);
    // The first machine's completion must not consume the other frame's fuel,
    // slots or continuation, despite both retaining the same structural body.
    let RunPoll::Completed(report) = second.poll(&mut db) else {
        panic!("second original main did not complete independently")
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    drop(first);
    drop(second);
    db.finish_run().unwrap();
}

#[test]
fn source_call_plan_and_callee_share_query_body_through_return_cleanup() {
    let (mut db, mut run, ticket) = return_tests::original_caller_return_frontier();
    let caller_body = Arc::clone(&run.calls[0].caller.body);
    let callee_body = Arc::clone(&run.frame.body);
    assert!(Arc::ptr_eq(&callee_body, &run.calls[0].call.body));
    let cached = db
        .fetch(
            &SyntaxProvider,
            QueryKey::BodyStructure(callee_body.body().clone()),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(body) = cached.value.as_ref() else {
        panic!("body absent")
    };
    assert!(Arc::ptr_eq(body, &callee_body));
    run.begin_return_cleanup(&mut db, ticket).unwrap();
    assert!(run.advance_return_cleanup().unwrap().is_none());
    assert!(Arc::ptr_eq(&run.frame.body, &caller_body));
    assert!(run.calls.is_empty());
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("original caller did not finish")
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    drop(cached);
    drop(run);
    db.finish_run().unwrap();
}
