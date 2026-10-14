//! Original unused-result.rs supplies `((), ());` and its complete commit
//! recipe. The exact pair is a prepared constructor frontier. Chunk controls
//! disclose an intrinsic 257-unit array in the first field; upstream diagnostics,
//! calls and full compiletest behavior are not executed by these controls.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{op::Op, pc::Phase, recipe::VerifiedRecipe},
    runtime::shape::TypeKey,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{body_index::NodeRole, parse::ParsedFile, scope_fixture_facts},
    vm::step::{StepOutcome, step},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lint/unused/unused-result.rs"
);
struct Context {
    frame: Frame,
    store: GuestStore,
    recipe: VerifiedRecipe,
    root: CellId,
    source: SlotId,
    fields: Vec<CellId>,
    scope: ScopeId,
}
fn context(expanded: bool) -> Context {
    let (statement, expression) = scope_fixture_facts::original_unit_pair_statement(ORIGINAL);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-discard-unit-pair"),
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
    let node = body
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Statement && node.range == statement)
        .unwrap();
    let owner = body
        .nodes()
        .iter()
        .find(|node| node.range == expression)
        .unwrap()
        .value
        .unwrap();
    let pc = body.pc(node.id, Phase::Commit).unwrap();
    let recipe = parsed.initial_thunk(handle, &body, &pc).unwrap();
    let [Op::Discard { source }, Op::ScopeLeave { scope }] = recipe.recipe().operations.as_ref()
    else {
        panic!("original pair statement lacks exact discard/leave recipe")
    };
    let (source, scope) = (*source, *scope);
    assert_eq!(source, owner);
    let mut store = GuestStore::new(1, 10000, 4_000_000).unwrap();
    let mut frame = Frame::new(body.clone(), pc, &mut store).unwrap();
    frame.set_fuel_limit(10000).unwrap();
    let mut ancestry = vec![];
    let mut cursor = Some(scope);
    while let Some(id) = cursor {
        ancestry.push(id);
        cursor = body.scopes()[id.0 as usize].parent;
    }
    for id in ancestry.into_iter().rev() {
        frame.enter(id).unwrap();
    }
    let mut fields = vec![];
    let first = if expanded {
        // Disclosed intrinsic stress adaptation only. No original type view is
        // manufactured and the original recipe/opcode identity is retained.
        let len = 257;
        let ty = store
            .intern_type(TypeKey::Array {
                element: TypeId(1),
                len,
            })
            .unwrap();
        let guard = store
            .begin_construction(&vec![TypeId(1); len as usize])
            .unwrap();
        for ordinal in 0..len {
            let unit = store.ticket(ValueRepr::unit()).unwrap();
            store.construction_push(guard, ordinal, unit).unwrap();
        }
        let ticket = store.commit_construction(guard, ty).unwrap();
        fields.extend((0..len).map(|ordinal| store.aggregate_field(ticket, ordinal).unwrap()));
        ticket
    } else {
        store.ticket(ValueRepr::unit()).unwrap()
    };
    let first_ty = TypeId(store.peek_ticket(first).unwrap().type_id);
    let ty = store
        .intern_type(TypeKey::Tuple(vec![first_ty, TypeId(1)].into()))
        .unwrap();
    let guard = store.begin_construction(&[first_ty, TypeId(1)]).unwrap();
    store.construction_push(guard, 0, first).unwrap();
    let unit = store.ticket(ValueRepr::unit()).unwrap();
    store.construction_push(guard, 1, unit).unwrap();
    let ticket = store.commit_construction(guard, ty).unwrap();
    fields.extend((0..2).map(|ordinal| store.aggregate_field(ticket, ordinal).unwrap()));
    frame.write(source, ticket, &mut store).unwrap();
    let root = frame.cell(source).unwrap();
    Context {
        frame,
        store,
        recipe,
        root,
        source,
        fields,
        scope,
    }
}
fn run(c: &mut Context) -> Result<StepOutcome, Diagnostic> {
    step(&mut c.frame, &mut c.store, &c.recipe)
}
#[test]
fn rustc_original_unit_pair_statement_discards_each_owned_field_once() {
    let mut c = context(false);
    let pc = c.frame.pc.clone();
    let mut complete = false;
    for _ in 0..64 {
        match run(&mut c).unwrap() {
            StepOutcome::Next(_) => {
                complete = true;
                break;
            }
            StepOutcome::Dispatch => {
                // The tiny pair is already fully cleaned. The following
                // original ScopeLeave scans a large body index in bounded
                // chunks; this cannot repeat Discard or enter the next source.
                assert_eq!(c.frame.pc, pc);
                c.frame
                    .check_cleanup_activation(1, &Op::ScopeLeave { scope: c.scope })
                    .unwrap();
                assert_eq!(c.store.state(c.root).unwrap(), CellState::Dropped);
                assert!(
                    c.fields
                        .iter()
                        .all(|field| c.store.state(*field).unwrap() == CellState::Dropped)
                );
                assert!(c.frame.cell(c.source).is_err());
                assert_eq!(c.frame.counters.operations_started, 2);
                assert_eq!(c.frame.counters.operations_completed, 1);
                assert_eq!(c.frame.counters.cleanup_elements_completed, 3);
                assert_eq!(c.frame.counters.started_by_opcode[9], 1);
                assert_eq!(c.frame.counters.started_by_opcode[5], 1);
                assert_eq!(c.frame.active_scopes.last(), Some(&c.scope));
            }
            other => panic!("original tuple statement entered unexpected outcome: {other:?}"),
        }
    }
    assert!(
        complete,
        "original scope scan never completed within its bounded resumption count"
    );
    assert_eq!(c.store.state(c.root).unwrap(), CellState::Dropped);
    assert!(
        c.fields
            .iter()
            .all(|field| c.store.state(*field).unwrap() == CellState::Dropped)
    );
    assert!(c.frame.cell(c.source).is_err());
    assert!(!c.frame.active_scopes.contains(&c.scope));
    assert!(!c.frame.has_cleanup_activation());
    assert_eq!(c.frame.counters.started_by_opcode[9], 1);
    assert_eq!(c.frame.counters.started_by_opcode[5], 1);
    assert_eq!(c.frame.counters.operations_started, 2);
    assert_eq!(c.frame.counters.operations_completed, 2);
    assert_eq!(c.frame.counters.cleanup_elements_completed, 3);
}
fn reach_partial(c: &mut Context) {
    let pc = c.frame.pc.clone();
    for _ in 0..64 {
        assert!(matches!(run(c).unwrap(), StepOutcome::Dispatch));
        c.frame
            .check_cleanup_activation(0, &Op::Discard { source: c.source })
            .unwrap();
        assert_eq!(c.frame.pc, pc);
        assert_eq!(c.frame.counters.started_by_opcode[9], 1);
        assert_eq!(c.frame.counters.started_by_opcode[5], 0);
        assert_eq!(c.frame.counters.operations_completed, 0);
        assert_eq!(c.frame.active_scopes.last(), Some(&c.scope));
        if c.fields
            .iter()
            .any(|field| c.store.state(*field).unwrap() == CellState::Dropped)
            && c.fields
                .iter()
                .any(|field| matches!(c.store.state(*field).unwrap(), CellState::Live(_)))
        {
            assert_eq!(c.frame.cell(c.source).unwrap(), c.root);
            assert_eq!(c.store.state(c.root).unwrap(), CellState::Dropping);
            return;
        }
    }
    panic!("bounded discard never exposed partially completed owner cleanup")
}
#[test]
fn rustc_unit_pair_discard_dispatch_keeps_scope_and_owner_until_completion() {
    let mut c = context(true);
    let owners = u64::try_from(c.fields.len())
        .unwrap()
        .checked_add(1)
        .unwrap();
    reach_partial(&mut c);
    let mut complete = false;
    for _ in 0..64 {
        match run(&mut c).unwrap() {
            StepOutcome::Dispatch => {
                assert_eq!(c.frame.counters.started_by_opcode[9], 1);
            }
            StepOutcome::Next(_) => {
                complete = true;
                break;
            }
            other => panic!("discard resumed into unexpected outcome: {other:?}"),
        }
    }
    assert!(complete);
    assert_eq!(c.store.state(c.root).unwrap(), CellState::Dropped);
    assert!(
        c.fields
            .iter()
            .all(|field| c.store.state(*field).unwrap() == CellState::Dropped)
    );
    assert_eq!(c.frame.counters.cleanup_elements_completed, owners);
    assert_eq!(c.frame.counters.started_by_opcode[9], 1);
    assert_eq!(c.frame.counters.started_by_opcode[5], 1);
    assert_eq!(c.frame.counters.operations_completed, 2);
    assert!(!c.frame.has_cleanup_activation());
    assert!(!c.frame.active_scopes.contains(&c.scope));
}
#[test]
fn rustc_unit_pair_discard_cancellation_preserves_pending_owner_and_live_fields() {
    let mut c = context(true);
    reach_partial(&mut c);
    let states: Vec<_> = c
        .fields
        .iter()
        .map(|field| c.store.state(*field).unwrap())
        .collect();
    let before = (
        c.frame.pc.clone(),
        c.frame.counters.operations_started,
        c.frame.counters.operations_completed,
        c.frame.counters.cleanup_elements_completed,
        c.store.budget().handles(),
        c.frame.fuel_snapshot().used,
    );
    c.frame.fuel.cancellation().cancel();
    assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(c.frame.status, FrameStatus::Faulted);
    assert_eq!(
        (
            c.frame.pc.clone(),
            c.frame.counters.operations_started,
            c.frame.counters.operations_completed,
            c.frame.counters.cleanup_elements_completed,
            c.store.budget().handles(),
            c.frame.fuel_snapshot().used
        ),
        before
    );
    assert_eq!(
        c.fields
            .iter()
            .map(|field| c.store.state(*field).unwrap())
            .collect::<Vec<_>>(),
        states
    );
    assert!(c.frame.has_cleanup_activation());
    assert_eq!(c.frame.cell(c.source).unwrap(), c.root);
    assert_eq!(c.store.state(c.root).unwrap(), CellState::Dropping);
    assert_eq!(c.frame.active_scopes.last(), Some(&c.scope));
    // Fatal cancellation never counts as guest panic or a successful fixture.
}
