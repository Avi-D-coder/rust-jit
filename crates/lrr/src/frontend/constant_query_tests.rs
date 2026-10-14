//! Current source/query controls using unchanged upstream components, not main passes.
use super::*;
use crate::{
    db::{dependency::DependencyStamp, key::QueryKey, provider::SyntaxProvider, value::QueryValue},
    frontend::constants::{query::fetch_current_constant, wire::ConstantWireType},
    ir::op::PrimitiveType,
};
fn fetch(
    db: &mut Database,
    view: &PreparedSourceView,
    proof: &ConstantSourceAdmission,
    ty: PrimitiveType,
) -> crate::db::poll::QueryPoll<crate::db::memo::Outcome> {
    fetch_current_constant(
        db,
        view,
        proof,
        &ConstantWireType::Scalar(ty),
        ConstLimits::default(),
        1024,
    )
}
#[test]
fn exact_nested_upstream_result_and_all_current_dependencies_are_cached_once() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, definition) = world(NESTED, "X", edition, true);
        let proof = admit(&db, &view, definition).unwrap();
        let first = fetch(&mut db, &view, &proof, PrimitiveType::U32)
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Constant(value) = first.value.as_ref() else {
            panic!("typed constant absent")
        };
        assert_eq!(
            value
                .payload()
                .decode_value()
                .unwrap()
                .scalar()
                .unwrap()
                .payload,
            22
        );
        let key = QueryKey::Constant(value.key().clone());
        let memo = db.memo(&key).unwrap();
        assert_eq!(memo.dependencies.len(), proof.dependencies().len());
        for (input, fingerprint) in proof.dependencies() {
            assert!(memo.dependencies.contains(&DependencyStamp::Input {
                key: input.clone(),
                fingerprint: *fingerprint
            }));
        }
        let second = fetch(&mut db, &view, &proof, PrimitiveType::U32)
            .into_ready()
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(&first.value, &second.value));
        assert_eq!(db.trace.counts["Constant:ProviderRun"], 1);
        assert_eq!(db.trace.counts["Constant:MemoHit"], 1);
        assert!(
            !db.trace
                .counts
                .keys()
                .any(|name| name.starts_with("Thunk:") || name.starts_with("FunctionSignature:"))
        );
    }
}
#[test]
fn exact_module_constant_uses_its_owning_source_scope_in_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, definition) = world(MODULE, "BITS", edition, true);
        let proof = admit(&db, &view, definition).unwrap();
        let result = fetch(&mut db, &view, &proof, PrimitiveType::Usize)
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Constant(value) = result.value.as_ref() else {
            panic!("typed constant absent")
        };
        assert_eq!(
            value
                .payload()
                .decode_value()
                .unwrap()
                .scalar()
                .unwrap()
                .payload,
            8
        );
        assert_eq!(proof.plan().owner_context().scope.module_path, ["u8"]);
    }
}
#[test]
fn wrong_expected_type_or_transport_budget_cannot_publish_constant_memo() {
    let (mut db, view, definition) = world(NESTED, "X", 2024, true);
    let proof = admit(&db, &view, definition).unwrap();
    let failure = fetch(&mut db, &view, &proof, PrimitiveType::U64)
        .into_ready()
        .unwrap()
        .unwrap_err();
    assert_eq!(failure.code, "LRR-E-CONST-TYPE");
    let failure = fetch_current_constant(
        &mut db,
        &view,
        &proof,
        &ConstantWireType::Scalar(PrimitiveType::U32),
        ConstLimits::default(),
        1,
    )
    .into_ready()
    .unwrap()
    .unwrap_err();
    assert_eq!(failure.code, "LRR-E-CONST-WIRE");
    let good = fetch(&mut db, &view, &proof, PrimitiveType::U32)
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::Constant(value) = good.value.as_ref() else {
        panic!("typed constant absent")
    };
    assert!(db.memo(&QueryKey::Constant(value.key().clone())).is_some());
}
#[test]
fn archived_real_constant_key_is_refused_after_actual_epoch_publication() {
    let (mut db, view, definition) = world(NESTED, "X", 2024, true);
    let proof = admit(&db, &view, definition).unwrap();
    let result = fetch(&mut db, &view, &proof, PrimitiveType::U32)
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::Constant(value) = result.value.as_ref() else {
        panic!("typed constant absent")
    };
    let key = QueryKey::Constant(value.key().clone());
    let runs = db.trace.counts["Constant:ProviderRun"];
    db.begin_expansion_epoch(2).unwrap();
    let error = db
        .fetch(&SyntaxProvider, key.clone())
        .into_ready()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-CONST-SOURCE");
    assert!(db.memo(&key).is_none());
    assert_eq!(db.trace.counts["Constant:ProviderRun"], runs);
    assert!(
        fetch(&mut db, &view, &proof, PrimitiveType::U32)
            .into_ready()
            .unwrap()
            .is_err()
    );
}
#[test]
fn stale_or_other_source_view_cannot_fetch_a_previously_cached_result() {
    let (mut db, view, definition) = world(NESTED, "X", 2024, true);
    let proof = admit(&db, &view, definition).unwrap();
    fetch(&mut db, &view, &proof, PrimitiveType::U32)
        .into_ready()
        .unwrap()
        .unwrap();
    let (_, other, _) = world("const X:u32=22;fn main(){}", "X", 2024, true);
    let runs = db.trace.counts["Constant:ProviderRun"];
    assert!(
        fetch(&mut db, &other, &proof, PrimitiveType::U32)
            .into_ready()
            .unwrap()
            .is_err()
    );
    assert_eq!(db.trace.counts["Constant:ProviderRun"], runs);
}

#[test]
fn cache_hits_must_still_obey_current_payload_and_value_byte_limits() {
    let (mut db, view, definition) = world(NESTED, "X", 2024, true);
    let proof = admit(&db, &view, definition).unwrap();
    fetch(&mut db, &view, &proof, PrimitiveType::U32)
        .into_ready()
        .unwrap()
        .unwrap();
    let runs = db.trace.counts["Constant:ProviderRun"];
    let ty = ConstantWireType::Scalar(PrimitiveType::U32);
    let error = fetch_current_constant(&mut db, &view, &proof, &ty, ConstLimits::default(), 1)
        .into_ready()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-CONST-WIRE");
    let tiny = ConstLimits::new(100_000, 128, 1).unwrap();
    let error = fetch_current_constant(&mut db, &view, &proof, &ty, tiny, 1024)
        .into_ready()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-CONST-LIMIT");
    assert_eq!(db.trace.counts["Constant:ProviderRun"], runs);
}

// The original fixture rejects reached B's out-of-bounds index. Selecting A is
// an initializer/query component observation, never a full original fixture pass.
const EMPTY_ARRAY: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/array_const_index-1.rs"
);
fn empty_type(length: u64) -> ConstantWireType {
    ConstantWireType::Array {
        element: Box::new(ConstantWireType::Scalar(PrimitiveType::I32)),
        length,
    }
}
#[test]
fn source_empty_array_length_is_a_real_child_query_before_initializer() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, definition) = world(EMPTY_ARRAY, "A", edition, true);
        let outcome = crate::frontend::constants::query::constant(
            &mut db,
            &view,
            definition,
            &empty_type(0),
            ConstLimits::default(),
            1024,
        )
        .into_ready()
        .unwrap()
        .unwrap();
        let QueryValue::Constant(value) = outcome.value.as_ref() else {
            panic!("typed empty array absent")
        };
        assert_eq!(
            value
                .payload()
                .decode_value()
                .unwrap()
                .elements()
                .unwrap()
                .len(),
            0
        );
        let key = QueryKey::Constant(value.key().clone());
        let memo = db.memo(&key).unwrap();
        let children = memo
            .dependencies
            .iter()
            .filter_map(|dependency| match dependency {
                DependencyStamp::Query {
                    key: QueryKey::Constant(child),
                    ..
                } => Some(child),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 1);
        assert_ne!(children[0], value.key());
        assert_eq!(
            children[0].result_type(),
            ConstantWireType::Scalar(PrimitiveType::Usize)
                .fingerprint_checked()
                .unwrap()
        );
        assert!(db.memo(&QueryKey::Constant(children[0].clone())).is_some());
        assert_eq!(db.trace.counts["Constant:ProviderRun"], 2);
        let again = crate::frontend::constants::query::constant(
            &mut db,
            &view,
            definition,
            &empty_type(0),
            ConstLimits::default(),
            1024,
        )
        .into_ready()
        .unwrap()
        .unwrap();
        assert!(Arc::ptr_eq(&outcome.value, &again.value));
        assert_eq!(db.trace.counts["Constant:ProviderRun"], 2);
        assert_eq!(db.trace.counts["Constant:MemoHit"], 1);
    }
}
#[test]
fn caller_array_count_cannot_replace_exact_upstream_source_length() {
    let (mut db, view, definition) = world(EMPTY_ARRAY, "A", 2024, true);
    let proof = admit(&db, &view, definition).unwrap();
    let wrong = empty_type(1);
    let error = crate::frontend::constants::query::constant(
        &mut db,
        &view,
        definition,
        &wrong,
        ConstLimits::default(),
        1024,
    )
    .into_ready()
    .unwrap()
    .unwrap_err();
    assert_eq!(error.code, "LRR-E-CONST-TYPE");
    assert_eq!(
        error.message,
        "caller array count differs from its exact source length"
    );
    let key = crate::db::key::ConstantKey::capture(proof.plan(), &wrong, proof.identity())
        .unwrap()
        .bind_universe(db.constant_input_universe().unwrap());
    assert!(db.memo(&QueryKey::Constant(key)).is_none());
    let good = crate::frontend::constants::query::constant(
        &mut db,
        &view,
        definition,
        &empty_type(0),
        ConstLimits::default(),
        1024,
    )
    .into_ready()
    .unwrap()
    .unwrap();
    let QueryValue::Constant(value) = good.value.as_ref() else {
        panic!("typed empty array absent")
    };
    assert_eq!(
        value
            .payload()
            .decode_value()
            .unwrap()
            .elements()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(db.trace.counts["Constant:ProviderRun"], 3);
    assert_eq!(db.trace.counts["Constant:MemoHit"], 1);
}

const ORIGINAL_DIV_ZERO: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-eval/const-eval-query-stack.rs"
);
#[test]
fn actual_upstream_numeric_failure_is_a_typed_cached_refusal() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, definition) = world(ORIGINAL_DIV_ZERO, "X", edition, true);
        let proof = admit(&db, &view, definition).unwrap();
        let first = fetch(&mut db, &view, &proof, PrimitiveType::I32)
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(first.code, "LRR-E-CONST-PANIC");
        assert_eq!(first.message, "division by zero");
        let key = QueryKey::Constant(
            crate::db::key::ConstantKey::capture(
                proof.plan(),
                &ConstantWireType::Scalar(PrimitiveType::I32),
                proof.identity(),
            )
            .unwrap()
            .bind_universe(db.constant_input_universe().unwrap()),
        );
        let memo = db
            .memo(&key)
            .expect("actual deterministic failure should be retained");
        let QueryValue::ConstantFailure(value) = memo.value.as_ref() else {
            panic!("untyped cached failure")
        };
        assert_eq!(value.key().selection(), proof.plan().identity());
        assert_eq!(
            value.numeric_panic(),
            Some(crate::vm::numeric::PanicKind::DivisionByZero)
        );
        assert!(value.panic().is_none());
        assert_eq!(
            value.source(),
            Digest::of_bytes(ORIGINAL_DIV_ZERO.as_bytes())
        );
        assert_eq!(
            &ORIGINAL_DIV_ZERO[value.range().start as usize..value.range().end as usize],
            "1 / 0"
        );
        assert_eq!(memo.dependencies.len(), proof.dependencies().len());
        let second = fetch(&mut db, &view, &proof, PrimitiveType::I32)
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(second.code, first.code);
        assert_eq!(second.message, first.message);
        assert_eq!(db.trace.counts["Constant:ProviderRun"], 1);
        assert_eq!(db.trace.counts["Constant:MemoHit"], 1);
        db.begin_expansion_epoch(2).unwrap();
        let error = db
            .fetch(&SyntaxProvider, key.clone())
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-CONST-SOURCE");
        assert!(db.memo(&key).is_none());
        assert_eq!(db.trace.counts["Constant:ProviderRun"], 1);
    }
}
#[test]
fn narrow_computation_budget_cannot_poison_a_later_real_constant_query() {
    let (mut db, view, definition) = world(NESTED, "X", 2024, true);
    let proof = admit(&db, &view, definition).unwrap();
    let ty = ConstantWireType::Scalar(PrimitiveType::U32);
    let error = fetch_current_constant(
        &mut db,
        &view,
        &proof,
        &ty,
        ConstLimits::new(1, 128, 1 << 20).unwrap(),
        1024,
    )
    .into_ready()
    .unwrap()
    .unwrap_err();
    assert_eq!(error.code, "LRR-E-CONST-LIMIT");
    let key = QueryKey::Constant(
        crate::db::key::ConstantKey::capture(proof.plan(), &ty, proof.identity())
            .unwrap()
            .bind_universe(db.constant_input_universe().unwrap()),
    );
    assert!(db.memo(&key).is_none());
    let good = fetch(&mut db, &view, &proof, PrimitiveType::U32)
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::Constant(value) = good.value.as_ref() else {
        panic!("real constant absent after quota recovery")
    };
    assert_eq!(
        value
            .payload()
            .decode_value()
            .unwrap()
            .scalar()
            .unwrap()
            .payload,
        22
    );
    assert_eq!(db.trace.counts["Constant:ProviderRun"], 2);
    assert!(db.memo(&key).is_some());
}
