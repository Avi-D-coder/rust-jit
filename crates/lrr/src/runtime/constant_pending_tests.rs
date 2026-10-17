//! Physical unpublished-owner tests; no source authority or whole-fixture pass.
use super::*;
use crate::{ir::constant_data::wire, ir::op::PrimitiveType};
fn value() -> (ConstValue, ConstantWireType) {
    let w = ConstantWireType::Array {
        element: Box::new(ConstantWireType::Scalar(PrimitiveType::Unit)),
        length: 1,
    };
    let mut b = b"LRRCNST\0".to_vec();
    b.extend_from_slice(&1u16.to_le_bytes());
    b.push(1);
    b.extend_from_slice(&1u64.to_le_bytes());
    b.push(0);
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&0u64.to_le_bytes());
    (wire::decode(&b, &w, 1 << 20).unwrap(), w)
}
#[test]
fn pending_constant_abort_retires_only_private_owner() {
    let mut s = GuestStore::for_epoch(1).unwrap();
    let (v, w) = value();
    let ty = s
        .intern_type(TypeKey::Array {
            element: TypeId(1),
            len: 1,
        })
        .unwrap();
    let plan = s.prepare_constant_instantiation(&v, &w, ty).unwrap();
    let mut f = RunSteps::default();
    f.charge();
    let pending = s.instantiate_pending_constant(plan, &mut f).unwrap();
    let ticket = pending.ticket.unwrap();
    let repr = s.peek_ticket(ticket).unwrap();
    s.abort_pending_constant(pending).unwrap();
    assert!(s.peek_ticket(ticket).is_err());
    assert!(s.objects.get(ObjectId::from_bits(repr.payload)).is_err());
    assert_eq!(f.snapshot().used, 2);
}
#[test]
fn pending_constant_bad_destination_retains_owner_then_commit_consumes_once() {
    let mut s = GuestStore::for_epoch(1).unwrap();
    let (v, w) = value();
    let ty = s
        .intern_type(TypeKey::Array {
            element: TypeId(1),
            len: 1,
        })
        .unwrap();
    let plan = s.prepare_constant_instantiation(&v, &w, ty).unwrap();
    let mut f = RunSteps::default();
    let mut pending = s.instantiate_pending_constant(plan, &mut f).unwrap();
    let ticket = pending.ticket.unwrap();
    let frame = s.new_frame_anchor().unwrap();
    let wrong = s
        .new_cell(
            StorageContract::Exact(TypeId(2)),
            true,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    assert!(s.commit_pending_constant(&mut pending, wrong).is_err());
    assert_eq!(pending.ticket, Some(ticket));
    assert!(s.peek_ticket(ticket).is_ok());
    assert_eq!(s.state(wrong).unwrap(), CellState::Uninit);
    let destination = s
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot { frame, slot: 1 },
        )
        .unwrap();
    assert!(s.commit_pending_constant(&mut pending, destination).is_ok());
    assert!(s.peek_ticket(ticket).is_err());
    assert_eq!(pending.ticket, None);
    let before = (
        s.budget().handles(),
        s.budget().bytes(),
        s.state(destination).unwrap(),
    );
    assert_eq!(
        s.commit_pending_constant(&mut pending, destination)
            .unwrap_err()
            .code,
        "LRR-E-STALE-HANDLE"
    );
    assert_eq!(
        before,
        (
            s.budget().handles(),
            s.budget().bytes(),
            s.state(destination).unwrap()
        )
    );
    assert!(matches!(s.state(destination).unwrap(), CellState::Live(_)));
}

#[test]
fn store_session_prepared_constant_refuses_equal_epoch_other_store_before_charge() {
    let a = GuestStore::for_epoch(1).unwrap();
    let mut b = GuestStore::for_epoch(1).unwrap();
    let v = ConstValue::from_scalar(ValueRepr::unit()).unwrap();
    let w = ConstantWireType::Scalar(PrimitiveType::Unit);
    let plan = a.prepare_constant_instantiation(&v, &w, TypeId(1)).unwrap();
    let before = (b.budget().handles(), b.budget().bytes());
    let mut fuel = RunSteps::default();
    let error = b.instantiate_constant(plan, &mut fuel).unwrap_err();
    assert_eq!(error.code, "LRR-E-STALE-HANDLE");
    assert_eq!(fuel.snapshot().used, 0);
    assert_eq!(before, (b.budget().handles(), b.budget().bytes()));
}

#[test]
fn store_session_pending_owner_refuses_colliding_actual_ticket_and_destination() {
    let mut a = GuestStore::for_epoch(1).unwrap();
    let mut b = GuestStore::for_epoch(1).unwrap();
    let v = ConstValue::from_scalar(ValueRepr::unit()).unwrap();
    let w = ConstantWireType::Scalar(PrimitiveType::Unit);
    let pa = a.prepare_constant_instantiation(&v, &w, TypeId(1)).unwrap();
    let pb = b.prepare_constant_instantiation(&v, &w, TypeId(1)).unwrap();
    let mut fa = RunSteps::default();
    let mut fb = RunSteps::default();
    let mut pending_a = a.instantiate_pending_constant(pa, &mut fa).unwrap();
    let mut pending_b = b.instantiate_pending_constant(pb, &mut fb).unwrap();
    assert_eq!(
        pending_a.ticket.unwrap().bits(),
        pending_b.ticket.unwrap().bits()
    );
    let frame = b.new_frame_anchor().unwrap();
    let destination = b
        .new_cell(
            StorageContract::Exact(TypeId(1)),
            true,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    let before = (b.budget().handles(), b.budget().bytes());
    let error = b
        .commit_pending_constant(&mut pending_a, destination)
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-STALE-HANDLE");
    assert_eq!(b.state(destination).unwrap(), CellState::Uninit);
    assert_eq!(before, (b.budget().handles(), b.budget().bytes()));
    assert!(a.validate_pending_constant(&pending_a).is_ok());
    assert!(b.validate_pending_constant(&pending_b).is_ok());
    a.abort_pending_constant(pending_a).unwrap();
    assert!(
        b.commit_pending_constant(&mut pending_b, destination)
            .is_ok()
    );
}
