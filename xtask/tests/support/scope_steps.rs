//! Resume bounded ScopeLeave without treating Dispatch as recipe completion.
use lrr::{
    core::diagnostic::Diagnostic,
    ir::{op::Op, recipe::VerifiedRecipe},
    runtime::cell::GuestStore,
    vm::{
        frame::Frame,
        step::{StepOutcome, step},
    },
};
pub fn finish_recipe(
    frame: &mut Frame,
    store: &mut GuestStore,
    recipe: &VerifiedRecipe,
) -> Result<StepOutcome, Diagnostic> {
    let pc = frame.pc().clone();
    assert_eq!(&pc, &recipe.recipe().key);
    let before = frame.counters().clone();
    let operations = &recipe.recipe().operations;
    let scope_entries = operations
        .iter()
        .filter(|op| matches!(op, Op::ScopeLeave { .. }))
        .count() as u64;
    let mut last: Option<(usize, u64, u64)> = None;
    for _ in 0..4096 {
        let result = step(frame, store, recipe)?;
        if matches!(result, StepOutcome::Dispatch) {
            assert_eq!(frame.pc(), &pc, "cleanup dispatch changed recipe PC");
            let started = frame.counters().operations_started;
            let index = usize::try_from(
                started
                    .checked_sub(before.operations_started)
                    .and_then(|count| count.checked_sub(1))
                    .expect("dispatch has no entered opcode"),
            )
            .unwrap();
            let Some(Op::ScopeLeave { scope }) = operations.get(index) else {
                panic!("scope fixture dispatched outside ScopeLeave")
            };
            assert!(frame.active_scopes().contains(scope));
            assert_eq!(
                frame.counters().operations_completed,
                before.operations_completed + index as u64,
                "pending ScopeLeave counted complete or replayed recipe prefix"
            );
            let scope_started = frame.counters().started_by_opcode[5];
            if let Some((previous, previous_started, previous_scope_started)) = last {
                assert!(index >= previous, "cleanup replayed earlier opcode");
                if index == previous {
                    assert_eq!(started, previous_started, "cleanup recharged entry");
                    assert_eq!(scope_started, previous_scope_started);
                }
            }
            last = Some((index, started, scope_started));
            continue;
        }
        assert_eq!(
            frame.counters().operations_started,
            before.operations_started + operations.len() as u64
        );
        assert_eq!(
            frame.counters().operations_completed,
            before.operations_completed + operations.len() as u64
        );
        assert_eq!(
            frame.counters().started_by_opcode[5],
            before.started_by_opcode[5] + scope_entries
        );
        return Ok(result);
    }
    panic!("bounded scope fixture did not finish its recipe");
}
