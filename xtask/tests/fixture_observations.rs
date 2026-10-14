use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database, poll::QueryPoll},
    engine::{
        fixture::{FixtureManifest, FixtureMode},
        fixture_observer::FixturePreparationObserver,
        run::{RunMachine, RunPoll, ValueSummary},
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn prepare(text: &str) -> (PreparedSourceInputs, PreparedSourceView) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([183; 32]);
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
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    (inputs, view)
}
fn contract(marker: &str) -> FixtureManifest {
    FixtureManifest::parse(&format!(
        r#"schema_version=1
id="T034-OBSERVATION"
category="relaxed"
source="main.rs"
entry="case"
modes=["off"]
oracle=false
[expect]
status="passed"
stdout=""
stderr=""
[laziness]
no_body_preparation=["broken"]
no_marker_preparation=["{marker}"]
"#
    ))
    .unwrap()
}
fn start(db: &mut Database, view: &PreparedSourceView) -> RunMachine {
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "case")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(run)) =
        RunMachine::start_with_sources(db, body, 10_000, vec![view.share()])
    else {
        panic!("run did not start")
    };
    run
}
#[test]
fn actual_disk_guest_run_measures_cold_preparation_but_keeps_unmeasured_channels_absent() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../compat/fixtures/T034-FORMAT");
    let text = std::fs::read_to_string(directory.join("main.rs")).unwrap();
    let contract =
        FixtureManifest::parse(&std::fs::read_to_string(directory.join("fixture.toml")).unwrap())
            .unwrap();
    let (inputs, view) = prepare(&text);
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let markers = view.fixture_markers().unwrap();
    let before = db.trace.events.len();
    let observer =
        FixturePreparationObserver::start(&db, &[view.share()], &view, &markers, &contract)
            .unwrap();
    assert_eq!(db.trace.events.len(), before);
    let mut run = start(&mut db, &view);
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("guest execution did not complete")
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    let events = db.trace.events.len();
    let fuel = run.fuel_snapshot();
    let observed = observer.collect(&db, &run).unwrap();
    assert_eq!(db.trace.events.len(), events);
    assert_eq!(run.fuel_snapshot(), fuel);
    assert_eq!(observed.body_preparations["broken"], 0);
    let marker = &observed.markers["cold_branch"];
    assert_eq!(marker.thunk_executions, Some(0));
    assert_eq!(marker.installed_operations, Some(0));
    assert_eq!(marker.native_operations, None);
    assert!(observed.stdout.is_none());
    assert!(observed.stderr.is_none());
    assert_eq!(
        observed.outcome,
        Some(lrr::engine::fixture::FixtureOutcome::Completed)
    );
    assert!(observed.native_entries.is_none());
    assert!(contract.check(FixtureMode::Off, &observed).is_err());
    db.finish_run().unwrap();
}
#[test]
fn reached_fixture_marker_collects_actual_thunk_and_installed_origins_on_guest_fault() {
    let (inputs, view) = prepare(
        "fn broken(){absent();} #[test]fn case(){if false {} else {/*lrr-marker:cold*/unknown();}}",
    );
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let markers = view.fixture_markers().unwrap();
    let observer =
        FixturePreparationObserver::start(&db, &[view.share()], &view, &markers, &contract("cold"))
            .unwrap();
    let mut run = start(&mut db, &view);
    assert!(matches!(run.poll(&mut db), RunPoll::Fault(_)));
    let observed = observer.collect(&db, &run).unwrap();
    assert_eq!(observed.body_preparations["broken"], 0);
    assert!(observed.markers["cold"].thunk_executions.unwrap() > 0);
    assert!(observed.markers["cold"].installed_operations.unwrap() > 0);
    assert!(observed.markers["cold"].native_operations.is_none());
    db.finish_run().unwrap();
}
#[test]
fn warm_fixture_marker_still_records_installed_operations_when_provider_work_is_reused() {
    let (inputs, view) =
        prepare("fn broken(){absent();} #[test]fn case(){/*lrr-marker:hot*/7i64;}");
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let markers = view.fixture_markers().unwrap();
    for warm in [false, true] {
        let observer = FixturePreparationObserver::start(
            &db,
            &[view.share()],
            &view,
            &markers,
            &contract("hot"),
        )
        .unwrap();
        let mut run = start(&mut db, &view);
        assert!(matches!(run.poll(&mut db), RunPoll::Completed(_)));
        let observed = observer.collect(&db, &run).unwrap();
        assert!(observed.markers["hot"].installed_operations.unwrap() > 0);
        if warm {
            assert_eq!(observed.markers["hot"].thunk_executions, Some(0));
        } else {
            assert!(observed.markers["hot"].thunk_executions.unwrap() > 0);
        }
        assert_eq!(observed.body_preparations["broken"], 0);
        db.finish_run().unwrap();
    }
}
#[test]
fn fixture_observer_refuses_missing_pending_or_incomplete_source_and_trace_evidence() {
    let (inputs, view) = prepare(
        "fn broken(){absent();} #[test]fn case(){/*lrr-marker:cold*/1i64;assert!(/*lrr-marker:pending*/true);}",
    );
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let markers = view.fixture_markers().unwrap();
    assert!(
        FixturePreparationObserver::start(&db, &[view.share()], &view, &markers, &contract("cold"))
            .is_err()
    );
    db.trace.detailed = true;
    assert!(
        FixturePreparationObserver::start(
            &db,
            &[view.share()],
            &view,
            &markers,
            &contract("missing")
        )
        .is_err()
    );
    assert!(
        FixturePreparationObserver::start(
            &db,
            &[view.share()],
            &view,
            &markers,
            &contract("pending")
        )
        .is_err()
    );
    assert!(
        FixturePreparationObserver::start(&db, &[], &view, &markers, &contract("cold")).is_err()
    );
    let observer =
        FixturePreparationObserver::start(&db, &[view.share()], &view, &markers, &contract("cold"))
            .unwrap();
    let mut run = start(&mut db, &view);
    assert!(matches!(run.poll(&mut db), RunPoll::Fault(_)));
    db.trace.truncated = 1;
    assert!(observer.collect(&db, &run).is_err());
    db.trace.truncated = 0;
    db.finish_run().unwrap();
    inputs.publish(&mut db).unwrap();
    assert!(observer.collect(&db, &run).is_err());
}

#[test]
fn fixture_terminal_collection_preserves_actual_completion_fault_and_unmeasured_channels() {
    use lrr::engine::fixture::FixtureOutcome;
    for (text, outcome, status, code, cleaned) in [
        (
            "fn broken(){absent();} fn case(){/*lrr-marker:cold*/true;}",
            FixtureOutcome::Completed,
            "passed",
            None,
            true,
        ),
        (
            "fn broken(){absent();} fn case(){/*lrr-marker:cold*/absent();}",
            FixtureOutcome::Fault,
            "fault",
            Some("LRR-E-NAME-MISSING"),
            false,
        ),
    ] {
        let (inputs, view) = prepare(text);
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        let observer = FixturePreparationObserver::start(
            &db,
            &[view.share()],
            &view,
            &view.fixture_markers().unwrap(),
            &contract("cold"),
        )
        .unwrap();
        let mut run = start(&mut db, &view);
        assert!(observer.collect(&db, &run).unwrap().outcome.is_none());
        let poll = run.poll(&mut db);
        assert!(matches!(poll, RunPoll::Completed(_) | RunPoll::Fault(_)));
        let fuel = run.fuel_snapshot();
        let trace = db.trace.events.len();
        let observed = observer.collect(&db, &run).unwrap();
        assert_eq!(observed.outcome, Some(outcome));
        assert_eq!(observed.status.as_deref(), Some(status));
        assert_eq!(observed.diagnostic_code.as_deref(), code);
        assert_eq!(observed.cleanup_ran, Some(cleaned));
        assert!(observed.stdout.is_none() && observed.stderr.is_none() && observed.mode.is_none());
        assert!(
            observed.phase.is_none()
                && observed.drop_log.is_none()
                && observed.native_entries.is_none()
        );
        assert_eq!(run.fuel_snapshot(), fuel);
        assert_eq!(db.trace.events.len(), trace);
        db.finish_run().unwrap();
    }
}
#[test]
fn fixture_terminal_guest_panic_is_retained_after_repoll_and_cannot_be_replaced_by_external_report()
{
    use lrr::engine::{fixture::FixtureOutcome, standalone_macros::StandaloneRuntimeMacros};
    let (inputs, view) = prepare(
        "fn broken(){absent();} fn case(){/*lrr-marker:cold*/1i64;panic!(\"observed panic\");}",
    );
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let observer = FixturePreparationObserver::start(
        &db,
        &[view.share()],
        &view,
        &view.fixture_markers().unwrap(),
        &contract("cold"),
    )
    .unwrap();
    let macros = StandaloneRuntimeMacros::new(&view).unwrap();
    let mut run = start(&mut db, &view);
    let RunPoll::Panicked(mut returned) =
        run.poll_with_runtime_macros(&mut db, &[macros.context(&view).unwrap()])
    else {
        panic!("panic absent")
    };
    returned.panic.message = Some("fabricated external message".into());
    assert!(matches!(run.poll(&mut db), RunPoll::Fault(_)));
    let observed = observer.collect(&db, &run).unwrap();
    assert_eq!(observed.outcome, Some(FixtureOutcome::GuestPanic));
    assert_eq!(observed.status.as_deref(), Some("panicked"));
    assert_eq!(observed.panic_message.as_deref(), Some("observed panic"));
    assert!(observed.diagnostic_code.is_none());
    assert_eq!(observed.cleanup_ran, Some(true));
    db.finish_run().unwrap();
}
#[test]
fn fixture_terminal_collection_refuses_another_entry_on_the_same_source_publication() {
    let (inputs, view) =
        prepare("fn broken(){absent();} fn case(){/*lrr-marker:cold*/true;}fn other(){}");
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let observer = FixturePreparationObserver::start(
        &db,
        &[view.share()],
        &view,
        &view.fixture_markers().unwrap(),
        &contract("cold"),
    )
    .unwrap();
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "other")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body, 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    assert!(matches!(run.poll(&mut db), RunPoll::Completed(_)));
    assert_eq!(
        observer.collect(&db, &run).unwrap_err().code,
        "LRR-E-FIXTURE-OBSERVATION"
    );
    db.finish_run().unwrap();
}

#[test]
fn fixture_terminal_collection_cannot_adopt_a_run_started_before_the_observer() {
    let (inputs, view) = prepare("fn broken(){absent();}fn case(){/*lrr-marker:cold*/1i64;}");
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let mut run = start(&mut db, &view);
    assert!(matches!(run.poll(&mut db), RunPoll::Completed(_)));
    let observer = FixturePreparationObserver::start(
        &db,
        &[view.share()],
        &view,
        &view.fixture_markers().unwrap(),
        &contract("cold"),
    )
    .unwrap();
    assert_eq!(
        observer.collect(&db, &run).unwrap_err().code,
        "LRR-E-FIXTURE-OBSERVATION"
    );
    db.finish_run().unwrap();
}
