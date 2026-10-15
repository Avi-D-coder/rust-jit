//! LRR authority controls; the unchanged float-nan main is a separate runtime gate.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    facade::primitive_float_methods::{PrimitiveFloatMethodBody, PrimitiveFloatReceiverKind},
    frontend::compile_input_catalog::CompileInputCatalog,
    ir::op::PrimitiveType,
};
fn profile(edition: u16) -> ProfileIdentity {
    ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    }
}
#[test]
fn actual_standard_compiler_publishes_exact_closed_float_members_all_editions() {
    let mut member_ids = BTreeSet::new();
    for edition in [2015, 2018, 2021, 2024] {
        let profile = profile(edition);
        let compiler = CompileInputCatalog::standalone(&profile).unwrap();
        let runtime = compiler.runtime_catalog().unwrap().unwrap();
        let macro_only = RuntimeMacroCatalog::standard(&profile).unwrap();
        assert_eq!(
            runtime.definitions().collect::<Vec<_>>(),
            macro_only.definitions().collect::<Vec<_>>()
        );
        assert_ne!(runtime.fingerprint(), macro_only.fingerprint());
        assert_eq!(
            runtime.fingerprint(),
            compiler.runtime_catalog().unwrap().unwrap().fingerprint()
        );
        for receiver in [PrimitiveType::F32, PrimitiveType::F64] {
            let member = runtime.primitive_float_method(receiver, "is_nan").unwrap();
            assert!(member_ids.insert(member.identity()));
            assert_eq!(member.receiver(), receiver);
            assert_eq!(member.receiver_kind(), PrimitiveFloatReceiverKind::Value);
            assert_eq!(member.result(), PrimitiveType::Bool);
            assert_eq!(member.body(), PrimitiveFloatMethodBody::SelfNotEqual);
            member.require_profile(&profile).unwrap();
            assert!(member.allocated_bytes().unwrap() < 4096);
            runtime.require_primitive_float_method(member).unwrap();
            let cloned = member.clone();
            runtime.require_primitive_float_method(&cloned).unwrap();
            assert!(macro_only.require_primitive_float_method(member).is_err());
        }
        for receiver in [PrimitiveType::Bool, PrimitiveType::I32, PrimitiveType::U64] {
            assert!(runtime.primitive_float_method(receiver, "is_nan").is_none());
        }
        for name in ["is_NaN", "is_infinite", "r#is_nan", "is_nan::", ""] {
            assert!(
                runtime
                    .primitive_float_method(PrimitiveType::F64, name)
                    .is_none()
            );
        }
        assert!(compiler.owning_profile_metadata_bytes().unwrap() < 64 * 1024);
    }
    assert_eq!(member_ids.len(), 8);
}
#[test]
fn manual_and_macro_only_catalogues_never_acquire_float_members() {
    for edition in [2015, 2018, 2021, 2024] {
        let profile = profile(edition);
        let core = crate::facade::unit(&profile, "core");
        let raw_compiler = CompileInputCatalog::new(core, edition, []).unwrap();
        assert!(raw_compiler.runtime_catalog().unwrap().is_none());
        let actual = CompileInputCatalog::standalone(&profile)
            .unwrap()
            .runtime_catalog()
            .unwrap()
            .unwrap();
        let member = actual
            .primitive_float_method(PrimitiveType::F64, "is_nan")
            .unwrap();
        for runtime in [
            RuntimeMacroCatalog::new(core, edition).unwrap(),
            RuntimeMacroCatalog::standard(&profile).unwrap(),
            RuntimeMacroCatalog::standard_for_profiles(
                &profile,
                &BTreeSet::from([profile.clone()]),
            )
            .unwrap(),
        ] {
            assert!(
                runtime
                    .primitive_float_method(PrimitiveType::F64, "is_nan")
                    .is_none()
            );
            assert!(runtime.require_primitive_float_method(member).is_err());
        }
        // Actual standard decoration cannot make an incomplete/custom descriptor
        // inventory canonical merely by installing the same public module names.
        let partial = raw_compiler.with_standard_facades(&profile).unwrap();
        assert!(partial.runtime_catalog().is_err());
    }
}
#[test]
fn copied_receipt_requires_actual_owning_profile_and_current_catalogue_membership() {
    let a = profile(2015);
    let b = profile(2024);
    let left = CompileInputCatalog::standalone(&a)
        .unwrap()
        .runtime_catalog()
        .unwrap()
        .unwrap();
    let right = CompileInputCatalog::standalone(&b)
        .unwrap()
        .runtime_catalog()
        .unwrap()
        .unwrap();
    let receipt = left
        .primitive_float_method(PrimitiveType::F32, "is_nan")
        .unwrap()
        .clone();
    assert!(receipt.require_profile(&b).is_err());
    assert!(right.require_primitive_float_method(&receipt).is_err());
    assert_ne!(left.fingerprint(), right.fingerprint());
    let compiler = CompileInputCatalog::standalone(&a).unwrap();
    assert!(
        RuntimeMacroCatalog::standard(&b)
            .unwrap()
            .with_primitive_float_methods(&compiler)
            .is_err()
    );
    let mut incompatible = a.clone();
    incompatible.semantics.push_str("-foreign");
    assert!(receipt.require_profile(&incompatible).is_err());
    assert!(CompileInputCatalog::standalone(&incompatible).is_err());
    incompatible = a;
    incompatible.edition = 2099;
    assert!(CompileInputCatalog::standalone(&incompatible).is_err());
}

#[test]
fn completed_profile_catalogue_retains_foreign_macro_selection_but_only_own_float_members() {
    let profiles = [2015, 2018, 2021, 2024]
        .into_iter()
        .map(profile)
        .collect::<BTreeSet<_>>();
    for owner in &profiles {
        let compiler = CompileInputCatalog::standalone(owner).unwrap();
        let completed = compiler.runtime_catalog_for_profiles(&profiles).unwrap();
        let macro_only = RuntimeMacroCatalog::standard_for_profiles(owner, &profiles).unwrap();
        assert_eq!(
            completed.definitions().collect::<Vec<_>>(),
            macro_only.definitions().collect::<Vec<_>>()
        );
        assert_ne!(completed.fingerprint(), macro_only.fingerprint());
        for source in &profiles {
            for definition in RuntimeMacroCatalog::standard(source).unwrap().definitions() {
                assert_eq!(
                    completed.select(definition).unwrap(),
                    macro_only.select(definition).unwrap()
                );
            }
            let own = CompileInputCatalog::standalone(source)
                .unwrap()
                .runtime_catalog()
                .unwrap()
                .unwrap();
            for primitive in [PrimitiveType::F32, PrimitiveType::F64] {
                let own_member = own.primitive_float_method(primitive, "is_nan").unwrap();
                let selected = completed
                    .primitive_float_method(primitive, "is_nan")
                    .unwrap();
                selected.require_profile(owner).unwrap();
                assert_eq!(
                    completed.require_primitive_float_method(own_member).is_ok(),
                    owner == source
                );
                if owner != source {
                    assert_ne!(selected.identity(), own_member.identity());
                }
            }
        }
        let mut missing_owner = profiles.clone();
        assert!(missing_owner.remove(owner));
        assert!(
            compiler
                .runtime_catalog_for_profiles(&missing_owner)
                .is_err()
        );
        let manual =
            CompileInputCatalog::new(crate::facade::unit(owner, "core"), owner.edition, [])
                .unwrap();
        assert!(manual.runtime_catalog_for_profiles(&profiles).is_err());
    }
}
