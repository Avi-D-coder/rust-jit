#[path = "support/scope_steps.rs"]
mod scope_steps;
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{BodyIndex, BodyIndexLimits, ScopeKind},
        parse::{ParseKey, ParsedFile},
    },
};
use scope_steps::finish_recipe as step;
fn prepare(text: &str) -> (PreparedSourceInputs, PreparedSourceView, BodyIndex) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([220; 32]);
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let parsed = ParsedFile::parse(&blob, profile.clone()).unwrap();
    let outline = parsed.outline(unit, vec![], &cfg).unwrap();
    let body = outline.items[0].body.as_ref().unwrap();
    let index = parsed.index_body(body, BodyIndexLimits::default()).unwrap();
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    (inputs, view, index)
}
#[test]
fn ordinary_lazy_operands_have_sibling_boolean_scopes_without_wrapping_pattern_aggregators() {
    let (_, _, index) = prepare("fn case()->bool{false || (true && false)}");
    assert_eq!(
        index
            .nodes()
            .iter()
            .filter(|n| n.boolean_scope.is_some())
            .count(),
        4
    );
    for n in index.nodes().iter().filter(|n| n.boolean_scope.is_some()) {
        let scope = &index.scopes()[n.boolean_scope.unwrap().0 as usize];
        assert_eq!(scope.kind, ScopeKind::BooleanClause);
        assert_eq!(scope.node, n.id);
    }
    let (_, _, chain) =
        prepare("fn case()->i64{if let x=1_i64 && let y=x+1_i64 && x<y{x+y}else{0_i64}}");
    assert_eq!(
        chain
            .nodes()
            .iter()
            .filter(|n| n.clause_scope.is_some())
            .count(),
        2
    );
    assert_eq!(
        chain
            .nodes()
            .iter()
            .filter(|n| n.boolean_scope.is_some())
            .count(),
        1
    );
}
fn execute(text: &str, wanted: bool, leaves: usize) {
    use lrr::{
        db::{
            key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider,
            value::QueryValue,
        },
        ir::{
            op::Op,
            pc::{NodeId, Phase},
        },
        runtime::cell::{CellState, GuestStore},
        vm::{frame::Frame, step::StepOutcome},
    };
    let (inputs, view, index) = prepare(text);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let mut store = GuestStore::new(1, 1000, 1_048_576).unwrap();
    let mut frame = Frame::new(
        index.clone(),
        index.pc(NodeId(0), Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    frame.set_fuel_limit(2000).unwrap();
    let mut count = 0;
    let mut retired = Vec::new();
    let ticket = loop {
        let QueryPoll::Ready(Ok(outcome)) =
            db.fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
        else {
            panic!("recipe absent")
        };
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("wrong recipe")
        };
        for op in &recipe.recipe().operations {
            if let Op::ScopeLeave { scope } = op
                && index.scopes()[scope.0 as usize].kind == ScopeKind::BooleanClause
            {
                assert_eq!(frame.active_scopes().last(), Some(scope));
                count += 1;
                for n in index
                    .nodes()
                    .iter()
                    .filter(|n| n.scope == *scope && n.boolean_scope != Some(*scope))
                {
                    if let Some(slot) = n.value
                        && let Ok(cell) = frame.cell(slot)
                    {
                        retired.push(cell);
                    }
                }
            }
        }
        let result = step(&mut frame, &mut store, recipe).unwrap();
        for old in &retired {
            assert_eq!(store.state(*old).unwrap(), CellState::Dropped);
        }
        match result {
            StepOutcome::Return(t) => break t,
            StepOutcome::Next(_) => {}
            _ => panic!("unexpected effect"),
        }
    };
    assert_eq!(count, leaves);
    assert_eq!(
        store.peek_ticket(ticket).unwrap().payload,
        u64::from(wanted)
    );
    let offset = view.outline().items[0].body.as_ref().unwrap().range.start as usize;
    for node in index.nodes().iter().filter(|n| {
        &text[offset + n.range.start as usize..offset + n.range.end as usize] == "missing()"
    }) {
        assert!(
            db.memo(&QueryKey::Thunk(index.pc(node.id, Phase::Entry).unwrap()))
                .is_none()
        );
    }
    frame.finish_return(&mut store).unwrap();
    store.discard(ticket).unwrap();
    db.finish_run().unwrap();
}
#[test]
fn lazy_and_or_cleanup_preserves_result_and_leaves_skipped_calls_unprepared() {
    execute("fn case()->bool{false && missing()}", false, 1);
    execute("fn case()->bool{true || missing()}", true, 1);
    execute("fn case()->bool{true && ({let x=1_i64;x>0_i64})}", true, 2);
    execute("fn case()->bool{false || (true && false)}", false, 4);
}
#[test]
fn nested_lazy_conditions_preserve_retained_pattern_bindings_and_actual_state_effects() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll, ValueSummary},
    };
    let (inputs, view, index) = prepare(
        "fn case()->i64{let mut n=0_i64;if let x=1_i64 && ({n=n+1_i64;x>0_i64} || missing()) && let y=x+1_i64{x+y+n}else{missing()}}",
    );
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, index.body().clone(), 4000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("nested condition failed")
    };
    assert!(matches!(report.value,ValueSummary::Signed(ref n) if n=="4"));
    assert_eq!(run.action_snapshot().published, 0);
    db.finish_run().unwrap();
}

#[test]
fn lazy_operand_scope_budget_refuses_without_changing_repeated_body_identity() {
    let text = "fn case()->bool{true && (false || true)}";
    let (_, _, index) = prepare(text);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([220; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline.items[0].body.as_ref().unwrap();
    assert_eq!(
        parsed
            .index_body(
                body,
                BodyIndexLimits {
                    metadata_bytes: index.estimated_bytes() - 1,
                    ..Default::default()
                }
            )
            .unwrap_err()
            .code,
        "LRR-E-BODY-INDEX-LIMIT"
    );
    assert_eq!(index.fingerprint(), prepare(text).2.fingerprint());
}
#[test]
fn reached_let_outside_condition_refuses_before_scrutinee_or_binding_execution() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll},
    };
    let (inputs, view, index) = prepare("fn case()->bool{true && let x=1_i64}");
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, index.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("misplaced let executed")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert_eq!(
        error.message,
        "let pattern requires a top-level if/while condition clause"
    );
    let counters = run.opcode_counters().unwrap();
    assert_eq!(counters.started_by_opcode[0], 1);
    assert_eq!(counters.started_by_opcode[7], 0);
    assert_eq!(run.action_snapshot().published, 0);
    db.finish_run().unwrap();
}
