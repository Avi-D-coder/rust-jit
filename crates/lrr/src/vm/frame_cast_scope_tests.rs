//! Stock cast ownership plus LRR source-identity and refusal controls.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{op::Op, pc::Phase},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{parse::ParsedFile, scope_fixture_facts},
    vm::step::{StepOutcome, step},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/cast/cast-not-treated-as-lvalue.rs"
);
const CASTS: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/cast/cast.rs");
const SHADOWS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/resolve/shadow-primitives.rs"
);
#[test]
fn rustc_borrowed_cast_owns_extended_temporary_distinct_from_original_local() {
    let facts = scope_fixture_facts::original_cast_borrow(ORIGINAL);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-cast-temporary"),
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
    let id = |range| {
        body.nodes()
            .iter()
            .find(|node| node.range == range)
            .unwrap()
            .id
    };
    let statement = id(facts.statement);
    let original_binding = id(facts.original_binding);
    let borrowed_binding = id(facts.borrowed_binding);
    let temporary = id(facts.temporary);
    let stop = body.pc(id(facts.stop), Phase::Entry).unwrap();
    let block = body.nodes()[temporary.0 as usize]
        .extended_to
        .as_ref()
        .unwrap()
        .scope;
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(
        body.clone(),
        body.pc(statement, Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    let mut scopes = vec![];
    let mut scope = Some(body.nodes()[statement.0 as usize].scope);
    while let Some(id) = scope {
        scopes.push(id);
        scope = body.scopes()[id.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    let mut reached = false;
    for _ in 0..64 {
        if frame.pc() == &stop {
            reached = true;
            break;
        }
        let recipe = parsed.initial_thunk(handle, &body, frame.pc()).unwrap();
        assert!(matches!(
            step(&mut frame, &mut store, &recipe).unwrap(),
            StepOutcome::Next(_)
        ));
    }
    assert!(reached, "original first three statements did not complete");
    let owner = frame.binding(original_binding).unwrap();
    let reference_owner = frame.binding(borrowed_binding).unwrap();
    let target = frame
        .cell(body.nodes()[temporary.0 as usize].value.unwrap())
        .unwrap();
    assert_ne!(owner, target);
    assert_eq!(
        store.read_scalar(owner).unwrap().payload as i64,
        facts.assigned
    );
    let reference = store.reference_from_cell(reference_owner).unwrap();
    assert_eq!(
        store.read_reference(reference).unwrap().payload as i64,
        facts.original
    );
    assert_eq!(
        frame.slots[body.nodes()[temporary.0 as usize].value.unwrap().0 as usize].scope,
        block
    );
    assert_eq!(frame.counters.started_by_opcode[12], 1);
    assert_eq!(frame.counters.started_by_opcode[31], 1);
    assert_eq!(frame.counters.started_by_opcode[26], 0);
    frame.leave(block, &mut store).unwrap();
    assert_eq!(store.state(owner).unwrap(), CellState::Dropped);
    assert_eq!(store.state(target).unwrap(), CellState::Dropped);
    assert!(store.read_reference(reference).is_err());
    // No raw pointer comparison, source Drop, issue_36936 or whole run-pass claim.
}
#[test]
fn rustc_stock_shadowed_cast_target_refuses_before_operand_entry() {
    let adapted = scope_fixture_facts::original_cast_with_stock_shadow(CASTS, SHADOWS);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(adapted.source.into_bytes()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-cast-shadow-guard"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline.items[0].body.as_ref().unwrap();
    let body = parsed.index_body(handle, Default::default()).unwrap();
    let cast = body
        .nodes()
        .iter()
        .find(|node| node.range == adapted.cast)
        .unwrap();
    let entry = parsed
        .initial_thunk(handle, &body, &body.pc(cast.id, Phase::Entry).unwrap())
        .unwrap();
    assert!(matches!(
        entry.recipe().operations.as_ref(),
        [Op::Unsupported(_)]
    ));
    assert!(
        !entry
            .recipe()
            .operations
            .iter()
            .any(|op| matches!(op, Op::PrimitiveCast { .. } | Op::SetHint { .. }))
    );
    let commit = parsed
        .initial_thunk(handle, &body, &body.pc(cast.id, Phase::Operand(0)).unwrap())
        .unwrap();
    assert!(matches!(
        commit.recipe().operations.as_ref(),
        [Op::Unsupported(_)]
    ));
}

#[test]
fn rustc_stock_value_function_keeps_cast_target_in_primitive_type_namespace() {
    let adapted = scope_fixture_facts::original_cast_with_stock_value_function(CASTS, SHADOWS);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(adapted.source.into_bytes()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-value-function-cast-type"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline.items[0].body.as_ref().unwrap();
    let body = parsed.index_body(handle, Default::default()).unwrap();
    let cast = body
        .nodes()
        .iter()
        .find(|node| node.range == adapted.cast)
        .unwrap();
    let entry = parsed
        .initial_thunk(handle, &body, &body.pc(cast.id, Phase::Entry).unwrap())
        .unwrap();
    assert!(matches!(
        entry.recipe().operations.as_ref(),
        [Op::SetHint { .. }]
    ));
    let commit = parsed
        .initial_thunk(handle, &body, &body.pc(cast.id, Phase::Operand(0)).unwrap())
        .unwrap();
    assert!(matches!(
        commit.recipe().operations.as_ref(),
        [Op::PrimitiveCast {
            target: crate::ir::op::PrimitiveType::U8,
            ..
        }]
    ));
}

#[test]
fn rustc_stock_hoisted_trait_refuses_primitive_let_context_before_operand_entry() {
    let adapted = scope_fixture_facts::original_let_with_stock_trait(SHADOWS);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(adapted.source.into_bytes()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-local-trait-let-type"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline.items[0].body.as_ref().unwrap();
    let body = parsed.index_body(handle, Default::default()).unwrap();
    let statement = body
        .nodes()
        .iter()
        .find(|node| node.range == adapted.statement)
        .unwrap();
    let entry = parsed
        .initial_thunk(handle, &body, &body.pc(statement.id, Phase::Entry).unwrap())
        .unwrap();
    let expected_scope = statement.statement_scope.unwrap();
    assert!(
        matches!(
            entry.recipe().operations.as_ref(),
            [Op::ScopeEnter { scope }, Op::Unsupported(diagnostic)]
                if *scope == expected_scope
                    && diagnostic.message == "local type requires later symbolic type resolution"
        ),
        "unexpected guarded let recipe: {:?}",
        entry.recipe().operations
    );
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(body.clone(), entry.recipe().key.clone(), &mut store).unwrap();
    let mut ancestors = vec![];
    let mut parent = body.scopes()[expected_scope.0 as usize].parent;
    while let Some(scope) = parent {
        ancestors.push(scope);
        parent = body.scopes()[scope.0 as usize].parent;
    }
    for scope in ancestors.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    let refusal = step(&mut frame, &mut store, &entry).unwrap_err();
    assert_eq!(refusal.code, "LRR-E-UNSUPPORTED");
    assert_eq!(
        refusal.message,
        "local type requires later symbolic type resolution"
    );
    assert!(frame.slots.iter().all(|slot| slot.cell.is_none()
        && slot.hint.is_none()
        && slot.source.is_none()
        && slot.place.is_none()));
    assert_eq!(frame.counters.started_by_opcode[12], 0);
    assert_eq!(frame.counters.started_by_opcode[2], 0);
    assert_eq!(frame.counters.started_by_opcode[7], 0);
}
