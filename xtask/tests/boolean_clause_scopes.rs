#[path = "support/scope_steps.rs"]
mod scope_steps;
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
fn boolean_clause_metadata_uses_short_sibling_scopes_without_changing_retained_pattern_ancestry() {
    let text = "fn case()->i64{if true && let x=1_i64 && (x>0_i64){x}else{0_i64}}";
    let (_, _, index) = prepare(text);
    let booleans = index
        .nodes()
        .iter()
        .filter_map(|n| n.boolean_scope.map(|s| (&index.scopes()[s.0 as usize], n)))
        .collect::<Vec<_>>();
    assert_eq!(booleans.len(), 2);
    let clause = index.nodes().iter().find_map(|n| n.clause_scope).unwrap();
    assert!(
        booleans
            .iter()
            .all(|(s, n)| s.kind == ScopeKind::BooleanClause && s.node == n.id)
    );
    assert_eq!(
        booleans[0].0.parent,
        index.scopes()[clause.0 as usize].parent
    );
    assert_eq!(booleans[1].0.parent, Some(clause));
    assert_eq!(index.fingerprint(), prepare(text).2.fingerprint());
}
fn execute(text: &str, wanted: u64, wanted_leaves: usize) {
    use lrr::{
        db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
        ir::{
            op::Op,
            pc::{NodeId, Phase},
        },
        runtime::cell::{CellState, GuestStore},
        vm::{frame::Frame, step::StepOutcome},
    };
    let (inputs, _, index) = prepare(text);
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
    frame.set_fuel_limit(4000).unwrap();
    let mut leaves = 0;
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
        let ops = &recipe.recipe().operations;
        for op in ops {
            if let Op::ScopeLeave { scope } = op
                && index.scopes()[scope.0 as usize].kind == ScopeKind::BooleanClause
            {
                assert_eq!(frame.active_scopes().last(), Some(scope));
                leaves += 1;
                for node in index
                    .nodes()
                    .iter()
                    .filter(|n| n.scope == *scope && n.boolean_scope != Some(*scope))
                {
                    if let Some(value) = node.value
                        && let Ok(cell) = frame.cell(value)
                    {
                        retired.push(cell);
                    }
                }
            }
        }
        let result = step(&mut frame, &mut store, recipe).unwrap();
        for cell in &retired {
            assert_eq!(store.state(*cell).unwrap(), CellState::Dropped);
        }
        match result {
            StepOutcome::Return(t) => break t,
            StepOutcome::Next(_) | StepOutcome::Backedge(_) => {}
            _ => panic!("unexpected effect"),
        }
    };
    assert_eq!(leaves, wanted_leaves);
    assert_eq!(store.peek_ticket(ticket).unwrap().payload, wanted);
    frame.finish_return(&mut store).unwrap();
    store.discard(ticket).unwrap();
    db.finish_run().unwrap();
}
#[test]
fn boolean_clause_cleanup_precedes_later_clauses_and_keeps_failed_successors_cold() {
    execute(
        "fn case()->i64{if false && let x=missing(){x}else{2_i64}}",
        2,
        1,
    );
    execute(
        "fn case()->i64{if let x=1_i64 && x>0_i64 && let y=x+1_i64 && (y==2_i64){x+y}else{missing()}}",
        3,
        2,
    );
}
#[test]
fn block_and_nested_boolean_clause_results_escape_their_short_scope_before_retirement() {
    execute(
        "fn case()->i64{if {let x=1_i64;x>0_i64} && (false || true){3_i64}else{missing()}}",
        3,
        4,
    );
    execute(
        "fn case()->i64{if if true{true}else{missing()} {3_i64}else{missing()}}",
        3,
        2,
    );
}
#[test]
fn while_boolean_clause_scopes_reenter_after_each_backedge_without_stale_storage() {
    execute(
        "fn case()->i64{let mut n=0_i64;while n<3_i64 && let x=n && x>=0_i64{n=x+1_i64;}n}",
        3,
        7,
    );
}
#[test]
fn reached_runtime_panic_macro_enters_boolean_clause_scope_and_uses_normal_unwind() {
    use lrr::{
        engine::{
            run::{RunMachine, RunPoll},
            standalone_macros::StandaloneRuntimeMacros,
        },
        ir::op::Op,
    };
    let (inputs, view, index) =
        prepare("fn case()->i64{if panic!(\"boolean panic\"){1_i64}else{2_i64}}");
    let macros = StandaloneRuntimeMacros::new(&view).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, index.body().clone(), 4000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let RunPoll::Panicked(report) =
        run.poll_with_runtime_macros(&mut db, &[macros.context(&view).unwrap()])
    else {
        panic!("macro panic missing")
    };
    assert_eq!(report.panic.description(), "boolean panic");
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 4);
    let boolean = index.nodes().iter().find_map(|n| n.boolean_scope).unwrap();
    assert_eq!(
        run.installed_recipes()
            .recipes()
            .filter(|r| r
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op,Op::ScopeEnter{scope} if *scope==boolean)))
            .count(),
        1
    );
    db.finish_run().unwrap();
}
#[test]
fn runtime_macro_predicate_call_keeps_verified_seal_and_typed_nonboolean_result_fault() {
    use lrr::{
        engine::{
            run::{RunMachine, RunPoll},
            standalone_macros::StandaloneRuntimeMacros,
        },
        ir::op::Op,
    };
    let (inputs, view, index) = prepare(
        "fn case()->i64{if assert!(predicate()){1_i64}else{2_i64}} fn predicate()->bool{true}",
    );
    let macros = StandaloneRuntimeMacros::new(&view).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, index.body().clone(), 4000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let RunPoll::Fault(error) =
        run.poll_with_runtime_macros(&mut db, &[macros.context(&view).unwrap()])
    else {
        panic!("unit assertion became boolean")
    };
    assert_eq!(error.code, "LRR-E-OWNERSHIP", "{error:?}");
    assert_eq!(run.action_snapshot().published, 1);
    assert_eq!(run.action_snapshot().consumed, 1);
    let boolean = index.nodes().iter().find_map(|n| n.boolean_scope).unwrap();
    assert!(run.installed_recipes().recipes().any(|r|matches!(r.recipe().operations.as_ref(),[Op::ScopeEnter{scope},Op::SetHint{..},Op::CallSeal{..}] if *scope==boolean)));
    db.finish_run().unwrap();
}

#[test]
fn boolean_clause_metadata_budget_refuses_without_changing_repeated_body_identity() {
    let text = "fn case()->i64{if true && let x=1_i64 && x>0_i64{x}else{0_i64}}";
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
