//! Original first-initializer dispatch only, plus retained-proof controls.
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
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
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
fn unchanged_float_nan_main_commits_first_binding_then_reaches_natural_method_frontier() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run, macros, _) = start(edition);
        let contexts = macros.contexts().unwrap();
        let RunPoll::Fault(error) = run.poll_with_runtime_macros(&mut db, &contexts) else {
            panic!("unchanged original did not reach the expected unsupported method frontier")
        };
        assert_eq!(error.code, "LRR-E-UNSUPPORTED");
        assert_eq!(
            error.message,
            "reached assertion operand requires pending non-scalar expression integration"
        );
        assert_eq!(run.frame.counters.started_by_opcode[41], 1);
        assert_eq!(run.frame.counters.started_by_opcode[7], 1);
        assert_eq!(run.frame.counters.started_by_opcode[26], 0);
        assert!(run.frame.fuel_snapshot().used < 1000);
        assert_eq!(run.frame.fuel_snapshot().stop, None);

        // The natural current PC is the original assertion containing is_nan;
        // the pure scalar-operand planner rejects its MethodCallExpr before the
        // assertion opcode or later INFINITY initializer can execute.
        let written = "assert!(nan.is_nan())";
        let source = &run.sources[0];
        let handle = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let absolute = u32::try_from(ORIGINAL.find(written).unwrap()).unwrap();
        let expected = crate::syntax::spans::BodySpan {
            body_key: run.root_body.clone(),
            relative_start: absolute - handle.range.start,
            relative_end: absolute - handle.range.start + u32::try_from(written.len()).unwrap(),
        };
        let pc = run.frame.pc();
        assert_eq!(pc.phase, Phase::Entry.code().unwrap());
        let node = &run.frame.body.nodes()[pc.node as usize];
        assert_eq!(node.role, NodeRole::Expression(ExpressionKind::Macro));
        assert_eq!(run.frame.body.span(node.id).unwrap(), expected);
        let probe = node.runtime_temporaries[0].unwrap();
        let stack = error
            .guest_stack
            .as_ref()
            .expect("natural fault source origin absent");
        assert_eq!(stack.frames.len(), 1);
        assert_eq!(stack.omitted_frames, 0);
        assert_eq!(stack.frames[0].pc, pc.fingerprint());
        assert_eq!(stack.frames[0].body, expected);
        assert_eq!(stack.frames[0].locations.len(), 1);
        let at = &stack.frames[0].locations[0];
        assert_eq!(at.range.start, absolute);
        assert_eq!(
            at.range.end,
            absolute + u32::try_from(written.len()).unwrap()
        );
        assert_eq!(
            &ORIGINAL[at.range.start as usize..at.range.end as usize],
            written
        );
        assert_eq!(
            at.source.content.0,
            source.selection().outline.parse.content.0
        );
        assert_eq!(at.source.unit.0, source.selection().outline.crate_key.0);
        assert_eq!(at.source.path, source.selection().path.as_str());
        assert!(error.origin_attachment_failure.is_none());

        let bindings: Vec<_> = run
            .installed
            .recipes()
            .flat_map(|recipe| recipe.recipe().operations.iter())
            .filter_map(|op| match op {
                Op::BindLocal { binding, .. } => Some(*binding),
                _ => None,
            })
            .collect();
        assert_eq!(bindings.len(), 1);
        let binding = bindings[0];
        let span = run.frame.body.span(binding).unwrap();
        assert_eq!(
            &ORIGINAL[(handle.range.start + span.relative_start) as usize
                ..(handle.range.start + span.relative_end) as usize],
            "nan"
        );
        let cell = run.frame.binding_cell_for_test(binding).unwrap();
        let value = run.store.read_scalar(cell).unwrap();
        assert_eq!((value.kind, value.type_id), (7, 15));
        assert!(f64::from_bits(value.payload).is_nan());
        assert_eq!(
            run.store.declared(cell).unwrap(),
            crate::runtime::cell::StorageContract::Exact(crate::runtime::shape::TypeId(15))
        );
        assert_eq!(
            run.store.state(cell).unwrap(),
            crate::runtime::cell::CellState::Live(value)
        );

        // Existing local_hint is a metadata-only projection of the committed
        // binding's source table. The current rejected assertion has an unused
        // value slot, so this inspection allocates no cell/ticket and changes no
        // executed counters or fuel. It does not execute the unsupported method.
        assert!(run.frame.cell(probe).is_err());
        let before = (
            run.frame.fuel_snapshot(),
            serde_json::to_value(&run.frame.counters).unwrap(),
            run.store.budget().handles(),
            run.store.budget().bytes(),
        );
        run.frame.local_hint(binding, probe, &run.store).unwrap();
        assert_eq!(
            run.frame.hint(probe).unwrap(),
            Some(crate::runtime::shape::TypeId(15))
        );
        assert_eq!(
            run.frame.source_view(probe).unwrap(),
            Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::F64))
        );
        assert_eq!(
            (
                run.frame.fuel_snapshot(),
                serde_json::to_value(&run.frame.counters).unwrap(),
                run.store.budget().handles(),
                run.store.budget().bytes()
            ),
            before
        );
        assert_eq!(run.store.read_scalar(cell).unwrap(), value);
        db.finish_run().unwrap();
        // Actual original progress ends at the natural first is_nan frontier.
        // This test requires unsupported, and is not a full fixture pass.
    }
}
