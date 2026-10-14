#[path = "support/cached_integer_fixture.rs"]
mod cached_integer_fixture;
use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{
        op::{
            HintRecipe, LiteralKind, LiteralRecipe, NumericBinary, NumericUnary, Op, PrimitiveType,
        },
        pc::{NodeId, Phase, SlotId},
        recipe::{ControlRecipe, Origin, RecipeDraft, RecipeExit, VerifiedRecipe},
        validate::verify,
    },
    runtime::{
        cell::{CellState, GuestStore},
        shape::TypeId,
        value::ValueRepr,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole, ScopeId},
        parse::ParsedFile,
    },
    vm::{
        frame::{Frame, FrameStatus},
        numeric::PanicKind,
        step::{StepOutcome, step},
    },
};
fn body() -> BodyIndex {
    let blob = SourceBlob::new(b"fn live(){let _=(127_i8,1_i8,0_i8);}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([86; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap()
}
fn recipe(body: &BodyIndex, op: Op, left: &str, right: &str, exit: RecipeExit) -> VerifiedRecipe {
    let literals: Vec<_> = body
        .nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Literal))
        .collect();
    let mut operations = vec![
        Op::ScopeEnter { scope: ScopeId(0) },
        Op::ScopeEnter {
            scope: literals[0].scope,
        },
    ];
    for (node, spelling) in [(literals[0], left), (literals[1], right)] {
        operations.push(Op::Literal {
            literal: LiteralRecipe {
                decoded: Some(cached_integer_fixture::integer(spelling)),
                kind: LiteralKind::Integer,
                spelling: spelling.into(),
                source: body.span(node.id).unwrap(),
            },
            hint: HintRecipe::None,
            destination: node.value.unwrap(),
        })
    }
    operations.push(op);
    verify(
        RecipeDraft {
            key: body.pc(NodeId(0), Phase::Entry).unwrap(),
            origin: Origin {
                span: body.span(NodeId(0)).unwrap(),
                expansion_ancestry: vec![],
            },
            operations,
            exits: vec![exit],
            dependencies: vec![],
        },
        body,
    )
    .unwrap()
}
fn slots(body: &BodyIndex) -> Vec<SlotId> {
    body.nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Literal))
        .map(|node| node.value.unwrap())
        .collect()
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
fn numeric_guest_panic_preserves_origin_stops_the_recipe_and_runs_scalar_cleanup_once() {
    let body = body();
    let slots = slots(&body);
    for (operator, right, kind) in [
        (NumericBinary::Add, "1_i8", PanicKind::Overflow),
        (NumericBinary::Div, "0_i8", PanicKind::DivisionByZero),
        (NumericBinary::Shl, "8_u32", PanicKind::InvalidShift),
    ] {
        let mut verified = recipe(
            &body,
            Op::NumericBinary {
                operator,
                left: slots[0],
                right: slots[1],
                destination: SlotId(0),
            },
            "127_i8",
            right,
            RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
        );
        // Add a following operation that must not start after the panic.
        let mut draft = RecipeDraft {
            key: verified.recipe().key.clone(),
            origin: verified.recipe().origin.clone(),
            operations: verified.recipe().operations.to_vec(),
            exits: vec![verified.recipe().exit.clone()],
            dependencies: vec![],
        };
        draft.operations.push(Op::Unit {
            destination: slots[2],
        });
        verified = verify(draft, &body).unwrap();
        let (mut store, mut frame) = setup(&body);
        let StepOutcome::GuestPanic(panic) = step(&mut frame, &mut store, &verified).unwrap()
        else {
            panic!("arithmetic panic was not typed guest control flow")
        };
        assert_eq!(panic.kind, kind);
        assert_eq!(panic.pc, verified.recipe().key);
        assert_eq!(panic.origin, verified.recipe().origin);
        assert_eq!(frame.status(), FrameStatus::Panicked);
        assert_eq!(frame.counters().operations_started, 5);
        assert_eq!(frame.counters().operations_completed, 4);
        assert_eq!(frame.counters().exits_completed, 0);
        assert!(frame.cell(slots[2]).is_err());
        let left = frame.cell(slots[0]).unwrap();
        assert!(matches!(store.state(left).unwrap(), CellState::Live(_)));
        let output = frame.cell(SlotId(0)).unwrap();
        assert_eq!(store.state(output).unwrap(), CellState::Uninit);
        assert!(step(&mut frame, &mut store, &verified).is_err());
        frame.finish_panic(&mut store).unwrap();
        assert_eq!(frame.status(), FrameStatus::PanicCleaned);
        assert!(frame.active_scopes().is_empty());
        assert_eq!(store.state(left).unwrap(), CellState::Dropped);
        assert_eq!(store.state(output).unwrap(), CellState::Dropped);
        assert_eq!(frame.counters().panic_scopes_cleaned, 2);
        assert!(frame.finish_panic(&mut store).is_err());
    }
}
#[test]
fn numeric_runtime_fault_preserves_live_storage_and_cannot_enter_panic_cleanup() {
    let body = body();
    let slots = slots(&body);
    let verified = recipe(
        &body,
        Op::NumericBinary {
            operator: NumericBinary::Add,
            left: slots[0],
            right: slots[1],
            destination: SlotId(0),
        },
        "127_i8",
        "1_u8",
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    let (mut store, mut frame) = setup(&body);
    assert_eq!(
        step(&mut frame, &mut store, &verified).unwrap_err().code,
        "LRR-E-OP-TYPE"
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
        store.read_scalar(frame.cell(slots[0]).unwrap()).unwrap(),
        ValueRepr::signed(TypeId(4), 127).unwrap()
    );
    assert!(frame.finish_panic(&mut store).is_err());
    assert_eq!(frame.counters().panic_scopes_cleaned, 0);
}
#[test]
fn numeric_unary_and_cast_recipes_commit_actual_values_and_keep_opcode_identity() {
    let body = body();
    let slots = slots(&body);
    for (op, expected, tag) in [
        (
            Op::NumericUnary {
                operator: NumericUnary::Neg,
                source: slots[0],
                destination: SlotId(0),
            },
            ValueRepr::signed(TypeId(4), -127).unwrap(),
            11,
        ),
        (
            Op::PrimitiveCast {
                source: slots[0],
                target: PrimitiveType::U8,
                destination: SlotId(0),
            },
            ValueRepr::unsigned(TypeId(9), 127).unwrap(),
            12,
        ),
    ] {
        let verified = recipe(
            &body,
            op,
            "127_i8",
            "1_i8",
            RecipeExit::Control(ControlRecipe::Return { value: SlotId(0) }),
        );
        let (mut store, mut frame) = setup(&body);
        let StepOutcome::Return(ticket) = step(&mut frame, &mut store, &verified).unwrap() else {
            panic!("missing numeric result ticket")
        };
        assert_eq!(store.peek_ticket(ticket).unwrap(), expected);
        assert_eq!(frame.counters().operations_completed, 5);
        assert_eq!(frame.counters().started_by_opcode[tag], 1);
        frame.finish_return(&mut store).unwrap();
        store.discard(ticket).unwrap();
    }
}
#[test]
fn numeric_recipe_admission_rejects_operand_aliases_wrong_kinds_and_missing_slots() {
    let body = body();
    let slots = slots(&body);
    for op in [
        Op::NumericBinary {
            operator: NumericBinary::Add,
            left: slots[0],
            right: slots[1],
            destination: slots[0],
        },
        Op::NumericUnary {
            operator: NumericUnary::Not,
            source: SlotId(1),
            destination: SlotId(0),
        },
        Op::PrimitiveCast {
            source: SlotId(u32::MAX),
            target: PrimitiveType::I64,
            destination: SlotId(0),
        },
    ] {
        let result = verify(
            RecipeDraft {
                key: body.pc(NodeId(0), Phase::Entry).unwrap(),
                origin: Origin {
                    span: body.span(NodeId(0)).unwrap(),
                    expansion_ancestry: vec![],
                },
                operations: vec![op],
                exits: vec![RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap())],
                dependencies: vec![],
            },
            &body,
        );
        assert_eq!(result.unwrap_err().code, "LRR-E-INTERNAL-IR");
    }
}

#[test]
fn numeric_type_faults_precede_fuel_and_result_storage_reservation() {
    let body = body();
    let slots = slots(&body);
    let verified = recipe(
        &body,
        Op::NumericBinary {
            operator: NumericBinary::Add,
            left: slots[0],
            right: slots[1],
            destination: SlotId(0),
        },
        "127_i8",
        "1_u8",
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    let (mut store, mut frame) = setup(&body);
    assert_eq!(
        step(&mut frame, &mut store, &verified).unwrap_err().code,
        "LRR-E-OP-TYPE"
    );
    assert_eq!(frame.fuel_snapshot().used, 4);
    assert_eq!(frame.counters().operations_started, 4);
    assert!(frame.cell(SlotId(0)).is_err());
}

#[test]
fn rejected_numeric_operator_does_not_bind_previewed_shared_literal_width() {
    let body = body();
    let slots = slots(&body);
    let initial = recipe(
        &body,
        Op::NumericUnary {
            operator: NumericUnary::Neg,
            source: slots[0],
            destination: SlotId(0),
        },
        "1",
        "0_u8",
        RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap()),
    );
    let mut operations = initial.recipe().operations.to_vec();
    operations.insert(
        3,
        Op::SetHint {
            child: SlotId(0),
            hint: HintRecipe::Primitive(PrimitiveType::U8),
        },
    );
    let verified = verify(
        RecipeDraft {
            key: initial.recipe().key.clone(),
            origin: initial.recipe().origin.clone(),
            operations,
            exits: vec![initial.recipe().exit.clone()],
            dependencies: vec![],
        },
        &body,
    )
    .unwrap();
    let (mut store, mut frame) = setup(&body);
    assert_eq!(
        step(&mut frame, &mut store, &verified).unwrap_err().code,
        "LRR-E-OP-TYPE"
    );
    assert_eq!(frame.fuel_snapshot().used, 5);
    let cell = frame.cell(slots[0]).unwrap();
    assert_eq!(
        store.read_scalar(cell).unwrap_err().code,
        "LRR-E-TYPE-CONTEXT"
    );
    let alias = store.copy_from(cell).unwrap();
    let value = store.materialize_ticket(alias, TypeId(7)).unwrap();
    assert_eq!(
        store.peek_ticket(value).unwrap(),
        ValueRepr::signed(TypeId(7), 1).unwrap()
    );
    store.discard(value).unwrap();
    store.discard(alias).unwrap();
}
