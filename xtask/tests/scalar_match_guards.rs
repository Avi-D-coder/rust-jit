//! T050/T051 ordinary zero-binding single-alternative boolean guards.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    engine::run::{RunMachine, RunPoll, ValueSummary},
    ir::{
        op::{HintRecipe, Op, PrimitiveType},
        pc::Phase,
        recipe::RecipeExit,
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        parse::ParseKey,
    },
};
const ORIGINAL: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/nll/match-on-borrowed.rs");
fn source(text: &str, edition: u16) -> (Database, PreparedSourceView) {
    let path = SourcePath::new("integer-pattern.rs").unwrap();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let owner = Digest::of_bytes(b"LRR scalar numeric pattern source");
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: owner,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            blob,
            profile,
            cfg,
            crate_key: owner,
            module_path: vec![],
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("pattern source absent")
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    (db, view)
}

#[test]
fn unchanged_wildcard_guard_commit_preserves_indexed_scope_bool_hint_and_cold_guard() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view) = source(ORIGINAL, edition);
        let handle = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "underscore_example")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let body = handle.key();
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::BodyStructure(body.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
            panic!("guard index")
        };
        let owner = index
            .nodes()
            .iter()
            .find(|n| n.role == NodeRole::Expression(ExpressionKind::Match))
            .unwrap();
        let arm = index
            .nodes()
            .iter()
            .find(|n| {
                n.role == NodeRole::MatchArm
                    && n.range.start > owner.range.start
                    && n.range.end < owner.range.end
            })
            .unwrap();
        let scope = arm.guard_scope.unwrap();
        let guard_range = index.scopes()[scope.0 as usize].range;
        let guard = index
            .nodes()
            .iter()
            .find(|n| n.range == guard_range && matches!(n.role, NodeRole::Expression(_)))
            .unwrap();
        let pattern = index
            .nodes()
            .iter()
            .find(|n| {
                n.role == NodeRole::Pattern
                    && n.range.start >= arm.range.start
                    && n.range.end < guard_range.start
            })
            .unwrap();
        let pc = index.pc(pattern.id, Phase::Commit).unwrap();
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!("guard commit")
        };
        assert_eq!(recipe.recipe().origin.span, index.span(pattern.id).unwrap());
        assert!(
            recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op,Op::ScopeEnter{scope} if *scope==arm.scope))
        );
        assert!(recipe.recipe().operations.iter().any(|op|matches!(op,Op::SetHint{child,hint:HintRecipe::Primitive(PrimitiveType::Bool)} if Some(*child)==guard.value)));
        assert_eq!(
            recipe.recipe().exit,
            RecipeExit::Next(index.pc(guard.id, Phase::Entry).unwrap())
        );
        assert!(
            !db.trace
                .events
                .iter()
                .any(|e| matches!(&e.key,QueryKey::Thunk(key) if key.node==guard.id.0))
        );
    }
}
fn run(text: &str, expected: &str, assignments: u64, compares: u64) {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view) = source(text, edition);
        let handle = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "value")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .clone();
        let body = handle.key();
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::BodyStructure(body.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
            panic!("guard cold index")
        };
        let cold = index
            .nodes()
            .iter()
            .filter(|node| {
                let start = handle.range.start.checked_add(node.range.start).unwrap() as usize;
                let end = handle.range.start.checked_add(node.range.end).unwrap() as usize;
                text.get(start..end) == Some("unknown()")
            })
            .map(|node| node.id.0)
            .collect::<Vec<_>>();
        let revision = db.source_revision();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut machine)) =
            RunMachine::start_with_sources(&mut db, body.clone(), 100_000, vec![view])
        else {
            panic!("guard startup")
        };
        let RunPoll::Completed(report) = machine.poll(&mut db) else {
            panic!("guard runtime not completed")
        };
        assert!(matches!(report.value,ValueSummary::Signed(ref value) if value==expected));
        assert_eq!(report.opcode_counters.started_by_opcode[16], assignments);
        assert_eq!(report.opcode_counters.started_by_opcode[10], compares);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        assert_eq!(
            report.opcode_counters.fuel_used,
            report
                .opcode_counters
                .operations_started
                .checked_add(report.opcode_counters.exits_completed)
                .unwrap()
                .checked_add(report.opcode_counters.cleanup_elements_completed)
                .unwrap()
        );
        assert_eq!(db.source_revision(), revision);
        assert_eq!(db.trace.truncated, 0);
        assert!(!db.trace.events.iter().any(|event| matches!(&event.key,
            QueryKey::Thunk(pc) if pc.body == body && cold.contains(&pc.node))));
        db.finish_run().unwrap();
    }
}
#[test]
fn disclosed_guard_order_adapters_select_true_and_false_paths_without_source_replay() {
    run(
        "fn value()->i32{match 1{_ if true=>2,1=>unknown()}}fn main(){}",
        "2",
        0,
        0,
    );
    run(
        "fn value()->i32{match 1{1=>1,_ if true=>unknown()}}fn main(){}",
        "1",
        0,
        1,
    );
    run(
        "fn value()->i32{match 1{1 if false=>unknown(),1=>2,_=>unknown()}}fn main(){}",
        "2",
        0,
        2,
    );
    run(
        "fn value()->i32{match 1{0 if unknown()=>unknown(),_=>2}}fn main(){}",
        "2",
        0,
        1,
    );
}
#[test]
fn false_guard_and_true_guard_effects_survive_once_before_selected_body() {
    run(
        "fn value()->i32{let mut seen:i32=0;seen=seen+1;match 1{1 if {seen=seen+1;false}=>unknown(),_=>{seen=seen+1;}};seen}fn main(){}",
        "3",
        3,
        4,
    );
    run(
        "fn value()->i32{let mut seen:i32=0;match 1{1 if {seen=seen+1;true}=>{seen=seen+1;},_=>unknown()};seen}fn main(){}",
        "2",
        2,
        3,
    );
    run(
        "fn value()->i32{let source:i8=1;let result=match source{1 if false=>0,_=>2};source;result}fn main(){}",
        "2",
        0,
        1,
    );
}
#[test]
fn guarded_multiple_alternatives_keep_explicit_continuation_frontier_after_matching() {
    let text = "fn value()->bool{match true{true | true if false=>false,_=>true}}fn main(){}";
    let (mut db, view) = source(text, 2024);
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "value")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut machine)) =
        RunMachine::start_with_sources(&mut db, body, 100_000, vec![view])
    else {
        panic!("overlap startup")
    };
    let RunPoll::Fault(error) = machine.poll(&mut db) else {
        panic!("overlapping guarded alternatives must retain prerequisite")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert!(error.message.contains("alternative continuation"));
    db.finish_run().unwrap();
}
#[test]
fn all_false_guards_use_typed_no_arm_and_reached_let_chain_guard_keeps_frontier() {
    for (text, code) in [
        (
            "fn value()->bool{match true{true if false=>true}}fn main(){}",
            "LRR-E-MATCH-NO-ARM",
        ),
        (
            "fn value()->bool{match true{true if let true = true=>true,_=>false}}fn main(){}",
            "LRR-E-UNSUPPORTED",
        ),
    ] {
        let (mut db, view) = source(text, 2024);
        let body = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "value")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut machine)) =
            RunMachine::start_with_sources(&mut db, body, 100_000, vec![view])
        else {
            panic!("guard frontier startup")
        };
        let RunPoll::Fault(error) = machine.poll(&mut db) else {
            panic!("guard frontier absent")
        };
        assert_eq!(error.code, code);
        db.finish_run().unwrap();
    }
}

#[test]
fn false_guard_kernel_cleans_exact_scopes_preserves_source_and_charges_bound_phases() {
    use lrr::{
        ir::pc::NodeId,
        runtime::cell::GuestStore,
        vm::{
            frame::Frame,
            step::{StepOutcome, step},
        },
    };
    let text = "fn main(){let input:i8=1;match input{1 if false=>{true},_=>{false}};}";
    let (mut db, view) = source(text, 2024);
    let handle = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let fetched = db
        .fetch(&SyntaxProvider, QueryKey::BodyStructure(handle.key()))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
        panic!("guard kernel index")
    };
    let index = index.as_ref().clone();
    let owner = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Match))
        .unwrap();
    let arm = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::MatchArm && n.guard_scope.is_some())
        .unwrap();
    let guard_scope = arm.guard_scope.unwrap();
    let guard_range = index.scopes()[guard_scope.0 as usize].range;
    let guard = index
        .nodes()
        .iter()
        .find(|n| n.range == guard_range && matches!(n.role, NodeRole::Expression(_)))
        .unwrap();
    let guard_ordinal = owner
        .operands
        .iter()
        .position(|id| *id == guard.id)
        .unwrap();
    let join_pc = index
        .pc(
            owner.id,
            Phase::Operand(u16::try_from(guard_ordinal).unwrap()),
        )
        .unwrap();
    let mut pc = index.pc(NodeId(0), Phase::Entry).unwrap();
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let mut frame = Frame::new(index.clone(), pc.clone(), &mut store).unwrap();
    frame.set_fuel_limit(1000).unwrap();
    let fetch = |db: &mut Database, pc| {
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(pc))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!("guard kernel thunk")
        };
        recipe.as_ref().clone()
    };
    for _ in 0..64 {
        if pc == join_pc {
            break;
        }
        let recipe = fetch(&mut db, pc);
        let StepOutcome::Next(next) = step(&mut frame, &mut store, &recipe).unwrap() else {
            panic!("guard kernel continuation")
        };
        pc = next;
    }
    assert_eq!(pc, join_pc);
    assert!(frame.active_scopes().contains(&guard_scope));
    assert!(frame.active_scopes().contains(&arm.scope));
    let source = index.nodes()[owner.operands[0].0 as usize].value.unwrap();
    let cell = frame.cell(source).unwrap();
    let state = (
        store.state(cell).unwrap(),
        store.location_epoch(cell).unwrap(),
    );
    let guard_cell = frame.cell(guard.value.unwrap()).unwrap();
    assert!(
        matches!(store.state(guard_cell).unwrap(), lrr::runtime::cell::CellState::Live(value) if value.kind==2 && value.payload==0)
    );
    let before = (
        frame.fuel_snapshot().used,
        frame.counters().operations_started,
        frame.counters().exits_completed,
    );
    let recipe = fetch(&mut db, pc);
    let StepOutcome::Next(next) = step(&mut frame, &mut store, &recipe).unwrap() else {
        panic!("guard decision")
    };
    assert_eq!(next, index.pc(arm.id, Phase::Exit).unwrap());
    assert!(!frame.active_scopes().contains(&guard_scope));
    assert!(frame.active_scopes().contains(&arm.scope));
    assert_eq!(frame.cell(guard.value.unwrap()).unwrap(), guard_cell);
    assert!(
        matches!(store.state(guard_cell).unwrap(), lrr::runtime::cell::CellState::Live(value) if value.kind==2 && value.payload==0)
    );
    assert_eq!(frame.fuel_snapshot().used, before.0.checked_add(2).unwrap());
    assert_eq!(
        frame.counters().operations_started,
        before.1.checked_add(1).unwrap()
    );
    assert_eq!(
        frame.counters().exits_completed,
        before.2.checked_add(1).unwrap()
    );
    assert_eq!(
        (
            store.state(cell).unwrap(),
            store.location_epoch(cell).unwrap()
        ),
        state
    );
    let before = (
        frame.fuel_snapshot().used,
        frame.counters().operations_started,
        frame.counters().exits_completed,
    );
    let cleanup_before = frame.counters().cleanup_elements_completed;
    let recipe = fetch(&mut db, next);
    let StepOutcome::Next(next) = step(&mut frame, &mut store, &recipe).unwrap() else {
        panic!("guard cleanup")
    };
    let cleaned = frame
        .counters()
        .cleanup_elements_completed
        .checked_sub(cleanup_before)
        .unwrap();
    assert_eq!(
        cleaned, 0,
        "Discard already retired the scalar guard cell before ScopeLeave"
    );
    assert!(!frame.active_scopes().contains(&arm.scope));
    assert!(!frame.active_scopes().contains(&guard_scope));
    assert_eq!(
        frame.fuel_snapshot().used,
        before
            .0
            .checked_add(3)
            .unwrap()
            .checked_add(cleaned)
            .unwrap()
    );
    assert_eq!(
        frame.counters().operations_started,
        before.1.checked_add(2).unwrap()
    );
    assert_eq!(
        frame.counters().exits_completed,
        before.2.checked_add(1).unwrap()
    );
    assert_eq!(
        (
            store.state(cell).unwrap(),
            store.location_epoch(cell).unwrap()
        ),
        state
    );
    assert!(frame.cell(guard.value.unwrap()).is_err());
    assert_eq!(next.phase, Phase::Entry.code().unwrap());
    assert_ne!(next.node, owner.id.0);
}
