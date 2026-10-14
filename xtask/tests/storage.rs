use lrr::runtime::{
    cell::{CellOwner, CellState, GuestStore, StorageContract},
    shape::TypeId,
    value::ValueRepr,
};
fn store() -> GuestStore {
    GuestStore::new(1, 1000, 1024 * 1024).unwrap()
}
fn cell(store: &mut GuestStore, ty: StorageContract, writable: bool) -> lrr::runtime::cell::CellId {
    let frame = store.new_frame_anchor().unwrap();
    store
        .new_cell(ty, writable, CellOwner::FrameSlot { frame, slot: 0 })
        .unwrap()
}
#[test]
fn primitive_values_validate_widths_unicode_float_bits_and_reserved_ids() {
    for (id, width) in [(4, 8), (5, 16), (6, 32), (7, 64), (8, 64)] {
        let min = if width == 64 {
            i64::MIN
        } else {
            -(1i64 << (width - 1))
        };
        assert_eq!(
            ValueRepr::signed(TypeId(id), min).unwrap().payload,
            min as u64
        );
    }
    assert!(ValueRepr::signed(TypeId(4), 128).is_err());
    assert!(ValueRepr::unsigned(TypeId(9), 256).is_err());
    assert!(
        ValueRepr {
            kind: 3,
            type_id: 3,
            payload: 0xd800
        }
        .validate_scalar()
        .is_err()
    );
    assert!(
        ValueRepr {
            kind: 2,
            type_id: 2,
            payload: 2
        }
        .validate_scalar()
        .is_err()
    );
    assert_eq!(
        ValueRepr::f32(f32::from_bits(0x7fc01234)).payload,
        0x7fc01234
    );
    assert_eq!(
        ValueRepr::f64(f64::from_bits(0x7ff8000000001234)).payload,
        0x7ff8000000001234
    );
    assert_eq!(std::mem::size_of::<ValueRepr>(), 16);
    assert_eq!(std::mem::align_of::<ValueRepr>(), 8);
}
#[test]
fn cells_move_copy_replace_and_preserve_location_epoch_for_scalar_reassignment() {
    let mut s = store();
    let id = cell(&mut s, StorageContract::Exact(TypeId(7)), true);
    let ticket = s.ticket(ValueRepr::signed(TypeId(7), 3).unwrap()).unwrap();
    s.initialize(id, ticket).unwrap();
    let epoch = s.location_epoch(id).unwrap();
    let copy = s.copy_from(id).unwrap();
    assert_eq!(s.peek_ticket(copy).unwrap().payload, 3);
    s.discard(copy).unwrap();
    let incoming = s.ticket(ValueRepr::signed(TypeId(7), 5).unwrap()).unwrap();
    let old = s.replace(id, incoming).unwrap();
    assert_eq!(s.peek_ticket(old).unwrap().payload, 3);
    assert_eq!(s.location_epoch(id).unwrap(), epoch);
    s.discard(old).unwrap();
    let moved = s.move_from(id).unwrap();
    assert_eq!(s.state(id).unwrap(), CellState::Moved);
    assert!(s.read_scalar(id).is_err());
    assert_eq!(s.location_epoch(id).unwrap(), epoch + 1);
    s.initialize(id, moved).unwrap();
    assert_eq!(s.read_scalar(id).unwrap().payload, 5);
}
#[test]
fn owning_value_bit_copies_and_consumed_tickets_cannot_create_second_owners() {
    let mut s = store();
    let original = s.string("owned").unwrap();
    let raw = s.peek_ticket(original).unwrap();
    assert!(s.ticket(raw).is_err());
    let id = cell(&mut s, StorageContract::Exact(TypeId(raw.type_id)), true);
    s.initialize(id, original).unwrap();
    assert!(s.peek_ticket(original).is_err());
    assert!(s.copy_from(id).is_err());
    assert!(s.ticket(raw).is_err());
    let moved = s.move_from(id).unwrap();
    assert_eq!(s.string_contents(moved).unwrap(), "owned");
    s.discard(moved).unwrap();
    assert!(s.ticket(raw).is_err());
}
#[test]
fn failed_contract_and_ticket_reservation_preserve_existing_ownership() {
    let mut s = store();
    let id = cell(&mut s, StorageContract::Exact(TypeId(7)), true);
    let t = s.ticket(ValueRepr::signed(TypeId(7), 7).unwrap()).unwrap();
    s.initialize(id, t).unwrap();
    let wrong = s.ticket(ValueRepr::signed(TypeId(6), 9).unwrap()).unwrap();
    assert!(s.replace(id, wrong).is_err());
    assert_eq!(s.read_scalar(id).unwrap().payload, 7);
    assert_eq!(s.peek_ticket(wrong).unwrap().payload, 9);
    let mut s = GuestStore::new(1, 3, 1024 * 1024).unwrap();
    let id = cell(&mut s, StorageContract::Unbound, true);
    let t = s.ticket(ValueRepr::boolean(true)).unwrap();
    s.initialize(id, t).unwrap();
    let epoch = s.location_epoch(id).unwrap();
    assert_eq!(s.move_from(id).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(s.read_scalar(id).unwrap(), ValueRepr::boolean(true));
    assert_eq!(s.location_epoch(id).unwrap(), epoch);
}
#[test]
fn unresolved_numeric_aliases_share_one_binding_and_never_default_width() {
    let mut s = store();
    let literal = s.integer_literal(false, 3, "3".into()).unwrap();
    let id = cell(&mut s, StorageContract::Unbound, true);
    s.initialize(id, literal).unwrap();
    assert_eq!(s.read_scalar(id).unwrap_err().code, "LRR-E-TYPE-CONTEXT");
    let alias = s.copy_from(id).unwrap();
    s.materialize(alias, TypeId(7)).unwrap();
    assert_eq!(
        s.read_scalar(id).unwrap(),
        ValueRepr::signed(TypeId(7), 3).unwrap()
    );
    assert!(s.materialize(alias, TypeId(6)).is_err());
    let min = s
        .integer_literal(true, 1u128 << 63, "-9223372036854775808".into())
        .unwrap();
    assert_eq!(
        s.materialize(min, TypeId(7)).unwrap().payload,
        i64::MIN as u64
    );
    let too_large = s.integer_literal(false, 256, "256".into()).unwrap();
    assert!(s.materialize(too_large, TypeId(9)).is_err());
    assert!(s.materialize(too_large, TypeId(10)).is_ok());
}
#[test]
fn run_reset_invalidates_cells_objects_tickets_literals_and_frame_owners() {
    let mut s = store();
    let id = cell(&mut s, StorageContract::Unbound, true);
    let frame = s.new_frame_anchor().unwrap();
    let owning = s.string("before").unwrap();
    let raw = s.peek_ticket(owning).unwrap();
    let literal = s.integer_literal(false, 3, "3".into()).unwrap();
    s.reset(2).unwrap();
    assert_eq!(s.state(id).unwrap_err().code, "LRR-E-STALE-HANDLE");
    assert!(s.peek_ticket(owning).is_err());
    assert!(s.materialize(literal, TypeId(7)).is_err());
    assert!(
        s.new_cell(
            StorageContract::Unbound,
            true,
            CellOwner::FrameSlot { frame, slot: 0 }
        )
        .is_err()
    );
    assert!(s.ticket(raw).is_err());
    assert_eq!(s.budget().handles(), 0);
    assert!(s.reset(2).is_err());
}

#[test]
fn materialized_numeric_local_reassignment_preserves_its_fixed_type_contract() {
    let mut s = store();
    let literal = s.integer_literal(false, 3, "3".into()).unwrap();
    let id = cell(&mut s, StorageContract::Unbound, true);
    s.initialize(id, literal).unwrap();
    let alias = s.copy_from(id).unwrap();
    s.materialize(alias, TypeId(7)).unwrap();
    let incoming = s.ticket(ValueRepr::signed(TypeId(7), 5).unwrap()).unwrap();
    let old = s.replace(id, incoming).unwrap();
    assert_eq!(s.declared(id).unwrap(), StorageContract::Exact(TypeId(7)));
    assert_eq!(s.read_scalar(id).unwrap().payload, 5);
    s.discard(old).unwrap();
    let wrong = s.ticket(ValueRepr::signed(TypeId(6), 7).unwrap()).unwrap();
    assert!(s.replace(id, wrong).is_err());
    assert_eq!(s.peek_ticket(wrong).unwrap().payload, 7);
}

#[test]
fn unresolved_numeric_contract_rejects_non_numeric_reassignment_atomically() {
    let mut s = store();
    let id = cell(&mut s, StorageContract::NumericLiteral, true);
    let original = s.integer_literal(false, 3, "3".into()).unwrap();
    s.initialize(id, original).unwrap();
    let incoming = s.ticket(ValueRepr::boolean(true)).unwrap();
    let before = s.state(id).unwrap();
    assert!(s.replace(id, incoming).is_err());
    assert_eq!(s.state(id).unwrap(), before);
    assert_eq!(s.peek_ticket(incoming).unwrap(), ValueRepr::boolean(true));
}
