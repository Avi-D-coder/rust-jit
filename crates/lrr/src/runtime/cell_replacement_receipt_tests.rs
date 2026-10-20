//! LRR ownership controls, outside the unchanged upstream Drop witness.
use super::*;

fn tuple(store: &mut GuestStore, n: i64) -> TicketId {
    let field = TypeId(6);
    let ty = store.types.intern(TypeKey::Tuple(vec![field].into())).unwrap();
    let guard = store.begin_construction(&[field]).unwrap();
    let value = store.ticket(ValueRepr::signed(field, n).unwrap()).unwrap();
    store.construction_push(guard, 0, value).unwrap();
    store.commit_construction(guard, ty).unwrap()
}

fn replaced() -> (
    GuestStore,
    CellId,
    CellId,
    u32,
    ValueRepr,
    ReplacementCleanupRoot,
) {
    let mut store = GuestStore::for_epoch(1).unwrap();
    let frame = store.new_frame_anchor().unwrap();
    let initial = tuple(&mut store, 42);
    let ty = TypeId(store.peek_ticket(initial).unwrap().type_id);
    let source = store
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    store.initialize(source, initial).unwrap();
    let root = store
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot { frame, slot: 1 },
        )
        .unwrap();
    let previous = store.admit_cleanup_root(source).unwrap();
    let epoch = store.location_epoch(source).unwrap();
    let incoming = tuple(&mut store, 7);
    let receipt = store.replace_for_cleanup(source, incoming, root).unwrap();
    (store, source, root, epoch, previous, receipt)
}

#[test]
fn replacement_receipt_accepts_actual_ordinary_detached_owner() {
    let (store, source, root, epoch, previous, receipt) = replaced();
    assert_ne!(store.admit_cleanup_root(source).unwrap(), previous);
    assert_eq!(
        receipt.admit(source, epoch, previous, &store).unwrap(),
        (root, store.location_epoch(root).unwrap(), previous)
    );
}

#[test]
fn replacement_receipt_rejects_other_sources_epochs_values_and_store_sessions() {
    for case in 0..5 {
        let (mut store, source, root, epoch, previous, receipt) = replaced();
        let (source, epoch, previous) = match case {
            0 => (root, epoch, previous),
            1 => (source, epoch + 1, previous),
            2 => (source, epoch, store.admit_cleanup_root(source).unwrap()),
            3 => {
                // Same ObjectId in the same cell, but a new location lifetime.
                let old = store.move_from(root).unwrap();
                store.initialize(root, old).unwrap();
                (source, epoch, previous)
            }
            _ => {
                // Same Type and stamp, but a different legitimate owning value.
                let different = tuple(&mut store, 99);
                let _old_owner = store.replace(root, different).unwrap();
                (source, epoch, previous)
            }
        };
        let before = store.owner_inventory_for_test();
        assert!(
            receipt.admit(source, epoch, previous, &store).is_err(),
            "case {case}"
        );
        assert_eq!(store.owner_inventory_for_test(), before);
    }
    // Equal arena handles and values in another store are still foreign.
    let (a, source, root, epoch, previous, receipt) = replaced();
    let (b, source_b, root_b, epoch_b, previous_b, _receipt_b) = replaced();
    assert_eq!(
        (source, root, epoch, previous),
        (source_b, root_b, epoch_b, previous_b)
    );
    assert_ne!(a.pattern_binding_session(), b.pattern_binding_session());
    let before = b.owner_inventory_for_test();
    assert!(receipt.admit(source, epoch, previous, &b).is_err());
    assert_eq!(b.owner_inventory_for_test(), before);
}
