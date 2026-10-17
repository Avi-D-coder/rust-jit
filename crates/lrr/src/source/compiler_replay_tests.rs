//! Pure source-replay controls over unchanged actual core annotations.
//! No currentness/namespace/facet/runtime authority is minted by these tests.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::key::OutlineKey,
    source::{
        changes::{PreparedSourceInputs, SourceSelectionOutcome, SourceUnitInput},
        snapshot::SourcePath,
    },
    syntax::{declaration_scan::DeclarationProducerKind, parse::ParseKey},
};
const CORE: &[u8] =
    include_bytes!("../../../../fixtures/upstream/rustc-checkout/library/core/src/macros/mod.rs");
fn captured(unit: Digest, cfg: CfgSet, edition: u16) -> PreparedSourceView {
    let path = SourcePath::new("src/macros/mod.rs").unwrap();
    let blob = SourceBlob::new(CORE.to_vec()).unwrap();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let module_path = vec!["macros".into()];
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: module_path.clone(),
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path,
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap() else {
        panic!("unchanged core source was not captured");
    };
    source
}
#[test]
fn retained_core_annotation_replay_uses_independent_scan_owner_and_same_kernel() {
    let source = captured(Digest([201; 32]), CfgSet::guest_default(), 2024);
    // The unbounded scan is a pure function of the immutable publication, so
    // both demands share its one retained owner; the replay still retains
    // exactly the scan handed to it rather than re-scanning.
    let proof_scan = source.declaration_inventory(Default::default()).unwrap();
    let actual_scan = source.declaration_inventory(Default::default()).unwrap();
    assert!(Arc::ptr_eq(&proof_scan, &actual_scan));
    let replay = CompilerReplaySource::prepare(&source, &actual_scan).unwrap();
    assert!(Arc::ptr_eq(replay.scan(), &actual_scan));
    assert!(Arc::ptr_eq(
        replay.parser(),
        source.retained_declaration_parser().unwrap()
    ));
    assert_eq!(replay.selection(), source.selection());
    assert!(replay.source().same_publication(&source));
    assert_eq!(replay.blob().bytes(), CORE);
    assert_eq!(replay.cfg(), source.captured_macro_cfg().unwrap());
    assert_eq!(
        replay.exclusive_metadata_bytes(),
        std::mem::size_of::<CompilerReplaySource<'_>>()
    );
    // Builtin inert core annotations (`macro_export`, `rustc_*`, `stable`)
    // are not declaration producers, so no inert proof replays over them.
    assert!(proof_scan.producers().iter().all(|row| {
        row.kind != DeclarationProducerKind::Attribute
            || row.unqualified_attribute.as_deref() != Some("macro_export")
    }));
}
#[test]
fn retained_core_replay_rejects_wrong_source_profile_and_current_cfg() {
    let unit = Digest([202; 32]);
    let source = captured(unit, CfgSet::guest_default(), 2024);
    let old_scan = source.declaration_inventory(Default::default()).unwrap();
    let wrong_unit = captured(Digest([203; 32]), CfgSet::guest_default(), 2024);
    assert!(CompilerReplaySource::prepare(&wrong_unit, &old_scan).is_err());
    let wrong_profile = captured(unit, CfgSet::guest_default(), 2021);
    assert!(CompilerReplaySource::prepare(&wrong_profile, &old_scan).is_err());
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("lrr_replay_current_cfg_control".into());
    let changed = captured(unit, cfg, 2024);
    assert!(CompilerReplaySource::prepare(&changed, &old_scan).is_err());
    let actual_changed_scan = changed.declaration_inventory(Default::default()).unwrap();
    CompilerReplaySource::prepare(&changed, &actual_changed_scan).unwrap();
}
