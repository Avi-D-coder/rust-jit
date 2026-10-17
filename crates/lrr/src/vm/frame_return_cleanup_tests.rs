//! Original nested return recipe, with a disclosed prepared intrinsic frontier.
//! Result/Bar/Foo/partial guard/user Drop and whole fixture remain unexecuted.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{op::Op, pc::Phase},
    runtime::shape::TypeKey,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        parse::ParsedFile,
        scope_fixture_facts,
    },
    vm::step::{StepOutcome, step},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/nested-return-drop-order.rs"
);
struct Context {
    frame: Frame,
    store: GuestStore,
    ticket: TicketId,
    root: CellId,
    fields: Vec<CellId>,
    owner: SlotId,
}
fn returned_context() -> Context {
    returned_context_with_guard(false).0
}
fn returned_context_with_guard(partial: bool) -> (Context, Option<SlotId>) {
    let facts = scope_fixture_facts::original_nested_return(ORIGINAL);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-nested-return-cleanup"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "foo")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let body = parsed.index_body(handle, Default::default()).unwrap();
    let returned = body
        .nodes()
        .iter()
        .find(|node| {
            node.range == facts.returned
                && node.role == NodeRole::Expression(ExpressionKind::Return)
        })
        .unwrap();
    let prepared = body
        .nodes()
        .iter()
        .find(|node| node.range == facts.prepared_owner)
        .unwrap();
    let owner = prepared.value.unwrap();
    let scope = returned.scope;
    let pc = body.pc(returned.id, Phase::Operand(0)).unwrap();
    let recipe = parsed.initial_thunk(handle, &body, &pc).unwrap();
    let [Op::ResultTransfer { source, .. }] = recipe.recipe().operations.as_ref() else {
        panic!("stock nested return transfer absent")
    };
    let source = *source;
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(body.clone(), pc, &mut store).unwrap();
    let mut ancestry = Vec::new();
    let mut current = Some(scope);
    while let Some(scope) = current {
        ancestry.push(scope);
        current = body.scopes()[scope.0 as usize].parent;
    }
    for scope in ancestry.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    // The completed Err call is adapted to owning String tagged by its written
    // argument. Bar's completed field becomes a chunk-sized intrinsic array.
    // No construction guard or enclosing nominal value is claimed to execute.
    let ticket = store.string(&facts.returned_tag).unwrap();
    frame.write(source, ticket, &mut store).unwrap();
    let count = 257;
    let element = TypeId(8);
    let ty = store
        .intern_type(TypeKey::Array {
            element,
            len: count,
        })
        .unwrap();
    let guard = store
        .begin_construction(&vec![element; count as usize])
        .unwrap();
    for field in 0..count {
        let ticket = store
            .ticket(ValueRepr::signed(element, facts.field_tag).unwrap())
            .unwrap();
        store.construction_push(guard, field, ticket).unwrap();
    }
    let ticket = store.commit_construction(guard, ty).unwrap();
    let fields = (0..count)
        .map(|field| store.aggregate_field(ticket, field).unwrap())
        .collect();
    frame.write(owner, ticket, &mut store).unwrap();
    let root = frame.cell(owner).unwrap();
    let partial_guard = if partial {
        let (members, completed) =
            scope_fixture_facts::original_nested_constructor_fields(ORIGINAL);
        let guards: Vec<_> = body
            .nodes()
            .iter()
            .filter(|node| {
                node.constructor_guard.is_some()
                    && node.range.start < facts.prepared_owner.start
                    && node.range.end > facts.returned.end
            })
            .collect();
        let [constructor] = guards.as_slice() else {
            panic!("original enclosing Foo constructor is not unique")
        };
        let guard = constructor.constructor_guard.unwrap();
        let mut types = vec![TypeId(8); members.len()];
        types[completed as usize] = ty;
        let enclosing = store.intern_type(TypeKey::Tuple(types.into())).unwrap();
        // Original Foo field ordinal/guard and return recipe; adapt the nominal
        // layout to intrinsic tuple storage. Bar/User Drop remains unexecuted.
        frame
            .begin_structural_constructor(guard, enclosing, &mut store)
            .unwrap();
        frame
            .constructor_field(guard, completed, owner, &mut store)
            .unwrap();
        assert_eq!(store.state(root).unwrap(), CellState::Moved);
        Some(guard)
    } else {
        None
    };
    let StepOutcome::Return(ticket) = step(&mut frame, &mut store, &recipe).unwrap() else {
        panic!("original nested return did not transfer its ticket")
    };
    assert_eq!(frame.status, FrameStatus::Returned);
    (
        Context {
            frame,
            store,
            ticket,
            root,
            fields,
            owner,
        },
        partial_guard,
    )
}
#[test]
fn rustc_nested_return_cleanup_retains_returned_ticket_until_all_scopes_finish() {
    let mut c = returned_context();
    let value = c.store.peek_ticket(c.ticket).unwrap();
    let pc = c.frame.pc.clone();
    let before = (
        c.frame.counters.operations_started,
        c.frame.counters.operations_completed,
        c.frame.counters.exits_completed,
    );
    assert!(!c.frame.advance_return_cleanup(&mut c.store).unwrap());
    assert_eq!(c.frame.status, FrameStatus::Returned);
    assert_eq!(c.frame.pc, pc);
    assert_eq!(c.frame.cell(c.owner).unwrap(), c.root);
    assert_eq!(c.store.peek_ticket(c.ticket).unwrap(), value);
    let mut complete = false;
    for _ in 0..32 {
        if c.frame.advance_return_cleanup(&mut c.store).unwrap() {
            complete = true;
            break;
        }
        assert_eq!(c.store.peek_ticket(c.ticket).unwrap(), value);
    }
    assert!(complete);
    assert!(c.frame.active_scopes.is_empty());
    assert!(c.frame.counters.return_scopes_cleaned > 0);
    assert_eq!(
        (
            c.frame.counters.operations_started,
            c.frame.counters.operations_completed,
            c.frame.counters.exits_completed
        ),
        before
    );
    assert_eq!(c.store.peek_ticket(c.ticket).unwrap(), value);
    assert_eq!(c.store.state(c.root).unwrap(), CellState::Dropped);
    assert!(
        c.fields
            .iter()
            .all(|field| c.store.state(*field).unwrap() == CellState::Dropped)
    );
    let used = c.frame.fuel_snapshot().used;
    assert!(c.frame.advance_return_cleanup(&mut c.store).unwrap());
    assert_eq!(c.frame.fuel_snapshot().used, used);
    c.store.discard(c.ticket).unwrap();
    assert_eq!(
        c.store.peek_ticket(c.ticket).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
}
fn begin_guard_cleanup_chunk(c: &mut Context, guard: SlotId) {
    for _ in 0..64 {
        assert!(!c.frame.advance_return_cleanup(&mut c.store).unwrap());
        if c.frame.slots[guard.0 as usize]
            .construction
            .as_ref()
            .unwrap()
            .cleaning
        {
            return;
        }
    }
    panic!("bounded cleanup did not reach original partial constructor");
}
#[test]
fn rustc_nested_return_drains_completed_guard_field_before_retiring_enclosing_scope() {
    let (mut c, guard) = returned_context_with_guard(true);
    let guard = guard.unwrap();
    let construction = c.frame.slots[guard.0 as usize]
        .construction
        .as_ref()
        .unwrap()
        .guard;
    let value = c.store.peek_ticket(c.ticket).unwrap();
    let pc = c.frame.pc.clone();
    let counters = (
        c.frame.counters.operations_started,
        c.frame.counters.operations_completed,
        c.frame.counters.exits_completed,
    );
    begin_guard_cleanup_chunk(&mut c, guard);
    assert!(
        c.frame.slots[guard.0 as usize]
            .construction
            .as_ref()
            .unwrap()
            .cleaning
    );
    assert_eq!(
        c.store
            .construction_cleanup_next(construction)
            .unwrap_err()
            .code,
        "LRR-E-OWNERSHIP"
    );
    assert_eq!(c.frame.pc, pc);
    assert_eq!(c.store.peek_ticket(c.ticket).unwrap(), value);
    assert!(
        c.fields
            .iter()
            .any(|field| matches!(c.store.state(*field).unwrap(), CellState::Live(_)))
    );
    let mut done = false;
    for _ in 0..64 {
        if c.frame.advance_return_cleanup(&mut c.store).unwrap() {
            done = true;
            break;
        }
        assert_eq!(c.store.peek_ticket(c.ticket).unwrap(), value);
    }
    assert!(done);
    assert!(c.frame.slots[guard.0 as usize].construction.is_none());
    assert_eq!(
        c.store
            .construction_cleanup_next(construction)
            .unwrap_err()
            .code,
        "LRR-E-STALE-HANDLE"
    );
    assert!(c.frame.active_scopes.is_empty());
    assert!(
        c.fields
            .iter()
            .all(|field| c.store.state(*field).unwrap() == CellState::Dropped)
    );
    assert_eq!(
        (
            c.frame.counters.operations_started,
            c.frame.counters.operations_completed,
            c.frame.counters.exits_completed
        ),
        counters
    );
    assert_eq!(c.store.peek_ticket(c.ticket).unwrap(), value);
    c.store.discard(c.ticket).unwrap();
}
