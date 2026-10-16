//! Actual contained Type facets over an unchanged official qualified-path source.
//! These are authority/preparation controls, not a stock runtime pass.
use super::*;
const QUALIFIED: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/module-qualified-struct-destructure.rs"
);
fn qualified(edition: u16) -> (Database, PreparedSourceView) {
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(QUALIFIED, edition);
    let source = provider.module_source().share();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.trace.detailed = true;
    (db, source)
}
fn type_key(source: &PreparedSourceView) -> QueryKey {
    // Select the actual retained declaration scope, never infer it from a path.
    let (scope, _) = source
        .outline()
        .scopes
        .iter()
        .find(|(_, members)| {
            members
                .candidates
                .contains_key(&(Namespace::Type, "S".into()))
        })
        .expect("unchanged S declaration scope");
    QueryKey::CurrentModuleName(Box::new(CurrentModuleNameKey {
        source: source.selection().outline.clone(),
        path: source.selection().path.as_str().to_owned(),
        bucket: BucketKey {
            scope: scope.clone(),
            namespace: Namespace::Type,
            name: "S".into(),
        },
    }))
}
#[test]
fn inline_type_bucket_retains_actual_value_parent_and_exact_namespace_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source) = qualified(edition);
        let request = type_key(&source);
        assert_eq!(
            observe(&mut db, &request).completion(),
            ModuleNameCompletion::Unsealed
        );
        publish(&mut db, &source, Namespace::Value, 1);
        let answer = observe(&mut db, &request);
        assert_eq!(answer.completion(), ModuleNameCompletion::Complete);
        assert!(matches!(answer.lookup(), Some(NameLookup::Defined(_))));
        let Some(DependencyStamp::Query {
            key: QueryKey::SealedName(bucket),
            ..
        }) = answer.answer_dependency()
        else {
            panic!("actual sealed bucket dependency")
        };
        assert!(matches!(
            bucket.seal.domain,
            crate::db::seals::SealDomain::Scope {
                namespace: Namespace::Value,
                ..
            }
        ));
        assert_eq!(bucket.bucket.namespace, Namespace::Type);
        let parent = db
            .input(&InputKey::DiscoverySeal(bucket.seal.clone()))
            .unwrap();
        let InputValue::DiscoverySeal(parent) = parent.as_ref() else {
            panic!()
        };
        assert_ne!(parent.scope.as_ref().unwrap().scope, bucket.bucket.scope);
        parent.require_scope_certificate_layout().unwrap();
        let mut reordered = parent.clone();
        reordered.type_scopes.reverse();
        assert!(
            reordered.require_scope_certificate_layout().is_err(),
            "actual sorted ownership is required before locator publication"
        );
        let mut detached = parent.clone();
        detached.seal = crate::db::seals::Seal::completed(
            parent.seal.domain().clone(),
            Digest::of_bytes(b"different actual graph"),
            parent.seal.key().epoch,
            Digest::of_bytes(b"different members"),
            Digest::of_bytes(b"different scan"),
            vec![],
        );
        assert!(
            detached.require_scope_certificate_layout().is_err(),
            "a genuine Type receipt cannot be transplanted to another parent"
        );
        let mut wrong_profile = source.selection().outline.parse.profile.clone();
        wrong_profile.semantics.push_str("-foreign");
        assert!(
            parent
                .scope_certificate(&bucket.bucket.scope, Namespace::Type, &wrong_profile)
                .is_err()
        );
        let certificate = parent
            .scope_certificate(
                &bucket.bucket.scope,
                Namespace::Type,
                &source.selection().outline.parse.profile,
            )
            .unwrap()
            .unwrap();
        assert!(matches!(
            certificate.lookup(&bucket.bucket).unwrap(),
            NameLookup::Defined(_)
        ));
        let mut wrong = bucket.bucket.clone();
        wrong.namespace = Namespace::Value;
        assert!(
            parent.bucket_certificate(&wrong).unwrap().is_none(),
            "Type completion grants no Value completeness at the inline scope"
        );
        assert!(
            matches!(db.fetch(&SyntaxProvider, QueryKey::SealedName(Box::new(
            SealedNameKey { seal: bucket.seal.clone(), bucket: wrong }))),
            QueryPoll::Ready(Err(error)) if error.code == "LRR-E-QUERY-REQUEST"),
            "an immutable parent cannot be retried as authority for another facet"
        );
        let mut absent = bucket.bucket.clone();
        absent.name = "not_declared_in_module".into();
        let value = match db.fetch(
            &SyntaxProvider,
            QueryKey::SealedName(Box::new(SealedNameKey {
                seal: bucket.seal.clone(),
                bucket: absent,
            })),
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            other => panic!("actual negative bucket: {other:?}"),
        };
        assert!(matches!(
            value.value.as_ref(),
            QueryValue::SealedName {
                lookup: NameLookup::Absent,
                ..
            }
        ));
        // An actual second parent is relevant to the same contained scope. It
        // must invalidate the selected-candidate dependency and report ambiguity.
        publish(&mut db, &source, Namespace::Type, 1);
        assert!(
            matches!(db.fetch(&SyntaxProvider, request.clone()), QueryPoll::Ready(Err(error))
            if error.message.contains("selection is ambiguous"))
        );
        db.begin_expansion_epoch(2).unwrap();
        assert_eq!(
            observe(&mut db, &request).completion(),
            ModuleNameCompletion::Unsealed
        );
    }
}
