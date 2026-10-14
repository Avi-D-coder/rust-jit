#[path = "support/cached_integer_fixture.rs"]
mod cached_integer_fixture;
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
fn match_guard_scope_is_nested_in_arm_and_scrutinee_retains_enclosing_boundary() {
    use lrr::syntax::body_index::{ExpressionKind, NodeRole};
    let text = "fn case()->i64{match true{true if 1_i64>0_i64=>7_i64,false=>8_i64}}";
    let (_, _, index) = prepare(text);
    let matched = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Match))
        .unwrap();
    let arm = index
        .nodes()
        .iter()
        .find(|n| n.guard_scope.is_some())
        .unwrap();
    let guard = arm.guard_scope.unwrap();
    assert_eq!(index.scopes()[guard.0 as usize].kind, ScopeKind::MatchGuard);
    assert_eq!(index.scopes()[guard.0 as usize].parent, Some(arm.scope));
    assert_eq!(
        index.scopes()[arm.scope.0 as usize].parent,
        Some(matched.scope)
    );
    assert_eq!(
        index.nodes()[matched.operands[0].0 as usize].scope,
        matched.scope
    );
    assert_eq!(index.fingerprint(), prepare(text).2.fingerprint());
}
#[test]
fn reached_pattern_match_guard_refuses_before_boolean_operand_preparation() {
    use lrr::{
        db::{
            key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider,
            value::QueryValue,
        },
        ir::pc::Phase,
        syntax::body_index::{ExpressionKind, NodeRole},
    };
    let (inputs, _, index) =
        prepare("fn case()->i64{match true{_ if false && let x=missing()=>1_i64,_=>2_i64}}");
    let guard = index
        .nodes()
        .iter()
        .find(|n| {
            n.boolean_scope
                .is_some_and(|s| index.scopes()[s.0 as usize].kind == ScopeKind::MatchGuard)
        })
        .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let error = match db.fetch(
        &SyntaxProvider,
        QueryKey::Thunk(index.pc(guard.id, Phase::Entry).unwrap()),
    ) {
        QueryPoll::Ready(Err(error)) => error,
        QueryPoll::Ready(Ok(outcome)) => {
            let QueryValue::Error(error) = outcome.value.as_ref() else {
                panic!("pattern guard prepared")
            };
            error.clone()
        }
        _ => panic!("guard input missing"),
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert_eq!(
        error.message,
        "pattern and let-chain match guards are unsupported"
    );
    for n in index
        .nodes()
        .iter()
        .filter(|n| n.role == NodeRole::Expression(ExpressionKind::Call))
    {
        assert!(
            db.memo(&QueryKey::Thunk(index.pc(n.id, Phase::Entry).unwrap()))
                .is_none()
        );
    }
    db.finish_run().unwrap();
}
#[test]
fn source_backed_guard_thunks_retire_before_arm_result_transfer_in_kernel() {
    use lrr::{
        db::{
            key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider,
            value::QueryValue,
        },
        ir::{
            op::{HintRecipe, LiteralKind, LiteralRecipe, Op},
            pc::{NodeId, Phase},
            recipe::{Origin, RecipeDraft, RecipeExit},
            validate::verify,
        },
        runtime::cell::GuestStore,
        syntax::body_index::{ExpressionKind, NodeRole},
        vm::{frame::Frame, step::StepOutcome},
    };
    let (inputs, _, index) =
        prepare("fn case()->i64{match true{true if 1_i64>0_i64=>7_i64,_=>8_i64}}");
    let matched = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Match))
        .unwrap();
    let arm = index
        .nodes()
        .iter()
        .find(|n| n.guard_scope.is_some())
        .unwrap();
    let guard_scope = arm.guard_scope.unwrap();
    let guard = index
        .nodes()
        .iter()
        .find(|n| n.boolean_scope == Some(guard_scope))
        .unwrap();
    let selected = index
        .nodes()
        .iter()
        .find(|n| {
            n.role == NodeRole::Expression(ExpressionKind::Literal) && n.parent == Some(arm.id)
        })
        .unwrap();
    let scrutinee = &index.nodes()[matched.operands[0].0 as usize];
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
    let prelude = verify(
        RecipeDraft {
            key: frame.pc().clone(),
            origin: Origin {
                span: index.span(NodeId(0)).unwrap(),
                expansion_ancestry: vec![],
            },
            operations: vec![
                Op::ScopeEnter {
                    scope: lrr::syntax::body_index::ScopeId(0),
                },
                Op::ScopeEnter {
                    scope: matched.scope,
                },
                Op::Literal {
                    literal: LiteralRecipe {
                        kind: LiteralKind::Bool,
                        spelling: "true".into(),
                        decoded: None,
                        source: index.span(scrutinee.id).unwrap(),
                    },
                    hint: HintRecipe::None,
                    destination: scrutinee.value.unwrap(),
                },
                Op::ScopeEnter { scope: arm.scope },
            ],
            exits: vec![RecipeExit::Next(index.pc(guard.id, Phase::Entry).unwrap())],
            dependencies: vec![],
        },
        &index,
    )
    .unwrap();
    step(&mut frame, &mut store, &prelude).unwrap();
    loop {
        let QueryPoll::Ready(Ok(outcome)) =
            db.fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
        else {
            panic!("guard thunk absent")
        };
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("wrong guard thunk")
        };
        let StepOutcome::Next(pc) = step(&mut frame, &mut store, recipe).unwrap() else {
            panic!("guard effect")
        };
        if pc.node == matched.id.0 {
            break;
        }
    }
    assert_eq!(frame.active_scopes().last(), Some(&guard_scope));
    assert_eq!(
        store
            .read_scalar(frame.cell(guard.value.unwrap()).unwrap())
            .unwrap()
            .payload,
        1
    );
    let join = verify(
        RecipeDraft {
            key: frame.pc().clone(),
            origin: Origin {
                span: index.span(matched.id).unwrap(),
                expansion_ancestry: vec![],
            },
            operations: vec![
                Op::ScopeLeave { scope: guard_scope },
                Op::Literal {
                    literal: LiteralRecipe {
                        kind: LiteralKind::Integer,
                        spelling: "7_i64".into(),
                        decoded: Some(cached_integer_fixture::integer("7_i64")),
                        source: index.span(selected.id).unwrap(),
                    },
                    hint: HintRecipe::None,
                    destination: selected.value.unwrap(),
                },
                Op::ResultTransfer {
                    source: selected.value.unwrap(),
                    destination: matched.value.unwrap(),
                },
                Op::ScopeLeave { scope: arm.scope },
            ],
            exits: vec![RecipeExit::Next(
                index.pc(NodeId(0), Phase::Commit).unwrap(),
            )],
            dependencies: vec![],
        },
        &index,
    )
    .unwrap();
    step(&mut frame, &mut store, &join).unwrap();
    assert_eq!(
        store
            .read_scalar(frame.cell(matched.value.unwrap()).unwrap())
            .unwrap()
            .payload,
        7
    );
    assert_eq!(
        store
            .read_scalar(frame.cell(scrutinee.value.unwrap()).unwrap())
            .unwrap()
            .payload,
        1
    );
    assert!(!frame.active_scopes().contains(&arm.scope));
    db.finish_run().unwrap();
}
