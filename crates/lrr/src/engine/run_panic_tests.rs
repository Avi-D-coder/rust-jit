//! The unchanged upstream panic program plus disclosed LRR suspension controls.
use super::*;
use crate::{
    runtime::{cell::CellState, shape::TypeId},
    vm::{frame::FrameStatus, numeric::PanicKind},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/panics/panic-macro-explicit.rs"
);
const PARENS: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/panics/panic-parens.rs");
const WHILE_BODY: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/panics/while-body-panics.rs"
);
const ARGUMENT: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/panics/panic-arg.rs");
const ARGUMENTS: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/panics/args-panic.rs");

#[test]
fn rustc_original_second_argument_panic_keeps_later_box_hint_and_callee_cold() {
    let (mut db, run, report) = run_original_panic(ARGUMENTS);
    let cold = run.sources[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "f")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let start = u32::try_from(ARGUMENTS.find("Box::new(42)").unwrap()).unwrap()
        - original_root_body_start(&run);
    let later = run
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| {
            node.range.start == start
                && node.range.end == start + u32::try_from("Box::new(42)".len()).unwrap()
                && node.role
                    == crate::syntax::body_index::NodeRole::Expression(
                        crate::syntax::body_index::ExpressionKind::Call,
                    )
        })
        .unwrap();
    assert_eq!(report.panic.pc.body, run.root_body);
    assert_eq!(run.peak_guest_frames, 1);
    assert_eq!(report.opcode_counters.started_by_opcode[22], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[23], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[19], 0);
    assert!(db.trace.events.iter().all(|event| match &event.key {
        QueryKey::BodyStructure(body) => *body != cold,
        QueryKey::Thunk(pc) =>
            pc.body != cold && (pc.body != run.root_body || pc.node != later.id.0),
        _ => true,
    }));
    db.finish_run().unwrap();
    // The unchanged Box signature and constructor are not adapted away. Their
    // source hint is never demanded because the second argument panics first.
}

#[test]
fn rustc_original_argument_panic_retires_selected_metadata_after_root_cleanup() {
    let (mut db, run, report) = run_original_panic(ARGUMENT);
    assert_eq!(report.opcode_counters.started_by_opcode[22], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[19], 0);
    assert!(run.pending.is_empty());
    assert_eq!(run.pending_bytes, 0);
    assert_eq!(run.frame.status(), FrameStatus::PanicCleaned);
    db.finish_run().unwrap();
}

#[test]
fn rustc_panic_metadata_admission_counts_pending_source_calls_before_cleanup() {
    let (mut db, mut run, root, fields) = panic_frontier_source(ARGUMENT);
    // Disclosed resource frontier: normal source selection retains the original
    // f call while the prepared structural stress owner remains unprocessed.
    let active = run.unwinding.take().unwrap();
    let selected = run
        .select_argument_call(
            &mut db,
            "f",
            &[],
            &[run.frame.body.nodes()[run.frame.pc().node as usize]
                .value
                .unwrap()],
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let bytes = selected.bytes;
    let call = run
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| {
            node.role
                == crate::syntax::body_index::NodeRole::Expression(
                    crate::syntax::body_index::ExpressionKind::Call,
                )
        })
        .unwrap()
        .id
        .0;
    run.pending.insert(call, selected);
    run.pending_bytes = bytes;
    let limits = run.call_limits;
    run.call_limits.max_metadata_bytes = bytes;
    let pc = run.frame.pc().clone();
    let used = run.frame.fuel_snapshot().used;
    let root_state = run.store.state(root).unwrap();
    let states: Vec<_> = fields
        .iter()
        .map(|field| run.store.state(*field).unwrap())
        .collect();
    let origin = crate::ir::recipe::Origin {
        span: run.frame.body.span(NodeId(pc.node)).unwrap(),
        expansion_ancestry: vec![],
    };
    assert_eq!(
        run.begin_panic_cleanup(crate::vm::step::GuestPanic {
            kind: PanicKind::Explicit,
            message: Some("woe".into()),
            pc: pc.clone(),
            origin: origin.clone(),
        })
        .unwrap_err()
        .code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert!(run.unwinding.is_none());
    assert_eq!(run.frame.pc(), &pc);
    assert_eq!(run.frame.fuel_snapshot().used, used);
    assert_eq!(run.pending_bytes, bytes);
    assert_eq!(run.pending.len(), 1);
    assert_eq!(run.store.state(root).unwrap(), root_state);
    assert_eq!(
        fields
            .iter()
            .map(|field| run.store.state(*field).unwrap())
            .collect::<Vec<_>>(),
        states
    );
    run.call_limits = limits;
    drop(active);
    run.begin_panic_cleanup(crate::vm::step::GuestPanic {
        kind: PanicKind::Explicit,
        message: Some("woe".into()),
        pc,
        origin,
    })
    .unwrap();
    assert!(run.advance_panic_cleanup().unwrap().is_none());
    assert_eq!(run.pending_bytes, bytes);
    assert_eq!(run.pending.len(), 1);
    assert!(matches!(run.poll(&mut db), RunPoll::Panicked(_)));
    assert!(run.pending.is_empty());
    assert_eq!(run.pending_bytes, 0);
    db.finish_run().unwrap();
}

const DEFAULT_PANIC: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/panics/explicit-panic.rs");
const COLD_SECOND_PANIC: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/panics/doublepanic.rs");
const ASSERTION_PANIC: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/panics/panic.rs");

#[test]
fn rustc_original_default_panic_preserves_message_and_cleans_root() {
    let (mut db, run, report) = run_original_panic(DEFAULT_PANIC);
    assert_eq!(report.panic.kind, PanicKind::Explicit);
    assert_eq!(report.panic.pc.body, run.root_body);
    assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
    db.finish_run().unwrap();
}

#[test]
fn rustc_original_first_panic_keeps_second_panic_recipe_cold() {
    let (mut db, run, report) = run_original_panic(COLD_SECOND_PANIC);
    let spelling = "panic!(\"Two\")";
    let start = u32::try_from(COLD_SECOND_PANIC.find(spelling).unwrap()).unwrap()
        - original_root_body_start(&run);
    let second = run
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| {
            node.range.start == start
                && node.range.end == start + u32::try_from(spelling.len()).unwrap()
                && node.role
                    == crate::syntax::body_index::NodeRole::Expression(
                        crate::syntax::body_index::ExpressionKind::Macro,
                    )
        })
        .unwrap();
    assert_ne!(report.panic.pc.node, second.id.0);
    assert!(db.trace.events.iter().all(|event| !matches!(&event.key, QueryKey::Thunk(pc) if pc.body == run.root_body && pc.node == second.id.0)));
    assert_eq!(run.peak_guest_frames, 1);
    assert_eq!(run.action_snapshot().published, 0);
    db.finish_run().unwrap();
}

#[test]
fn rustc_original_failed_assertion_preserves_assertion_kind_and_cleans_root() {
    let (mut db, run, report) = run_original_guest_panic(ASSERTION_PANIC, PanicKind::Assertion, 26);
    assert_eq!(report.panic.kind, PanicKind::Assertion);
    assert_eq!(report.panic.pc.body, run.root_body);
    assert_eq!(report.opcode_counters.started_by_opcode[25], 0);
    assert!(run.pending.is_empty() && run.pending_bytes == 0);
    db.finish_run().unwrap();
}

fn original_root_body_start(run: &RunMachine) -> u32 {
    run.sources
        .iter()
        .flat_map(|view| view.outline().items.iter())
        .filter_map(|item| item.body.as_ref())
        .find(|body| body.key() == run.root_body)
        .unwrap()
        .range
        .start
}

fn original_error_pattern(source: &str) -> &str {
    let mut patterns = source
        .lines()
        .filter_map(|line| line.strip_prefix("//@ error-pattern:"));
    let pattern = patterns.next().unwrap().trim();
    assert!(patterns.next().is_none());
    pattern
}
fn run_original_panic(source: &str) -> (Database, RunMachine, PanicReport) {
    run_original_guest_panic(source, PanicKind::Explicit, 25)
}
fn run_original_guest_panic(
    source: &str,
    kind: PanicKind,
    opcode: usize,
) -> (Database, RunMachine, PanicReport) {
    let (mut db, mut run) = borrows::tests::start_stock_body(source, "main");
    run.frame.set_fuel_limit(1_000_000).unwrap();
    let view = run.sources[0].share();
    let macros = crate::engine::standalone_macros::StandaloneRuntimeMacros::new(&view).unwrap();
    let contexts = [macros.context(&view).unwrap()];
    let report = match run.poll_with_runtime_macros(&mut db, &contexts) {
        RunPoll::Panicked(report) => report,
        RunPoll::Fault(error) => panic!("unchanged panic program faulted: {error:?}"),
        _ => panic!("unchanged panic program did not produce its ordinary guest panic"),
    };
    assert!(
        report
            .panic
            .description()
            .contains(original_error_pattern(source))
    );
    assert_eq!(report.panic.kind, kind);
    assert_eq!(report.opcode_counters.started_by_opcode[opcode], 1);
    assert_eq!(run.frame.status(), FrameStatus::PanicCleaned);
    assert!(run.frame.active_scopes().is_empty());
    assert!(run.calls.is_empty() && run.unwinding.is_none());
    assert_eq!(run.call_bytes, 0);
    assert!(run.terminal);
    (db, run, report)
}

#[test]
fn rustc_original_callee_condition_panic_unwinds_caller_without_entering_cold_inner_panics() {
    let (mut db, run, report) = run_original_panic(PARENS);
    let callee = run.sources[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "bigpanic")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    assert_eq!(report.panic.pc.body, callee);
    assert_ne!(report.panic.pc.body, run.root_body);
    assert!(run.peak_guest_frames >= 2);
    assert_eq!(run.action_snapshot().published, 1);
    assert_eq!(run.action_snapshot().consumed, 1);
    assert!(report.opcode_counters.panic_scopes_cleaned >= 2);
    db.finish_run().unwrap();
}

#[test]
fn rustc_original_loop_body_panic_cleans_enclosing_scopes_before_publication() {
    let (mut db, run, report) = run_original_panic(WHILE_BODY);
    assert_eq!(report.panic.pc.body, run.root_body);
    assert_eq!(run.peak_guest_frames, 1);
    assert_eq!(run.action_snapshot().published, 0);
    assert_eq!(run.action_snapshot().consumed, 0);
    assert!(report.opcode_counters.panic_scopes_cleaned > 0);
    db.finish_run().unwrap();
}

#[test]
fn rustc_original_argument_panic_keeps_println_callee_body_cold() {
    let (mut db, run, report) = run_original_panic(ARGUMENT);
    let cold = run.sources[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "f")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    assert_eq!(report.panic.pc.body, run.root_body);
    assert_eq!(run.peak_guest_frames, 1);
    assert!(db.trace.events.iter().all(|event| match &event.key {
        QueryKey::BodyStructure(body) => *body != cold,
        QueryKey::Thunk(pc) => pc.body != cold,
        _ => true,
    }));
    db.finish_run().unwrap();
    // The original f/println body stays unchanged and unentered. This library
    // control doesn't fulfill the copied needs-subprocess directive by itself.
}

#[test]
fn rustc_original_explicit_panic_program_publishes_only_after_cleanup() {
    let (mut db, mut run) = borrows::tests::start_stock_body(ORIGINAL, "main");
    let view = run.sources[0].share();
    let macros = crate::engine::standalone_macros::StandaloneRuntimeMacros::new(&view).unwrap();
    let contexts = [macros.context(&view).unwrap()];
    let report = match run.poll_with_runtime_macros(&mut db, &contexts) {
        RunPoll::Panicked(report) => report,
        RunPoll::Fault(error) => panic!("unchanged panic program faulted: {error:?}"),
        _ => panic!("unchanged panic program did not produce an ordinary guest panic"),
    };
    assert_eq!(report.panic.description(), "explicit panic");
    assert!(report.opcode_counters.panic_scopes_cleaned > 0);
    assert_eq!(run.frame.status(), FrameStatus::PanicCleaned);
    assert!(run.frame.active_scopes().is_empty());
    assert!(run.calls.is_empty() && run.unwinding.is_none());
    assert!(run.terminal);
    db.finish_run().unwrap();
    // Original needs-subprocess/error-pattern directives are retained, but this
    // host library control alone is not official subprocess compiletest acceptance.
}

fn panic_frontier() -> (
    Database,
    RunMachine,
    crate::runtime::cell::CellId,
    Vec<crate::runtime::cell::CellId>,
) {
    panic_frontier_source(ORIGINAL)
}

fn panic_frontier_source(
    source: &str,
) -> (
    Database,
    RunMachine,
    crate::runtime::cell::CellId,
    Vec<crate::runtime::cell::CellId>,
) {
    let (db, mut run) = borrows::tests::start_stock_body(source, "main");
    let node = run
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
    let owner = node.value.unwrap();
    let pc = run.frame.body.pc(node.id, Phase::Entry).unwrap();
    let origin = crate::ir::recipe::Origin {
        span: run.frame.body.span(node.id).unwrap(),
        expansion_ancestry: vec![],
    };
    let mut scope = Some(node.scope);
    let mut ancestry = Vec::new();
    while let Some(id) = scope {
        ancestry.push(id);
        scope = run.frame.body.scopes()[id.0 as usize].parent;
    }
    for id in ancestry.into_iter().rev() {
        run.frame.enter(id).unwrap();
    }
    run.frame.pc = pc.clone();
    run.frame.set_fuel_limit(10_000).unwrap();
    let ty = run
        .store
        .intern_type(crate::runtime::shape::TypeKey::Array {
            element: TypeId(8),
            len: 257,
        })
        .unwrap();
    let guard = run.store.begin_construction(&vec![TypeId(8); 257]).unwrap();
    for ordinal in 0..257 {
        let ticket = run
            .store
            .ticket(ValueRepr::signed(TypeId(8), i64::from(ordinal)).unwrap())
            .unwrap();
        run.store.construction_push(guard, ordinal, ticket).unwrap();
    }
    let ticket = run.store.commit_construction(guard, ty).unwrap();
    let fields = (0..257)
        .map(|ordinal| run.store.aggregate_field(ticket, ordinal).unwrap())
        .collect();
    run.frame.write(owner, ticket, &mut run.store).unwrap();
    let root = run.frame.cell(owner).unwrap();
    // Supplemental continuation injection at the original panic node. The
    // prepared intrinsic array is a disclosed stress owner, absent upstream.
    run.frame.status = FrameStatus::Panicked;
    run.begin_panic_cleanup(crate::vm::step::GuestPanic {
        kind: PanicKind::Explicit,
        message: Some("explicit panic".into()),
        pc,
        origin,
    })
    .unwrap();
    (db, run, root, fields)
}

#[test]
fn rustc_panic_frontier_retains_first_panic_through_bounded_aggregate_cleanup() {
    let (mut db, mut run, root, fields) = panic_frontier();
    let pc = run.frame.pc().clone();
    let before = run.frame.counters().clone();
    let trace = db.trace.events.len();
    assert!(run.advance_panic_cleanup().unwrap().is_none());
    assert!(run.unwinding.is_some() && !run.terminal);
    assert!(run.frame.has_cleanup_activation());
    assert_eq!(run.frame.status(), FrameStatus::Panicked);
    assert_eq!(run.frame.pc(), &pc);
    let RunPoll::Panicked(report) = run.poll(&mut db) else {
        panic!("bounded panic cleanup did not publish the retained panic")
    };
    assert_eq!(report.panic.description(), "explicit panic");
    assert_eq!(report.panic.pc, pc);
    assert!(run.unwinding.is_none() && run.terminal);
    assert_eq!(db.trace.events.len(), trace);
    assert_eq!(
        report.opcode_counters.operations_started,
        before.operations_started
    );
    assert_eq!(
        report.opcode_counters.operations_completed,
        before.operations_completed
    );
    assert_eq!(run.store.state(root).unwrap(), CellState::Dropped);
    assert!(
        fields
            .iter()
            .all(|field| run.store.state(*field).unwrap() == CellState::Dropped)
    );
    db.finish_run().unwrap();
}

#[test]
fn rustc_panic_frontier_cancellation_stops_without_expected_panic_publication() {
    let (mut db, mut run, root, fields) = panic_frontier();
    assert!(run.advance_panic_cleanup().unwrap().is_none());
    let states: Vec<_> = fields
        .iter()
        .map(|field| run.store.state(*field).unwrap())
        .collect();
    let root_state = run.store.state(root).unwrap();
    let used = run.frame.fuel_snapshot().used;
    run.frame.fuel.cancellation().cancel();
    assert!(
        matches!(run.poll(&mut db), RunPoll::Fault(error) if error.code == "LRR-E-RUNTIME-RESOURCE")
    );
    assert_eq!(run.frame.status(), FrameStatus::Faulted);
    assert!(run.unwinding.is_some() && run.frame.has_cleanup_activation());
    assert_eq!(run.frame.fuel_snapshot().used, used);
    assert_eq!(run.store.state(root).unwrap(), root_state);
    assert_eq!(
        fields
            .iter()
            .map(|field| run.store.state(*field).unwrap())
            .collect::<Vec<_>>(),
        states
    );
    assert!(run.terminal);
    db.finish_run().unwrap();
}

#[test]
fn rustc_panic_frontier_second_panic_admission_refuses_before_more_cleanup() {
    let (mut db, mut run, root, fields) = panic_frontier();
    assert!(run.advance_panic_cleanup().unwrap().is_none());
    let states: Vec<_> = fields
        .iter()
        .map(|field| run.store.state(*field).unwrap())
        .collect();
    let used = run.frame.fuel_snapshot().used;
    let pc = run.frame.pc().clone();
    let origin = crate::ir::recipe::Origin {
        span: run.frame.body.span(NodeId(pc.node)).unwrap(),
        expansion_ancestry: vec![],
    };
    let root_state = run.store.state(root).unwrap();
    assert_eq!(
        run.begin_panic_cleanup(crate::vm::step::GuestPanic {
            kind: PanicKind::Explicit,
            message: Some("second".into()),
            pc,
            origin,
        })
        .unwrap_err()
        .code,
        "LRR-E-DOUBLE-PANIC"
    );
    assert!(run.unwinding.is_some());
    assert_eq!(run.frame.fuel_snapshot().used, used);
    assert_eq!(run.store.state(root).unwrap(), root_state);
    assert_eq!(
        fields
            .iter()
            .map(|field| run.store.state(*field).unwrap())
            .collect::<Vec<_>>(),
        states
    );
    db.finish_run().unwrap();
    // Admission-only injection: actual guest destructor second panic and
    // terminal worker/test isolation are pending, never credited by this control.
}
