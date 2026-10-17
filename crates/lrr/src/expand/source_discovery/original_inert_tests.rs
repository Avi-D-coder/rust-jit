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
fn unchanged_header_array_original_seals_builtin_allow_without_proof_value_and_type_all_editions() {
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
            // A builtin inert root attribute is never a macro call: the scan
            // records no producer and discovery publishes no attribute proof.
            assert!(input.nested_inert_proofs.is_empty());
            assert!(d.original_prefix_jobs.is_empty());
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
fn builtin_inert_root_attribute_is_not_a_producer_job() {
    let (_, view) = prepare(ORIGINAL, 2024, CfgSet::guest_default(), &[]);
    let d = discovery(&view, Namespace::Type, true);
    let scan = view.declaration_inventory(Default::default()).unwrap();
    let ids = original_inert::capture(&view, &scan, 32 * 1024 * 1024).unwrap();
    assert!(ids.is_empty());
    assert!(d.original_prefix_jobs.is_empty());
    assert!(d.nested_inert_proofs.is_empty());
    assert!(d.seal.is_none());
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
    // The bang-only `allow!` neither shadows nor proves the builtin inert
    // attribute, which needs no producer or proof.
    assert!(input.nested_inert_proofs.is_empty());
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
fn cfg_attr_selected_builtin_meta_and_disabled_root_have_no_proof() {
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
    // The cfg_attr-selected `allow(dead_code)` is builtin inert: no proof.
    assert!(input.nested_inert_proofs.is_empty());
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

#[path = "body_macro_environment_tests.rs"]
mod body_macro_environment_tests;

#[path = "original_module_staging_tests.rs"]
mod original_module_staging_tests;

#[path = "completed_source_environment_tests.rs"]
mod completed_source_environment_tests;
#[path = "item_boundary_tests.rs"]
mod item_boundary_tests;
