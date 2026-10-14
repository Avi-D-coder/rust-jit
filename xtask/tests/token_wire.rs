//! Frontend/runtime boundary exchanges only the shared owned schema.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    source::{
        manifest::{CrateKey, PackageKey, TargetKind},
        snapshot::{CfgSet, ProjectManifest, Snapshot, SourceBlob, SourcePath},
    },
    syntax::{spans::TokenSpanLease, token_wire::TokenForest},
};
use lrr_expand_adapter::token_wire::{
    ContextRecord, Edition, Key, SourceIdentity, Transparency, parse_source, transport_round_trip,
};
use std::{collections::BTreeMap, sync::Arc};
#[test]
fn owned_adapter_wire_attaches_to_runtime_snapshot_without_upstream_identity() {
    let cfg = CfgSet::guest_default();
    let key = CrateKey {
        package: PackageKey {
            source_identity: "token-fixture".into(),
            package_name: "token-fixture".into(),
            version: "0.2.0".into(),
        },
        target_kind: TargetKind::LibraryTests,
        target_name: "token-fixture".into(),
        guest_edition: 2024,
        semantics_profile: ProfileIdentity::current().fingerprint(),
        cfg_fingerprint: cfg.fingerprint(),
        dependency_aliases: Digest([0; 32]),
    };
    let unit = key.fingerprint();
    let path = SourcePath::new("generated.rs").unwrap();
    let blob = SourceBlob::new(
        "/// docs\nfn r#type<'life>() { let café = r##\"value\"##; }"
            .as_bytes()
            .to_vec(),
    )
    .unwrap();
    let source = SourceIdentity {
        unit: Key(unit.0),
        content: Key(blob.content().0),
        path: path.as_str().into(),
    };
    let snapshot = Arc::new(
        Snapshot::new(
            ProjectManifest {
                identity: "token-fixture".into(),
                root_source: path.clone(),
                selected_crate: unit,
                crates: BTreeMap::from([(unit, key)]),
            },
            BTreeMap::from([(path, blob.clone())]),
            BTreeMap::from([(unit, cfg)]),
            BTreeMap::new(),
            None,
        )
        .unwrap(),
    );
    let forest = parse_source(
        blob.text().unwrap(),
        source,
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key(unit.0),
            edition: Edition::E2024,
        },
        Key([22; 32]),
    )
    .unwrap();
    let transported = transport_round_trip(&forest).unwrap();
    let runtime = TokenForest::decode(&transported.encode().unwrap()).unwrap();
    assert_eq!(runtime.fingerprint().0, forest.fingerprint().0);
    assert_eq!(runtime.encode().unwrap(), forest.encode().unwrap());
    drop(forest);
    drop(transported);
    let lease = TokenSpanLease::new(snapshot.clone(), unit);
    let selected = runtime
        .spans()
        .iter()
        .find(|s| lease.source_text(s).is_ok_and(|t| t == "café"))
        .unwrap();
    assert_eq!(lease.source_text(selected).unwrap(), "café");
    assert_eq!(lease.attach(selected).unwrap().start.line, 2);
    assert_eq!(selected.origin.0, [22; 32]);
    let mut tampered = selected.clone();
    tampered.source.content = lrr::syntax::token_wire::Key([99; 32]);
    assert!(lease.attach(&tampered).is_err());
    drop(snapshot);
    drop(blob);
    assert_eq!(lease.source_text(selected).unwrap(), "café");
}
