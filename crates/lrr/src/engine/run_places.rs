//! Original rustc expression controls. Disclosed entry adaptations stay outside
//! the licensed source and never count as complete compiletest fixture passes.
use super::borrows::tests::{next, start_stock};
use super::*;
use crate::{
    ir::op::PrimitiveType,
    runtime::{
        cell::CellId,
        place::Place,
        reference::Access,
        shape::{TypeId, TypeKey},
    },
    syntax::{place_fixture_facts as facts, spans::SourceRange},
    traits::term::{ConstTerm, TyTerm},
};

const BOUNDS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lint/unconditional_panic_98444.rs"
);
const TUPLES: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/tuple/index-float.rs");
const ARRAY_BORROW: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-fixed-length-vecs.rs"
);

fn original_element_borrow_at_commit() -> (Database, RunMachine, crate::db::memo::Outcome) {
    let (mut db, mut run) = start_stock(ARRAY_BORROW);
    let len = u32::try_from(facts::array_binding(ARRAY_BORROW).len()).unwrap();
    for _ in 0..100 {
        let outcome = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(run.frame.pc().clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("original element borrow thunk absent")
        };
        if let Some(guard) = recipe.recipe().operations.iter().find_map(|op| match op {
            Op::ConstructStart { guard } => Some(*guard),
            _ => None,
        }) {
            let result = run
                .frame
                .body
                .nodes()
                .iter()
                .find(|node| node.constructor_guard == Some(guard))
                .unwrap()
                .value
                .unwrap();
            let array = run
                .store
                .intern_type(TypeKey::Array {
                    element: TypeId(6),
                    len,
                })
                .unwrap();
            // Explicit source-bound worker context; the unchanged unannotated
            // array's full inferred source typing remains later M017 work.
            run.frame.set_hint(result, Some(array)).unwrap();
            run.frame
                .set_source_view(
                    result,
                    Some(TyTerm::Array(
                        Box::new(TyTerm::Primitive(PrimitiveType::I32)),
                        ConstTerm::Value(u64::from(len)),
                    )),
                )
                .unwrap();
            run.prepare_constructor(&mut db, &[])
                .into_ready()
                .unwrap()
                .unwrap();
        }
        if recipe
            .recipe()
            .operations
            .iter()
            .any(|op| matches!(op, Op::Borrow { .. }))
        {
            return (db, run, outcome);
        }
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
    }
    panic!("original &x[0] did not reach Borrow");
}
#[test]
fn rustc_original_array_element_borrow_enters_once_without_reading_the_referent() {
    let (mut db, mut run, outcome) = original_element_borrow_at_commit();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("element borrow thunk absent")
    };
    let destination = recipe
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::Borrow { destination, .. } => Some(*destination),
            _ => None,
        })
        .unwrap();
    assert_eq!(run.frame.counters.started_by_opcode[36], 1);
    assert_eq!(run.frame.counters.started_by_opcode[33], 0);
    assert_eq!(run.frame.counters.started_by_opcode[8], 0);
    assert!(run.frame.projection_bytes_for_test() > 0);
    assert!(matches!(
        step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
        StepOutcome::Next(_)
    ));
    let reference = run
        .store
        .reference_from_cell(run.frame.cell(destination).unwrap())
        .unwrap();
    let value = run.store.read_reference(reference).unwrap();
    assert_eq!(value.type_id, 6);
    assert_eq!(value.payload as i64, facts::array_binding(ARRAY_BORROW)[0]);
    assert!(
        matches!(run.frame.source_view(destination).unwrap(), Some(TyTerm::Ref(_, false, element)) if element.as_ref() == &TyTerm::Primitive(PrimitiveType::I32))
    );
    assert_eq!(run.frame.counters.started_by_opcode[31], 1);
    assert_eq!(run.frame.projection_bytes_for_test(), 0);
    // Stops after the original borrow; original assertion and full run-pass pending.
    db.finish_run().unwrap();
}
#[test]
fn rustc_projected_borrow_source_mismatch_refuses_before_lease_publication() {
    let (mut db, mut run, outcome) = original_element_borrow_at_commit();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("element borrow thunk absent")
    };
    let place = recipe
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::Borrow { place, .. } => Some(*place),
            _ => None,
        })
        .unwrap();
    run.frame
        .set_source_view(place, Some(TyTerm::Primitive(PrimitiveType::I64)))
        .unwrap();
    let before = (
        run.frame.fuel.snapshot().used,
        run.store.budget().handles(),
        run.store.budget().bytes(),
        run.frame.projection_bytes_for_test(),
    );
    assert_eq!(
        step::step(&mut run.frame, &mut run.store, recipe)
            .unwrap_err()
            .code,
        "LRR-E-OWNERSHIP"
    );
    assert_eq!(
        (
            run.frame.fuel.snapshot().used,
            run.store.budget().handles(),
            run.store.budget().bytes(),
            run.frame.projection_bytes_for_test()
        ),
        before
    );
    assert_eq!(run.frame.counters.started_by_opcode[31], 0);
    db.finish_run().unwrap();
}

fn original_array_prefix() -> (Database, RunMachine, CellId) {
    let (mut db, mut run) = start_stock(BOUNDS);
    for _ in 0..100 {
        let outcome = next(&mut db, &mut run);
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("original array thunk absent")
        };
        let bound = recipe.recipe().operations.iter().find_map(|op| match op {
            Op::BindLocal { binding, .. } => Some(*binding),
            _ => None,
        });
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        if let Some(binding) = bound {
            let cell = run.frame.binding_cell_for_test(binding).unwrap();
            return (db, run, cell);
        }
    }
    panic!("original array binding never executed");
}
fn enter_original_expression(
    run: &mut RunMachine,
    range: SourceRange,
) -> crate::syntax::body_index::StructuralNode {
    let node = run
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| {
            node.range == range
                && matches!(
                    node.role,
                    crate::syntax::body_index::NodeRole::Expression(_)
                )
        })
        .unwrap()
        .clone();
    let mut scopes = Vec::new();
    let mut scope = Some(node.scope);
    while let Some(id) = scope {
        scopes.push(id);
        scope = run.frame.body.scopes()[id.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        if !run.frame.active_scopes().contains(&scope) {
            run.frame.enter(scope).unwrap();
        }
    }
    run.frame.pc = run.frame.body.pc(node.id, Phase::Entry).unwrap();
    node
}

fn original_whole_array_borrow_at_commit()
-> (Database, RunMachine, CellId, crate::db::memo::Outcome) {
    let (mut db, mut run, array) = original_array_prefix();
    let ranges = facts::borrow_ranges_of(BOUNDS, "xs");
    assert_eq!(ranges.len(), 1);
    // Enter the original borrow inside `let _`. Wildcard statement admission
    // remains a separate pending requirement, not a passing source rewrite.
    enter_original_expression(&mut run, ranges[0]);
    for _ in 0..20 {
        let outcome = next(&mut db, &mut run);
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("borrow thunk absent")
        };
        if recipe
            .recipe()
            .operations
            .iter()
            .any(|op| matches!(op, Op::Borrow { .. }))
        {
            return (db, run, array, outcome);
        }
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
    }
    panic!("original whole array borrow absent");
}

#[test]
fn rustc_original_whole_array_borrow_retains_source_length_and_original_owner() {
    let (mut db, mut run, array, outcome) = original_whole_array_borrow_at_commit();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("borrow thunk absent")
    };
    let destination = recipe
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::Borrow { destination, .. } => Some(*destination),
            _ => None,
        })
        .unwrap();
    let epoch = run.store.location_epoch(array).unwrap();
    let original = run.store.admit_move(array).unwrap();
    let actual_type = TypeId(original.type_id);
    let values = facts::array_binding(BOUNDS);
    assert!(matches!(
        step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
        StepOutcome::Next(_)
    ));
    assert_eq!(run.store.admit_move(array).unwrap(), original);
    assert_eq!(run.store.location_epoch(array).unwrap(), epoch);
    let reference = run
        .store
        .reference_from_cell(run.frame.cell(destination).unwrap())
        .unwrap();
    let target = Place::deref(reference);
    assert_eq!(run.store.place_type_id(&target).unwrap(), actual_type);
    for (index, expected) in values.iter().enumerate() {
        let field = run
            .store
            .project_index(
                &target,
                ValueRepr::unsigned(TypeId(13), u64::try_from(index).unwrap()).unwrap(),
            )
            .unwrap();
        assert_eq!(
            run.store.read_place(&field).unwrap(),
            ValueRepr::signed(TypeId(6), *expected).unwrap()
        );
    }
    assert!(
        matches!(run.frame.source_view(destination).unwrap(), Some(TyTerm::Ref(_, false, array))
        if array.as_ref() == &TyTerm::Array(Box::new(TyTerm::Primitive(PrimitiveType::I32)),
            ConstTerm::Value(u64::try_from(values.len()).unwrap())))
    );
    assert_eq!(run.frame.counters.started_by_opcode[31], 1);
    assert_eq!(run.frame.counters.started_by_opcode[8], 0);
    assert_eq!(run.frame.projection_bytes_for_test(), 0);
    // Stops after original &xs; wildcard binding, later OOB, compiler lint
    // diagnostics and ordinary aggregate cleanup are not exercised here.
    db.finish_run().unwrap();
}

#[test]
fn rustc_whole_array_borrow_element_or_length_mismatch_refuses_before_borrow_fuel() {
    let length = u64::try_from(facts::array_binding(BOUNDS).len()).unwrap();
    for (primitive, length) in [
        (PrimitiveType::Isize, length),
        (PrimitiveType::I32, length.checked_add(1).unwrap()),
    ] {
        let (mut db, mut run, array, outcome) = original_whole_array_borrow_at_commit();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("borrow thunk absent")
        };
        let binding = recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| match op {
                Op::MakePlace { binding, .. } => Some(*binding),
                _ => None,
            })
            .unwrap();
        run.frame
            .set_binding_source(
                binding,
                &crate::ir::op::HintRecipe::ArrayPrimitive {
                    element: primitive,
                    len: u32::try_from(length).unwrap(),
                },
            )
            .unwrap();
        let before = (
            run.frame.fuel.snapshot().used,
            run.store.budget().handles(),
            run.store.budget().bytes(),
            run.store.admit_move(array).unwrap(),
        );
        assert_eq!(
            step::step(&mut run.frame, &mut run.store, recipe)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(
            (
                run.frame.fuel.snapshot().used,
                run.store.budget().handles(),
                run.store.budget().bytes(),
                run.store.admit_move(array).unwrap()
            ),
            // Original MakePlace executes and charges once; Borrow validates
            // the retained binding view before its own fuel/publication.
            (before.0 + 1, before.1, before.2, before.3)
        );
        assert_eq!(run.frame.counters.started_by_opcode[30], 1);
        assert_eq!(run.frame.counters.started_by_opcode[31], 0);
        db.finish_run().unwrap();
    }
}

#[test]
fn rustc_whole_array_borrow_without_source_view_uses_closed_descriptor() {
    // A closed runtime descriptor is the reached concrete type of the place;
    // a cleared source view no longer blocks the borrow.
    let (mut db, mut run, _array, outcome) = original_whole_array_borrow_at_commit();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("borrow thunk absent")
    };
    let binding = recipe
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::MakePlace { binding, .. } => Some(*binding),
            _ => None,
        })
        .unwrap();
    run.frame.clear_binding_source_for_test(binding).unwrap();
    step::step(&mut run.frame, &mut run.store, recipe).unwrap();
    db.finish_run().unwrap();
}

#[test]
fn rustc_whole_array_borrow_wrong_destination_source_refuses_before_publication() {
    use crate::traits::term::RegionTerm;
    let (mut db, mut run, array, outcome) = original_whole_array_borrow_at_commit();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("borrow thunk absent")
    };
    let destination = recipe
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::Borrow { destination, .. } => Some(*destination),
            _ => None,
        })
        .unwrap();
    run.frame
        .set_source_view(
            destination,
            Some(TyTerm::Ref(
                RegionTerm::Static,
                false,
                Box::new(TyTerm::Array(
                    Box::new(TyTerm::Primitive(PrimitiveType::U32)),
                    ConstTerm::Value(u64::try_from(facts::array_binding(BOUNDS).len()).unwrap()),
                )),
            )),
        )
        .unwrap();
    let before = (
        run.frame.fuel.snapshot().used,
        run.store.budget().handles(),
        run.store.budget().bytes(),
        run.store.admit_move(array).unwrap(),
    );
    assert_eq!(
        step::step(&mut run.frame, &mut run.store, recipe)
            .unwrap_err()
            .code,
        "LRR-E-OWNERSHIP"
    );
    assert_eq!(
        (
            run.frame.fuel.snapshot().used,
            run.store.budget().handles(),
            run.store.budget().bytes(),
            run.store.admit_move(array).unwrap()
        ),
        (before.0 + 1, before.1, before.2, before.3)
    );
    assert_eq!(run.frame.counters.started_by_opcode[30], 1);
    assert!(run.frame.cell(destination).is_err());
    assert_eq!(run.frame.counters.started_by_opcode[31], 0);
    db.finish_run().unwrap();
}

#[test]
fn rustc_whole_array_reference_deref_checks_written_target_and_each_live_lease() {
    let (mut db, mut run, array, outcome) = original_whole_array_borrow_at_commit();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("borrow thunk absent")
    };
    let value = recipe
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::Borrow { destination, .. } => Some(*destination),
            _ => None,
        })
        .unwrap();
    let borrow_node = &run.frame.body.nodes()[recipe.recipe().key.node as usize];
    let reference_place = borrow_node.place.unwrap();
    assert_eq!(borrow_node.operands.len(), 1);
    let target_node = &run.frame.body.nodes()[borrow_node.operands[0].0 as usize];
    assert_eq!(
        target_node.role,
        crate::syntax::body_index::NodeRole::Expression(
            crate::syntax::body_index::ExpressionKind::Path,
        )
    );
    assert_eq!(target_node.scope, borrow_node.scope);
    // The original &xs recipe forms its own place, leaving the operand PathExpr
    // place unused in the same active scope. No sibling scope is manufactured.
    let destination = target_node.place.unwrap();
    assert_ne!(reference_place, destination);
    assert!(matches!(
        step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
        StepOutcome::Next(_)
    ));
    let reference_cell = run.frame.cell(value).unwrap();
    let original_reference = run.store.admit_move(reference_cell).unwrap();
    let original_array = run.store.admit_move(array).unwrap();
    let original_scopes = run.frame.active_scopes().to_vec();
    run.frame
        .value_place(value, reference_place, &run.store)
        .unwrap();
    run.frame
        .deref_place(reference_place, destination, &run.store)
        .unwrap();
    assert_eq!(run.frame.active_scopes(), original_scopes);
    assert_eq!(
        run.frame.source_view(destination).unwrap(),
        Some(TyTerm::Array(
            Box::new(TyTerm::Primitive(PrimitiveType::I32)),
            ConstTerm::Value(u64::try_from(facts::array_binding(BOUNDS).len()).unwrap())
        ))
    );
    assert_eq!(
        run.store.admit_move(reference_cell).unwrap(),
        original_reference
    );
    assert_eq!(run.store.admit_move(array).unwrap(), original_array);
    let target = run.frame.executed_place_for_test(destination).unwrap();
    for (index, expected) in facts::array_binding(BOUNDS).into_iter().enumerate() {
        let field = run
            .store
            .project_index(
                target,
                ValueRepr::unsigned(TypeId(13), u64::try_from(index).unwrap()).unwrap(),
            )
            .unwrap();
        assert_eq!(
            run.store.read_place(&field).unwrap(),
            ValueRepr::signed(TypeId(6), expected).unwrap()
        );
    }
    let moved = run.store.move_from(array).unwrap();
    let target = run.frame.executed_place_for_test(destination).unwrap();
    let before = (run.store.budget().handles(), run.store.budget().bytes());
    let Err(crate::runtime::place::PlaceFailure::RuntimeFault(error)) =
        run.store.place_type_id(target)
    else {
        panic!("array dereference must revalidate its now stale lease")
    };
    assert_eq!(error.code, "LRR-E-STALE-HANDLE");
    assert_eq!(
        (run.store.budget().handles(), run.store.budget().bytes()),
        before
    );
    while let Some(field) = run.store.aggregate_cleanup_next(moved).unwrap() {
        run.store.discard(field).unwrap();
    }
    run.store.discard(moved).unwrap();
    // Direct frame/lease adaptation uses the actual original &xs owner, not a
    // rewritten Rust PrefixExpr or full upstream fixture execution.
    db.finish_run().unwrap();
}
fn index_at_commit() -> (Database, RunMachine, CellId, crate::db::memo::Outcome) {
    let (mut db, mut run, array) = original_array_prefix();
    // The original `let _ = &xs` and wildcard statement admission remain later
    // work. This harness enters the unchanged index AST, keeping source origins.
    enter_original_expression(&mut run, facts::index_ranges(BOUNDS)[0]);
    for _ in 0..20 {
        let outcome = next(&mut db, &mut run);
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("index thunk absent")
        };
        if recipe
            .recipe()
            .operations
            .iter()
            .any(|op| matches!(op, Op::ProjectIndex { .. }))
        {
            return (db, run, array, outcome);
        }
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
    }
    panic!("original index projection not reached");
}
#[test]
fn rustc_original_array_annotation_constructs_values_and_retains_source_view() {
    let (mut db, mut run, array) = original_array_prefix();
    let values = facts::array_binding(BOUNDS);
    let lease = run
        .store
        .borrow(&Place::cell(array), Access::Shared)
        .unwrap();
    for (index, expected) in values.iter().enumerate() {
        let place = run
            .store
            .project_index(
                &Place::deref(lease),
                ValueRepr::unsigned(TypeId(13), index as u64).unwrap(),
            )
            .unwrap();
        let value = run.store.read_place(&place).unwrap();
        assert_eq!(value.type_id, 6);
        assert_eq!(value.payload as i64, *expected);
    }
    assert_eq!(run.frame.counters.started_by_opcode[27], 1);
    assert_eq!(
        run.frame.counters.started_by_opcode[28],
        values.len() as u64
    );
    assert_eq!(run.frame.counters.started_by_opcode[29], 1);
    let range = facts::index_ranges(BOUNDS)[0];
    enter_original_expression(&mut run, range);
    for _ in 0..10 {
        let outcome = next(&mut db, &mut run);
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("index thunk absent")
        };
        if let Some(destination) = recipe.recipe().operations.iter().find_map(|op| match op {
            Op::MakePlace { destination, .. } => Some(*destination),
            _ => None,
        }) {
            step::step(&mut run.frame, &mut run.store, recipe).unwrap();
            assert_eq!(
                run.frame.source_view(destination).unwrap(),
                Some(TyTerm::Array(
                    Box::new(TyTerm::Primitive(PrimitiveType::I32)),
                    ConstTerm::Value(values.len() as u64)
                ))
            );
            db.finish_run().unwrap();
            return;
        }
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
    }
    panic!("retained array place source absent");
}
#[test]
fn rustc_original_array_index_panics_with_original_pc_and_origin() {
    let (mut db, mut run, _, outcome) = index_at_commit();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("index thunk absent")
    };
    let before = (
        run.frame.fuel.snapshot().used,
        run.store.budget().handles(),
        run.store.budget().bytes(),
    );
    let StepOutcome::GuestPanic(panic) =
        step::step(&mut run.frame, &mut run.store, recipe).unwrap()
    else {
        panic!("original out-of-bounds index must panic")
    };
    assert_eq!(panic.kind, crate::vm::numeric::PanicKind::IndexOutOfBounds);
    assert_eq!(panic.pc, recipe.recipe().key);
    assert_eq!(panic.origin.span, recipe.recipe().origin.span);
    assert_eq!(
        panic.origin.span.relative_start,
        facts::index_ranges(BOUNDS)[0].start
    );
    assert_eq!(
        panic.origin.span.relative_end,
        facts::index_ranges(BOUNDS)[0].end
    );
    assert_eq!(run.frame.fuel.snapshot().used, before.0 + 1);
    assert_eq!(
        (run.store.budget().handles(), run.store.budget().bytes()),
        (before.1, before.2)
    );
    assert_eq!(run.frame.counters.started_by_opcode[36], 1);
    assert_eq!(run.frame.counters.started_by_opcode[33], 0);
    assert_eq!(run.frame.status(), crate::vm::frame::FrameStatus::Panicked);
    db.finish_run().unwrap();
}
#[test]
fn rustc_array_index_wrong_identity_refuses_before_projection_fuel_or_allocation() {
    for value in [
        ValueRepr::signed(TypeId(8), 0).unwrap(),
        ValueRepr::unsigned(TypeId(12), 0).unwrap(),
    ] {
        let (mut db, mut run, _, outcome) = index_at_commit();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("index thunk absent")
        };
        let index = recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| match op {
                Op::ProjectIndex { index, .. } => Some(*index),
                _ => None,
            })
            .unwrap();
        // Supplementary LRR representation injection at the original index PC.
        run.frame
            .inject_test_index_scalar(index, value, &mut run.store)
            .unwrap();
        let before = (
            run.frame.fuel.snapshot().used,
            run.store.budget().handles(),
            run.store.budget().bytes(),
        );
        assert_eq!(
            step::step(&mut run.frame, &mut run.store, recipe)
                .unwrap_err()
                .code,
            "LRR-E-INDEX-TYPE"
        );
        assert_eq!(
            (
                run.frame.fuel.snapshot().used,
                run.store.budget().handles(),
                run.store.budget().bytes()
            ),
            before
        );
        assert_eq!(run.frame.counters.started_by_opcode[36], 0);
        db.finish_run().unwrap();
    }
}
#[test]
fn rustc_array_index_huge_usize_is_bounds_panic_before_ordinal_conversion() {
    let (mut db, mut run, _, outcome) = index_at_commit();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("index thunk absent")
    };
    let index = recipe
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::ProjectIndex { index, .. } => Some(*index),
            _ => None,
        })
        .unwrap();
    run.frame
        .inject_test_index_scalar(
            index,
            ValueRepr::unsigned(TypeId(13), u64::MAX).unwrap(),
            &mut run.store,
        )
        .unwrap();
    assert!(matches!(
        step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
        StepOutcome::GuestPanic(crate::vm::step::GuestPanic {
            kind: crate::vm::numeric::PanicKind::IndexOutOfBounds,
            ..
        })
    ));
    assert_eq!(run.frame.counters.started_by_opcode[36], 1);
    assert_eq!(run.frame.projection_bytes_for_test(), 0);
    db.finish_run().unwrap();
}
#[test]
fn rustc_array_projected_read_freezes_checked_index_and_preserves_element_source() {
    let values = facts::array_binding(BOUNDS);
    for (chosen, expected) in values.iter().enumerate() {
        let (mut db, mut run, _, outcome) = index_at_commit();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("index thunk absent")
        };
        let (index, destination) = recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| match op {
                Op::ProjectIndex {
                    index, destination, ..
                } => Some((*index, *destination)),
                _ => None,
            })
            .unwrap();
        // The original source remains xs[7]; successful-index injections are LRR
        // supplements, not a pass of its compile-fail or runtime-panic oracle.
        run.frame
            .inject_test_index_scalar(
                index,
                ValueRepr::unsigned(TypeId(13), chosen as u64).unwrap(),
                &mut run.store,
            )
            .unwrap();
        let result = recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| match op {
                Op::ReadPlace { destination, .. } => Some(*destination),
                _ => None,
            })
            .unwrap();
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        let value = run
            .store
            .read_scalar(run.frame.cell(result).unwrap())
            .unwrap();
        assert_eq!(value.payload as i64, *expected);
        assert_eq!(
            run.frame.source_view(result).unwrap(),
            Some(TyTerm::Primitive(PrimitiveType::I32))
        );
        assert_eq!(run.frame.source_view(destination).unwrap(), None);
        assert_eq!(run.frame.counters.started_by_opcode[36], 1);
        assert_eq!(run.frame.counters.started_by_opcode[33], 1);
        assert_eq!(run.frame.projection_bytes_for_test(), 0);
        db.finish_run().unwrap();
    }
}
#[test]
fn rustc_array_index_stale_receiver_lease_faults_before_projection_effects() {
    let (mut db, mut run, array, outcome) = index_at_commit();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("index thunk absent")
    };
    let base = recipe
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::ProjectIndex { base, .. } => Some(*base),
            _ => None,
        })
        .unwrap();
    let reference = run
        .store
        .borrow(&Place::cell(array), Access::Shared)
        .unwrap();
    run.frame
        .inject_test_place(base, Place::deref(reference))
        .unwrap();
    let moved = run.store.move_from(array).unwrap();
    let before = (
        run.frame.fuel.snapshot().used,
        run.store.budget().handles(),
        run.store.budget().bytes(),
    );
    assert_eq!(
        step::step(&mut run.frame, &mut run.store, recipe)
            .unwrap_err()
            .code,
        "LRR-E-STALE-HANDLE"
    );
    assert_eq!(
        (
            run.frame.fuel.snapshot().used,
            run.store.budget().handles(),
            run.store.budget().bytes()
        ),
        before
    );
    assert_eq!(run.frame.counters.started_by_opcode[36], 0);
    while let Some(field) = run.store.aggregate_cleanup_next(moved).unwrap() {
        run.store.discard(field).unwrap();
    }
    run.store.discard(moved).unwrap();
    db.finish_run().unwrap();
}
#[test]
fn rustc_tuple_whitespace_and_comment_selectors_execute_checked_projection_chains() {
    let selectors = facts::tuple_selector_ranges(TUPLES);
    assert_eq!(selectors.len(), 3);
    for (range, fields) in selectors {
        let (mut db, mut run) = start_stock(TUPLES);
        let inner = run
            .store
            .intern_type(TypeKey::Tuple(Box::new([TypeId(1)])))
            .unwrap();
        let outer = run
            .store
            .intern_type(TypeKey::Tuple(Box::new([inner])))
            .unwrap();
        let mut bound = false;
        for _ in 0..100 {
            let outcome = db
                .fetch(&SyntaxProvider, QueryKey::Thunk(run.frame.pc().clone()))
                .into_ready()
                .unwrap()
                .unwrap();
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("tuple thunk absent")
            };
            if let Some(guard) = recipe.recipe().operations.iter().find_map(|op| match op {
                Op::ConstructStart { guard } => Some(*guard),
                _ => None,
            }) {
                let node = run
                    .frame
                    .body
                    .nodes()
                    .iter()
                    .find(|node| node.constructor_guard == Some(guard))
                    .unwrap();
                // Existing source-bound structural-constructor adaptation. The
                // inferred tuple result context is still pending M017 integration.
                if run.frame.hint(node.value.unwrap()).unwrap().is_none() {
                    run.frame
                        .set_hint(node.value.unwrap(), Some(outer))
                        .unwrap();
                }
                run.prepare_constructor(&mut db, &[])
                    .into_ready()
                    .unwrap()
                    .unwrap();
            }
            let binds = recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::BindLocal { .. }));
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if binds {
                bound = true;
                break;
            }
        }
        assert!(bound);
        let node = enter_original_expression(&mut run, range);
        let mut selected = false;
        for _ in 0..30 {
            let outcome = next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("tuple selector thunk absent")
            };
            // `let () = place` tests the projected place in the shared
            // pattern transaction; the selector never performs an owning read.
            let opens = recipe.recipe().operations.iter().any(|op| {
                matches!(
                    op,
                    Op::Pattern {
                        operation: crate::ir::op::PatternOperation::Open { .. }
                    }
                )
            });
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if opens {
                assert!(run.frame.cell(node.value.unwrap()).is_err());
                selected = true;
                break;
            }
        }
        assert!(selected);
        assert_eq!(
            run.frame.counters.started_by_opcode[35],
            fields.len() as u64
        );
        assert_eq!(run.frame.counters.started_by_opcode[33], 0);
        assert_eq!(run.frame.counters.started_by_opcode[8], 0);
        assert_eq!(run.frame.projection_bytes_for_test(), 0);
        db.finish_run().unwrap();
    }
}
