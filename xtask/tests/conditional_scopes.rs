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
        body_index::{BodyIndex, BodyIndexLimits, ExpressionKind, NodeRole, ScopeKind},
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
fn conditional_scope_metadata_separates_consequent_from_else_and_is_deterministic() {
    let text = "fn case()->i64{if let true=false{1_i64}else{2_i64}}";
    let (_, _, index) = prepare(text);
    let node = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::If))
        .unwrap();
    let condition = node.condition_scope.unwrap();
    let scope = &index.scopes()[condition.0 as usize];
    assert_eq!(scope.kind, ScopeKind::Conditional);
    assert_eq!(scope.parent, Some(node.scope));
    let operands = &node.operands;
    let clause = index.nodes()[operands[0].0 as usize].clause_scope.unwrap();
    assert_eq!(index.scopes()[clause.0 as usize].parent, Some(condition));
    assert_eq!(index.nodes()[operands[0].0 as usize].scope, clause);
    assert_eq!(
        index.scopes()[index.nodes()[operands[1].0 as usize].scope.0 as usize].parent,
        Some(clause)
    );
    assert_eq!(
        index.scopes()[index.nodes()[operands[2].0 as usize].scope.0 as usize].parent,
        Some(node.scope)
    );
    assert_eq!(index.fingerprint(), prepare(text).2.fingerprint());
}
fn execute(text: &str) -> (u64, Vec<(bool, ScopeKind)>) {
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
    frame.set_fuel_limit(2000).unwrap();
    let mut events = Vec::new();
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
                && index.scopes()[scope.0 as usize].kind == ScopeKind::Conditional
            {
                let owner = &index.nodes()[index.scopes()[scope.0 as usize].node.0 as usize];
                if owner.role == NodeRole::Expression(ExpressionKind::If)
                    && recipe.recipe().key.phase == Phase::Operand(1).code().unwrap()
                {
                    assert!(
                        operations[..position]
                            .iter()
                            .any(|op| matches!(op, Op::ResultTransfer { .. }))
                    );
                }
                let slot = index.nodes()[owner.operands[0].0 as usize].value.unwrap();
                let cell = frame.cell(slot).unwrap();
                assert!(!retired.contains(&cell));
                retired.push(cell);
            }
        }
        let result = step(&mut frame, &mut store, recipe).unwrap();
        for cell in &retired {
            assert_eq!(store.state(*cell).unwrap(), CellState::Dropped);
        }
        events.extend(operations.iter().filter_map(|op| match op {
            Op::ScopeEnter { scope } => Some((true, index.scopes()[scope.0 as usize].kind)),
            Op::ScopeLeave { scope } => Some((false, index.scopes()[scope.0 as usize].kind)),
            _ => None,
        }));
        match result {
            StepOutcome::Return(t) => break t,
            StepOutcome::Next(_) | StepOutcome::Backedge(_) => {}
            _ => panic!("unexpected effect"),
        }
    };
    let value = store.peek_ticket(ticket).unwrap().payload;
    frame.finish_return(&mut store).unwrap();
    store.discard(ticket).unwrap();
    db.finish_run().unwrap();
    (value, events)
}
#[test]
fn failed_if_condition_retires_before_else_scope_entry_in_actual_vm() {
    let (value, events) = execute("fn case()->i64{if let true=false{missing()}else{2_i64}}");
    assert_eq!(value, 2);
    let condition_exit = events
        .iter()
        .position(|e| *e == (false, ScopeKind::Conditional))
        .unwrap();
    assert_eq!(events[condition_exit + 1], (true, ScopeKind::Block));
    assert_eq!(
        events
            .iter()
            .filter(|e| **e == (true, ScopeKind::Conditional))
            .count(),
        1
    );
}
#[test]
fn successful_if_retains_condition_until_consequent_result_transfer_in_actual_vm() {
    let (value, events) = execute("fn case()->i64{if let true=true{3_i64}else{missing()}}");
    assert_eq!(value, 3);
    let condition_exit = events
        .iter()
        .position(|e| *e == (false, ScopeKind::Conditional))
        .unwrap();
    assert_eq!(events[condition_exit - 1], (false, ScopeKind::Block));
    assert_eq!(
        events
            .iter()
            .filter(|e| **e == (false, ScopeKind::Conditional))
            .count(),
        1
    );
}
#[test]
fn while_conditions_reenter_fresh_storage_and_retire_before_each_backedge() {
    let (value, events) =
        execute("fn case()->i64{let mut n=0_i64;while n<3_i64 && let true=true{n=n+1_i64;}n}");
    assert_eq!(value, 3);
    assert_eq!(
        events
            .iter()
            .filter(|e| **e == (true, ScopeKind::Conditional))
            .count(),
        4
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| **e == (false, ScopeKind::Conditional))
            .count(),
        4
    );
    for (i, event) in events.iter().enumerate() {
        if *event == (false, ScopeKind::Conditional) {
            assert_eq!(events[i + 1], (false, ScopeKind::Loop));
        }
    }
}

#[test]
fn conditional_scope_metadata_budget_refuses_without_changing_repeated_body_identity() {
    let text = "fn case()->i64{if true{1_i64}else{2_i64}}";
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
#[test]
fn condition_scopes_follow_existing_continue_break_and_guest_panic_cleanup() {
    use lrr::engine::run::{RunMachine, RunPoll, ValueSummary};
    for (text, panics) in [
        (
            "fn case()->i64{let mut n=0_i64;while let true=n<4_i64{n=n+1_i64;if n<3_i64{continue;}break;}n}",
            false,
        ),
        (
            "fn case()->i8{if let true=true{127_i8+1_i8}else{0_i8}}",
            true,
        ),
    ] {
        let (inputs, view, index) = prepare(text);
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut run)) =
            RunMachine::start_with_sources(&mut db, index.body().clone(), 2000, vec![view.share()])
        else {
            panic!("run absent")
        };
        match run.poll(&mut db) {
            RunPoll::Completed(report) => {
                assert!(!panics);
                assert!(matches!(report.value,ValueSummary::Signed(ref n) if n=="3"));
            }
            RunPoll::Panicked(report) => {
                assert!(panics);
                assert_eq!(report.opcode_counters.panic_scopes_cleaned, 6);
            }
            _ => panic!("condition control failed"),
        }
        db.finish_run().unwrap();
    }
}
