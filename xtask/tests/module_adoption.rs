//! M013 immutable module adoption, stale/cancelled demand and retained byte budgets.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::{ConstantKey, QueryKey},
        memo::Database,
        poll::{InputRequest, QueryPoll},
        provider::SyntaxProvider,
    },
    service::{
        expansion::{DemandContext, DemandToken, ExpansionCoordinator, PhaseIdentity},
        source_capture::SourceCapture,
    },
    source::{
        include_paths::{IncludeAuthorities, IncludeLocation, IncludeRoot, IncludeRootKind},
        manifest::BodyKey,
        module_capture::{CapturedModule, ModuleCaptureRequest},
        snapshot::SourcePath,
    },
    syntax::{modules::ModuleRequest, token_wire::Key},
};
use std::{collections::BTreeMap, fs, path::PathBuf};
struct Fixture {
    path: PathBuf,
    request: ModuleCaptureRequest,
    input: InputRequest,
    context: DemandContext,
    coordinator: ExpansionCoordinator,
    token: DemandToken,
    capture: SourceCapture,
    record: CapturedModule,
    db: Database,
}
fn fixture(two_files: bool) -> Fixture {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT
        .try_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_add(1),
        )
        .unwrap();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m013-module-adoption-{}-{sequence}",
            std::process::id()
        ));
    fs::create_dir_all(&path).unwrap();
    let path = fs::canonicalize(path).unwrap();
    fs::write(path.join("child.rs"), "pub fn made()->i64{1657_i64}").unwrap();
    if two_files {
        fs::create_dir(path.join("child")).unwrap();
        fs::write(path.join("child/mod.rs"), "pub fn other(){}").unwrap();
    }
    let unit = Digest([1; 32]);
    let generation = Digest([2; 32]);
    let root = Digest([3; 32]);
    let authorities = IncludeAuthorities::new(
        unit,
        generation,
        [IncludeRoot {
            identity: root,
            path: path.clone(),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap();
    let from = IncludeLocation {
        unit,
        root,
        path: SourcePath::new("main.rs").unwrap(),
        origin: Key([4; 32]),
    };
    let request = authorities
        .module_request(
            &from,
            std::path::Path::new(""),
            ModuleRequest {
                module_path: vec!["child".into()],
                directory: vec![],
                file_relative: false,
                name: "child".into(),
                path_override: None,
            },
            Digest([5; 32]),
            Key([6; 32]),
        )
        .unwrap();
    let input = InputRequest::CaptureModule {
        request: Box::new(request.clone()),
        compile_environment: Digest([7; 32]),
    };
    let db = Database::default();
    let context = DemandContext {
        request: Digest([8; 32]),
        session: Digest([9; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: generation,
        native_generation: None,
        compile_environment: Digest([7; 32]),
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let query = QueryKey::Constant(ConstantKey {
        definition: Digest([10; 32]),
        arguments: vec![],
        cfg: Digest::default(),
        profile: ProfileIdentity::current(),
    });
    let phase = PhaseIdentity {
        frame: Digest([11; 32]),
        body: BodyKey {
            definition: Digest([12; 32]),
            exact_body_bytes: Digest([13; 32]),
        },
        index: 0,
    };
    let token = coordinator.issue(query, phase, input.clone()).unwrap();
    coordinator.start(&context, &token).unwrap();
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let record = capture.capture_module(&request).unwrap();
    Fixture {
        path,
        request,
        input,
        context,
        coordinator,
        token,
        capture,
        record,
        db,
    }
}
fn adopt(f: &mut Fixture) {
    f.coordinator
        .accept_module(&f.context, &f.token, &f.record)
        .unwrap();
    f.coordinator
        .adopt_module(&f.context, &f.token, &mut f.db, f.record.clone())
        .unwrap();
}
#[test]
fn module_adoption_rejects_foreign_context_unaccepted_reply_and_cancelled_demand() {
    let mut f = fixture(false);
    let mut foreign = f.context.clone();
    foreign.compile_environment = Digest([14; 32]);
    assert!(
        f.coordinator
            .accept_module(&foreign, &f.token, &f.record)
            .is_err()
    );
    assert!(
        f.coordinator
            .adopt_module(&f.context, &f.token, &mut f.db, f.record.clone())
            .is_err()
    );
    assert!(
        f.db.input(&InputKey::ModuleRecord(f.input.fingerprint()))
            .is_none()
    );
    f.coordinator
        .accept_module(&f.context, &f.token, &f.record)
        .unwrap();
    f.coordinator.cancel();
    assert!(
        f.coordinator
            .adopt_module(&f.context, &f.token, &mut f.db, f.record.clone())
            .is_err()
    );
    assert!(matches!(
        SyntaxProvider::module_capture(&mut f.db, &f.input),
        QueryPoll::NeedInput(_)
    ));
}
#[test]
fn module_adoption_rejects_stale_source_revision_without_publication() {
    let mut f = fixture(false);
    f.coordinator
        .accept_module(&f.context, &f.token, &f.record)
        .unwrap();
    f.db.publish_inputs(BTreeMap::new()).unwrap();
    assert_ne!(f.db.source_revision(), f.context.revision);
    assert!(
        f.coordinator
            .adopt_module(&f.context, &f.token, &mut f.db, f.record.clone())
            .unwrap_err()
            .message
            .contains("source revision")
    );
    assert!(
        f.db.input(&InputKey::ModuleRecord(f.input.fingerprint()))
            .is_none()
    );
}
#[test]
fn module_adoption_accounts_every_candidate_and_retains_leased_bytes_after_replacement() {
    let mut f = fixture(true);
    let bytes = f
        .record
        .observations()
        .iter()
        .filter_map(|o| match o {
            lrr::source::module_capture::ModuleObservation::Present(c) => {
                Some(c.blob().bytes().len())
            }
            _ => None,
        })
        .sum::<usize>();
    adopt(&mut f);
    assert_eq!(f.db.cache_usage().unwrap().source_input_bytes, bytes);
    let QueryPoll::Ready(Ok(lease)) = SyntaxProvider::module_capture(&mut f.db, &f.input) else {
        panic!("adopted input missing");
    };
    assert_eq!(
        lease.record().selected().unwrap_err().code,
        "LRR-E-MODULE-AMBIGUOUS"
    );
    f.db.publish_inputs(BTreeMap::new()).unwrap();
    assert_eq!(f.db.cache_usage().unwrap().source_input_bytes, bytes);
    assert!(matches!(
        SyntaxProvider::module_capture(&mut f.db, &f.input),
        QueryPoll::NeedInput(_)
    ));
    drop(lease);
    assert_eq!(f.db.cache_usage().unwrap().source_input_bytes, 0);
}
#[test]
fn module_adoption_source_budget_refusal_is_atomic_and_does_not_resume_the_phase() {
    let mut f = fixture(true);
    let one = f
        .record
        .observations()
        .iter()
        .find_map(|o| match o {
            lrr::source::module_capture::ModuleObservation::Present(c) => {
                Some(c.blob().bytes().len())
            }
            _ => None,
        })
        .unwrap();
    f.db.begin_run_with_budgets(lrr::engine::telemetry::CacheBudgets {
        source_input_bytes: one,
        ..Default::default()
    })
    .unwrap();
    f.coordinator
        .accept_module(&f.context, &f.token, &f.record)
        .unwrap();
    assert_eq!(
        f.coordinator
            .adopt_module(&f.context, &f.token, &mut f.db, f.record.clone())
            .unwrap_err()
            .code,
        "LRR-E-METADATA-LIMIT"
    );
    assert!(
        f.coordinator
            .take_ready(&f.context, &f.token)
            .unwrap()
            .is_none()
    );
    assert_eq!(f.db.cache_usage().unwrap().source_input_bytes, 0);
    f.db.finish_run().unwrap();
    f.coordinator
        .adopt_module(&f.context, &f.token, &mut f.db, f.record.clone())
        .unwrap();
}
#[test]
fn module_adoption_immutable_conflict_cannot_replace_first_captured_bytes() {
    let mut f = fixture(false);
    adopt(&mut f);
    fs::write(f.path.join("child.rs"), "pub fn made()->i64{1663_i64}").unwrap();
    let changed = f.capture.capture_module(&f.request).unwrap();
    assert_ne!(changed.identity(), f.record.identity());
    let mut other = ExpansionCoordinator::new(f.context.clone());
    let query = QueryKey::Constant(ConstantKey {
        definition: Digest([17; 32]),
        arguments: vec![],
        cfg: Digest::default(),
        profile: ProfileIdentity::current(),
    });
    let token = other
        .issue(query, f.token.phase().clone(), f.input.clone())
        .unwrap();
    other.start(&f.context, &token).unwrap();
    other.accept_module(&f.context, &token, &changed).unwrap();
    assert_eq!(
        other
            .adopt_module(&f.context, &token, &mut f.db, changed)
            .unwrap_err()
            .code,
        "LRR-E-MODULE-CONFLICT"
    );
    let QueryPoll::Ready(Ok(lease)) = SyntaxProvider::module_capture(&mut f.db, &f.input) else {
        panic!("first capture lost");
    };
    assert_eq!(lease.record().identity(), f.record.identity());
}
#[test]
fn module_adoption_cannot_be_injected_through_ordinary_source_publication() {
    let mut f = fixture(false);
    assert!(
        f.db.publish_inputs(BTreeMap::from([(
            InputKey::ModuleRecord(f.input.fingerprint()),
            InputValue::ModuleRecord {
                input: f.input.fingerprint(),
                captured: std::sync::Arc::new(f.record.clone())
            }
        )]))
        .is_err()
    );
    assert!(
        f.db.input(&InputKey::ModuleRecord(f.input.fingerprint()))
            .is_none()
    );
}

#[test]
fn module_adoption_request_identity_binds_environment_producer_unit_and_generation() {
    let mut f = fixture(false);
    adopt(&mut f);
    let mut other_env = f.input.clone();
    let InputRequest::CaptureModule {
        compile_environment,
        ..
    } = &mut other_env
    else {
        unreachable!();
    };
    *compile_environment = Digest([18; 32]);
    assert_ne!(other_env.fingerprint(), f.input.fingerprint());
    assert!(matches!(
        SyntaxProvider::module_capture(&mut f.db, &other_env),
        QueryPoll::NeedInput(_)
    ));
    let query = || {
        QueryKey::Constant(ConstantKey {
            definition: Digest([19; 32]),
            arguments: vec![],
            cfg: Digest::default(),
            profile: ProfileIdentity::current(),
        })
    };
    assert!(
        f.coordinator
            .issue(query(), f.token.phase().clone(), other_env)
            .is_err()
    );
    for (unit, generation) in [
        (Digest([20; 32]), f.context.source_generation),
        (f.context.unit, Digest([21; 32])),
    ] {
        let root = Digest([3; 32]);
        let authorities = IncludeAuthorities::new(
            unit,
            generation,
            [IncludeRoot {
                identity: root,
                path: f.path.clone(),
                kind: IncludeRootKind::Source,
            }],
        )
        .unwrap();
        let from = IncludeLocation {
            unit,
            root,
            path: SourcePath::new("main.rs").unwrap(),
            origin: Key([4; 32]),
        };
        let request = authorities
            .module_request(
                &from,
                std::path::Path::new(""),
                ModuleRequest {
                    module_path: vec!["child".into()],
                    directory: vec![],
                    file_relative: false,
                    name: "child".into(),
                    path_override: None,
                },
                Digest([5; 32]),
                Key([6; 32]),
            )
            .unwrap();
        let input = InputRequest::CaptureModule {
            request: Box::new(request),
            compile_environment: f.context.compile_environment,
        };
        assert!(
            f.coordinator
                .issue(query(), f.token.phase().clone(), input)
                .is_err()
        );
    }
    let authorities = IncludeAuthorities::new(
        f.context.unit,
        f.context.source_generation,
        [IncludeRoot {
            identity: Digest([3; 32]),
            path: f.path.clone(),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap();
    let from = IncludeLocation {
        unit: f.context.unit,
        root: Digest([3; 32]),
        path: SourcePath::new("main.rs").unwrap(),
        origin: Key([4; 32]),
    };
    let other = authorities
        .module_request(
            &from,
            std::path::Path::new(""),
            ModuleRequest {
                module_path: vec!["child".into()],
                directory: vec![],
                file_relative: false,
                name: "child".into(),
                path_override: None,
            },
            Digest([22; 32]),
            Key([6; 32]),
        )
        .unwrap();
    let input = InputRequest::CaptureModule {
        request: Box::new(other),
        compile_environment: f.context.compile_environment,
    };
    assert_ne!(input.fingerprint(), f.input.fingerprint());
    let token = f
        .coordinator
        .issue(query(), f.token.phase().clone(), input)
        .unwrap();
    f.coordinator.start(&f.context, &token).unwrap();
    assert!(
        f.coordinator
            .accept_module(&f.context, &token, &f.record)
            .is_err()
    );
}
#[test]
fn module_adoption_absence_is_evidence_and_never_a_completed_namespace() {
    let mut f = fixture(false);
    fs::remove_file(f.path.join("child.rs")).unwrap();
    f.record = f.capture.capture_module(&f.request).unwrap();
    assert_eq!(
        f.record.selected().unwrap_err().code,
        "LRR-E-SOURCE-MISSING"
    );
    adopt(&mut f);
    let QueryPoll::Ready(Ok(lease)) = SyntaxProvider::module_capture(&mut f.db, &f.input) else {
        panic!("absence record was lost");
    };
    assert_eq!(
        lease.record().selected().unwrap_err().code,
        "LRR-E-SOURCE-MISSING"
    );
    assert_eq!(f.db.cache_usage().unwrap().source_input_bytes, 0);
    fs::write(f.path.join("child.rs"), "pub fn new(){}").unwrap();
    assert_eq!(
        f.capture
            .revalidate_module(&f.request, lease.record())
            .unwrap_err()
            .code,
        "LRR-E-SNAPSHOT-RACE"
    );
    assert_eq!(
        lease.record().selected().unwrap_err().code,
        "LRR-E-SOURCE-MISSING"
    );
}

#[test]
fn module_adoption_pure_lookup_retains_snapshot_when_the_original_file_disappears() {
    let mut f = fixture(false);
    adopt(&mut f);
    fs::remove_file(f.path.join("child.rs")).unwrap();
    let QueryPoll::Ready(Ok(lease)) = SyntaxProvider::module_capture(&mut f.db, &f.input) else {
        panic!("pure lookup probed the changed filesystem");
    };
    assert_eq!(lease.record().identity(), f.record.identity());
    assert_eq!(
        lease.record().selected().unwrap().blob().text().unwrap(),
        "pub fn made()->i64{1657_i64}"
    );
    assert_eq!(
        f.capture
            .revalidate_module(&f.request, lease.record())
            .unwrap_err()
            .code,
        "LRR-E-SNAPSHOT-RACE"
    );
}
