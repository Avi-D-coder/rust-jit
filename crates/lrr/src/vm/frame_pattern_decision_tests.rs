//! Common entered-source traversal/transaction controls. The upstream source is
//! unchanged; prepared guest storage is a component setup, not fixture execution.
use super::tests::{enter_slot_scopes, original_source, prepared_tuple};
use super::*;
use crate::{
    ir::{
        op::{Op, PatternDecision, PatternOperation},
        pc::Phase,
        recipe::{Origin, RecipeDraft, RecipeExit},
        validate::verify,
    },
    syntax::pattern_product::{DecisionKind, PatternProductPlan},
    vm::step::{StepOutcome, step},
};
use ra_ap_syntax::{AstNode, ast};
fn prepared() -> (
    crate::syntax::parse::ParsedFile,
    crate::syntax::outline::BodyHandle,
    Arc<PatternProductPlan>,
    Frame,
    GuestStore,
) {
    let (parsed, handle, index, source, _) = original_source();
    let index = Arc::new(index);
    let pattern = parsed
        .entered_body_root(&handle)
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
    let range = pattern.syntax().text_range();
    let root = index
        .nodes_at_range(crate::syntax::spans::SourceRange {
            start: u32::from(range.start()) - handle.range.start,
            end: u32::from(range.end()) - handle.range.start,
        })
        .find(|node| node.role == crate::syntax::body_index::NodeRole::Pattern)
        .unwrap()
        .id;
    let plan = Arc::new(PatternProductPlan::prepare(&parsed, &handle, &index, root).unwrap());
    let mut store = GuestStore::new(1, 10000, 16 << 20).unwrap();
    let mut frame = Frame::new(
        Arc::clone(&index),
        index.pc(plan.arm().unwrap(), Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    enter_slot_scopes(&mut frame, plan.source());
    let ticket = prepared_tuple(&mut store, &source);
    frame.write(plan.source(), ticket, &mut store).unwrap();
    frame.set_source_view(plan.source(), Some(source)).unwrap();
    (parsed, handle, plan, frame, store)
}
fn begin(plan: &Arc<PatternProductPlan>, frame: &mut Frame, store: &mut GuestStore) {
    let index = plan.index();
    let arm = plan.arm().unwrap();
    let recipe = verify(
        RecipeDraft {
            key: frame.pc.clone(),
            origin: Origin {
                span: index.span(arm).unwrap(),
                expansion_ancestry: vec![],
            },
            operations: vec![
                Op::ScopeEnter {
                    scope: index.nodes()[arm.0 as usize].scope,
                },
                Op::Pattern {
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
                },
            ],
            exits: vec![RecipeExit::PatternNext {
                owner: plan.owner(),
            }],
            dependencies: vec![],
        },
        index,
    )
    .unwrap();
    assert!(matches!(
        step(frame, store, &recipe).unwrap(),
        StepOutcome::Next(_)
    ));
}
#[test]
fn unchanged_source_tuple_edges_use_common_places_and_stop_at_held_binding_proof() {
    let (parsed, handle, plan, mut frame, mut store) = prepared();
    let source = frame.cell(plan.source()).unwrap();
    let before = (
        store.state(source).unwrap(),
        store.location_epoch(source).unwrap(),
    );
    let baseline = frame.patterns.dynamic_metadata_bytes();
    begin(&plan, &mut frame, &mut store);
    let charged = frame.patterns.dynamic_metadata_bytes();
    assert!(charged > baseline);
    let mut reached = false;
    let mut steps = 0;
    for _ in 0..128 {
        let row = plan.row(NodeId(frame.pc.node)).unwrap();
        let recipe = plan.lower_node(&parsed, &handle, &frame.pc).unwrap();
        assert!(recipe.recipe().operations.len() <= 8);
        if matches!(row.kind, DecisionKind::Binding(_))
            && Phase::from_code(frame.pc.phase) == Phase::Operand(1)
        {
            assert!(matches!(
                recipe.recipe().operations.as_ref(),
                [Op::Unsupported(_)]
            ));
            let cursor = frame
                .patterns
                .decision(plan.owner())
                .unwrap()
                .source_cursor
                .unwrap();
            assert_eq!(cursor.depth, row.source_path.len());
            assert!(cursor.ready);
            assert_eq!(
                frame.source_view_ref(cursor.place.unwrap()).unwrap(),
                Some(&TyTerm::Primitive(crate::ir::op::PrimitiveType::Bool))
            );
            reached = true;
            break;
        }
        assert!(matches!(
            step(&mut frame, &mut store, &recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        steps += 1;
        assert_eq!(frame.patterns.dynamic_metadata_bytes(), charged);
        assert_eq!(
            (
                store.state(source).unwrap(),
                store.location_epoch(source).unwrap()
            ),
            before
        );
    }
    assert!(reached && steps > 4);
    assert!(frame.bindings.iter().all(Option::is_none));
    assert_eq!(
        frame.counters.operations_started,
        frame.counters.operations_completed
    );
    // Existing common return cleanup retires both source-place ownership and
    // decision metadata. This is cleanup authority, not a Rust return witness.
    frame.status = FrameStatus::Returned;
    let mut complete = false;
    for _ in 0..10000 {
        if frame.advance_return_cleanup(&mut store).unwrap() {
            complete = true;
            break;
        }
    }
    assert!(complete);
    assert_eq!(frame.patterns.dynamic_metadata_bytes(), baseline);
    assert!(frame.patterns.current(plan.owner()).is_err());
    assert_eq!(frame.projection_bytes, 0);
    assert_eq!(frame.source_extra_bytes, 0);
}
#[test]
fn wrong_source_and_repeated_decision_exit_preserve_current_guest_ownership() {
    let (parsed, handle, plan, mut frame, mut store) = prepared();
    begin(&plan, &mut frame, &mut store);
    assert!(frame.preflight_pattern_next(plan.owner(), &store).is_err());
    // Exit receipt was consumed by the successful step; observing/reusing the
    // same PC cannot publish a second continuation.
    assert!(frame.pattern_next(plan.owner(), &store).is_err());
    let source = frame.cell(plan.source()).unwrap();
    let before = (
        store.state(source).unwrap(),
        store.location_epoch(source).unwrap(),
    );
    frame.set_source_view(plan.source(), None).unwrap();
    let start = plan.lower_node(&parsed, &handle, &frame.pc).unwrap();
    step(&mut frame, &mut store, &start).unwrap();
    let source_step = plan.lower_node(&parsed, &handle, &frame.pc).unwrap();
    let cursor = frame
        .patterns
        .decision(plan.owner())
        .unwrap()
        .source_cursor
        .unwrap();
    assert_eq!(
        step(&mut frame, &mut store, &source_step).unwrap_err().code,
        "LRR-E-UNSUPPORTED"
    );
    assert_eq!(
        frame
            .patterns
            .decision(plan.owner())
            .unwrap()
            .source_cursor
            .unwrap()
            .depth,
        cursor.depth
    );
    assert_eq!(
        (
            store.state(source).unwrap(),
            store.location_epoch(source).unwrap()
        ),
        before
    );
}

#[test]
fn changed_source_storage_epoch_or_cell_cannot_resume_a_retained_decision() {
    for replace_cell in [false, true] {
        let (_, _, plan, mut frame, mut store) = prepared();
        begin(&plan, &mut frame, &mut store);
        let source = frame.cell(plan.source()).unwrap();
        let old_epoch = store.location_epoch(source).unwrap();
        if replace_cell {
            let view = frame.source_view(plan.source()).unwrap().unwrap();
            let ticket = prepared_tuple(&mut store, &view);
            let replacement = store
                .new_cell(
                    StorageContract::Unbound,
                    true,
                    CellOwner::FrameSlot {
                        frame: frame.anchor,
                        slot: plan.source().0,
                    },
                )
                .unwrap();
            store.initialize(replacement, ticket).unwrap();
            frame.slots[plan.source().0 as usize].cell = Some(replacement);
        } else {
            // An external authority control, not an executed Rust guard: even
            // restoring the identical value cannot restore a consumed epoch.
            let ticket = store.move_from(source).unwrap();
            store.initialize(source, ticket).unwrap();
            assert_ne!(store.location_epoch(source).unwrap(), old_epoch);
        }
        let actual = frame.cell(plan.source()).unwrap();
        let before = (
            store.state(actual).unwrap(),
            store.location_epoch(actual).unwrap(),
            frame.patterns.dynamic_metadata_bytes(),
        );
        let operation = PatternOperation::Decision {
            owner: plan.owner(),
            plan: Arc::clone(&plan),
            transition: PatternDecision::StartSource,
        };
        let error = frame.execute_pattern(&operation, &mut store).unwrap_err();
        assert!(error.message.contains("source storage changed"));
        assert!(
            frame
                .patterns
                .decision(plan.owner())
                .unwrap()
                .source_cursor
                .is_none()
        );
        assert_eq!(
            (
                store.state(actual).unwrap(),
                store.location_epoch(actual).unwrap(),
                frame.patterns.dynamic_metadata_bytes()
            ),
            before
        );
    }
}

#[test]
fn guard_retry_requires_one_completed_arm_leave_and_reenters_it_once() {
    use crate::vm::pattern::decision::DecisionTarget;
    let (parsed, handle, plan, mut frame, mut store) = prepared();
    begin(&plan, &mut frame, &mut store);
    let source = frame.cell(plan.source()).unwrap();
    let before = (
        store.state(source).unwrap(),
        store.location_epoch(source).unwrap(),
    );
    let arm = plan.arm().unwrap();
    let arm_scope = plan.index().nodes()[arm.0 as usize].scope;
    let guard_scope = plan.index().nodes()[arm.0 as usize].guard_scope.unwrap();
    assert_eq!(
        plan.matched(),
        &plan.index().pc(arm, Phase::Operand(0)).unwrap()
    );
    // With the real cursor still at root Entry, an unrelated live child
    // scope must be rejected by the attempt-scope check itself.
    frame.enter(guard_scope).unwrap();
    let enter = PatternOperation::Decision {
        owner: plan.owner(),
        plan: Arc::clone(&plan),
        transition: PatternDecision::EnterAttempt { scope: arm_scope },
    };
    assert!(
        frame
            .preflight_pattern(&enter, &store)
            .unwrap_err()
            .message
            .contains("nested scope")
    );
    assert!(
        frame
            .begin_scope_cleanup(0, &Op::ScopeLeave { scope: guard_scope }, &mut store)
            .unwrap()
    );
    let mut cleaned = false;
    for _ in 0..10000 {
        if frame.advance_scope_cleanup(&mut store).unwrap() {
            cleaned = true;
            break;
        }
    }
    assert!(cleaned);
    // Inject only the cursor's completed-test event. Binding authority and the
    // stock guard Rust body remain held; this tests ordinary scope protocol.
    let mut target = DecisionTarget::Node(plan.root(), Phase::Entry);
    for _ in 0..1000 {
        target = match target {
            DecisionTarget::Node(node, Phase::Entry) => frame
                .patterns
                .decision_mut(plan.owner())
                .unwrap()
                .state
                .enter(node)
                .unwrap(),
            DecisionTarget::Node(node, Phase::Commit) => frame
                .patterns
                .decision_mut(plan.owner())
                .unwrap()
                .state
                .success(node)
                .unwrap(),
            DecisionTarget::Matched => break,
            _ => panic!("unexpected source scheduling target"),
        };
    }
    assert!(
        frame
            .patterns
            .decision(plan.owner())
            .unwrap()
            .state
            .matched()
    );
    let fail = PatternOperation::Decision {
        owner: plan.owner(),
        plan: Arc::clone(&plan),
        transition: PatternDecision::Fail { node: None },
    };
    frame.pc = plan.index().pc(arm, Phase::Exit).unwrap();
    assert!(frame.preflight_pattern(&fail, &store).is_err());
    frame.enter(guard_scope).unwrap();
    assert!(frame.preflight_pattern(&fail, &store).is_err());
    frame.pc = plan.index().pc(arm, Phase::Exit).unwrap();
    assert!(
        frame
            .begin_scope_cleanup(0, &Op::ScopeLeave { scope: guard_scope }, &mut store)
            .unwrap()
    );
    let mut cleaned = false;
    for _ in 0..10000 {
        if frame.advance_scope_cleanup(&mut store).unwrap() {
            cleaned = true;
            break;
        }
    }
    assert!(cleaned);
    let make = |operations| RecipeDraft {
        key: plan.index().pc(arm, Phase::Exit).unwrap(),
        origin: Origin {
            span: plan.index().span(arm).unwrap(),
            expansion_ancestry: vec![],
        },
        operations,
        exits: vec![RecipeExit::PatternNext {
            owner: plan.owner(),
        }],
        dependencies: vec![],
    };
    assert!(
        verify(
            make(vec![Op::Pattern {
                operation: fail.clone()
            }]),
            plan.index()
        )
        .is_err()
    );
    assert!(
        verify(
            make(vec![
                Op::ScopeLeave { scope: arm_scope },
                Op::ScopeLeave { scope: arm_scope },
                Op::Pattern {
                    operation: fail.clone()
                }
            ]),
            plan.index()
        )
        .is_err()
    );
    let recipe = verify(
        make(vec![
            Op::ScopeLeave { scope: arm_scope },
            Op::Pattern { operation: fail },
        ]),
        plan.index(),
    )
    .unwrap();
    let mut completed = false;
    for _ in 0..10000 {
        match step(&mut frame, &mut store, &recipe).unwrap() {
            StepOutcome::Dispatch => {}
            StepOutcome::Next(_) => {
                completed = true;
                break;
            }
            _ => panic!("scope protocol selected an unrelated boundary"),
        }
    }
    assert!(completed);
    assert!(!frame.active_scopes.contains(&arm_scope));
    assert!(!frame.active_scopes.contains(&guard_scope));
    assert_eq!(
        frame.pc,
        plan.index().pc(plan.root(), Phase::PatternCleanup).unwrap()
    );
    let cleanup = plan.lower_node(&parsed, &handle, &frame.pc).unwrap();
    step(&mut frame, &mut store, &cleanup).unwrap();
    assert_eq!(
        frame.pc,
        plan.index().pc(plan.root(), Phase::Entry).unwrap()
    );
    let retry = plan.lower_node(&parsed, &handle, &frame.pc).unwrap();
    step(&mut frame, &mut store, &retry).unwrap();
    assert_eq!(
        frame
            .active_scopes
            .iter()
            .filter(|scope| **scope == arm_scope)
            .count(),
        1
    );
    assert_eq!(
        (
            store.state(source).unwrap(),
            store.location_epoch(source).unwrap()
        ),
        before
    );
}
