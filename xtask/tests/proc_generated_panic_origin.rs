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

struct Case {
    source: &'static str,
    auxiliary: &'static [u8],
    alias: &'static str,
    export: &'static str,
    function: &'static str,
    edition: u16,
    message: &'static str,
}

#[test]
fn rustc_generated_function_panic_retains_admitted_invocation_definition_and_parent() {
    let case = Case {
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
        message: "not a valid identifier",
    };
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
            "xtask/tests/proc_generated_panic_origin.rs",
            include_bytes!("proc_generated_panic_origin.rs").as_slice(),
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
    let original = case.source;
    let stock_invocation = original
        .lines()
        .find(|line| line.starts_with("proc_macro_panic::"))
        .unwrap()
        .split("//~")
        .next()
        .unwrap()
        .trim_end();
    let adapted = format!(
        "extern crate proc_macro_panic;\nmacro_rules! emitted {{ () => {{ {stock_invocation} }} }}\nemitted!();\nfn main() {{}}\n"
    );
    let source = adapted.as_str();
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

    // The adaptation wraps the unchanged stock invocation in one declarative
    // producer; the helper source remains unchanged. Original directives/stderr remain retained;
    // this scoped LRR origin test does not assert full compiletest parity.
    let mut steps = 0u32;
    let mut parent_key = None;
    let (error, request, expected_call) = loop {
        steps = steps.checked_add(1).unwrap();
        assert!(
            steps <= 32,
            "generated stock panic exceeded bounded discovery"
        );
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => {
                assert!(request.procedural_request().is_none());
                assert!(
                    parent_key.is_none(),
                    "unexpected second declarative producer"
                );
                let record = Arc::new(request.expand().unwrap());
                parent_key = Some(record.key().clone());
                discovery.admit_expansion(record).unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                let native = request
                    .procedural_request()
                    .expect("stock generated request must be procedural");
                assert_eq!(native.export, case.export);
                let expected_call = native
                    .input
                    .spans()
                    .iter()
                    .find(|span| span.key() == native.call_site)
                    .unwrap()
                    .clone();
                assert_eq!(request.parent_record().key(), parent_key.as_ref().unwrap());
                assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-PERMISSION");
                let phase = PhaseIdentity {
                    frame: Digest([222; 32]),
                    body: BodyKey {
                        definition: request.key().invocation,
                        exact_body_bytes: request.key().input_tokens,
                    },
                    index: 1,
                };
                let error = run
                    .fulfil_generated_discovery(
                        &mut db,
                        &mut discovery,
                        phase,
                        &request,
                        &mut cache,
                    )
                    .err()
                    .expect("original stock generated invocation must panic");
                break (error, request.key().clone(), expected_call);
            }
            _ => panic!("generated stock panic did not produce exact expected demand"),
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
    assert_eq!(at.range.start, expected_call.start);
    assert_eq!(at.range.end, expected_call.end);
    assert_eq!(at.source.unit.0, expected_call.source.unit.0);
    assert_eq!(at.source.content.0, expected_call.source.content.0);
    assert_eq!(at.source.path, expected_call.source.path);
    let stock_start = source.find(stock_invocation).unwrap();
    assert!(
        (stock_start..stock_start + stock_invocation.len()).contains(&(at.range.start as usize))
    );
    assert!(at.range.end as usize <= stock_start + stock_invocation.len());
    assert!(at.range.start < at.range.end);
    let parent_key = parent_key.unwrap();
    assert_eq!(origin.parents.len(), 1);
    let parent = &origin.parents[0];
    assert_eq!(parent.expansion, parent_key.fingerprint());
    assert_eq!(parent.invocation_identity, Some(parent_key.invocation));
    let parent_at = parent.invocation.as_ref().unwrap();
    assert_eq!(parent_at.source, at.source);
    let outer = view
        .outline()
        .item_invocations
        .iter()
        .find(|call| call.item_input == parent_key.invocation)
        .unwrap();
    assert_eq!(parent_at.range, outer.range);
    assert_eq!(
        &source[outer.range.start as usize..outer.range.end as usize],
        "emitted!();"
    );
    assert_eq!(run.unit_invocations(unit), 1);
    assert_eq!(
        fs::read(work.join("macro/src/lib.rs")).unwrap(),
        upstream_auxiliary
    );

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
