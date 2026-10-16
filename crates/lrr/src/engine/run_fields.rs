//! Reached field selection from retained source identity, before guest mutation.
use super::*;
use crate::{
    frontend::types::{NominalFieldBinding, SchemaNames},
    source::manifest::ItemKind,
};

#[path = "run_field_source.rs"]
mod source;

fn field_fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-SCHEMA", message.into())
}

impl RunMachine {
    #[cfg(test)]
    pub(crate) fn current_field_source_for_test(
        db: &mut Database,
        sources: &[PreparedSourceView],
        index: &crate::syntax::body_index::BodyIndex,
        pc: &PcKey,
    ) -> QueryPoll<(String, crate::syntax::outline::ScopeKey)> {
        match source::capture(db, sources, index, pc) {
            QueryPoll::Ready(Ok(proof)) => QueryPoll::Ready(
                proof
                    .scope()
                    .map(|scope| (proof.member().to_owned(), scope.clone())),
            ),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }

    pub(super) fn prepare_field(
        &mut self,
        db: &mut Database,
        base: SlotId,
        destination: SlotId,
    ) -> QueryPoll<()> {
        match self.prepare_field_inner(db, base, destination) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }

    fn prepare_field_inner(
        &mut self,
        db: &mut Database,
        base: SlotId,
        destination: SlotId,
    ) -> Result<QueryPoll<()>, Diagnostic> {
        // The definition comes from the executed expression's retained source
        // view. A runtime descriptor or a same-spelled type cannot select it.
        let definition = self.frame.nominal_place_source(base)?;
        let graph = self
            .graph
            .as_ref()
            .ok_or_else(|| field_fault("field requires a checked source catalogue"))?;
        for view in &self.sources {
            graph.require_source(view.selection(), view.outline())?;
        }
        let caller = match source::capture(db, &self.sources, &self.frame.body, self.frame.pc()) {
            QueryPoll::Ready(Ok(caller)) => caller,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        };
        let captured = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(definition)) {
            QueryPoll::Ready(Ok(captured)) => captured,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        };
        let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
            return Err(field_fault("selected field schema query outcome differs"));
        };
        self.sources
            .len()
            .checked_mul(std::mem::size_of::<&PreparedSourceView>())
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(self.sources.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        sources.extend(self.sources.iter());
        let names = match SchemaNames::from_current_database(db, graph, &sources, schema) {
            QueryPoll::Ready(Ok(names)) => names,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        };
        if names.definition().item_kind != ItemKind::Struct {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "field expression requires an entered enum variant or other source member view"
                    .into(),
            ));
        }
        match names.bind_named_field_current(db, &sources, None, caller.member(), caller.scope()?) {
            QueryPoll::Ready(Ok(NominalFieldBinding::Ready(binding))) => Ok(QueryPoll::Ready(
                self.frame.prepare_source_field(base, destination, binding),
            )),
            QueryPoll::Ready(Ok(NominalFieldBinding::Deferred { .. })) => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "field expression requires further selected source type or generic binding".into(),
            )),
            QueryPoll::Ready(Err(error)) => Err(error),
            QueryPoll::NeedInput(request) => Ok(QueryPoll::NeedInput(request)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::digest::Digest,
        db::dependency::{InputKey, InputValue},
        ir::op::PrimitiveType,
        syntax::place_fixture_facts,
        traits::term::TyTerm,
    };
    const STOCK: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-comp.rs"
    );

    fn first_field() -> (
        Database,
        RunMachine,
        crate::db::memo::Outcome,
        SlotId,
        SlotId,
    ) {
        // Enter the original d body; do not rewrite the licensed compile-fail
        // source or claim that its rustc borrow-checker oracle passes under LRR.
        let (mut db, mut run) = super::super::borrows::tests::start_stock_body(STOCK, "d");
        for _ in 0..100 {
            let outcome = super::super::borrows::tests::next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock named field thunk absent")
            };
            if let Some((base, destination)) = recipe.recipe().operations.iter().find_map(|op| {
                if let Op::ProjectField { base, destination } = op {
                    Some((*base, *destination))
                } else {
                    None
                }
            }) {
                return (db, run, outcome, base, destination);
            }
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
        }
        panic!("original &p.y did not reach named projection");
    }

    fn guest_snapshot(run: &RunMachine) -> (u64, u32, usize, u64) {
        (
            run.frame.fuel_snapshot().used,
            run.store.budget().handles(),
            run.store.budget().bytes(),
            run.frame.counters.operations_started,
        )
    }

    fn borrowed_member_at_commit() -> (
        Database,
        RunMachine,
        crate::db::memo::Outcome,
        SlotId,
        SlotId,
    ) {
        let (mut db, mut run) = super::super::borrows::tests::start_stock_body(STOCK, "a");
        for _ in 0..120 {
            let outcome = super::super::borrows::tests::next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("original borrowed-member continuation absent")
            };
            for op in &recipe.recipe().operations {
                if let Op::ProjectField { base, destination } = op {
                    if matches!(
                        run.frame.source_view(*base).unwrap(),
                        Some(TyTerm::Ref(_, _, _))
                    ) {
                        let slots = (*base, *destination);
                        return (db, run, outcome, slots.0, slots.1);
                    }
                    run.prepare_field(&mut db, *base, *destination)
                        .into_ready()
                        .unwrap()
                        .unwrap();
                }
            }
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
        }
        panic!("original q.x through &Point did not reach named projection");
    }

    #[test]
    fn rustc_original_borrowed_nominal_field_reads_owner_assignment_without_copying_reference() {
        let (mut db, mut run, outcome, base, _) = borrowed_member_at_commit();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("borrowed named field recipe absent")
        };
        let Some(TyTerm::Ref(_, false, target)) = run.frame.source_view(base).unwrap() else {
            panic!("original shared &Point source view was lost")
        };
        assert!(matches!(target.as_ref(), TyTerm::Nominal(_, args) if args.is_empty()));
        let (base, destination) = recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| {
                if let Op::ProjectField { base, destination } = op {
                    Some((*base, *destination))
                } else {
                    None
                }
            })
            .unwrap();
        let read = recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| {
                if let Op::ReadPlace { destination, .. } = op {
                    Some(*destination)
                } else {
                    None
                }
            })
            .unwrap();
        let reference_cell = run
            .frame
            .body
            .nodes()
            .iter()
            .filter(|node| node.role == crate::syntax::body_index::NodeRole::Pattern)
            .filter_map(|node| run.frame.binding_cell_for_test(node.id).ok())
            .find(|cell| run.store.reference_from_cell(*cell).is_ok())
            .expect("original q reference binding absent");
        let reference = run.store.reference_from_cell(reference_cell).unwrap();
        let before = guest_snapshot(&run);
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        let value = run
            .store
            .preview_scalar(run.frame.cell(read).unwrap(), None)
            .unwrap();
        let (member, expected) = place_fixture_facts::assignment(STOCK, "a");
        assert_eq!(member, "p.x");
        assert_eq!(
            value.type_id,
            crate::frontend::call_contracts::primitive_runtime_id(PrimitiveType::Isize)
                .unwrap()
                .0
        );
        assert_eq!(value.payload as i64, expected);
        assert_eq!(
            run.store.reference_from_cell(reference_cell).unwrap(),
            reference
        );
        assert_eq!(run.frame.counters.started_by_opcode[31], 1);
        assert_eq!(run.frame.counters.started_by_opcode[37], 2);
        assert_eq!(run.frame.counters.started_by_opcode[34], 1);
        assert_eq!(run.frame.counters.started_by_opcode[33], 1);
        assert_eq!(run.frame.counters.started_by_opcode[8], 0);
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_borrowed_nominal_field_shared_lease_refuses_writes_before_effects() {
        let (mut db, mut run, outcome, base, destination) = borrowed_member_at_commit();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("borrowed field recipe absent")
        };
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        let projection = recipe
            .recipe()
            .operations
            .iter()
            .find(|op| matches!(op, Op::ProjectField { .. }))
            .unwrap();
        let before = guest_snapshot(&run);
        let prepared = run
            .frame
            .prepare_projection(projection, &run.store)
            .unwrap()
            .unwrap();
        let source = run
            .frame
            .body
            .nodes()
            .iter()
            .filter(|node| node.role == crate::syntax::body_index::NodeRole::Pattern)
            .find_map(|node| run.frame.binding_cell_for_test(node.id).ok())
            .unwrap();
        let error = run
            .store
            .preview_intrinsic_place_assignment(prepared.place_for_test(), source)
            .unwrap_err();
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-REFERENCE-ACCESS")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(
            run.store
                .read_place(prepared.place_for_test())
                .unwrap()
                .payload as i64,
            place_fixture_facts::assignment(STOCK, "a").1
        );
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_borrowed_field_reference_source_access_mismatch_refuses_before_fuel() {
        let (mut db, mut run, outcome, base, destination) = borrowed_member_at_commit();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("borrowed field recipe absent")
        };
        let Some(TyTerm::Ref(region, false, target)) = run.frame.source_view(base).unwrap() else {
            panic!("original shared source absent")
        };
        run.frame
            .set_source_view(base, Some(TyTerm::Ref(region, true, target)))
            .unwrap();
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        let before = guest_snapshot(&run);
        let projection = recipe
            .recipe()
            .operations
            .iter()
            .find(|op| matches!(op, Op::ProjectField { .. }))
            .unwrap();
        let error = run
            .frame
            .prepare_projection(projection, &run.store)
            .err()
            .expect("source/access mismatch admitted");
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-OWNERSHIP")
        );
        assert_eq!(guest_snapshot(&run), before);
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_borrowed_nominal_field_replaced_owner_invalidates_before_projection() {
        let (mut db, mut run, outcome, base, destination) = borrowed_member_at_commit();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("borrowed field recipe absent")
        };
        let owner = run.frame.body.nodes().iter().filter(|node| node.role == crate::syntax::body_index::NodeRole::Pattern)
            .find_map(|node| run.frame.binding_cell_for_test(node.id).ok().filter(|cell|
                matches!(run.store.state(*cell), Ok(crate::runtime::cell::CellState::Live(value)) if value.kind == 8)))
            .unwrap();
        let held = run.store.move_from(owner).unwrap();
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        let before = guest_snapshot(&run);
        let projection = recipe
            .recipe()
            .operations
            .iter()
            .find(|op| matches!(op, Op::ProjectField { .. }))
            .unwrap();
        let error = run
            .frame
            .prepare_projection(projection, &run.store)
            .err()
            .expect("stale owner lease admitted");
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-STALE-HANDLE")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.store.peek_ticket(held).unwrap().kind, 8);
        db.finish_run().unwrap();
    }

    fn original_nominal_owners(
        run: &RunMachine,
    ) -> (crate::runtime::cell::CellId, crate::runtime::cell::CellId) {
        let cells: Vec<_> = run
            .frame
            .body
            .nodes()
            .iter()
            .filter(|node| node.role == crate::syntax::body_index::NodeRole::Pattern)
            .filter_map(|node| run.frame.binding_cell_for_test(node.id).ok())
            .collect();
        let reference = *cells
            .iter()
            .find(|cell| run.store.reference_from_cell(**cell).is_ok())
            .unwrap();
        let target = *cells
            .iter()
            .find(|cell| {
                matches!(run.store.state(**cell),
            Ok(crate::runtime::cell::CellState::Live(value)) if value.kind == 8)
            })
            .unwrap();
        (reference, target)
    }

    #[test]
    fn rustc_nominal_reference_deref_retains_source_owner_and_shared_access() {
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        let Some(TyTerm::Ref(_, false, pointee)) = run.frame.source_view(base).unwrap() else {
            panic!("original shared nominal source absent")
        };
        assert!(matches!(pointee.as_ref(), TyTerm::Nominal(_, args) if args.is_empty()));
        let (reference_cell, target_cell) = original_nominal_owners(&run);
        let reference = run.store.reference_from_cell(reference_cell).unwrap();
        let reference_owner = run.store.state(reference_cell).unwrap();
        let target_owner = run.store.state(target_cell).unwrap();
        let epoch = run.store.location_epoch(target_cell).unwrap();
        let reference_type = run
            .store
            .place_type_id(run.frame.executed_place_for_test(base).unwrap())
            .unwrap();
        let crate::runtime::shape::TypeKey::Ref { target, .. } = run
            .store
            .type_descriptor(reference_type)
            .unwrap()
            .key
            .as_ref()
        else {
            panic!("original reference descriptor absent")
        };
        let target = *target;
        let before = guest_snapshot(&run);
        run.frame
            .preflight_deref_place(base, destination, &run.store)
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        // LRR frame adaptation of the original q/p lease, not execution of a
        // rewritten source PrefixExpr or completion of the borrowck oracle.
        run.frame
            .deref_place(base, destination, &run.store)
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.frame.source_view(destination).unwrap(), Some(*pointee));
        let place = run.frame.executed_place_for_test(destination).unwrap();
        assert_eq!(run.store.place_type_id(place).unwrap(), target);
        let error = run
            .store
            .preview_intrinsic_place_assignment(place, reference_cell)
            .unwrap_err();
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-REFERENCE-ACCESS")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.store.state(reference_cell).unwrap(), reference_owner);
        assert_eq!(run.store.state(target_cell).unwrap(), target_owner);
        assert_eq!(run.store.location_epoch(target_cell).unwrap(), epoch);
        assert_eq!(
            run.store.reference_from_cell(reference_cell).unwrap(),
            reference
        );
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_nominal_deref_destination_source_mismatch_preserves_constraint_and_owner() {
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        let Some(TyTerm::Ref(_, _, pointee)) = run.frame.source_view(base).unwrap() else {
            panic!("original nominal source absent")
        };
        let TyTerm::Nominal(actual, args) = *pointee else {
            panic!("original nominal definition absent")
        };
        let foreign = Digest::of_bytes(b"foreign-nominal-deref-destination");
        assert_ne!(foreign, actual);
        let expected = TyTerm::Nominal(foreign, args);
        run.frame
            .set_source_view(destination, Some(expected.clone()))
            .unwrap();
        let (reference, target) = original_nominal_owners(&run);
        let owners = (
            run.store.state(reference).unwrap(),
            run.store.state(target).unwrap(),
        );
        let epoch = run.store.location_epoch(target).unwrap();
        let before = guest_snapshot(&run);
        let error = run
            .frame
            .deref_place(base, destination, &run.store)
            .unwrap_err();
        assert!(matches!(error,
            crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
                if error.code == "LRR-E-OWNERSHIP"));
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.frame.source_view(destination).unwrap(), Some(expected));
        assert!(run.frame.executed_place_for_test(destination).is_err());
        assert!(run.frame.executed_place_for_test(base).is_ok());
        assert_eq!(
            (
                run.store.state(reference).unwrap(),
                run.store.state(target).unwrap()
            ),
            owners
        );
        assert_eq!(run.store.location_epoch(target).unwrap(), epoch);
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_nominal_deref_matching_destination_source_preserves_original_lease() {
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        let Some(TyTerm::Ref(_, false, pointee)) = run.frame.source_view(base).unwrap() else {
            panic!("original shared nominal source absent")
        };
        let expected = *pointee;
        run.frame
            .set_source_view(destination, Some(expected.clone()))
            .unwrap();
        let (reference, target) = original_nominal_owners(&run);
        let lease = run.store.reference_from_cell(reference).unwrap();
        let owners = (
            run.store.state(reference).unwrap(),
            run.store.state(target).unwrap(),
        );
        let epoch = run.store.location_epoch(target).unwrap();
        let before = guest_snapshot(&run);
        run.frame
            .preflight_deref_place(base, destination, &run.store)
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        run.frame
            .deref_place(base, destination, &run.store)
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.frame.source_view(destination).unwrap(), Some(expected));
        assert!(run.frame.executed_place_for_test(destination).is_ok());
        assert!(run.frame.executed_place_for_test(base).is_err());
        assert_eq!(run.store.reference_from_cell(reference).unwrap(), lease);
        assert_eq!(
            (
                run.store.state(reference).unwrap(),
                run.store.state(target).unwrap()
            ),
            owners
        );
        assert_eq!(run.store.location_epoch(target).unwrap(), epoch);
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_nominal_reference_deref_foreign_source_refuses_before_publication() {
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        let Some(TyTerm::Ref(region, mutable, pointee)) = run.frame.source_view(base).unwrap()
        else {
            panic!("original nominal source absent")
        };
        let TyTerm::Nominal(actual, args) = *pointee else {
            panic!("original nominal definition absent")
        };
        let foreign = Digest::of_bytes(b"foreign-nominal-dereference-source");
        assert_ne!(foreign, actual);
        run.frame
            .set_source_view(
                base,
                Some(TyTerm::Ref(
                    region,
                    mutable,
                    Box::new(TyTerm::Nominal(foreign, args)),
                )),
            )
            .unwrap();
        let (reference, target) = original_nominal_owners(&run);
        let owners = (
            run.store.state(reference).unwrap(),
            run.store.state(target).unwrap(),
        );
        let before = guest_snapshot(&run);
        let error = run
            .frame
            .preflight_deref_place(base, destination, &run.store)
            .unwrap_err();
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-OWNERSHIP")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(
            (
                run.store.state(reference).unwrap(),
                run.store.state(target).unwrap()
            ),
            owners
        );
        assert!(run.frame.source_view(destination).unwrap().is_none());
        assert!(run.frame.executed_place_for_test(destination).is_err());
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_nominal_reference_deref_missing_source_refuses_shape_erasure() {
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        run.frame.set_source_view(base, None).unwrap();
        let (reference, target) = original_nominal_owners(&run);
        let owners = (
            run.store.state(reference).unwrap(),
            run.store.state(target).unwrap(),
        );
        let before = guest_snapshot(&run);
        let error = run
            .frame
            .preflight_deref_place(base, destination, &run.store)
            .unwrap_err();
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-UNSUPPORTED")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(
            (
                run.store.state(reference).unwrap(),
                run.store.state(target).unwrap()
            ),
            owners
        );
        assert!(run.frame.source_view(destination).unwrap().is_none());
        assert!(run.frame.executed_place_for_test(destination).is_err());
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_nominal_reference_deref_moved_owner_refuses_stale_lease() {
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        let (reference, target) = original_nominal_owners(&run);
        let owner = run.store.state(reference).unwrap();
        let held = run.store.move_from(target).unwrap();
        let before = guest_snapshot(&run);
        let error = run
            .frame
            .preflight_deref_place(base, destination, &run.store)
            .unwrap_err();
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-STALE-HANDLE")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.store.state(reference).unwrap(), owner);
        assert_eq!(
            run.store.state(target).unwrap(),
            crate::runtime::cell::CellState::Moved
        );
        assert_eq!(run.store.peek_ticket(held).unwrap().kind, 8);
        assert!(run.frame.source_view(destination).unwrap().is_none());
        assert!(run.frame.executed_place_for_test(destination).is_err());
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_original_named_field_borrow_and_assignment_use_source_selected_member() {
        let (mut db, mut run, first, base, destination) = first_field();
        let source_definition = run.frame.nominal_place_source(base).unwrap();
        let schema = run.sources[0]
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "Point")
            .unwrap();
        assert_eq!(source_definition, schema.def_key.fingerprint());
        let before = guest_snapshot(&run);
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        let QueryValue::Thunk(recipe) = first.value.as_ref() else {
            panic!("first named projection absent")
        };
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        assert!(matches!(
            run.frame.source_view(destination).unwrap(),
            Some(TyTerm::Primitive(PrimitiveType::Isize))
        ));
        let mut reference = None;
        let mut result = None;
        for _ in 0..100 {
            let outcome = super::super::borrows::tests::next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock field continuation absent")
            };
            for op in &recipe.recipe().operations {
                if let Op::ProjectField { base, destination } = op {
                    run.prepare_field(&mut db, *base, *destination)
                        .into_ready()
                        .unwrap()
                        .unwrap();
                }
            }
            let borrow = recipe.recipe().operations.iter().find_map(|op| {
                if let Op::Borrow { destination, .. } = op {
                    Some(*destination)
                } else {
                    None
                }
            });
            let read = recipe.recipe().operations.iter().find_map(|op| {
                if let Op::ReadPlace { destination, .. } = op {
                    Some(*destination)
                } else {
                    None
                }
            });
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if let Some(destination) = borrow {
                reference = Some(
                    run.store
                        .reference_from_cell(run.frame.cell(destination).unwrap())
                        .unwrap(),
                );
                assert_eq!(run.frame.counters.started_by_opcode[33], 0);
            }
            if let Some(destination) = read {
                result = Some(
                    run.store
                        .preview_scalar(run.frame.cell(destination).unwrap(), None)
                        .unwrap(),
                );
                break;
            }
        }
        let (target, expected) = place_fixture_facts::assignment(STOCK, "d");
        assert_eq!(target, "p.y");
        let actual = result.expect("original *q was not reached");
        assert_eq!(
            actual.type_id,
            crate::frontend::call_contracts::primitive_runtime_id(PrimitiveType::Isize)
                .unwrap()
                .0
        );
        assert_eq!(actual.payload as i64, expected);
        assert_eq!(
            run.store.read_reference(reference.unwrap()).unwrap(),
            actual
        );
        assert_eq!(run.frame.counters.started_by_opcode[37], 2);
        assert_eq!(run.frame.counters.started_by_opcode[34], 1);
        assert_eq!(run.frame.counters.started_by_opcode[33], 1);
        assert_eq!(db.trace.counts.get("TypeSchema:ProviderRun"), Some(&1));
        assert!(!db.trace.counts.keys().any(|key| key.starts_with("Goal:")));
        // Ends at the original dereference. Aggregate scope cleanup and the
        // upstream compile-fail diagnostics are not exercised by this control.
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_named_field_without_dispatcher_binding_refuses_before_fuel_or_projection() {
        let (mut db, mut run, outcome, base, destination) = first_field();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("named field recipe absent")
        };
        let before = guest_snapshot(&run);
        let selected = recipe
            .recipe()
            .operations
            .iter()
            .find(|op| matches!(op, Op::ProjectField { .. }))
            .unwrap();
        let error = run
            .frame
            .prepare_projection(selected, &run.store)
            .err()
            .expect("unbound field projection was admitted");
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-INTERNAL-IR")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert!(!run.frame.has_source_field_for_test());
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        assert!(
            run.frame
                .prepare_projection(selected, &run.store)
                .unwrap()
                .is_some()
        );
        assert_eq!(guest_snapshot(&run), before);
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_named_field_changed_caller_header_refuses_before_guest_effects() {
        let (mut db, mut run, _, base, destination) = first_field();
        let caller = run.frame.body.body().definition;
        let original = db.input(&InputKey::DefinitionHeader(caller)).unwrap();
        let InputValue::DefinitionHeader(header) = original.as_ref() else {
            panic!("stock caller header absent")
        };
        let mut changed = header.clone();
        changed.visibility = crate::syntax::outline::Visibility::Public;
        let mut inputs = run.sources[0].prepared_inputs().clone();
        inputs.insert(
            InputKey::DefinitionHeader(caller),
            InputValue::DefinitionHeader(changed),
        );
        db.finish_run().unwrap();
        db.publish_inputs(inputs).unwrap();
        db.begin_run().unwrap();
        let before = guest_snapshot(&run);
        let error = run
            .prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-TYPE-SCHEMA");
        assert_eq!(guest_snapshot(&run), before);
        assert!(!run.frame.has_source_field_for_test());
        db.finish_run().unwrap();
        db.publish_inputs(run.sources[0].prepared_inputs().clone())
            .unwrap();
        db.begin_run().unwrap();
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_named_field_fuel_refusal_retains_bound_member_without_publishing_projection() {
        let (mut db, mut run, outcome, base, destination) = first_field();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("named field recipe absent")
        };
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        let before = (
            run.store.budget().handles(),
            run.store.budget().bytes(),
            run.frame.counters.operations_started,
        );
        run.frame.fuel = crate::vm::fuel::RunFuel::new(0).unwrap();
        let pc = run.frame.pc().clone();
        let error =
            step::step(&mut run.frame, &mut run.store, recipe).expect_err("fuel refusal absent");
        assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
        assert_eq!(run.frame.pc(), &pc);
        assert_eq!(run.frame.fuel_snapshot().used, 0);
        assert_eq!(
            (
                run.store.budget().handles(),
                run.store.budget().bytes(),
                run.frame.counters.operations_started
            ),
            before
        );
        assert!(run.frame.has_source_field_for_test());
        assert!(run.frame.nominal_place_source(base).is_ok());
        assert!(run.frame.nominal_place_source(destination).is_err());
        db.finish_run().unwrap();
    }
}
