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
fn statement_scope_metadata_separates_initializer_temporaries_from_block_bindings() {
    use lrr::syntax::body_index::NodeRole;
    let text = "fn case()->i64{let x=1_i64;x+1_i64; x}";
    let (_, _, index) = prepare(text);
    let statements = index
        .nodes()
        .iter()
        .filter(|n| n.statement_scope.is_some())
        .collect::<Vec<_>>();
    assert_eq!(statements.len(), 2);
    for statement in statements {
        let scope = statement.statement_scope.unwrap();
        assert_eq!(index.scopes()[scope.0 as usize].kind, ScopeKind::Statement);
        assert_eq!(
            index.scopes()[scope.0 as usize].parent,
            Some(statement.scope)
        );
    }
    let binding = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Pattern)
        .unwrap();
    assert_eq!(binding.scope, index.nodes()[0].scope);
    assert_eq!(index.fingerprint(), prepare(text).2.fingerprint());
}
#[test]
fn actual_statement_cleanup_retires_moved_initializer_before_following_binding_read() {
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
    let (inputs, _, index) = prepare("fn case()->i64{let x=1_i64;x+1_i64;x}");
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
    let mut original = None;
    let mut retired = false;
    let mut leaves = 0;
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
            if let Op::BindLocal { source, .. } = op {
                original = Some(frame.cell(*source).unwrap());
            }
            if matches!(op, Op::ReadLocal { .. }) {
                assert!(retired);
            }
        }
        let leaving=recipe.recipe().operations.iter().any(|op|matches!(op,Op::ScopeLeave{scope} if index.scopes()[scope.0 as usize].kind==ScopeKind::Statement));
        let result = step(&mut frame, &mut store, recipe).unwrap();
        if leaving {
            leaves += 1;
            assert_eq!(store.state(original.unwrap()).unwrap(), CellState::Dropped);
            retired = true;
        }
        match result {
            StepOutcome::Return(t) => break t,
            StepOutcome::Next(_) => {}
            _ => panic!("unexpected effect"),
        }
    };
    assert_eq!(leaves, 2);
    assert_eq!(store.peek_ticket(ticket).unwrap().payload, 1);
    frame.finish_return(&mut store).unwrap();
    store.discard(ticket).unwrap();
    db.finish_run().unwrap();
}
#[test]
fn statement_scopes_follow_return_continue_break_and_leave_cold_statements_unentered() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll, ValueSummary},
    };
    for text in [
        "fn case()->i64{return 3_i64;missing();}",
        "fn case()->i64{let mut n=0_i64;while n<3_i64{n=n+1_i64;continue;}n}",
        "fn case()->i64{let n=loop{let x=3_i64;break x;};n}",
    ] {
        let (inputs, view, index) = prepare(text);
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut run)) =
            RunMachine::start_with_sources(&mut db, index.body().clone(), 4000, vec![view.share()])
        else {
            panic!("run absent")
        };
        let RunPoll::Completed(report) = run.poll(&mut db) else {
            panic!("statement control failed")
        };
        assert!(matches!(report.value,ValueSummary::Signed(ref n) if n=="3"));
        if text.contains("missing()") {
            let offset = view.outline().items[0].body.as_ref().unwrap().range.start as usize;
            let cold = index
                .nodes()
                .iter()
                .find(|n| {
                    n.statement_scope.is_some()
                        && &text[offset + n.range.start as usize..offset + n.range.end as usize]
                            == "missing();"
                })
                .unwrap();
            assert!(
                db.memo(&lrr::db::key::QueryKey::Thunk(
                    index.pc(cold.id, lrr::ir::pc::Phase::Entry).unwrap()
                ))
                .is_none()
            );
            assert!(!run.installed_recipes().recipes().any(|r|r.recipe().operations.iter().any(|op|matches!(op,lrr::ir::op::Op::ScopeEnter{scope} if Some(*scope)==cold.statement_scope))));
        }
        db.finish_run().unwrap();
    }
}

#[test]
fn statement_scope_metadata_budget_refuses_without_changing_repeated_body_identity() {
    let text = "fn case()->i64{let x=1_i64;x+1_i64;x}";
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
