//! Original temporary-expression entry; whole match/Drop execution stays pending.
use super::*;
use crate::{
    runtime::cell::CellState,
    syntax::place_fixture_facts,
    traits::term::{ParamKey, TyTerm},
};
const STOCK: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/match-naked-record-expr.rs"
);
const PARENS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-partial-reinit-4.rs"
);

fn at_value_place() -> (
    Database,
    RunMachine,
    crate::db::memo::Outcome,
    SlotId,
    SlotId,
) {
    at_value_place_with_fuel(None)
}
fn at_value_place_with_fuel(
    fuel: Option<u64>,
) -> (
    Database,
    RunMachine,
    crate::db::memo::Outcome,
    SlotId,
    SlotId,
) {
    let (mut db, mut run) = super::borrows::tests::start_stock(STOCK);
    if let Some(fuel) = fuel {
        run.frame.set_fuel_limit(fuel).unwrap();
    }
    let selected: Vec<_> = run
        .frame
        .body
        .nodes()
        .iter()
        .filter(|node| {
            matches!(
                node.role,
                crate::syntax::body_index::NodeRole::Expression(_)
            )
        })
        .filter(|node| {
            run.sources[0]
                .named_field_member(&run.frame.body, node.id)
                .is_ok()
        })
        .map(|node| (node.id, node.scope))
        .collect();
    let [(selected, scope)] = selected.as_slice() else {
        panic!("original temporary field boundary ambiguous")
    };
    // Disclosed harness entry into the original match arm's FieldExpr. Enter
    // the actual lexical ancestry, never rewrite or bypass its child expression.
    let mut ancestry = Vec::new();
    let mut current = Some(*scope);
    while let Some(scope) = current {
        ancestry.push(scope);
        current = run.frame.body.scopes()[scope.0 as usize].parent;
    }
    for scope in ancestry.into_iter().rev() {
        run.frame.enter(scope).unwrap();
    }
    run.frame.pc = run.frame.body.pc(*selected, Phase::Entry).unwrap();
    for _ in 0..80 {
        let outcome = super::borrows::tests::next(&mut db, &mut run);
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("temporary thunk absent")
        };
        let slots = recipe.recipe().operations.iter().find_map(|op| match op {
            Op::ValuePlace {
                source,
                destination,
            } => Some((*source, *destination)),
            _ => None,
        });
        if let Some((source, destination)) = slots {
            return (db, run, outcome, source, destination);
        }
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
    }
    panic!("original record receiver did not reach ValuePlace");
}
fn snapshot(run: &RunMachine) -> (u64, u32, usize, u64) {
    (
        run.frame.fuel_snapshot().used,
        run.store.budget().handles(),
        run.store.budget().bytes(),
        run.frame.counters.operations_started,
    )
}
#[test]
fn rustc_original_temporary_record_field_keeps_the_constructed_owner_and_evaluates_once() {
    let (mut db, mut run, outcome, source, place) = at_value_place();
    let cell = run.frame.cell(source).unwrap();
    let before = run.store.state(cell).unwrap();
    assert!(matches!(before,CellState::Live(value) if value.kind == 8));
    let source_view = run.frame.source_view(source).unwrap();
    assert!(matches!(&source_view,Some(TyTerm::Nominal(_,args)) if args.is_empty()));
    let epoch = run.store.location_epoch(cell).unwrap();
    let budget = (run.store.budget().handles(), run.store.budget().bytes());
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("temporary formation absent")
    };
    assert!(matches!(
        step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
        StepOutcome::Next(_)
    ));
    assert_eq!(run.store.state(cell).unwrap(), before);
    assert_eq!(run.store.location_epoch(cell).unwrap(), epoch);
    assert_eq!(
        (run.store.budget().handles(), run.store.budget().bytes()),
        budget
    );
    assert_eq!(run.frame.source_view(place).unwrap(), source_view);
    let outcome = super::borrows::tests::next(&mut db, &mut run);
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("temporary projection absent")
    };
    assert_eq!(recipe.recipe().key.phase, Phase::Commit.code().unwrap());
    let (base, destination) = recipe
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::ProjectField { base, destination } => Some((*base, *destination)),
            _ => None,
        })
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
    let before_binding = snapshot(&run);
    run.prepare_field(&mut db, base, destination)
        .into_ready()
        .unwrap()
        .unwrap();
    assert_eq!(snapshot(&run), before_binding);
    assert!(matches!(
        step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
        StepOutcome::Next(_)
    ));
    let (member, expected) = place_fixture_facts::temporary_record_field(STOCK);
    assert_eq!(
        member,
        run.sources[0]
            .named_field_member(&run.frame.body, NodeId(recipe.recipe().key.node))
            .unwrap()
    );
    let actual = run
        .store
        .preview_scalar(run.frame.cell(result).unwrap(), None)
        .unwrap();
    assert_eq!(actual.payload as i64, expected);
    assert_eq!(run.store.state(cell).unwrap(), before);
    assert_eq!(run.store.location_epoch(cell).unwrap(), epoch);
    for tag in [0, 27, 28, 29, 33, 37, 38] {
        assert_eq!(run.frame.counters.started_by_opcode[tag], 1);
    }
    assert_eq!(run.frame.counters.started_by_opcode[8], 0);
    // The original match selector, final binding and aggregate Drop are not
    // executed by this focused original-expression adaptation.
    db.finish_run().unwrap();
}
#[test]
fn rustc_temporary_record_place_fuel_refusal_keeps_its_original_owner() {
    // Derive the exact original prefix cost on an independent run, then
    // replay from an unstarted frame with that quota. Never reset consumed fuel.
    let limit = {
        let (mut db, run, ..) = at_value_place();
        let used = run.frame.fuel_snapshot().used;
        assert!(used > 0);
        db.finish_run().unwrap();
        used
    };
    let (mut db, mut run, outcome, source, place) = at_value_place_with_fuel(Some(limit));
    assert_eq!(run.frame.fuel_snapshot().used, limit);
    assert_eq!(run.frame.fuel_snapshot().remaining, 0);
    let cell = run.frame.cell(source).unwrap();
    let owner = run.store.state(cell).unwrap();
    let before = snapshot(&run);
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("temporary formation absent")
    };
    let error = step::step(&mut run.frame, &mut run.store, recipe).unwrap_err();
    assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(snapshot(&run), before);
    assert_eq!(
        run.frame.fuel_snapshot().stop,
        Some(crate::vm::fuel::FuelStop::Exhausted)
    );
    assert_eq!(run.store.state(cell).unwrap(), owner);
    assert!(run.frame.nominal_place_source(place).is_err());
    db.finish_run().unwrap();
}
#[test]
fn rustc_temporary_record_rigid_source_refuses_runtime_shape_erasure_before_fuel() {
    let (mut db, mut run, outcome, source, place) = at_value_place();
    let cell = run.frame.cell(source).unwrap();
    let owner = run.store.state(cell).unwrap();
    let rigid = TyTerm::Param(ParamKey {
        definition: run.frame.body.body().definition,
        ordinal: 0,
    });
    run.frame
        .set_source_view(source, Some(rigid.clone()))
        .unwrap();
    let before = snapshot(&run);
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("temporary formation absent")
    };
    let error = step::step(&mut run.frame, &mut run.store, recipe).unwrap_err();
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert_eq!(snapshot(&run), before);
    assert_eq!(run.store.state(cell).unwrap(), owner);
    assert_eq!(run.frame.source_view(source).unwrap(), Some(rigid));
    assert!(run.frame.nominal_place_source(place).is_err());
    db.finish_run().unwrap();
}
#[test]
fn rustc_temporary_record_moved_receiver_refuses_before_place_publication() {
    let (mut db, mut run, outcome, source, place) = at_value_place();
    let cell = run.frame.cell(source).unwrap();
    let held = run.store.move_from(cell).unwrap();
    let before = snapshot(&run);
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("temporary formation absent")
    };
    let error = step::step(&mut run.frame, &mut run.store, recipe).unwrap_err();
    assert_eq!(error.code, "LRR-E-STALE-HANDLE");
    assert_eq!(snapshot(&run), before);
    assert_eq!(run.store.state(cell).unwrap(), CellState::Moved);
    assert_eq!(run.store.peek_ticket(held).unwrap().kind, 8);
    assert!(run.frame.nominal_place_source(place).is_err());
    db.finish_run().unwrap();
}
#[test]
fn rustc_parenthesized_field_receiver_lowers_address_transfer_without_reading_value() {
    let (mut db, run) = super::borrows::tests::start_stock_body(PARENS, "stuff");
    let ranges = place_fixture_facts::parenthesized_field_ranges(PARENS, "stuff");
    let [range] = ranges.as_slice() else {
        panic!("original parenthesized field range ambiguous")
    };
    let node = run
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| {
            node.range == *range
                && matches!(
                    node.role,
                    crate::syntax::body_index::NodeRole::Expression(_)
                )
        })
        .unwrap();
    let key = run.frame.body.pc(node.id, Phase::Operand(0)).unwrap();
    let outcome = db
        .fetch(&SyntaxProvider, QueryKey::Thunk(key.clone()))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("original parentheses thunk absent")
    };
    assert!(
        matches!(recipe.recipe().operations.as_ref(),[Op::PlaceTransfer {source,destination}]
        if node.place==Some(*destination)
            && run.frame.body.nodes()[node.operands[0].0 as usize].place==Some(*source))
    );
    assert_eq!(recipe.recipe().origin.span.relative_start, range.start);
    let mut forged = crate::ir::recipe::RecipeDraft {
        key,
        origin: recipe.recipe().origin.clone(),
        operations: recipe.recipe().operations.to_vec(),
        exits: vec![recipe.recipe().exit.clone()],
        dependencies: recipe.recipe().dependencies.to_vec(),
    };
    let Op::PlaceTransfer {
        source,
        destination,
    } = &mut forged.operations[0]
    else {
        unreachable!()
    };
    *source = *destination;
    assert_eq!(
        crate::ir::validate::verify(forged, &run.frame.body)
            .unwrap_err()
            .code,
        "LRR-E-INTERNAL-IR"
    );
    assert_eq!(run.frame.counters.started_by_opcode[39], 0);
    // Pure original-source lowering only: the uninitialized Test2/Option owner
    // and rustc's E0381 diagnostic require later initialization/Drop integration.
    db.finish_run().unwrap();
}

const COMPUTED: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/qualif-indirect-mutation-pass.rs"
);

fn computed_reference_at_formation() -> (
    Database,
    RunMachine,
    crate::db::memo::Outcome,
    SlotId,
    SlotId,
    NodeId,
) {
    computed_reference_at_formation_with_fuel(None)
}
fn computed_reference_at_formation_with_fuel(
    fuel: Option<u64>,
) -> (
    Database,
    RunMachine,
    crate::db::memo::Outcome,
    SlotId,
    SlotId,
    NodeId,
) {
    let facts = place_fixture_facts::computed_deref_facts(COMPUTED);
    let (mut db, mut run) = super::borrows::tests::start_stock_body(COMPUTED, "f");
    if let Some(fuel) = fuel {
        run.frame.set_fuel_limit(fuel).unwrap();
    }
    let (statement, scope) = {
        let node = run
            .frame
            .body
            .nodes()
            .iter()
            .find(|node| {
                node.range == facts.binding_statement
                    && node.role == crate::syntax::body_index::NodeRole::Statement
            })
            .unwrap();
        (node.id, node.scope)
    };
    let binding = run
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| {
            node.range == facts.binding_pattern
                && node.role == crate::syntax::body_index::NodeRole::Pattern
        })
        .unwrap()
        .id;
    // Disclosed expression-prefix adaptation: enter the original `let mut a`,
    // leaving x's Option<String>/tuple initializer and const qualification cold.
    let mut ancestry = Vec::new();
    let mut current = Some(scope);
    while let Some(scope) = current {
        ancestry.push(scope);
        current = run.frame.body.scopes()[scope.0 as usize].parent;
    }
    for scope in ancestry.into_iter().rev() {
        run.frame.enter(scope).unwrap();
    }
    run.frame.pc = run.frame.body.pc(statement, Phase::Entry).unwrap();
    let target = crate::runtime::shape::PrimitiveIdentity::from_recipe(facts.target)
        .unwrap()
        .id();
    let mut binding_completed = false;
    for _ in 0..80 {
        let outcome = super::borrows::tests::next(&mut db, &mut run);
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("original computed-reference phase absent")
        };
        if !binding_completed {
            for op in &recipe.recipe().operations {
                if let Op::Literal { destination, .. } = op {
                    // Explicit worker context derived from the original later
                    // x.1=a and x's written u32 member; no general inference claim.
                    run.frame.set_hint(*destination, Some(target)).unwrap();
                }
            }
        }
        if let Some((source, destination)) = recipe.recipe().operations.iter().find_map(|op| {
            if let Op::ValuePlace {
                source,
                destination,
            } = op
            {
                Some((*source, *destination))
            } else {
                None
            }
        }) {
            assert!(binding_completed);
            return (db, run, outcome, source, destination, binding);
        }
        let binds = recipe
            .recipe()
            .operations
            .iter()
            .any(|op| matches!(op, Op::BindLocal { binding: found, .. } if *found == binding));
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        if !binding_completed {
            for op in &recipe.recipe().operations {
                if let Op::Literal { destination, .. } = op {
                    // Literal publication clears pre-execution source views.
                    // Attach the disclosed source-derived context to its actual
                    // value before the subsequent BindLocal transfers it.
                    run.frame
                        .set_source_view(*destination, Some(TyTerm::Primitive(facts.target)))
                        .unwrap();
                }
            }
        }
        binding_completed |= binds;
    }
    panic!("original *(&mut a) did not reach reference place formation")
}
fn computed_reference_at_deref() -> (
    Database,
    RunMachine,
    crate::db::memo::Outcome,
    SlotId,
    SlotId,
    NodeId,
) {
    let (mut db, mut run, outcome, source, place, binding) = computed_reference_at_formation();
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("computed reference formation absent")
    };
    assert!(matches!(
        step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
        StepOutcome::Next(_)
    ));
    let outcome = super::borrows::tests::next(&mut db, &mut run);
    (db, run, outcome, source, place, binding)
}
#[test]
fn rustc_original_computed_mutable_reference_assigns_without_copying_its_owner() {
    let (mut db, mut run, outcome, source, reference_place, binding) =
        computed_reference_at_formation();
    let reference_cell = run.frame.cell(source).unwrap();
    let owner = run.store.state(reference_cell).unwrap();
    let reference = run.store.reference_from_cell(reference_cell).unwrap();
    let target = run.frame.binding_cell_for_test(binding).unwrap();
    let epoch = run.store.location_epoch(target).unwrap();
    let before = (run.store.budget().handles(), run.store.budget().bytes());
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("formation absent")
    };
    assert!(matches!(
        step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
        StepOutcome::Next(_)
    ));
    assert_eq!(
        (run.store.budget().handles(), run.store.budget().bytes()),
        before
    );
    assert_eq!(run.store.state(reference_cell).unwrap(), owner);
    assert_eq!(
        run.frame.source_view(reference_place).unwrap(),
        run.frame.source_view(source).unwrap()
    );
    let deref = super::borrows::tests::next(&mut db, &mut run);
    let QueryValue::Thunk(recipe) = deref.value.as_ref() else {
        panic!("deref absent")
    };
    assert_eq!(recipe.recipe().key.phase, Phase::Commit.code().unwrap());
    assert!(
        matches!(recipe.recipe().operations.as_ref(),[Op::DerefPlace {reference,..}] if *reference==reference_place)
    );
    assert!(matches!(
        step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
        StepOutcome::Next(_)
    ));
    let assignment = super::borrows::tests::next(&mut db, &mut run);
    let QueryValue::Thunk(recipe) = assignment.value.as_ref() else {
        panic!("assignment absent")
    };
    assert!(
        recipe
            .recipe()
            .operations
            .iter()
            .any(|op| matches!(op, Op::AssignPlace { .. }))
    );
    assert!(matches!(
        step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
        StepOutcome::Next(_)
    ));
    let facts = place_fixture_facts::computed_deref_facts(COMPUTED);
    let actual = run.store.read_reference(reference).unwrap();
    assert_eq!(
        actual.type_id,
        crate::runtime::shape::PrimitiveIdentity::from_recipe(facts.target)
            .unwrap()
            .id()
            .0
    );
    assert_eq!(actual.payload, facts.assigned);
    assert_eq!(run.store.location_epoch(target).unwrap(), epoch);
    assert_eq!(run.store.state(reference_cell).unwrap(), owner);
    for tag in [30, 31, 32, 34, 38] {
        assert_eq!(run.frame.counters.started_by_opcode[tag], 1);
    }
    assert_eq!(run.frame.counters.started_by_opcode[0], 2);
    assert_eq!(run.frame.counters.started_by_opcode[8], 0);
    // Original x.1=a, Option<String> construction, function cleanup and native
    // const-qualification oracle are deliberately outside this prefix control.
    db.finish_run().unwrap();
}
#[test]
fn rustc_computed_reference_place_fuel_refusal_retains_the_original_reference_cell() {
    // Derive the exact original prefix cost on an independent run, then
    // replay from an unstarted frame with that quota. Never reset consumed fuel.
    let limit = {
        let (mut db, run, ..) = computed_reference_at_formation();
        let used = run.frame.fuel_snapshot().used;
        assert!(used > 0);
        db.finish_run().unwrap();
        used
    };
    let (mut db, mut run, outcome, source, place, binding) =
        computed_reference_at_formation_with_fuel(Some(limit));
    assert_eq!(run.frame.fuel_snapshot().used, limit);
    assert_eq!(run.frame.fuel_snapshot().remaining, 0);
    let cell = run.frame.cell(source).unwrap();
    let owner = run.store.state(cell).unwrap();
    let reference = run.store.reference_from_cell(cell).unwrap();
    let target = run.frame.binding_cell_for_test(binding).unwrap();
    let original = run.store.state(target).unwrap();
    let before = snapshot(&run);
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("formation absent")
    };
    assert_eq!(
        step::step(&mut run.frame, &mut run.store, recipe)
            .unwrap_err()
            .code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert_eq!(snapshot(&run), before);
    assert_eq!(
        run.frame.fuel_snapshot().stop,
        Some(crate::vm::fuel::FuelStop::Exhausted)
    );
    assert_eq!(run.store.state(cell).unwrap(), owner);
    assert_eq!(run.store.state(target).unwrap(), original);
    assert_eq!(run.store.reference_from_cell(cell).unwrap(), reference);
    assert!(run.frame.source_view(place).unwrap().is_none());
    assert_eq!(run.frame.counters.started_by_opcode[38], 0);
    db.finish_run().unwrap();
}
#[test]
fn rustc_computed_reference_source_access_mismatch_refuses_before_deref_fuel() {
    let (mut db, mut run, outcome, source, place, binding) = computed_reference_at_deref();
    let cell = run.frame.cell(source).unwrap();
    let owner = run.store.state(cell).unwrap();
    let target = run.frame.binding_cell_for_test(binding).unwrap();
    let original = run.store.state(target).unwrap();
    let Some(TyTerm::Ref(region, true, pointee)) = run.frame.source_view(place).unwrap() else {
        panic!("original mutable source absent")
    };
    run.frame
        .set_source_view(place, Some(TyTerm::Ref(region, false, pointee)))
        .unwrap();
    let before = snapshot(&run);
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("deref absent")
    };
    assert_eq!(
        step::step(&mut run.frame, &mut run.store, recipe)
            .unwrap_err()
            .code,
        "LRR-E-OWNERSHIP"
    );
    assert_eq!(snapshot(&run), before);
    assert_eq!(run.store.state(cell).unwrap(), owner);
    assert_eq!(run.store.state(target).unwrap(), original);
    assert_eq!(run.frame.counters.started_by_opcode[32], 0);
    db.finish_run().unwrap();
}
#[test]
fn rustc_computed_reference_moved_referent_refuses_before_deref_publication() {
    let (mut db, mut run, outcome, source, _, binding) = computed_reference_at_deref();
    let cell = run.frame.cell(source).unwrap();
    let owner = run.store.state(cell).unwrap();
    let target = run.frame.binding_cell_for_test(binding).unwrap();
    let held = run.store.move_from(target).unwrap();
    let before = snapshot(&run);
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("deref absent")
    };
    assert_eq!(
        step::step(&mut run.frame, &mut run.store, recipe)
            .unwrap_err()
            .code,
        "LRR-E-STALE-HANDLE"
    );
    assert_eq!(snapshot(&run), before);
    assert_eq!(run.store.state(cell).unwrap(), owner);
    assert_eq!(run.store.state(target).unwrap(), CellState::Moved);
    assert_eq!(
        run.store.peek_ticket(held).unwrap().type_id,
        crate::runtime::shape::PrimitiveIdentity::from_recipe(
            place_fixture_facts::computed_deref_facts(COMPUTED).target
        )
        .unwrap()
        .id()
        .0
    );
    assert_eq!(run.frame.counters.started_by_opcode[32], 0);
    db.finish_run().unwrap();
}
