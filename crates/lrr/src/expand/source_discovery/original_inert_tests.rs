//! Genuine coordinator discovery/publication of exact original root Meta proofs.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::{
        dependency::{InputKey, InputValue},
        key::OutlineKey,
        memo::Database,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;
const ORIGINAL: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-const-eval.rs"
);
const SHADOW: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/shadow_builtin_macros.rs"
);
fn prepare(
    text: &str,
    edition: u16,
    cfg: CfgSet,
    module: &[&str],
) -> (Database, PreparedSourceView) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("original-inert.rs").unwrap();
    let unit = Digest::of_bytes(b"source-proven original root prefix");
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let module_path: Vec<_> = module.iter().map(|s| (*s).into()).collect();
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
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("genuine original source absent")
    };
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    (db, view)
}
fn discovery(view: &PreparedSourceView, namespace: Namespace, compiler: bool) -> SourceDiscovery {
    let d = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        namespace,
        Digest::of_bytes(b"original prefix source world"),
        1,
    )
    .unwrap();
    if compiler {
        d.with_declarative_compiler(
            Digest::of_bytes(b"original prefix compile environment"),
            crate::expand::source_declarative::engine_version(),
            Arc::new(
                crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                    &view.selection().outline.parse.profile,
                )
                .unwrap(),
            ),
        )
        .unwrap()
    } else {
        d.with_declarative(
            Digest::of_bytes(b"original prefix compile environment"),
            crate::expand::source_declarative::engine_version(),
        )
        .unwrap()
    }
}
fn seal(d: &mut SourceDiscovery) -> Seal {
    loop {
        match d.poll().unwrap() {
            SourceDiscoveryStep::Sealed(seal) => return seal,
            SourceDiscoveryStep::NeedExpansion { request } => d
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => d
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Incomplete { diagnostic, .. } => panic!(
                "actual original discovery incomplete: {} {}",
                diagnostic.code, diagnostic.message
            ),
            _ => panic!("actual original source requires unestablished macro authority"),
        }
    }
}
#[test]
fn unchanged_header_array_original_publishes_actual_allow_proof_value_and_type_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view) = prepare(ORIGINAL, edition, CfgSet::guest_default(), &[]);
        for namespace in [Namespace::Value, Namespace::Type] {
            let mut d = discovery(&view, namespace, true);
            let sealed = seal(&mut d);
            let before = db.trace.events.len();
            d.publish(&mut db).unwrap();
            let Some((InputValue::DiscoverySeal(input), _)) =
                db.published_input(&InputKey::DiscoverySeal(sealed.key()))
            else {
                panic!("genuine scoped publication absent")
            };
            assert_eq!(input.nested_inert_proofs.len(), 1);
            let proof = &input.nested_inert_proofs[0];
            assert_eq!(proof.name().unwrap(), "allow");
            assert!(proof.attribute_facet().is_some());
            assert_eq!(proof.scan().path(), &view.selection().path);
            assert_eq!(proof.scan().parse(), &view.selection().outline.parse);
            assert_eq!(proof.scan().scope(), &view.outline().root_scope);
            let row = proof
                .scan()
                .producers()
                .iter()
                .find(|p| p.identity == proof.producer())
                .unwrap();
            assert_eq!(
                &ORIGINAL[row.range.start as usize..row.range.end as usize],
                "allow(dead_code)"
            );
            assert!(d.original_prefix_jobs.contains(&proof.producer()));
            assert_eq!(d.nested_jobs.get(&proof.producer()), Some(&0));
            proof
                .revalidate(
                    view.captured_macro_blob().unwrap(),
                    view.captured_macro_cfg().unwrap(),
                )
                .unwrap();
            assert_eq!(db.trace.events.len(), before);
            assert!(input.records.is_empty());
            assert!(input.runtime_no_item_proofs.is_empty());
            // Original ARR/add remain unevaluated; this is preparation evidence,
            // not array length execution or a full run-pass fixture claim.
            assert!(
                view.outline()
                    .items
                    .iter()
                    .any(|i| i.def_key.declared_name == "ARR")
            );
        }
    }
}
#[test]
fn original_root_attribute_is_an_actual_job_not_a_delegated_completion() {
    let (_, view) = prepare(ORIGINAL, 2024, CfgSet::guest_default(), &[]);
    let d = discovery(&view, Namespace::Type, true);
    let scan = view.declaration_inventory(Default::default()).unwrap();
    let ids = original_inert::capture(&view, &scan, 32 * 1024 * 1024).unwrap();
    assert_eq!(ids.len(), 1);
    for id in ids {
        assert!(d.original_prefix_jobs.contains(&id));
        assert!(d.nested_jobs.contains_key(&id));
        assert!(d.nested_sources[&0].producer(id).is_ok());
        assert!(!d.nested_inert_proofs.contains_key(&id));
    }
    assert!(d.seal.is_none());
}
#[test]
fn missing_current_compiler_attribute_facet_cannot_seal_original_prefix() {
    let (mut db, view) = prepare(ORIGINAL, 2024, CfgSet::guest_default(), &[]);
    let mut d = discovery(&view, Namespace::Value, false);
    let id = *d.original_prefix_jobs.first().unwrap();
    let SourceDiscoveryStep::Incomplete { blocked, .. } = d.poll().unwrap() else {
        panic!("missing actual Attribute facet must remain a prerequisite")
    };
    assert!(blocked.iter().any(|(_, inputs)| inputs.contains(&id)));
    assert!(d.nested_inert_proofs.is_empty());
    assert!(d.publish(&mut db).is_err());
}
#[test]
fn bang_only_allow_definition_does_not_supply_competing_attribute_authority() {
    let text = "#![allow(dead_code)]macro_rules! allow{()=>{}}const X:usize=1;fn main(){}";
    let (mut db, view) = prepare(text, 2024, CfgSet::guest_default(), &[]);
    let mut d = discovery(&view, Namespace::Type, true);
    let sealed = seal(&mut d);
    d.publish(&mut db).unwrap();
    let Some((InputValue::DiscoverySeal(input), _)) =
        db.published_input(&InputKey::DiscoverySeal(sealed.key()))
    else {
        panic!()
    };
    assert_eq!(input.nested_inert_proofs.len(), 1);
    assert_eq!(input.nested_inert_proofs[0].name().unwrap(), "allow");
    assert!(input.nested_inert_proofs[0].attribute_facet().is_some());
}
#[test]
fn genuine_prelude_imported_compiler_attribute_shadow_cannot_be_retired_as_allow() {
    let text = "#![allow(dead_code)]extern crate core;use core::prelude::v1::test as allow;const X:usize=1;fn main(){}";
    for edition in [2015, 2018, 2021, 2024] {
        let (_, view) = prepare(text, edition, CfgSet::guest_default(), &[]);
        let mut d = discovery(&view, Namespace::Type, true);
        let providers = &d.declarative.as_ref().unwrap().2;
        let names = SourceDiscovery::install_macro_routes_with_modules(
            &mut d.graph,
            providers,
            &d.module_routes,
            &d.nested_sources,
            &d.nested_inert_proofs,
        )
        .unwrap();
        let producer = *d.original_prefix_jobs.first().unwrap();
        let scan = d.nested_sources[&0].scan();
        let crate::frontend::macro_names::PathLookup::Defined(definition) =
            crate::service::source_attribute_requests::SourceAttributeSelection::cold_lookup(
                &view, scan, producer, &names,
            )
            .unwrap()
        else {
            panic!("actual prelude import must select its compiler Attribute definition")
        };
        let catalog = providers
            .iter()
            .find(|p| p.module_source().same_publication(&view))
            .unwrap()
            .compiler
            .as_ref()
            .unwrap();
        let descriptor = catalog
            .attribute_catalog()
            .unwrap()
            .select(&definition)
            .unwrap()
            .unwrap();
        assert_eq!(
            descriptor.action(),
            crate::expand::builtin_attributes::BuiltinAttributeAction::Test
        );
        assert_eq!(
            definition.defining_unit,
            crate::facade::unit(&view.outline().profile, "core")
        );
        assert_eq!(definition.definition_edition, edition);
        assert_eq!(descriptor.name(), "test");
        let error = d
            .poll()
            .err()
            .expect("actual selected Test Attribute facet conflicts with inert allow");
        assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
        assert!(d.nested_inert_proofs.is_empty());
        assert!(d.seal.is_none());
    }
}
#[test]
fn unavailable_core_root_test_preserves_exact_pending_import_frontier() {
    // Actual canonical singleton 40806/59fbfe/101 on 0438a468 disclosed this
    // precise Incomplete step, rather than selecting a root Test export.
    let text =
        "#![allow(dead_code)]extern crate core;use core::test as allow;const X:usize=1;fn main(){}";
    let (mut db, view) = prepare(text, 2024, CfgSet::guest_default(), &[]);
    let mut d = discovery(&view, Namespace::Type, true);
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = d.poll().unwrap()
    else {
        panic!("unavailable root Test route must retain its observed prerequisite")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(
        diagnostic
            .message
            .contains("fixed point retains unresolved import or macro producers")
    );
    assert_eq!(blocked.len(), 2);
    assert!(blocked.iter().all(|(_, inputs)| !inputs.is_empty()));
    let prefix = *d.original_prefix_jobs.first().unwrap();
    // DiscoveryDriver returns the producer identity as the first field;
    // the second field contains the inputs that this exact job is awaiting.
    let (_, waits) = blocked
        .iter()
        .find(|(job, _)| *job == prefix)
        .expect("the actual original prefix producer must remain blocked");
    assert!(!waits.is_empty());
    assert!(d.nested_inert_proofs.is_empty());
    assert!(d.seal.is_none());
    assert!(d.publish(&mut db).is_err());
}
#[test]
fn foreign_compiler_profile_cannot_mint_source_prefix_facet_proof() {
    let (_, view) = prepare(ORIGINAL, 2024, CfgSet::guest_default(), &[]);
    let mut provider =
        crate::expand::source_declarative::SourceDeclarative::new(view.share()).unwrap();
    provider.compiler = Some(Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(&ProfileIdentity {
            edition: 2015,
            ..ProfileIdentity::current()
        })
        .unwrap(),
    ));
    let e = original_inert::matched_attribute_facet(&view, &provider)
        .err()
        .unwrap();
    assert_eq!(e.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(e.message.contains("facet/profile"));
}
#[test]
fn copied_source_cfg_and_producer_cannot_replay_original_prefix_proof() {
    let (_, view) = prepare(ORIGINAL, 2024, CfgSet::guest_default(), &[]);
    let mut d = discovery(&view, Namespace::Value, true);
    seal(&mut d);
    let proof = d.nested_inert_proofs.values().next().unwrap();
    let (_, changed) = prepare(
        "#![allow(unused)]const ARR:usize=1;fn main(){}",
        2024,
        CfgSet::guest_default(),
        &[],
    );
    assert!(original_inert::capture(&changed, proof.scan(), 32 * 1024 * 1024).is_err());
    assert!(
        proof
            .revalidate(
                changed.captured_macro_blob().unwrap(),
                changed.captured_macro_cfg().unwrap()
            )
            .is_err()
    );
    let mut changed_cfg = CfgSet::guest_default();
    changed_cfg.atoms.insert("changed".into());
    let (_, changed) = prepare(ORIGINAL, 2024, changed_cfg, &[]);
    assert!(original_inert::capture(&changed, proof.scan(), 32 * 1024 * 1024).is_err());
    let (_, scoped) = prepare(ORIGINAL, 2024, CfgSet::guest_default(), &["owned"]);
    assert!(original_inert::capture(&scoped, proof.scan(), 32 * 1024 * 1024).is_err());
}
#[test]
fn actual_epoch_refuses_old_preparation_publication_without_source_advance() {
    let (mut db, view) = prepare(ORIGINAL, 2024, CfgSet::guest_default(), &[]);
    let mut d = discovery(&view, Namespace::Type, true);
    seal(&mut d);
    let revision = db.source_revision();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(db.source_revision(), revision);
    assert!(d.publish(&mut db).is_err());
}
#[test]
fn cfg_attr_selects_exact_original_meta_while_disabled_root_has_no_proof() {
    let text = "#![cfg_attr(all(),allow(dead_code))]const X:usize=1;fn main(){}";
    let (mut db, view) = prepare(text, 2024, CfgSet::guest_default(), &[]);
    let mut d = discovery(&view, Namespace::Value, true);
    let sealed = seal(&mut d);
    d.publish(&mut db).unwrap();
    let Some((InputValue::DiscoverySeal(input), _)) =
        db.published_input(&InputKey::DiscoverySeal(sealed.key()))
    else {
        panic!()
    };
    assert_eq!(input.nested_inert_proofs.len(), 1);
    let p = &input.nested_inert_proofs[0];
    let row = p
        .scan()
        .producers()
        .iter()
        .find(|r| r.identity == p.producer())
        .unwrap();
    assert_eq!(
        &text[row.range.start as usize..row.range.end as usize],
        "allow(dead_code)"
    );
    let (_, view) = prepare(
        "#![cfg(any())]#![allow(dead_code)]const X:usize=1;",
        2024,
        CfgSet::guest_default(),
        &[],
    );
    let scan = view.declaration_inventory(Default::default()).unwrap();
    assert!(
        original_inert::capture(&view, &scan, 32 * 1024 * 1024)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn metadata_refusal_keeps_original_attribute_unproven() {
    let (_, view) = prepare(ORIGINAL, 2024, CfgSet::guest_default(), &[]);
    let scan = view.declaration_inventory(Default::default()).unwrap();
    let e = original_inert::capture(&view, &scan, 128).unwrap_err();
    assert_eq!(e.code, "LRR-E-METADATA-LIMIT");
    let mut d = discovery(&view, Namespace::Type, true);
    d.nested_bytes = 512 * 1024 * 1024;
    assert!(d.poll().is_err());
    assert!(d.nested_inert_proofs.is_empty());
    assert!(d.seal.is_none());
}
#[test]
fn unchanged_user_macro_source_is_not_a_root_attribute_proof() {
    // Original shadow fixture has bang producers and aux imports; no full seal
    // or macro execution is asserted by this root-prefix component control.
    let (_, view) = prepare(SHADOW, 2024, CfgSet::guest_default(), &[]);
    let scan = view.declaration_inventory(Default::default()).unwrap();
    assert!(
        original_inert::capture(&view, &scan, 32 * 1024 * 1024)
            .unwrap()
            .is_empty()
    );
    let row = scan
        .producers()
        .iter()
        .find(|r| r.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro)
        .unwrap();
    assert!(
        crate::expand::nested_inert::NestedInertProof::select(scan.clone(), row.identity)
            .unwrap()
            .is_none()
    );
}

#[path = "retained_original_inert_tests.rs"]
mod retained_replay_tests;

#[path = "body_macro_environment_tests.rs"]
mod body_macro_environment_tests;

#[path = "original_module_staging_tests.rs"]
mod original_module_staging_tests;
