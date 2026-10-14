#[path = "support/cached_integer_fixture.rs"]
mod cached_integer_fixture;
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    ir::{
        op::{DiagnosticTemplate, HintRecipe, LiteralKind, LiteralRecipe, Op, PrimitiveType},
        pc::{NodeId, Phase, SlotId},
        recipe::{ControlRecipe, Origin, RecipeDraft, RecipeExit, VerifiedRecipe},
        validate::verify,
    },
    runtime::{
        cell::{CellState, GuestStore},
        value::ValueRepr,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole, ScopeId},
        parse::ParsedFile,
    },
    vm::{
        frame::{Frame, FrameStatus},
        step::{StepOutcome, step},
    },
};
fn body() -> BodyIndex {
    let blob = SourceBlob::new(
        b"fn live(){let _=(40_i64, 0); true} fn cold(){missing::<Undefined>();}".to_vec(),
    )
    .unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([77; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap()
}
fn expressions(body: &BodyIndex) -> Vec<(NodeId, SlotId)> {
    body.nodes()
        .iter()
        .filter(|n| n.role == NodeRole::Expression(ExpressionKind::Literal))
        .map(|n| (n.id, n.value.unwrap()))
        .collect()
}
fn recipe(
    body: &BodyIndex,
    node: NodeId,
    phase: Phase,
    mut ops: Vec<Op>,
    exit: RecipeExit,
) -> VerifiedRecipe {
    // Raw kernel fixtures keep jointly used operands in one statement. Enter
    // that actual indexed scope when the recipe starts its root explicitly.
    if matches!(ops.first(),Some(Op::ScopeEnter{scope}) if *scope==ScopeId(0)) {
        let wanted = ops
            .iter()
            .find_map(|op| {
                if let Op::Literal { destination, .. } = op {
                    body.nodes()
                        .iter()
                        .find(|n| n.value == Some(*destination))
                        .map(|n| n.scope)
                } else {
                    None
                }
            })
            .or_else(|| {
                if let RecipeExit::Next(pc) = &exit {
                    Some(body.nodes()[pc.node as usize].scope)
                } else {
                    None
                }
            });
        if let Some(scope) = wanted
            && body.scopes()[scope.0 as usize].kind == lrr::syntax::body_index::ScopeKind::Statement
            && body.scopes()[scope.0 as usize].parent == Some(ScopeId(0))
            && !ops
                .iter()
                .any(|op| matches!(op,Op::ScopeEnter{scope:other} if *other==scope))
        {
            ops.insert(1, Op::ScopeEnter { scope });
        }
    }
    verify(
        RecipeDraft {
            key: body.pc(node, phase).unwrap(),
            origin: Origin {
                span: body.span(node).unwrap(),
                expansion_ancestry: vec![],
            },
            operations: ops,
            exits: vec![exit],
            dependencies: vec![],
        },
        body,
    )
    .unwrap()
}
fn setup(body: &BodyIndex) -> (GuestStore, Frame) {
    let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
    let frame = Frame::new(
        body.clone(),
        body.pc(NodeId(0), Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    (store, frame)
}
#[test]
fn verified_scalar_recipe_executes_on_physical_cells_with_exact_opcode_counts() {
    let body = body();
    let e = expressions(&body);
    let (mut store, mut frame) = setup(&body);
    let first = body.pc(e[0].0, Phase::Entry).unwrap();
    let enter = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![
            Op::ScopeEnter { scope: ScopeId(0) },
            Op::SetHint {
                child: e[1].1,
                hint: HintRecipe::Primitive(PrimitiveType::I64),
            },
        ],
        RecipeExit::Next(first.clone()),
    );
    assert!(matches!(step(&mut frame,&mut store,&enter).unwrap(),StepOutcome::Next(p) if p==first));
    let next = body.pc(e[1].0, Phase::Commit).unwrap();
    let literal = recipe(
        &body,
        e[0].0,
        Phase::Entry,
        vec![Op::Literal {
            literal: LiteralRecipe {
                decoded: Some(cached_integer_fixture::integer("40_i64")),
                kind: LiteralKind::Integer,
                spelling: "40_i64".into(),
                source: body.span(e[0].0).unwrap(),
            },
            hint: HintRecipe::None,
            destination: e[0].1,
        }],
        RecipeExit::Next(next),
    );
    step(&mut frame, &mut store, &literal).unwrap();
    let source = frame.cell(e[0].1).unwrap();
    assert_eq!(store.read_scalar(source).unwrap().payload, 40);
    let transfer = recipe(
        &body,
        e[1].0,
        Phase::Commit,
        vec![Op::ResultTransfer {
            source: e[0].1,
            destination: e[1].1,
        }],
        RecipeExit::Control(ControlRecipe::Return { value: e[1].1 }),
    );
    let StepOutcome::Return(result) = step(&mut frame, &mut store, &transfer).unwrap() else {
        panic!("missing actual return")
    };
    assert_eq!(
        store.peek_ticket(result).unwrap(),
        ValueRepr::signed(lrr::runtime::shape::TypeId(7), 40).unwrap()
    );
    assert_eq!(store.state(source).unwrap(), CellState::Moved);
    assert_eq!(frame.status(), FrameStatus::Returned);
    assert_eq!(frame.counters().operations_started, 5);
    assert_eq!(frame.counters().operations_completed, 5);
    assert_eq!(frame.counters().exits_completed, 3);
    let mut expected_opcode_counts = [0; lrr::ir::op::OPCODE_COUNT];
    expected_opcode_counts[0] = 1;
    expected_opcode_counts[2] = 1;
    expected_opcode_counts[3] = 1;
    expected_opcode_counts[4] = 2;
    assert_eq!(frame.counters().started_by_opcode, expected_opcode_counts);
    assert!(step(&mut frame, &mut store, &transfer).is_err());
    assert_eq!(frame.counters().operations_started, 5);
}
#[test]
fn reached_unsupported_fault_is_terminal_without_running_cleanup_or_successor() {
    let body = body();
    let e = expressions(&body);
    let (mut store, mut frame) = setup(&body);
    let unsupported = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![
            Op::ScopeEnter { scope: ScopeId(0) },
            Op::Literal {
                literal: LiteralRecipe {
                    decoded: Some(cached_integer_fixture::integer("40_i64")),
                    kind: LiteralKind::Integer,
                    spelling: "40_i64".into(),
                    source: body.span(e[0].0).unwrap(),
                },
                hint: HintRecipe::None,
                destination: e[0].1,
            },
            Op::Unsupported(DiagnosticTemplate {
                message: "reached unsupported witness".into(),
            }),
            Op::ScopeLeave { scope: ScopeId(0) },
        ],
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    assert_eq!(
        step(&mut frame, &mut store, &unsupported).unwrap_err().code,
        "LRR-E-UNSUPPORTED"
    );
    assert_eq!(frame.status(), FrameStatus::Faulted);
    assert_eq!(
        frame.active_scopes(),
        [
            ScopeId(0),
            body.nodes().iter().find_map(|n| n.statement_scope).unwrap()
        ]
    );
    assert_eq!(
        store
            .read_scalar(frame.cell(e[0].1).unwrap())
            .unwrap()
            .payload,
        40
    );
    assert_eq!(frame.counters().operations_started, 4);
    assert_eq!(frame.counters().operations_completed, 3);
    assert_eq!(frame.counters().exits_completed, 0);
    let mut expected_opcode_counts = [0; lrr::ir::op::OPCODE_COUNT];
    expected_opcode_counts[0] = 1;
    expected_opcode_counts[4] = 2;
    expected_opcode_counts[6] = 1;
    assert_eq!(frame.counters().started_by_opcode, expected_opcode_counts);
    assert!(step(&mut frame, &mut store, &unsupported).is_err());
    assert_eq!(frame.counters().operations_started, 4);
}
#[test]
fn branch_exit_selects_actual_bool_pc_and_scope_leave_retires_storage() {
    let body = body();
    let e = expressions(&body);
    let tail = body.nodes()[0].tail_scope.unwrap();
    let (mut store, mut frame) = setup(&body);
    let yes = body.pc(e[0].0, Phase::Entry).unwrap();
    let no = body.pc(e[1].0, Phase::Entry).unwrap();
    let branch = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![
            Op::ScopeEnter { scope: ScopeId(0) },
            Op::ScopeEnter { scope: tail },
            Op::Literal {
                literal: LiteralRecipe {
                    decoded: None,
                    kind: LiteralKind::Bool,
                    spelling: "true".into(),
                    source: body.span(e[2].0).unwrap(),
                },
                hint: HintRecipe::None,
                destination: e[2].1,
            },
        ],
        RecipeExit::Branch {
            condition: e[2].1,
            yes: yes.clone(),
            no,
        },
    );
    assert!(
        matches!(step(&mut frame,&mut store,&branch).unwrap(),StepOutcome::Next(pc) if pc==yes)
    );
    let old = frame.cell(e[2].1).unwrap();
    let leave = recipe(
        &body,
        e[0].0,
        Phase::Entry,
        vec![
            Op::ScopeLeave { scope: tail },
            Op::ScopeLeave { scope: ScopeId(0) },
        ],
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    step(&mut frame, &mut store, &leave).unwrap();
    assert_eq!(store.state(old).unwrap(), CellState::Dropped);
    assert!(frame.cell(e[2].1).is_err());
    assert!(frame.active_scopes().is_empty());
    assert_eq!(frame.counters().operations_completed, 5);
}
#[test]
fn rejected_transfer_contract_preserves_live_source_and_faults_the_frame() {
    let body = body();
    let e = expressions(&body);
    let (mut store, mut frame) = setup(&body);
    let entry = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![
            Op::ScopeEnter { scope: ScopeId(0) },
            Op::SetHint {
                child: e[1].1,
                hint: HintRecipe::Primitive(PrimitiveType::Bool),
            },
            Op::Literal {
                literal: LiteralRecipe {
                    decoded: Some(cached_integer_fixture::integer("40_i64")),
                    kind: LiteralKind::Integer,
                    spelling: "40_i64".into(),
                    source: body.span(e[0].0).unwrap(),
                },
                hint: HintRecipe::None,
                destination: e[0].1,
            },
        ],
        RecipeExit::Next(body.pc(e[1].0, Phase::Commit).unwrap()),
    );
    step(&mut frame, &mut store, &entry).unwrap();
    let source = frame.cell(e[0].1).unwrap();
    let before = store.state(source).unwrap();
    let transfer = recipe(
        &body,
        e[1].0,
        Phase::Commit,
        vec![Op::ResultTransfer {
            source: e[0].1,
            destination: e[1].1,
        }],
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    assert_eq!(
        step(&mut frame, &mut store, &transfer).unwrap_err().code,
        "LRR-E-OWNERSHIP"
    );
    assert_eq!(store.state(source).unwrap(), before);
    assert!(frame.cell(e[1].1).is_err());
    assert_eq!(frame.status(), FrameStatus::Faulted);
    assert_eq!(frame.counters().operations_completed, 4);
    assert_eq!(frame.counters().operations_started, 4);
    assert_eq!(frame.fuel_snapshot().used, 5);
}

#[test]
fn literal_recipe_hint_cannot_reinterpret_an_explicit_suffix_or_bool() {
    for (kind, spelling) in [
        (LiteralKind::Integer, "40_i64"),
        (LiteralKind::Bool, "true"),
    ] {
        let body = body();
        let e = expressions(&body);
        let (mut store, mut frame) = setup(&body);
        let witness = recipe(
            &body,
            NodeId(0),
            Phase::Entry,
            vec![
                Op::ScopeEnter { scope: ScopeId(0) },
                Op::Literal {
                    literal: LiteralRecipe {
                        decoded: match kind {
                            LiteralKind::Integer => Some(cached_integer_fixture::integer(spelling)),
                            LiteralKind::Bool => None,
                            _ => panic!("host fixture has no other literal kind"),
                        },
                        kind,
                        spelling: spelling.into(),
                        source: body.span(e[0].0).unwrap(),
                    },
                    hint: HintRecipe::Primitive(PrimitiveType::U8),
                    destination: e[0].1,
                },
            ],
            RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
        );
        assert_eq!(
            step(&mut frame, &mut store, &witness).unwrap_err().code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(frame.status(), FrameStatus::Faulted);
        assert!(frame.cell(e[0].1).is_err());
        assert_eq!(frame.fuel_snapshot().used, 2);
        assert_eq!(store.budget().handles(), 1);
        assert_eq!(frame.counters().operations_completed, 2);
    }
}

#[test]
fn invalid_scope_state_faults_before_charging_or_mutating_scope_stack() {
    let body = body();
    let (mut store, mut frame) = setup(&body);
    let invalid_leave = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![Op::ScopeLeave { scope: ScopeId(0) }],
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    assert_eq!(
        step(&mut frame, &mut store, &invalid_leave)
            .unwrap_err()
            .code,
        "LRR-E-INTERNAL-IR"
    );
    assert_eq!(frame.fuel_snapshot().used, 0);
    assert!(frame.active_scopes().is_empty());
    let (mut store, mut frame) = setup(&body);
    let duplicate = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![
            Op::ScopeEnter { scope: ScopeId(0) },
            Op::ScopeEnter { scope: ScopeId(0) },
        ],
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    assert_eq!(
        step(&mut frame, &mut store, &duplicate).unwrap_err().code,
        "LRR-E-INTERNAL-IR"
    );
    assert_eq!(frame.fuel_snapshot().used, 1);
    assert_eq!(frame.counters().operations_started, 1);
    assert_eq!(frame.active_scopes(), [ScopeId(0)]);
}

#[test]
fn invalid_hint_and_branch_condition_do_not_spend_failed_opcode_fuel() {
    let body = body();
    for (op, expected_code) in [
        (
            Op::SetHint {
                child: SlotId(0),
                hint: HintRecipe::Primitive(PrimitiveType::I64),
            },
            "LRR-E-INTERNAL-IR",
        ),
        (
            Op::BranchHint {
                parent: SlotId(0),
                condition: SlotId(0),
                yes: None,
                no: None,
                boolean: false,
            },
            "LRR-E-OP-TYPE",
        ),
    ] {
        let (mut store, mut frame) = setup(&body);
        let witness = recipe(
            &body,
            NodeId(0),
            Phase::Entry,
            vec![
                Op::ScopeEnter { scope: ScopeId(0) },
                Op::Unit {
                    destination: SlotId(0),
                },
                op,
            ],
            RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
        );
        let error = step(&mut frame, &mut store, &witness).unwrap_err();
        assert_eq!(error.code, expected_code);
        assert_eq!(frame.fuel_snapshot().used, 2);
        assert_eq!(frame.counters().operations_started, 2);
        assert_eq!(
            store.read_scalar(frame.cell(SlotId(0)).unwrap()).unwrap(),
            ValueRepr::unit()
        );
    }
}

fn local_body() -> BodyIndex {
    let blob = SourceBlob::new(b"fn live(){let x:i64;(1_i64,2_i64);}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([89; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap()
}
#[test]
fn uninitialized_local_read_preflight_spends_no_failed_fuel_or_output_handles() {
    let body = local_body();
    let binding = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Pattern)
        .unwrap()
        .id;
    let (mut store, mut frame) = setup(&body);
    let witness = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![
            Op::ScopeEnter { scope: ScopeId(0) },
            Op::DeclareLocal {
                binding,
                hint: HintRecipe::Primitive(PrimitiveType::I64),
                writable: false,
            },
            Op::ReadLocal {
                binding,
                destination: SlotId(0),
            },
        ],
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    assert_eq!(
        step(&mut frame, &mut store, &witness).unwrap_err().code,
        "LRR-E-OWNERSHIP"
    );
    assert_eq!(frame.fuel_snapshot().used, 2);
    assert_eq!(frame.counters().operations_started, 2);
    assert_eq!(store.budget().handles(), 2);
    assert!(frame.cell(SlotId(0)).is_err());
}
#[test]
fn immutable_assignment_preflight_preserves_rhs_and_allocations() {
    let body = local_body();
    let binding = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Pattern)
        .unwrap()
        .id;
    let e = expressions(&body);
    let (mut store, mut frame) = setup(&body);
    let literal = |i: usize, text: &str| Op::Literal {
        literal: LiteralRecipe {
            kind: LiteralKind::Integer,
            decoded: Some(cached_integer_fixture::integer(text)),
            spelling: text.into(),
            source: body.span(e[i].0).unwrap(),
        },
        hint: HintRecipe::None,
        destination: e[i].1,
    };
    let entry = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![
            Op::ScopeEnter { scope: ScopeId(0) },
            Op::DeclareLocal {
                binding,
                hint: HintRecipe::Primitive(PrimitiveType::I64),
                writable: false,
            },
            literal(0, "1_i64"),
            Op::AssignLocal {
                binding,
                source: e[0].1,
                destination: SlotId(0),
            },
            Op::Discard { source: SlotId(0) },
            literal(1, "2_i64"),
        ],
        RecipeExit::Next(body.pc(NodeId(0), Phase::Commit).unwrap()),
    );
    step(&mut frame, &mut store, &entry).unwrap();
    let budget = (store.budget().handles(), store.budget().bytes());
    let fuel = frame.fuel_snapshot().used;
    let rejected = recipe(
        &body,
        NodeId(0),
        Phase::Commit,
        vec![Op::AssignLocal {
            binding,
            source: e[1].1,
            destination: SlotId(0),
        }],
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    assert_eq!(
        step(&mut frame, &mut store, &rejected).unwrap_err().code,
        "LRR-E-OWNERSHIP"
    );
    assert_eq!(frame.fuel_snapshot().used, fuel);
    assert_eq!((store.budget().handles(), store.budget().bytes()), budget);
    assert_eq!(
        store.read_scalar(frame.cell(e[1].1).unwrap()).unwrap(),
        ValueRepr::signed(lrr::runtime::shape::TypeId(7), 2).unwrap()
    );
    assert!(frame.cell(SlotId(0)).is_err());
}

#[test]
fn invalid_branch_and_return_exit_do_not_charge_or_move_values() {
    let body = body();
    let e = expressions(&body);
    let (mut store, mut frame) = setup(&body);
    let branch = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![
            Op::ScopeEnter { scope: ScopeId(0) },
            Op::Unit {
                destination: SlotId(0),
            },
        ],
        RecipeExit::Branch {
            condition: SlotId(0),
            yes: body.pc(e[0].0, Phase::Entry).unwrap(),
            no: body.pc(e[1].0, Phase::Entry).unwrap(),
        },
    );
    assert_eq!(
        step(&mut frame, &mut store, &branch).unwrap_err().code,
        "LRR-E-INTERNAL-IR"
    );
    assert_eq!(frame.fuel_snapshot().used, 2);
    assert_eq!(frame.counters().exits_completed, 0);
    assert_eq!(
        store.read_scalar(frame.cell(SlotId(0)).unwrap()).unwrap(),
        ValueRepr::unit()
    );
    let (mut store, mut frame) = setup(&body);
    let returned = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![
            Op::ScopeEnter { scope: ScopeId(0) },
            Op::SetHint {
                child: SlotId(0),
                hint: HintRecipe::Primitive(PrimitiveType::I64),
            },
            Op::Literal {
                literal: LiteralRecipe {
                    kind: LiteralKind::Integer,
                    decoded: Some(cached_integer_fixture::integer("1_i32")),
                    spelling: "1_i32".into(),
                    source: body.span(e[0].0).unwrap(),
                },
                hint: HintRecipe::None,
                destination: e[0].1,
            },
        ],
        RecipeExit::Control(ControlRecipe::Return { value: e[0].1 }),
    );
    assert_eq!(
        step(&mut frame, &mut store, &returned).unwrap_err().code,
        "LRR-E-CALL-RETURN-TYPE"
    );
    assert_eq!(frame.fuel_snapshot().used, 4);
    assert_eq!(frame.counters().exits_completed, 0);
    assert_eq!(
        store.read_scalar(frame.cell(e[0].1).unwrap()).unwrap(),
        ValueRepr::signed(lrr::runtime::shape::TypeId(6), 1).unwrap()
    );
    assert_eq!(
        frame.active_scopes(),
        [
            ScopeId(0),
            body.nodes().iter().find_map(|n| n.statement_scope).unwrap()
        ]
    );
}
#[test]
fn inactive_continue_target_preflight_preserves_live_outer_scope() {
    let blob = SourceBlob::new(b"fn live(){loop{continue;}}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([91; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap();
    let node = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Continue))
        .unwrap();
    let target = node.enclosing_syntax_target.unwrap();
    let (mut store, mut frame) = setup(&body);
    let entry = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![
            Op::ScopeEnter { scope: ScopeId(0) },
            Op::Unit {
                destination: SlotId(0),
            },
        ],
        RecipeExit::Next(body.pc(node.id, Phase::Entry).unwrap()),
    );
    step(&mut frame, &mut store, &entry).unwrap();
    let continuation = recipe(
        &body,
        node.id,
        Phase::Entry,
        vec![Op::SetHint {
            child: node.value.unwrap(),
            hint: HintRecipe::None,
        }],
        RecipeExit::Control(ControlRecipe::Continue { target }),
    );
    assert_eq!(
        step(&mut frame, &mut store, &continuation)
            .unwrap_err()
            .code,
        "LRR-E-INTERNAL-IR"
    );
    assert_eq!(frame.fuel_snapshot().used, 4);
    assert_eq!(frame.counters().exits_completed, 1);
    assert_eq!(frame.active_scopes(), [ScopeId(0)]);
    assert_eq!(
        store.read_scalar(frame.cell(SlotId(0)).unwrap()).unwrap(),
        ValueRepr::unit()
    );
}

#[test]
fn rejected_break_result_preserves_value_and_crossed_scopes() {
    let blob = SourceBlob::new(b"fn live(){loop{break 1_i32;}}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([93; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap();
    let breaking = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Break))
        .unwrap();
    let literal = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Literal))
        .unwrap();
    let target = breaking.enclosing_syntax_target.unwrap();
    let loop_node = &body.nodes()[body.labels()[target.0 as usize].node.0 as usize];
    let tail = body.nodes()[0].tail_scope.unwrap();
    let (mut store, mut frame) = setup(&body);
    let witness = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![
            Op::ScopeEnter { scope: ScopeId(0) },
            Op::ScopeEnter { scope: tail },
            Op::SetHint {
                child: loop_node.value.unwrap(),
                hint: HintRecipe::Primitive(PrimitiveType::I64),
            },
            Op::ScopeEnter {
                scope: loop_node.scope,
            },
            Op::ScopeEnter {
                scope: body.scopes()[breaking.scope.0 as usize].parent.unwrap(),
            },
            Op::ScopeEnter {
                scope: breaking.scope,
            },
            Op::Literal {
                literal: LiteralRecipe {
                    kind: LiteralKind::Integer,
                    decoded: Some(cached_integer_fixture::integer("1_i32")),
                    spelling: "1_i32".into(),
                    source: body.span(literal.id).unwrap(),
                },
                hint: HintRecipe::None,
                destination: literal.value.unwrap(),
            },
        ],
        RecipeExit::Next(body.pc(breaking.id, Phase::Entry).unwrap()),
    );
    step(&mut frame, &mut store, &witness).unwrap();
    let before = (store.budget().handles(), store.budget().bytes());
    let fuel = frame.fuel_snapshot().used;
    let rejected = recipe(
        &body,
        breaking.id,
        Phase::Entry,
        vec![Op::SetHint {
            child: breaking.value.unwrap(),
            hint: HintRecipe::None,
        }],
        RecipeExit::Control(ControlRecipe::Break {
            target,
            value: Some(literal.value.unwrap()),
            resume: body.pc(NodeId(0), Phase::Commit).unwrap(),
        }),
    );
    assert_eq!(
        step(&mut frame, &mut store, &rejected).unwrap_err().code,
        "LRR-E-OWNERSHIP"
    );
    assert_eq!(frame.fuel_snapshot().used, fuel + 1);
    assert_eq!((store.budget().handles(), store.budget().bytes()), before);
    assert_eq!(
        frame.active_scopes(),
        [
            ScopeId(0),
            tail,
            loop_node.scope,
            body.scopes()[breaking.scope.0 as usize].parent.unwrap(),
            breaking.scope
        ]
    );
    assert_eq!(
        store
            .read_scalar(frame.cell(literal.value.unwrap()).unwrap())
            .unwrap(),
        ValueRepr::signed(lrr::runtime::shape::TypeId(6), 1).unwrap()
    );
    assert!(frame.cell(loop_node.value.unwrap()).is_err());
}

#[test]
fn invalid_literal_spelling_suffix_and_range_precede_fuel_and_guest_allocation() {
    for (kind, spelling, decoded, code) in [
        (
            LiteralKind::Integer,
            "not_an_integer",
            Some(lrr::ir::op::DecodedLiteral::InvalidInteger {
                negative: false,
                radix: 10,
                suffix: None,
                reason: lrr::ir::op::IntegerLexicalError::InvalidMagnitude,
            }),
            "LRR-E-INTERNAL-IR",
        ),
        (
            LiteralKind::Integer,
            "1_i128",
            Some(cached_integer_fixture::integer("1_i128")),
            "LRR-E-UNSUPPORTED",
        ),
        (
            LiteralKind::Integer,
            "128_i8",
            Some(cached_integer_fixture::integer("128_i8")),
            "LRR-E-VALUE-REPR",
        ),
        (
            LiteralKind::Float,
            "1e500_f64",
            Some(lrr::ir::op::DecodedLiteral::Float {
                decimal: "1e500".into(),
                suffix: Some(PrimitiveType::F64),
            }),
            "LRR-E-VALUE-REPR",
        ),
    ] {
        let body = body();
        let e = expressions(&body);
        let (mut store, mut frame) = setup(&body);
        let witness = recipe(
            &body,
            NodeId(0),
            Phase::Entry,
            vec![
                Op::ScopeEnter { scope: ScopeId(0) },
                Op::Literal {
                    literal: LiteralRecipe {
                        kind,
                        decoded,
                        spelling: spelling.into(),
                        source: body.span(e[0].0).unwrap(),
                    },
                    hint: HintRecipe::None,
                    destination: e[0].1,
                },
            ],
            RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
        );
        assert_eq!(
            step(&mut frame, &mut store, &witness).unwrap_err().code,
            code
        );
        assert_eq!(frame.fuel_snapshot().used, 2);
        assert_eq!(frame.counters().operations_started, 2);
        assert_eq!(store.budget().handles(), 1);
        assert!(frame.cell(e[0].1).is_err());
    }
}

#[test]
fn caller_and_callee_frames_consume_one_fuel_budget_without_replenishment() {
    let body = body();
    let (mut store, mut caller) = setup(&body);
    caller.set_fuel_limit(6).unwrap();
    let pc = body.pc(NodeId(0), Phase::Entry).unwrap();
    let tick = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![Op::SetHint {
            child: expressions(&body)[0].1,
            hint: HintRecipe::Primitive(PrimitiveType::I64),
        }],
        RecipeExit::Next(pc.clone()),
    );
    step(&mut caller, &mut store, &tick).unwrap();
    let mut callee = Frame::new_in_run(body.clone(), pc, &mut store, &mut caller).unwrap();
    assert!(callee.set_fuel_limit(100).is_err());
    assert_eq!(callee.fuel_snapshot(), caller.fuel_snapshot());
    step(&mut callee, &mut store, &tick).unwrap();
    step(&mut caller, &mut store, &tick).unwrap();
    assert_eq!(callee.fuel_snapshot().used, 6);
    assert_eq!(caller.fuel_snapshot().remaining, 0);
    assert_eq!(
        step(&mut callee, &mut store, &tick).unwrap_err().code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert_eq!(caller.fuel_snapshot(), callee.fuel_snapshot());
    assert_eq!(
        caller.fuel_snapshot().stop,
        Some(lrr::vm::fuel::FuelStop::Exhausted)
    );
    assert_eq!(caller.counters().exits_completed, 2);
    assert_eq!(callee.counters().exits_completed, 1);
    assert_eq!(callee.status(), FrameStatus::Faulted);
}

fn panic_body() -> (BodyIndex, NodeId) {
    let blob = SourceBlob::new(b"fn live(){panic!(\"owned guest message\");}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([78; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap();
    let node = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Macro))
        .unwrap()
        .id;
    (body, node)
}
#[test]
fn explicit_guest_panic_preserves_owned_message_origin_and_terminal_counts() {
    let (body, node) = panic_body();
    let (mut store, mut frame) = setup(&body);
    let pc = body.pc(node, Phase::Entry).unwrap();
    let enter = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![Op::ScopeEnter { scope: ScopeId(0) }],
        RecipeExit::Next(pc.clone()),
    );
    step(&mut frame, &mut store, &enter).unwrap();
    let panic = recipe(
        &body,
        node,
        Phase::Entry,
        vec![Op::Panic {
            message: "owned guest message 🦀".into(),
        }],
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    let fingerprint = panic.recipe().fingerprint();
    let StepOutcome::GuestPanic(report) = step(&mut frame, &mut store, &panic).unwrap() else {
        panic!("expected guest panic")
    };
    assert_eq!(report.description(), "owned guest message 🦀");
    assert_eq!(report.kind, lrr::vm::numeric::PanicKind::Explicit);
    assert_eq!(report.pc, pc);
    assert_eq!(report.origin.span, body.span(node).unwrap());
    assert_eq!(frame.status(), FrameStatus::Panicked);
    assert_eq!(
        frame.active_scopes(),
        [
            ScopeId(0),
            body.nodes().iter().find_map(|n| n.statement_scope).unwrap()
        ]
    );
    assert_eq!(frame.counters().started_by_opcode[25], 1);
    assert_eq!(frame.counters().operations_started, 3);
    assert_eq!(frame.counters().operations_completed, 2);
    assert_eq!(frame.counters().exits_completed, 1);
    assert_eq!(frame.fuel_snapshot().used, 4);
    assert!(step(&mut frame, &mut store, &panic).is_err());
    assert_eq!(frame.counters().started_by_opcode[25], 1);
    let changed = recipe(
        &body,
        node,
        Phase::Entry,
        vec![Op::Panic {
            message: "different".into(),
        }],
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    assert_ne!(fingerprint, changed.recipe().fingerprint());
    frame.finish_panic(&mut store).unwrap();
    assert_eq!(frame.status(), FrameStatus::PanicCleaned);
    assert!(frame.active_scopes().is_empty());
    assert_eq!(frame.counters().panic_scopes_cleaned, 2);
    assert!(frame.finish_panic(&mut store).is_err());
    assert_eq!(frame.counters().panic_scopes_cleaned, 2);
}
#[test]
fn explicit_guest_panic_fuel_refusal_preserves_unstarted_message_and_scope() {
    let (body, node) = panic_body();
    let (mut store, mut frame) = setup(&body);
    frame.set_fuel_limit(3).unwrap();
    let enter = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![Op::ScopeEnter { scope: ScopeId(0) }],
        RecipeExit::Next(body.pc(node, Phase::Entry).unwrap()),
    );
    step(&mut frame, &mut store, &enter).unwrap();
    let panic = recipe(
        &body,
        node,
        Phase::Entry,
        vec![Op::Panic {
            message: "must not start".into(),
        }],
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    assert_eq!(
        step(&mut frame, &mut store, &panic).unwrap_err().code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert_eq!(frame.status(), FrameStatus::Faulted);
    assert_eq!(frame.counters().started_by_opcode[25], 0);
    assert_eq!(frame.fuel_snapshot().used, 3);
    assert_eq!(
        frame.active_scopes(),
        [
            ScopeId(0),
            body.nodes().iter().find_map(|n| n.statement_scope).unwrap()
        ]
    );
}
#[test]
fn explicit_guest_panic_admission_rejects_oversized_message_and_wrong_origin() {
    let (body, node) = panic_body();
    for (selected, message) in [(node, "x".repeat(65_537)), (NodeId(0), "bounded".into())] {
        assert!(
            verify(
                RecipeDraft {
                    key: body.pc(selected, Phase::Entry).unwrap(),
                    origin: Origin {
                        span: body.span(selected).unwrap(),
                        expansion_ancestry: vec![]
                    },
                    operations: vec![Op::Panic { message }],
                    exits: vec![RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap())],
                    dependencies: vec![]
                },
                &body
            )
            .is_err()
        );
    }
}

fn assertion_setup(
    spelling: &str,
    kind: LiteralKind,
    fuel_limit: Option<u64>,
) -> (BodyIndex, NodeId, SlotId, SlotId, GuestStore, Frame) {
    let blob = SourceBlob::new(b"fn live(){let _=(true,assert!(true));}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([79; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap();
    let literal = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Literal))
        .unwrap()
        .id;
    let node = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Macro))
        .unwrap()
        .id;
    let condition = body.nodes()[literal.0 as usize].value.unwrap();
    let destination = body.nodes()[node.0 as usize].value.unwrap();
    let (mut store, mut frame) = setup(&body);
    if let Some(limit) = fuel_limit {
        frame.set_fuel_limit(limit).unwrap();
    }
    let enter = recipe(
        &body,
        NodeId(0),
        Phase::Entry,
        vec![Op::ScopeEnter { scope: ScopeId(0) }],
        RecipeExit::Next(body.pc(literal, Phase::Entry).unwrap()),
    );
    step(&mut frame, &mut store, &enter).unwrap();
    let value = recipe(
        &body,
        literal,
        Phase::Entry,
        vec![Op::Literal {
            literal: LiteralRecipe {
                decoded: match kind {
                    LiteralKind::Integer => Some(cached_integer_fixture::integer(spelling)),
                    LiteralKind::Bool => None,
                    _ => panic!("assertion fixture has no other literal kind"),
                },
                kind,
                spelling: spelling.into(),
                source: body.span(literal).unwrap(),
            },
            hint: HintRecipe::None,
            destination: condition,
        }],
        RecipeExit::Next(body.pc(node, Phase::Entry).unwrap()),
    );
    step(&mut frame, &mut store, &value).unwrap();
    (body, node, condition, destination, store, frame)
}
#[test]
fn guest_assertion_true_returns_unit_and_false_panics_without_consuming_condition() {
    for (spelling, expected) in [("true", 1), ("false", 0)] {
        let (body, node, condition, destination, mut store, mut frame) =
            assertion_setup(spelling, LiteralKind::Bool, None);
        let before = store.read_scalar(frame.cell(condition).unwrap()).unwrap();
        let next = body.pc(NodeId(0), Phase::Exit).unwrap();
        let assertion = recipe(
            &body,
            node,
            Phase::Entry,
            vec![Op::Assert {
                condition,
                destination,
                message: "assertion failed: source condition".into(),
            }],
            RecipeExit::Next(next.clone()),
        );
        let outcome = step(&mut frame, &mut store, &assertion).unwrap();
        assert_eq!(
            store.read_scalar(frame.cell(condition).unwrap()).unwrap(),
            before
        );
        assert_eq!(before.payload, expected);
        assert_eq!(frame.counters().started_by_opcode[26], 1);
        if expected == 1 {
            assert!(matches!(outcome,StepOutcome::Next(p) if p==next));
            assert_eq!(
                store.read_scalar(frame.cell(destination).unwrap()).unwrap(),
                ValueRepr::unit()
            );
            assert_eq!(frame.counters().operations_completed, 4);
            assert_eq!(frame.counters().exits_completed, 3);
            assert_eq!(frame.fuel_snapshot().used, 7);
        } else {
            let StepOutcome::GuestPanic(panic) = outcome else {
                panic!("false assertion must panic")
            };
            assert_eq!(panic.kind, lrr::vm::numeric::PanicKind::Assertion);
            assert_eq!(panic.description(), "assertion failed: source condition");
            assert_eq!(panic.origin.span, body.span(node).unwrap());
            assert_eq!(
                frame.cell(destination).unwrap_err().message,
                "VM slot has no executed storage"
            );
            assert_eq!(frame.counters().operations_completed, 3);
            assert_eq!(frame.counters().exits_completed, 2);
            assert_eq!(frame.fuel_snapshot().used, 6);
            frame.finish_panic(&mut store).unwrap();
            assert_eq!(frame.counters().panic_scopes_cleaned, 2);
            assert!(frame.active_scopes().is_empty());
        }
    }
}
#[test]
fn guest_assertion_type_and_fuel_faults_preserve_operand_and_unstarted_opcode() {
    for (spelling, kind, code) in [
        ("7i64", LiteralKind::Integer, "LRR-E-OP-TYPE"),
        ("false", LiteralKind::Bool, "LRR-E-RUNTIME-RESOURCE"),
    ] {
        let (body, node, condition, destination, mut store, mut frame) = assertion_setup(
            spelling,
            kind,
            if kind == LiteralKind::Bool {
                Some(5)
            } else {
                None
            },
        );
        let before = store.read_scalar(frame.cell(condition).unwrap()).unwrap();
        let fuel = frame.fuel_snapshot().used;
        let assertion = recipe(
            &body,
            node,
            Phase::Entry,
            vec![Op::Assert {
                condition,
                destination,
                message: "unstarted".into(),
            }],
            RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
        );
        assert_eq!(
            step(&mut frame, &mut store, &assertion).unwrap_err().code,
            code
        );
        assert_eq!(frame.status(), FrameStatus::Faulted);
        assert_eq!(
            store.read_scalar(frame.cell(condition).unwrap()).unwrap(),
            before
        );
        assert_eq!(
            frame.cell(destination).unwrap_err().message,
            "VM slot has no executed storage"
        );
        assert_eq!(frame.counters().started_by_opcode[26], 0);
        assert_eq!(frame.fuel_snapshot().used, fuel);
        assert_eq!(
            frame.active_scopes(),
            [
                ScopeId(0),
                body.nodes().iter().find_map(|n| n.statement_scope).unwrap()
            ]
        );
    }
}
#[test]
fn guest_assertion_admission_checks_origin_slots_message_budget_and_stable_identity() {
    let (body, node, condition, destination, _, _) =
        assertion_setup("true", LiteralKind::Bool, None);
    let build = |selected, condition, destination, message| {
        verify(
            RecipeDraft {
                key: body.pc(selected, Phase::Entry).unwrap(),
                origin: Origin {
                    span: body.span(selected).unwrap(),
                    expansion_ancestry: vec![],
                },
                operations: vec![Op::Assert {
                    condition,
                    destination,
                    message,
                }],
                exits: vec![RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap())],
                dependencies: vec![],
            },
            &body,
        )
    };
    assert!(build(NodeId(0), condition, destination, "message".into()).is_err());
    assert!(build(node, condition, condition, "message".into()).is_err());
    assert!(build(node, SlotId(u32::MAX), destination, "message".into()).is_err());
    assert!(build(node, condition, destination, "x".repeat(65_537)).is_err());
    assert_ne!(
        build(node, condition, destination, "first".into())
            .unwrap()
            .recipe()
            .fingerprint(),
        build(node, condition, destination, "second".into())
            .unwrap()
            .recipe()
            .fingerprint()
    );
}
