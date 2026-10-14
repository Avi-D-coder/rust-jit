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
    syntax::{outline::ScopeKey, parse::ParsedFile},
};
#[test]
fn source_names_use_nfc_for_ids_duplicates_and_buckets_without_rewriting_tokens() {
    let unit = Digest([1; 32]);
    let source = "mod cafe\u{301} { pub fn r#gen()->u64{17} pub fn cafe\u{301}(){} pub fn café(){} pub fn ａ(){} pub fn a(){} }";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed.outline(unit, vec![], &CfgSet::default()).unwrap();
    let canonical_source = source.replace("cafe\u{301}", "café");
    let canonical = ParsedFile::parse(
        &SourceBlob::new(canonical_source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .outline(unit, vec![], &CfgSet::default())
    .unwrap();
    assert_eq!(
        outline
            .items
            .iter()
            .map(|item| item.def_key.fingerprint())
            .collect::<Vec<_>>(),
        canonical
            .items
            .iter()
            .map(|item| item.def_key.fingerprint())
            .collect::<Vec<_>>()
    );
    let duplicates: Vec<_> = outline
        .items
        .iter()
        .filter(|i| i.def_key.declared_name == "café" && i.def_key.namespace == Namespace::Value)
        .collect();
    assert_eq!(duplicates.len(), 2);
    assert_eq!(
        duplicates
            .iter()
            .map(|i| i.def_key.duplicate_ordinal)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_ne!(
        duplicates[0].def_key.fingerprint(),
        duplicates[1].def_key.fingerprint()
    );
    assert!(
        duplicates[0]
            .header
            .tokens
            .iter()
            .any(|t| t.text == "cafe\u{301}")
    );
    let mut db = Database::default();
    let prepared = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path: SourcePath::new("source.rs").unwrap(),
            crate_key: unit,
            module_path: vec![],
            profile: ProfileIdentity::current(),
            cfg: CfgSet::default(),
            blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        }],
        Default::default(),
    )
    .unwrap();
    prepared.publish(&mut db).unwrap();
    for (name, count) in [("gen", 1), ("café", 2), ("ａ", 1), ("a", 1)] {
        let fetched = db
            .fetch(
                &SyntaxProvider,
                QueryKey::ScopeBucket(BucketKey {
                    scope: ScopeKey {
                        crate_key: unit,
                        module_path: vec!["café".into()],
                        owner: None,
                    },
                    namespace: Namespace::Value,
                    name: name.into(),
                }),
            )
            .into_ready()
            .expect("fixture query must complete")
            .unwrap();
        let QueryValue::ScopeBucket(bucket) = fetched.value.as_ref() else {
            panic!("missing bucket")
        };
        assert_eq!(bucket.candidates.len(), count);
        assert!(bucket.blockers.is_empty());
    }
}
#[test]
fn source_context_module_paths_share_canonical_identity_and_duplicate_publication_rejects() {
    let unit = Digest([1; 32]);
    let source = "pub fn helper(){}";
    let make = |name: &str| SourceUnitInput {
        path: SourcePath::new("helper.rs").unwrap(),
        crate_key: unit,
        module_path: vec![name.into()],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::default(),
        blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
    };
    assert!(
        PreparedSourceInputs::refresh(
            None,
            vec![make("café"), make("cafe\u{301}")],
            Default::default()
        )
        .is_err()
    );
    let prepared =
        PreparedSourceInputs::refresh(None, vec![make("cafe\u{301}")], Default::default()).unwrap();
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let fetched = db
        .fetch(
            &SyntaxProvider,
            QueryKey::ScopeBucket(BucketKey {
                scope: ScopeKey {
                    crate_key: unit,
                    module_path: vec!["café".into()],
                    owner: None,
                },
                namespace: Namespace::Value,
                name: "helper".into(),
            }),
        )
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::ScopeBucket(bucket) = fetched.value.as_ref() else {
        panic!("missing bucket")
    };
    assert_eq!(bucket.candidates.len(), 1);
}
