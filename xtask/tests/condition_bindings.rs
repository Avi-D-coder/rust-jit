use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database, poll::QueryPoll},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{BodyIndex, BodyIndexLimits},
        parse::{ParseKey, ParsedFile},
    },
};
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
fn run(text: &str) -> lrr::engine::run::RunPoll {
    use lrr::engine::run::RunMachine;
    let (inputs, view, index) = prepare(text);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, index.body().clone(), 4000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let outcome = run.poll(&mut db);
    db.finish_run().unwrap();
    outcome
}
fn value(text: &str, wanted: &str) {
    use lrr::engine::run::{RunPoll, ValueSummary};
    match run(text) {
        RunPoll::Completed(report) => {
            assert!(matches!(report.value,ValueSummary::Signed(ref n) if n==wanted))
        }
        RunPoll::Fault(e) => panic!("condition binding fault: {e:?}"),
        _ => panic!("condition binding did not complete"),
    }
}
#[test]
fn condition_binding_commits_once_and_is_visible_to_later_clauses_and_consequent() {
    value(
        "fn case()->i64{let mut n=0_i64;if let x={n=n+1_i64;3_i64} && let y=x+1_i64 && y==4_i64{x+y+n}else{missing()}}",
        "8",
    );
}
#[test]
fn condition_binding_shadowing_excludes_own_scrutinee_else_and_following_statements() {
    value(
        "fn case()->i64{let x=9_i64;let y=if let x=x+1_i64 && false{x}else{x};x+y}",
        "18",
    );
    value(
        "fn case()->i64{let x=9_i64;if let x=x+1_i64{x}else{missing()}}",
        "10",
    );
    value(
        "fn case()->i64{let x=9_i64;if let x=3_i64{let x=4_i64;x}else{missing()}}",
        "4",
    );
}
#[test]
fn mutable_while_condition_bindings_recreate_storage_across_continue_and_break() {
    value(
        "fn case()->i64{let mut n=0_i64;while let mut x=n{x=x+1_i64;n=x;if n<3_i64{continue;}break;}n}",
        "3",
    );
}
#[test]
fn wildcard_condition_patterns_evaluate_once_without_binding_or_copying_source() {
    value(
        "fn case()->i64{let mut n=0_i64;if let _={n=n+1_i64;3_i64} && let x=n{x}else{missing()}}",
        "1",
    );
}
#[test]
fn condition_bindings_do_not_leak_into_else_following_statements_or_own_scrutinee() {
    use lrr::engine::run::RunPoll;
    for text in [
        "fn case()->i64{if let x=1_i64 && false{0_i64}else{x}}",
        "fn case()->i64{if let x=1_i64{}x}",
        "fn case()->i64{if let x=x{x}else{0_i64}}",
    ] {
        let RunPoll::Fault(error) = run(text) else {
            panic!("binding leaked")
        };
        assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    }
}
#[test]
fn irrefutable_condition_test_preserves_scalar_source_until_move_commit() {
    use lrr::{
        db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
        ir::{
            op::Op,
            pc::{NodeId, Phase},
        },
        runtime::cell::{CellState, GuestStore},
        vm::{
            frame::Frame,
            step::{StepOutcome, step},
        },
    };
    let (inputs, _, index) = prepare("fn case()->i64{if let bound=3_i64{bound}else{missing()}}");
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
    frame.set_fuel_limit(1000).unwrap();
    let literal = index
        .nodes()
        .iter()
        .find(|n| {
            n.role
                == lrr::syntax::body_index::NodeRole::Expression(
                    lrr::syntax::body_index::ExpressionKind::Literal,
                )
        })
        .unwrap();
    let mut tested = false;
    let mut committed = false;
    let mut original = None;
    let ticket = loop {
        let QueryPoll::Ready(Ok(outcome)) =
            db.fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
        else {
            panic!("recipe absent")
        };
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("wrong recipe")
        };
        let ops = &recipe.recipe().operations;
        if let Some(Op::Pattern {
            operation: lrr::ir::op::PatternOperation::StageBinding { source, .. },
        }) = ops.iter().find(|op| {
            matches!(
                op,
                Op::Pattern {
                    operation: lrr::ir::op::PatternOperation::StageBinding { .. }
                }
            )
        }) {
            assert!(tested);
            let cell = frame.cell(*source).unwrap();
            assert!(matches!(store.state(cell).unwrap(), CellState::Live(_)));
            original = Some(cell);
        }
        let result = step(&mut frame, &mut store, recipe).unwrap();
        if ops.iter().any(|op| {
            matches!(
                op,
                Op::Pattern {
                    operation: lrr::ir::op::PatternOperation::Finish { matched: None, .. }
                }
            )
        }) {
            assert_eq!(store.state(original.unwrap()).unwrap(), CellState::Moved);
            committed = true;
        }
        if recipe.recipe().key.node != 0
            && recipe.recipe().key.phase >= 3
            && matches!(&recipe.recipe().exit,lrr::ir::recipe::RecipeExit::Next(pc) if pc.node==recipe.recipe().key.node && pc.phase==Phase::Commit.code().unwrap())
            && index.nodes()[recipe.recipe().key.node as usize].role
                == lrr::syntax::body_index::NodeRole::Expression(
                    lrr::syntax::body_index::ExpressionKind::Other,
                )
        {
            assert!(!committed);
            assert!(matches!(
                store
                    .state(frame.cell(literal.value.unwrap()).unwrap())
                    .unwrap(),
                CellState::Live(_)
            ));
            assert!(
                !ops.iter()
                    .any(|op| matches!(op, Op::BindLocal { .. } | Op::Discard { .. }))
            );
            tested = true;
        }
        match result {
            StepOutcome::Return(t) => break t,
            StepOutcome::Next(_) => {}
            _ => panic!("unexpected effect"),
        }
    };
    assert!(tested && committed);
    assert_eq!(store.peek_ticket(ticket).unwrap().payload, 3);
    frame.finish_return(&mut store).unwrap();
    store.discard(ticket).unwrap();
    db.finish_run().unwrap();
}

#[test]
fn owning_condition_source_keeps_typed_value_milestone_refusal() {
    let lrr::engine::run::RunPoll::Fault(error) =
        run("fn case()->i64{if let owned=\"owned\"{3_i64}else{missing()}}")
    else {
        panic!("unsupported source falsely accepted")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert_eq!(
        error.message,
        "literal kind awaits numeric/source integration"
    );
}
