//! dynamic-drop.rs supplies the original `ar = [a.alloc(), a.alloc()]`
//! commit recipe and declaration identities. Prepared intrinsic String arrays
//! replace allocator/Drop values; the source partial patterns and callbacks stay
//! cold. The 257-element expansion is disclosed stress coverage only.
use super::*;
use crate::{
    core::digest::Digest,
    ir::{op::Op, pc::Phase, recipe::VerifiedRecipe},
    runtime::shape::TypeKey,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{parse::ParsedFile, scope_fixture_facts},
    vm::step::{StepOutcome, step},
};
const ORIGINAL: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/dynamic-drop.rs");
struct Context {
    frame: Frame,
    store: GuestStore,
    recipe: VerifiedRecipe,
    binding: NodeId,
    source: SlotId,
    destination: SlotId,
    old: CellId,
    fields: Vec<CellId>,
    incoming: ValueRepr,
    cleanup_before: u64,
    moved_field: usize,
}
fn enter_to(frame: &mut Frame, scope: ScopeId) {
    let mut ancestry = vec![];
    let mut cursor = Some(scope);
    while let Some(id) = cursor {
        ancestry.push(id);
        cursor = frame.body.scopes()[id.0 as usize].parent;
    }
    for id in ancestry.into_iter().rev() {
        if !frame.active_scopes.contains(&id) {
            frame.enter(id).unwrap();
        }
    }
}
fn array(store: &mut GuestStore, count: u32, prefix: &str) -> (TicketId, Vec<CellId>) {
    let first = store.string(prefix).unwrap();
    let element = TypeId(store.peek_ticket(first).unwrap().type_id);
    let ty = store
        .intern_type(TypeKey::Array {
            element,
            len: count,
        })
        .unwrap();
    let guard = store
        .begin_construction(&vec![element; count as usize])
        .unwrap();
    store.construction_push(guard, 0, first).unwrap();
    for ordinal in 1..count {
        let field = store.string(&format!("{prefix}:{ordinal}")).unwrap();
        store.construction_push(guard, ordinal, field).unwrap();
    }
    let ticket = store.commit_construction(guard, ty).unwrap();
    let fields = (0..count)
        .map(|index| store.aggregate_field(ticket, index).unwrap())
        .collect();
    (ticket, fields)
}
fn context(expanded: bool, handles: u32) -> Context {
    let facts = scope_fixture_facts::original_array_reassignment(ORIGINAL);
    let (binding_range, init_range, assignment, rhs, stock_count) = (
        facts.binding,
        facts.initializer,
        facts.assignment,
        facts.rhs,
        facts.count,
    );
    let parsed = // Explicit fixture preparation of the original's owning2021 source;
    // this component is not a live2021 root or a whole-program observation.
    ParsedFile::parse_dependency(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        scope_fixture_facts::original_array_reassignment_profile(ORIGINAL),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-bounded-local-replacement"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "slice_pattern_reassign")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let body = parsed.index_body(handle, Default::default()).unwrap();
    let binding = body
        .nodes()
        .iter()
        .find(|node| node.range == binding_range)
        .unwrap()
        .id;
    let initial = body
        .nodes()
        .iter()
        .find(|node| node.range == init_range)
        .unwrap()
        .value
        .unwrap();
    let assigned = body
        .nodes()
        .iter()
        .find(|node| node.range == assignment)
        .unwrap()
        .id;
    let source = body
        .nodes()
        .iter()
        .find(|node| node.range == rhs)
        .unwrap()
        .value
        .unwrap();
    // RHS is prepared, so enter its actual Operand(1) return continuation.
    // Assignment Commit is not an ordinary `=` executable phase.
    let pc = body.pc(assigned, Phase::Operand(1)).unwrap();
    let recipe = parsed.initial_thunk(handle, &body, &pc).unwrap();
    let [
        Op::AssignLocal {
            binding: selected,
            source: selected_source,
            destination,
        },
    ] = recipe.recipe().operations.as_ref()
    else {
        panic!("original local assignment recipe differs")
    };
    assert_eq!(*selected, binding);
    assert_eq!(*selected_source, source);
    let destination = *destination;
    let mut store = GuestStore::new(1, handles, 8_000_000).unwrap();
    let mut frame = Frame::new(body, pc, &mut store).unwrap();
    frame.set_fuel_limit(10000).unwrap();
    let count = if expanded { 257 } else { stock_count };
    assert!(count > 0);
    let initial_scope = frame_slot_scope(&frame, initial);
    enter_to(&mut frame, initial_scope);
    let (old_ticket, fields) = array(&mut store, count, "old");
    let ty = TypeId(store.peek_ticket(old_ticket).unwrap().type_id);
    frame.write(initial, old_ticket, &mut store).unwrap();
    frame
        .bind(binding, initial, Some(ty), true, &mut store)
        .unwrap();
    while frame.active_scopes.last() != Some(&ScopeId(0)) {
        frame
            .leave(*frame.active_scopes.last().unwrap(), &mut store)
            .unwrap();
    }
    let old = frame.binding(binding).unwrap();
    let source_scope = frame_slot_scope(&frame, source);
    enter_to(&mut frame, source_scope);
    let (ticket, _) = array(&mut store, count, "incoming");
    let incoming = store.peek_ticket(ticket).unwrap();
    frame.write(source, ticket, &mut store).unwrap();
    let cleanup_before = frame.counters.cleanup_elements_completed;
    Context {
        frame,
        store,
        recipe,
        binding,
        source,
        destination,
        old,
        fields,
        incoming,
        cleanup_before,
        moved_field: usize::try_from(facts.moved_field).unwrap(),
    }
}
fn frame_slot_scope(frame: &Frame, slot: SlotId) -> ScopeId {
    frame.slot(slot).unwrap().scope
}
fn run(c: &mut Context) -> Result<StepOutcome, Diagnostic> {
    step(&mut c.frame, &mut c.store, &c.recipe)
}
#[test]
fn rustc_original_local_array_replacement_commits_rhs_before_old_cleanup() {
    for expanded in [false, true] {
        let mut c = context(expanded, 10000);
        let pc = c.frame.pc.clone();
        let epoch = c.store.location_epoch(c.old).unwrap();
        let mut cleanup_root = None;
        let mut complete = false;
        for _ in 0..64 {
            match run(&mut c).unwrap() {
                StepOutcome::Next(_) => {
                    complete = true;
                    break;
                }
                StepOutcome::Dispatch => {
                    c.frame
                        .check_cleanup_activation(0, &c.recipe.recipe().operations[0])
                        .unwrap();
                    assert_eq!(c.frame.pc, pc);
                    let (target, old, unit) = c.frame.replacement_frontier_for_test();
                    assert_eq!(target, c.old);
                    assert_ne!(old, target);
                    assert_eq!(c.frame.binding(c.binding).unwrap(), target);
                    assert_eq!(c.store.state(target).unwrap(), CellState::Live(c.incoming));
                    assert_eq!(c.store.location_epoch(target).unwrap(), epoch);
                    assert_eq!(
                        c.store.state(c.frame.cell(c.source).unwrap()).unwrap(),
                        CellState::Moved
                    );
                    match c.store.state(old).unwrap() {
                        CellState::Dropping => {}
                        CellState::Dropped => {
                            assert!(c.fields.iter().all(|field| {
                                c.store.state(*field).unwrap() == CellState::Dropped
                            }))
                        }
                        state => panic!("replacement lost its detached old obligation: {state:?}"),
                    }
                    cleanup_root = Some(old);
                    assert_eq!(c.store.peek_ticket(unit).unwrap(), ValueRepr::unit());
                    assert_eq!(c.frame.counters.started_by_opcode[16], 1);
                    assert_eq!(c.frame.counters.operations_completed, 0);
                }
                other => panic!("original assignment entered unexpected outcome: {other:?}"),
            }
        }
        assert!(
            complete,
            "replacement did not complete within bounded chunks"
        );
        if expanded {
            assert!(cleanup_root.is_some(), "stress must exercise suspension");
        }
        if let Some(old) = cleanup_root {
            assert_eq!(c.store.state(old).unwrap(), CellState::Dropped);
            assert_eq!(
                c.store.location_epoch(old).unwrap_err().code,
                "LRR-E-STALE-HANDLE"
            );
        }
        assert!(
            c.fields
                .iter()
                .all(|field| c.store.state(*field).unwrap() == CellState::Dropped)
        );
        for field in &c.fields {
            assert_eq!(
                c.store.location_epoch(*field).unwrap_err().code,
                "LRR-E-STALE-HANDLE"
            );
        }
        assert_eq!(c.frame.binding(c.binding).unwrap(), c.old);
        assert_eq!(c.store.state(c.old).unwrap(), CellState::Live(c.incoming));
        assert_eq!(c.store.location_epoch(c.old).unwrap(), epoch);
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
        assert_eq!(c.frame.counters.started_by_opcode[16], 1);
        assert_eq!(c.frame.counters.operations_completed, 1);
        assert_eq!(
            c.frame.counters.cleanup_elements_completed - c.cleanup_before,
            u64::try_from(c.fields.len()).unwrap() + 1
        );
        assert!(!c.frame.has_cleanup_activation());
    }
}
fn partial(c: &mut Context) -> (CellId, TicketId) {
    for _ in 0..64 {
        assert!(matches!(run(c).unwrap(), StepOutcome::Dispatch));
        let (target, old, unit) = c.frame.replacement_frontier_for_test();
        assert_eq!(target, c.old);
        assert_eq!(c.frame.binding(c.binding).unwrap(), target);
        assert_eq!(c.store.state(target).unwrap(), CellState::Live(c.incoming));
        assert_eq!(c.store.state(old).unwrap(), CellState::Dropping);
        assert_eq!(c.store.peek_ticket(unit).unwrap(), ValueRepr::unit());
        assert_eq!(
            c.store.state(c.frame.cell(c.destination).unwrap()).unwrap(),
            CellState::Uninit
        );
        if c.fields
            .iter()
            .any(|field| c.store.state(*field).unwrap() == CellState::Dropped)
            && c.fields
                .iter()
                .any(|field| matches!(c.store.state(*field).unwrap(), CellState::Live(_)))
        {
            return (old, unit);
        }
    }
    panic!("bounded replacement did not expose partial cleanup")
}
#[test]
fn rustc_local_array_replacement_cancellation_keeps_rhs_and_old_continuation() {
    let mut c = context(true, 10000);
    let epoch = c.store.location_epoch(c.old).unwrap();
    let (old, unit) = partial(&mut c);
    let pc = c.frame.pc.clone();
    let fuel = c.frame.fuel_snapshot().used;
    let states: Vec<_> = c
        .fields
        .iter()
        .map(|field| c.store.state(*field).unwrap())
        .collect();
    let handles = c.store.budget().handles();
    c.frame.fuel.cancellation().cancel();
    assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(c.frame.status, FrameStatus::Faulted);
    assert_eq!(c.frame.pc, pc);
    assert_eq!(c.frame.fuel_snapshot().used, fuel);
    assert_eq!(c.store.budget().handles(), handles);
    assert_eq!(
        c.fields
            .iter()
            .map(|field| c.store.state(*field).unwrap())
            .collect::<Vec<_>>(),
        states
    );
    assert_eq!(c.frame.binding(c.binding).unwrap(), c.old);
    assert_eq!(c.store.state(c.old).unwrap(), CellState::Live(c.incoming));
    assert_eq!(c.store.location_epoch(c.old).unwrap(), epoch);
    assert_eq!(c.store.state(old).unwrap(), CellState::Dropping);
    assert_eq!(c.store.peek_ticket(unit).unwrap(), ValueRepr::unit());
    assert_eq!(c.frame.counters.started_by_opcode[16], 1);
    assert_eq!(c.frame.counters.operations_completed, 0);
    assert!(c.frame.has_cleanup_activation());
}
#[test]
fn rustc_local_array_replacement_batch_refusal_leaves_both_owners_live() {
    let probe = context(false, 10000);
    let used = probe.store.budget().handles();
    // Two cells and four tickets (incoming/old/unit/drop) are required. One
    // missing monotonic handle refuses before ownership or location changes.
    let mut c = context(false, used.checked_add(5).unwrap());
    assert_eq!(c.store.budget().handles(), used);
    let fuel = c.frame.fuel_snapshot().used;
    assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
    assert!(matches!(c.store.state(c.old).unwrap(), CellState::Live(_)));
    assert_eq!(
        c.store.state(c.frame.cell(c.source).unwrap()).unwrap(),
        CellState::Live(c.incoming)
    );
    assert!(!c.frame.has_cleanup_activation());
    assert!(c.frame.cell(c.destination).is_err());
    assert_eq!(c.store.budget().handles(), used);
    assert_eq!(c.frame.fuel_snapshot().used, fuel.checked_add(1).unwrap());
    assert_eq!(c.frame.counters.started_by_opcode[16], 1);
    assert_eq!(c.frame.counters.operations_completed, 0);
}

#[test]
fn rustc_local_array_replacement_skips_moved_old_field_without_consuming_extracted_owner() {
    let mut c = context(false, 10000);
    let moved = c.store.move_from(c.fields[c.moved_field]).unwrap();
    let extracted = c.store.string_contents(moved).unwrap().to_owned();
    assert_eq!(
        c.store.state(c.fields[c.moved_field]).unwrap(),
        CellState::Moved
    );
    // The ordinary whole-value move must still reject a partial aggregate.
    assert_eq!(
        c.store.admit_move(c.old).unwrap_err().code,
        "LRR-E-USE-AFTER-MOVE"
    );
    let epoch = c.store.location_epoch(c.old).unwrap();
    let mut complete = false;
    for _ in 0..64 {
        match run(&mut c).unwrap() {
            StepOutcome::Next(_) => {
                complete = true;
                break;
            }
            StepOutcome::Dispatch => {
                assert_eq!(c.store.state(c.old).unwrap(), CellState::Live(c.incoming));
                assert_eq!(c.store.string_contents(moved).unwrap(), extracted);
            }
            other => panic!("partial old replacement entered unexpected outcome: {other:?}"),
        }
    }
    assert!(
        complete,
        "partial old cleanup did not complete within bounded chunks"
    );
    assert_eq!(c.frame.binding(c.binding).unwrap(), c.old);
    assert_eq!(c.store.state(c.old).unwrap(), CellState::Live(c.incoming));
    assert_eq!(c.store.location_epoch(c.old).unwrap(), epoch);
    assert!(
        c.fields
            .iter()
            .all(|field| c.store.state(*field).unwrap() == CellState::Dropped)
    );
    assert_eq!(c.store.string_contents(moved).unwrap(), extracted);
    assert_eq!(c.frame.counters.started_by_opcode[16], 1);
    assert_eq!(c.frame.counters.operations_completed, 1);
    assert!(!c.frame.has_cleanup_activation());
}
