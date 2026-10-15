//! Executes the unchanged official labelled-block main and its assertions.
//! Each owning edition has an independent result. This is runtime-contract
//! acceptance; a rustc/compiletest or complete-suite result remains separate.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    engine::{
        run::{RunMachine, RunPoll, ValueSummary},
        standalone_macros::CompletedStandaloneRuntimeMacros,
    },
    expand::{
        source_declarative,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;

const ORIGINAL: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/for-loop-while/label_break_value.rs"
);

#[test]
fn unchanged_label_break_value_main_edition_2015() {
    run_original(2015);
}

#[test]
fn unchanged_label_break_value_main_edition_2018() {
    run_original(2018);
}

#[test]
fn unchanged_label_break_value_main_edition_2021() {
    run_original(2021);
}

#[test]
fn unchanged_label_break_value_main_edition_2024() {
    run_original(2024);
}

fn run_original(edition: u16) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let owner = Digest::of_bytes(b"LRR unchanged stock label_break_value owning crate");
    let path = SourcePath::new("tests/ui/for-loop-while/label_break_value.rs").unwrap();
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
    let selection = SourceSelection {
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
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: owner,
            module_path: vec![],
            profile: profile.clone(),
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("edition {edition}: unchanged original source selection unavailable");
    };
    let body_for = |name: &str| {
        view.outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == name)
            .unwrap_or_else(|| panic!("edition {edition}: original header {name} missing"))
            .body
            .as_ref()
            .unwrap()
            .key()
    };
    let main = body_for("main");
    let reached_bodies: Vec<_> = [
        "main",
        "label_break",
        "break_value",
        "label_break_mixed",
        "label_break_match",
        "label_break_macro",
    ]
    .into_iter()
    .map(|name| (name, body_for(name)))
    .collect();
    let cold_body = body_for("label_break_nested");

    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    // Publish the actual epoch before beginning a run; no active-run input or
    // source replacement is used to make discovery appear complete.
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Value,
        Digest::of_bytes(b"LRR unchanged stock labelled-block discovery world"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR unchanged stock labelled-block declarative environment"),
        source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&profile).unwrap()),
    )
    .unwrap();
    // Retain both original root attributes and the real source macro producer.
    // No synthetic inert proof, completed gate, or procedural authority is used.
    loop {
        match discovery.poll().unwrap_or_else(|error| {
            panic!("edition {edition}: unchanged original discovery fault: {error}")
        }) {
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::NeedExpansion { request } => {
                let expansion = request.expand().unwrap_or_else(|error| {
                    panic!("edition {edition}: original declarative expansion: {error}")
                });
                discovery.admit_expansion(Arc::new(expansion)).unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                let expansion = request.expand().unwrap_or_else(|error| {
                    panic!("edition {edition}: generated declarative expansion: {error}")
                });
                discovery.admit_expansion(Arc::new(expansion)).unwrap();
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!(
                "edition {edition}: original discovery incomplete: {diagnostic}; blocked {blocked:?}"
            ),
            _ => panic!(
                "edition {edition}: original discovery requires additional attribute/derive/procedural preparation"
            ),
        }
    }
    let macros = CompletedStandaloneRuntimeMacros::from_discovery(&view, &discovery)
        .unwrap_or_else(|error| panic!("edition {edition}: sealed runtime preparation: {error}"));
    let contexts = macros.contexts().unwrap();
    db.begin_run().unwrap();
    let started = RunMachine::start_with_discovery(&mut db, main, 1_000_000, discovery);
    let mut machine = match started {
        QueryPoll::Ready(Ok(machine)) => machine,
        QueryPoll::Ready(Err(error)) => {
            db.finish_run().unwrap();
            panic!("edition {edition}: unchanged original main startup: {error}");
        }
        QueryPoll::NeedInput(input) => {
            db.finish_run().unwrap();
            panic!("edition {edition}: original startup requires {input:?}");
        }
    };
    let outcome = machine.poll_with_runtime_macros(&mut db, &contexts);
    // Every non-completion remains a failure; preserve the exact natural guest
    // frontier, stack, counters and queries for the next interpreter repair.
    db.finish_run().unwrap();
    let report = match outcome {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!(
            "edition {edition}: unchanged original main fault: {error}; stack {:?}; counters {:?}; last queries {:?}",
            error.guest_stack,
            machine.opcode_counters().unwrap(),
            db.trace
                .events
                .iter()
                .rev()
                .take(12)
                .map(|event| &event.key)
                .collect::<Vec<_>>()
        ),
        RunPoll::Panicked(report) => {
            panic!("edition {edition}: unchanged original main panicked: {report:?}")
        }
        RunPoll::NeedInput(input) => panic!(
            "edition {edition}: unchanged original main requires {input:?}; counters {:?}",
            machine.opcode_counters().unwrap()
        ),
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.live_edition, edition);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
    assert!(report.opcode_counters.backedges_completed > 0);
    // Original dynamic paths: main 13 equality assertions, mixed-loop calls 7,
    // match calls 12, macro function 2, plus the reached mixed-loop predicate 1.
    // Original assertions themselves execute; this count is a coverage guard.
    assert_eq!(report.opcode_counters.started_by_opcode[26], 35);
    for (name, body) in reached_bodies {
        assert!(
            db.trace.events.iter().any(|event| matches!(
                &event.key,
                QueryKey::Thunk(key) if key.body == body
            )),
            "edition {edition}: original {name} was not entered"
        );
        let QueryPoll::Ready(Ok(value)) = db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body))
        else {
            panic!("edition {edition}: entered original {name} body unavailable");
        };
        let QueryValue::BodyStructure(index) = value.value.as_ref() else {
            panic!("edition {edition}: entered original {name} returned another query record");
        };
        assert_eq!(index.profile().edition, edition);
    }
    assert!(
        db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) => body != &cold_body,
            QueryKey::Thunk(key) => key.body != cold_body,
            _ => true,
        }),
        "edition {edition}: uncalled original printing body was semantically entered"
    );
}
