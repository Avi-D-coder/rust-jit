//! T054 cache/type regressions; originals retain every upstream assertion and diagnostic.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    frontend::constants::{
        ConstLimits, evaluate,
        memo::ConstMemoPayload,
        wire::{ConstantWireType, encode},
    },
    ir::op::PrimitiveType,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        pure_constants::{ConstantPlan, ConstantSource},
        spans::SourceRange,
    },
};
fn length_plan(edition: u16, crate_key: Digest) -> ConstantPlan {
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
        crate_key,
        CfgSet::guest_default(),
    )
    .unwrap()
    .array_length(SourceRange {
        start,
        end: start + 3,
    })
    .unwrap()
}
#[test]
fn memo_payload_preserves_upstream_length_type_and_all_owning_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let plan = length_plan(edition, Digest([179; 32]));
        let evaluation = evaluate(&plan, ConstLimits::default()).unwrap();
        let payload = ConstMemoPayload::capture(&plan, &evaluation, 1024).unwrap();
        assert!(matches!(
            evaluation.value_type(),
            ConstantWireType::Scalar(PrimitiveType::Usize)
        ));
        assert_eq!(payload.selection(), plan.identity());
        assert_eq!(payload.source(), evaluation.source());
        assert_eq!(
            payload.decode_value().unwrap().scalar(),
            evaluation.value().scalar()
        );
        assert_eq!(
            payload.bytes(),
            encode(evaluation.value(), evaluation.value_type(), 1024).unwrap()
        );
        assert_eq!(
            payload.fingerprint(),
            ConstMemoPayload::capture(&plan, &evaluation, 1024)
                .unwrap()
                .fingerprint()
        );
        assert!(payload.metadata_bytes().unwrap() >= payload.bytes().len());
    }
}
#[test]
fn memo_payload_empty_upstream_array_keeps_declared_i32_and_cold_invalid_initializer() {
    // This is a compile-fail upstream fixture. Selecting A's pure declaration is a
    // component witness only; it does not satisfy B's diagnostic or run main.
    let text = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/array_const_index-1.rs"
    );
    let source = ConstantSource::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
        Digest([180; 32]),
        CfgSet::guest_default(),
    )
    .unwrap();
    let plan = source
        .constant(
            source
                .declarations()
                .iter()
                .find(|decl| decl.name() == "A")
                .unwrap()
                .range(),
        )
        .unwrap();
    let evaluation = evaluate(&plan, ConstLimits::default()).unwrap();
    let ConstantWireType::Array { element, length } = evaluation.value_type() else {
        panic!("declared array type absent")
    };
    assert_eq!(*length, 0);
    assert!(matches!(
        element.as_ref(),
        ConstantWireType::Scalar(PrimitiveType::I32)
    ));
    assert!(evaluation.value().elements().unwrap().is_empty());
    assert_eq!(evaluation.dependencies().len(), 1);
    let payload = ConstMemoPayload::capture(&plan, &evaluation, 1024).unwrap();
    assert!(
        payload
            .decode_value()
            .unwrap()
            .elements()
            .unwrap()
            .is_empty()
    );
    let wrong = ConstantWireType::Array {
        element: Box::new(ConstantWireType::Scalar(PrimitiveType::U32)),
        length: 0,
    };
    assert!(lrr::frontend::constants::wire::decode(payload.bytes(), &wrong, 1024).is_err());
}
#[test]
fn memo_payload_rejects_changed_source_owner_and_edition_selection() {
    let plan = length_plan(2024, Digest([179; 32]));
    let evaluation = evaluate(&plan, ConstLimits::default()).unwrap();
    for other in [
        length_plan(2024, Digest([181; 32])),
        length_plan(2015, Digest([179; 32])),
    ] {
        assert!(ConstMemoPayload::capture(&other, &evaluation, 1024).is_err());
    }
}
#[test]
fn memo_payload_rejects_insufficient_transport_budget_before_publication() {
    let plan = length_plan(2024, Digest([179; 32]));
    let evaluation = evaluate(&plan, ConstLimits::default()).unwrap();
    let payload = ConstMemoPayload::capture(&plan, &evaluation, 1024).unwrap();
    assert!(ConstMemoPayload::capture(&plan, &evaluation, payload.bytes().len() - 1).is_err());
    assert!(ConstMemoPayload::capture(&plan, &evaluation, 0).is_err());
}
