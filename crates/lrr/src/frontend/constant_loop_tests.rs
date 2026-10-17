//! Upstream source selections plus LRR control/budget regressions, not fixture passes.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::{dependency::InputKey, key::OutlineKey, memo::Database},
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::constant_source_admission::{ConstantRequest, ConstantSourceAdmission},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/control-flow/loop.rs"
);
fn source(text: &str, edition: u16) -> ConstantSource {
    ConstantSource::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
        Digest([213; 32]),
        CfgSet::guest_default(),
    )
    .unwrap()
}
fn constant(text: &str, name: &str, edition: u16) -> ConstantPlan {
    let source = source(text, edition);
    let at = source
        .declarations()
        .iter()
        .find(|d| d.name() == name)
        .unwrap()
        .range();
    source.constant(at).unwrap()
}
// Genuine current preparation mints the root inert-attribute evidence. No test
// constructor issues an inert-attribute, primitive, body or completion proof.
fn prepared_array_source(text: &str, edition: u16) -> (Database, ConstantSource) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("original-loop-array-length.rs").unwrap();
    let unit = Digest([213; 32]);
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile: profile.clone(),
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("original source missing")
    };
    let item = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap();
    let definition = item.def_key.fingerprint();
    let scope = item.scope.clone();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    for namespace in [Namespace::Value, Namespace::Type] {
        let mut discovery = SourceDiscovery::new(
            vec![view.share()],
            scope.clone(),
            namespace,
            Digest::of_bytes(b"LRR actual array-length original preparation"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"array original declarative"),
            Digest::of_bytes(b"array original compiler"),
            std::sync::Arc::new(
                crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(&profile)
                    .unwrap(),
            ),
        )
        .unwrap();
        loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::Sealed(_) => break,
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Incomplete { diagnostic, .. } => {
                    panic!("{} {}", diagnostic.code, diagnostic.message)
                }
                _ => panic!("unestablished original array-length authority"),
            }
        }
        discovery.publish(&mut db).unwrap();
    }
    let at = crate::syntax::pure_constants::fixture_length_sites(text, "main", edition)[0];
    let admission = ConstantSourceAdmission::capture_current(
        &db,
        &view,
        definition,
        ConstantRequest::ArrayLength(at),
        32 * 1024 * 1024,
    )
    .unwrap();
    admission.revalidate(&db, &view).unwrap();
    assert!(
        admission
            .dependencies()
            .contains_key(&InputKey::DefinitionBody(definition))
    );
    let source = admission.plan().source().clone();
    (db, source)
}
#[test]
fn original_zero_iteration_while_array_length_keeps_source_and_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (_db, source) = prepared_array_source(ORIGINAL, edition);
        let sites = crate::syntax::pure_constants::fixture_length_sites(ORIGINAL, "main", edition);
        assert_eq!(sites.len(), 1);
        let plan = source.array_length(sites[0]).unwrap();
        let result = evaluate(&plan, ConstLimits::default()).unwrap();
        assert_eq!(
            result.value().scalar().unwrap(),
            ValueRepr::unsigned(TypeId(13), 4).unwrap()
        );
        assert_eq!(result.source(), Digest::of_bytes(ORIGINAL.as_bytes()));
        assert_eq!(plan.profile().edition, edition);
    }
}
#[test]
fn loop_break_value_uses_owning_result_context_and_skips_the_suffix() {
    let text = "const A:u8=loop { break 7; missing(); };";
    for edition in [2015, 2018, 2021, 2024] {
        let result = evaluate(&constant(text, "A", edition), ConstLimits::default()).unwrap();
        assert_eq!(
            result.value().scalar().unwrap(),
            ValueRepr::unsigned(TypeId(9), 7).unwrap()
        );
    }
}
#[test]
fn nested_labelled_break_reaches_its_exact_outer_loop() {
    let text = "const A:i32='outer: loop { loop { break 'outer 9; } };";
    assert_eq!(
        evaluate(&constant(text, "A", 2024), ConstLimits::default())
            .unwrap()
            .value()
            .scalar()
            .unwrap(),
        ValueRepr::signed(TypeId(6), 9).unwrap()
    );
}
#[test]
fn break_in_while_condition_exits_the_selected_loop_without_body_demand() {
    let text = "const A:i32={ 'a: while break 'a {} 5 };";
    assert_eq!(
        evaluate(&constant(text, "A", 2024), ConstLimits::default())
            .unwrap()
            .value()
            .scalar()
            .unwrap(),
        ValueRepr::signed(TypeId(6), 5).unwrap()
    );
}
#[test]
fn return_inside_loop_uses_const_function_return_context() {
    let text = "const fn f()->u8 { loop { return 17; } } const A:u8=f();";
    assert_eq!(
        evaluate(&constant(text, "A", 2024), ConstLimits::default())
            .unwrap()
            .value()
            .scalar()
            .unwrap(),
        ValueRepr::unsigned(TypeId(9), 17).unwrap()
    );
}
#[test]
fn while_break_value_and_wrong_loop_result_remain_reached_type_errors() {
    for text in [
        "const A:i32={ while true { break 3; } 5 };",
        "const A:u8=loop { break true; };",
    ] {
        assert_eq!(
            evaluate(&constant(text, "A", 2024), ConstLimits::default())
                .unwrap_err()
                .diagnostic()
                .code,
            "LRR-E-CONST-TYPE"
        );
    }
}
#[test]
fn labelled_block_shadow_cannot_be_skipped_to_authorize_outer_loop_exit() {
    let text = "const A:i32='a: loop { 'a: { break 'a; } break 'a 3; };";
    for edition in [2015, 2018, 2021, 2024] {
        let result = evaluate(&constant(text, "A", edition), ConstLimits::default()).unwrap();
        assert_eq!(
            result.value().scalar().unwrap(),
            ValueRepr::signed(TypeId(6), 3).unwrap()
        );
    }
}
#[test]
fn a_function_body_cannot_break_a_callers_loop() {
    let text = "const fn f()->i32 { break 'a 3; } const A:i32='a: loop { f(); };";
    let error = evaluate(&constant(text, "A", 2024), ConstLimits::default()).unwrap_err();
    assert_eq!(error.diagnostic().code, "LRR-E-CONST-SOURCE");
}
