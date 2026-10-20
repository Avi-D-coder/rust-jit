//! LRR-specific publication/currentness controls supplement stock language cases.
use super::{configured_impl_catalog_discovery, temporary_source};
use crate::{
    db::{
        dependency::{InputKey, InputValue},
        key::{BucketKey, QueryKey},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    source::manifest::Namespace,
    syntax::outline::ScopeMembers,
};
use std::collections::BTreeMap;

#[test]
fn independent_generated_namespaces_merge_and_restore_originals_on_epoch_change() {
    for edition in [2015, 2018, 2021, 2024] {
        let path = temporary_source(
            &format!("namespace-publication-{edition}"),
            "trait Original { type Item; }\n\
             macro_rules! emit { ($($t:tt)*) => { $($t)* } }\n\
             emit! { trait Left { type Output; } struct First; }\n\
             emit! { trait Right { type Output; } struct Second; }\n\
             fn main() {}\n",
        );
        let (mut db, discovery, root) = configured_impl_catalog_discovery(&path, edition).unwrap();
        let base = db.input(&InputKey::ScopeMembers(root.clone())).unwrap();
        let InputValue::ScopeMembers(base_members) = base.as_ref() else {
            panic!("ordinary root members")
        };
        assert!(
            base_members
                .candidates
                .contains_key(&(Namespace::Type, "Original".into()))
        );
        assert!(
            !base_members
                .candidates
                .contains_key(&(Namespace::Type, "Left".into()))
        );
        let mut before = Vec::new();
        for name in ["Left", "Original", "NeverEmitted"] {
            let key = QueryKey::ScopeBucket(BucketKey {
                scope: root.clone(),
                namespace: Namespace::Type,
                name: name.into(),
            });
            let outcome = match db.fetch(&SyntaxProvider, key.clone()) {
                QueryPoll::Ready(Ok(value)) => value,
                other => panic!("actual prepublication bucket: {other:?}"),
            };
            if name != "Original" {
                assert!(
                    matches!(outcome.value.as_ref(), QueryValue::ScopeBucket(bucket)
                    if bucket.candidates.is_empty() && bucket.blockers == base_members.blockers)
                );
            }
            let guard = db
                .capture_prepared_inputs(&key, outcome.fingerprint)
                .unwrap();
            guard.require_current(&db).unwrap();
            before.push((name, key, outcome.value, guard));
        }
        discovery.publish(&mut db).unwrap();
        let mut positive_left = None;
        for (name, key, old_value, guard) in &before {
            if *name == "Left" {
                assert!(guard.require_current(&db).is_err());
            } else {
                guard.require_current(&db).unwrap();
            }
            let outcome = match db.fetch(&SyntaxProvider, key.clone()) {
                QueryPoll::Ready(Ok(value)) => value,
                other => panic!("actual postpublication bucket: {other:?}"),
            };
            if *name == "Left" {
                assert!(
                    matches!(outcome.value.as_ref(), QueryValue::ScopeBucket(bucket)
                    if bucket.candidates.len() == 1 && bucket.blockers == base_members.blockers)
                );
                positive_left = Some(
                    db.capture_prepared_inputs(key, outcome.fingerprint)
                        .unwrap(),
                );
            } else {
                assert!(std::sync::Arc::ptr_eq(old_value, &outcome.value));
            }
        }
        let inventory = match db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(discovery.key().clone()),
        ) {
            QueryPoll::Ready(Ok(value)) => value.value,
            other => panic!("actual published source inventory: {other:?}"),
        };
        let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.as_ref() else {
            panic!("actual inventory kind")
        };
        let environment = inventory.inventory_root().environment().unwrap();
        let mut expected = BTreeMap::from([(root.clone(), base_members.clone())]);
        let mut root_units = 0;
        for unit in environment.generated_publication_for_control().0.iter() {
            // Embedded dependencies have their own ordinary bases. This
            // control targets the selected root's two actual emit records.
            if unit.scope.crate_key != root.crate_key {
                continue;
            }
            if unit.scope == root {
                root_units += 1;
            }
            for (scope, members) in unit.scope_contributions() {
                expected
                    .entry(scope.clone())
                    .or_insert_with(ScopeMembers::default)
                    .merge(members);
                for candidates in members.candidates.values() {
                    for identity in candidates {
                        assert!(
                            unit.definitions.contains_key(identity),
                            "canonical generated declaration identity"
                        );
                    }
                }
            }
        }
        assert_eq!(root_units, 2, "two independent actual macro records");
        for (scope, members) in &expected {
            let actual = db.input(&InputKey::ScopeMembers(scope.clone())).unwrap();
            assert!(
                matches!(actual.as_ref(), InputValue::ScopeMembers(actual) if actual == members)
            );
            let blockers = db.input(&InputKey::ScopeBlockers(scope.clone())).unwrap();
            assert!(
                matches!(blockers.as_ref(), InputValue::ScopeBlockers(actual) if actual == &members.blockers)
            );
            for ((namespace, name), candidates) in &members.candidates {
                let bucket = BucketKey {
                    scope: scope.clone(),
                    namespace: *namespace,
                    name: name.clone(),
                };
                let input = db.input(&InputKey::NameCandidates(bucket.clone())).unwrap();
                assert!(
                    matches!(input.as_ref(), InputValue::NameCandidates(actual) if actual == candidates)
                );
                match db.fetch(&SyntaxProvider, QueryKey::ScopeBucket(bucket)) {
                    QueryPoll::Ready(Ok(value)) => assert!(matches!(value.value.as_ref(),
                        QueryValue::ScopeBucket(actual) if actual.candidates == *candidates
                            && actual.blockers == members.blockers)),
                    other => panic!("shared granular bucket: {other:?}"),
                }
            }
        }
        for name in ["Left", "Right", "First", "Second"] {
            assert!(
                expected[&root]
                    .candidates
                    .contains_key(&(Namespace::Type, name.into()))
            );
        }
        db.begin_expansion_epoch(2).unwrap();
        assert!(positive_left.unwrap().require_current(&db).is_err());
        let restored = db.input(&InputKey::ScopeMembers(root.clone())).unwrap();
        assert!(
            matches!(restored.as_ref(), InputValue::ScopeMembers(actual) if actual == base_members)
        );
        for (scope, members) in &expected {
            if *scope != root {
                assert!(db.input(&InputKey::ScopeMembers(scope.clone())).is_none());
                assert!(db.input(&InputKey::ScopeBlockers(scope.clone())).is_none());
            }
            for (namespace, name) in members.candidates.keys() {
                let actual = db.input(&InputKey::NameCandidates(BucketKey {
                    scope: scope.clone(),
                    namespace: *namespace,
                    name: name.clone(),
                }));
                let original = if *scope == root {
                    base_members.candidates.get(&(*namespace, name.clone()))
                } else {
                    None
                };
                match original {
                    Some(original) => assert!(
                        matches!(actual.as_deref(), Some(InputValue::NameCandidates(actual)) if actual == original)
                    ),
                    None => assert!(actual.is_none(), "generated candidate withdrawn"),
                }
            }
        }
        // Reuse a previously cached positive query after withdrawal. The base
        // scope is still published, so the common query must now return its
        // ordinary empty candidate set and original blockers.
        match db.fetch(
            &SyntaxProvider,
            QueryKey::ScopeBucket(BucketKey {
                scope: root.clone(),
                namespace: Namespace::Type,
                name: "Left".into(),
            }),
        ) {
            QueryPoll::Ready(Ok(value)) => assert!(matches!(value.value.as_ref(),
                QueryValue::ScopeBucket(actual) if actual.candidates.is_empty()
                    && actual.blockers == base_members.blockers)),
            other => panic!("withdrawn cached positive: {other:?}"),
        }
        db.publish_inputs(BTreeMap::new()).unwrap();
        assert!(db.input(&InputKey::ScopeMembers(root)).is_none());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
        eprintln!("generated namespace merge/withdrawal: edition{edition},2actualrecords");
    }
}
