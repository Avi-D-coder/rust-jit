//! Unchanged stock named primitive return programs in paired owning editions.
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

const ORIGINALS: [&str; 2] = [
    include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/isize-base.rs"
    ),
    include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/usize-base.rs"
    ),
];

#[test]
fn original_named_primitive_return_program_runs_in_its_recorded_dependency_edition() {
    for original in ORIGINALS {
        for edition in [2015, 2018, 2021, 2024] {
            run_original(original, edition, true, 2024);
        }
        run_original(original, 2024, true, 2015);
    }
}

#[test]
fn original_named_primitive_return_program_runs_as_each_official_edition_root() {
    for original in ORIGINALS {
        for edition in [2015, 2018, 2021, 2024] {
            run_original(original, edition, false, edition);
        }
    }
}

fn run_original(original: &str, edition: u16, dependency: bool, root_edition: u16) {
    let consumer = Digest::of_bytes(b"LRR original named return runtime consumer");
    let provider = Digest::of_bytes(b"LRR original named return runtime provider");
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
            ("src/original.rs", provider, original, edition),
        ]
    } else {
        vec![("src/original.rs", provider, original, edition)]
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
    let original_bodies: Vec<_> = ["main", "return_val"]
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
        Digest::of_bytes(b"LRR stock named return recorded dependency graph"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR stock named return declarative environment"),
        Digest::of_bytes(b"LRR stock named return compiler facade"),
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
            _ => {
                panic!("stock named-return case unexpectedly requires procedural/derive authority")
            }
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
            "unchanged stock named-return program: {error}; counters {:?}; last query keys {:?}",
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
    // The entry adapter calls original main; original main calls return_val once.
    assert_eq!(
        report.opcode_counters.started_by_opcode[19],
        if dependency { 2 } else { 1 }
    );
    assert_eq!(
        report.opcode_counters.started_by_opcode[26],
        u64::try_from(original.matches("assert_eq!").count()).unwrap()
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
