//! Explicit ordinary pattern transition/cache schema and structural authority.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{
        op::{LRIR_RECIPE_SCHEMA, PatternOperation},
        pc::{NodeId, Phase},
        validate::verify,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        parse::ParsedFile,
    },
};

fn indexed() -> BodyIndex {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(b"fn main(){match 2_i32{1_i32..=3_i32 if false=>(),_=>()}}".to_vec())
            .unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([42; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap()
}
#[test]
fn pattern_begin_and_retry_have_explicit_stable_tags_and_exact_source_roles() {
    assert_eq!(LRIR_RECIPE_SCHEMA, 12);
    assert_eq!(Recipe::DOMAIN, "lrr/v12/lrir-recipe");
    let body = indexed();
    let owner = body
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Expression(ExpressionKind::Match))
        .unwrap();
    let arm = body
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::MatchArm && node.guard_scope.is_some())
        .unwrap();
    let alternative = body
        .nodes()
        .iter()
        .find(|node| node.scalar_range_phase)
        .unwrap();
    let source = body.nodes()[owner.operands[0].0 as usize].value.unwrap();
    let retry = body
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::MatchArm && node.id != arm.id)
        .unwrap();
    let retry = body.pc(retry.id, Phase::Entry).unwrap();
    let operation = Op::Pattern {
        operation: PatternOperation::Begin {
            owner: owner.id,
            alternative: alternative.id,
            source,
            retry: retry.clone(),
        },
    };
    let make = |operation| RecipeDraft {
        key: body.pc(alternative.id, Phase::Entry).unwrap(),
        origin: Origin {
            span: body.span(alternative.id).unwrap(),
            expansion_ancestry: vec![],
        },
        operations: vec![operation],
        exits: vec![RecipeExit::Next(
            body.pc(alternative.id, Phase::Commit).unwrap(),
        )],
        dependencies: vec![],
    };
    let verified = verify(make(operation), &body).unwrap();
    let recipe = verified.recipe();
    let mut expected = StableEncoder::new("lrr/v12/lrir-recipe");
    expected.record(&recipe.key);
    span(&mut expected, &recipe.origin.span);
    expected.u64(0); // expansion ancestry sequence
    expected.u64(1); // operations sequence
    expected.u32(45);
    expected.u32(0);
    expected.u32(owner.id.0);
    expected.u32(alternative.id.0);
    expected.u32(source.0);
    expected.record(&retry);
    expected.u32(0);
    expected.record(&body.pc(alternative.id, Phase::Commit).unwrap());
    expected.u64(0); // dependency sequence
    assert_eq!(recipe.fingerprint(), expected.finish());
    let mut wrong_source = make(Op::Pattern {
        operation: PatternOperation::Begin {
            owner: owner.id,
            alternative: alternative.id,
            source: owner.value.unwrap(),
            retry: retry.clone(),
        },
    });
    assert!(verify(wrong_source.clone(), &body).is_err());
    wrong_source.key.body.definition = Digest([0; 32]);
    assert!(verify(wrong_source, &body).is_err());
    let exit = RecipeDraft {
        key: body.pc(arm.id, Phase::Exit).unwrap(),
        origin: Origin {
            span: body.span(arm.id).unwrap(),
            expansion_ancestry: vec![],
        },
        operations: vec![],
        exits: vec![RecipeExit::PatternRetry { owner: owner.id }],
        dependencies: vec![],
    };
    verify(exit.clone(), &body).unwrap();
    let mut wrong_owner = exit;
    wrong_owner.exits = vec![RecipeExit::PatternRetry { owner: NodeId(0) }];
    assert!(verify(wrong_owner, &body).is_err());
}

#[test]
fn prepared_definition_result_hint_has_an_explicit_tag_and_cannot_move_from_root() {
    let mut actual = StableEncoder::new("lrr/reference-result-hint-control");
    hint(&mut actual, &HintRecipe::DefinitionResult);
    let mut expected = StableEncoder::new("lrr/reference-result-hint-control");
    expected.u32(8);
    assert_eq!(actual.finish(), expected.finish());
    let body = indexed();
    let root = &body.nodes()[0];
    let mut draft = RecipeDraft {
        key: body.pc(root.id, Phase::Entry).unwrap(),
        origin: Origin {
            span: body.span(root.id).unwrap(),
            expansion_ancestry: vec![],
        },
        operations: vec![Op::SetHint {
            child: root.value.unwrap(),
            hint: HintRecipe::DefinitionResult,
        }],
        exits: vec![RecipeExit::Next(body.pc(root.id, Phase::Commit).unwrap())],
        dependencies: vec![],
    };
    verify(draft.clone(), &body).unwrap(); // structural only; runtime needs actual opaque result proof
    draft.operations = vec![Op::Literal {
        literal: super::super::op::LiteralRecipe {
            kind: super::super::op::LiteralKind::Bool,
            spelling: "true".into(),
            decoded: None,
            source: body.span(root.id).unwrap(),
        },
        hint: HintRecipe::DefinitionResult,
        destination: root.value.unwrap(),
    }];
    assert!(verify(draft, &body).is_err());
}
