//! Cursor/source-order controls over unchanged upstream syntax. These exercise
//! retained scheduling, not guest guards, bindings or a whole fixture executor.
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{BodyIndex, NodeRole},
        parse::ParsedFile,
        pattern_product::{DecisionKind, PatternProductPlan},
        spans::SourceRange,
    },
    vm::pattern::decision::{DecisionState, DecisionTarget},
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/or-patterns/search-via-bindings.rs"
);
fn plan(function: &str, edition: u16) -> Arc<PatternProductPlan> {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([87; 32]), vec![], &CfgSet::guest_default())
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
    let range = SourceRange {
        start: u32::from(range.start()) - body.range.start,
        end: u32::from(range.end()) - body.range.start,
    };
    let root = index
        .nodes_at_range(range)
        .find(|node| node.role == NodeRole::Pattern)
        .unwrap()
        .id;
    Arc::new(PatternProductPlan::prepare(&parsed, body, &index, root).unwrap())
}
fn attempts(plan: Arc<PatternProductPlan>) -> (Vec<Vec<Vec<u32>>>, usize) {
    let mut state = DecisionState::prepare(Arc::clone(&plan), 32 * 1024 * 1024).unwrap();
    let bytes = state.metadata_bytes().unwrap();
    let mut target = DecisionTarget::Node(plan.root(), crate::ir::pc::Phase::Entry);
    let mut attempts = Vec::new();
    let mut paths = Vec::new();
    let mut cleanup_steps = 0;
    for _ in 0..10000 {
        assert_eq!(state.metadata_bytes(), Some(bytes));
        target = match target {
            DecisionTarget::Node(node, crate::ir::pc::Phase::Entry) => {
                if matches!(plan.row(node).unwrap().kind, DecisionKind::Binding(_)) {
                    paths.push(plan.row(node).unwrap().source_path.to_vec());
                }
                state.enter(node).unwrap()
            }
            DecisionTarget::Node(node, crate::ir::pc::Phase::Commit) => {
                state.success(node).unwrap()
            }
            DecisionTarget::Matched => {
                attempts.push(std::mem::take(&mut paths));
                // This is an explicit synthetic false-guard transaction event,
                // never a claim to execute the upstream guard's Rust body.
                state.begin_failure(None).unwrap();
                DecisionTarget::Cleanup
            }
            DecisionTarget::Cleanup => {
                cleanup_steps += 1;
                state.backtrack_step().unwrap()
            }
            DecisionTarget::Unmatched => return (attempts, cleanup_steps),
            _ => panic!("cursor selected an unprepared decision phase"),
        };
    }
    panic!("bounded stock decision graph did not exhaust")
}
#[test]
fn original_nested_product_and_spelled_out_choices_retry_in_source_order_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (nested, cleanup) = attempts(plan("search", edition));
        let (spelled, _) = attempts(plan("search_old_style", edition));
        assert_eq!(nested, spelled);
        assert_eq!(nested.len(), 8);
        for (ordinal, paths) in nested.iter().enumerate() {
            assert_eq!(
                paths,
                &vec![
                    vec![0, (ordinal / 4) as u32],
                    vec![1, ((ordinal / 2) % 2) as u32],
                    vec![2, (ordinal % 2) as u32]
                ]
            );
        }
        // Exhausted choices each require their own cleanup invocation as well
        // as the seven successful advances and the final exhaustion event.
        assert!(cleanup > 8);
    }
}
#[test]
fn original_dummy_choices_retry_even_without_binding_or_scalar_test() {
    let (paths, _) = attempts(plan("search_with_dummy", 2024));
    assert_eq!(paths.len(), 8);
    for pair in paths.chunks_exact(2) {
        assert_eq!(pair[0], pair[1]);
    }
}
#[test]
fn source_cursor_rejects_foreign_node_phase_and_unmatched_guard_without_mutation() {
    let plan = plan("search", 2024);
    let mut state = DecisionState::prepare(Arc::clone(&plan), 32 * 1024 * 1024).unwrap();
    let bytes = state.metadata_bytes();
    assert!(state.begin_failure(None).is_err());
    assert!(state.success(plan.root()).is_err());
    let other = plan
        .rows()
        .iter()
        .find(|row| row.node != plan.root())
        .unwrap()
        .node;
    assert!(state.enter(other).is_err());
    assert_eq!(state.metadata_bytes(), bytes);
    state.enter(plan.root()).unwrap();
    assert!(state.enter(plan.root()).is_err());
    assert!(DecisionState::prepare(Arc::clone(&plan), 0).is_err());
    assert!(plan.require_index(&BodyIndex::clone(plan.index())).is_ok());
    let mut wrong_profile = ProfileIdentity::current();
    wrong_profile.edition = 2015;
    assert!(
        plan.require_runtime(plan.index().body(), &wrong_profile)
            .is_err()
    );
}

#[test]
fn pending_intent_marks_survive_nested_rollback_and_vector_position_reuse() {
    use crate::syntax::body_index::ScopeId;
    use crate::vm::pattern::{PendingBinding, Transactions};
    let plan = plan("search", 2024);
    let bindings = plan
        .rows()
        .iter()
        .filter(|row| matches!(row.kind, DecisionKind::Binding(_)))
        .take(3)
        .map(|row| {
            (
                row.node,
                plan.index().nodes()[row.node.0 as usize].runtime_temporaries[0].unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 3);
    let mut transactions = Transactions::new(
        plan.index().nodes().len(),
        plan.index().slot_count() as usize,
    )
    .unwrap();
    let owner = plan.owner();
    let nested = plan.root();
    transactions
        .begin(
            owner,
            plan.root(),
            plan.source(),
            plan.unmatched().clone(),
            ScopeId(0),
        )
        .unwrap();
    let pending = |binding: usize, source: usize| PendingBinding {
        binding: bindings[binding].0,
        source: bindings[source].1,
        writable: false,
    };
    transactions.stage(owner, pending(0, 0)).unwrap();
    transactions
        .begin(
            nested,
            plan.root(),
            plan.source(),
            plan.unmatched().clone(),
            ScopeId(1),
        )
        .unwrap();
    transactions.stage(nested, pending(1, 1)).unwrap();
    transactions.retire_scope(ScopeId(1));
    transactions
        .begin(
            nested,
            plan.root(),
            plan.source(),
            plan.unmatched().clone(),
            ScopeId(1),
        )
        .unwrap();
    // Row1's old binding marker now points to a different intent at the same
    // vector position; its stale identity cannot reject or authorize a stage.
    transactions.stage(nested, pending(2, 1)).unwrap();
    transactions.stage(nested, pending(1, 2)).unwrap();
    assert!(transactions.stage(nested, pending(0, 1)).is_err());
    transactions.finish(nested).unwrap();
    assert_eq!(transactions.pending(owner).unwrap().len(), 1);
    transactions.finish(owner).unwrap();
    transactions
        .begin(
            owner,
            plan.root(),
            plan.source(),
            plan.unmatched().clone(),
            ScopeId(0),
        )
        .unwrap();
    transactions.stage(owner, pending(0, 0)).unwrap();
    transactions.stage(owner, pending(1, 1)).unwrap();
}
