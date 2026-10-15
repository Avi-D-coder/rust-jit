//! Executes both unchanged official scalar-range mains in each edition.
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

const INFERENCE: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/binding/match-range-infer.rs");
const RANGES: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/binding/match-range.rs");

struct OriginalCase {
    source: &'static str,
    path: &'static str,
    comparisons: u64,
    // In source order: (upper bound reached, successful range commit reached).
    phases: &'static [(bool, bool)],
    cold_panics: usize,
}

const INFERENCE_CASE: OriginalCase = OriginalCase {
    source: INFERENCE,
    path: "tests/ui/binding/match-range-infer.rs",
    comparisons: 6,
    phases: &[(true, true); 3],
    cold_panics: 3,
};
const RANGE_CASE: OriginalCase = OriginalCase {
    source: RANGES,
    path: "tests/ui/binding/match-range.rs",
    // Twenty-one range comparisons and the fifth match's prior literal Eq.
    comparisons: 22,
    phases: &[
        (true, true),
        (true, true),
        (false, false),
        (true, false),
        (true, true),
        (true, true),
        (true, true),
        (true, true),
        (true, true),
        (true, false),
        (true, true),
    ],
    cold_panics: 12,
};

#[test]
fn unchanged_match_range_infer_main_edition_2015() {
    run_original(2015, &INFERENCE_CASE);
}

#[test]
fn unchanged_match_range_infer_main_edition_2018() {
    run_original(2018, &INFERENCE_CASE);
}

#[test]
fn unchanged_match_range_infer_main_edition_2021() {
    run_original(2021, &INFERENCE_CASE);
}

#[test]
fn unchanged_match_range_infer_main_edition_2024() {
    run_original(2024, &INFERENCE_CASE);
}

#[test]
fn unchanged_match_range_main_edition_2015() {
    run_original(2015, &RANGE_CASE);
}

#[test]
fn unchanged_match_range_main_edition_2018() {
    run_original(2018, &RANGE_CASE);
}

#[test]
fn unchanged_match_range_main_edition_2021() {
    run_original(2021, &RANGE_CASE);
}

#[test]
fn unchanged_match_range_main_edition_2024() {
    run_original(2024, &RANGE_CASE);
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
    // There is exactly one original function and no helper-call obligation.
    assert_eq!(view.outline().items.len(), 1);

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
        Digest::of_bytes(b"LRR unchanged stock scalar-range discovery world"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR unchanged stock scalar-range declarative environment"),
        source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&profile).unwrap()),
    )
    .unwrap();
    // Prepare the original through the real owning compiler/macro producer.
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
    // Whole-main success requires the original branch outcomes, including
    // lower mismatches and excluded upper endpoints. Every panic stays cold.
    assert_eq!(
        counts[10], case.comparisons,
        "edition {edition}: missing/extra bound comparison"
    );
    for (opcode, name) in [
        (19, "CallSeal"),
        (22, "CallPrepare"),
        (23, "CallAdvance"),
        (25, "Panic"),
        (26, "Assert"),
    ] {
        assert_eq!(counts[opcode], 0, "edition {edition}: unexpected {name}");
    }
    assert_eq!(report.opcode_counters.backedges_completed, 0);
    assert!(report.opcode_counters.operations_completed > case.comparisons);

    let QueryPoll::Ready(Ok(value)) =
        db.fetch(&SyntaxProvider, QueryKey::BodyStructure(main.clone()))
    else {
        panic!("edition {edition}: executed original main index unavailable");
    };
    let QueryValue::BodyStructure(index) = value.value.as_ref() else {
        panic!("edition {edition}: main returned another query record");
    };
    assert_eq!(index.body(), &main);
    assert_eq!(index.profile().edition, edition);
    let ranges: Vec<_> = index
        .nodes()
        .iter()
        .filter(|node| node.scalar_range_phase)
        .collect();
    assert_eq!(ranges.len(), case.phases.len());
    for (range, &(upper, committed)) in ranges.into_iter().zip(case.phases) {
        assert_eq!(range.role, NodeRole::Pattern);
        for (phase, reached) in [
            (Phase::Entry, true),
            (Phase::Operand(0), upper),
            (Phase::Commit, committed),
        ] {
            let pc = index.pc(range.id, phase).unwrap();
            assert_eq!(
                db.trace
                    .events
                    .iter()
                    .any(|event| matches!(&event.key, QueryKey::Thunk(key) if key == &pc)),
                reached,
                "edition {edition}: original range {:?} phase {phase:?} has wrong demand state",
                range.id
            );
        }
    }
    let cold_macros: Vec<_> = index
        .nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Macro))
        .collect();
    assert_eq!(cold_macros.len(), case.cold_panics);
    for node in cold_macros {
        assert!(
            db.trace.events.iter().all(|event| !matches!(&event.key,
            QueryKey::Thunk(key) if key.body == main && key.node == node.id.0)),
            "edition {edition}: original panic wildcard macro {:?} was entered",
            node.id
        );
    }
    // No hidden helper is substituted for the sole original main.
    assert!(
        db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) => body == &main,
            QueryKey::Thunk(key) => key.body == main,
            _ => true,
        }),
        "edition {edition}: a non-original executable body was entered"
    );
}
