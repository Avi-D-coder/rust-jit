//! Reached-preparation controls; compiler-negative originals are not runtime passes.
use super::*;
use crate::{
    db::{
        key::QueryKey,
        provider::{Provided, Provider},
        value::QueryValue,
    },
    frontend::constants::{
        ConstLimits,
        query::{fetch_constant_preparing_macros, fetch_current_constant_data},
        query_macro_demand::ConstantMacroDemand,
        wire::ConstantWireType,
    },
    ir::op::PrimitiveType,
};
const PANIC: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-eval/const_panic.rs"
);
const NESTED: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-nested.rs");
fn fixture(name: &str, edition: u16) -> (Database, PreparedSourceView, ConstantSourceAdmission) {
    let (db, view, definition) = world(PANIC, name, edition, true);
    let admission = admit(&db, &view, definition).unwrap();
    (db, view, admission)
}
fn unit() -> ConstantWireType {
    ConstantWireType::Scalar(PrimitiveType::Unit)
}
fn pending(
    db: &mut Database,
    view: &PreparedSourceView,
    admission: &ConstantSourceAdmission,
) -> crate::db::memo::Outcome {
    fetch_current_constant_data(
        db,
        view,
        admission,
        &unit(),
        ConstLimits::default(),
        1 << 20,
    )
    .into_ready()
    .unwrap()
    .unwrap()
}
#[test]
fn original_panic_query_returns_uncached_actual_reached_site_without_panic() {
    for edition in [2015, 2018, 2021] {
        for name in ["Z", "Z2", "Z_CORE", "Z2_CORE"] {
            let (mut db, view, admission) = fixture(name, edition);
            let outcome = pending(&mut db, &view, &admission);
            let QueryValue::ConstantMacroDemand(demand) = outcome.value.as_ref() else {
                panic!("no actual site")
            };
            assert!(!outcome.cacheable);
            assert_eq!(outcome.fingerprint, outcome.value.fingerprint());
            assert_eq!(demand.body(), &admission.plan().owner_body().unwrap().key());
            assert!(
                view.entered_macro_facts(demand.body())
                    .unwrap()
                    .invocations
                    .iter()
                    .any(|i| i.range == demand.range())
            );
            assert!(demand.evaluation_steps() > 0);
            assert!(db.memo(&QueryKey::Constant(demand.key().clone())).is_none());
            demand.validate_current(&db, &view, &admission).unwrap();
            let failure =
                crate::frontend::constants::evaluate(admission.plan(), ConstLimits::default())
                    .unwrap_err();
            assert_eq!(failure.diagnostic().code, "LRR-E-CONST-MACRO-SOURCE");
            assert!(failure.panic().is_none());
        }
    }
}
#[test]
fn coordinator_retries_original_qualified_panic_with_actual_message_and_origin() {
    for edition in [2015, 2018, 2021] {
        for (name, message) in [
            ("Z", "cheese"),
            ("Z2", "explicit panic"),
            ("Z_CORE", "cheese"),
            ("Z2_CORE", "explicit panic"),
        ] {
            let (mut db, view, admission) = fixture(name, edition);
            let old = admission.identity();
            let first = pending(&mut db, &view, &admission);
            let QueryValue::ConstantMacroDemand(demand) = first.value.as_ref() else {
                panic!("no actual site")
            };
            let (current, outcome) = fetch_constant_preparing_macros(
                &mut db,
                &view,
                admission,
                &unit(),
                ConstLimits::default(),
                1 << 20,
            )
            .into_ready()
            .unwrap()
            .unwrap();
            let QueryValue::ConstantFailure(failure) = outcome.value.as_ref() else {
                panic!("no source panic")
            };
            assert_ne!(old, current.identity());
            assert_eq!(failure.diagnostic().code, "LRR-E-CONST-PANIC");
            assert_eq!(failure.diagnostic().message, message);
            assert_eq!(failure.range(), demand.range());
            assert_eq!(failure.source(), Digest::of_bytes(PANIC.as_bytes()));
            assert_eq!(failure.panic().unwrap().message(), message);
            assert!(db.memo(&QueryKey::Constant(demand.key().clone())).is_none());
            assert!(
                db.memo(&QueryKey::Constant(failure.key().clone()))
                    .is_some()
            );
            current.revalidate(&db, &view).unwrap();
        }
    }
}
#[test]
fn query_engine_refuses_to_memoize_macro_demand_from_a_caching_provider() {
    struct WrongCache(ConstantMacroDemand);
    impl Provider for WrongCache {
        fn execute(&self, _: &mut Database, _: &QueryKey) -> crate::db::poll::QueryPoll<Provided> {
            Provided::pure(QueryValue::ConstantMacroDemand(Box::new(self.0.clone())))
        }
    }
    let (mut db, view, admission) = fixture("Z", 2015);
    let first = pending(&mut db, &view, &admission);
    let QueryValue::ConstantMacroDemand(demand) = first.value.as_ref() else {
        panic!("no site")
    };
    let key = QueryKey::Constant(demand.key().clone());
    let result = db
        .fetch(&WrongCache((**demand).clone()), key.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    assert!(!result.cacheable);
    assert!(db.memo(&key).is_none());
}
#[test]
fn macro_demand_rejects_changed_epoch_before_proof_selection() {
    let (mut db, view, admission) = fixture("Z", 2015);
    let first = pending(&mut db, &view, &admission);
    let QueryValue::ConstantMacroDemand(demand) = first.value.as_ref() else {
        panic!("no site")
    };
    db.begin_expansion_epoch(2).unwrap();
    assert!(admission.prepare_reached_macro(&db, &view, demand).is_err());
    assert!(db.memo(&QueryKey::Constant(demand.key().clone())).is_none());
}
#[test]
fn macro_retry_uses_cumulative_steps_without_replenishing_zero_budget() {
    let (mut db, view, admission) = fixture("Z", 2015);
    let error = fetch_constant_preparing_macros(
        &mut db,
        &view,
        admission,
        &unit(),
        ConstLimits::new(1, 128, 1 << 20).unwrap(),
        1 << 20,
    )
    .into_ready()
    .unwrap()
    .unwrap_err();
    assert_eq!(error.code, "LRR-E-CONST-LIMIT");
    assert_eq!(error.message, "constant retry step budget exhausted");
}
#[test]
fn original_nonliteral_panic_operands_remain_an_exact_frontier() {
    for edition in [2015, 2018, 2021] {
        for name in ["W", "W2", "W_CORE", "W2_CORE"] {
            let (mut db, view, admission) = fixture(name, edition);
            let error = fetch_constant_preparing_macros(
                &mut db,
                &view,
                admission,
                &unit(),
                ConstLimits::default(),
                1 << 20,
            )
            .into_ready()
            .unwrap()
            .unwrap_err();
            assert_eq!(error.code, "LRR-E-CONST-MACRO-OPERANDS");
        }
    }
}
#[test]
fn cold_macro_branch_with_upstream_declarations_retains_original_admission() {
    // Disclosed LRR laziness supplement around unchanged upstream source.
    let text = format!("{NESTED}\nconst C:u32=if false{{panic!(\"cold\")}}else{{7}};\n");
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, definition) = world(&text, "C", edition, true);
        let admission = admit(&db, &view, definition).unwrap();
        let old = admission.identity();
        let (current, outcome) = fetch_constant_preparing_macros(
            &mut db,
            &view,
            admission,
            &ConstantWireType::Scalar(PrimitiveType::U32),
            ConstLimits::default(),
            1 << 20,
        )
        .into_ready()
        .unwrap()
        .unwrap();
        assert_eq!(old, current.identity());
        let QueryValue::Constant(value) = outcome.value.as_ref() else {
            panic!("cold macro demanded")
        };
        assert_eq!(
            value
                .payload()
                .decode_value()
                .unwrap()
                .scalar()
                .unwrap()
                .payload,
            7
        );
    }
}

#[test]
fn parent_array_query_prepares_real_helper_macro_in_its_type_length_child() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-const-eval.rs"
    );
    let text = format!(
        "{original}\nconst fn failed_length()->usize{{panic!(\"length failure\")}}const BAD:[u8;failed_length()]=[];\n"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, definition) = world(&text, "BAD", edition, true);
        let error = crate::frontend::constants::query::constant(
            &mut db,
            &view,
            definition,
            &ConstantWireType::Array {
                element: Box::new(ConstantWireType::Scalar(PrimitiveType::U8)),
                length: 0,
            },
            ConstLimits::default(),
            1 << 20,
        )
        .into_ready()
        .unwrap()
        .unwrap_err();
        assert_eq!(error.code, "LRR-E-CONST-PANIC");
        assert_eq!(error.message, "length failure");
    }
}

#[test]
fn forwarded_length_macro_demand_retains_exact_parent_and_child_without_cache() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-const-eval.rs"
    );
    let text = format!(
        "{original}\nconst fn failed_length()->usize{{panic!(\"length failure\")}}const BAD:[u8;failed_length()]=[];\n"
    );
    let (mut db, view, definition) = world(&text, "BAD", 2024, true);
    let admission = admit(&db, &view, definition).unwrap();
    let wire = ConstantWireType::Array {
        element: Box::new(ConstantWireType::Scalar(PrimitiveType::U8)),
        length: 0,
    };
    let outcome = fetch_current_constant_data(
        &mut db,
        &view,
        &admission,
        &wire,
        ConstLimits::default(),
        1 << 20,
    )
    .into_ready()
    .unwrap()
    .unwrap();
    let QueryValue::ConstantMacroDemand(demand) = outcome.value.as_ref() else {
        panic!("length site lost")
    };
    let parent =
        crate::db::key::ConstantKey::capture(admission.plan(), &wire, admission.identity())
            .unwrap()
            .bind_universe(db.constant_input_universe().unwrap());
    let ready = crate::frontend::constant_result_type::CurrentConstantResultType::capture_current(
        &db,
        &view,
        admission.clone(),
        MAX_BYTES,
    )
    .unwrap();
    let (_, length) = ready.array().unwrap();
    let child = admission
        .reselect_array_length_current(&db, &view, length, MAX_BYTES)
        .unwrap();
    let child_key = crate::db::key::ConstantKey::capture(
        child.plan(),
        &length.expected_wire(),
        child.identity(),
    )
    .unwrap()
    .bind_universe(db.constant_input_universe().unwrap());
    assert_eq!(demand.key(), &parent);
    assert_eq!(demand.evaluated_key(), &child_key);
    assert_ne!(parent, child_key);
    assert!(!outcome.cacheable);
    assert!(db.memo(&QueryKey::Constant(parent)).is_none());
    assert!(db.memo(&QueryKey::Constant(child_key.clone())).is_none());
    demand.validate_current(&db, &view, &admission).unwrap();
    struct WrongParent(ConstantMacroDemand);
    impl Provider for WrongParent {
        fn execute(&self, _: &mut Database, _: &QueryKey) -> crate::db::poll::QueryPoll<Provided> {
            Provided::pure(QueryValue::ConstantMacroDemand(Box::new(self.0.clone())))
        }
    }
    let error = db
        .fetch(
            &WrongParent((**demand).clone()),
            QueryKey::Constant(child_key.clone()),
        )
        .into_ready()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
    assert_eq!(error.message, "query result variant mismatch");
    assert!(db.memo(&QueryKey::Constant(child_key)).is_none());
}
