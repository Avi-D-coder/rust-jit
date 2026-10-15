//! Executes two unchanged official numeric-source mains and all original assertions.
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
    ir::pc::Phase,
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        parse::ParseKey,
    },
};
use std::sync::Arc;

struct OriginalCase {
    source: &'static str,
    path: &'static str,
    assertions: u64,
    numeric_binary: u64,
    borrows: u64,
    assignments: u64,
}

const FLOATS: OriginalCase = OriginalCase {
    source: include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/float2.rs"
    ),
    path: "tests/ui/numbers-arithmetic/float2.rs",
    assertions: 9,
    numeric_binary: 9,
    borrows: 0,
    assignments: 0,
};
const COMPOUND: OriginalCase = OriginalCase {
    source: include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/binop/compound-assign-by-ref.rs"
    ),
    path: "tests/ui/binop/compound-assign-by-ref.rs",
    assertions: 10,
    // One real compound operation and one assertion comparison per block.
    numeric_binary: 20,
    borrows: 10,
    assignments: 10,
};

#[test]
fn unchanged_float2_main_edition_2015() {
    run_original(2015, &FLOATS);
}

#[test]
fn unchanged_float2_main_edition_2018() {
    run_original(2018, &FLOATS);
}

#[test]
fn unchanged_float2_main_edition_2021() {
    run_original(2021, &FLOATS);
}

#[test]
fn unchanged_float2_main_edition_2024() {
    run_original(2024, &FLOATS);
}

#[test]
fn unchanged_compound_assign_by_ref_main_edition_2015() {
    run_original(2015, &COMPOUND);
}

#[test]
fn unchanged_compound_assign_by_ref_main_edition_2018() {
    run_original(2018, &COMPOUND);
}

#[test]
fn unchanged_compound_assign_by_ref_main_edition_2021() {
    run_original(2021, &COMPOUND);
}

#[test]
fn unchanged_compound_assign_by_ref_main_edition_2024() {
    run_original(2024, &COMPOUND);
}

fn run_original(edition: u16, case: &OriginalCase) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let owner = Digest::of_bytes(case.path.as_bytes());
    let path = SourcePath::new(case.path).unwrap();
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(case.source.as_bytes().to_vec()).unwrap();
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
    assert_eq!(
        view.outline().items.len(),
        1,
        "original must retain its sole main"
    );

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
        Digest::of_bytes(b"LRR unchanged stock numeric-source discovery world"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR unchanged stock numeric-source declarative environment"),
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
    let started = RunMachine::start_with_discovery(&mut db, main.clone(), 1_000_000, discovery);
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
    let counts = &report.opcode_counters.started_by_opcode;
    assert_eq!(
        counts[26], case.assertions,
        "all original assertions must execute"
    );
    assert_eq!(
        counts[10], case.numeric_binary,
        "all original numeric effects must execute"
    );
    assert_eq!(
        counts[31], case.borrows,
        "all original RHS reference borrows must execute"
    );
    assert_eq!(
        counts[16], case.assignments,
        "all original compound commits must execute"
    );
    for (opcode, name) in [
        (19, "CallSeal"),
        (22, "CallPrepare"),
        (23, "CallAdvance"),
        (25, "Panic"),
    ] {
        assert_eq!(counts[opcode], 0, "unexpected {name}");
    }
    assert_eq!(report.opcode_counters.backedges_completed, 0);
    assert!(report.opcode_counters.operations_completed > case.numeric_binary);

    let QueryPoll::Ready(Ok(value)) =
        db.fetch(&SyntaxProvider, QueryKey::BodyStructure(main.clone()))
    else {
        panic!("edition {edition}: entered original main index unavailable");
    };
    let QueryValue::BodyStructure(index) = value.value.as_ref() else {
        panic!("edition {edition}: entered original main returned another query record");
    };
    assert_eq!(index.body(), &main);
    assert_eq!(index.profile().edition, edition);
    assert_eq!(
        db.trace.truncated, 0,
        "ordered source demand evidence must be complete"
    );
    let assertion_nodes: Vec<_> = index
        .nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Macro))
        .collect();
    assert_eq!(assertion_nodes.len() as u64, case.assertions);
    // These originals have no loops/helpers: indexed macro order is source
    // order. A later assertion cannot hide a missing earlier success phase.
    // Intrinsic macro recipes are installed directly by the coordinator;
    // their preparation does not require a SyntaxProvider Thunk query. Inspect
    // this actual run's append-only installed table, retaining source order.
    let installed: Vec<_> = machine.installed_recipes().recipes().collect();
    let mut previous_exit = None;
    for node in assertion_nodes {
        let entry = index.pc(node.id, Phase::Entry).unwrap();
        let exit = index.pc(node.id, Phase::Exit).unwrap();
        let reached = |pc: &lrr::ir::pc::PcKey| {
            installed
                .iter()
                .position(|verified| &verified.recipe().key == pc)
                .unwrap_or_else(|| {
                    panic!("edition {edition}: original assertion phase not reached: {pc:?}")
                })
        };
        let entry_at = reached(&entry);
        let exit_at = reached(&exit);
        assert!(entry_at < exit_at, "assertion must evaluate before success");
        let failure = index.pc(node.id, Phase::Commit).unwrap();
        assert!(
            installed
                .iter()
                .all(|verified| verified.recipe().key != failure),
            "an original assertion entered its failure path"
        );
        if let Some(previous) = previous_exit {
            assert!(previous < entry_at, "original assertion order changed");
        }
        previous_exit = Some(exit_at);
    }
    assert!(
        db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) => body == &main,
            QueryKey::Thunk(key) => key.body == main,
            _ => true,
        }),
        "non-original executable body substituted for original main"
    );
}
