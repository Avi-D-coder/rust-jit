//! Genuine published-row identity and distinct-copy fallback/currentness controls.
use super::*;
use crate::{
    db::{key::OutlineKey, seals::SealKey},
    frontend::{compile_input_catalog::CompileInputCatalog, names::NameLookup},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;
fn published(edition: u16) -> (Database, PreparedSourceView, SealKey) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let unit = SourceUnitInput {
        path: SourcePath::new("receipt-encoding.rs").unwrap(),
        crate_key: Digest([201; 32]),
        module_path: vec![],
        profile: profile.clone(),
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(
            b"mod child{pub fn value()->i64{1_i64}}fn main()->i64{child::value()}".to_vec(),
        )
        .unwrap(),
    };
    let selection = SourceSelection {
        path: unit.path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: unit.blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit.crate_key,
            module_path: vec![],
            cfg: unit.cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        vec![unit],
        Default::default(),
        vec![],
    )
    .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap() else {
        panic!()
    };
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Value,
        Digest([202; 32]),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest([203; 32]),
        crate::expand::source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&profile).unwrap()),
    )
    .unwrap();
    let SourceDiscoveryStep::Sealed(seal) = discovery.poll().unwrap() else {
        panic!("ordinary exact source preparation incomplete")
    };
    discovery.publish(&mut db).unwrap();
    (db, source, seal.key())
}
#[test]
fn actual_published_type_receipt_and_distinct_immutable_copy_replay_identically_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, source, key) = published(edition);
        let (InputValue::DiscoverySeal(current), _) =
            db.published_input(&InputKey::DiscoverySeal(key)).unwrap()
        else {
            panic!()
        };
        assert!(current.type_scopes.len() >= 2);
        let copy = current.as_ref().clone();
        assert!(!std::ptr::eq(current.as_ref(), &copy));
        assert_eq!(current.fingerprint(), copy.fingerprint());
        for proof in &current.type_scopes {
            assert_eq!(proof.parent, current.seal.fingerprint());
            assert_eq!(proof.fingerprint(), proof.clone().fingerprint());
            proof.require_current(&db, current).unwrap();
            proof.require_current(&db, &copy).unwrap();
            assert_eq!(
                proof.profile.edition,
                source.selection().outline.parse.profile.edition
            );
        }
        let mut altered = copy.clone();
        altered
            .scope
            .as_mut()
            .unwrap()
            .names
            .insert("not_a_published_name".into(), NameLookup::Absent);
        assert_ne!(altered.fingerprint(), current.fingerprint());
        let proof = &current.type_scopes[0];
        assert_eq!(
            proof.require_current(&db, &altered).unwrap_err().message,
            "Type receipt enclosing publication is not current"
        );
    }
}
#[test]
fn actual_same_publication_row_still_refuses_after_current_epoch_changes() {
    let (mut db, _source, key) = published(2024);
    // Retain the actual published allocation while the coordinator changes only
    // the separate epoch input. This specifically exercises the same-row path.
    let input = db.input(&InputKey::DiscoverySeal(key.clone())).unwrap();
    let InputValue::DiscoverySeal(publication) = input.as_ref() else {
        panic!()
    };
    let proof = &publication.type_scopes[0];
    proof.require_current(&db, publication).unwrap();
    db.begin_expansion_epoch(2).unwrap();
    let (InputValue::DiscoverySeal(current), _) =
        db.published_input(&InputKey::DiscoverySeal(key)).unwrap()
    else {
        panic!()
    };
    assert!(std::ptr::eq(current.as_ref(), publication.as_ref()));
    assert_eq!(
        proof.require_current(&db, publication).unwrap_err().message,
        "Type receipt requires its published matching ExpansionEpoch"
    );
}
