//! Original rustc assignment operations with disclosed prepared RHS/binding state.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{pc::Phase, recipe::VerifiedRecipe},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{body_index::NodeRole, parse::ParsedFile, place_fixture_facts as facts},
};

const UNIT_ASSIGNMENT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/type/unit-type-basic-usages.rs"
);

const STRING_REPLACEMENT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/issue-58776-borrowck-scans-children.rs"
);
const STRING_INITIALIZATION: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/nll/issue-61424.rs");
const IMMUTABLE_ASSIGNMENT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/assign-imm-local-twice.rs"
);

struct AssignmentFrame {
    frame: Frame,
    store: GuestStore,
    recipe: VerifiedRecipe,
    binding: NodeId,
    source: SlotId,
    destination: SlotId,
    incoming_text: Option<String>,
}

fn original_assignment(source: &str, local: &str, ordinal: usize) -> AssignmentFrame {
    let facts = facts::local_assignment(source, local, ordinal);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"rustc-original-local-assignment"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let body = parsed.index_body(handle, Default::default()).unwrap();
    let pattern = body
        .nodes()
        .iter()
        .find(|node| node.range == facts.pattern && node.role == NodeRole::Pattern)
        .unwrap()
        .id;
    let assignment = body
        .nodes()
        .iter()
        .find(|node| node.range == facts.assignment)
        .unwrap()
        .clone();
    let pc = body.pc(assignment.id, Phase::Operand(1)).unwrap();
    let recipe = parsed.initial_thunk(handle, &body, &pc).unwrap();
    let [
        crate::ir::op::Op::AssignLocal {
            binding,
            source,
            destination,
        },
    ] = recipe.recipe().operations.as_ref()
    else {
        panic!("original checked assignment operation absent")
    };
    assert_eq!(*binding, pattern);
    let (source, destination) = (*source, *destination);
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(body, pc, &mut store).unwrap();
    let mut scopes = Vec::new();
    let mut scope = Some(assignment.scope);
    while let Some(id) = scope {
        scopes.push(id);
        scope = frame.body.scopes()[id.0 as usize].parent;
    }
    for id in scopes.into_iter().rev() {
        frame.enter(id).unwrap();
    }
    let hint = facts
        .annotation
        .as_deref()
        .map(|annotation| match annotation {
            "isize" => TypeId(8),
            "()" => TypeId(1),
            _ => panic!("unselected original local annotation"),
        });
    // Supply the declaration and completed initializer/RHS, without running
    // the original closure, String methods, println/dbg or later statements.
    frame
        .declare_local(pattern, hint, facts.writable, &mut store)
        .unwrap();
    if let Some(text) = facts.initial_text {
        let ticket = store.string(&text).unwrap();
        store
            .initialize(frame.binding(pattern).unwrap(), ticket)
            .unwrap();
    }
    if facts.incoming_unit {
        // Original f(u1) returns Unit. This scoped harness supplies that completed
        // call result; the unchanged full source is a separate CLI observation.
        let ticket = store.ticket(ValueRepr::unit()).unwrap();
        store
            .initialize(frame.binding(pattern).unwrap(), ticket)
            .unwrap();
    }
    let incoming = match &facts.incoming_text {
        Some(text) => store.string(text).unwrap(),
        None if facts.incoming_unit => store.ticket(ValueRepr::unit()).unwrap(),
        None => store
            .ticket(ValueRepr::signed(hint.unwrap(), facts.incoming_integer.unwrap()).unwrap())
            .unwrap(),
    };
    frame.write(source, incoming, &mut store).unwrap();
    AssignmentFrame {
        frame,
        store,
        recipe,
        binding: pattern,
        source,
        destination,
        incoming_text: facts.incoming_text,
    }
}

fn states(context: &AssignmentFrame) -> (CellState, u32, CellState, u32, u32, usize) {
    let source = context.frame.cell(context.source).unwrap();
    let target = context.frame.binding(context.binding).unwrap();
    (
        context.store.state(source).unwrap(),
        context.store.location_epoch(source).unwrap(),
        context.store.state(target).unwrap(),
        context.store.location_epoch(target).unwrap(),
        context.store.budget().handles(),
        context.store.budget().bytes(),
    )
}

fn execute(context: &mut AssignmentFrame) -> Result<crate::vm::step::StepOutcome, Diagnostic> {
    crate::vm::step::step(&mut context.frame, &mut context.store, &context.recipe)
}

#[test]
fn rustc_original_string_assignment_transfers_rhs_and_reclaims_old_owner_once() {
    let mut context = original_assignment(STRING_REPLACEMENT, "greeting", 0);
    let before = states(&context);
    let CellState::Live(old) = before.2 else {
        panic!("original old String absent")
    };
    assert!(matches!(
        execute(&mut context).unwrap(),
        crate::vm::step::StepOutcome::Next(_)
    ));
    let target = context.frame.binding(context.binding).unwrap();
    assert_eq!(
        context
            .store
            .state(context.frame.cell(context.source).unwrap())
            .unwrap(),
        CellState::Moved
    );
    assert_eq!(
        context
            .store
            .location_epoch(context.frame.cell(context.source).unwrap())
            .unwrap(),
        before.1 + 1
    );
    assert_eq!(context.store.state(target).unwrap(), before.0);
    assert_eq!(context.store.location_epoch(target).unwrap(), before.3);
    assert_eq!(
        context
            .store
            .admit_new_contract(None, old)
            .unwrap_err()
            .code,
        "LRR-E-STALE-HANDLE"
    );
    assert_eq!(
        context
            .store
            .state(context.frame.cell(context.destination).unwrap())
            .unwrap(),
        CellState::Live(ValueRepr::unit())
    );
    assert_eq!(context.frame.counters.started_by_opcode[16], 1);
    assert_eq!(context.frame.counters.operations_completed, 1);
    let ticket = context.store.move_from(target).unwrap();
    assert_eq!(
        context.store.string_contents(ticket).unwrap(),
        context.incoming_text.unwrap()
    );
}

#[test]
fn rustc_original_uninitialized_string_assignment_establishes_storage_contract() {
    let mut context = original_assignment(STRING_INITIALIZATION, "x", 0);
    let target = context.frame.binding(context.binding).unwrap();
    assert_eq!(context.store.state(target).unwrap(), CellState::Uninit);
    assert_eq!(
        context.store.declared(target).unwrap(),
        StorageContract::Unbound
    );
    let before = states(&context);
    execute(&mut context).unwrap();
    let CellState::Live(incoming) = before.0 else {
        panic!("original RHS absent")
    };
    assert_eq!(context.store.state(target).unwrap(), before.0);
    assert_eq!(
        context.store.declared(target).unwrap(),
        StorageContract::Exact(TypeId(incoming.type_id))
    );
    assert_eq!(context.store.location_epoch(target).unwrap(), before.3);
    let ticket = context.store.move_from(target).unwrap();
    assert_eq!(
        context.store.string_contents(ticket).unwrap(),
        context.incoming_text.unwrap()
    );
}

#[test]
fn rustc_string_assignment_reinitializes_moved_location_without_reviving_old_epoch() {
    let mut context = original_assignment(STRING_REPLACEMENT, "greeting", 0);
    let target = context.frame.binding(context.binding).unwrap();
    let old = context.store.move_from(target).unwrap();
    let before = states(&context);
    assert_eq!(before.2, CellState::Moved);
    execute(&mut context).unwrap();
    assert_eq!(context.store.state(target).unwrap(), before.0);
    assert_eq!(context.store.location_epoch(target).unwrap(), before.3);
    // Supplemental moved-destination state. Its previous value remains owned
    // by its external ticket and must not be dropped by reinitialization.
    assert_eq!(
        context.store.string_contents(old).unwrap(),
        facts::local_assignment(STRING_REPLACEMENT, "greeting", 0)
            .initial_text
            .unwrap()
    );
    context.store.discard(old).unwrap();
}

#[test]
fn rustc_original_immutable_local_accepts_first_assignment_and_refuses_second() {
    let mut first = original_assignment(IMMUTABLE_ASSIGNMENT, "v", 0);
    execute(&mut first).unwrap();
    let mut second = original_assignment(IMMUTABLE_ASSIGNMENT, "v", 1);
    let target = second.frame.binding(second.binding).unwrap();
    let facts = facts::local_assignment(IMMUTABLE_ASSIGNMENT, "v", 0);
    let original_first = ValueRepr::signed(TypeId(8), facts.incoming_integer.unwrap()).unwrap();
    let ticket = second.store.ticket(original_first).unwrap();
    second.store.initialize(target, ticket).unwrap();
    let before = states(&second);
    assert_eq!(execute(&mut second).unwrap_err().code, "LRR-E-OWNERSHIP");
    assert_eq!(states(&second), before);
    assert_eq!(second.frame.fuel_snapshot().used, 0);
    assert!(
        second
            .frame
            .slot(second.destination)
            .unwrap()
            .cell
            .is_none()
    );
}

#[test]
fn rustc_string_assignment_forged_rhs_target_alias_refuses_before_fuel() {
    let mut context = original_assignment(STRING_REPLACEMENT, "greeting", 0);
    let target = context.frame.binding(context.binding).unwrap();
    // A malformed prepared frame cannot consume its own destination as RHS.
    // This differs from valid source `x=x`, whose RHS has a separate ticket.
    context.frame.slots[context.source.0 as usize].cell = Some(target);
    let before = states(&context);
    assert_eq!(execute(&mut context).unwrap_err().code, "LRR-E-OWNERSHIP");
    assert_eq!(states(&context), before);
    assert_eq!(context.frame.fuel_snapshot().used, 0);
    assert!(
        context
            .frame
            .slot(context.destination)
            .unwrap()
            .cell
            .is_none()
    );
}

#[test]
fn local_assignment_forged_structural_owner_refuses_before_owner_change() {
    let mut context = original_assignment(STRING_REPLACEMENT, "greeting", 0);
    // LRR-specific forged-owner control. Closed tuples now have a selected
    // intrinsic cleanup policy, but their binding owner must still use the exact
    // indexed FrameSlot ordinal. The deliberately forged 0/1 owners below must
    // refuse before fuel or either owner changes. This does not execute the
    // original String assignment's source types.
    for old in [
        context.frame.binding(context.binding).unwrap(),
        context.frame.cell(context.source).unwrap(),
    ] {
        let ticket = context.store.begin_drop(old).unwrap();
        context.store.discard(ticket).unwrap();
        context.store.retire_cell(old).unwrap();
    }
    let ty = context
        .store
        .intern_type(crate::runtime::shape::TypeKey::Tuple(Box::new([
            TypeId(1),
            TypeId(1),
        ])))
        .unwrap();
    let mut cells = Vec::new();
    for slot in 0..2 {
        let guard = context
            .store
            .begin_construction(&[TypeId(1), TypeId(1)])
            .unwrap();
        for field in 0..2 {
            let unit = context.store.ticket(ValueRepr::unit()).unwrap();
            context.store.construction_push(guard, field, unit).unwrap();
        }
        let ticket = context.store.commit_construction(guard, ty).unwrap();
        let cell = context
            .store
            .new_cell(
                StorageContract::Exact(ty),
                true,
                CellOwner::FrameSlot {
                    frame: context.frame.anchor,
                    slot,
                },
            )
            .unwrap();
        context.store.initialize(cell, ticket).unwrap();
        cells.push(cell);
    }
    context.frame.bindings[context.binding.0 as usize] = Some(cells[0]);
    context.frame.slots[context.source.0 as usize].cell = Some(cells[1]);
    let before = states(&context);
    let error = execute(&mut context).unwrap_err();
    assert_eq!(error.code, "LRR-E-INTERNAL-IR");
    assert_eq!(
        error.message,
        "replacement local location belongs to another frame slot"
    );
    assert_eq!(states(&context), before);
    assert_eq!(context.frame.fuel_snapshot().used, 0);
    assert!(
        context
            .frame
            .slot(context.destination)
            .unwrap()
            .cell
            .is_none()
    );
}

#[test]
fn rustc_original_unit_assignment_replaces_after_completed_rhs_without_epoch_change() {
    let mut context = original_assignment(UNIT_ASSIGNMENT, "_u2", 0);
    let before = states(&context);
    assert!(matches!(
        execute(&mut context).unwrap(),
        crate::vm::step::StepOutcome::Next(_)
    ));
    let target = context.frame.binding(context.binding).unwrap();
    assert_eq!(
        context.store.read_scalar(target).unwrap(),
        ValueRepr::unit()
    );
    assert_eq!(context.store.location_epoch(target).unwrap(), before.3);
    assert_eq!(
        context
            .store
            .state(context.frame.cell(context.source).unwrap())
            .unwrap(),
        CellState::Moved
    );
    // The exact original recipe has one AssignLocal operation and one Next
    // exit. Both are charged by vm/step.rs before successful completion.
    assert_eq!(context.frame.counters.operations_completed, 1);
    assert_eq!(context.frame.counters.exits_completed, 1);
    assert_eq!(context.frame.fuel_snapshot().used, 2);
}
