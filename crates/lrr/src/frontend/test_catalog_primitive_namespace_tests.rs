//! Real TestCatalog publications, not caller-created namespace completion flags.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::{key::OutlineKey, value::QueryValue},
    engine::{
        run::{RunMachine, RunPoll, ValueSummary},
        standalone_macros::CompletedStandaloneRuntimeMacros,
        test_catalog::TestCatalog,
    },
    expand::{
        source_declarative,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::{
        callables::{CallResolver, CallSelection},
        compile_input_catalog::CompileInputCatalog,
    },
    ir::pc::Phase,
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        parse::ParseKey,
    },
    vm::context::GenericEnv,
};
use std::sync::Arc;

const SOURCE: &str = "use std::assert as check; fn predicate()->bool{true} #[test]fn chosen(){check!(predicate(), \"{}\", missing());} #[test]fn broken(){missing_unselected();}";

struct World {
    db: Database,
    source: PreparedSourceView,
    discovery: SourceDiscovery,
    catalog: TestCatalog,
}
fn world(text: &str, edition: u16) -> World {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let unit = Digest::of_bytes(b"LRR genuine TestCatalog primitive namespace");
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("test-catalog-primitive.rs").unwrap();
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
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
            profile: profile.clone(),
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap() else {
        panic!("actual original source absent");
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::for_tests(
        vec![source.share()],
        source.outline().root_scope.clone(),
        unit,
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR actual test compiler environment"),
        source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&profile).unwrap()),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::NeedExpansion { request } => {
                discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            _ => panic!("source requires additional real declaration authority"),
        }
    }
    discovery.publish(&mut db).unwrap();
    assert!(matches!(
        &discovery.key().domain,
        crate::db::seals::SealDomain::TestCatalog { .. }
    ));
    // `--test` discovery publishes no Type facets under the test-catalog
    // seal; Type authority comes from the root Type fixed point.
    let Some((InputValue::DiscoverySeal(_), _)) =
        db.published_input(&InputKey::DiscoverySeal(discovery.key().clone()))
    else {
        panic!("actual published TestCatalog absent");
    };
    let catalog = discovery.test_catalog(&mut db).unwrap();
    World {
        db,
        source,
        discovery,
        catalog,
    }
}
fn predicate(world: &mut World) -> Box<SourceCallable> {
    let graph = world.catalog.runtime_environment().unwrap().names();
    let caller = world
        .source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "chosen")
        .unwrap();
    let sources = [&world.source];
    let resolver = CallResolver::new(graph, &sources).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut world.db, caller.def_key.fingerprint(), "predicate")
    else {
        panic!("actual original predicate unavailable");
    };
    callee
}
fn packet(world: &mut World, callee: &SourceCallable) -> PreparedCallContracts {
    let QueryPoll::Ready(Ok(ContractPreparation::Ready(packet))) = prepare_contracts(
        &mut world.db,
        callee,
        world.catalog.runtime_environment().unwrap().names(),
        &[&world.source],
        &[],
        None,
        &[],
    ) else {
        panic!("actual nested TestCatalog Type receipt refused");
    };
    packet
}

#[test]
fn genuine_test_catalog_primitive_result_certificate_keeps_owning_type_authority_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut w = world(SOURCE, edition);
        let callee = predicate(&mut w);
        let packet = packet(&mut w, &callee);
        assert_eq!(
            packet.source_result,
            Some(TyTerm::Primitive(PrimitiveType::Bool))
        );
        assert!(packet.normalized_deferred_return.is_some());
        assert!(GenericEnv::capture_current(&mut w.db, &callee, &packet).is_ok());
    }
}
#[test]
fn selected_aliased_assertion_executes_predicate_and_keeps_message_and_unselected_body_cold_all_editions()
 {
    for edition in [2015, 2018, 2021, 2024] {
        let mut w = world(SOURCE, edition);
        let body = |name: &str| {
            w.source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == name)
                .unwrap()
                .body
                .as_ref()
                .unwrap()
                .key()
        };
        let chosen = body("chosen");
        let predicate = body("predicate");
        let broken = body("broken");
        let selected = w.catalog.select(Some("chosen"), true);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].body.as_ref(), Some(&chosen));
        let macros =
            CompletedStandaloneRuntimeMacros::from_discovery(&w.source, &w.discovery).unwrap();
        let contexts = macros.contexts().unwrap();
        w.db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut run)) = RunMachine::start_with_test_catalog(
            &mut w.db,
            chosen.clone(),
            100_000,
            w.discovery.completed_test_sources().unwrap(),
            &w.catalog,
        ) else {
            panic!("actual selected test startup failed");
        };
        let outcome = run.poll_with_runtime_macros(&mut w.db, &contexts);
        w.db.finish_run().unwrap();
        let report = match outcome {
            RunPoll::Completed(report) => report,
            RunPoll::Fault(error) => panic!("selected original-source call failed: {error}"),
            RunPoll::Panicked(report) => {
                panic!("selected original-source call panicked: {report:?}")
            }
            RunPoll::NeedInput(input) => panic!("selected original-source call requires {input:?}"),
        };
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(report.live_edition, edition);
        // Legacy zero-argument assertion success branches to Unit at Exit;
        // only the unreached failure Commit emits an Assert opcode.
        assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
        assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        assert!(
            w.db.trace
                .events
                .iter()
                .any(|event| matches!(&event.key, QueryKey::Thunk(pc) if pc.body == predicate))
        );
        assert!(w.db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) => body != &broken,
            QueryKey::Thunk(pc) => pc.body != broken,
            _ => true,
        }));
        let QueryPoll::Ready(Ok(value)) =
            w.db.fetch(&SyntaxProvider, QueryKey::BodyStructure(chosen.clone()))
        else {
            panic!("selected index absent");
        };
        let QueryValue::BodyStructure(index) = value.value.as_ref() else {
            panic!("selected index variant");
        };
        assert_eq!(index.body(), &chosen);
        assert_eq!(index.profile().edition, edition);
        assert_eq!(
            w.db.trace.truncated, 0,
            "macro phase evidence must be complete"
        );
        let invocations: Vec<_> = index
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Macro))
            .collect();
        assert_eq!(
            invocations.len(),
            1,
            "selected source has exactly one check invocation"
        );
        let invocation = invocations[0];
        // Runtime intrinsic recipes are prepared by the coordinator and
        // installed directly at dispatch; no SyntaxProvider Thunk query is
        // required for those PCs. Inspect the actual run's immutable installed
        // recipes in their append-only installation order instead.
        let actual: Vec<_> = run
            .installed_recipes()
            .recipes()
            .filter_map(|verified| {
                let pc = &verified.recipe().key;
                (pc.body == chosen && pc.node == invocation.id.0).then_some(pc.clone())
            })
            .collect();
        let expected: Vec<_> = [Phase::Entry, Phase::Operand(0), Phase::Exit]
            .into_iter()
            .map(|phase| index.pc(invocation.id, phase).unwrap())
            .collect();
        assert_eq!(
            actual, expected,
            "selected intrinsic installation phases differ"
        );
        let failure = index.pc(invocation.id, Phase::Commit).unwrap();
        assert!(
            run.installed_recipes()
                .recipes()
                .all(|verified| verified.recipe().key != failure),
            "unused failure/message recipe became installed"
        );
        assert!(
            w.db.trace
                .events
                .iter()
                .all(|event| !matches!(&event.key, QueryKey::Thunk(pc) if pc == &failure)),
            "unused formatted message became demanded"
        );
    }
}
#[test]
fn test_catalog_type_shadow_cannot_be_erased_by_unrelated_supplied_graph() {
    let mut w = world(
        "use std::assert as check; type bool=u8; fn predicate()->bool{1u8} #[test]fn chosen(){}",
        2024,
    );
    let callee = predicate(&mut w);
    let wrong = NameNamespaces::default();
    let error = NormalizedDeferredReturn::mint_primitive(
        &mut w.db,
        &callee,
        &wrong,
        &[&w.source],
        PrimitiveType::Bool,
    )
    .unwrap_err();
    assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
}
#[test]
fn actual_test_catalog_epoch_change_refuses_retained_primitive_return_certificate() {
    let mut w = world(SOURCE, 2024);
    let callee = predicate(&mut w);
    let packet = packet(&mut w, &callee);
    w.db.begin_expansion_epoch(2).unwrap();
    assert!(GenericEnv::capture_current(&mut w.db, &callee, &packet).is_err());
}
