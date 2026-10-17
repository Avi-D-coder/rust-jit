//! Stock-bound tuple ownership and supplemental whole-object transfer invariants.
use super::*;

fn snapshot(
    store: &GuestStore,
    root: CellId,
    field: CellId,
    sibling: CellId,
) -> (CellState, u32, CellState, u32, CellState, u32, usize) {
    (
        store.state(root).unwrap(),
        store.location_epoch(root).unwrap(),
        store.state(field).unwrap(),
        store.location_epoch(field).unwrap(),
        store.state(sibling).unwrap(),
        store.budget.handles(),
        store.budget.bytes(),
    )
}
#[test]
fn rustc_noncopy_tuple_whole_read_transfers_object_and_leaves_cold_drop_sibling_owned() {
    let (mut store, root, field, sibling, lease) = tests::stock_tuple_reference_owner();
    let value = store.admit_move(root).unwrap();
    assert_eq!(
        store.types.descriptor(TypeId(value.type_id)).unwrap().copy,
        super::super::shape::CopyStatus::NotCopy
    );
    let sibling_value = store.admit_move(sibling).unwrap();
    assert_eq!(
        store
            .types
            .descriptor(TypeId(sibling_value.type_id))
            .unwrap()
            .copy,
        super::super::shape::CopyStatus::Deferred
    );
    let prior = store.borrow(&Place::cell(root), Access::Shared).unwrap();
    let before = snapshot(&store, root, field, sibling);
    assert_eq!(store.admit_owning_read(root).unwrap(), IntrinsicRead::Move);
    let ticket = store.read_owning(root).unwrap();
    assert_eq!(store.peek_ticket(ticket).unwrap(), value);
    assert_eq!(store.state(root).unwrap(), CellState::Moved);
    assert_eq!(store.location_epoch(root).unwrap(), before.1 + 1);
    assert_eq!(store.state(field).unwrap(), before.2);
    assert_eq!(store.location_epoch(field).unwrap(), before.3);
    assert_eq!(store.state(sibling).unwrap(), before.4);
    assert_eq!(store.read_reference(lease).unwrap(), ValueRepr::unit());
    assert_eq!(
        store.read_reference(prior).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    assert_eq!(
        store.read_owning(root).unwrap_err().code,
        "LRR-E-USE-AFTER-MOVE"
    );
    // A forged duplicate payload cannot publish another ticket owner.
    assert_eq!(store.ticket(value).unwrap_err().code, "LRR-E-OWNERSHIP");
    let owner = store.new_frame_anchor().unwrap();
    let destination = store
        .new_cell(
            StorageContract::Exact(TypeId(value.type_id)),
            true,
            CellOwner::FrameSlot {
                frame: owner,
                slot: 0,
            },
        )
        .unwrap();
    store.initialize(destination, ticket).unwrap();
    assert_eq!(store.admit_move(destination).unwrap(), value);
    assert_eq!(store.state(sibling).unwrap(), before.4);
}
#[test]
fn rustc_noncopy_tuple_whole_read_through_shared_reference_refuses_without_transfer() {
    let (mut store, root, field, sibling, _) = tests::stock_tuple_reference_owner();
    let borrowed = store.borrow(&Place::cell(root), Access::Shared).unwrap();
    let before = snapshot(&store, root, field, sibling);
    let PlaceFailure::RuntimeFault(error) = store
        .read_owning_place(&Place::deref(borrowed))
        .unwrap_err()
    else {
        panic!("unexpected bounds panic")
    };
    assert_eq!(error.code, "LRR-E-REFERENCE-ACCESS");
    assert_eq!(snapshot(&store, root, field, sibling), before);
}
#[test]
fn rustc_noncopy_array_whole_transfer_keeps_element_move_restriction() {
    let (mut store, tuple, field, sibling, lease) = tests::stock_tuple_reference_owner();
    let tuple_ty = TypeId(store.admit_move(tuple).unwrap().type_id);
    let array_ty = store
        .intern_type(TypeKey::Array {
            element: tuple_ty,
            len: 1,
        })
        .unwrap();
    let ticket = store.move_from(tuple).unwrap();
    let guard = store.begin_construction(&[tuple_ty]).unwrap();
    store.construction_push(guard, 0, ticket).unwrap();
    let array_ticket = store.commit_construction(guard, array_ty).unwrap();
    let owner = store.new_frame_anchor().unwrap();
    let root = store
        .new_cell(
            StorageContract::Exact(array_ty),
            true,
            CellOwner::FrameSlot {
                frame: owner,
                slot: 0,
            },
        )
        .unwrap();
    store.initialize(root, array_ticket).unwrap();
    // Supplemental homogeneous array wrapper around the original tuple shape;
    // this does not execute a new original array program or its destructor.
    let place = store
        .project_index(
            &Place::cell(root),
            ValueRepr::unsigned(TypeId(13), 0).unwrap(),
        )
        .unwrap();
    let before = snapshot(&store, root, field, sibling);
    let PlaceFailure::RuntimeFault(error) = store.read_owning_place(&place).unwrap_err() else {
        panic!("unexpected bounds panic")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert_eq!(snapshot(&store, root, field, sibling), before);
    let whole = store.read_owning(root).unwrap();
    assert_eq!(store.peek_ticket(whole).unwrap().type_id, array_ty.0);
    assert_eq!(store.state(field).unwrap(), before.2);
    assert_eq!(store.state(sibling).unwrap(), before.4);
    assert_eq!(store.read_reference(lease).unwrap(), ValueRepr::unit());
}
#[test]
fn rustc_cold_nominal_tuple_policy_remains_deferred_without_intrinsic_noncopy_member() {
    let (mut store, _root, _field, sibling, _) = tests::stock_tuple_reference_owner();
    let ty = TypeId(store.admit_move(sibling).unwrap().type_id);
    let ticket = store.move_from(sibling).unwrap();
    let tuple_ty = store.intern_type(TypeKey::Tuple(Box::new([ty]))).unwrap();
    let guard = store.begin_construction(&[ty]).unwrap();
    store.construction_push(guard, 0, ticket).unwrap();
    let whole = store.commit_construction(guard, tuple_ty).unwrap();
    let owner = store.new_frame_anchor().unwrap();
    let root = store
        .new_cell(
            StorageContract::Exact(tuple_ty),
            true,
            CellOwner::FrameSlot {
                frame: owner,
                slot: 0,
            },
        )
        .unwrap();
    store.initialize(root, whole).unwrap();
    assert_eq!(
        store.types.descriptor(tuple_ty).unwrap().copy,
        super::super::shape::CopyStatus::Deferred
    );
    let before = (
        store.state(root).unwrap(),
        store.location_epoch(root).unwrap(),
        store.budget.handles(),
        store.budget.bytes(),
    );
    assert_eq!(
        store.read_owning(root).unwrap_err().code,
        "LRR-E-UNSUPPORTED"
    );
    assert_eq!(
        (
            store.state(root).unwrap(),
            store.location_epoch(root).unwrap(),
            store.budget.handles(),
            store.budget.bytes()
        ),
        before
    );
}
