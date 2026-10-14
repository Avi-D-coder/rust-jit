use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{BucketKey, QueryKey},
        memo::Database,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    source::{
        changes::{PreparedSourceInputs, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::outline::ScopeKey,
};
use std::{collections::BTreeMap, sync::Arc};
fn unit(path: &str, module: &[&str], source: &str) -> SourceUnitInput {
    SourceUnitInput {
        path: SourcePath::new(path).unwrap(),
        crate_key: Digest::of_bytes(b"crate"),
        module_path: module.iter().map(|s| s.to_string()).collect(),
        profile: ProfileIdentity::current(),
        cfg: CfgSet::default(),
        blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
    }
}
fn bucket(name: &str) -> QueryKey {
    QueryKey::ScopeBucket(BucketKey {
        scope: ScopeKey {
            crate_key: Digest::of_bytes(b"crate"),
            module_path: vec![],
            owner: None,
        },
        namespace: Namespace::Value,
        name: name.into(),
    })
}
fn candidates(value: &QueryValue) -> &[Digest] {
    let QueryValue::ScopeBucket(value) = value else {
        panic!("expected bucket: {value:?}")
    };
    &value.candidates
}
fn runs(db: &Database, kind: &str) -> u64 {
    db.trace
        .counts
        .get(&format!("{kind}:ProviderRun"))
        .copied()
        .unwrap_or(0)
}

#[test]
fn retained_prepared_sources_are_charged_and_release_their_storage() {
    use lrr::engine::telemetry::CacheBudgets;
    let mut db = Database::default();
    let before = PreparedSourceInputs::refresh(
        None,
        vec![unit("lib.rs", &[], "fn stable() { 1; }")],
        BTreeMap::new(),
    )
    .unwrap();
    before.publish(&mut db).unwrap();
    let first = db.cache_usage().unwrap();
    assert!(first.pinned_parse_bytes > 0);
    assert!(first.pinned_memo_bytes > 0);
    let parse_key = QueryKey::ParseFile(lrr::syntax::parse::ParseKey {
        content: SourceBlob::new(b"fn stable() { 1; }".to_vec())
            .unwrap()
            .content(),
        profile: ProfileIdentity::current(),
    });
    let parsed = db
        .fetch(&SyntaxProvider, parse_key.clone())
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    assert!(matches!(parsed.value.as_ref(), QueryValue::ParseFile(_)));
    assert_eq!(db.cache_usage().unwrap().parse_bytes, first.parse_bytes);
    drop(parsed);
    assert_eq!(
        db.begin_run_with_budgets(CacheBudgets {
            parse_bytes: 0,
            ..CacheBudgets::default()
        })
        .unwrap_err()
        .code,
        "LRR-E-METADATA-LIMIT"
    );
    before.publish(&mut db).unwrap();
    assert_eq!(db.cache_usage().unwrap().parse_bytes, first.parse_bytes);
    let after = PreparedSourceInputs::refresh(
        Some(&before),
        vec![unit("lib.rs", &[], "fn stable() { 2; }")],
        BTreeMap::new(),
    )
    .unwrap();
    after.publish(&mut db).unwrap();
    let retained = db.cache_usage().unwrap();
    assert!(retained.parse_bytes > first.parse_bytes);
    // The old memo still owns the old tree; evict it after releasing preparation.
    drop(before);
    db.evict_between_runs(CacheBudgets {
        parse_bytes: first.parse_bytes,
        ..CacheBudgets::default()
    })
    .unwrap();
    let released = db.cache_usage().unwrap();
    assert!(released.parse_bytes < retained.parse_bytes);
    assert!(released.source_input_bytes < retained.source_input_bytes);
    drop(after);
    db.publish_inputs(BTreeMap::new()).unwrap();
    let report = db
        .evict_between_runs(CacheBudgets {
            query_bytes: 0,
            parse_bytes: 0,
            source_input_bytes: 0,
        })
        .unwrap();
    assert_eq!(report.after.parse_bytes, 0);
    assert_eq!(report.after.memo_bytes, 0);
}

#[test]
fn retired_outline_inner_handle_is_charged_until_release() {
    use lrr::{db::key::OutlineKey, engine::telemetry::CacheBudgets, syntax::parse::ParseKey};
    let input = unit("lib.rs", &[], "fn stable() {} fn another() {}");
    let key = QueryKey::Outline(OutlineKey {
        parse: ParseKey {
            content: input.blob.content(),
            profile: input.profile.clone(),
        },
        crate_key: input.crate_key,
        module_path: vec![],
        cfg: input.cfg.fingerprint(),
    });
    let prepared = PreparedSourceInputs::refresh(None, vec![input], BTreeMap::new()).unwrap();
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let result = db
        .fetch(&SyntaxProvider, key.clone())
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::Outline(outline) = result.value.as_ref() else {
        panic!("expected outline")
    };
    let retained = outline.clone();
    drop(result);
    db.publish_inputs(BTreeMap::new()).unwrap();
    let deleted = db
        .fetch(&SyntaxProvider, key)
        .into_ready()
        .expect("deleted header is terminal")
        .unwrap_err();
    assert_eq!(deleted.code, "LRR-E-INTERNAL-SNAPSHOT");
    drop(prepared);
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
fn prepared_trees_are_shared_with_queries_and_cfg_identity_is_checked() {
    use lrr::{db::key::OutlineKey, syntax::parse::ParseKey};
    let source = unit(
        "lib.rs",
        &[],
        "fn stable() {} #[cfg(enabled)] fn conditional() {}",
    );
    let key = QueryKey::Outline(OutlineKey {
        parse: ParseKey {
            content: source.blob.content(),
            profile: source.profile.clone(),
        },
        crate_key: source.crate_key,
        module_path: source.module_path.clone(),
        cfg: source.cfg.fingerprint(),
    });
    let prepared =
        PreparedSourceInputs::refresh(None, vec![source.clone()], BTreeMap::new()).unwrap();
    let mut a = Database::default();
    let mut b = Database::default();
    prepared.publish(&mut a).unwrap();
    prepared.publish(&mut b).unwrap();
    let parse_bytes = a.cache_usage().unwrap().parse_bytes;
    let first = a
        .fetch(&SyntaxProvider, key.clone())
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let second = b
        .fetch(&SyntaxProvider, key.clone())
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let (QueryValue::Outline(first_tree), QueryValue::Outline(second_tree)) =
        (first.value.as_ref(), second.value.as_ref())
    else {
        panic!("expected outlines")
    };
    assert!(Arc::ptr_eq(first_tree, second_tree));
    assert_eq!(a.cache_usage().unwrap().parse_bytes, parse_bytes);
    let mut changed = source;
    changed.cfg.atoms.insert("enabled".into());
    let next =
        PreparedSourceInputs::refresh(Some(&prepared), vec![changed], BTreeMap::new()).unwrap();
    next.publish(&mut a).unwrap();
    let rejected = a
        .fetch(&SyntaxProvider, key)
        .into_ready()
        .expect("cfg mismatch is terminal")
        .unwrap_err();
    assert_eq!(rejected.code, "LRR-E-INTERNAL-SNAPSHOT");
}

#[test]
fn adding_missing_name_invalidates_only_its_negative_bucket() {
    let mut db = Database::default();
    db.trace.detailed = true;
    let before = PreparedSourceInputs::refresh(
        None,
        vec![unit("lib.rs", &[], "fn stable() {}")],
        BTreeMap::new(),
    )
    .unwrap();
    before.publish(&mut db).unwrap();
    assert!(
        candidates(
            &db.fetch(&SyntaxProvider, bucket("missing"))
                .into_ready()
                .expect("fixture query must complete")
                .unwrap()
                .value
        )
        .is_empty()
    );
    let stable = db
        .fetch(&SyntaxProvider, bucket("stable"))
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let after = PreparedSourceInputs::refresh(
        Some(&before),
        vec![unit("lib.rs", &[], "fn stable() {} fn missing() {}")],
        BTreeMap::new(),
    )
    .unwrap();
    after.publish(&mut db).unwrap();
    let retained = db
        .fetch(&SyntaxProvider, bucket("stable"))
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    assert!(Arc::ptr_eq(&stable.value, &retained.value));
    assert_eq!(runs(&db, "ScopeBucket"), 2);
    assert_eq!(
        candidates(
            &db.fetch(&SyntaxProvider, bucket("missing"))
                .into_ready()
                .expect("fixture query must complete")
                .unwrap()
                .value
        )
        .len(),
        1
    );
    assert_eq!(runs(&db, "ScopeBucket"), 3);
    assert!(
        db.trace
            .events
            .iter()
            .any(|e| e.key == bucket("missing") && e.action == lrr::db::trace::Action::Invalidated)
    );
}

#[test]
fn body_edit_preserves_bucket_and_header_and_reuses_unchanged_outline() {
    let mut db = Database::default();
    let before = PreparedSourceInputs::refresh(
        None,
        vec![
            unit("lib.rs", &[], "fn stable() { 1; }"),
            unit("other.rs", &["other"], "fn untouched() {}"),
        ],
        BTreeMap::new(),
    )
    .unwrap();
    before.publish(&mut db).unwrap();
    let bucket_before = db
        .fetch(&SyntaxProvider, bucket("stable"))
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let definition = candidates(&bucket_before.value)[0];
    let header_before = db
        .fetch(&SyntaxProvider, QueryKey::Signature(definition))
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let after = PreparedSourceInputs::refresh(
        Some(&before),
        vec![
            unit("lib.rs", &[], "fn stable() { absent_in_cold_body(); }"),
            unit("other.rs", &["other"], "fn untouched() {}"),
        ],
        BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(
        (after.parsed_files, after.outlined_files, after.reused_files),
        (1, 1, 1)
    );
    after.publish(&mut db).unwrap();
    assert!(Arc::ptr_eq(
        &bucket_before.value,
        &db.fetch(&SyntaxProvider, bucket("stable"))
            .into_ready()
            .expect("fixture query must complete")
            .unwrap()
            .value
    ));
    assert!(Arc::ptr_eq(
        &header_before.value,
        &db.fetch(&SyntaxProvider, QueryKey::Signature(definition))
            .into_ready()
            .expect("fixture query must complete")
            .unwrap()
            .value
    ));
    assert_eq!(runs(&db, "ScopeBucket"), 1);
    assert_eq!(runs(&db, "Signature"), 1);
}

#[test]
fn unchanged_missing_signature_failure_is_cached_and_new_definition_revalidates() {
    let mut db = Database::default();
    let future = PreparedSourceInputs::refresh(
        None,
        vec![unit("lib.rs", &[], "fn later() {}")],
        BTreeMap::new(),
    )
    .unwrap();
    future.publish(&mut db).unwrap();
    let definition = candidates(
        &db.fetch(&SyntaxProvider, bucket("later"))
            .into_ready()
            .expect("fixture query must complete")
            .unwrap()
            .value,
    )[0];
    let missing =
        PreparedSourceInputs::refresh(None, vec![unit("lib.rs", &[], "")], BTreeMap::new())
            .unwrap();
    missing.publish(&mut db).unwrap();
    let failed = db
        .fetch(&SyntaxProvider, QueryKey::Signature(definition))
        .into_ready()
        .expect("missing header is terminal")
        .unwrap_err();
    assert_eq!(failed.code, "LRR-E-NAME-MISSING");
    let cached = db
        .memo(&QueryKey::Signature(definition))
        .unwrap()
        .value
        .clone();
    missing.publish(&mut db).unwrap();
    let again = db
        .fetch(&SyntaxProvider, QueryKey::Signature(definition))
        .into_ready()
        .expect("missing header is terminal")
        .unwrap_err();
    assert_eq!(again.code, failed.code);
    assert!(Arc::ptr_eq(
        &cached,
        &db.memo(&QueryKey::Signature(definition)).unwrap().value
    ));
    assert_eq!(runs(&db, "Signature"), 1);
    future.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, QueryKey::Signature(definition))
            .into_ready()
            .expect("fixture query must complete")
            .unwrap()
            .value
            .as_ref(),
        QueryValue::Signature(_)
    ));
    assert_eq!(runs(&db, "Signature"), 2);
}

#[test]
fn failed_refresh_is_atomic_and_duplicate_candidates_are_deterministic() {
    let mut db = Database::default();
    let first = PreparedSourceInputs::refresh(
        None,
        vec![unit("lib.rs", &[], "fn same() {} fn same() {}")],
        BTreeMap::new(),
    )
    .unwrap();
    first.publish(&mut db).unwrap();
    let original = db
        .fetch(&SyntaxProvider, bucket("same"))
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let ids = candidates(&original.value);
    assert_eq!(ids.len(), 2);
    assert!(ids[0] < ids[1]);
    let revision = db.revision();
    let mut invalid = unit("bad.rs", &["bad"], "");
    invalid.profile.edition = 2099;
    let error = PreparedSourceInputs::refresh(
        Some(&first),
        vec![unit("lib.rs", &[], "fn changed() {}"), invalid],
        BTreeMap::new(),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "LRR-E-LIVE-EDITION");
    assert_eq!(db.revision(), revision);
    assert!(Arc::ptr_eq(
        &original.value,
        &db.fetch(&SyntaxProvider, bucket("same"))
            .into_ready()
            .expect("fixture query must complete")
            .unwrap()
            .value
    ));
    let repeat = PreparedSourceInputs::refresh(
        Some(&first),
        vec![unit("lib.rs", &[], "\nfn same() {}\nfn same() {}")],
        BTreeMap::new(),
    )
    .unwrap();
    repeat.publish(&mut db).unwrap();
    assert_eq!(
        candidates(
            &db.fetch(&SyntaxProvider, bucket("same"))
                .into_ready()
                .expect("fixture query must complete")
                .unwrap()
                .value
        ),
        ids
    );
}

#[test]
fn deleted_declaration_removes_header_and_invalidates_positive_bucket() {
    let mut db = Database::default();
    let before = PreparedSourceInputs::refresh(
        None,
        vec![unit("lib.rs", &[], "fn removed() {}")],
        BTreeMap::new(),
    )
    .unwrap();
    before.publish(&mut db).unwrap();
    let definition = candidates(
        &db.fetch(&SyntaxProvider, bucket("removed"))
            .into_ready()
            .expect("fixture query must complete")
            .unwrap()
            .value,
    )[0];
    assert!(matches!(
        db.fetch(&SyntaxProvider, QueryKey::Signature(definition))
            .into_ready()
            .expect("fixture query must complete")
            .unwrap()
            .value
            .as_ref(),
        QueryValue::Signature(_)
    ));
    let after = PreparedSourceInputs::refresh(
        Some(&before),
        vec![unit("lib.rs", &[], "")],
        BTreeMap::new(),
    )
    .unwrap();
    after.publish(&mut db).unwrap();
    assert!(
        candidates(
            &db.fetch(&SyntaxProvider, bucket("removed"))
                .into_ready()
                .expect("fixture query must complete")
                .unwrap()
                .value
        )
        .is_empty()
    );
    assert_eq!(
        db.fetch(&SyntaxProvider, QueryKey::Signature(definition))
            .into_ready()
            .expect("deleted signature is terminal")
            .unwrap_err()
            .code,
        "LRR-E-NAME-MISSING"
    );
}
