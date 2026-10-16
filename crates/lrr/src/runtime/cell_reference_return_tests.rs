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
    let current = current.location().unwrap();
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

const POINT_SOURCE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-borrow-scope.rs"
);
fn point_members() -> (
    crate::frontend::types::BoundNominalVariant,
    crate::frontend::types::BoundNominalField,
) {
    use crate::frontend::types::{NominalBinding, NominalFieldBinding};
    let ((bound, field), _) = super::super::tests::stock_schema_selection(
        POINT_SOURCE,
        "regions-infer-borrow-scope.rs",
        "Point",
        |names, caller| {
            Ok((
                names.bind_variant(None)?,
                names.bind_named_field(None, "x", caller)?,
            ))
        },
    );
    let (NominalBinding::Ready(bound), NominalFieldBinding::Ready(field)) = (bound, field) else {
        panic!("unchanged source Point member not selected")
    };
    (bound, field)
}
fn point(
    store: &mut GuestStore,
    frame: FrameId,
    slot: u32,
    bound: &crate::frontend::types::BoundNominalVariant,
) -> (CellId, TypeId) {
    let (ty, guard) = store.begin_nominal_construction(bound).unwrap();
    for (index, value) in [3, 4].into_iter().enumerate() {
        let ticket = store
            .ticket(ValueRepr::signed(TypeId(8), value).unwrap())
            .unwrap();
        store
            .construction_push(guard, index as u32, ticket)
            .unwrap();
    }
    let ticket = store.commit_construction(guard, ty).unwrap();
    let cell = store
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot { frame, slot },
        )
        .unwrap();
    store.initialize(cell, ticket).unwrap();
    (cell, ty)
}

#[test]
fn selected_source_field_return_survives_reborrow_and_input_reference_cleanup() {
    let (bound, field) = point_members();
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let frame = store.new_frame_anchor().unwrap();
    let (root, ty) = point(&mut store, frame, 0, &bound);
    let argument = reference_cell(&mut store, frame, 1, root, Access::Shared);
    let receipt = store.reference_target(argument).unwrap();
    let lease = store.reference_from_cell(argument).unwrap();
    let selected = store
        .project_source_field(&Place::deref(lease), &field)
        .unwrap();
    // Separate arena capacity from retained receipt ownership, then check the
    // actual lease allocation charges its source evidence as well as its path.
    store.references.reserve(1, &mut store.budget).unwrap();
    store.tickets.reserve(1, &mut store.budget).unwrap();
    let before_bytes = store.budget().bytes();
    let result = store.borrow_ticket(&selected, Access::Shared).unwrap();
    store
        .require_reference_projection_return(result, &receipt, false, ty, TypeId(8))
        .unwrap();
    assert!(
        store
            .require_reference_return(result, &receipt, false, TypeId(8))
            .is_err()
    );
    let actual = RefId::from_bits(store.peek_ticket(result).unwrap().payload);
    let record = store.references.get(actual).unwrap();
    assert_eq!(record.location().unwrap().source_projections.len(), 1);
    assert_eq!(
        store.budget().bytes() - before_bytes,
        record.owned_bytes().unwrap()
    );
    assert!(
        record.owned_bytes().unwrap()
            > checked_chain_bytes(record.location().unwrap().enclosing.capacity()).unwrap()
    );
    let again = store
        .borrow_ticket(&Place::deref(actual), Access::Shared)
        .unwrap();
    store
        .require_reference_projection_return(again, &receipt, false, ty, TypeId(8))
        .unwrap();
    let cleanup = store.begin_drop(argument).unwrap();
    store.discard(cleanup).unwrap();
    store.retire_cell(argument).unwrap();
    store
        .require_reference_projection_return(again, &receipt, false, ty, TypeId(8))
        .unwrap();
    assert_eq!(store.read_reference(actual).unwrap().payload, 3);
}

#[test]
fn raw_field_equal_target_wrong_input_and_wrong_source_type_cannot_prove_a_return() {
    let (bound, field) = point_members();
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let frame = store.new_frame_anchor().unwrap();
    let (root, ty) = point(&mut store, frame, 0, &bound);
    let (other, _) = point(&mut store, frame, 1, &bound);
    let argument = reference_cell(&mut store, frame, 2, root, Access::Shared);
    let receipt = store.reference_target(argument).unwrap();
    let lease = store.reference_from_cell(argument).unwrap();
    let selected = store
        .project_source_field(&Place::deref(lease), &field)
        .unwrap();
    let raw = Place::deref(lease)
        .projected(Projection::Field(field.field()))
        .unwrap();
    let raw_ticket = store.borrow_ticket(&raw, Access::Shared).unwrap();
    let actual = store.borrow_ticket(&selected, Access::Shared).unwrap();
    let other_place = store
        .project_source_field(&Place::cell(other), &field)
        .unwrap();
    let other_ticket = store.borrow_ticket(&other_place, Access::Shared).unwrap();
    let before = (store.budget().handles(), store.budget().bytes());
    for ticket in [raw_ticket, other_ticket] {
        assert!(
            store
                .require_reference_projection_return(ticket, &receipt, false, ty, TypeId(8))
                .is_err()
        );
        assert!(store.peek_ticket(ticket).is_ok());
    }
    assert!(
        store
            .require_reference_projection_return(actual, &receipt, false, TypeId(8), TypeId(8))
            .is_err()
    );
    assert!(
        store
            .require_reference_projection_return(actual, &receipt, false, ty, TypeId(13))
            .is_err()
    );
    assert!(
        store
            .require_reference_projection_return(actual, &receipt, true, ty, TypeId(8))
            .is_err()
    );
    assert_eq!(before, (store.budget().handles(), store.budget().bytes()));
    store
        .require_reference_projection_return(actual, &receipt, false, ty, TypeId(8))
        .unwrap();
}

#[test]
fn derived_result_checks_source_receipt_and_original_intermediate_ownership() {
    let (bound, field) = point_members();
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let frame = store.new_frame_anchor().unwrap();
    let (first, ty) = point(&mut store, frame, 0, &bound);
    let (second, _) = point(&mut store, frame, 1, &bound);
    let tuple_ty = store
        .intern_type(TypeKey::Tuple(Box::new([ty, ty])))
        .unwrap();
    let guard = store.begin_construction(&[ty, ty]).unwrap();
    for (index, cell) in [first, second].into_iter().enumerate() {
        let ticket = store.move_from(cell).unwrap();
        store
            .construction_push(guard, index as u32, ticket)
            .unwrap();
    }
    let tuple = store.commit_construction(guard, tuple_ty).unwrap();
    let root = store
        .new_cell(
            StorageContract::Exact(tuple_ty),
            true,
            CellOwner::FrameSlot { frame, slot: 2 },
        )
        .unwrap();
    store.initialize(root, tuple).unwrap();
    let (object, storage) = store.place_object(root).unwrap();
    let [leaf, sibling] = [storage.fields[0], storage.fields[1]];
    let argument = reference_cell(&mut store, frame, 3, leaf, Access::Shared);
    let receipt = store.reference_target(argument).unwrap();
    let lease = store.reference_from_cell(argument).unwrap();
    let selected = store
        .project_source_field(&Place::deref(lease), &field)
        .unwrap();
    let result = store.borrow_ticket(&selected, Access::Shared).unwrap();
    store
        .require_reference_projection_return(result, &receipt, false, ty, TypeId(8))
        .unwrap();
    let actual = RefId::from_bits(store.peek_ticket(result).unwrap().payload);
    let source = store
        .references
        .get(actual)
        .unwrap()
        .location()
        .unwrap()
        .source_projections[0];
    assert_eq!(source.chain_index, 1);
    // Adversarial corruption controls only: production cannot mint these
    // receipts except through the checked selected-member operation.
    for wrong in [0, 2] {
        store
            .references
            .get_mut(actual)
            .unwrap()
            .location_mut()
            .unwrap()
            .source_projections[0]
            .chain_index = wrong;
        assert!(
            store
                .require_reference_projection_return(result, &receipt, false, ty, TypeId(8))
                .is_err()
        );
    }
    store
        .references
        .get_mut(actual)
        .unwrap()
        .location_mut()
        .unwrap()
        .source_projections[0] = source;
    store
        .references
        .get_mut(actual)
        .unwrap()
        .location_mut()
        .unwrap()
        .source_projections[0]
        .member
        .schema = crate::core::digest::Digest::of_bytes(b"wrong selected schema");
    assert!(
        store
            .require_reference_projection_return(result, &receipt, false, ty, TypeId(8))
            .is_err()
    );
    store
        .references
        .get_mut(actual)
        .unwrap()
        .location_mut()
        .unwrap()
        .source_projections[0] = source;
    store
        .require_reference_projection_return(result, &receipt, false, ty, TypeId(8))
        .unwrap();
    let endpoints = (
        store.place_stamp(root).unwrap(),
        store.place_stamp(leaf).unwrap(),
    );
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
    // A fresh certified result follows the changed chain. The original input
    // receipt remains stale even though its root and final stamps still match.
    let current = store
        .project_source_field(&Place::cell(leaf), &field)
        .unwrap();
    let new_result = store.borrow_ticket(&current, Access::Shared).unwrap();
    assert!(
        store
            .require_reference_projection_return(new_result, &receipt, false, ty, TypeId(8))
            .is_err()
    );
    assert!(store.peek_ticket(new_result).is_ok());
}

#[test]
fn selected_projection_receipt_cannot_cross_store_sessions() {
    let (bound, field) = point_members();
    let mut a = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let af = a.new_frame_anchor().unwrap();
    let (av, ty) = point(&mut a, af, 0, &bound);
    let ar = reference_cell(&mut a, af, 1, av, Access::Shared);
    let receipt = a.reference_target(ar).unwrap();
    let mut b = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let bf = b.new_frame_anchor().unwrap();
    let (bv, bty) = point(&mut b, bf, 0, &bound);
    let br = reference_cell(&mut b, bf, 1, bv, Access::Shared);
    assert_eq!(ty, bty);
    assert_eq!(av, bv);
    let lease = b.reference_from_cell(br).unwrap();
    let place = b
        .project_source_field(&Place::deref(lease), &field)
        .unwrap();
    let ticket = b.borrow_ticket(&place, Access::Shared).unwrap();
    let before = (b.budget().handles(), b.budget().bytes());
    assert!(
        b.require_reference_projection_return(ticket, &receipt, false, ty, TypeId(8))
            .is_err()
    );
    assert_eq!(before, (b.budget().handles(), b.budget().bytes()));
    assert!(b.peek_ticket(ticket).is_ok());
}
