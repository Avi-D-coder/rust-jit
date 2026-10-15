//! Unchanged float-nan runtime contract and LRR retained-source authority controls.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    engine::standalone_macros::CompletedStandaloneRuntimeMacros,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        parse::ParseKey,
    },
    traits::term::TyTerm,
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/float-nan.rs"
);
fn discovery(view: &PreparedSourceView, graph: Digest) -> SourceDiscovery {
    let mut d = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Type,
        graph,
        1,
    )
    .unwrap()
    .with_declarative(
        Digest::of_bytes(b"floating-dispatch-expansions"),
        Digest::of_bytes(b"floating-dispatch-generations"),
    )
    .unwrap()
    .with_compiler(std::sync::Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &view.selection().outline.parse.profile,
        )
        .unwrap(),
    ))
    .unwrap();
    assert!(matches!(d.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    d
}
fn start_with_fuel(
    edition: u16,
    fuel: u64,
) -> (
    Database,
    RunMachine,
    CompletedStandaloneRuntimeMacros,
    PreparedSourceInputs,
) {
    start_source(ORIGINAL, edition, fuel)
}
fn start_source(
    source: &str,
    edition: u16,
    fuel: u64,
) -> (
    Database,
    RunMachine,
    CompletedStandaloneRuntimeMacros,
    PreparedSourceInputs,
) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("float-nan.rs").unwrap();
    let unit = Digest::of_bytes(b"original-floating-dispatch");
    let selected = SourceSelection {
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
    let inputs = PreparedSourceInputs::refresh(
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
    inputs.publish(&mut db).unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selected).unwrap() else {
        panic!("source absent")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let d = discovery(&view, Digest::of_bytes(b"first-sealed-dispatch"));
    let macros = CompletedStandaloneRuntimeMacros::from_discovery(&view, &d).unwrap();
    db.begin_run().unwrap();
    let run = RunMachine::start_with_discovery(&mut db, body, fuel, d)
        .into_ready()
        .unwrap()
        .unwrap();
    (db, run, macros, inputs)
}
fn start(
    edition: u16,
) -> (
    Database,
    RunMachine,
    CompletedStandaloneRuntimeMacros,
    PreparedSourceInputs,
) {
    start_with_fuel(edition, 1000)
}
fn reach_first(db: &mut Database, run: &mut RunMachine) -> SlotId {
    let source = run.sources[0].share();
    let owner = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let at = u32::try_from(ORIGINAL.find("f64::NAN").unwrap()).unwrap() - owner.range.start;
    let node = run
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| {
            node.role == NodeRole::Expression(ExpressionKind::Path) && node.range.start == at
        })
        .unwrap();
    let target = run.frame.body.pc(node.id, Phase::Entry).unwrap();
    let destination = node.value.unwrap();
    for _ in 0..64 {
        if run.frame.pc() == &target {
            return destination;
        }
        let id = run
            .installed
            .resolve(db, run.frame.pc().clone())
            .into_ready()
            .unwrap()
            .unwrap();
        assert!(matches!(
            step::step(
                &mut run.frame,
                &mut run.store,
                run.installed.get(id).unwrap()
            )
            .unwrap(),
            StepOutcome::Next(_)
        ));
    }
    panic!("original first initializer was not reached")
}
#[test]
fn original_first_nan_dispatch_bootstraps_only_sealed_no_item_certificates_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run, macros, _) = start(edition);
        let contexts = macros.contexts().unwrap();
        assert!(!run.runtime_no_item_proofs.is_empty());
        reach_first(&mut db, &mut run);
        let recipe = run
            .prepare_floating_recipe(&mut db, &contexts)
            .into_ready()
            .unwrap()
            .unwrap()
            .unwrap();
        let fuel = run.frame.fuel_snapshot().used
            + u64::try_from(recipe.recipe().operations.len()).unwrap();
        db.finish_run().unwrap();
        let (mut db, mut run, macros, _) = start_with_fuel(edition, fuel);
        let contexts = macros.contexts().unwrap();
        let destination = reach_first(&mut db, &mut run);
        assert!(matches!(
            run.poll_with_runtime_macros(&mut db, &contexts),
            RunPoll::Fault(Diagnostic {
                code: "LRR-E-RUNTIME-RESOURCE",
                ..
            })
        ));
        assert_eq!(run.frame.counters.started_by_opcode[41], 1);
        let value = run
            .store
            .read_scalar(run.frame.cell(destination).unwrap())
            .unwrap();
        assert_eq!((value.kind, value.type_id), (7, 15));
        assert!(f64::from_bits(value.payload).is_nan());
        assert_eq!(
            run.frame.source_view(destination).unwrap(),
            Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::F64))
        );
        assert_eq!(run.frame.counters.started_by_opcode[26], 0);
        db.finish_run().unwrap();
        // First unchanged original initializer only; no let commit or is_nan.
    }
}
#[test]
fn installed_floating_proof_revalidates_across_derived_query_revision_before_entry() {
    let (mut db, mut run, macros, _) = start(2024);
    let contexts = macros.contexts().unwrap();
    reach_first(&mut db, &mut run);
    let recipe = run
        .prepare_floating_recipe(&mut db, &contexts)
        .into_ready()
        .unwrap()
        .unwrap()
        .unwrap();
    let id = run
        .installed
        .install_prepared(&db, run.frame.pc().clone(), &recipe)
        .unwrap();
    let source_revision = db.source_revision();
    let old = db.revision();
    discovery(
        &run.sources[0],
        Digest::of_bytes(b"second-sealed-query-revision"),
    )
    .publish(&mut db)
    .unwrap();
    assert_eq!(db.source_revision(), source_revision);
    assert!(db.revision() > old);
    let before = (run.frame.fuel_snapshot(), run.store.budget().handles());
    run.revalidate_floating_recipe(&mut db, &contexts, id)
        .into_ready()
        .unwrap()
        .unwrap();
    assert_eq!(
        (run.frame.fuel_snapshot(), run.store.budget().handles()),
        before
    );
    assert_eq!(run.frame.counters.started_by_opcode[41], 0);
    db.finish_run().unwrap();
}
#[test]
fn stale_source_refuses_retained_floating_dispatch_before_entry_or_ticket() {
    let (mut db, mut run, macros, inputs) = start(2024);
    let contexts = macros.contexts().unwrap();
    let destination = reach_first(&mut db, &mut run);
    let recipe = run
        .prepare_floating_recipe(&mut db, &contexts)
        .into_ready()
        .unwrap()
        .unwrap()
        .unwrap();
    run.installed
        .install_prepared(&db, run.frame.pc().clone(), &recipe)
        .unwrap();
    let before = (run.frame.fuel_snapshot(), run.store.budget().handles());
    db.finish_run().unwrap();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    assert!(matches!(
        run.poll_with_runtime_macros(&mut db, &contexts),
        RunPoll::Fault(Diagnostic {
            code: "LRR-E-INTERNAL-SNAPSHOT",
            ..
        })
    ));
    assert_eq!(
        (run.frame.fuel_snapshot(), run.store.budget().handles()),
        before
    );
    assert_eq!(run.frame.counters.started_by_opcode[41], 0);
    assert!(run.frame.cell(destination).is_err());
    db.finish_run().unwrap();
}
#[test]
fn floating_dispatch_fuel_refusal_does_not_enter_or_publish_scalar() {
    let (mut db, mut run, macros, _) = start(2024);
    let _contexts = macros.contexts().unwrap();
    reach_first(&mut db, &mut run);
    let used = run.frame.fuel_snapshot().used;
    db.finish_run().unwrap();
    let (mut db, mut run, macros, _) = start_with_fuel(2024, used);
    let contexts = macros.contexts().unwrap();
    let destination = reach_first(&mut db, &mut run);
    let handles = run.store.budget().handles();
    assert!(matches!(
        run.poll_with_runtime_macros(&mut db, &contexts),
        RunPoll::Fault(Diagnostic {
            code: "LRR-E-RUNTIME-RESOURCE",
            ..
        })
    ));
    assert_eq!(run.frame.fuel_snapshot().used, used);
    assert_eq!(run.store.budget().handles(), handles);
    assert_eq!(run.frame.counters.started_by_opcode[41], 0);
    assert!(run.frame.cell(destination).is_err());
    db.finish_run().unwrap();
}

#[test]
fn cached_floating_recipe_refuses_new_rigid_source_even_with_matching_descriptor() {
    let (mut db, mut run, macros, _) = start(2024);
    let contexts = macros.contexts().unwrap();
    let destination = reach_first(&mut db, &mut run);
    let recipe = run
        .prepare_floating_recipe(&mut db, &contexts)
        .into_ready()
        .unwrap()
        .unwrap()
        .unwrap();
    let id = run
        .installed
        .install_prepared(&db, run.frame.pc().clone(), &recipe)
        .unwrap();
    let rigid = TyTerm::Param(crate::traits::term::ParamKey {
        definition: run.frame.body.body().definition,
        ordinal: 0,
    });
    run.frame
        .set_source_view(destination, Some(rigid.clone()))
        .unwrap();
    run.frame
        .set_hint(destination, Some(crate::runtime::shape::TypeId(15)))
        .unwrap();
    let before = (run.frame.fuel_snapshot(), run.store.budget().handles());
    assert!(
        run.revalidate_floating_recipe(&mut db, &contexts, id)
            .into_ready()
            .unwrap()
            .is_err()
    );
    assert_eq!(
        (run.frame.fuel_snapshot(), run.store.budget().handles()),
        before
    );
    assert_eq!(run.frame.source_view(destination).unwrap(), Some(rigid));
    assert_eq!(run.frame.counters.started_by_opcode[41], 0);
    assert!(run.frame.cell(destination).is_err());
    db.finish_run().unwrap();
}

#[test]
fn unchanged_float_nan_whole_main_executes_all_assertions_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run, macros, _) = start_with_fuel(edition, 100_000);
        let contexts = macros.contexts().unwrap();
        let report = match run.poll_with_runtime_macros(&mut db, &contexts) {
            RunPoll::Completed(report) => report,
            RunPoll::Fault(error) => panic!(
                "unchanged float-nan.rs edition {edition}, pc {:?}, assertions {}: {error:?}",
                run.frame.pc(),
                run.frame.counters.started_by_opcode[26]
            ),
            RunPoll::Panicked(report) => {
                panic!("unchanged float-nan.rs edition {edition} panicked: {report:?}")
            }
            RunPoll::NeedInput(request) => {
                panic!("unchanged float-nan.rs edition {edition} suspended: {request:?}")
            }
        };
        assert!(matches!(report.value, ValueSummary::Unit));
        let assertions =
            ORIGINAL.matches("assert!(").count() + ORIGINAL.matches("assert_eq!(").count();
        assert_eq!(
            report.opcode_counters.started_by_opcode[26],
            assertions as u64
        );
        assert_eq!(report.opcode_counters.started_by_opcode[41], 3);
        assert_eq!(report.opcode_counters.started_by_opcode[7], 3);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        assert!(run.frame.active_scopes().is_empty());
        assert!(run.calls.is_empty() && run.pending.is_empty());
        assert!(run.returning.is_none() && run.unwinding.is_none());
        assert!(run.terminal);
        assert_eq!(
            run.primitive_methods.len(),
            ORIGINAL.matches(".is_nan()").count()
        );
        db.finish_run().unwrap();
    }
}

#[test]
fn primitive_float_method_receiver_call_is_evaluated_once() {
    // LRR exact-once protocol regression; the language witness is unchanged above.
    let source = "fn receiver() -> f64 { f64::NAN } fn main() { assert!(receiver().is_nan()); }";
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run, macros, _) = start_source(source, edition, 100_000);
        let contexts = macros.contexts().unwrap();
        let RunPoll::Completed(report) = run.poll_with_runtime_macros(&mut db, &contexts) else {
            panic!("receiver call did not complete");
        };
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[41], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[26], 1);
        assert_eq!(run.primitive_methods.len(), 1);
        assert!(run.frame.active_scopes().is_empty());
        db.finish_run().unwrap();
    }
}

#[test]
fn primitive_float_method_does_not_infer_source_from_runtime_numeric_defaults() {
    for source in [
        "fn main(){ assert!((0.0).is_nan()); }",
        "fn main(){ let n:i64=0; assert!(n.is_nan()); }",
    ] {
        for edition in [2015, 2018, 2021, 2024] {
            let (mut db, mut run, macros, _) = start_source(source, edition, 100_000);
            let contexts = macros.contexts().unwrap();
            let RunPoll::Fault(error) = run.poll_with_runtime_macros(&mut db, &contexts) else {
                panic!("unadmitted receiver acquired a primitive method");
            };
            assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
            assert!(run.primitive_methods.is_empty());
            assert_eq!(run.frame.counters.started_by_opcode[26], 0);
            db.finish_run().unwrap();
        }
    }
}

#[test]
fn primitive_float_method_literal_widths_and_cold_unresolved_receiver() {
    let source = "fn main(){ assert!((0.0f32/0.0).is_nan()); assert!(!(1.0f64).is_nan()); if false { assert!((0.0).is_nan()); } }";
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run, macros, _) = start_source(source, edition, 100_000);
        let contexts = macros.contexts().unwrap();
        let RunPoll::Completed(report) = run.poll_with_runtime_macros(&mut db, &contexts) else {
            panic!("written widths or cold receiver failed");
        };
        assert_eq!(report.opcode_counters.started_by_opcode[26], 2);
        assert_eq!(run.primitive_methods.len(), 2);
        assert!(run.frame.active_scopes().is_empty());
        db.finish_run().unwrap();
    }
}

// Stop on the real original source method after executing its receiver once.
// This is a retained-proof control, not a whole-fixture execution adapter.
fn prepare_first_method(
    db: &mut Database,
    run: &mut RunMachine,
    contexts: &[crate::engine::runtime_macros::RuntimeMacroContext<'_>],
) -> (crate::engine::install::RecipeId, SlotId) {
    for _ in 0..128 {
        let key = run.frame.pc().clone();
        let node = &run.frame.body.nodes()[key.node as usize];
        let prepared = if node.role == NodeRole::Expression(ExpressionKind::Macro) {
            contexts.iter().find_map(|context| {
                context
                    .resolve_at(
                        &run.frame.body,
                        &key,
                        run.runtime_no_item_proofs.values(),
                        Some((&run.frame, &run.store)),
                    )
                    .unwrap()
            })
        } else {
            None
        };
        let id = if let Some(prepared) = prepared {
            if let Some(proof) = prepared.no_item {
                run.retain_no_item_proof(proof).unwrap();
            }
            if let Some(method) = prepared.primitive_method {
                let receiver = match prepared.recipe.recipe().operations.as_ref() {
                    [
                        Op::NumericBinary {
                            operator: crate::ir::op::NumericBinary::Ne,
                            left,
                            right,
                            ..
                        },
                    ] if left == right => *left,
                    _ => panic!("selected method does not compare its single retained receiver"),
                };
                run.retain_primitive_method(&key, method, prepared.recipe.recipe().fingerprint())
                    .unwrap();
                let id = run
                    .installed
                    .install_prepared(db, key, &prepared.recipe)
                    .unwrap();
                return (id, receiver);
            }
            assert!(prepared.call.is_none());
            run.installed
                .install_prepared(db, key, &prepared.recipe)
                .unwrap()
        } else if let Some(recipe) = run
            .prepare_floating_recipe(db, contexts)
            .into_ready()
            .unwrap()
            .unwrap()
        {
            run.installed.install_prepared(db, key, &recipe).unwrap()
        } else {
            run.installed
                .resolve(db, key)
                .into_ready()
                .unwrap()
                .unwrap()
        };
        match step::step(
            &mut run.frame,
            &mut run.store,
            run.installed.get(id).unwrap(),
        )
        .unwrap()
        {
            // Dispatch is documented bounded operation progress at the same PC;
            // replay the retained opcode until its real exit, within this bound.
            StepOutcome::Next(_) | StepOutcome::Dispatch => {}
            outcome => panic!(
                "unexpected original receiver outcome at {:?}: {outcome:?}",
                run.frame.pc()
            ),
        }
    }
    panic!("original first primitive method was not reached")
}

#[test]
fn primitive_float_cached_method_rechecks_source_type_and_actual_descriptor_before_effects() {
    for primitive in [
        crate::ir::op::PrimitiveType::F64,
        crate::ir::op::PrimitiveType::F32,
    ] {
        let (mut db, mut run, macros, _) = start(2024);
        let contexts = macros.contexts().unwrap();
        let (id, receiver) = prepare_first_method(&mut db, &mut run, &contexts);
        run.revalidate_primitive_method(&mut db, &contexts, id)
            .unwrap();
        let replacement = if primitive == crate::ir::op::PrimitiveType::F64 {
            TyTerm::Param(crate::traits::term::ParamKey {
                definition: run.root_body.definition,
                ordinal: 0,
            })
        } else {
            TyTerm::Primitive(primitive)
        };
        run.frame
            .set_source_view(receiver, Some(replacement))
            .unwrap();
        let before = (
            run.frame.fuel_snapshot(),
            run.store.budget().handles(),
            run.store.budget().bytes(),
            serde_json::to_value(&run.frame.counters).unwrap(),
        );
        let error = run
            .revalidate_primitive_method(&mut db, &contexts, id)
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
        assert_eq!(
            (
                run.frame.fuel_snapshot(),
                run.store.budget().handles(),
                run.store.budget().bytes(),
                serde_json::to_value(&run.frame.counters).unwrap()
            ),
            before
        );
        assert_eq!(run.frame.counters.started_by_opcode[26], 0);
        db.finish_run().unwrap();
    }
}

#[test]
fn primitive_float_cached_method_requires_current_context_and_source_generation() {
    let (mut db, mut run, macros, inputs) = start(2024);
    let contexts = macros.contexts().unwrap();
    let (id, _) = prepare_first_method(&mut db, &mut run, &contexts);
    let before = (run.frame.fuel_snapshot(), run.store.budget().handles());
    assert!(run.revalidate_primitive_method(&mut db, &[], id).is_err());
    run.revalidate_primitive_method(&mut db, &contexts, id)
        .unwrap();
    db.finish_run().unwrap();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    assert!(matches!(
        run.poll_with_runtime_macros(&mut db, &contexts),
        RunPoll::Fault(Diagnostic {
            code: "LRR-E-INTERNAL-SNAPSHOT",
            ..
        })
    ));
    assert_eq!(
        (run.frame.fuel_snapshot(), run.store.budget().handles()),
        before
    );
    assert_eq!(run.frame.counters.started_by_opcode[26], 0);
    db.finish_run().unwrap();
}
