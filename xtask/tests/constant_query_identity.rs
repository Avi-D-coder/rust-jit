//! T054 query-boundary adapters. These are component controls, not stock passes.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    frontend::constants::{
        ConstLimits, evaluate, memo::ConstMemoPayload, query_identity::ConstQueryIdentity,
        wire::ConstantWireType,
    },
    ir::op::PrimitiveType,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        pure_constants::{ConstantPlan, ConstantSource},
        spans::SourceRange,
    },
};
fn length_plan(edition: u16, owner: u8, cfg: CfgSet) -> ConstantPlan {
    let text = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/consts/arithmetic-expr-in-array-len.rs"
    );
    let start = u32::try_from(text.find("2*4").unwrap()).unwrap();
    ConstantSource::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
        Digest([owner; 32]),
        cfg,
    )
    .unwrap()
    .array_length(SourceRange {
        start,
        end: start + 3,
    })
    .unwrap()
}
fn payload(plan: &ConstantPlan) -> ConstMemoPayload {
    ConstMemoPayload::capture(plan, &evaluate(plan, ConstLimits::default()).unwrap(), 1024).unwrap()
}
#[test]
fn exact_upstream_length_payload_matches_selected_query_in_all_editions() {
    let mut keys = std::collections::BTreeSet::new();
    for edition in [2015, 2018, 2021, 2024] {
        let plan = length_plan(edition, 184, CfgSet::guest_default());
        let value = payload(&plan);
        let key = ConstQueryIdentity::capture(
            &plan,
            &ConstantWireType::Scalar(PrimitiveType::Usize),
            Digest([185; 32]),
        )
        .unwrap();
        assert!(key.matches_payload(&value));
        assert!(keys.insert(key.fingerprint()));
        assert_eq!(key.selection(), plan.identity());
        assert!(key.metadata_bytes().unwrap() >= std::mem::size_of_val(&key));
    }
}
#[test]
fn equal_value_from_other_source_owner_or_edition_cannot_match_query() {
    let plan = length_plan(2024, 184, CfgSet::guest_default());
    let key = ConstQueryIdentity::capture(
        &plan,
        &ConstantWireType::Scalar(PrimitiveType::Usize),
        Digest([185; 32]),
    )
    .unwrap();
    for other in [
        length_plan(2024, 186, CfgSet::guest_default()),
        length_plan(2015, 184, CfgSet::guest_default()),
    ] {
        assert_eq!(
            payload(&other).decode_value().unwrap().scalar(),
            payload(&plan).decode_value().unwrap().scalar()
        );
        assert!(!key.matches_payload(&payload(&other)));
    }
}
#[test]
fn same_bits_with_different_concrete_query_type_are_rejected() {
    let plan = length_plan(2024, 184, CfgSet::guest_default());
    let wrong = ConstQueryIdentity::capture(
        &plan,
        &ConstantWireType::Scalar(PrimitiveType::U64),
        Digest([185; 32]),
    )
    .unwrap();
    assert!(!wrong.matches_payload(&payload(&plan)));
}
#[test]
fn changed_admission_fingerprint_changes_key_without_claiming_authority() {
    let plan = length_plan(2024, 184, CfgSet::guest_default());
    let ty = ConstantWireType::Scalar(PrimitiveType::Usize);
    let a = ConstQueryIdentity::capture(&plan, &ty, Digest([185; 32])).unwrap();
    let b = ConstQueryIdentity::capture(&plan, &ty, Digest([187; 32])).unwrap();
    assert_ne!(a, b);
    assert_ne!(a.fingerprint(), b.fingerprint());
    // Payload content is independent of authority. The coordinator/provider
    // still has to prove the supplied admission matches current published facts.
    assert!(a.matches_payload(&payload(&plan)));
    assert!(b.matches_payload(&payload(&plan)));
}
#[test]
fn empty_upstream_array_preserves_exact_element_type_in_query_key() {
    // The selected A initializer is a component witness in a compile-fail file.
    // B's diagnostic and main are not exercised by this adapter.
    let text = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/array_const_index-1.rs"
    );
    let source = ConstantSource::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
        Digest([188; 32]),
        CfgSet::guest_default(),
    )
    .unwrap();
    let plan = source
        .constant(
            source
                .declarations()
                .iter()
                .find(|d| d.name() == "A")
                .unwrap()
                .range(),
        )
        .unwrap();
    let value = payload(&plan);
    let right = ConstQueryIdentity::capture(&plan, value.value_type(), Digest([185; 32])).unwrap();
    let wrong_ty = ConstantWireType::Array {
        element: Box::new(ConstantWireType::Scalar(PrimitiveType::U32)),
        length: 0,
    };
    let wrong = ConstQueryIdentity::capture(&plan, &wrong_ty, Digest([185; 32])).unwrap();
    assert!(right.matches_payload(&value));
    assert!(!wrong.matches_payload(&value));
    assert_ne!(right.fingerprint(), wrong.fingerprint());
}
#[test]
fn oversized_concrete_shape_is_rejected_before_key_construction() {
    let plan = length_plan(2024, 184, CfgSet::guest_default());
    let ty = ConstantWireType::Array {
        element: Box::new(ConstantWireType::Scalar(PrimitiveType::U64)),
        length: u64::MAX,
    };
    assert_eq!(
        ConstQueryIdentity::capture(&plan, &ty, Digest([185; 32]))
            .unwrap_err()
            .code,
        "LRR-E-CONST-WIRE"
    );
}

#[test]
fn different_cfg_cannot_reuse_equal_upstream_constant_payload() {
    let plain = length_plan(2024, 184, CfgSet::guest_default());
    let mut changed_cfg = CfgSet::guest_default();
    changed_cfg
        .atoms
        .insert("lrr_query_identity_control".into());
    let changed = length_plan(2024, 184, changed_cfg);
    let key = ConstQueryIdentity::capture(
        &plain,
        &ConstantWireType::Scalar(PrimitiveType::Usize),
        Digest([185; 32]),
    )
    .unwrap();
    assert_eq!(
        payload(&plain).decode_value().unwrap().scalar(),
        payload(&changed).decode_value().unwrap().scalar()
    );
    assert!(!key.matches_payload(&payload(&changed)));
}

#[test]
fn equal_recursive_upstream_results_retain_distinct_concrete_arguments() {
    use lrr::{
        frontend::constants::ConstValue,
        runtime::{shape::TypeId, value::ValueRepr},
    };
    let text = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/consts/control-flow/basics.rs"
    );
    let source = ConstantSource::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
        Digest([189; 32]),
        CfgSet::guest_default(),
    )
    .unwrap();
    let range = source
        .declarations()
        .iter()
        .find(|decl| decl.name() == "gcd")
        .unwrap()
        .range();
    let plans = [[48, 18], [18, 48]].map(|args| {
        source
            .const_call(
                range,
                args.into_iter()
                    .map(|n| {
                        ConstValue::from_scalar(ValueRepr::unsigned(TypeId(11), n).unwrap())
                            .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
    });
    let values = plans.each_ref().map(payload);
    assert_eq!(
        values[0].decode_value().unwrap().scalar(),
        values[1].decode_value().unwrap().scalar()
    );
    let keys = plans.each_ref().map(|plan| {
        ConstQueryIdentity::capture(plan, values[0].value_type(), Digest([185; 32])).unwrap()
    });
    assert_ne!(keys[0], keys[1]);
    assert!(keys[0].matches_payload(&values[0]));
    assert!(!keys[0].matches_payload(&values[1]));
}
