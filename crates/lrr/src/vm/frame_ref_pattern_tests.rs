//! Executes the original two initialized ref patterns, not the stock error body.
use super::*;
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    ir::{op::Op, pc::Phase, recipe::VerifiedRecipe},
    runtime::{
        place::{Place, PlaceFailure},
        reference::Access,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{outline::BodyHandle, parse::ParsedFile, scope_fixture_facts},
    vm::step::{StepOutcome, step},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/cannot-assign-borrowed-ref-in-slice.rs"
);
const PLACE_ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/let-destruct-ref.rs"
);
struct Context {
    parsed: ParsedFile,
    handle: BodyHandle,
    frame: Frame,
    store: GuestStore,
    statement: NodeId,
    binding: NodeId,
    initializer: NodeId,
    owner_binding: Option<NodeId>,
}
fn context(mutable: bool, fuel: u64) -> Context {
    context_source(ORIGINAL, mutable, fuel)
}
fn context_source(original: &str, mutable: bool, fuel: u64) -> Context {
    let facts = scope_fixture_facts::original_reference_patterns(original);
    let facts = facts.iter().find(|fact| fact.mutable == mutable).unwrap();
    let parsed = ParsedFile::parse(
        &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-initialized-ref-pattern"),
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
        .clone()
        .unwrap();
    let body = parsed.index_body(&handle, Default::default()).unwrap();
    let id = |range| {
        body.nodes()
            .iter()
            .find(|node| node.range == range)
            .unwrap()
            .id
    };
    let (statement, binding, initializer) = (
        id(facts.statement),
        id(facts.binding),
        id(facts.initializer),
    );
    let owner_binding = facts.owner_binding.map(id);
    let entry = body
        .pc(facts.owner_statement.map_or(statement, id), Phase::Entry)
        .unwrap();
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let mut frame = Frame::new(body.clone(), entry, &mut store).unwrap();
    frame.set_fuel_limit(fuel).unwrap();
    let mut scopes = vec![];
    let mut scope = Some(body.nodes()[statement.0 as usize].scope);
    while let Some(id) = scope {
        scopes.push(id);
        scope = body.scopes()[id.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    Context {
        parsed,
        handle,
        frame,
        store,
        statement,
        binding,
        initializer,
        owner_binding,
    }
}
fn recipe(c: &Context) -> VerifiedRecipe {
    c.parsed
        .initial_thunk(&c.handle, &c.frame.body, c.frame.pc())
        .unwrap()
}
fn run(c: &mut Context, r: &VerifiedRecipe) -> Result<StepOutcome, Diagnostic> {
    step(&mut c.frame, &mut c.store, r)
}
fn reach_commit(c: &mut Context) -> VerifiedRecipe {
    for _ in 0..16 {
        let r = recipe(c);
        if c.frame.pc() == &c.frame.body.pc(c.statement, Phase::Commit).unwrap() {
            let prefix = usize::from(c.owner_binding.is_none());
            if prefix == 1 {
                assert!(matches!(
                    r.recipe().operations.first(),
                    Some(Op::ValuePlace { .. })
                ));
            }
            assert!(matches!(
                r.recipe().operations.get(prefix),
                Some(Op::Borrow { .. })
            ));
            assert!(matches!(
                r.recipe().operations.get(prefix + 1),
                Some(Op::BindLocal {
                    writable: false,
                    ..
                })
            ));
            return r;
        }
        assert!(matches!(run(c, &r).unwrap(), StepOutcome::Next(_)));
    }
    panic!("original reference pattern did not reach commit")
}
fn target(c: &Context) -> CellId {
    if let Some(binding) = c.owner_binding {
        return c.frame.binding(binding).unwrap();
    }
    c.frame
        .cell(
            c.frame.body.nodes()[c.initializer.0 as usize]
                .value
                .unwrap(),
        )
        .unwrap()
}
#[test]
fn rustc_intrinsic_reference_places_share_their_reserved_value_statement_scope() {
    let c = context_source(PLACE_ORIGINAL, false, 100);
    let macro_node = c
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| {
            node.role
                == crate::syntax::body_index::NodeRole::Expression(
                    crate::syntax::body_index::ExpressionKind::Macro,
                )
        })
        .unwrap();
    assert_ne!(macro_node.scope, ScopeId(0));
    for value in macro_node
        .runtime_temporaries
        .iter()
        .flatten()
        .chain(macro_node.runtime_scalar_slots.iter())
    {
        let place = SlotId(value.0.checked_add(1).unwrap());
        assert_eq!(c.frame.body.slot_kind(place).unwrap(), SlotKind::Place);
        assert_eq!(c.frame.slots[value.0 as usize].scope, macro_node.scope);
        assert_eq!(c.frame.slots[place.0 as usize].scope, macro_node.scope);
    }
    let statement = &c.frame.body.nodes()[c.statement.0 as usize];
    let value = statement.runtime_temporaries[0].unwrap();
    let place = SlotId(value.0.checked_add(1).unwrap());
    assert_eq!(
        c.frame.slots[value.0 as usize].scope,
        statement.statement_scope.unwrap()
    );
    assert_eq!(
        c.frame.slots[place.0 as usize].scope,
        statement.statement_scope.unwrap()
    );
    assert_eq!(c.frame.counters.operations_started, 0);
    assert_eq!(c.frame.fuel_snapshot().used, 0);
}
#[test]
fn rustc_reference_pattern_verifier_rejects_unrelated_target_or_result_slots() {
    use crate::ir::{recipe::RecipeDraft, validate::verify};
    let mut c = context(false, 100);
    let r = reach_commit(&mut c);
    let draft = || RecipeDraft {
        key: r.recipe().key.clone(),
        origin: r.recipe().origin.clone(),
        operations: r.recipe().operations.to_vec(),
        exits: vec![r.recipe().exit.clone()],
        dependencies: r.recipe().dependencies.to_vec(),
    };
    let handles = c.store.budget().handles();
    let fuel = c.frame.fuel_snapshot().used;
    let initializer = &c.frame.body.nodes()[c.initializer.0 as usize];
    let unrelated = c
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| node.parent != Some(c.statement) && node.value.is_some())
        .unwrap();
    let mut forged = draft();
    let Op::ValuePlace { source, .. } = &mut forged.operations[0] else {
        unreachable!()
    };
    *source = unrelated.value.unwrap();
    assert_eq!(
        verify(forged, &c.frame.body).unwrap_err().code,
        "LRR-E-INTERNAL-IR"
    );
    let mut forged = draft();
    let Op::Borrow { destination, .. } = &mut forged.operations[1] else {
        unreachable!()
    };
    *destination = initializer.value.unwrap();
    assert_eq!(
        verify(forged, &c.frame.body).unwrap_err().code,
        "LRR-E-INTERNAL-IR"
    );
    assert_eq!(c.store.budget().handles(), handles);
    assert_eq!(c.frame.fuel_snapshot().used, fuel);
    assert!(c.frame.binding(c.binding).is_err());
}
#[test]
fn rustc_reference_pattern_slots_are_deterministic_and_checked_before_frame_publication() {
    let c = context(false, 100);
    let repeated = c.parsed.index_body(&c.handle, Default::default()).unwrap();
    assert_eq!(repeated.fingerprint(), c.frame.body.fingerprint());
    let slot = c.frame.body.nodes()[c.statement.0 as usize].runtime_temporaries[0].unwrap();
    assert_eq!(slot.0 % 2, 0);
    assert_eq!(c.frame.body.slot_kind(slot).unwrap(), SlotKind::Value);
    assert_eq!(
        c.frame
            .body
            .slot_kind(SlotId(slot.0.checked_add(1).unwrap()))
            .unwrap(),
        SlotKind::Place
    );
    let handles = c.store.budget().handles();
    let error = c
        .parsed
        .index_body(
            &c.handle,
            crate::syntax::body_index::BodyIndexLimits {
                slots: slot.0.checked_add(1).unwrap(),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-BODY-INDEX-LIMIT");
    assert_eq!(error.message, "reference binding temporary slot budget");
    assert_eq!(c.store.budget().handles(), handles);
    assert_eq!(c.frame.fuel_snapshot().used, 0);
    assert_eq!(c.frame.counters.operations_started, 0);
}
#[test]
fn rustc_initialized_ref_pattern_borrows_original_local_without_reading_or_moving_it() {
    let mut c = context_source(PLACE_ORIGINAL, false, 100);
    let r = reach_commit(&mut c);
    let target = target(&c);
    let original = c.store.read_scalar(target).unwrap();
    let epoch = c.store.location_epoch(target).unwrap();
    assert!(
        c.frame
            .cell(
                c.frame.body.nodes()[c.initializer.0 as usize]
                    .value
                    .unwrap()
            )
            .is_err()
    );
    assert_eq!(c.frame.counters.started_by_opcode[0], 1);
    assert_eq!(c.frame.counters.started_by_opcode[8], 0);
    assert_eq!(c.frame.counters.started_by_opcode[30], 1);
    assert!(matches!(run(&mut c, &r).unwrap(), StepOutcome::Next(_)));
    let reference = c
        .store
        .reference_from_cell(c.frame.binding(c.binding).unwrap())
        .unwrap();
    assert_eq!(c.store.read_reference(reference).unwrap(), original);
    assert_eq!(c.store.state(target).unwrap(), CellState::Live(original));
    assert_eq!(c.store.location_epoch(target).unwrap(), epoch);
    assert_eq!(c.frame.counters.started_by_opcode[8], 0);
    assert_eq!(c.frame.counters.started_by_opcode[38], 0);
    assert_eq!(c.frame.counters.started_by_opcode[31], 1);
    let block = c.frame.body.nodes()[c.binding.0 as usize].scope;
    c.frame.leave(block, &mut c.store).unwrap();
    assert_eq!(c.store.state(target).unwrap(), CellState::Dropped);
    assert!(c.store.read_reference(reference).is_err());
}
fn successful_binding(mutable: bool) {
    let mut c = context(mutable, 100);
    let r = reach_commit(&mut c);
    let target = target(&c);
    let original = c.store.read_scalar(target).unwrap();
    let epoch = c.store.location_epoch(target).unwrap();
    let node = &c.frame.body.nodes()[c.initializer.0 as usize];
    let extension = node.extended_to.as_ref().unwrap();
    assert!(extension.constructors.is_empty());
    let block = extension.scope;
    assert_eq!(c.frame.slots[node.value.unwrap().0 as usize].scope, block);
    let statement_node = &c.frame.body.nodes()[c.statement.0 as usize];
    let statement_scope = statement_node.statement_scope.unwrap();
    let reference_slot = statement_node.runtime_temporaries[0].unwrap();
    assert_ne!(block, statement_scope);
    assert_eq!(
        c.frame.slots[reference_slot.0 as usize].scope,
        statement_scope
    );
    assert!(matches!(run(&mut c, &r).unwrap(), StepOutcome::Next(_)));
    assert!(!c.frame.active_scopes().contains(&statement_scope));
    let owner = c.frame.binding(c.binding).unwrap();
    let reference = c.store.reference_from_cell(owner).unwrap();
    assert_eq!(c.store.read_reference(reference).unwrap(), original);
    assert_eq!(c.store.location_epoch(target).unwrap(), epoch);
    assert_eq!(c.store.state(target).unwrap(), CellState::Live(original));
    assert!(c.frame.cell(reference_slot).is_err());
    // `ref mut` makes the referent mutable, not the pointer binding.
    let handles = c.store.budget().handles();
    let PlaceFailure::RuntimeFault(error) = c
        .store
        .borrow(&Place::cell(owner), Access::Mutable)
        .unwrap_err()
    else {
        panic!("binding access refusal was not a runtime fault")
    };
    assert_eq!(error.code, "LRR-E-REFERENCE-ACCESS");
    assert_eq!(c.store.budget().handles(), handles);
    // A lease obtained by ref mut can replace its original place without
    // changing location identity. A shared lease refuses and retains the ticket.
    let incoming = c.store.ticket(original).unwrap();
    if mutable {
        let old = c.store.replace_reference(reference, incoming).unwrap();
        c.store.discard(old).unwrap();
    } else {
        assert_eq!(
            c.store
                .replace_reference(reference, incoming)
                .unwrap_err()
                .code,
            "LRR-E-REFERENCE-ACCESS"
        );
        assert_eq!(c.store.peek_ticket(incoming).unwrap(), original);
        c.store.discard(incoming).unwrap();
    }
    assert_eq!(c.store.read_reference(reference).unwrap(), original);
    assert_eq!(c.store.location_epoch(target).unwrap(), epoch);
    assert_eq!(c.frame.counters.started_by_opcode[0], 1);
    assert_eq!(c.frame.counters.started_by_opcode[38], 1);
    assert_eq!(c.frame.counters.started_by_opcode[31], 1);
    assert_eq!(c.frame.counters.started_by_opcode[7], 1);
    assert_eq!(c.frame.counters.started_by_opcode[8], 0);
    c.frame.leave(block, &mut c.store).unwrap();
    assert_eq!(c.store.state(target).unwrap(), CellState::Dropped);
    assert_eq!(c.store.state(owner).unwrap(), CellState::Dropped);
    assert!(c.store.read_reference(reference).is_err());
}
#[test]
fn rustc_initialized_ref_pattern_preserves_initializer_owner_through_statement_cleanup() {
    successful_binding(false);
}
#[test]
fn rustc_initialized_ref_mut_pattern_keeps_binding_immutable_and_referent_mutable() {
    successful_binding(true);
}
#[test]
fn rustc_initialized_ref_pattern_fuel_refusal_preserves_target_before_borrow_publication() {
    let mut witness = context(false, 100);
    reach_commit(&mut witness);
    let fuel = witness.frame.fuel_snapshot().used.checked_add(1).unwrap();
    let mut c = context(false, fuel);
    let r = reach_commit(&mut c);
    let target = target(&c);
    let state = c.store.state(target).unwrap();
    let epoch = c.store.location_epoch(target).unwrap();
    let handles = c.store.budget().handles();
    assert_eq!(run(&mut c, &r).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(c.frame.status(), FrameStatus::Faulted);
    assert!(c.frame.binding(c.binding).is_err());
    assert_eq!(c.store.state(target).unwrap(), state);
    assert_eq!(c.store.location_epoch(target).unwrap(), epoch);
    assert_eq!(c.store.budget().handles(), handles);
    assert_eq!(c.frame.counters.started_by_opcode[0], 1);
    assert_eq!(c.frame.counters.started_by_opcode[38], 1);
    assert_eq!(c.frame.counters.started_by_opcode[31], 0);
    assert_eq!(c.frame.counters.started_by_opcode[7], 0);
}
#[test]
fn rustc_initialized_ref_mut_pattern_cancellation_preserves_target_before_place_publication() {
    let mut c = context(true, 100);
    let r = reach_commit(&mut c);
    let target = target(&c);
    let state = c.store.state(target).unwrap();
    let epoch = c.store.location_epoch(target).unwrap();
    let handles = c.store.budget().handles();
    c.frame.fuel.cancellation().cancel();
    assert_eq!(run(&mut c, &r).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(c.frame.status(), FrameStatus::Faulted);
    assert_eq!(
        c.frame.fuel_snapshot().stop,
        Some(crate::vm::fuel::FuelStop::Cancelled)
    );
    assert!(c.frame.binding(c.binding).is_err());
    assert_eq!(c.store.state(target).unwrap(), state);
    assert_eq!(c.store.location_epoch(target).unwrap(), epoch);
    assert_eq!(c.store.budget().handles(), handles);
    assert_eq!(c.frame.counters.started_by_opcode[0], 1);
    assert_eq!(c.frame.counters.started_by_opcode[38], 0);
    assert_eq!(c.frame.counters.started_by_opcode[31], 0);
    assert_eq!(c.frame.counters.started_by_opcode[7], 0);
}
