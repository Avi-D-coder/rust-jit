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
fn pattern_clause_scope_metadata_nests_in_written_order_and_preserves_else_ancestry() {
    let text = "fn case()->i64{if let x=1_i64 && x>0_i64 && let y=x+1_i64{x+y}else{0_i64}}";
    let (_, _, index) = prepare(text);
    let clauses = index
        .nodes()
        .iter()
        .filter_map(|n| n.clause_scope.map(|s| (&index.scopes()[s.0 as usize], n)))
        .collect::<Vec<_>>();
    assert_eq!(clauses.len(), 2);
    assert!(
        clauses
            .iter()
            .all(|(s, n)| s.kind == ScopeKind::Clause && s.node == n.id && n.scope == s.id)
    );
    assert_eq!(clauses[1].0.parent, Some(clauses[0].0.id));
    assert_eq!(
        index.scopes()[clauses[0].0.parent.unwrap().0 as usize].kind,
        ScopeKind::Conditional
    );
    assert_eq!(index.fingerprint(), prepare(text).2.fingerprint());
}
fn execute(text: &str, expected: u64, expected_prefix: usize) {
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
    let mut sources = Vec::new();
    let mut drained = false;
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
            if let Op::BindLocal { source, .. } = op {
                let cell = frame.cell(*source).unwrap();
                assert!(!sources.contains(&cell));
                sources.push(cell);
            }
        }
        let conditional = ops.iter().find_map(|op| {
            if let Op::ScopeLeave { scope } = op {
                (index.scopes()[scope.0 as usize].kind == ScopeKind::Conditional).then_some(*scope)
            } else {
                None
            }
        });
        if conditional.is_some() {
            assert_eq!(
                frame
                    .active_scopes()
                    .iter()
                    .filter(|s| index.scopes()[s.0 as usize].kind == ScopeKind::Clause)
                    .count(),
                expected_prefix
            );
            for cell in &sources {
                assert_eq!(store.state(*cell).unwrap(), CellState::Moved);
            }
        }
        let result = step(&mut frame, &mut store, recipe).unwrap();
        if conditional.is_some() {
            drained = true;
            assert!(
                !frame
                    .active_scopes()
                    .iter()
                    .any(|s| index.scopes()[s.0 as usize].kind == ScopeKind::Clause)
            );
            for cell in &sources {
                assert_eq!(store.state(*cell).unwrap(), CellState::Dropped);
            }
        }
        match result {
            StepOutcome::Return(t) => break t,
            StepOutcome::Next(_) => {}
            _ => panic!("unexpected effect"),
        }
    };
    assert!(drained);
    assert_eq!(store.peek_ticket(ticket).unwrap().payload, expected);
    frame.finish_return(&mut store).unwrap();
    store.discard(ticket).unwrap();
    db.finish_run().unwrap();
}
#[test]
fn failed_clause_prefix_drains_before_else_without_entering_skipped_pattern() {
    execute(
        "fn case()->i64{if let x=1_i64 && false && let y=missing(){x+y}else{2_i64}}",
        2,
        1,
    );
    execute(
        "fn case()->i64{if let x=1_i64 && let true=false && let y=missing(){x+y}else{2_i64}}",
        2,
        2,
    );
}
#[test]
fn successful_clause_storage_survives_consequent_and_retires_after_result_transfer() {
    execute(
        "fn case()->i64{if let x=1_i64 && let y=x+1_i64{x+y}else{missing()}}",
        3,
        2,
    );
}
#[test]
fn while_pattern_clause_cells_reincarnate_after_continue_and_break_cleanup() {
    use lrr::{
        db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
        ir::{
            op::Op,
            pc::{NodeId, Phase},
        },
        runtime::cell::{CellState, GuestStore},
        vm::{frame::Frame, step::StepOutcome},
    };
    let (inputs, _, index) = prepare(
        "fn case()->i64{let mut n=0_i64;while let x=n{n=x+1_i64;if n<3_i64{continue;}break;}n}",
    );
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
    let mut sources = Vec::new();
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
            if let Op::BindLocal {
                binding, source, ..
            } = op
                && index.scopes()[index.nodes()[binding.0 as usize].scope.0 as usize].kind
                    == ScopeKind::Clause
            {
                let cell = frame.cell(*source).unwrap();
                assert!(!sources.contains(&cell));
                for old in &sources {
                    assert_eq!(store.state(*old).unwrap(), CellState::Dropped);
                }
                sources.push(cell);
            }
        }
        match step(&mut frame, &mut store, recipe).unwrap() {
            StepOutcome::Return(t) => break t,
            StepOutcome::Next(_) | StepOutcome::Backedge(_) => {}
            _ => panic!("unexpected effect"),
        }
    };
    assert_eq!(sources.len(), 3);
    for old in &sources {
        assert_eq!(store.state(*old).unwrap(), CellState::Dropped);
    }
    assert_eq!(store.peek_ticket(ticket).unwrap().payload, 3);
    frame.finish_return(&mut store).unwrap();
    store.discard(ticket).unwrap();
    db.finish_run().unwrap();
}

#[test]
fn pattern_clause_metadata_budget_refuses_without_changing_repeated_body_identity() {
    let text = "fn case()->i64{if let x=1_i64 && let y=x+1_i64{x+y}else{0_i64}}";
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
