//! M015 mandatory item admission after a successful matched procedural callback.
//! Native control requires explicit preparation; source-only compilation never runs it.
use lrr::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::InputKey,
        key::{OutlineKey, QueryKey},
        memo::Database,
    },
    expand::{
        cache::ExpansionOutputCache,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    service::{
        expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
        macro_session::MacroSession,
        run::MacroRun,
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        manifest::BodyKey,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
#[allow(dead_code)]
#[path = "support/macro_service.rs"]
mod service_fixture;

const UPSTREAM: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-bad.rs");
const AUXILIARY: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/derive-bad.rs"
);
const STDERR: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-bad.stderr");
static EVIDENCE_ID: AtomicU64 = AtomicU64::new(0);

fn adapted_source() -> String {
    // Existing support legitimately publishes dependency alias lrr_fixture_macros.
    // Preserve the original consumer alias via this one explicit extern rename.
    // The supplementary known test enables filter-intention controls; neither
    // its body nor the stock main body is compiled or executed by this harness.
    let source = UPSTREAM.replacen(
        "extern crate derive_bad;",
        "extern crate lrr_fixture_macros as derive_bad;",
        1,
    );
    format!("{source}\n#[test] fn known() {{}}\n")
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
fn evidence() -> PathBuf {
    let base = root().join("artifacts/macos-first");
    fs::create_dir_all(&base).unwrap();
    loop {
        let id = EVIDENCE_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .unwrap();
        let path = base.join(format!(
            "m015-proc-catalog-admission-{}-{id}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return fs::canonicalize(path).unwrap(),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("exclusive evidence directory: {error}"),
        }
    }
}
fn capture(text: &str, unit: Digest) -> (PreparedSourceInputs, PreparedSourceView) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    let path = SourcePath::new("lib.rs").unwrap();
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
        panic!("exact consumer absent")
    };
    (inputs, view)
}
fn assert_no_body_demand(db: &Database) {
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
}
fn assert_catalog_incomplete(error: &Diagnostic) {
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(
        error.message,
        "expansion input publication requires complete discovery"
    );
}
fn assert_closed_run(error: &Diagnostic) {
    assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
    assert_eq!(
        error.message,
        "macro run is closed or its frozen revision/epoch changed"
    );
}
fn assert_item_refusal(error: &Diagnostic) {
    assert_eq!(error.code, "LRR-E-MACRO-OUTPUT");
    assert_eq!(error.message, "invalid Items expansion: expected COLON");
}
fn filtered_names(
    discovery: &SourceDiscovery,
    db: &mut Database,
    filter: Option<&str>,
    exact: bool,
) -> Result<Vec<String>, Diagnostic> {
    let catalog = discovery.test_catalog(db)?;
    Ok(catalog
        .select(filter, exact)
        .into_iter()
        .map(|case| case.name.clone())
        .collect())
}

#[test]
fn upstream_derive_bad_and_alias_adaptation_preserve_exact_failure_source() {
    assert!(UPSTREAM.contains("//@ proc-macro: derive-bad.rs"));
    assert!(UPSTREAM.contains("#[derive(A)]"));
    assert!(AUXILIARY.contains("\"struct A { inner }\".parse().unwrap()"));
    assert!(STDERR.contains("error: expected `:`, found `}`"));
    assert!(STDERR.contains("proc-macro derive produced unparsable tokens"));
    assert!(STDERR.contains("the name `A` is defined multiple times"));
    let adapted = adapted_source();
    let original = adapted
        .strip_suffix("\n#[test] fn known() {}\n")
        .unwrap()
        .replacen(
            "extern crate lrr_fixture_macros as derive_bad;",
            "extern crate derive_bad;",
            1,
        );
    assert_eq!(original, UPSTREAM);
}

fn exercise(raw_component: bool) {
    let work = evidence();
    let macro_source = work.join("macro-source");
    fs::create_dir_all(macro_source.join("src")).unwrap();
    fs::write(macro_source.join("Cargo.toml"), "[package]\nname=\"lrr_fixture_macros\"\nversion=\"0.1.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    fs::write(macro_source.join("src/lib.rs"), AUXILIARY).unwrap();
    assert_eq!(
        fs::read(macro_source.join("src/lib.rs")).unwrap(),
        AUXILIARY.as_bytes()
    );
    let source = adapted_source();
    let manifest = service_fixture::prepare_sources(
        &work.join("prepared"),
        &macro_source,
        &[("lib.rs", &source)],
    );
    let plan = lrr::project::tool_manifest::InputPlan::new(
        [work.join("prepared/app"), macro_source.clone()]
            .into_iter()
            .map(|path| {
                lrr::project::tool_manifest::InputRoot::open(
                    lrr::project::tool_manifest::InputKind::Source,
                    &path,
                    Default::default(),
                    Default::default(),
                    true,
                    Default::default(),
                )
                .unwrap()
            })
            .collect(),
        Default::default(),
    )
    .unwrap();
    let unit = *manifest.graph.roots.first().unwrap();
    let (inputs, view) = capture(&source, unit);
    let known = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "known")
        .unwrap()
        .def_key
        .fingerprint();
    let environment =
        lrr::expand::builtins::FrozenCompileEnvironment::new(unit, &manifest.compile_env[&unit])
            .unwrap();
    let inputs = inputs
        .with_compile_environment(Arc::new(environment))
        .unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let context = DemandContext {
        request: Digest([241; 32]),
        session: Digest([242; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
    let procedural = run.catalog(&mut db, unit).unwrap();
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_declarative_procedural(context.compile_environment, Digest([243; 32]), procedural)
    .unwrap()
    .with_compiler(Arc::new(
        CompileInputCatalog::standalone(&ProfileIdentity::current()).unwrap(),
    ))
    .unwrap()
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap();
    assert_catalog_incomplete(
        &discovery
            .test_catalog(&mut db)
            .expect_err("unsealed catalog cannot list"),
    );
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut request = None;
    for _ in 0..16 {
        match discovery
            .poll()
            .unwrap_or_else(|error| panic!("pre-append prerequisite refusal: {error:?}"))
        {
            SourceDiscoveryStep::NeedDeriveDeclaration {
                request: declaration,
            } => {
                let lrr::service::derive_declarations::DeriveDeclarationPoll::Ready(receipt) = run
                    .resolve_derive_declaration(&mut db, &declaration)
                    .unwrap()
                else {
                    panic!("actual helper declaration unavailable")
                };
                discovery
                    .adopt_derive_declaration(&mut db, &declaration, Arc::new(receipt))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedSourceDeriveSelection { selection } => {
                assert!(request.is_none(), "one original derive occurrence only");
                let bound = run
                    .prepare_source_derive_selection(&mut db, &selection)
                    .unwrap();
                assert_eq!(bound.request().export, "A");
                discovery.bind_source_derive_request(bound.clone()).unwrap();
                request = Some(bound);
            }
            SourceDiscoveryStep::NeedSourceDeriveExpansion { request: bound } => {
                assert_eq!(bound.key(), request.as_ref().unwrap().key());
                let phase = PhaseIdentity {
                    frame: Digest([244; 32]),
                    body: BodyKey {
                        definition: Digest([245; 32]),
                        exact_body_bytes: Digest([246; 32]),
                    },
                    index: 0,
                };
                if raw_component {
                    // Separate explicit session: the rejected operation never supplies a raw record to
                    // MacroRun, SourceDiscovery, the database or the output cache.
                    run.cancel();
                    let mut raw_context = context.clone();
                    raw_context.session = Digest([248; 32]);
                    let mut session = MacroSession::open(
                        raw_context.clone(),
                        &manifest,
                        true,
                        work.join("prepared/app"),
                        std::time::Duration::from_secs(10),
                        4 * 1024 * 1024,
                    )
                    .unwrap();
                    let key = session.request_key(&raw_context, bound.request()).unwrap();
                    assert_eq!(&key, bound.key());
                    let mut coordinator = ExpansionCoordinator::new(raw_context.clone());
                    let token = coordinator
                        .issue(
                            QueryKey::CurrentExpansionInput(Box::new(key.clone())),
                            phase,
                            lrr::db::poll::InputRequest::Expand(Box::new(
                                lrr::db::poll::ExpansionRequest { key: key.clone() },
                            )),
                        )
                        .unwrap();
                    let primary = session
                        .fulfil(&raw_context, &mut coordinator, &token, bound.request())
                        .expect_err(
                            "public session rejects invalid Items before returning a schema record",
                        );
                    assert_item_refusal(&primary);
                    let replay = session
                        .fulfil(&raw_context, &mut coordinator, &token, bound.request())
                        .expect_err(
                            "failed operation replays its exact refusal without another callback",
                        );
                    assert_eq!(primary.code, replay.code);
                    assert_eq!(primary.message, replay.message);
                    assert_eq!(session.invocations(), 1);
                    assert_eq!(run.unit_invocations(unit), 0);
                    assert!(
                        db.input(&InputKey::ExpansionRecord(key.fingerprint()))
                            .is_none()
                    );
                    assert_catalog_incomplete(
                        &discovery
                            .test_catalog(&mut db)
                            .expect_err("separate session grants no catalog seal"),
                    );
                    assert_no_body_demand(&db);
                    fs::write(work.join("session-component-report.json"),serde_json::to_vec_pretty(&serde_json::json!({
                        "component":"separate public MacroSession procedural output grammar refusal", "requested_key":key.fingerprint().hex(),
                        "original_root_unit":unit.hex(),"session_callbacks":session.invocations(),"run_callbacks":run.unit_invocations(unit),
                        "schema_record_returned":false,"database_adoption":false,
                        "primary":{"code":primary.code,"message":primary.message},"exact_error_replay":true,
                        "body_queries":0,"stock_full_fixture_passes":0,"guest_execution":false
                    })).unwrap()).unwrap();
                    session.cancel();
                    return;
                }
                let primary = run
                    .fulfil_source_derive(
                        &mut db,
                        &mut discovery,
                        phase.clone(),
                        &bound,
                        &mut cache,
                    )
                    .err()
                    .expect("real MacroRun rejects malformed Items before database adoption");
                assert_item_refusal(&primary);
                assert_eq!(run.unit_invocations(unit), 1);
                assert!(
                    db.input(&InputKey::ExpansionRecord(bound.key().fingerprint()))
                        .is_none()
                );
                let mut polls = Vec::new();
                let mut filters = Vec::new();
                for _ in 0..3 {
                    match discovery.poll() {
                        Ok(SourceDiscoveryStep::NeedSourceDeriveExpansion { request: again }) => {
                            assert_eq!(again.key(), bound.key());
                            polls.push(serde_json::json!({"state":"pending unadopted expansion","primary_failure_replay_proven":false}));
                        }
                        Err(error) => {
                            assert_eq!(error.code, primary.code);
                            assert_eq!(error.message, primary.message);
                            polls.push(serde_json::json!({"state":"exact primary refusal","code":error.code,"message":error.message}));
                        }
                        Ok(_) => panic!(
                            "required output refusal cannot seal or advance to unrelated producers"
                        ),
                    }
                    for (filter, exact) in [
                        (None, false),
                        (Some("known"), true),
                        (Some("kno"), false),
                        (Some("absent"), true),
                    ] {
                        let error = filtered_names(&discovery, &mut db, filter, exact).expect_err(
                            "required output refusal forbids successful filtered catalog",
                        );
                        assert_catalog_incomplete(&error);
                        filters.push(serde_json::json!({"filter":filter,"exact":exact,"code":error.code,"message":error.message}));
                    }
                    let error = run
                        .fulfil_source_derive(
                            &mut db,
                            &mut discovery,
                            phase.clone(),
                            &bound,
                            &mut cache,
                        )
                        .err()
                        .expect("failed run cannot execute another callback");
                    assert_closed_run(&error);
                    assert_eq!(run.unit_invocations(unit), 1);
                    assert_no_body_demand(&db);
                }
                let publication = run
                    .publish_discovery(&mut db, &mut discovery)
                    .expect_err("closed failed run cannot publish");
                assert_closed_run(&publication);
                assert_catalog_incomplete(
                    &discovery
                        .test_catalog(&mut db)
                        .expect_err("failed publication has no catalog"),
                );
                assert!(
                    db.input(&InputKey::ExpansionRecord(bound.key().fingerprint()))
                        .is_none()
                );
                assert_eq!(run.unit_invocations(unit), 1);
                assert_no_body_demand(&db);
                fs::write(work.join("component-report.json"),serde_json::to_vec_pretty(&serde_json::json!({
                    "fixture":"tests/ui/proc-macro/derive-bad.rs","upstream_commit":"ae0fe511b15ab760d7e963b13389d1bc2ab42b18",
                    "primary":{"code":primary.code,"message":primary.message},"database_adoption":false,"item_append_reached":false,
                    "original_root_unit":unit.hex(),"known_original_definition":known.hex(),"repeated_polls":polls,"filtered_catalog_outcomes":filters,
                    "publication_refusal":{"code":publication.code,"message":publication.message},"macro_callbacks":1,"body_queries":0,
                    "failure_boundary":"real MacroRun adapter procedural output grammar admission before schema/database adoption; repeat fulfil and publication prove run closed",
                    "filter_scope":"public catalog/select composition only; no CLI execution claim","stock_full_fixture_passes":0,"guest_execution":false
                })).unwrap()).unwrap();
                return;
            }
            SourceDiscoveryStep::Sealed(_) => {
                panic!("malformed mandatory output was silently sealed")
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!("callback prerequisite incomplete: {diagnostic:?}, {blocked:?}"),
            _ => panic!("unrelated producer requested instead of original derive"),
        }
    }
    panic!("bounded control failed to reach original derive callback");
}

#[test]
fn matched_derive_bad_callback_cannot_seal_any_filtered_catalog() {
    exercise(false);
}

#[test]
fn separate_session_derive_bad_refusal_cannot_return_schema() {
    exercise(true);
}
