//! Actual cells/tickets/leases; no positive target receipt is fabricated.
use super::*;

fn scalar(store: &mut GuestStore, frame: FrameId, slot: u32) -> CellId {
    let cell = store
        .new_cell(
            StorageContract::Exact(TypeId(8)),
            true,
            CellOwner::FrameSlot { frame, slot },
        )
        .unwrap();
    let ticket = store
        .ticket(ValueRepr::signed(TypeId(8), 3).unwrap())
        .unwrap();
    store.initialize(cell, ticket).unwrap();
    cell
}
fn reference_cell(
    store: &mut GuestStore,
    frame: FrameId,
    slot: u32,
    target: CellId,
    access: Access,
) -> CellId {
    let lease = store.borrow(&Place::cell(target), access).unwrap();
    let ticket = store.reference_ticket(lease).unwrap();
    let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
    let cell = store
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot { frame, slot },
        )
        .unwrap();
    store.initialize(cell, ticket).unwrap();
    cell
}
#[test]
fn checked_reference_result_accepts_new_reborrow_lease_and_preserves_actual_target() {
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let frame = store.new_frame_anchor().unwrap();
    let target = scalar(&mut store, frame, 0);
    let argument = reference_cell(&mut store, frame, 1, target, Access::Shared);
    let receipt = store.reference_target(argument).unwrap();
    let original = store.reference_from_cell(argument).unwrap();
    let result = store
        .borrow_ticket(&Place::deref(original), Access::Shared)
        .unwrap();
    assert_ne!(store.peek_ticket(result).unwrap().payload, original.bits());
    let returned_type = store
        .require_reference_return(result, &receipt, false, TypeId(8))
        .unwrap();
    assert_eq!(
        store.type_descriptor(returned_type).unwrap().key.as_ref(),
        &TypeKey::Ref {
            mutable: false,
            target: TypeId(8)
        }
    );
    let argument_cleanup = store.begin_drop(argument).unwrap();
    store.discard(argument_cleanup).unwrap();
    store.retire_cell(argument).unwrap();
    store
        .require_reference_return(result, &receipt, false, TypeId(8))
        .unwrap();
    assert_eq!(store.read_scalar(target).unwrap().payload, 3);
    store.discard(result).unwrap();
}
#[test]
fn equal_layout_and_value_do_not_authorize_a_different_return_target() {
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let frame = store.new_frame_anchor().unwrap();
    let target = scalar(&mut store, frame, 0);
    let other = scalar(&mut store, frame, 1);
    let argument = reference_cell(&mut store, frame, 2, target, Access::Shared);
    let receipt = store.reference_target(argument).unwrap();
    let result = store
        .borrow_ticket(&Place::cell(other), Access::Shared)
        .unwrap();
    let before = store.peek_ticket(result).unwrap();
    let handles = store.budget().handles();
    let bytes = store.budget().bytes();
    assert!(
        store
            .require_reference_return(result, &receipt, false, TypeId(8))
            .is_err()
    );
    assert_eq!(store.peek_ticket(result).unwrap(), before);
    assert_eq!(store.budget().handles(), handles);
    assert_eq!(store.budget().bytes(), bytes);
    assert_eq!(
        store.read_scalar(target).unwrap(),
        store.read_scalar(other).unwrap()
    );
}
#[test]
fn reference_result_receipt_refuses_cross_store_even_with_matching_handle_bits() {
    let mut a = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let af = a.new_frame_anchor().unwrap();
    let av = scalar(&mut a, af, 0);
    let ar = reference_cell(&mut a, af, 1, av, Access::Shared);
    let receipt = a.reference_target(ar).unwrap();
    let mut b = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let bf = b.new_frame_anchor().unwrap();
    let bv = scalar(&mut b, bf, 0);
    let br = reference_cell(&mut b, bf, 1, bv, Access::Shared);
    assert_eq!(
        a.admit_move(ar).unwrap().payload,
        b.admit_move(br).unwrap().payload
    );
    let ticket = b.copy_from(br).unwrap();
    assert!(
        b.require_reference_return(ticket, &receipt, false, TypeId(8))
            .is_err()
    );
    assert!(b.peek_ticket(ticket).is_ok());
}
#[test]
fn reference_result_checks_live_target_access_and_real_mutable_ticket_owner() {
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let frame = store.new_frame_anchor().unwrap();
    let target = scalar(&mut store, frame, 0);
    let argument = reference_cell(&mut store, frame, 1, target, Access::Mutable);
    let receipt = store.reference_target(argument).unwrap();
    let ticket = store.move_from(argument).unwrap();
    store
        .require_reference_return(ticket, &receipt, true, TypeId(8))
        .unwrap();
    assert!(
        store
            .require_reference_return(ticket, &receipt, false, TypeId(8))
            .is_err()
    );
    let value = store.peek_ticket(ticket).unwrap();
    assert!(store.ticket(value).is_err());
    let moved = store.move_from(target).unwrap();
    assert!(
        store
            .require_reference_return(ticket, &receipt, true, TypeId(8))
            .is_err()
    );
    assert!(store.peek_ticket(ticket).is_ok());
    store.discard(moved).unwrap();
}

#[test]
fn reference_result_replays_original_enclosing_chain_even_when_endpoints_match() {
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let frame = store.new_frame_anchor().unwrap();
    let tuple_type = store
        .intern_type(TypeKey::Tuple(Box::new([TypeId(8), TypeId(8)])))
        .unwrap();
    let guard = store.begin_construction(&[TypeId(8), TypeId(8)]).unwrap();
    for index in [0, 1] {
        let value = store
            .ticket(ValueRepr::signed(TypeId(8), 3).unwrap())
            .unwrap();
        store.construction_push(guard, index, value).unwrap();
    }
    let tuple = store.commit_construction(guard, tuple_type).unwrap();
    let root = store
        .new_cell(
            StorageContract::Exact(tuple_type),
            true,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    store.initialize(root, tuple).unwrap();
    let (object, storage) = store.place_object(root).unwrap();
    let leaf = storage.fields[0];
    let sibling = storage.fields[1];
    let argument = reference_cell(&mut store, frame, 1, leaf, Access::Shared);
    let receipt = store.reference_target(argument).unwrap();
    assert_eq!(receipt.enclosing.len(), 1);
    let endpoints = (
        store.place_stamp(root).unwrap(),
        store.place_stamp(leaf).unwrap(),
    );
    // Adversarial ownership-state control, not a new production mutation API:
    // reparent fields without advancing endpoint stamps. A fresh actual lease
    // follows the new path, while the original input's field authority is stale.
    let ObjectStorage::Aggregate(storage) = &mut store.objects.get_mut(object).unwrap().storage
    else {
        panic!()
    };
    storage.fields.swap(0, 1);
    store.cells.get_mut(leaf).unwrap().owner = CellOwner::Field { object, index: 1 };
    store.cells.get_mut(sibling).unwrap().owner = CellOwner::Field { object, index: 0 };
    assert_eq!(
        endpoints,
        (
            store.place_stamp(root).unwrap(),
            store.place_stamp(leaf).unwrap()
        )
    );
    assert!(store.reference_from_cell(argument).is_err());
    let result = store
        .borrow_ticket(&Place::cell(leaf), Access::Shared)
        .unwrap();
    let actual = RefId::from_bits(store.peek_ticket(result).unwrap().payload);
    let current = store.references.get(actual).unwrap();
    store.validate_reference_record(current).unwrap();
    assert_eq!(current.root, receipt.root);
    assert_eq!(current.final_cell, receipt.final_cell);
    assert_ne!(current.enclosing, receipt.enclosing);
    assert_eq!(store.read_reference(actual).unwrap().payload, 3);
    assert!(
        store
            .require_reference_return(result, &receipt, false, TypeId(8))
            .is_err()
    );
    assert!(store.peek_ticket(result).is_ok());
}
