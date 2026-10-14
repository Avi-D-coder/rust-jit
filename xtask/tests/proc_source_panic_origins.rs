//! M016 panic-origin control using unchanged official rustc source and auxiliary.
//! This does not claim full compiletest or guest-tier acceptance.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
    },
    expand::{
        cache::ExpansionOutputCache,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    service::{
        expansion::{DemandContext, PhaseIdentity},
        run::MacroRun,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
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
    time::{SystemTime, UNIX_EPOCH},
};
#[allow(dead_code)]
#[path = "support/macro_service.rs"]
mod service_fixture;

#[test]
fn rustc_original_function_like_panic_retains_bound_invocation_and_definition() {
    exercise(Case {
        source: include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/issue-76270-panic-in-libproc-macro.rs"
        ),
        auxiliary: include_bytes!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/proc-macro-panic.rs"
        ),
        alias: "proc_macro_panic",
        export: "panic_in_libproc_macro",
        function: "panic_in_libproc_macro",
        edition: 2018,
        derive: false,
        message: "not a valid identifier",
    });
}
#[test]
fn rustc_original_derive_panic_retains_bound_occurrence_and_native_definition() {
    exercise(Case {
        source: include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/load-panic.rs"
        ),
        auxiliary: include_bytes!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/test-macros.rs"
        ),
        alias: "test_macros",
        export: "Panic",
        function: "panic_derive",
        edition: 2015,
        derive: true,
        message: "panic-derive",
    });
}
struct Case {
    source: &'static str,
    auxiliary: &'static [u8],
    alias: &'static str,
    export: &'static str,
    function: &'static str,
    edition: u16,
    derive: bool,
    message: &'static str,
}
fn exercise(case: Case) {
    service_fixture::require_current_validation_source();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();

    for (relative, embedded) in [
        (
            "crates/lrr/src/frontend/procedural_exports.rs",
            include_bytes!("../../crates/lrr/src/frontend/procedural_exports.rs").as_slice(),
        ),
        (
            "crates/lrr/src/service/source_derive_requests.rs",
            include_bytes!("../../crates/lrr/src/service/source_derive_requests.rs").as_slice(),
        ),
        (
            "xtask/tests/proc_source_panic_origins.rs",
            include_bytes!("proc_source_panic_origins.rs").as_slice(),
        ),
    ] {
        assert_eq!(
            embedded,
            fs::read(root.join(relative)).unwrap(),
            "stale validation source: {relative}"
        );
    }
    let work = allocate_work(&root.join("artifacts/macos-first"));
    fs::create_dir_all(work.join("macro/src")).unwrap();
    let work = fs::canonicalize(work).unwrap();
    let upstream_auxiliary = case.auxiliary;
    fs::write(work.join("macro/src/lib.rs"), upstream_auxiliary).unwrap();
    fs::write(work.join("macro/Cargo.toml"), format!("[package]\nname=\"{}\"\nversion=\"0.0.0\"\nedition=\"{}\"\n[lib]\nproc-macro=true\n[workspace]\n", case.alias, case.edition)).unwrap();
    let source = case.source;
    // The helper must prepare only this macro-host dependency with its original crate alias.
    let manifest = service_fixture::prepare_rustc_macro_sources(
        &work.join("prepared"),
        &work.join("macro"),
        source,
        case.alias,
    );
    let plan = lrr::project::tool_manifest::InputPlan::new(
        [work.join("prepared/app"), work.join("macro")]
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
    let profile = ProfileIdentity::current();
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("main.rs").unwrap();
    let selected = SourceSelection {
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
    let environment =
        lrr::expand::builtins::FrozenCompileEnvironment::new(unit, &manifest.compile_env[&unit])
            .unwrap();
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg: cfg.clone(),
            blob,
        }],
        Default::default(),
    )
    .unwrap()
    .with_compile_environment(Arc::new(environment))
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selected).unwrap() else {
        panic!("original rustc source absent")
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let context = DemandContext {
        request: Digest([201; 32]),
        session: Digest([202; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
    let catalog = run.catalog(&mut db, unit).unwrap();
    let compiler = Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &ProfileIdentity::current(),
        )
        .unwrap(),
    );
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    // The original extern crate needs the already prepared macro-only alias
    // before original source routes are constructed.
    .with_declarative_procedural(
        context.compile_environment,
        Digest([203; 32]),
        catalog.clone(),
    )
    .unwrap()
    .with_compiler(compiler)
    .unwrap()
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap();
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();

    // All requests below use the original retained source in the required live
    // Rust 2024 profile. Stock edition directives/expected stderr remain retained;
    // this scoped LRR origin test does not assert full compiletest parity.
    let mut bound = None;
    let mut steps = 0u32;
    let (error, request) = loop {
        steps = steps.checked_add(1).unwrap();
        assert!(
            steps <= 32,
            "stock native panic discovery exceeded bounded fixture work"
        );
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedDeriveDeclaration { request } if case.derive => {
                let lrr::service::derive_declarations::DeriveDeclarationPoll::Ready(receipt) =
                    run.resolve_derive_declaration(&mut db, &request).unwrap()
                else {
                    panic!("original native derive header was not admitted");
                };
                discovery
                    .adopt_derive_declaration(&mut db, &request, Arc::new(receipt))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedSourceDeriveSelection { selection } if case.derive => {
                let request = run
                    .prepare_source_derive_selection(&mut db, &selection)
                    .unwrap();
                assert_eq!(request.request().export, case.export);
                bound = Some(request.clone());
                discovery.bind_source_derive_request(request).unwrap();
            }
            SourceDiscoveryStep::NeedProceduralSelection { selection } if !case.derive => {
                use lrr_expand_adapter::{
                    proc_client::{CapturedSpanSource, SourceDigestEncoding},
                    token_wire::{
                        self, ContextRecord, Edition, Key, SourceIdentity, TokenTree, Transparency,
                    },
                };
                let (producer, artifact) = manifest
                    .artifacts
                    .iter()
                    .find(|(_, artifact)| {
                        artifact.kind == lrr::project::tool_manifest::ArtifactKind::ProcMacroLibrary
                    })
                    .unwrap();
                let artifact =
                    lrr::service::macro_session::macro_artifact_identity(*producer, artifact);
                let text = std::str::from_utf8(upstream_auxiliary).unwrap();
                let blob = SourceBlob::new(upstream_auxiliary.to_vec()).unwrap();
                let native = token_wire::parse_source(
                    text,
                    SourceIdentity {
                        unit: Key(producer.0),
                        content: Key(blob.content().0),
                        path: "src/lib.rs".into(),
                    },
                    ContextRecord {
                        parent: None,
                        expansion: None,
                        transparency: Transparency::Opaque,
                        defining_unit: Key(producer.0),
                        edition: Edition::E2018,
                    },
                    Key(artifact.0),
                )
                .unwrap();
                let [TokenTree::Group { open, .. }] = native.roots() else {
                    panic!("native source root absent");
                };
                let native_site = *open;
                let native_source = native
                    .spans()
                    .iter()
                    .find(|span| span.key() == native_site)
                    .unwrap()
                    .source
                    .clone();
                let native = token_wire::TokenForest::decode(&native.encode().unwrap()).unwrap();
                let target = lrr::service::item_macro_requests::ResolvedItemTarget {
                    artifact: Key(artifact.0),
                    export: case.export.into(),
                    scope: Key(selection.input().scope().fingerprint().0),
                    parent_origin: selection.input().parent_origin(),
                    registry: &native,
                    definition_site: native_site,
                    call_site: selection.input().call_site(),
                    mixed_site: selection.input().call_site(),
                    sources: vec![
                        CapturedSpanSource::new(
                            native_source,
                            text.into(),
                            SourceDigestEncoding::LrrSourceBytes,
                            None,
                        )
                        .unwrap(),
                    ],
                };
                let request = run
                    .prepare_source_selection(&mut db, &selection, target)
                    .unwrap();
                bound = Some(request.clone());
                discovery.bind_procedural_request(request).unwrap();
            }
            SourceDiscoveryStep::NeedExpansion { request } if !case.derive => {
                let selected = bound.as_ref().expect("stock source request was not bound");
                assert!(request.procedural_request().is_some());
                assert_eq!(request.key(), selected.key());
                let phase = PhaseIdentity {
                    frame: Digest([222; 32]),
                    body: BodyKey {
                        definition: Digest([223; 32]),
                        exact_body_bytes: Digest([224; 32]),
                    },
                    index: 0,
                };
                let error = run
                    .fulfil_discovery(&mut db, &mut discovery, phase, selected, &mut cache)
                    .err()
                    .expect("original stock native invocation did not panic");
                break (error, selected.key().clone());
            }
            SourceDiscoveryStep::NeedSourceDeriveExpansion { request } if case.derive => {
                assert_eq!(
                    Some(request.key()),
                    bound.as_ref().map(|request| request.key())
                );
                let phase = PhaseIdentity {
                    frame: Digest([222; 32]),
                    body: BodyKey {
                        definition: Digest([223; 32]),
                        exact_body_bytes: Digest([224; 32]),
                    },
                    index: 0,
                };
                let error = run
                    .fulfil_source_derive(&mut db, &mut discovery, phase, &request, &mut cache)
                    .err()
                    .expect("original stock native invocation did not panic");
                break (error, request.key().clone());
            }
            _ => panic!("stock native panic did not produce its exact source request"),
        }
    };
    assert_eq!(error.code, "LRR-E-MACRO-PANIC", "{error}");
    assert!(error.message.contains(case.message), "{error}");
    assert!(error.origin_attachment_failure.is_none(), "{error}");
    let origin = error
        .macro_origin
        .as_ref()
        .expect("actual stock native panic lacks retained origin");
    assert_eq!(origin.invocation_identity, request.invocation);
    let at = origin.invocation.as_ref().unwrap();
    assert_eq!(at.source.unit.0, unit.0);
    assert_eq!(at.source.content.0, Digest::of_bytes(source.as_bytes()).0);
    assert_eq!(at.source.path, "main.rs");
    let invocation = &source[at.range.start as usize..at.range.end as usize];
    if case.derive {
        assert_eq!(invocation, "Panic");
    } else {
        assert!(invocation.contains("proc_macro_panic::panic_in_libproc_macro!()"));
    }
    let definition = origin.definition.as_ref().unwrap();
    assert_eq!(definition.name, case.export);
    assert_ne!(definition.location.source.unit.0, unit.0);
    assert_eq!(
        definition.location.source.content.0,
        Digest::of_bytes(upstream_auxiliary).0
    );
    assert_eq!(definition.location.source.path, "src/lib.rs");
    let text = std::str::from_utf8(upstream_auxiliary).unwrap();
    assert_eq!(
        &text[definition.location.range.start as usize..definition.location.range.end as usize],
        format!("fn {}", case.function)
    );
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
    assert!(
        db.input(&lrr::db::dependency::InputKey::ExpansionRecord(
            request.fingerprint()
        ))
        .is_none()
    );
    assert!(
        db.input(&lrr::db::dependency::InputKey::DiscoverySeal(
            discovery.key().clone()
        ))
        .is_none()
    );
    assert!(discovery.sealed_expansions().is_err());
    let owned = serde_json::to_value(&error).unwrap();
    drop(run);
    drop(discovery);
    assert_eq!(owned, serde_json::to_value(&error).unwrap());
}
static WORK_SEQUENCE: AtomicU64 = AtomicU64::new(0);
fn allocate_work(base: &std::path::Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    base.join(format!(
        "m016-stock-source-panic-{}-{stamp}-{}",
        std::process::id(),
        WORK_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}
