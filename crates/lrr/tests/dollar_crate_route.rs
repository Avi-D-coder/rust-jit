use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::{key::QueryKey, memo::Database, provider::SyntaxProvider, value::QueryValue},
    expand::scopes::MacroName,
    frontend::hygiene::HygieneFacts,
    source::{
        changes::{PreparedSourceInputs, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        outline::AttributeClass,
        token_wire::{ContextRecord, Edition, Key, ResolutionContext, Transparency},
    },
};
use std::collections::BTreeMap;
#[test]
fn macro_export_is_a_known_facet_and_dollar_crate_routes_to_owner_bucket() {
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([2; 32]),
        edition: Edition::E2024,
    };
    let marked = ContextRecord {
        parent: Some(root.key()),
        expansion: Some(Key([8; 32])),
        transparency: Transparency::SemiTransparent,
        defining_unit: Key([1; 32]),
        edition: Edition::E2024,
    };
    let facts = HygieneFacts::new(
        vec![root.clone(), marked.clone()],
        vec![ResolutionContext {
            original: marked.key(),
            item: root.key(),
            local: marked.key(),
        }],
    )
    .unwrap();
    let bucket = facts
        .dollar_crate_bucket(
            marked.key(),
            &[MacroName::parse("helper", 2024).unwrap()],
            Namespace::Value,
        )
        .unwrap();
    assert_eq!(bucket.scope.crate_key, Digest([1; 32]));
    assert!(bucket.scope.module_path.is_empty());
    assert!(
        facts
            .dollar_crate_bucket(root.key(), &[], Namespace::Value)
            .is_err()
    );
    let source = "pub fn helper() -> u64 { 17 } #[macro_export] macro_rules! exported { () => {$crate::helper()}; }";
    let input = SourceUnitInput {
        path: SourcePath::new("provider.rs").unwrap(),
        crate_key: Digest([1; 32]),
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::default(),
        blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
    };
    let mut db = Database::default();
    let prepared = PreparedSourceInputs::refresh(None, vec![input], BTreeMap::new()).unwrap();
    prepared.publish(&mut db).unwrap();
    let result = db
        .fetch(&SyntaxProvider, QueryKey::ScopeBucket(bucket))
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::ScopeBucket(bucket) = result.value.as_ref() else {
        panic!("missing helper bucket")
    };
    assert_eq!(bucket.candidates.len(), 1);
    assert!(bucket.blockers.is_empty());
    let signature = db
        .fetch(&SyntaxProvider, QueryKey::Signature(bucket.candidates[0]))
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::Signature(header) = signature.value.as_ref() else {
        panic!("missing helper signature")
    };
    assert!(header.tokens.iter().any(|t| t.text == "helper"));
    // The macro attribute remains in its own header as explicit export metadata.
    let macro_bucket = db
        .fetch(
            &SyntaxProvider,
            QueryKey::ScopeBucket(lrr::db::key::BucketKey {
                scope: lrr::syntax::outline::ScopeKey {
                    crate_key: Digest([1; 32]),
                    module_path: vec![],
                    owner: None,
                },
                namespace: Namespace::Macro,
                name: "exported".into(),
            }),
        )
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::ScopeBucket(bucket) = macro_bucket.value.as_ref() else {
        panic!("missing macro bucket")
    };
    assert_eq!(bucket.candidates.len(), 1);
    let signature = db
        .fetch(&SyntaxProvider, QueryKey::Signature(bucket.candidates[0]))
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::Signature(header) = signature.value.as_ref() else {
        panic!("missing macro signature")
    };
    assert!(
        header
            .attributes
            .iter()
            .any(|a| a.name == "macro_export" && a.class == AttributeClass::MacroExport)
    );
}
