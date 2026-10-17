//! Stock source controls for the entered local-borrow path.
#[cfg(test)]
pub(super) mod tests {
    use super::super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        ir::op::BorrowAccess,
        runtime::{cell::RefId, reference::Access},
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    const ALIASES: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/mut-borrow-outside-loop.rs"
    );
    const IMMUTABLE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-mut-addr-of-imm-var.rs"
    );
    const ANNOTATED: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-simple.rs"
    );
    pub(in crate::engine::run) fn start_stock(source: &str) -> (Database, RunMachine) {
        start_stock_body(source, "main")
    }
    pub(in crate::engine::run) fn start_stock_body(
        source: &str,
        name: &str,
    ) -> (Database, RunMachine) {
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
        let path = SourcePath::new("borrow.rs").unwrap();
        let unit = Digest::of_bytes(b"rustc-local-borrow-prefix");
        let selection = SourceSelection {
            path: path.clone(),
            outline: crate::db::key::OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let prepared = PreparedSourceInputs::refresh(
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
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
            panic!("stock source absent")
        };
        let body = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == name)
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        db.begin_run().unwrap();
        let run = RunMachine::start_with_sources(&mut db, body, 1000, vec![view.share()])
            .into_ready()
            .unwrap()
            .unwrap();
        (db, run)
    }
    /// The production standalone route: a sealed discovery that expands every
    /// reached macro (including the library's own `panic!`/`assert!`
    /// definitions), the retained entry body, and a discovery-backed run with
    /// the completed runtime macro contexts. Callers poll with
    /// `run.poll_with_runtime_macros(db, &macros.contexts()?)`.
    pub(crate) fn start_discovered_body(
        source: &str,
        name: &str,
        edition: u16,
    ) -> (
        Database,
        RunMachine,
        crate::engine::standalone_macros::CompletedStandaloneRuntimeMacros,
    ) {
        let (mut db, view, discovery) =
            super::super::lexical_numeric_callees_tests::setup(source, edition);
        db.begin_expansion_epoch(1).unwrap();
        discovery.publish(&mut db).unwrap();
        let selected = match discovery.entry_function(&mut db, name) {
            crate::db::poll::QueryPoll::Ready(Ok(
                crate::frontend::callables::CallSelection::Source(selected),
            )) => selected,
            other => panic!("entry {name} did not select a source function: {other:?}"),
        };
        let selected =
            crate::cli::retained_entry::prepare(&mut db, &discovery, &view, name, selected)
                .unwrap();
        selected.revalidate(&mut db).unwrap();
        let body = selected.body().expect("selected entry body").key();
        let macros = crate::engine::standalone_macros::CompletedStandaloneRuntimeMacros::from_discovery(
            &view, &discovery,
        )
        .unwrap();
        let run = RunMachine::start_with_discovery(&mut db, body, 1_000_000, discovery)
            .into_ready()
            .unwrap()
            .unwrap();
        db.begin_run().unwrap();
        (db, run, macros)
    }
    pub(in crate::engine::run) fn next(
        db: &mut Database,
        run: &mut RunMachine,
    ) -> crate::db::memo::Outcome {
        let outcome = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(run.frame.pc().clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("stock thunk absent")
        };
        if recipe
            .recipe()
            .operations
            .iter()
            .any(|op| matches!(op, Op::ConstructStart { .. }))
        {
            run.prepare_constructor(db, &[])
                .into_ready()
                .unwrap()
                .unwrap();
        }
        outcome
    }
    #[test]
    fn rustc_original_mutable_alias_borrow_prefix_executes_without_copying_target() {
        let (mut db, mut run) = start_stock(ALIASES);
        let mut references: Vec<RefId> = Vec::new();
        for _ in 0..100 {
            let outcome = next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock thunk absent")
            };
            let borrow = recipe.recipe().operations.iter().find_map(|op| match op {
                Op::Borrow {
                    access,
                    destination,
                    ..
                } => Some((*access, *destination)),
                _ => None,
            });
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if let Some((access, destination)) = borrow {
                assert_eq!(access, BorrowAccess::Mutable);
                references.push(
                    run.store
                        .reference_from_cell(run.frame.cell(destination).unwrap())
                        .unwrap(),
                );
                if references.len() == 2 {
                    break;
                }
            }
        }
        assert_eq!(references.len(), 2);
        assert_ne!(references[0], references[1]);
        assert_eq!(run.frame.counters.started_by_opcode[30], 2);
        assert_eq!(run.frame.counters.started_by_opcode[31], 2);
        assert_eq!(run.frame.counters.started_by_opcode[8], 0); // no ReadLocal of void
        for reference in &references {
            let incoming = run.store.ticket(ValueRepr::unit()).unwrap();
            let old = run.store.replace_reference(*reference, incoming).unwrap();
            run.store.discard(old).unwrap();
            for alias in &references {
                assert_eq!(run.store.read_reference(*alias).unwrap(), ValueRepr::unit());
            }
        }
        let shared = run
            .store
            .borrow(
                &crate::runtime::place::Place::deref(references[0]),
                Access::Shared,
            )
            .unwrap();
        assert_eq!(run.store.read_reference(shared).unwrap(), ValueRepr::unit());
        // This stops immediately after the second original borrow. Fake methods,
        // the loop and its compile-fail stderr are deliberately not accepted.
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_original_primitive_reference_annotation_reaches_checked_borrow() {
        let (mut db, mut run) = start_stock(ANNOTATED);
        let mut reached = false;
        for _ in 0..100 {
            let outcome = next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock thunk absent")
            };
            let destination = recipe.recipe().operations.iter().find_map(|op| match op {
                Op::Borrow {
                    destination,
                    access: BorrowAccess::Mutable,
                    ..
                } => Some(*destination),
                _ => None,
            });
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if let Some(destination) = destination {
                let cell = run.frame.cell(destination).unwrap();
                let reference = run.store.reference_from_cell(cell).unwrap();
                let value = run.store.read_reference(reference).unwrap();
                assert_eq!(value.type_id, 8);
                assert_eq!(value.payload, 3);
                let source = run.frame.source_view(destination).unwrap().unwrap();
                let crate::traits::term::TyTerm::Ref(region, mutable, target) = source else {
                    panic!("written reference source identity absent")
                };
                assert!(mutable);
                assert_eq!(
                    target.as_ref(),
                    &crate::traits::term::TyTerm::Primitive(crate::ir::op::PrimitiveType::Isize)
                );
                assert!(
                    matches!(region, crate::traits::term::RegionTerm::Local { definition, start, end }
                    if definition == run.frame.body.body().definition && start < end)
                );
                assert_eq!(run.frame.counters.started_by_opcode[30], 1);
                assert_eq!(run.frame.counters.started_by_opcode[31], 1);
                assert_eq!(run.frame.counters.started_by_opcode[8], 0);
                reached = true;
                break;
            }
        }
        assert!(reached);
        // Stops before original dereference assignment and printing; not a full fixture pass.
        db.finish_run().unwrap();
        // Supplementary LRR context tampering on the unchanged stock borrow PC.
        for (mutable, target) in [
            (false, crate::ir::op::PrimitiveType::Isize),
            (true, crate::ir::op::PrimitiveType::I64),
        ] {
            let (mut db, mut run) = start_stock(ANNOTATED);
            let mut refused = false;
            for _ in 0..100 {
                let outcome = next(&mut db, &mut run);
                let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                    panic!("stock thunk absent")
                };
                if let Some(destination) = recipe.recipe().operations.iter().find_map(|op| match op
                {
                    Op::Borrow { destination, .. } => Some(*destination),
                    _ => None,
                }) {
                    run.frame
                        .set_source_view(
                            destination,
                            Some(crate::traits::term::TyTerm::Ref(
                                crate::traits::term::RegionTerm::Static,
                                mutable,
                                Box::new(crate::traits::term::TyTerm::Primitive(target)),
                            )),
                        )
                        .unwrap();
                    let handles = run.store.budget().handles();
                    let bytes = run.store.budget().bytes();
                    let fuel = run.frame.fuel.snapshot().used;
                    assert_eq!(
                        step::step(&mut run.frame, &mut run.store, recipe)
                            .unwrap_err()
                            .code,
                        "LRR-E-OWNERSHIP"
                    );
                    assert_eq!(run.store.budget().handles(), handles);
                    assert_eq!(run.store.budget().bytes(), bytes);
                    assert_eq!(run.frame.fuel.snapshot().used, fuel + 1);
                    assert_eq!(run.frame.counters.started_by_opcode[31], 0);
                    refused = true;
                    break;
                }
                assert!(matches!(
                    step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                    StepOutcome::Next(_)
                ));
            }
            assert!(refused);
            db.finish_run().unwrap();
        }
    }
    #[test]
    fn rustc_original_dereference_assignment_replaces_scalar_after_rhs() {
        let (mut db, mut run) = start_stock(ANNOTATED);
        let (_, expected) = crate::syntax::place_fixture_facts::assignment(ANNOTATED, "main");
        let mut reference = None;
        let mut assigned = false;
        for _ in 0..200 {
            let outcome = next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock thunk absent")
            };
            let borrowed = recipe.recipe().operations.iter().find_map(|op| match op {
                Op::Borrow { destination, .. } => Some(*destination),
                _ => None,
            });
            let assignment = recipe.recipe().operations.iter().find_map(|op| match op {
                Op::AssignPlace { source, .. } => Some(*source),
                _ => None,
            });
            if let Some(source) = assignment {
                let evaluated = run
                    .store
                    .preview_scalar(
                        run.frame.cell(source).unwrap(),
                        Some(crate::runtime::shape::TypeId(8)),
                    )
                    .unwrap();
                assert_eq!(evaluated.payload as i64, expected);
                // Resolving the LHS cannot consume/copy the mutable reference.
                assert_eq!(run.frame.counters.started_by_opcode[8], 0);
                assert_eq!(run.frame.counters.started_by_opcode[32], 1);
            }
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if let Some(destination) = borrowed {
                reference = Some(
                    run.store
                        .reference_from_cell(run.frame.cell(destination).unwrap())
                        .unwrap(),
                );
            }
            if assignment.is_some() {
                let after = run.store.read_reference(reference.unwrap()).unwrap();
                assert_eq!(after.payload as i64, expected);
                assert_eq!(run.frame.counters.started_by_opcode[34], 1);
                assigned = true;
                break;
            }
        }
        assert!(assigned);
        // Stops before original println macro. Full run-pass oracle remains pending.
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_dereference_entry_without_ordinary_operations_advances_once_with_exit_fuel() {
        let (mut db, mut run) = start_stock(ANNOTATED);
        let range = crate::syntax::place_fixture_facts::dereference_ranges(ANNOTATED)[0];
        let selected = run
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
            .id;
        for _ in 0..200 {
            let outcome = next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock continuation thunk absent")
            };
            if recipe.recipe().key.node == selected.0
                && recipe.recipe().key.phase == Phase::Entry.code().unwrap()
            {
                assert!(recipe.recipe().operations.is_empty());
                let crate::ir::recipe::RecipeExit::Next(expected) = &recipe.recipe().exit else {
                    panic!("entry must continue to its receiver")
                };
                let child = run.frame.body.nodes()[selected.0 as usize].operands[0];
                assert_eq!(expected, &run.frame.body.pc(child, Phase::Entry).unwrap());
                assert!(db.memo(&QueryKey::Thunk(expected.clone())).is_none());
                let before = (
                    run.frame.fuel.snapshot().used,
                    run.frame.counters.operations_started,
                    run.frame.counters.exits_completed,
                    run.store.budget().handles(),
                    run.store.budget().bytes(),
                );
                let StepOutcome::Next(actual) =
                    step::step(&mut run.frame, &mut run.store, recipe).unwrap()
                else {
                    panic!("entry must advance")
                };
                assert_eq!(&actual, expected);
                assert_eq!(run.frame.pc(), expected);
                assert_eq!(run.frame.fuel.snapshot().used, before.0 + 1);
                assert_eq!(run.frame.counters.operations_started, before.1);
                assert_eq!(run.frame.counters.exits_completed, before.2 + 1);
                assert_eq!(
                    (run.store.budget().handles(), run.store.budget().bytes()),
                    (before.3, before.4)
                );
                assert!(db.memo(&QueryKey::Thunk(expected.clone())).is_none());
                db.finish_run().unwrap();
                return;
            }
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
        }
        panic!("stock dereference Entry not reached");
    }
    #[test]
    fn rustc_dereference_assignment_shared_lease_refuses_before_write_effects() {
        let (mut db, mut run) = start_stock(ANNOTATED);
        let mut shared = None;
        let mut refused = false;
        for _ in 0..200 {
            let outcome = next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock thunk absent")
            };
            let borrowed = recipe.recipe().operations.iter().find_map(|op| match op {
                Op::Borrow { destination, .. } => Some(*destination),
                _ => None,
            });
            if let Some(place) = recipe.recipe().operations.iter().find_map(|op| match op {
                Op::AssignPlace { place, .. } => Some(*place),
                _ => None,
            }) {
                // LRR-only permission injection at the unchanged stock write PC.
                run.frame
                    .inject_test_place(place, crate::runtime::place::Place::deref(shared.unwrap()))
                    .unwrap();
                let old = run.store.read_reference(shared.unwrap()).unwrap();
                let fuel = run.frame.fuel.snapshot().used;
                let handles = run.store.budget().handles();
                let bytes = run.store.budget().bytes();
                assert_eq!(
                    step::step(&mut run.frame, &mut run.store, recipe)
                        .unwrap_err()
                        .code,
                    "LRR-E-REFERENCE-ACCESS"
                );
                assert_eq!(run.store.read_reference(shared.unwrap()).unwrap(), old);
                assert_eq!(run.frame.fuel.snapshot().used, fuel);
                assert_eq!(run.store.budget().handles(), handles);
                assert_eq!(run.store.budget().bytes(), bytes);
                assert_eq!(run.frame.counters.started_by_opcode[34], 0);
                refused = true;
                break;
            }
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if let Some(destination) = borrowed {
                let reference = run
                    .store
                    .reference_from_cell(run.frame.cell(destination).unwrap())
                    .unwrap();
                shared = Some(
                    run.store
                        .borrow(
                            &crate::runtime::place::Place::deref(reference),
                            Access::Shared,
                        )
                        .unwrap(),
                );
            }
        }
        assert!(refused);
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_dereference_receiver_stale_lease_refuses_before_dereference_fuel() {
        let (mut db, mut run) = start_stock(ANNOTATED);
        let mut referent = None;
        let mut refused = false;
        for _ in 0..200 {
            let outcome = next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock thunk absent")
            };
            if recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::Borrow { .. }))
            {
                let binding = recipe
                    .recipe()
                    .operations
                    .iter()
                    .find_map(|op| match op {
                        Op::MakePlace { binding, .. } => Some(*binding),
                        _ => None,
                    })
                    .unwrap();
                referent = Some(run.frame.binding_cell_for_test(binding).unwrap());
            }
            if recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::DerefPlace { .. }))
            {
                // Supplementary LRR move invalidates the referent lease while
                // leaving the original receiver/reference source bits intact.
                let moved = run.store.move_from(referent.unwrap()).unwrap();
                run.store.discard(moved).unwrap();
                let fuel = run.frame.fuel.snapshot().used;
                let handles = run.store.budget().handles();
                let bytes = run.store.budget().bytes();
                assert_eq!(
                    step::step(&mut run.frame, &mut run.store, recipe)
                        .unwrap_err()
                        .code,
                    "LRR-E-STALE-HANDLE"
                );
                assert_eq!(run.frame.fuel.snapshot().used, fuel);
                assert_eq!(run.store.budget().handles(), handles);
                assert_eq!(run.store.budget().bytes(), bytes);
                assert_eq!(run.frame.counters.started_by_opcode[32], 0);
                assert_eq!(run.frame.counters.started_by_opcode[34], 0);
                assert_eq!(run.frame.status(), crate::vm::frame::FrameStatus::Faulted);
                refused = true;
                break;
            }
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
        }
        assert!(refused);
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_original_immutable_local_borrow_faults_before_borrow_fuel_and_allocation() {
        let (mut db, mut run) = start_stock(IMMUTABLE);
        let mut reached = false;
        for _ in 0..100 {
            let outcome = next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock thunk absent")
            };
            if recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::Borrow { .. }))
            {
                let handles = run.store.budget().handles();
                let bytes = run.store.budget().bytes();
                let fuel = run.frame.fuel.snapshot().used;
                assert_eq!(
                    step::step(&mut run.frame, &mut run.store, recipe)
                        .unwrap_err()
                        .code,
                    "LRR-E-REFERENCE-ACCESS"
                );
                assert_eq!(run.store.budget().handles(), handles);
                assert_eq!(run.store.budget().bytes(), bytes);
                assert_eq!(run.frame.fuel.snapshot().used, fuel + 1); // successful MakePlace only
                assert_eq!(run.frame.counters.started_by_opcode[30], 1);
                assert_eq!(run.frame.counters.started_by_opcode[31], 0);
                assert_eq!(run.frame.status(), crate::vm::frame::FrameStatus::Faulted);
                let before = run.frame.fuel.snapshot().used;
                assert_eq!(
                    step::step(&mut run.frame, &mut run.store, recipe)
                        .unwrap_err()
                        .code,
                    "LRR-E-INTERNAL-IR"
                );
                assert_eq!(run.frame.fuel.snapshot().used, before);
                reached = true;
                break;
            }
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
        }
        assert!(reached);
        // Original assignment/printing and rustc diagnostic oracle are unexecuted.
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_local_borrow_retained_rigid_source_refuses_before_place_fuel_or_allocation() {
        let (mut db, mut run) = start_stock(ALIASES);
        let mut reached = false;
        for _ in 0..100 {
            let outcome = next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock thunk absent")
            };
            if let Some(binding) = recipe.recipe().operations.iter().find_map(|op| match op {
                Op::MakePlace { binding, .. } => Some(*binding),
                _ => None,
            }) {
                // LRR source-view refusal injection on the original borrow PC.
                // It does not pretend the original nongeneric main has a bound.
                let key = crate::traits::term::ParamKey {
                    definition: run.frame.body.body().definition,
                    ordinal: 0,
                };
                run.frame
                    .set_binding_source(binding, &crate::ir::op::HintRecipe::Parameter(key))
                    .unwrap();
                let before = (
                    run.store.budget().handles(),
                    run.store.budget().bytes(),
                    run.frame.fuel.snapshot().used,
                );
                let error = step::step(&mut run.frame, &mut run.store, recipe).unwrap_err();
                assert_eq!(error.code, "LRR-E-UNSUPPORTED");
                assert_eq!(
                    error.message,
                    "local borrow requires retained symbolic reference source integration"
                );
                assert_eq!(
                    before,
                    (
                        run.store.budget().handles(),
                        run.store.budget().bytes(),
                        run.frame.fuel.snapshot().used
                    )
                );
                assert_eq!(run.frame.counters.started_by_opcode[30], 0);
                assert_eq!(run.frame.counters.started_by_opcode[31], 0);
                assert_eq!(run.frame.status(), crate::vm::frame::FrameStatus::Faulted);
                reached = true;
                break;
            }
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
        }
        assert!(reached);
        db.finish_run().unwrap();
    }
}
