#[path = "support/scope_steps.rs"]
mod scope_steps;
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database, poll::QueryPoll},
    engine::run::{RunMachine, RunPoll, ValueSummary},
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
fn block_tail_scopes_are_deterministic_bounded_metadata_with_block_parents() {
    let text = "fn case()->i64{let local=1_i64;{let inner=2_i64;local+inner}}";
    let (_, _, index) = prepare(text);
    assert_eq!(
        index
            .scopes()
            .iter()
            .filter(|s| s.kind == ScopeKind::Tail)
            .count(),
        2
    );
    for node in index.nodes() {
        if let Some(tail) = node.tail_scope {
            let scope = &index.scopes()[tail.0 as usize];
            assert_eq!(scope.kind, ScopeKind::Tail);
            assert_eq!(scope.parent, Some(node.scope));
            assert_eq!(scope.node, node.id);
        }
    }
    assert_eq!(index.fingerprint(), prepare(text).2.fingerprint());
}
#[test]
fn actual_nested_tail_result_transfers_before_tail_and_block_scope_retirement() {
    use lrr::{
        db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
        ir::{
            op::Op,
            pc::{NodeId, Phase},
        },
        runtime::cell::GuestStore,
        vm::{frame::Frame, step::StepOutcome},
    };
    let (inputs, _, index) =
        prepare("fn case()->i64{let local=1_i64;{let inner=2_i64;local+inner}}");
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
        let operations = &recipe.recipe().operations;
        for (position, op) in operations.iter().enumerate() {
            if let Op::ScopeLeave { scope } = op
                && index.scopes()[scope.0 as usize].kind == ScopeKind::Tail
            {
                assert!(
                    operations[..position]
                        .iter()
                        .any(|op| matches!(op, Op::ResultTransfer { .. }))
                );
            }
        }
        let result = step(&mut frame, &mut store, recipe).unwrap();
        retired.extend(operations.iter().filter_map(|op| {
            if let Op::ScopeLeave { scope } = op {
                Some(*scope)
            } else {
                None
            }
        }));
        match result {
            StepOutcome::Return(ticket) => break ticket,
            StepOutcome::Next(_) | StepOutcome::Backedge(_) => {}
            _ => panic!("unexpected effect"),
        }
    };
    let kinds = retired
        .iter()
        .map(|s| index.scopes()[s.0 as usize].kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            ScopeKind::Statement,
            ScopeKind::Statement,
            ScopeKind::Tail,
            ScopeKind::Block,
            ScopeKind::Tail
        ]
    );
    assert_eq!(store.peek_ticket(ticket).unwrap().payload, 3);
    frame.finish_return(&mut store).unwrap();
    assert!(frame.active_scopes().is_empty());
    assert_eq!(store.peek_ticket(ticket).unwrap().payload, 3);
    store.discard(ticket).unwrap();
    db.finish_run().unwrap();
}
#[test]
fn tail_control_transfers_preserve_loop_result_and_panic_scope_cleanup() {
    for text in [
        "fn case()->i64{let mut n=0_i64;loop{n=n+1_i64;if n<3_i64{continue;}break n;}}",
        "fn case()->i64{let local=1_i64;{return local+2_i64;}}",
    ] {
        let (inputs, view, index) = prepare(text);
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut run)) =
            RunMachine::start_with_sources(&mut db, index.body().clone(), 1000, vec![view.share()])
        else {
            panic!("run absent")
        };
        let RunPoll::Completed(report) = run.poll(&mut db) else {
            panic!("tail control failed")
        };
        assert!(matches!(report.value,ValueSummary::Signed(ref n) if n=="3"));
        db.finish_run().unwrap();
    }
    let (inputs, view, index) = prepare("fn case()->i8{let local=1_i64;127_i8+1_i8}");
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, index.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let RunPoll::Panicked(report) = run.poll(&mut db) else {
        panic!("overflow absent")
    };
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 2);
    db.finish_run().unwrap();
}

#[test]
fn cold_tail_scopes_remain_unentered_and_opaque_async_bodies_are_not_inspected() {
    use lrr::{
        ir::op::Op,
        syntax::body_index::{ExpressionKind, NodeRole},
    };
    let text = "fn case()->i64{if true{7_i64}else{async{missing()};9_i64}}";
    let (inputs, view, index) = prepare(text);
    let offset = view.outline().items[0].body.as_ref().unwrap().range.start as usize;
    let async_node = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Async))
        .unwrap();
    assert!(async_node.opaque);
    assert!(async_node.tail_scope.is_none());
    let cold = index
        .nodes()
        .iter()
        .find(|n| {
            n.id.0 != 0
                && n.role == NodeRole::Expression(ExpressionKind::Block)
                && text[offset + n.range.start as usize..offset + n.range.end as usize]
                    .contains("async")
        })
        .unwrap()
        .tail_scope
        .unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, index.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("cold tail was forced")
    };
    assert!(matches!(report.value,ValueSummary::Signed(ref n) if n=="7"));
    assert!(!run.installed_recipes().recipes().any(|r| {
        r.recipe()
            .operations
            .iter()
            .any(|op| matches!(op,Op::ScopeEnter{scope} if *scope==cold))
    }));
    db.finish_run().unwrap();
}
#[test]
fn tail_scope_metadata_budget_refuses_without_changing_repeated_body_identity() {
    let text = "fn case()->i64{let local=1_i64;{let inner=2_i64;local+inner}}";
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
    let limits = BodyIndexLimits {
        metadata_bytes: index.estimated_bytes() - 1,
        ..BodyIndexLimits::default()
    };
    assert_eq!(
        parsed.index_body(body, limits).unwrap_err().code,
        "LRR-E-BODY-INDEX-LIMIT"
    );
    assert_eq!(
        parsed
            .index_body(body, BodyIndexLimits::default())
            .unwrap()
            .fingerprint(),
        index.fingerprint()
    );
}
