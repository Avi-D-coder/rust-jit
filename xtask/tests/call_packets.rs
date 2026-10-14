use lrr::{
    runtime::{
        cell::{CellId, CellOwner, CellState, GuestStore, StorageContract, TicketId},
        shape::TypeId,
        value::ValueRepr,
    },
    vm::call::ArgumentPacket,
};
fn cell(store: &mut GuestStore, ty: Option<TypeId>) -> CellId {
    let frame = store.new_frame_anchor().unwrap();
    store
        .new_cell(
            ty.map_or(StorageContract::Unbound, StorageContract::Exact),
            false,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap()
}
fn scalar(store: &mut GuestStore, value: i64) -> CellId {
    let c = cell(store, Some(TypeId(7)));
    let ticket = store
        .ticket(ValueRepr::signed(TypeId(7), value).unwrap())
        .unwrap();
    store.initialize(c, ticket).unwrap();
    c
}
fn owned(store: &mut GuestStore, text: &str) -> (CellId, TypeId) {
    let ticket = store.string(text).unwrap();
    let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
    let c = cell(store, Some(ty));
    store.initialize(c, ticket).unwrap();
    (c, ty)
}
#[test]
fn argument_packet_descriptor_arity_and_alias_rejection_preserve_all_source_cells() {
    let mut store = GuestStore::new(1, 1000, 1024 * 1024).unwrap();
    let a = scalar(&mut store, 17);
    let b = scalar(&mut store, 29);
    let budget = (store.budget().handles(), store.budget().bytes());
    for (cells, types, code) in [
        (vec![a, b], vec![TypeId(7)], "LRR-E-CALL-ARITY"),
        (
            vec![a, b],
            vec![TypeId(7), TypeId(12)],
            "LRR-E-CALL-ARG-TYPE",
        ),
        (vec![a, a], vec![TypeId(7), TypeId(7)], "LRR-E-OWNERSHIP"),
    ] {
        assert_eq!(
            ArgumentPacket::seal(&mut store, &cells, &types)
                .err()
                .unwrap()
                .code,
            code
        );
        assert_eq!(
            store.read_scalar(a).unwrap(),
            ValueRepr::signed(TypeId(7), 17).unwrap()
        );
        assert_eq!(
            store.read_scalar(b).unwrap(),
            ValueRepr::signed(TypeId(7), 29).unwrap()
        );
        assert_eq!((store.budget().handles(), store.budget().bytes()), budget);
    }
}
#[test]
fn argument_packet_handle_quota_is_checked_before_any_source_move() {
    // Each source uses an anchor, cell and initial ticket: six handles already.
    let mut store = GuestStore::new(1, 8, 1024 * 1024).unwrap();
    let a = scalar(&mut store, 1);
    let b = scalar(&mut store, 2);
    let budget = (store.budget().handles(), store.budget().bytes());
    assert_eq!(
        ArgumentPacket::seal(&mut store, &[a, b], &[TypeId(7), TypeId(7)])
            .err()
            .unwrap()
            .code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert!(matches!(store.state(a).unwrap(), CellState::Live(_)));
    assert!(matches!(store.state(b).unwrap(), CellState::Live(_)));
    assert_eq!((store.budget().handles(), store.budget().bytes()), budget);
}
#[test]
fn argument_packet_moves_owning_values_and_delivers_each_ticket_once() {
    let mut store = GuestStore::new(1, 1000, 1024 * 1024).unwrap();
    let (a, ty) = owned(&mut store, "call-owned");
    let b = scalar(&mut store, 42);
    let left = cell(&mut store, Some(ty));
    let right = cell(&mut store, Some(TypeId(7)));
    let epoch = store.location_epoch(a).unwrap();
    let packet = ArgumentPacket::seal(&mut store, &[a, b], &[ty, TypeId(7)]).unwrap();
    assert_eq!(store.state(a).unwrap(), CellState::Moved);
    assert_eq!(store.state(b).unwrap(), CellState::Moved);
    assert_ne!(store.location_epoch(a).unwrap(), epoch);
    packet.deliver(&mut store, &[left, right]).unwrap();
    assert!(matches!(store.state(left).unwrap(),CellState::Live(value)if value.type_id==ty.0));
    assert_eq!(
        store.read_scalar(right).unwrap(),
        ValueRepr::signed(TypeId(7), 42).unwrap()
    );
    assert_eq!(
        packet.deliver(&mut store, &[left, right]).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    assert_eq!(
        packet.discard(&mut store).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    let transfer: TicketId = store.move_from(left).unwrap();
    store.discard(transfer).unwrap();
}
#[test]
fn rejected_argument_delivery_preserves_the_packet_and_all_callee_cells() {
    let mut store = GuestStore::new(1, 1000, 1024 * 1024).unwrap();
    let a = scalar(&mut store, 5);
    let b = scalar(&mut store, 6);
    let left = cell(&mut store, Some(TypeId(7)));
    let wrong = cell(&mut store, Some(TypeId(12)));
    let right = cell(&mut store, Some(TypeId(7)));
    let packet = ArgumentPacket::seal(&mut store, &[a, b], &[TypeId(7), TypeId(7)]).unwrap();
    let budget = (store.budget().handles(), store.budget().bytes());
    assert_eq!(
        packet.deliver(&mut store, &[left, wrong]).unwrap_err().code,
        "LRR-E-OWNERSHIP"
    );
    assert_eq!(store.state(left).unwrap(), CellState::Uninit);
    assert_eq!(store.state(wrong).unwrap(), CellState::Uninit);
    assert_eq!((store.budget().handles(), store.budget().bytes()), budget);
    packet.deliver(&mut store, &[left, right]).unwrap();
    assert_eq!(
        store.read_scalar(left).unwrap(),
        ValueRepr::signed(TypeId(7), 5).unwrap()
    );
    assert_eq!(
        store.read_scalar(right).unwrap(),
        ValueRepr::signed(TypeId(7), 6).unwrap()
    );
}
#[test]
fn argument_packet_discard_releases_owned_payload_and_reset_invalidates_packets() {
    let mut store = GuestStore::new(1, 1000, 1024 * 1024).unwrap();
    let (value, ty) = owned(&mut store, &"x".repeat(4096));
    let packet = ArgumentPacket::seal(&mut store, &[value], &[ty]).unwrap();
    let before = store.budget().bytes();
    packet.discard(&mut store).unwrap();
    assert!(store.budget().bytes() + 4096 <= before);
    assert_eq!(
        packet.discard(&mut store).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    let next = scalar(&mut store, 9);
    let packet = ArgumentPacket::seal(&mut store, &[next], &[TypeId(7)]).unwrap();
    store.reset(2).unwrap();
    assert_eq!(
        packet.discard(&mut store).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
}

#[test]
fn unresolved_argument_literals_are_not_defaulted_or_bound_during_packet_sealing() {
    let mut store = GuestStore::new(1, 1000, 1024 * 1024).unwrap();
    let source = cell(&mut store, None);
    let ticket = store.integer_literal(false, 3, "3".into()).unwrap();
    store.initialize(source, ticket).unwrap();
    let budget = (store.budget().handles(), store.budget().bytes());
    assert_eq!(
        ArgumentPacket::seal(&mut store, &[source], &[TypeId(7)])
            .err()
            .unwrap()
            .code,
        "LRR-E-TYPE-CONTEXT"
    );
    assert!(matches!(store.state(source).unwrap(),CellState::Live(value)if value.kind==11));
    assert_eq!(
        store.read_scalar(source).unwrap_err().code,
        "LRR-E-TYPE-CONTEXT"
    );
    assert_eq!((store.budget().handles(), store.budget().bytes()), budget);
}
