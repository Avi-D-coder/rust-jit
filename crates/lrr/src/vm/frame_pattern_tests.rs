//! Transaction ownership/quota controls using ordinary conditional source PCs.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{
        op::{Op, PatternOperation},
        pc::Phase,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::parse::ParsedFile,
    vm::step::{StepOutcome, step},
};

fn pending_condition() -> (Frame, GuestStore, PatternOperation, NodeId, SlotId) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(b"fn main(){if let value=7_i32{let _=value;}}".to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([43; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline.items[0].body.as_ref().unwrap();
    let index = parsed.index_body(body, Default::default()).unwrap();
    let mut store = GuestStore::new(1, 1000, 16 << 20).unwrap();
    let mut frame = Frame::new(
        index.clone(),
        index.pc(NodeId(0), Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    for _ in 0..128 {
        let recipe = parsed.initial_thunk(body, &index, &frame.pc).unwrap();
        if let Some(Op::Pattern {
            operation:
                stage @ PatternOperation::StageBinding {
                    binding, source, ..
                },
        }) = recipe.recipe().operations.iter().find(|operation| {
            matches!(
                operation,
                Op::Pattern {
                    operation: PatternOperation::StageBinding { .. }
                }
            )
        }) {
            return (frame, store, stage.clone(), *binding, *source);
        }
        assert!(matches!(
            step(&mut frame, &mut store, &recipe).unwrap(),
            StepOutcome::Next(_)
        ));
    }
    panic!("ordinary pending condition commit not reached")
}

#[test]
fn pending_binding_test_is_non_consuming_and_success_moves_exactly_once() {
    let (mut frame, mut store, stage, binding, source) = pending_condition();
    let PatternOperation::StageBinding { owner, .. } = stage else {
        unreachable!()
    };
    let source_cell = frame.cell(source).unwrap();
    let before = (
        store.state(source_cell).unwrap(),
        store.location_epoch(source_cell).unwrap(),
    );
    frame.execute_pattern(&stage, &mut store).unwrap();
    assert_eq!(
        (
            store.state(source_cell).unwrap(),
            store.location_epoch(source_cell).unwrap()
        ),
        before
    );
    assert!(frame.binding(binding).is_err());
    let finish = PatternOperation::Finish {
        owner,
        matched: None,
    };
    frame.execute_pattern(&finish, &mut store).unwrap();
    assert!(matches!(
        store.state(source_cell).unwrap(),
        CellState::Moved
    ));
    let binding_cell = frame.binding(binding).unwrap();
    let value = store.read_scalar(binding_cell).unwrap();
    assert_eq!(value.payload, 7);
    let after = store.budget().handles();
    assert!(frame.execute_pattern(&finish, &mut store).is_err());
    assert_eq!(store.budget().handles(), after);
    assert_eq!(store.read_scalar(binding_cell).unwrap(), value);
}

#[test]
fn failed_binding_plan_and_overflow_reservation_leave_source_and_destination_untouched() {
    let (mut frame, mut store, stage, binding, source) = pending_condition();
    let source_cell = frame.cell(source).unwrap();
    let state = (
        store.state(source_cell).unwrap(),
        store.location_epoch(source_cell).unwrap(),
        store.budget().handles(),
        store.budget().bytes(),
    );
    frame.execute_pattern(&stage, &mut store).unwrap();
    assert!(frame.execute_pattern(&stage, &mut store).is_err());
    assert!(store.preflight_pattern_bindings(usize::MAX).is_err());
    assert!(store.reserve_pattern_bindings(usize::MAX).is_err());
    assert_eq!(
        (
            store.state(source_cell).unwrap(),
            store.location_epoch(source_cell).unwrap(),
            store.budget().handles(),
            store.budget().bytes()
        ),
        state
    );
    assert!(frame.binding(binding).is_err());
    let PatternOperation::StageBinding { owner, .. } = stage else {
        unreachable!()
    };
    frame
        .execute_pattern(&PatternOperation::Rollback { owner }, &mut store)
        .unwrap();
    assert!(frame.patterns.current(owner).is_err());
    assert_eq!(store.state(source_cell).unwrap(), state.0);
}

#[test]
fn generic_guard_preview_copies_without_consuming_pending_source_and_scope_cleanup_retires_it() {
    // Exercise the internal shared transition with real source/frame cells.
    // Public IR admission separately restricts emitted Preview to match guards;
    // this conditional source otherwise emits the unpreviewed move commit.
    let (mut frame, mut store, stage, binding, source) = pending_condition();
    let PatternOperation::StageBinding { owner, .. } = stage else {
        unreachable!()
    };
    let cell = frame.cell(source).unwrap();
    let before = (
        store.state(cell).unwrap(),
        store.location_epoch(cell).unwrap(),
    );
    frame.execute_pattern(&stage, &mut store).unwrap();
    frame
        .execute_pattern(&PatternOperation::Preview { owner }, &mut store)
        .unwrap();
    assert_eq!(
        (
            store.state(cell).unwrap(),
            store.location_epoch(cell).unwrap()
        ),
        before
    );
    assert_eq!(
        store
            .read_scalar(frame.binding(binding).unwrap())
            .unwrap()
            .payload,
        7
    );
    assert!(frame.patterns.current(owner).unwrap().previewed);
    while let Some(scope) = frame.active_scopes.last().copied() {
        frame.leave(scope, &mut store).unwrap();
    }
    assert!(frame.patterns.current(owner).is_err());
    assert!(frame.binding(binding).is_err());
    assert!(frame.cell(source).is_err());
}
