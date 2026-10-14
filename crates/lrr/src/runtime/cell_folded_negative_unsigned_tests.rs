//! Source-derived signed minima and supplemental folded-zero storage controls.
//! These do not run rustc or claim a complete upstream fixture pass.
use super::*;
use crate::vm::numeric::{self, Unary};

const SUFFIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);

#[test]
fn folded_negative_zero_refuses_every_unsigned_target_before_reservation() {
    for ty in [TypeId(9), TypeId(10), TypeId(11), TypeId(12), TypeId(13)] {
        for magnitude in [0, 1] {
            let error = GuestStore::preview_integer(true, magnitude, ty).unwrap_err();
            assert_eq!(error.code, "LRR-E-OP-TYPE");
            assert!(numeric::unary_type(Unary::Neg, ty).is_err());
        }
        assert_eq!(
            GuestStore::preview_integer(false, 0, ty).unwrap(),
            ValueRepr::unsigned(ty, 0).unwrap()
        );
    }
}

#[test]
fn failed_folded_zero_binding_preserves_shared_record_owner_and_allocation_budget() {
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let frame = store.new_frame_anchor().unwrap();
    let cell = store
        .new_cell(
            StorageContract::NumericLiteral,
            false,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    let first = store.integer_literal(true, 0, "-0".into()).unwrap();
    store.initialize(cell, first).unwrap();
    let a = store.copy_from(cell).unwrap();
    let b = store.copy_from(cell).unwrap();
    let fresh = store.integer_literal(true, 0, "-0".into()).unwrap();
    let shared = store.peek_ticket(a).unwrap();
    assert_eq!(shared.payload, store.peek_ticket(b).unwrap().payload);
    assert_ne!(shared.payload, store.peek_ticket(fresh).unwrap().payload);
    assert_eq!(store.integer_parts(shared).unwrap(), Some((true, 0)));
    let before = (
        store.budget().handles(),
        store.budget().bytes(),
        store.state(cell).unwrap(),
        store.location_epoch(cell).unwrap(),
        store.peek_ticket(a).unwrap(),
        store.peek_ticket(b).unwrap(),
    );
    assert_eq!(
        store.materialize_ticket(a, TypeId(11)).unwrap_err().code,
        "LRR-E-OP-TYPE"
    );
    assert_eq!(store.effective_type(shared).unwrap(), None);
    assert_eq!(
        (
            store.budget().handles(),
            store.budget().bytes(),
            store.state(cell).unwrap(),
            store.location_epoch(cell).unwrap(),
            store.peek_ticket(a).unwrap(),
            store.peek_ticket(b).unwrap(),
        ),
        before
    );
    let bound = store.materialize_ticket(b, TypeId(6)).unwrap();
    assert_eq!(
        store.peek_ticket(bound).unwrap(),
        ValueRepr::signed(TypeId(6), 0).unwrap()
    );
    assert_eq!(store.effective_type(shared).unwrap(), Some(TypeId(6)));
    let fresh_bound = store.materialize_ticket(fresh, TypeId(7)).unwrap();
    assert_eq!(
        store.peek_ticket(fresh_bound).unwrap(),
        ValueRepr::signed(TypeId(7), 0).unwrap()
    );
}

#[test]
fn original_signed_minima_and_signed_zero_remain_admitted_before_explicit_cast() {
    for (statement, magnitude, ty, value) in [
        ("let _i: i8 = -128;", 128, TypeId(4), i64::from(i8::MIN)),
        (
            "let _i: i16 = -32_768;",
            32768,
            TypeId(5),
            i64::from(i16::MIN),
        ),
        (
            "let _i: i32 = -2_147_483_648;",
            2147483648,
            TypeId(6),
            i64::from(i32::MIN),
        ),
        (
            "let _i: i64 = -9_223_372_036_854_775_808;",
            9223372036854775808,
            TypeId(7),
            i64::MIN,
        ),
    ] {
        assert!(SUFFIX.contains(statement));
        assert_eq!(
            GuestStore::preview_integer(true, magnitude, ty).unwrap(),
            ValueRepr::signed(ty, value).unwrap()
        );
        assert_eq!(
            GuestStore::preview_integer(true, 0, ty).unwrap(),
            ValueRepr::signed(ty, 0).unwrap()
        );
    }
    let signed = GuestStore::preview_integer(true, 1, TypeId(6)).unwrap();
    let converted = numeric::cast(signed, TypeId(11)).unwrap();
    assert_eq!(
        converted,
        ValueRepr::unsigned(TypeId(11), u64::from(u32::MAX)).unwrap()
    );
}
