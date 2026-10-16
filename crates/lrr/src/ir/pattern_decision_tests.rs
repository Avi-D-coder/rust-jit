//! Wire/source-authority controls for the common pattern cursor. No fixture run.
use super::*;
use crate::{
    core::{
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    ir::{op::LRIR_RECIPE_SCHEMA, recipe::Origin, validate::verify},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::NodeRole, parse::ParsedFile, pattern_product::PatternProductPlan,
        spans::SourceRange,
    },
};
use ra_ap_syntax::{AstNode, ast};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/or-patterns/search-via-bindings.rs"
);
fn plan(function: &str) -> Arc<PatternProductPlan> {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([86; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == function)
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let index = Arc::new(parsed.index_body(body, Default::default()).unwrap());
    let root = parsed
        .entered_body_root(body)
        .unwrap()
        .descendants()
        .find_map(ast::MatchExpr::cast)
        .unwrap()
        .match_arm_list()
        .unwrap()
        .arms()
        .next()
        .unwrap()
        .pat()
        .unwrap();
    let range = root.syntax().text_range();
    let root = index
        .nodes_at_range(SourceRange {
            start: u32::from(range.start()) - body.range.start,
            end: u32::from(range.end()) - body.range.start,
        })
        .find(|node| node.role == NodeRole::Pattern)
        .unwrap()
        .id;
    Arc::new(PatternProductPlan::prepare(&parsed, body, &index, root).unwrap())
}
fn begin(plan: &Arc<PatternProductPlan>) -> RecipeDraft {
    let arm = plan.arm().unwrap();
    let index = plan.index();
    RecipeDraft {
        key: index.pc(arm, Phase::Entry).unwrap(),
        origin: Origin {
            span: index.span(arm).unwrap(),
            expansion_ancestry: vec![],
        },
        operations: vec![Op::Pattern {
            operation: PatternOperation::Decision {
                owner: plan.owner(),
                plan: Arc::clone(plan),
                transition: PatternDecision::Begin {
                    source: plan.source(),
                    matched: plan.matched().clone(),
                    unmatched: plan.unmatched().clone(),
                    unmatched_inactive: plan.unmatched_inactive().clone(),
                },
            },
        }],
        exits: vec![RecipeExit::PatternNext {
            owner: plan.owner(),
        }],
        dependencies: vec![],
    }
}
#[test]
fn decision_wire_has_explicit_schema_plan_receipt_and_three_prepared_continuations() {
    assert_eq!(LRIR_RECIPE_SCHEMA, 13);
    let plan = plan("search");
    let index = plan.index();
    let recipe = verify(begin(&plan), index).unwrap();
    assert_eq!(recipe.product_roots().count(), 1);
    assert!(Arc::ptr_eq(recipe.product_roots().next().unwrap(), &plan));
    let recipe = recipe.recipe();
    let mut expected = StableEncoder::new("lrr/v13/lrir-recipe");
    expected.record(&recipe.key);
    expected.record(&recipe.origin.span.body_key);
    expected.u32(recipe.origin.span.relative_start);
    expected.u32(recipe.origin.span.relative_end);
    expected.u64(0);
    expected.u64(1);
    expected.u32(45);
    expected.u32(5);
    expected.u32(plan.owner().0);
    expected.digest(plan.identity());
    expected.u32(0);
    expected.u32(plan.source().0);
    expected.record(plan.matched());
    expected.record(plan.unmatched());
    expected.record(plan.unmatched_inactive());
    expected.u32(9);
    expected.u32(plan.owner().0);
    expected.u64(0);
    assert_eq!(recipe.fingerprint(), expected.finish());
    // The memo adapter must count actual handles, then deduplicate allocations
    // separately. Two actions in a flat recipe can own the same product Arc.
    let op = |transition| Op::Pattern {
        operation: PatternOperation::Decision {
            owner: plan.owner(),
            plan: Arc::clone(&plan),
            transition,
        },
    };
    let shared = verify(
        RecipeDraft {
            key: index.pc(plan.root(), Phase::Entry).unwrap(),
            origin: Origin {
                span: index.span(plan.root()).unwrap(),
                expansion_ancestry: vec![],
            },
            operations: vec![
                op(PatternDecision::Observe),
                op(PatternDecision::StartSource),
            ],
            exits: vec![RecipeExit::PatternNext {
                owner: plan.owner(),
            }],
            dependencies: vec![],
        },
        index,
    )
    .unwrap();
    assert_eq!(shared.product_roots().count(), 2);
    assert!(shared.product_roots().all(|root| Arc::ptr_eq(root, &plan)));
}
#[test]
fn caller_cannot_replace_owner_source_continuation_or_plan_and_keep_admission() {
    let plan = plan("search");
    let index = plan.index();
    let draft = begin(&plan);
    let mut wrong = draft.clone();
    wrong.exits = vec![RecipeExit::Next(plan.matched().clone())];
    assert!(verify(wrong, index).is_err());
    let mut wrong = draft.clone();
    if let Op::Pattern {
        operation: PatternOperation::Decision { owner, .. },
    } = &mut wrong.operations[0]
    {
        *owner = NodeId(0);
    }
    assert!(verify(wrong, index).is_err());
    let mut wrong = draft.clone();
    if let Op::Pattern {
        operation:
            PatternOperation::Decision {
                transition: PatternDecision::Begin { source, .. },
                ..
            },
    } = &mut wrong.operations[0]
    {
        *source = index.nodes()[plan.owner().0 as usize].value.unwrap();
    }
    assert!(verify(wrong, index).is_err());
    let mut wrong = draft.clone();
    if let Op::Pattern {
        operation:
            PatternOperation::Decision {
                transition:
                    PatternDecision::Begin {
                        matched, unmatched, ..
                    },
                ..
            },
    } = &mut wrong.operations[0]
    {
        std::mem::swap(matched, unmatched);
    }
    assert!(verify(wrong, index).is_err());
    let mut wrong = draft;
    if let Op::Pattern {
        operation: PatternOperation::Decision { plan: actual, .. },
    } = &mut wrong.operations[0]
    {
        *actual = self::plan("search_with_dummy");
    }
    assert!(verify(wrong, index).is_err());
}
