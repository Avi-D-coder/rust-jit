//! Actual V44 source-preparation controls; no guest/oracle execution.
use super::cross_source_tests::prepared_world;
use super::*;
use crate::{core::profile::ProfileIdentity, source::dependencies::SourceDependency};
const APP: Digest = Digest([201; 32]);
const LIB: Digest = Digest([202; 32]);
const LEAF: Digest = Digest([203; 32]);
fn publication<'a>(db: &'a Database, parse: &ParseKey) -> &'a crate::db::seals::DiscoveryInput {
    let InputValue::GeneratedSyntax(unit) = db
        .published_input(&InputKey::GeneratedSyntax(parse.content))
        .unwrap()
        .0
    else {
        panic!()
    };
    let InputValue::DiscoverySeal(publication) = db
        .published_input(&InputKey::DiscoverySeal(unit.seal.clone()))
        .unwrap()
        .0
    else {
        panic!()
    };
    publication
}
#[test]
fn actual_mixed_edition_nested_dependency_retains_each_observed_scope_profile() {
    let (mut db, signature, parse, _sources) = prepared_world(
        vec![
            (
                "app/main.rs",
                APP,
                "renamed::outer!();fn main()->i64{made()}",
                2024,
            ),
            (
                "dependency/lib.rs",
                LIB,
                "pub extern crate helper as tools;#[macro_export]macro_rules! outer{()=>{$crate::tools::seed!();}}",
                2015,
            ),
            (
                "helper/lib.rs",
                LEAF,
                "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{137_i64}}}",
                2024,
            ),
        ],
        vec![
            SourceDependency {
                consumer: APP,
                alias: "renamed".into(),
                provider: LIB,
            },
            SourceDependency {
                consumer: LIB,
                alias: "helper".into(),
                provider: LEAF,
            },
        ],
        true,
    );
    assert_eq!(signature.header.profile.edition, 2015);
    let publication = publication(&db, &parse);
    let actual = publication
        .type_scopes
        .iter()
        .filter(|proof| proof.scope().scope == signature.scope)
        .collect::<Vec<_>>();
    for edition in [2015, 2024] {
        let proof = actual
            .iter()
            .find(|proof| proof.scope().edition == edition)
            .expect("each genuinely observed owning profile is retained");
        proof
            .require_owner(
                &signature.scope,
                &ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
        proof.require_current(&db, publication).unwrap();
    }
    let proof = actual
        .iter()
        .find(|proof| proof.scope().edition == 2015)
        .unwrap();
    assert_eq!(
        proof
            .require_owner(&signature.scope, &ProfileIdentity::current())
            .unwrap_err()
            .message,
        "Type receipt owning scope/profile mismatch"
    );
    require(&db, &signature, &parse).unwrap();
    assert!(
        !db.trace
            .events
            .iter()
            .any(|event| matches!(event.key, QueryKey::BodyStructure(_)))
    );
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(
        require(&db, &signature, &parse).unwrap_err().message,
        "Type receipt requires its published matching ExpansionEpoch"
    );
}
#[test]
fn genuine_generated_export_retains_actual_parent_record_and_original_tokens() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, signature, parse, _sources) = prepared_world(
            vec![
                (
                    "app/main.rs",
                    APP,
                    "#[macro_use(seed)]extern crate renamed;seed!();fn main()->i64{made()}",
                    edition,
                ),
                (
                    "dependency/lib.rs",
                    LIB,
                    "macro_rules! emit{()=>{#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{631_i64}}}}}emit!();",
                    edition,
                ),
            ],
            vec![SourceDependency {
                consumer: APP,
                alias: "renamed".into(),
                provider: LIB,
            }],
            true,
        );
        let InputValue::GeneratedSyntax(unit) = db
            .published_input(&InputKey::GeneratedSyntax(parse.content))
            .unwrap()
            .0
        else {
            panic!()
        };
        assert!(generated_parent::is_generated_definition(
            &db,
            unit.items.record()
        ));
        let crate::expand::identity::MacroIdentity::Declaration { definition } =
            unit.items.record().key().macro_identity
        else {
            panic!()
        };
        let InputValue::DefinitionSource(parent) = db
            .published_input(&InputKey::DefinitionSource(definition))
            .unwrap()
            .0
        else {
            panic!()
        };
        let InputValue::GeneratedSyntax(parent_unit) = db
            .published_input(&InputKey::GeneratedSyntax(parent.content))
            .unwrap()
            .0
        else {
            panic!()
        };
        assert_ne!(parent_unit.items.record().key(), unit.items.record().key());
        assert_eq!(
            parent_unit.definitions[&definition].item_kind,
            crate::source::manifest::ItemKind::Macro
        );
        require(&db, &signature, &parse).unwrap();
        assert!(
            !db.trace
                .events
                .iter()
                .any(|event| matches!(event.key, QueryKey::BodyStructure(_)))
        );
        db.begin_expansion_epoch(2).unwrap();
        assert_eq!(
            require(&db, &signature, &parse).unwrap_err().message,
            "Type receipt requires its published matching ExpansionEpoch"
        );
    }
}
