//! Unchanged stock main/foo bodies and source-identity admission controls.
//! This exercises the program; full compiletest/native oracle remains separate.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database, poll::QueryPoll},
    engine::run::{RunMachine, RunPoll, ValueSummary},
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        dependencies::SourceDependency,
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};

const ORIGINAL: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/generics/generic-fn-twice.rs");

#[test]
fn original_repeated_explicit_generic_program_runs_in_its_recorded_dependency_edition() {
    for edition in [2015, 2018, 2021, 2024] {
        run_original(edition, true, 2024);
    }
    run_original(2024, true, 2015);
}

#[test]
fn original_repeated_explicit_generic_program_runs_as_each_official_edition_root() {
    for edition in [2015, 2018, 2021, 2024] {
        run_original(edition, false, edition);
    }
}

fn run_original(edition: u16, dependency: bool, root_edition: u16) {
    let consumer = Digest::of_bytes(b"LRR repeated explicit generic runtime consumer");
    let provider = Digest::of_bytes(b"LRR repeated explicit generic runtime provider");
    let cfg = CfgSet::guest_default();
    let mut units = Vec::new();
    let mut selections = Vec::new();
    let sources = if dependency {
        vec![
            (
                "src/consumer.rs",
                consumer,
                "extern crate original; fn main(){ original::main(); }",
                root_edition,
            ),
            ("src/original.rs", provider, ORIGINAL, edition),
        ]
    } else {
        vec![("src/original.rs", provider, ORIGINAL, edition)]
    };
    for (path, owner, text, edition) in sources {
        let path = SourcePath::new(path).unwrap();
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        selections.push(SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: owner,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        });
        units.push(SourceUnitInput {
            path,
            crate_key: owner,
            module_path: vec![],
            profile,
            cfg: cfg.clone(),
            blob,
        });
    }
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        units,
        Default::default(),
        if dependency {
            vec![SourceDependency {
                consumer,
                alias: "original".into(),
                provider,
            }]
        } else {
            Vec::new()
        },
    )
    .unwrap();
    let views: Vec<_> = selections
        .iter()
        .map(|selection| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(selection).unwrap() else {
                panic!("recorded stock source world missing");
            };
            view
        })
        .collect();
    let main = views[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let original_bodies: Vec<_> = ["main", "foo"]
        .into_iter()
        .map(|name| {
            views[usize::from(dependency)]
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == name)
                .unwrap()
                .body
                .as_ref()
                .unwrap()
                .key()
        })
        .collect();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    // The dependency-aware runtime route consumes the existing sealed source
    // discovery graph. The simple outline-only starter has no alias authority.
    let scope = views[0].outline().root_scope.clone();
    let mut discovery = SourceDiscovery::new(
        views.iter().map(|view| view.share()).collect(),
        scope,
        Namespace::Value,
        Digest::of_bytes(b"LRR stock generic recorded dependency graph"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR stock generic declarative environment"),
        Digest::of_bytes(b"LRR stock generic compiler facade"),
        std::sync::Arc::new(
            lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &ProfileIdentity {
                    edition: root_edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap(),
        ),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::NeedExpansion { request } => {
                discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                panic!("recorded stock discovery: {diagnostic}; {blocked:?}");
            }
            _ => panic!("stock generic case unexpectedly requires procedural/derive authority"),
        }
    }
    // Use exactly the sealed CLI source/index/namespace/catalog route.
    // These immutable contexts convey no procedural macro execution authority.
    let macros = lrr::engine::standalone_macros::CompletedStandaloneRuntimeMacros::from_discovery(
        &views[0], &discovery,
    )
    .unwrap();
    let contexts = macros.contexts().unwrap();
    db.begin_run().unwrap();
    let mut machine = match RunMachine::start_with_discovery(&mut db, main, 10_000, discovery) {
        QueryPoll::Ready(Ok(machine)) => machine,
        QueryPoll::Ready(Err(error)) => panic!("stock startup: {error}"),
        QueryPoll::NeedInput(input) => panic!("stock startup requested {input:?}"),
    };
    let report = match machine.poll_with_runtime_macros(&mut db, &contexts) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!(
            "unchanged stock generic program: {error}; counters {:?}; last query keys {:?}",
            machine.opcode_counters().unwrap(),
            db.trace
                .events
                .iter()
                .rev()
                .take(8)
                .map(|event| &event.key)
                .collect::<Vec<_>>()
        ),
        RunPoll::Panicked(report) => panic!(
            "original assertion/program panicked: {:?}",
            report.panic.kind
        ),
        RunPoll::NeedInput(input) => panic!("stock program requested {input:?}"),
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.live_edition, root_edition);
    // The entry adapter calls original main; each original explicit foo call enters.
    assert_eq!(
        report.opcode_counters.started_by_opcode[19],
        u64::try_from(ORIGINAL.matches("foomod::foo::<isize>()").count()).unwrap()
            + u64::from(dependency)
    );
    assert_eq!(
        report.opcode_counters.started_by_opcode[26],
        u64::try_from(ORIGINAL.matches("assert_eq!").count()).unwrap()
    );
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
    for body in original_bodies {
        assert!(db.trace.events.iter().any(|event| matches!(
            &event.key,
            lrr::db::key::QueryKey::Thunk(key) if key.body == body
        )));
        let QueryPoll::Ready(Ok(value)) = db.fetch(
            &lrr::db::provider::SyntaxProvider,
            lrr::db::key::QueryKey::BodyStructure(body),
        ) else {
            panic!("executed original body structure unavailable");
        };
        let lrr::db::value::QueryValue::BodyStructure(index) = value.value.as_ref() else {
            panic!("original body query returned another record");
        };
        assert_eq!(index.profile().edition, edition);
    }
    db.finish_run().unwrap();
}

const TEMPORARY: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/generics/generic-temporary.rs");

fn original_temporary_world(
    edition: u16,
) -> (
    PreparedSourceInputs,
    lrr::source::changes::PreparedSourceView,
) {
    let blob = SourceBlob::new(TEMPORARY.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("src/original.rs").unwrap();
    let crate_key = Digest::of_bytes(b"LRR original temporary generic world");
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("original generic temporary source missing")
    };
    (inputs, view)
}

#[test]
fn original_concrete_contracts_reject_rigid_identity_at_the_same_runtime_descriptor() {
    use lrr::{
        frontend::{
            call_contracts::{ArgumentType, ContractPreparation, prepare_contracts},
            callables::{CallResolver, CallSelection},
            names::NameNamespaces,
        },
        ir::op::PrimitiveType,
        runtime::shape::TypeId,
        traits::term::{ParamKey, TyTerm},
    };
    for edition in [2015, 2018, 2021, 2024] {
        let (inputs, view) = original_temporary_world(edition);
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let sources = [&view];
        let resolver = CallResolver::new(&graph, &sources).unwrap();
        let definition = |name| {
            view.outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == name)
                .unwrap()
                .def_key
                .fingerprint()
        };
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        let QueryPoll::Ready(Ok(CallSelection::Source(chk))) =
            resolver.module_function(&mut db, definition("main"), "chk")
        else {
            panic!("original chk signature missing")
        };
        let QueryPoll::Ready(Ok(CallSelection::Source(mk))) =
            resolver.module_function(&mut db, definition("main"), "mk")
        else {
            panic!("original mk signature missing")
        };
        let scalar = ArgumentType {
            source: TyTerm::Primitive(PrimitiveType::Isize),
            descriptor: TypeId(8),
        };
        let rigid = ArgumentType {
            source: TyTerm::Param(ParamKey {
                definition: definition("apply"),
                ordinal: 0,
            }),
            descriptor: scalar.descriptor,
        };
        // Preserve the unchanged declarations and bodies. Only these LRR
        // packets are corrupted; this does not execute the stock fn pointers.
        let QueryPoll::Ready(Ok(ContractPreparation::Ready(_))) = prepare_contracts(
            &mut db,
            &chk,
            &graph,
            &sources,
            &[],
            None,
            std::slice::from_ref(&scalar),
        ) else {
            panic!("original scalar chk contract refused")
        };
        match prepare_contracts(
            &mut db,
            &chk,
            &graph,
            &sources,
            &[],
            None,
            std::slice::from_ref(&rigid),
        ) {
            QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-GENERIC-BINDING"),
            _ => panic!("rigid argument identity accepted as concrete isize at equal descriptor"),
        }
        let unit = ArgumentType {
            source: TyTerm::Primitive(PrimitiveType::Unit),
            descriptor: TypeId(1),
        };
        let rigid_unit = ArgumentType {
            source: rigid.source.clone(),
            descriptor: unit.descriptor,
        };
        let QueryPoll::Ready(Ok(ContractPreparation::Ready(mut contract))) = prepare_contracts(
            &mut db,
            &chk,
            &graph,
            &sources,
            &[],
            Some(&unit),
            std::slice::from_ref(&scalar),
        ) else {
            panic!("original implicit Unit result contract refused")
        };
        assert_eq!(contract.source_result, Some(unit.source.clone()));
        assert_eq!(contract.result_descriptor, Some(unit.descriptor));
        assert_eq!(
            contract
                .bind_return_parameter(&rigid_unit)
                .unwrap_err()
                .code,
            "LRR-E-GENERIC-BINDING"
        );
        contract.bind_return_parameter(&unit).unwrap();
        match prepare_contracts(
            &mut db,
            &chk,
            &graph,
            &sources,
            &[],
            Some(&rigid_unit),
            std::slice::from_ref(&scalar),
        ) {
            QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-GENERIC-BINDING"),
            _ => panic!("rigid result context accepted as concrete Unit at equal descriptor"),
        }
        // A written named return remains deferred until source-name resolution.
        // Its typed refusal is not a pass for the original fn-pointer program.
        let QueryPoll::Ready(Ok(ContractPreparation::Ready(mut deferred))) =
            prepare_contracts(&mut db, &mk, &graph, &sources, &[], Some(&scalar), &[])
        else {
            panic!("original deferred mk result contract unavailable")
        };
        assert_eq!(deferred.source_result, None);
        assert_eq!(deferred.result_descriptor, None);
        assert_eq!(
            deferred.bind_return_parameter(&scalar).unwrap_err().code,
            "LRR-E-TYPE-CONTEXT"
        );
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            lrr::db::key::QueryKey::BodyStructure(_)
                | lrr::db::key::QueryKey::Thunk(_)
                | lrr::db::key::QueryKey::Constant(_)
        )));
    }
}
