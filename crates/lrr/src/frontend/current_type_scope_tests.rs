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
        // Type completion grants no Value completeness at the inline scope:
        // the Value fixed point certifies that module's own Value names as a
        // separate contained Value facet, never through the Type receipt.
        let value_facet = parent
            .bucket_certificate(&wrong)
            .unwrap()
            .expect("the Value fixed point's own inline Value facet");
        assert_eq!(value_facet.namespace, Namespace::Value);
        assert!(
            parent
                .value_scopes
                .iter()
                .any(|receipt| std::ptr::eq(receipt.scope(), value_facet))
                && parent
                    .type_scopes
                    .iter()
                    .all(|receipt| !std::ptr::eq(receipt.scope(), value_facet)),
            "the Value answer is the contained Value facet, not the Type receipt"
        );
        let mut retyped = parent.clone();
        retyped.value_scopes.clear();
        assert!(
            retyped.bucket_certificate(&wrong).unwrap().is_none(),
            "a Type receipt alone grants no Value completeness"
        );
        assert!(
            matches!(
                db.fetch(
                    &SyntaxProvider,
                    QueryKey::SealedName(Box::new(SealedNameKey {
                        seal: bucket.seal.clone(),
                        bucket: wrong
                    }))
                ),
                QueryPoll::Ready(Ok(_))
            ),
            "the inline Value facet answers its own namespace"
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
        let selected_dependency = db
            .memo(&request)
            .unwrap()
            .dependencies
            .iter()
            .find(|dependency| {
                matches!(
                    dependency,
                    DependencyStamp::Input {
                        key: InputKey::ScopeCertificateSelection(_),
                        ..
                    }
                )
            })
            .expect("actual relevant candidate-set dependency")
            .clone();
        // Both root discoveries certify this child scope derivatively. Their
        // independently produced answers agree, so adding the Type parent
        // refreshes the candidate dependency without changing the name answer.
        publish(&mut db, &source, Namespace::Type, 1);
        let refreshed = observe(&mut db, &request);
        assert!(!Arc::ptr_eq(&answer, &refreshed));
        assert_eq!(refreshed.completion(), ModuleNameCompletion::Complete);
        assert_eq!(answer.lookup(), refreshed.lookup());
        assert_eq!(answer.semantic_identity(), refreshed.semantic_identity());
        let refreshed_dependency = db
            .memo(&request)
            .unwrap()
            .dependencies
            .iter()
            .find(|dependency| {
                matches!(
                    dependency,
                    DependencyStamp::Input {
                        key: InputKey::ScopeCertificateSelection(_),
                        ..
                    }
                )
            })
            .expect("refreshed relevant candidate-set dependency");
        assert_ne!(&selected_dependency, refreshed_dependency);
        assert!(Arc::ptr_eq(&refreshed, &observe(&mut db, &request)));
        db.begin_expansion_epoch(2).unwrap();
        assert_eq!(
            observe(&mut db, &request).completion(),
            ModuleNameCompletion::Unsealed
        );
    }
}

#[test]
fn independently_published_primary_type_certificates_remain_ambiguous_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source) = qualified(edition);
        let request = key(&source, Namespace::Type, "m");
        publish(&mut db, &source, Namespace::Type, 1);
        assert!(matches!(
            observe(&mut db, &request).lookup(),
            Some(NameLookup::Defined(_))
        ));
        // A separate genuine discovery in another compilation-unit graph
        // produces a second primary certificate for the exact same source
        // scope. No cloned/retagged certificate is inserted into the database.
        let mut discovery = SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Type,
            Digest::of_bytes(b"independent primary Type discovery graph"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"current module-name actual compile environment"),
            crate::expand::source_declarative::engine_version(),
            Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
        )
        .unwrap();
        loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Sealed(_) => {
                    discovery.publish(&mut db).unwrap();
                    break;
                }
                _ => panic!("actual source discovery requires another producer"),
            }
        }
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
