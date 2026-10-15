//! Actual capture/discovery publication controls, not guest or upstream passes.
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::{Database, PreparedModuleRegistrySnapshot},
        poll::{InputRequest, QueryPoll},
    },
    engine::telemetry::CacheBudgets,
    expand::{
        builtins::FrozenCompileEnvironment,
        source_declarative,
        source_discovery::{
            ModuleSourceReadiness, OrdinaryModuleReadiness, SourceDiscovery, SourceDiscoveryStep,
        },
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    project::tool_manifest::CompileEnv,
    service::{
        expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
        source_capture::SourceCapture,
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        include_paths::{IncludeAuthorities, IncludeRoot, IncludeRootKind},
        manifest::BodyKey,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    sync::atomic::{AtomicU64, Ordering},
};
const UNIT: Digest = Digest([161; 32]);
const GENERATION: Digest = Digest([162; 32]);
const AUTHORITY: Digest = Digest([163; 32]);
const ROOT: &str = "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}";
struct Fixture {
    db: Database,
    discovery: SourceDiscovery,
    // These actual module views own the immutable source states. The DB registry
    // remains weak and must never resurrect a dropped source world.
    child: PreparedSourceView,
    grandchild: PreparedSourceView,
    directory: PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}
fn authorities(directory: &Path) -> IncludeAuthorities {
    IncludeAuthorities::new(
        UNIT,
        GENERATION,
        [IncludeRoot {
            identity: AUTHORITY,
            path: directory.to_owned(),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap()
}
fn root_inputs(text: &str, edition: u16) -> (PreparedSourceInputs, PreparedSourceView) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let selection = SourceSelection {
        path: path.clone(),
        outline: crate::db::key::OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: UNIT,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let environment = FrozenCompileEnvironment::new(
        UNIT,
        &CompileEnv {
            unit: UNIT,
            host: "aarch64-apple-darwin".into(),
            target: "aarch64-apple-darwin".into(),
            cfg: cfg.clone(),
            cargo: BTreeMap::new(),
            rustc_env: BTreeMap::new(),
            declared: BTreeMap::new(),
            out_dir: None,
        },
    )
    .unwrap();
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: UNIT,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap()
    .with_compile_environment(Arc::new(environment))
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("real root source absent")
    };
    (inputs, view)
}
fn adopt(
    db: &mut Database,
    directory: &Path,
    input: &InputRequest,
    index: u32,
    caller: &SourceSelection,
) {
    let InputRequest::CaptureModule {
        request,
        compile_environment,
    } = input
    else {
        panic!("actual module capture request absent")
    };
    let capture = SourceCapture::open(authorities(directory), BTreeMap::new()).unwrap();
    let record = capture.capture_module(request).unwrap();
    let context = DemandContext {
        request: input.fingerprint(),
        session: Digest([164; 32]),
        unit: UNIT,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: GENERATION,
        native_generation: None,
        compile_environment: *compile_environment,
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let phase = PhaseIdentity {
        frame: Digest([165; 32]),
        body: BodyKey {
            definition: request.producer(),
            exact_body_bytes: input.fingerprint(),
        },
        index,
    };
    let token = coordinator
        .issue(
            QueryKey::Outline(caller.outline.clone()),
            phase,
            input.clone(),
        )
        .unwrap();
    coordinator.start(&context, &token).unwrap();
    coordinator
        .accept_module(&context, &token, &record)
        .unwrap();
    coordinator
        .adopt_module(&context, &token, db, record)
        .unwrap();
}
fn fixture(edition: u16) -> Fixture {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let sequence = NEXT
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "lrr-prepared-module-registry-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(directory.join("child")).unwrap();
    let directory = std::fs::canonicalize(directory).unwrap();
    std::fs::write(
        directory.join("child.rs"),
        "pub mod grandchild;pub fn child_value()->i64{3_i64}",
    )
    .unwrap();
    std::fs::write(
        directory.join("child/grandchild.rs"),
        "pub fn made()->i64{5_i64}",
    )
    .unwrap();
    let (inputs, root) = root_inputs(ROOT, edition);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let environment = match db
        .published_input(&InputKey::UnitCompileEnv(UNIT))
        .unwrap()
        .0
    {
        InputValue::UnitCompileEnv(environment) => environment.fingerprint(),
        _ => panic!("actual owning compile environment absent"),
    };
    let mut discovery = SourceDiscovery::new(
        vec![root.share()],
        root.outline().root_scope.clone(),
        crate::source::manifest::Namespace::Value,
        Digest([166; 32]),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        environment,
        source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&root.selection().outline.parse.profile).unwrap()),
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = discovery.poll().unwrap() else {
        panic!("actual generated module producer absent")
    };
    discovery
        .admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = discovery
        .unresolved_module_sources()
        .next()
        .unwrap()
        .producer();
    let child_input = discovery
        .prepare_module_source(producer, &authorities(&directory), Path::new(""))
        .unwrap();
    adopt(
        &mut db,
        &directory,
        &child_input,
        0,
        &discovery.module_capture_caller(producer).unwrap(),
    );
    let child = match discovery.stage_module_source(&mut db, &child_input) {
        QueryPoll::Ready(Ok(syntax)) => syntax.prepared_source().share(),
        QueryPoll::Ready(Err(error)) => panic!("actual child staging: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual child staging demand: {input:?}"),
    };
    assert!(matches!(
        discovery.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let ordinary = discovery
        .unresolved_ordinary_module_sources()
        .next()
        .unwrap()
        .producer();
    let grandchild_input = discovery
        .prepare_ordinary_module_source(ordinary, &authorities(&directory))
        .unwrap();
    adopt(
        &mut db,
        &directory,
        &grandchild_input,
        1,
        &discovery.ordinary_module_capture_caller(ordinary).unwrap(),
    );
    let grandchild = match discovery.stage_ordinary_module_source(&mut db, &grandchild_input) {
        QueryPoll::Ready(Ok(syntax)) => syntax.prepared_source().share(),
        QueryPoll::Ready(Err(error)) => panic!("actual grandchild staging: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual grandchild staging demand: {input:?}"),
    };
    assert!(matches!(
        discovery.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        discovery.complete_ordinary_module_namespace(&mut db, &grandchild_input),
        QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Ready(_)))
    ));
    // The generated parent is still a separate genuine namespace obligation.
    assert!(matches!(
        discovery.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        discovery.complete_module_namespace(&mut db, &child_input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(_)))
    ));
    assert!(matches!(
        discovery.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    Fixture {
        db,
        discovery,
        child,
        grandchild,
        directory,
    }
}
fn state(db: &Database) -> PreparedModuleRegistrySnapshot {
    db.prepared_module_registry_snapshot().unwrap()
}
fn require_current_views(f: &Fixture) {
    for view in [&f.child, &f.grandchild] {
        let actual =
            f.db.prepared_source(view.selection())
                .unwrap()
                .expect("published adopted view lease");
        assert_eq!(actual.selection(), view.selection());
        assert!(std::ptr::eq(actual.outline(), view.outline()));
        let parsed =
            f.db.prepared_parse(&view.selection().outline.parse)
                .expect("published adopted parse lease");
        assert_eq!(parsed.key(), view.selection().outline.parse);
    }
}
#[test]
fn adopted_module_publication_registers_live_source_and_parse_and_is_idempotent() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut f = fixture(edition);
        assert!(f.db.prepared_source(f.child.selection()).unwrap().is_none());
        assert!(
            f.db.prepared_source(f.grandchild.selection())
                .unwrap()
                .is_none()
        );
        let source_revision = f.db.source_revision();
        f.discovery.publish(&mut f.db).unwrap();
        require_current_views(&f);
        assert_eq!(f.db.source_revision(), source_revision);
        let before = state(&f.db);
        f.discovery.publish(&mut f.db).unwrap();
        assert_eq!(
            state(&f.db),
            before,
            "identical publication must not grow the weak registry or input world"
        );
        require_current_views(&f);
    }
}
fn refuses_budget(query: bool) {
    let mut f = fixture(2024);
    let before = f.db.cache_usage().unwrap();
    let budgets = if query {
        CacheBudgets {
            query_bytes: before.memo_bytes,
            ..CacheBudgets::default()
        }
    } else {
        CacheBudgets {
            parse_bytes: before.parse_bytes,
            ..CacheBudgets::default()
        }
    };
    f.db.begin_run_with_budgets(budgets).unwrap();
    let before = state(&f.db);
    let error = f.discovery.publish(&mut f.db).unwrap_err();
    assert_eq!(error.code, "LRR-E-METADATA-LIMIT");
    assert_eq!(
        error.message,
        "discovery prepared source registry exceeds cache budgets"
    );
    assert_eq!(state(&f.db), before);
    assert!(f.db.prepared_source(f.child.selection()).unwrap().is_none());
    assert!(
        f.db.prepared_source(f.grandchild.selection())
            .unwrap()
            .is_none()
    );
    f.db.finish_run().unwrap();
    f.discovery.publish(&mut f.db).unwrap();
    require_current_views(&f);
}
#[test]
fn adopted_module_query_budget_refusal_preserves_all_publication_state() {
    refuses_budget(true);
}
#[test]
fn adopted_module_parse_budget_refusal_preserves_all_publication_state() {
    refuses_budget(false);
}
#[test]
fn genuine_source_replacement_refuses_retained_old_adopted_leases() {
    let mut f = fixture(2024);
    f.discovery.publish(&mut f.db).unwrap();
    require_current_views(&f);
    let (replacement, _root) = root_inputs("fn main()->i64{7_i64}", 2024);
    replacement.publish(&mut f.db).unwrap();
    assert!(f.db.prepared_source(f.child.selection()).unwrap().is_none());
    assert!(
        f.db.prepared_source(f.grandchild.selection())
            .unwrap()
            .is_none()
    );
    let before = state(&f.db);
    assert!(f.discovery.publish(&mut f.db).is_err());
    assert_eq!(state(&f.db), before);
}
