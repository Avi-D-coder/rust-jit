//! Source-only authority/reuse controls; originals remain unchanged. Registered
//! by the owning original_inert_tests module, not a second source harness.
use super::*;
use crate::db::{poll::QueryPoll, trace::Action};
const NESTED: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
);
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("completed module source authority: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("completed module source demand: {input:?}"),
    }
}
#[test]
fn actual_completed_module_fact_factory_shares_index_in_active_run_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source) = prepare(NESTED, edition, CfgSet::guest_default(), &[]);
        let mut d = discovery(&source, Namespace::Value, false);
        seal(&mut d);
        d.publish(&mut db).unwrap();
        let root = ready(d.completed_source_environment(&mut db));
        let first = ready(d.sealed_body_macro_environment(&mut db, source.selection()));
        let second = ready(root.sealed_body_macro_environment(&mut db, source.selection()));
        assert!(Arc::ptr_eq(first.index_root(), second.index_root()));
        assert!(Arc::ptr_eq(first.source_environment(), &root));
        assert_eq!(first.identity(), second.identity());
        assert_eq!(first.dependency(), second.dependency());
        assert_eq!(root.macro_index_roots().count(), root.sources().len());
        assert!(
            db.trace
                .events
                .iter()
                .all(|event| !matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
        );
        let revision = db.revision();
        db.begin_run().unwrap();
        let trace = db.trace.events.len();
        let reached = ready(root.sealed_body_macro_environment(&mut db, source.selection()));
        assert!(Arc::ptr_eq(first.index_root(), reached.index_root()));
        assert!(
            db.trace.events[trace..]
                .iter()
                .all(|event| event.action != Action::ProviderRun)
        );
        assert_eq!(
            db.revision(),
            revision,
            "factory must not publish or rebuild provider facts"
        );
        db.finish_run().unwrap();
        let mut wrong = source.selection().clone();
        wrong.path = SourcePath::new("unadmitted-other-module.rs").unwrap();
        assert!(matches!(
            root.sealed_body_macro_environment(&mut db, &wrong),
            QueryPoll::Ready(Err(_))
        ));
        wrong = source.selection().clone();
        wrong.outline.cfg = Digest([217; 32]);
        assert!(matches!(
            root.sealed_body_macro_environment(&mut db, &wrong),
            QueryPoll::Ready(Err(_))
        ));
        wrong = source.selection().clone();
        wrong.outline.crate_key = Digest([218; 32]);
        assert!(matches!(
            root.sealed_body_macro_environment(&mut db, &wrong),
            QueryPoll::Ready(Err(_))
        ));
        db.begin_expansion_epoch(2).unwrap();
        assert!(matches!(
            root.sealed_body_macro_environment(&mut db, source.selection()),
            QueryPoll::Ready(Err(_))
        ));
        assert!(matches!(
            reached.require_current(&mut db),
            QueryPoll::Ready(Err(_))
        ));
    }
}
#[test]
fn actual_module_envelope_retains_charged_completed_root_without_arc_cycle() {
    let (mut db, source) = prepare(NESTED, 2024, CfgSet::guest_default(), &[]);
    let mut d = discovery(&source, Namespace::Value, false);
    seal(&mut d);
    d.publish(&mut db).unwrap();
    let root = ready(d.completed_source_environment(&mut db));
    let envelope = ready(root.sealed_body_macro_environment(&mut db, source.selection()));
    let weak = Arc::downgrade(&root);
    drop(root);
    drop(d);
    drop(source);
    db.publish_inputs(Default::default()).unwrap();
    db.evict_between_runs(crate::engine::telemetry::CacheBudgets::default())
        .unwrap();
    let retained = weak
        .upgrade()
        .expect("actual envelope needs its completed retirement owner")
        .retained_metadata_bytes()
        .unwrap();
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= retained);
    drop(envelope);
    db.evict_between_runs(crate::engine::telemetry::CacheBudgets::default())
        .unwrap();
    assert!(
        weak.upgrade().is_none(),
        "completed facts must not own envelopes back through a cycle"
    );
}
