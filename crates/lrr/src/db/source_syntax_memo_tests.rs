//! Genuine completed source/input/memo ownership controls over unchanged rustc
//! source. Registered under lexical_inputs_tests to reuse its real setup.
use super::*;
use crate::{
    db::{key::PreparedBodySyntaxKey, trace::Action},
    engine::telemetry::CacheBudgets,
};
const NESTED_SOURCE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
);
fn typed(
    db: &mut Database,
    key: &PreparedBodySyntaxKey,
) -> Arc<crate::db::memo::source_syntax_inputs::PreparedBodySyntax> {
    let outcome = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::PreparedBodySyntax(Box::new(key.clone())),
    ));
    let QueryValue::PreparedBodySyntax(value) = outcome.value.as_ref() else {
        panic!("actual structural memo result absent")
    };
    Arc::clone(value)
}
#[test]
fn actual_nested_stock_body_preparation_memo_and_exact_key_controls_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source, discovery) = setup(NESTED_SOURCE, edition);
        db.begin_expansion_epoch(1).unwrap();
        discovery.publish(&mut db).unwrap();
        let completed = ready(discovery.completed_source_environment(&mut db));
        let main = source
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "main")
            .unwrap()
            .def_key
            .fingerprint();
        let key = ready(db.publish_completed_source_syntax(&completed, source.selection(), main));
        let revision = db.revision();
        assert_eq!(
            ready(db.publish_completed_source_syntax(&completed, source.selection(), main)),
            key
        );
        assert_eq!(
            db.revision(),
            revision,
            "actual input registration is idempotent"
        );
        let before = db.trace.events.len();
        let first = typed(&mut db, &key);
        let second = typed(&mut db, &key);
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first.syntax().source(), source.selection());
        assert!(first.syntax().permits_body(main));
        assert_eq!(first.syntax().outputs().len(), 2);
        assert_eq!(
            db.trace.events[before..]
                .iter()
                .filter(|e| matches!(e.key, QueryKey::PreparedBodySyntax(_))
                    && e.action == Action::ProviderRun)
                .count(),
            1
        );
        assert!(
            db.trace.events[before..]
                .iter()
                .all(|e| !matches!(e.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
        );
        let revision = db.revision();
        db.begin_run().unwrap();
        assert!(matches!(
            db.publish_completed_source_syntax(&completed, source.selection(), main),
            QueryPoll::Ready(Err(_))
        ));
        db.finish_run().unwrap();
        assert_eq!(db.revision(), revision);
        let input_key = InputKey::CompletedSourceSyntax(key.plan);
        let input = db.inputs.get(&input_key).unwrap().value.as_ref().clone();
        assert!(
            db.publish_inputs(BTreeMap::from([(input_key, input)]))
                .is_err()
        );
        assert_eq!(
            db.revision(),
            revision,
            "generic publication cannot mint derived structural authority"
        );
        for mutation in 0..6 {
            let mut wrong = key.clone();
            match mutation {
                0 => wrong.path = "other.rs".into(),
                1 => wrong.source.cfg = Digest([181; 32]),
                2 => wrong.epoch = 2,
                3 => wrong.plan = Digest([182; 32]),
                4 => wrong.definition = Digest([183; 32]),
                _ => wrong.source.parse.profile.edition = 2099,
            }
            assert!(matches!(
                db.fetch(
                    &SyntaxProvider,
                    QueryKey::PreparedBodySyntax(Box::new(wrong))
                ),
                QueryPoll::Ready(Err(_))
            ));
        }
        // An unrelated actual input stamp is not part of this source certificate.
        db.inputs.insert(
            InputKey::CompileEnv("LRR_UNRELATED_STRUCTURAL_CONTROL".into()),
            InputRecord {
                value: Arc::new(InputValue::CompileEnv("value".into())),
                fingerprint: InputValue::CompileEnv("value".into()).fingerprint(),
            },
        );
        db.revision += 1;
        assert!(Arc::ptr_eq(&first, &typed(&mut db, &key)));
        ready(first.admit(&mut db));
        db.begin_expansion_epoch(2).unwrap();
        assert!(matches!(
            db.fetch(&SyntaxProvider, QueryKey::PreparedBodySyntax(Box::new(key))),
            QueryPoll::Ready(Err(_))
        ));
        assert!(matches!(first.admit(&mut db), QueryPoll::Ready(Err(_))));
    }
}
#[test]
fn actual_structural_result_outlives_completed_input_and_memo_with_accounted_roots() {
    let (mut db, source, discovery) = setup(NESTED_SOURCE, 2024);
    db.begin_expansion_epoch(1).unwrap();
    discovery.publish(&mut db).unwrap();
    let completed = ready(discovery.completed_source_environment(&mut db));
    let main = source
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .def_key
        .fingerprint();
    let key = ready(db.publish_completed_source_syntax(&completed, source.selection(), main));
    let result = typed(&mut db, &key);
    let weak_completed = Arc::downgrade(&completed);
    let weak_result = Arc::downgrade(&result);
    let weak_plan = Arc::downgrade(result.plan());
    let retained = result.metadata_bytes() + result.plan().metadata_bytes();
    drop(completed);
    drop(discovery);
    drop(source);
    db.publish_inputs(Default::default()).unwrap();
    db.memos.clear(); // Explicitly retire memo owners while the actual result Arc escapes.
    db.evict_between_runs(CacheBudgets::default()).unwrap();
    assert!(
        weak_completed.upgrade().is_none(),
        "no plan/result to completed-root backreference"
    );
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= retained);
    assert!(weak_plan.upgrade().is_some());
    assert!(matches!(result.admit(&mut db), QueryPoll::Ready(Err(_))));
    drop(result);
    // The first pass may retire a memo and its last inner handle; subsequent
    // normal between-run pruning releases now-unowned retirement rows.
    db.evict_between_runs(CacheBudgets::default()).unwrap();
    db.evict_between_runs(CacheBudgets::default()).unwrap();
    assert!(weak_result.upgrade().is_none());
    assert!(weak_plan.upgrade().is_none());
}

#[test]
fn actual_inner_plan_input_arc_outlives_outer_input_with_charged_wrapper_and_plan() {
    let (mut db, source, discovery) = setup(NESTED_SOURCE, 2024);
    db.begin_expansion_epoch(1).unwrap();
    discovery.publish(&mut db).unwrap();
    let completed = ready(discovery.completed_source_environment(&mut db));
    let main = source
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .def_key
        .fingerprint();
    let key = ready(db.publish_completed_source_syntax(&completed, source.selection(), main));
    let outer = db
        .input(&InputKey::CompletedSourceSyntax(key.plan))
        .unwrap();
    let InputValue::CompletedSourceSyntax(input) = outer.as_ref() else {
        panic!("actual opaque inner input absent")
    };
    let inner = Arc::clone(input);
    let weak = Arc::downgrade(&inner);
    let plan = Arc::downgrade(inner.plan());
    let retained = inner.exclusive_metadata_bytes() + inner.plan().metadata_bytes();
    drop(outer);
    drop(completed);
    drop(discovery);
    drop(source);
    db.publish_inputs(Default::default()).unwrap();
    db.memos.clear();
    db.evict_between_runs(CacheBudgets::default()).unwrap();
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= retained);
    assert!(weak.upgrade().is_some());
    assert!(plan.upgrade().is_some());
    drop(inner);
    db.evict_between_runs(CacheBudgets::default()).unwrap();
    db.evict_between_runs(CacheBudgets::default()).unwrap();
    assert!(weak.upgrade().is_none());
    assert!(plan.upgrade().is_none());
}

#[test]
fn real_plan_input_two_root_quota_refusal_leaves_publication_atomic() {
    let (mut db, source, discovery) = setup(NESTED_SOURCE, 2024);
    db.begin_expansion_epoch(1).unwrap();
    discovery.publish(&mut db).unwrap();
    let completed = ready(discovery.completed_source_environment(&mut db));
    let main = source
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .def_key
        .fingerprint();
    let count = db.source_plan_pins.registered_artifact_count();
    db.source_plan_pins.set_test_artifact_limit(count + 1);
    let before = (
        db.revision(),
        db.inputs.len(),
        db.source_plan_pins.registered_artifact_count(),
    );
    let error = match db.publish_completed_source_syntax(&completed, source.selection(), main) {
        QueryPoll::Ready(Err(error)) => error,
        _ => panic!("two actual roots exceeded one remaining registry row"),
    };
    assert_eq!(error.code, "LRR-E-METADATA-LIMIT");
    assert_eq!(
        before,
        (
            db.revision(),
            db.inputs.len(),
            db.source_plan_pins.registered_artifact_count()
        )
    );
}
