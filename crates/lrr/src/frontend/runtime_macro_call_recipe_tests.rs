//! Source/recipe authority controls only; no guest execution or stock-main pass.
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        value::QueryValue,
    },
    engine::standalone_macros::StandaloneRuntimeMacros,
    ir::{
        op::Op,
        pc::{NodeId, Phase},
        recipe::{Recipe, RecipeDraft, RecipeExit},
        validate::{verify, verify_runtime_scalar_call},
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        parse::ParseKey,
        runtime_macro_scalars::RuntimeMacroCall,
    },
};
use std::{collections::BTreeSet, sync::Arc};
fn setup(text: &str, edition: u16) -> (Database, PreparedSourceView, BodyIndex) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("assertion-call-recipe.rs").unwrap();
    let unit = Digest::of_bytes(b"LRR scalar macro call recipe boundary");
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
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    let main = source
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    db.begin_run().unwrap();
    let outcome = db
        .fetch(
            &crate::db::provider::SyntaxProvider,
            QueryKey::BodyStructure(main),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(body) = outcome.value.as_ref() else {
        panic!("body variant differs")
    };
    (db, source, body.as_ref().clone())
}
fn draft(recipe: &Recipe) -> RecipeDraft {
    RecipeDraft {
        key: recipe.key.clone(),
        origin: recipe.origin.clone(),
        operations: recipe.operations.to_vec(),
        exits: vec![recipe.exit.clone()],
        dependencies: recipe.dependencies.to_vec(),
    }
}
#[test]
fn source_nested_calls_require_private_witness_and_exact_recipe_fields_all_editions() {
    const SOURCE: &str = "fn id(x:i32)->i32{x} fn main(){assert_eq!(id(id(1)),id(1));}";
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source, body) = setup(SOURCE, edition);
        let macros = StandaloneRuntimeMacros::new(&source).unwrap();
        let context = macros.context(&source).unwrap();
        let node = body
            .nodes()
            .iter()
            .find(|n| n.role == NodeRole::Expression(ExpressionKind::Macro))
            .unwrap();
        let mut key = body.pc(node.id, Phase::Entry).unwrap();
        let mut calls = BTreeSet::new();
        let mut previous: Option<(Arc<RuntimeMacroCall>, RecipeDraft)> = None;
        let mut prepares = 0;
        let mut fused = 0;
        let mut branched = false;
        for _ in 0..usize::from(node.runtime_scalar_phases) {
            let prepared = context
                .resolve(&body, &key, std::iter::empty())
                .unwrap()
                .unwrap();
            let recipe = prepared.recipe.recipe();
            if let Some(call) = &prepared.call {
                let original = draft(recipe);
                assert!(
                    verify(original.clone(), &body).is_err(),
                    "general macro call must not gain ordinary/legacy admission"
                );
                verify_runtime_scalar_call(original.clone(), &body, call).unwrap();
                calls.insert(call.identity());
                let mut wrong_origin = original.clone();
                wrong_origin.origin.span = body.span(node.id).unwrap();
                assert!(verify_runtime_scalar_call(wrong_origin, &body, call).is_err());
                let mut extra = original.clone();
                extra.operations.push(Op::Declaration);
                assert!(verify_runtime_scalar_call(extra, &body, call).is_err());
                let mut phase = original.clone();
                phase.key = body.pc(NodeId(key.node), Phase::Commit).unwrap();
                assert!(verify_runtime_scalar_call(phase, &body, call).is_err());
                if recipe
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::CallPrepare { .. }))
                {
                    prepares += 1;
                    let mut wrong_slot = original.clone();
                    for op in &mut wrong_slot.operations {
                        if let Op::CallPrepare { arguments, .. } = op {
                            arguments[0] = call.result();
                        }
                    }
                    assert!(verify_runtime_scalar_call(wrong_slot, &body, call).is_err());
                    if let Some((foreign, foreign_draft)) = previous.take() {
                        assert_ne!(foreign.identity(), call.identity());
                        assert!(verify_runtime_scalar_call(foreign_draft, &body, call).is_err());
                        assert!(
                            verify_runtime_scalar_call(original.clone(), &body, &foreign).is_err()
                        );
                    }
                    previous = Some((Arc::clone(call), original.clone()));
                }
                if recipe
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::CallSeal { .. }))
                {
                    fused += 1;
                    assert!(
                        recipe
                            .operations
                            .iter()
                            .any(|op| matches!(op, Op::CallAdvance { .. }))
                    );
                    let mut wrong_result = original.clone();
                    let RecipeExit::Call { result, .. } = &mut wrong_result.exits[0] else {
                        panic!("seal lacks call exit")
                    };
                    *result = call.arguments()[0];
                    assert!(verify_runtime_scalar_call(wrong_result, &body, call).is_err());
                    let mut wrong_resume = original;
                    let RecipeExit::Call { resume, .. } = &mut wrong_resume.exits[0] else {
                        panic!("seal lacks call exit")
                    };
                    *resume = call.prepare_key().clone();
                    assert!(verify_runtime_scalar_call(wrong_resume, &body, call).is_err());
                }
            }
            key = match &recipe.exit {
                RecipeExit::Next(next) | RecipeExit::Call { resume: next, .. } => next.clone(),
                RecipeExit::Branch { .. } => {
                    branched = true;
                    break;
                }
                other => panic!("unexpected structural scalar exit: {other:?}"),
            };
        }
        assert!(branched);
        assert_eq!(calls.len(), 3);
        assert_eq!(prepares, 3);
        assert_eq!(fused, 3);
        db.finish_run().unwrap();
    }
}

#[test]
fn legacy_zero_argument_bool_recipe_keeps_normal_verifier_admission_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source, body) = setup(
            "fn truth()->bool{true} fn main(){assert!(truth());}",
            edition,
        );
        let macros = StandaloneRuntimeMacros::new(&source).unwrap();
        let context = macros.context(&source).unwrap();
        let node = body
            .nodes()
            .iter()
            .find(|n| n.role == NodeRole::Expression(ExpressionKind::Macro))
            .unwrap();
        let key = body.pc(node.id, Phase::Entry).unwrap();
        let prepared = context
            .resolve(&body, &key, std::iter::empty())
            .unwrap()
            .unwrap();
        assert!(prepared.call.is_none());
        assert!(
            prepared
                .recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::CallSeal { arguments, .. } if arguments.is_empty()))
        );
        verify(draft(prepared.recipe.recipe()), &body).unwrap();
        db.finish_run().unwrap();
    }
}
