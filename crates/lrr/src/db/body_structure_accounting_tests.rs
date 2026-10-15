//! LRR cache lifetime/quota controls, using unchanged upstream body structure.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    engine::telemetry::CacheBudgets,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{body_index::BodyIndex, parse::ParsedFile},
};

struct BodyProvider(Arc<BodyIndex>);
impl Provider for BodyProvider {
    fn execute(&self, _: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
        Provided::pure(QueryValue::BodyStructure(Arc::clone(&self.0)))
    }
}
fn original_body() -> Arc<BodyIndex> {
    let original =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/return/return-nil.rs");
    let parsed = ParsedFile::parse(
        &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([73; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    Arc::new(parsed.index_body(body, Default::default()).unwrap())
}

#[test]
fn inner_body_handle_pins_cached_and_retired_allocations_until_drop() {
    let body = original_body();
    let weak = Arc::downgrade(&body);
    let key = QueryKey::BodyStructure(body.body().clone());
    let mut db = Database::default();
    let outcome = db
        .fetch(&BodyProvider(Arc::clone(&body)), key.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    drop(outcome);
    let usage = db.cache_usage().unwrap();
    assert!(usage.pinned_memo_bytes >= body.estimated_bytes() + 2 * std::mem::size_of::<usize>());
    assert_eq!(
        db.evict_between_runs(CacheBudgets {
            query_bytes: usage.pinned_memo_bytes - 1,
            ..CacheBudgets::default()
        })
        .unwrap_err()
        .code,
        "LRR-E-METADATA-LIMIT"
    );
    assert!(db.memo(&key).is_some());
    // Model dependency invalidation retiring the outer memo while an active
    // frame still owns only its inner structural allocation.
    db.memos.remove(&key);
    let retired = db.cache_usage().unwrap();
    assert_eq!(retired.memo_entries, 0);
    assert_eq!(retired.memo_bytes, retired.pinned_memo_bytes);
    assert!(retired.pinned_memo_bytes >= body.estimated_bytes() + 2 * std::mem::size_of::<usize>());
    drop(body);
    assert!(weak.upgrade().is_none());
    let reclaimed = db
        .evict_between_runs(CacheBudgets {
            query_bytes: 0,
            ..CacheBudgets::default()
        })
        .unwrap();
    assert_eq!(reclaimed.after.memo_bytes, 0);
    assert_eq!(reclaimed.after.pinned_memo_bytes, 0);
}

#[test]
fn retired_and_cached_query_wrappers_charge_shared_body_exactly_once() {
    let body = original_body();
    let key = QueryKey::BodyStructure(body.body().clone());
    let provider = BodyProvider(body);
    let mut db = Database::default();
    let first = db
        .fetch(&provider, key.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    db.memos.remove(&key);
    let second = db.fetch(&provider, key).into_ready().unwrap().unwrap();
    assert!(!Arc::ptr_eq(&first.value, &second.value));
    let QueryValue::BodyStructure(one) = first.value.as_ref() else {
        panic!("body absent")
    };
    let QueryValue::BodyStructure(two) = second.value.as_ref() else {
        panic!("body absent")
    };
    assert!(Arc::ptr_eq(one, two));
    let with_retired = db.cache_usage().unwrap();
    drop(first);
    let without_retired = db.cache_usage().unwrap();
    // Only the retired outer Arc box disappears. The shared structural heap
    // and existing weak registry rows must be unchanged.
    assert_eq!(
        with_retired.memo_bytes - without_retired.memo_bytes,
        std::mem::size_of::<QueryValue>() + 16
    );
    assert_eq!(
        with_retired.pinned_memo_bytes - without_retired.pinned_memo_bytes,
        std::mem::size_of::<QueryValue>() + 16
    );
    drop(second);
    drop(provider);
    let reclaimed = db
        .evict_between_runs(CacheBudgets {
            query_bytes: 0,
            ..CacheBudgets::default()
        })
        .unwrap();
    assert_eq!(reclaimed.victims.len(), 1);
    assert_eq!(db.cache_usage().unwrap().pinned_memo_bytes, 0);
}
