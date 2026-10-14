//! Draft component witnesses, not full upstream compiletest acceptance.
//! Whole unchanged main execution cannot satisfy cold UI diagnostics or MIR dumps.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
    },
    engine::run::{RunMachine, RunPoll, ValueSummary},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MissingContract {
    ColdCompilerDiagnostic,
    CompilerMirGraphviz,
}

fn component_unchanged_main(source: &str, path: &str, contract: MissingContract) {
    // One explicitly labelled LRR component edition, not a frozen upstream
    // configuration identity or an all-edition/full-suite denominator expansion.
    let profile = ProfileIdentity {
        edition: 2015,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let owner = Digest::of_bytes(path.as_bytes());
    let source_path = SourcePath::new(path).unwrap();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let selection = SourceSelection {
        path: source_path.clone(),
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
            path: source_path,
            crate_key: owner,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("unchanged source selection unavailable: {path}");
    };
    let main = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .and_then(|item| item.body.as_ref())
        .unwrap()
        .key();
    let cold_bodies: Vec<_> = view
        .outline()
        .items
        .iter()
        .filter_map(|item| item.body.as_ref().map(|body| body.key()))
        .filter(|body| body != &main)
        .collect();
    if contract == MissingContract::ColdCompilerDiagnostic {
        assert!(
            !cold_bodies.is_empty(),
            "diagnostic witness lost its cold body"
        );
    }
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let mut machine = match RunMachine::start_with_sources(&mut db, main, 4000, vec![view.share()])
    {
        QueryPoll::Ready(Ok(machine)) => machine,
        QueryPoll::Ready(Err(error)) => {
            panic!("{path}: startup fault is not contract acceptance: {error}")
        }
        QueryPoll::NeedInput(input) => panic!("{path}: startup requested input: {input:?}"),
    };
    let report = match machine.poll(&mut db) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => {
            panic!("{path}: generic/reached fault is not contract acceptance: {error}")
        }
        RunPoll::Panicked(report) => panic!(
            "{path}: main panic is not contract acceptance: {:?}",
            report.panic.kind
        ),
        RunPoll::NeedInput(input) => panic!("{path}: main requested input: {input:?}"),
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.live_edition, 2015);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
    assert_eq!(machine.action_snapshot().published, 0);
    for event in &db.trace.events {
        match &event.key {
            QueryKey::BodyStructure(body) => assert!(
                !cold_bodies.contains(body),
                "cold body structure demanded: {path}"
            ),
            QueryKey::Thunk(pc) => assert!(
                !cold_bodies.contains(&pc.body),
                "cold executable thunk demanded: {path}"
            ),
            _ => {}
        }
    }
    db.finish_run().unwrap();
    eprintln!(
        "CA037 component only: path={path}; unchanged_main=completed; missing_contract={contract:?}; full_fixture_pass=false; owning_configuration=unrecorded; required_owner=T034/M047"
    );
}

#[test]
fn component_cold_return_type_error_does_not_supply_upstream_e0308_contract() {
    component_unchanged_main(
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/fn/fn-bad-block-type.rs"),
        "tests/ui/fn/fn-bad-block-type.rs",
        MissingContract::ColdCompilerDiagnostic,
    );
}

#[test]
fn component_cold_local_reference_error_does_not_supply_upstream_e0515_contract() {
    component_unchanged_main(
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/issue-46471.rs"),
        "tests/ui/borrowck/issue-46471.rs",
        MissingContract::ColdCompilerDiagnostic,
    );
}

#[test]
fn component_cold_parameter_reference_error_does_not_supply_upstream_e0515_contract() {
    component_unchanged_main(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-local-borrow-outlives-fn.rs"
        ),
        "tests/ui/borrowck/borrowck-local-borrow-outlives-fn.rs",
        MissingContract::ColdCompilerDiagnostic,
    );
}

#[test]
fn component_empty_main_does_not_supply_compiler_private_mir_graphviz_contract() {
    component_unchanged_main(
        include_str!("../../fixtures/upstream/rustc-checkout/tests/mir-opt/graphviz.rs"),
        "tests/mir-opt/graphviz.rs",
        MissingContract::CompilerMirGraphviz,
    );
}
