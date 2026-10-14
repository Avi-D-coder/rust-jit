//! Original AssignPlace operations with disclosed, closed intrinsic runtime setup.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{op::Op, pc::Phase, recipe::VerifiedRecipe},
    runtime::{cell::RefId, place::Place, reference::Access, shape::TypeKey},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{parse::ParsedFile, place_fixture_facts as facts},
};
const REF_SOURCE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/issue-62007-assign-field.rs"
);
const STRING_SOURCE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/field-replace-in-struct-with-drop.rs"
);
struct ProjectedFrame {
    frame: Frame,
    store: GuestStore,
    recipe: VerifiedRecipe,
    root: CellId,
    target: CellId,
    source: SlotId,
    place: SlotId,
    destination: SlotId,
    old_reference: Option<RefId>,
    new_reference: Option<RefId>,
    incoming_text: Option<String>,
}
fn reference(
    store: &mut GuestStore,
    frame: crate::runtime::cell::FrameId,
    slot: u32,
) -> (TicketId, RefId) {
    let referent = store
        .new_cell(
            StorageContract::Exact(TypeId(1)),
            true,
            CellOwner::FrameSlot { frame, slot },
        )
        .unwrap();
    let unit = store.ticket(ValueRepr::unit()).unwrap();
    store.initialize(referent, unit).unwrap();
    let lease = store
        .borrow(&Place::cell(referent), Access::Mutable)
        .unwrap();
    (store.reference_ticket(lease).unwrap(), lease)
}
fn original_projected_assignment(reference_value: bool) -> ProjectedFrame {
    let (original, function) = if reference_value {
        (REF_SOURCE, "to_refs")
    } else {
        (STRING_SOURCE, "test_inline_replacement")
    };
    let facts = facts::projected_assignment(original, function, 0);
    assert_eq!(facts.field, 0);
    if reference_value {
        assert_eq!(facts.lhs_binding, "list");
        assert_eq!(facts.incoming_binding.as_deref(), Some("n"));
    } else {
        assert_eq!(facts.lhs_binding, "f");
        assert!(facts.incoming_binding.is_none());
    }
    let parsed = ParsedFile::parse(
        &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"original-projected-assignment"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == function)
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let body = parsed.index_body(handle, Default::default()).unwrap();
    let assignment = body
        .nodes()
        .iter()
        .find(|node| node.range == facts.assignment)
        .unwrap()
        .clone();
    let pc = body.pc(assignment.id, Phase::Operand(0)).unwrap();
    let recipe = parsed.initial_thunk(handle, &body, &pc).unwrap();
    let [
        Op::AssignPlace {
            place,
            source,
            destination,
        },
    ] = recipe.recipe().operations.as_ref()
    else {
        panic!("original projected assignment opcode absent")
    };
    let (place, source, destination) = (*place, *source, *destination);
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let mut frame = Frame::new(body, pc, &mut store).unwrap();
    let mut scopes = Vec::new();
    let mut scope = Some(assignment.scope);
    while let Some(id) = scope {
        scopes.push(id);
        scope = frame.body.scopes()[id.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    let owner = store.new_frame_anchor().unwrap();
    // Prepared runtime adaptations only: &mut List<T> becomes a closed &mut
    // Unit lease. Foo(String) becomes a structural one-field tuple containing
    // source-derived String bytes. Neither generic/nominal source binding nor
    // Foo's Drop body is bypassed or claimed implemented by this backend check.
    let (old, incoming, old_reference, new_reference) = if reference_value {
        let (old, old_lease) = reference(&mut store, owner, 0);
        let (incoming, new_lease) = reference(&mut store, owner, 1);
        (old, incoming, Some(old_lease), Some(new_lease))
    } else {
        let old = store.string(facts.initial_text.as_ref().unwrap()).unwrap();
        let incoming = store.string(facts.incoming_text.as_ref().unwrap()).unwrap();
        (old, incoming, None, None)
    };
    let ty = TypeId(store.peek_ticket(old).unwrap().type_id);
    let tuple = store.intern_type(TypeKey::Tuple(Box::new([ty]))).unwrap();
    let guard = store.begin_construction(&[ty]).unwrap();
    store.construction_push(guard, 0, old).unwrap();
    let ticket = store.commit_construction(guard, tuple).unwrap();
    let target = store.aggregate_field(ticket, facts.field).unwrap();
    let root = store
        .new_cell(
            StorageContract::Exact(tuple),
            true,
            CellOwner::FrameSlot {
                frame: owner,
                slot: 2,
            },
        )
        .unwrap();
    store.initialize(root, ticket).unwrap();
    let projected = store
        .project_tuple(&Place::cell(root), facts.field)
        .unwrap();
    frame.inject_test_place(place, projected).unwrap();
    frame.write(source, incoming, &mut store).unwrap();
    ProjectedFrame {
        frame,
        store,
        recipe,
        root,
        target,
        source,
        place,
        destination,
        old_reference,
        new_reference,
        incoming_text: facts.incoming_text,
    }
}
fn snapshot(c: &ProjectedFrame) -> (CellState, u32, CellState, u32, CellState, u32, usize) {
    (
        c.store.state(c.target).unwrap(),
        c.store.location_epoch(c.target).unwrap(),
        c.store.state(c.frame.cell(c.source).unwrap()).unwrap(),
        c.store.location_epoch(c.root).unwrap(),
        c.store.state(c.root).unwrap(),
        c.store.budget().handles(),
        c.store.budget().bytes(),
    )
}
fn execute(c: &mut ProjectedFrame) -> Result<crate::vm::step::StepOutcome, Diagnostic> {
    crate::vm::step::step(&mut c.frame, &mut c.store, &c.recipe)
}
#[test]
fn rustc_original_reference_field_assignment_transfers_new_lease_and_retires_old_once() {
    let mut c = original_projected_assignment(true);
    let before = snapshot(&c);
    assert!(matches!(
        execute(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Next(_)
    ));
    assert_eq!(
        c.store.reference_from_cell(c.target).unwrap(),
        c.new_reference.unwrap()
    );
    assert_eq!(
        c.store
            .read_reference(c.old_reference.unwrap())
            .unwrap_err()
            .code,
        "LRR-E-STALE-HANDLE"
    );
    assert_eq!(
        c.store.read_reference(c.new_reference.unwrap()).unwrap(),
        ValueRepr::unit()
    );
    assert_eq!(c.store.location_epoch(c.target).unwrap(), before.1);
    assert_eq!(c.store.location_epoch(c.root).unwrap(), before.3);
    assert_eq!(
        c.store.state(c.frame.cell(c.source).unwrap()).unwrap(),
        CellState::Moved
    );
    assert_eq!(
        c.store
            .read_scalar(c.frame.cell(c.destination).unwrap())
            .unwrap(),
        ValueRepr::unit()
    );
    assert!(c.frame.place_slot(c.place).unwrap().place.is_none());
    c.store.admit_move(c.root).unwrap();
    assert_eq!(c.frame.counters.started_by_opcode[34], 1);
}
#[test]
fn rustc_original_string_field_assignment_installs_rhs_and_reclaims_old_allocation() {
    let mut c = original_projected_assignment(false);
    let before = snapshot(&c);
    let CellState::Live(old) = before.0 else {
        panic!("original old String absent")
    };
    assert!(matches!(
        execute(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Next(_)
    ));
    assert_eq!(c.store.location_epoch(c.target).unwrap(), before.1);
    assert_eq!(c.store.location_epoch(c.root).unwrap(), before.3);
    assert_eq!(
        c.store.admit_new_contract(None, old).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    c.store.admit_move(c.root).unwrap();
    let result = c.store.move_from(c.target).unwrap();
    assert_eq!(
        c.store.string_contents(result).unwrap(),
        c.incoming_text.as_ref().unwrap()
    );
    assert_eq!(c.frame.counters.started_by_opcode[34], 1);
}
#[test]
fn rustc_reference_field_assignment_batch_quota_refusal_preserves_both_lease_owners() {
    let mut c = original_projected_assignment(true);
    while c.store.budget().handles() < 998 {
        c.store.ticket(ValueRepr::unit()).unwrap();
    }
    let before = snapshot(&c);
    assert_eq!(execute(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(snapshot(&c), before);
    assert_eq!(
        c.store.reference_from_cell(c.target).unwrap(),
        c.old_reference.unwrap()
    );
    assert_eq!(
        c.store.read_reference(c.new_reference.unwrap()).unwrap(),
        ValueRepr::unit()
    );
    assert!(c.frame.slot(c.destination).unwrap().cell.is_none());
    assert!(c.frame.place_slot(c.place).unwrap().place.is_some());
}
#[test]
fn rustc_string_field_assignment_zero_fuel_preserves_target_rhs_and_cached_completeness() {
    let mut c = original_projected_assignment(false);
    c.frame.set_fuel_limit(0).unwrap();
    let before = snapshot(&c);
    assert_eq!(execute(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
    let fuel = c.frame.fuel_snapshot();
    assert_eq!(fuel.used, 0);
    assert_eq!(fuel.stop, Some(crate::vm::fuel::FuelStop::Exhausted));
    assert_eq!(snapshot(&c), before);
    c.store.admit_move(c.root).unwrap();
    assert!(c.frame.slot(c.destination).unwrap().cell.is_none());
}
#[test]
fn rustc_reference_field_assignment_shared_enclosing_lease_refuses_before_fuel() {
    let mut c = original_projected_assignment(true);
    let lease = c
        .store
        .borrow(&Place::cell(c.root), Access::Shared)
        .unwrap();
    let place = c.store.project_tuple(&Place::deref(lease), 0).unwrap();
    c.frame.inject_test_place(c.place, place).unwrap();
    let before = snapshot(&c);
    assert_eq!(execute(&mut c).unwrap_err().code, "LRR-E-REFERENCE-ACCESS");
    assert_eq!(snapshot(&c), before);
    assert_eq!(c.frame.fuel_snapshot().used, 0);
}
#[test]
fn rustc_string_field_assignment_wrong_rhs_descriptor_refuses_before_transfer() {
    let mut c = original_projected_assignment(false);
    let owner = c.store.new_frame_anchor().unwrap();
    let (incoming, _) = reference(&mut c.store, owner, 0);
    let ty = TypeId(c.store.peek_ticket(incoming).unwrap().type_id);
    let source = c
        .store
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot {
                frame: owner,
                slot: 1,
            },
        )
        .unwrap();
    c.store.initialize(source, incoming).unwrap();
    // Supplemental forged incoming descriptor; old original RHS remains owned
    // in its existing cell. This is not a new source-valid program observation.
    c.frame.slots[c.source.0 as usize].cell = Some(source);
    let before = snapshot(&c);
    assert_eq!(execute(&mut c).unwrap_err().code, "LRR-E-OWNERSHIP");
    assert_eq!(snapshot(&c), before);
    assert_eq!(c.frame.fuel_snapshot().used, 0);
}
#[test]
fn rustc_reference_field_assignment_moved_enclosing_owner_refuses_stale_address() {
    let mut c = original_projected_assignment(true);
    let owner = c.store.move_from(c.root).unwrap();
    let before = snapshot(&c);
    assert_eq!(execute(&mut c).unwrap_err().code, "LRR-E-STALE-HANDLE");
    assert_eq!(snapshot(&c), before);
    assert_eq!(c.store.peek_ticket(owner).unwrap().kind, 8);
    assert_eq!(c.frame.fuel_snapshot().used, 0);
}

// Supplemental missing-field states applied to the exact original AssignPlace
// operation. The original source's generic/Drop obligations remain unexecuted.
fn extract_projected_field(c: &mut ProjectedFrame) -> TicketId {
    let place = c.store.project_tuple(&Place::cell(c.root), 0).unwrap();
    let ticket = c.store.read_owning_place(&place).unwrap();
    assert_eq!(c.store.state(c.target).unwrap(), CellState::Moved);
    assert_eq!(
        c.store.admit_move(c.root).unwrap_err().code,
        "LRR-E-USE-AFTER-MOVE"
    );
    // Form a fresh address after the move without reading the absent value.
    let fresh = c.store.project_tuple(&Place::cell(c.root), 0).unwrap();
    c.frame.inject_test_place(c.place, fresh).unwrap();
    ticket
}
#[test]
fn rustc_reference_field_assignment_reinitializes_moved_leaf_without_reviving_old_lease() {
    let mut c = original_projected_assignment(true);
    let prior = c
        .store
        .borrow(&Place::cell(c.target), Access::Shared)
        .unwrap();
    let external = extract_projected_field(&mut c);
    let before = snapshot(&c);
    assert!(matches!(
        execute(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Next(_)
    ));
    assert_eq!(
        c.store.reference_from_cell(c.target).unwrap(),
        c.new_reference.unwrap()
    );
    assert_eq!(c.store.location_epoch(c.target).unwrap(), before.1);
    assert_eq!(c.store.location_epoch(c.root).unwrap(), before.3);
    assert_eq!(
        c.store.read_reference(prior).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    // The extracted value still owns the old reference. Initialization has no
    // old-value cleanup obligation and must not destroy that external owner.
    assert_eq!(
        c.store.peek_ticket(external).unwrap().payload,
        c.old_reference.unwrap().bits()
    );
    assert_eq!(
        c.store.read_reference(c.old_reference.unwrap()).unwrap(),
        ValueRepr::unit()
    );
    assert_eq!(
        c.store.read_reference(c.new_reference.unwrap()).unwrap(),
        ValueRepr::unit()
    );
    assert_eq!(
        c.store.state(c.frame.cell(c.source).unwrap()).unwrap(),
        CellState::Moved
    );
    c.store.admit_move(c.root).unwrap();
    assert!(c.frame.place_slot(c.place).unwrap().place.is_none());
    assert_eq!(c.frame.counters.started_by_opcode[34], 1);
}
#[test]
fn rustc_string_field_assignment_reinitializes_moved_leaf_and_preserves_extracted_bytes() {
    let mut c = original_projected_assignment(false);
    let external = extract_projected_field(&mut c);
    let before = snapshot(&c);
    assert!(matches!(
        execute(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Next(_)
    ));
    assert_eq!(c.store.string_contents(external).unwrap(), "foo");
    assert_eq!(c.store.location_epoch(c.target).unwrap(), before.1);
    c.store.admit_move(c.root).unwrap();
    let installed = c.store.move_from(c.target).unwrap();
    assert_eq!(
        c.store.string_contents(installed).unwrap(),
        c.incoming_text.as_ref().unwrap()
    );
}
#[test]
fn rustc_moved_reference_field_assignment_reserves_exact_three_handle_batch() {
    let mut c = original_projected_assignment(true);
    let external = extract_projected_field(&mut c);
    while c.store.budget().handles() < 997 {
        c.store.ticket(ValueRepr::unit()).unwrap();
    }
    assert!(matches!(
        execute(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Next(_)
    ));
    assert_eq!(c.store.budget().handles(), 1000);
    assert_eq!(
        c.store.peek_ticket(external).unwrap().payload,
        c.old_reference.unwrap().bits()
    );
    c.store.admit_move(c.root).unwrap();
}
#[test]
fn rustc_moved_reference_field_assignment_quota_refusal_preserves_incomplete_parent() {
    let mut c = original_projected_assignment(true);
    let external = extract_projected_field(&mut c);
    while c.store.budget().handles() < 998 {
        c.store.ticket(ValueRepr::unit()).unwrap();
    }
    let before = snapshot(&c);
    assert_eq!(execute(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(snapshot(&c), before);
    assert_eq!(
        c.store.admit_move(c.root).unwrap_err().code,
        "LRR-E-USE-AFTER-MOVE"
    );
    assert_eq!(
        c.store.peek_ticket(external).unwrap().payload,
        c.old_reference.unwrap().bits()
    );
    assert!(c.frame.slot(c.destination).unwrap().cell.is_none());
}
#[test]
fn rustc_moved_string_field_assignment_zero_fuel_preserves_missing_leaf_and_owners() {
    let mut c = original_projected_assignment(false);
    let external = extract_projected_field(&mut c);
    c.frame.set_fuel_limit(0).unwrap();
    let before = snapshot(&c);
    assert_eq!(execute(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(
        c.frame.fuel_snapshot().stop,
        Some(crate::vm::fuel::FuelStop::Exhausted)
    );
    assert_eq!(c.frame.fuel_snapshot().used, 0);
    assert_eq!(snapshot(&c), before);
    assert_eq!(c.store.string_contents(external).unwrap(), "foo");
    assert_eq!(
        c.store.admit_move(c.root).unwrap_err().code,
        "LRR-E-USE-AFTER-MOVE"
    );
}
#[test]
fn rustc_moved_reference_field_assignment_shared_parent_refuses_before_effects() {
    let mut c = original_projected_assignment(true);
    let lease = c
        .store
        .borrow(&Place::cell(c.root), Access::Shared)
        .unwrap();
    let external = extract_projected_field(&mut c);
    let place = c.store.project_tuple(&Place::deref(lease), 0).unwrap();
    c.frame.inject_test_place(c.place, place).unwrap();
    let before = snapshot(&c);
    assert_eq!(execute(&mut c).unwrap_err().code, "LRR-E-REFERENCE-ACCESS");
    assert_eq!(snapshot(&c), before);
    assert_eq!(c.frame.fuel_snapshot().used, 0);
    assert_eq!(
        c.store.peek_ticket(external).unwrap().payload,
        c.old_reference.unwrap().bits()
    );
}
#[test]
fn rustc_moved_reference_field_assignment_direct_leaf_retains_owning_parent_permission() {
    let mut c = original_projected_assignment(true);
    let value = c.store.admit_move(c.root).unwrap();
    let whole = c.store.move_from(c.root).unwrap();
    let owner = c.store.new_frame_anchor().unwrap();
    let root = c
        .store
        .new_cell(
            StorageContract::Exact(TypeId(value.type_id)),
            false,
            CellOwner::FrameSlot {
                frame: owner,
                slot: 0,
            },
        )
        .unwrap();
    c.store.initialize(root, whole).unwrap();
    c.root = root;
    let external = extract_projected_field(&mut c);
    c.frame
        .inject_test_place(c.place, Place::cell(c.target))
        .unwrap();
    let before = snapshot(&c);
    assert_eq!(execute(&mut c).unwrap_err().code, "LRR-E-REFERENCE-ACCESS");
    assert_eq!(snapshot(&c), before);
    assert_eq!(c.frame.fuel_snapshot().used, 0);
    assert_eq!(
        c.store.peek_ticket(external).unwrap().payload,
        c.old_reference.unwrap().bits()
    );
}
#[test]
fn rustc_partial_reinitialization_does_not_project_fields_from_uninitialized_ancestor() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-partial-reinit-4.rs"
    );
    assert_eq!(
        facts::parenthesized_field_ranges(ORIGINAL, "stuff").len(),
        1
    );
    // Original `(x.0).0` has an uninitialized enclosing x. Descriptor-only
    // structural placeholders model this refusal; no nominal Drop is executed.
    let mut c = original_projected_assignment(true);
    let inner = c
        .store
        .intern_type(TypeKey::Tuple(Box::new([TypeId(1)])))
        .unwrap();
    let outer = c
        .store
        .intern_type(TypeKey::Tuple(Box::new([inner, inner])))
        .unwrap();
    let owner = c.store.new_frame_anchor().unwrap();
    let root = c
        .store
        .new_cell(
            StorageContract::Exact(outer),
            true,
            CellOwner::FrameSlot {
                frame: owner,
                slot: 0,
            },
        )
        .unwrap();
    let before = (
        c.store.state(root).unwrap(),
        c.store.budget().handles(),
        c.store.budget().bytes(),
    );
    let crate::runtime::place::PlaceFailure::RuntimeFault(error) =
        c.store.project_tuple(&Place::cell(root), 0).err().unwrap()
    else {
        panic!("unexpected bounds panic")
    };
    assert_eq!(error.code, "LRR-E-STALE-HANDLE");
    assert_eq!(
        (
            c.store.state(root).unwrap(),
            c.store.budget().handles(),
            c.store.budget().bytes()
        ),
        before
    );
}
#[test]
fn rustc_moved_unit_field_assignment_result_alias_refuses_before_initialization() {
    // Supplemental forged result aliases the absent Unit leaf. A live result
    // would already fail temporary admission; a moved result needs this guard.
    let mut c = original_projected_assignment(true);
    let owner = c.store.new_frame_anchor().unwrap();
    let tuple = c
        .store
        .intern_type(TypeKey::Tuple(Box::new([TypeId(1)])))
        .unwrap();
    let guard = c.store.begin_construction(&[TypeId(1)]).unwrap();
    let unit = c.store.ticket(ValueRepr::unit()).unwrap();
    c.store.construction_push(guard, 0, unit).unwrap();
    let whole = c.store.commit_construction(guard, tuple).unwrap();
    let target = c.store.aggregate_field(whole, 0).unwrap();
    let root = c
        .store
        .new_cell(
            StorageContract::Exact(tuple),
            true,
            CellOwner::FrameSlot {
                frame: owner,
                slot: 0,
            },
        )
        .unwrap();
    c.store.initialize(root, whole).unwrap();
    let source = c
        .store
        .new_cell(
            StorageContract::Exact(TypeId(1)),
            true,
            CellOwner::FrameSlot {
                frame: owner,
                slot: 1,
            },
        )
        .unwrap();
    let incoming = c.store.ticket(ValueRepr::unit()).unwrap();
    c.store.initialize(source, incoming).unwrap();
    let extracted = c.store.move_from(target).unwrap();
    c.root = root;
    c.target = target;
    c.frame.slots[c.source.0 as usize].cell = Some(source);
    c.frame.slots[c.destination.0 as usize].cell = Some(target);
    c.frame
        .inject_test_place(
            c.place,
            c.store.project_tuple(&Place::cell(root), 0).unwrap(),
        )
        .unwrap();
    let before = snapshot(&c);
    let error = execute(&mut c).unwrap_err();
    assert_eq!(error.code, "LRR-E-OWNERSHIP");
    assert_eq!(
        error.message,
        "assignment result aliases its reached target"
    );
    assert_eq!(snapshot(&c), before);
    assert_eq!(c.frame.fuel_snapshot().used, 0);
    assert_eq!(c.store.peek_ticket(extracted).unwrap(), ValueRepr::unit());
    assert_eq!(
        c.store.admit_move(root).unwrap_err().code,
        "LRR-E-USE-AFTER-MOVE"
    );
}
