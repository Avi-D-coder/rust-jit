use lrr::db::poll::QueryPoll;
use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::{
        dependency::{InputKey, InputValue},
        key::{ConstantKey, QueryKey},
        memo::Database,
        provider::{Provided, Provider, SyntaxProvider},
        value::QueryValue,
    },
    engine::telemetry::CacheBudgets,
    source::snapshot::SourceBlob,
    syntax::parse::ParseKey,
};
use std::collections::BTreeMap;
fn key(n: u8) -> QueryKey {
    QueryKey::Constant(ConstantKey {
        definition: Digest::of_bytes(&[n]),
        arguments: vec![],
        cfg: Digest::default(),
        profile: ProfileIdentity::current(),
    })
}
struct Fixture;
impl Provider for Fixture {
    fn execute(&self, _db: &mut Database, _key: &QueryKey) -> QueryPoll<Provided> {
        Provided::pure(QueryValue::Constant(1))
    }
}

#[test]
fn constrained_cache_evicts_oldest_and_recomputes_on_demand() {
    let mut db = Database::default();
    for n in 0..3 {
        drop(
            db.fetch(&Fixture, key(n))
                .into_ready()
                .expect("fixture query must complete")
                .unwrap(),
        );
    }
    drop(
        db.fetch(&Fixture, key(0))
            .into_ready()
            .expect("fixture query must complete")
            .unwrap(),
    );
    let entry = db.memo(&key(0)).unwrap().estimated_bytes;
    let report = db
        .evict_between_runs(CacheBudgets {
            query_bytes: entry * 2,
            ..CacheBudgets::default()
        })
        .unwrap();
    assert_eq!(report.victims, vec![key(1)]);
    assert_eq!(report.after.memo_entries, 2);
    assert_eq!(db.trace.counts["Constant:ProviderRun"], 3);
    drop(
        db.fetch(&Fixture, key(1))
            .into_ready()
            .expect("fixture query must complete")
            .unwrap(),
    );
    assert_eq!(db.trace.counts["Constant:ProviderRun"], 4);
}

#[test]
fn live_inner_parse_handle_is_pinned_until_released() {
    let mut db = Database::default();
    let blob = SourceBlob::new(b"fn live() {}".to_vec()).unwrap();
    let parse_key = QueryKey::ParseFile(ParseKey {
        content: blob.content(),
        profile: ProfileIdentity::current(),
    });
    db.publish_inputs(BTreeMap::from([(
        InputKey::SourceBlob(blob.content()),
        InputValue::SourceBlob(blob),
    )]))
    .unwrap();
    let result = db
        .fetch(&SyntaxProvider, parse_key.clone())
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    drop(result);
    // Cleanup can remove cache-only weak registrations. A memo hit must register
    // the newly returned ownership again before that memo is subsequently retired.
    db.evict_between_runs(CacheBudgets::default()).unwrap();
    let result = db
        .fetch(&SyntaxProvider, parse_key.clone())
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    assert_eq!(db.trace.counts["ParseFile:MemoHit"], 1);
    let QueryValue::ParseFile(parsed) = result.value.as_ref() else {
        panic!("parse failed")
    };
    let handle = parsed.clone();
    drop(result);
    let budgets = CacheBudgets {
        parse_bytes: 0,
        ..CacheBudgets::default()
    };
    assert_eq!(
        db.evict_between_runs(budgets).unwrap_err().code,
        "LRR-E-METADATA-LIMIT"
    );
    assert!(db.memo(&parse_key).is_some());
    assert!(handle.estimated_bytes() > 0);
    drop(handle);
    assert_eq!(
        db.evict_between_runs(budgets).unwrap().victims,
        vec![parse_key]
    );
}

#[test]
fn retired_outer_error_and_raw_inner_parse_remain_charged() {
    struct ErrorFixture;
    impl Provider for ErrorFixture {
        fn execute(&self, db: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
            if db.input(&InputKey::CompileEnv("error".into())).is_some() {
                Provided::error("LRR-E-NAME-MISSING", "retained error ".repeat(1024))
            } else {
                Provided::pure(QueryValue::Constant(1))
            }
        }
    }
    let mut db = Database::default();
    db.publish_inputs(BTreeMap::from([(
        InputKey::CompileEnv("error".into()),
        InputValue::CompileEnv("yes".into()),
    )]))
    .unwrap();
    let error = db
        .fetch(&ErrorFixture, key(0))
        .into_ready()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-NAME-MISSING");
    let retained = db.memo(&key(0)).unwrap().value.clone();
    db.publish_inputs(BTreeMap::new()).unwrap();
    drop(
        db.fetch(&ErrorFixture, key(0))
            .into_ready()
            .expect("fixture query must complete")
            .unwrap(),
    );
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= retained.estimated_bytes());
    assert_eq!(
        db.begin_run_with_budgets(CacheBudgets {
            query_bytes: 0,
            ..CacheBudgets::default()
        })
        .unwrap_err()
        .code,
        "LRR-E-METADATA-LIMIT"
    );
    drop(retained);
    db.evict_between_runs(CacheBudgets {
        query_bytes: 0,
        ..CacheBudgets::default()
    })
    .unwrap();
    let blob = SourceBlob::new(b"fn live() {}".to_vec()).unwrap();
    let parse_key = QueryKey::ParseFile(ParseKey {
        content: blob.content(),
        profile: ProfileIdentity::current(),
    });
    db.publish_inputs(BTreeMap::from([(
        InputKey::SourceBlob(blob.content()),
        InputValue::SourceBlob(blob),
    )]))
    .unwrap();
    let result = db
        .fetch(&SyntaxProvider, parse_key.clone())
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::ParseFile(parsed) = result.value.as_ref() else {
        panic!("parse failed")
    };
    let retained = parsed.clone();
    drop(result);
    db.publish_inputs(BTreeMap::new()).unwrap();
    let deleted = db
        .fetch(&SyntaxProvider, parse_key)
        .into_ready()
        .expect("deleted parse is terminal")
        .unwrap_err();
    assert_eq!(deleted.code, "LRR-E-INTERNAL-SNAPSHOT");
    assert_eq!(deleted.message, "parse source absent");
    assert!(db.cache_usage().unwrap().pinned_parse_bytes >= retained.estimated_bytes());
    assert_eq!(
        db.begin_run_with_budgets(CacheBudgets {
            parse_bytes: 0,
            ..CacheBudgets::default()
        })
        .unwrap_err()
        .code,
        "LRR-E-METADATA-LIMIT"
    );
    drop(retained);
    let report = db
        .evict_between_runs(CacheBudgets {
            query_bytes: 0,
            parse_bytes: 0,
            source_input_bytes: 0,
        })
        .unwrap();
    assert_eq!(report.after.memo_bytes, 0);
    assert_eq!(report.after.parse_bytes, 0);
}

#[test]
fn retained_input_value_is_charged_after_source_publication_replaces_it() {
    let mut db = Database::default();
    let blob = SourceBlob::new(b"fn live() {}".to_vec()).unwrap();
    let key = InputKey::SourceBlob(blob.content());
    let bytes = blob.bytes().len();
    db.publish_inputs(BTreeMap::from([(
        key.clone(),
        InputValue::SourceBlob(blob),
    )]))
    .unwrap();
    let retained = db.input(&key).unwrap();
    db.publish_inputs(BTreeMap::new()).unwrap();
    assert_eq!(db.cache_usage().unwrap().source_input_bytes, bytes);
    assert_eq!(
        db.begin_run_with_budgets(CacheBudgets {
            source_input_bytes: 0,
            ..CacheBudgets::default()
        })
        .unwrap_err()
        .code,
        "LRR-E-METADATA-LIMIT"
    );
    drop(retained);
    let report = db
        .evict_between_runs(CacheBudgets {
            query_bytes: 0,
            parse_bytes: 0,
            source_input_bytes: 0,
        })
        .unwrap();
    assert_eq!(report.after.memo_bytes, 0);
    assert_eq!(report.after.source_input_bytes, 0);
}

#[test]
fn eviction_and_publication_are_rejected_during_run_and_source_cap_prevents_start() {
    let mut db = Database::default();
    db.begin_run().unwrap();
    assert_eq!(
        db.evict_between_runs(CacheBudgets::default())
            .unwrap_err()
            .code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
    assert_eq!(
        db.publish_inputs(BTreeMap::new()).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
    db.finish_run().unwrap();
    let blob = SourceBlob::new(b"fn live() {}".to_vec()).unwrap();
    db.publish_inputs(BTreeMap::from([(
        InputKey::SourceBlob(blob.content()),
        InputValue::SourceBlob(blob),
    )]))
    .unwrap();
    assert_eq!(
        db.begin_run_with_budgets(CacheBudgets {
            source_input_bytes: 0,
            ..CacheBudgets::default()
        })
        .unwrap_err()
        .code,
        "LRR-E-METADATA-LIMIT"
    );
    // A rejected start did not enter the active-run state.
    db.publish_inputs(BTreeMap::new()).unwrap();
    db.begin_run().unwrap();
    db.finish_run().unwrap();
}
