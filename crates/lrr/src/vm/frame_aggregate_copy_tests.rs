//! Original owning reads with disclosed completed initializers and type context.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{op::Op, pc::Phase, recipe::VerifiedRecipe},
    runtime::{
        place::{Place, Projection},
        reference::Access,
        shape::TypeKey,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{body_index::NodeRole, parse::ParsedFile, place_fixture_facts as facts},
};
const TUPLE: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/tuple/tup.rs");
const ARRAY: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/fixed_length_copy.rs"
);
struct Context {
    frame: Frame,
    store: GuestStore,
    recipe: VerifiedRecipe,
    source: CellId,
    destination: SlotId,
    values: Vec<i64>,
    element: TypeId,
}
fn aggregate(store: &mut GuestStore, values: &[i64], element: TypeId, tuple: bool) -> TicketId {
    let fields = vec![element; values.len()];
    let ty = store
        .intern_type(if tuple {
            TypeKey::Tuple(fields.clone().into())
        } else {
            TypeKey::Array {
                element,
                len: u32::try_from(fields.len()).unwrap(),
            }
        })
        .unwrap();
    let guard = store.begin_construction(&fields).unwrap();
    for (index, value) in values.iter().enumerate() {
        let ticket = store
            .ticket(ValueRepr::signed(element, *value).unwrap())
            .unwrap();
        store
            .construction_push(guard, u32::try_from(index).unwrap(), ticket)
            .unwrap();
    }
    store.commit_construction(guard, ty).unwrap()
}
fn context(tuple: bool, expanded: bool) -> Context {
    let original = if tuple { TUPLE } else { ARRAY };
    let facts = facts::aggregate_copy_read(
        original,
        if tuple { "p" } else { "arr" },
        if tuple { "p2" } else { "arr2" },
    );
    assert_eq!(facts.tuple, tuple);
    // Chunk-boundary controls expand only the prepared runtime initializer;
    // the exact read operation still comes from unchanged upstream source.
    let values = if expanded {
        facts.values.iter().copied().cycle().take(257).collect()
    } else {
        facts.values
    };
    let parsed = ParsedFile::parse(
        &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"original-aggregate-copy"),
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
    let node = body
        .nodes()
        .iter()
        .find(|node| node.range == facts.read)
        .unwrap()
        .clone();
    let pc = body.pc(node.id, Phase::Entry).unwrap();
    let recipe = parsed.initial_thunk(handle, &body, &pc).unwrap();
    let [
        Op::ReadLocal {
            binding,
            destination,
        },
    ] = recipe.recipe().operations.as_ref()
    else {
        panic!("original aggregate owning read absent")
    };
    assert_eq!(*binding, pattern);
    let destination = *destination;
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(body, pc, &mut store).unwrap();
    let mut scopes = vec![];
    let mut scope = Some(node.scope);
    while let Some(id) = scope {
        scopes.push(id);
        scope = frame.body.scopes()[id.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    let element = if tuple { TypeId(8) } else { TypeId(6) };
    let ticket = aggregate(&mut store, &values, element, tuple);
    let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
    frame
        .declare_local(pattern, Some(ty), false, &mut store)
        .unwrap();
    let source = frame.binding(pattern).unwrap();
    store.initialize(source, ticket).unwrap();
    Context {
        frame,
        store,
        recipe,
        source,
        destination,
        values,
        element,
    }
}
fn run(context: &mut Context) -> Result<crate::vm::step::StepOutcome, Diagnostic> {
    crate::vm::step::step(&mut context.frame, &mut context.store, &context.recipe)
}
fn root_cell(store: &mut GuestStore, ticket: TicketId) -> CellId {
    let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
    let frame = store.new_frame_anchor().unwrap();
    let cell = store
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    store.initialize(cell, ticket).unwrap();
    cell
}
fn finish_kernel(
    store: &mut GuestStore,
    copy: &mut crate::runtime::cell::aggregate_copy::AggregateCopy,
) -> TicketId {
    for _ in 0..4096 {
        if let crate::runtime::cell::aggregate_copy::CopyProgress::Complete(ticket) =
            copy.advance(store).unwrap()
        {
            return ticket;
        }
    }
    panic!("bounded copy did not finish")
}
fn field(store: &GuestStore, root: CellId, index: u32, tuple: bool) -> CellId {
    let projection = if tuple {
        Projection::Tuple(index)
    } else {
        Projection::Index(index)
    };
    store
        .admit_owning_place_read(&Place::cell(root).projected(projection).unwrap())
        .unwrap()
        .0
}
fn original_read(tuple: bool) {
    let mut c = context(tuple, false);
    let before = (
        c.store.state(c.source).unwrap(),
        c.store.location_epoch(c.source).unwrap(),
    );
    assert!(matches!(
        run(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Next(_)
    ));
    assert_eq!(
        (
            c.store.state(c.source).unwrap(),
            c.store.location_epoch(c.source).unwrap()
        ),
        before
    );
    let destination = c.frame.cell(c.destination).unwrap();
    let old = c.store.admit_move(c.source).unwrap();
    let copy = c.store.admit_move(destination).unwrap();
    assert_ne!(old.payload, copy.payload);
    assert_eq!(old.type_id, copy.type_id);
    for (index, value) in c.values.iter().enumerate() {
        let left = field(&c.store, c.source, index as u32, tuple);
        let right = field(&c.store, destination, index as u32, tuple);
        assert_ne!(left, right);
        assert_eq!(
            c.store.read_scalar(left).unwrap(),
            ValueRepr::signed(c.element, *value).unwrap()
        );
        assert_eq!(
            c.store.read_scalar(right).unwrap(),
            c.store.read_scalar(left).unwrap()
        );
    }
    // Mutation of copied storage cannot change the original or its lease.
    let source_field = field(&c.store, c.source, 0, tuple);
    let lease = c
        .store
        .borrow(&Place::cell(source_field), Access::Shared)
        .unwrap();
    let target = field(&c.store, destination, 0, tuple);
    let changed = c
        .store
        .ticket(ValueRepr::signed(c.element, c.values[0] + 1).unwrap())
        .unwrap();
    let old = c.store.replace(target, changed).unwrap();
    c.store.discard(old).unwrap();
    assert_eq!(
        c.store.read_reference(lease).unwrap(),
        ValueRepr::signed(c.element, c.values[0]).unwrap()
    );
    assert_eq!(c.store.ticket(copy).unwrap_err().code, "LRR-E-OWNERSHIP");
    assert_eq!(c.frame.counters.operations_started, 1);
    assert_eq!(c.frame.counters.operations_completed, 1);
    assert_eq!(c.frame.counters.fuel_used, c.values.len() as u64 + 2);
}
#[test]
fn rustc_original_tuple_copy_read_preserves_source_and_allocates_distinct_fields() {
    original_read(true);
}
#[test]
fn rustc_original_fixed_array_copy_read_preserves_source_and_allocates_distinct_fields() {
    original_read(false);
}
#[test]
fn rustc_array_copy_chunk_resumes_without_repeating_opcode_charge_or_completed_fields() {
    let mut c = context(false, true);
    let pc = c.frame.pc.clone();
    let before = c.store.state(c.source).unwrap();
    assert!(matches!(
        run(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Dispatch
    ));
    assert_eq!(c.frame.pc, pc);
    assert_eq!(c.frame.counters.operations_started, 1);
    assert_eq!(c.frame.counters.operations_completed, 0);
    assert_eq!(c.frame.counters.fuel_used, 257);
    assert_eq!(
        c.store.state(c.frame.cell(c.destination).unwrap()).unwrap(),
        CellState::Uninit
    );
    assert!(matches!(
        run(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Next(_)
    ));
    assert_eq!(c.frame.counters.fuel_used, 259);
    assert_eq!(c.frame.counters.operations_completed, 1);
    assert_eq!(c.store.state(c.source).unwrap(), before);
    let destination = c.frame.cell(c.destination).unwrap();
    for index in 0..257 {
        assert_eq!(
            c.store
                .read_scalar(field(&c.store, destination, index, false))
                .unwrap(),
            ValueRepr::signed(c.element, c.values[index as usize]).unwrap()
        );
    }
}
#[test]
fn rustc_array_copy_changed_source_epoch_refuses_resume_without_publishing_partial_result() {
    let mut c = context(false, true);
    assert!(matches!(
        run(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Dispatch
    ));
    let moved = c.store.move_from(c.source).unwrap();
    assert_eq!(c.store.state(c.source).unwrap(), CellState::Moved);
    assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-STALE-HANDLE");
    assert_eq!(c.frame.counters.operations_completed, 0);
    assert_eq!(
        c.store.state(c.frame.cell(c.destination).unwrap()).unwrap(),
        CellState::Uninit
    );
    assert_eq!(
        c.store.peek_ticket(moved).unwrap().type_id,
        c.store.place_type_id(&Place::cell(c.source)).unwrap().0
    );
}

#[test]
fn rustc_copy_nested_runtime_graph_uses_iterative_guards_and_distinct_owning_objects() {
    let facts = facts::aggregate_copy_read(ARRAY, "arr", "arr2");
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut ticket = aggregate(&mut store, &facts.values, TypeId(6), false);
    // Supplemental depth stress around the original array values, independent
    // of the stock source's shallow syntax; no new language program is invented.
    for _ in 0..400 {
        let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
        let parent = store.intern_type(TypeKey::Tuple(vec![ty].into())).unwrap();
        let guard = store.begin_construction(&[ty]).unwrap();
        store.construction_push(guard, 0, ticket).unwrap();
        ticket = store.commit_construction(guard, parent).unwrap();
    }
    let source = root_cell(&mut store, ticket);
    let before = (
        store.state(source).unwrap(),
        store.location_epoch(source).unwrap(),
    );
    let mut copy =
        crate::runtime::cell::aggregate_copy::AggregateCopy::begin(&mut store, source).unwrap();
    let ticket = finish_kernel(&mut store, &mut copy);
    assert_eq!(
        (
            store.state(source).unwrap(),
            store.location_epoch(source).unwrap()
        ),
        before
    );
    let destination = root_cell(&mut store, ticket);
    let (mut left, mut right) = (source, destination);
    for _ in 0..400 {
        assert_ne!(
            store.admit_move(left).unwrap().payload,
            store.admit_move(right).unwrap().payload
        );
        left = field(&store, left, 0, true);
        right = field(&store, right, 0, true);
        assert_ne!(left, right);
    }
    for (index, value) in facts.values.iter().enumerate() {
        assert_eq!(
            store
                .read_scalar(field(&store, right, index as u32, false))
                .unwrap(),
            ValueRepr::signed(TypeId(6), *value).unwrap()
        );
    }
}
#[test]
fn rustc_copy_shared_reference_fields_reuse_leases_without_duplicating_field_cells() {
    let facts = facts::aggregate_copy_read(TUPLE, "p", "p2");
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut references = vec![];
    let mut tickets = vec![];
    // A supplemental closed-reference storage graph uses the original values;
    // it does not claim that the original scalar tuple has reference fields.
    for value in &facts.values {
        let ticket = store
            .ticket(ValueRepr::signed(TypeId(8), *value).unwrap())
            .unwrap();
        let cell = root_cell(&mut store, ticket);
        let lease = store.borrow(&Place::cell(cell), Access::Shared).unwrap();
        references.push(lease);
        tickets.push(store.reference_ticket(lease).unwrap());
    }
    let reference_ty = TypeId(store.peek_ticket(tickets[0]).unwrap().type_id);
    let schema = vec![reference_ty; tickets.len()];
    let ty = store
        .intern_type(TypeKey::Tuple(schema.clone().into()))
        .unwrap();
    let guard = store.begin_construction(&schema).unwrap();
    for (index, ticket) in tickets.into_iter().enumerate() {
        store
            .construction_push(guard, index as u32, ticket)
            .unwrap();
    }
    let ticket = store.commit_construction(guard, ty).unwrap();
    let source = root_cell(&mut store, ticket);
    let mut copy =
        crate::runtime::cell::aggregate_copy::AggregateCopy::begin(&mut store, source).unwrap();
    let ticket = finish_kernel(&mut store, &mut copy);
    let destination = root_cell(&mut store, ticket);
    for (index, lease) in references.iter().enumerate() {
        let left = field(&store, source, index as u32, true);
        let right = field(&store, destination, index as u32, true);
        assert_ne!(left, right);
        assert_eq!(store.reference_from_cell(left).unwrap(), *lease);
        assert_eq!(store.reference_from_cell(right).unwrap(), *lease);
        assert_eq!(
            store.read_reference(*lease).unwrap(),
            ValueRepr::signed(TypeId(8), facts.values[index]).unwrap()
        );
    }
}

#[test]
fn rustc_copy_place_activation_skips_completed_recipe_prefix_and_clears_place_once() {
    let mut c = context(false, true);
    let [
        Op::ReadLocal {
            binding,
            destination,
        },
    ] = c.recipe.recipe().operations.as_ref()
    else {
        unreachable!()
    };
    let (binding, destination) = (*binding, *destination);
    let place = c.frame.body.nodes()[c.frame.pc.node as usize]
        .place
        .unwrap();
    let original = c.recipe.recipe();
    // Supplemental verified address/read form at this same original source PC.
    // This tests activation replay, not a new claimed upstream language pass.
    c.recipe = crate::ir::validate::verify(
        crate::ir::recipe::RecipeDraft {
            key: original.key.clone(),
            origin: original.origin.clone(),
            dependencies: original.dependencies.to_vec(),
            exits: vec![original.exit.clone()],
            operations: vec![
                Op::MakePlace {
                    binding,
                    destination: place,
                },
                Op::ReadPlace { place, destination },
            ],
        },
        &c.frame.body,
    )
    .unwrap();
    assert!(matches!(
        run(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Dispatch
    ));
    assert_eq!(c.frame.resume_operation_index(), 1);
    assert_eq!(c.frame.counters.operations_started, 2);
    assert_eq!(c.frame.counters.operations_completed, 1);
    assert!(c.frame.executed_place(place).is_ok());
    assert!(matches!(
        run(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Next(_)
    ));
    assert_eq!(c.frame.counters.operations_started, 2);
    assert_eq!(c.frame.counters.operations_completed, 2);
    assert_eq!(c.frame.counters.fuel_used, 260);
    assert_eq!(c.frame.counters.exits_completed, 1);
    assert!(c.frame.executed_place(place).is_err());
    c.store.admit_move(c.source).unwrap();
    c.store
        .admit_move(c.frame.cell(destination).unwrap())
        .unwrap();
}
#[test]
fn rustc_copy_resume_refuses_changed_bound_pc_without_charging_or_publishing() {
    let mut c = context(false, true);
    assert!(matches!(
        run(&mut c).unwrap(),
        crate::vm::step::StepOutcome::Dispatch
    ));
    let used = c.frame.fuel_snapshot().used;
    c.frame.pc.phase = Phase::Commit.code().unwrap();
    assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-INTERNAL-IR");
    assert_eq!(c.frame.fuel_snapshot().used, used);
    assert_eq!(c.frame.counters.operations_completed, 0);
    assert_eq!(
        c.store.state(c.frame.cell(c.destination).unwrap()).unwrap(),
        CellState::Uninit
    );
}
