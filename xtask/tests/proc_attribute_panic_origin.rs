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
fn rustc_original_attribute_panic_retains_bound_invocation_and_definition() {
    service_fixture::require_current_validation_source();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = allocate_work(&root.join("artifacts/macos-first"));
    fs::create_dir_all(work.join("macro/src")).unwrap();
    let work = fs::canonicalize(work).unwrap();
    let upstream_auxiliary = include_bytes!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/test-macros.rs"
    );
    fs::write(work.join("macro/src/lib.rs"), upstream_auxiliary).unwrap();
    fs::write(work.join("macro/Cargo.toml"),"[package]\nname=\"test_macros\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/custom-attr-panic.rs"
    );
    // The helper must prepare only this macro-host dependency with its original crate alias.
    let manifest = service_fixture::prepare_rustc_macro_sources(
        &work.join("prepared"),
        &work.join("macro"),
        source,
        "test_macros",
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
    .with_declarative_procedural(context.compile_environment, Digest([203; 32]), catalog)
    .unwrap()
    .with_compiler(compiler)
    .unwrap()
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap();
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let SourceDiscoveryStep::NeedAttributeSelection { selection } = discovery.poll().unwrap()
    else {
        panic!("original panic attribute selection absent");
    };
    let request = run
        .prepare_attribute_selection(&mut db, &selection)
        .unwrap();
    let invocation = request.request();
    let call = invocation
        .input
        .spans()
        .iter()
        .find(|span| span.key() == invocation.call_site)
        .unwrap()
        .clone();
    let definition_source = invocation
        .input
        .spans()
        .iter()
        .find(|span| span.key() == invocation.definition_site)
        .unwrap()
        .source
        .clone();
    let source_parse = lrr::syntax::parse::ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let owner = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "foo")
        .unwrap();
    let facts = source_parse
        .attribute_inputs_at(owner.declaration_span, &cfg)
        .unwrap();
    let anchor = lrr::syntax::spans::SourceRange::checked(call.start, call.end).unwrap();
    let expected_meta = facts
        .attributes
        .iter()
        .find(|attribute| attribute.range.contains(anchor))
        .unwrap()
        .range;
    discovery.bind_attribute_request(request.clone()).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request: bound } = discovery.poll().unwrap() else {
        panic!("actual panic request absent");
    };
    assert_eq!(bound.key(), request.key());
    let phase = PhaseIdentity {
        frame: Digest([204; 32]),
        body: BodyKey {
            definition: Digest([205; 32]),
            exact_body_bytes: Digest([206; 32]),
        },
        index: 0,
    };
    let trace = db.trace.events.len();
    let error = run
        .fulfil_discovery(&mut db, &mut discovery, phase, &request, &mut cache)
        .err()
        .expect("unchanged stock helper must panic");
    assert_eq!(error.code, "LRR-E-MACRO-PANIC", "{error}");
    assert!(error.message.contains("panic-attr"), "{error}");
    assert!(error.origin_attachment_failure.is_none(), "{error}");
    let origin = error
        .macro_origin
        .as_ref()
        .expect("actual matched attribute panic lacks owned origin");
    assert_eq!(origin.invocation_identity, request.key().invocation);
    let at = origin.invocation.as_ref().unwrap();
    assert_eq!(at.source.unit.0, call.source.unit.0);
    assert_eq!(at.source.content.0, call.source.content.0);
    assert_eq!(at.source.path, call.source.path);
    assert_eq!(at.range, expected_meta);
    assert!(at.range.contains(anchor));
    assert!(
        source[at.range.start as usize..at.range.end as usize].contains("test_macros::panic_attr")
    );
    let definition = origin.definition.as_ref().unwrap();
    assert_eq!(definition.name, "panic_attr");
    assert_eq!(definition.location.source.unit.0, definition_source.unit.0);
    assert_eq!(
        definition.location.source.content.0,
        definition_source.content.0
    );
    assert_ne!(definition.location.source.unit.0, at.source.unit.0);
    let auxiliary = std::str::from_utf8(upstream_auxiliary).unwrap();
    assert_eq!(
        &auxiliary
            [definition.location.range.start as usize..definition.location.range.end as usize],
        "fn panic_attr"
    );
    assert!(db.trace.events[trace..].iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
    assert!(
        db.input(&lrr::db::dependency::InputKey::ExpansionRecord(
            request.key().fingerprint()
        ))
        .is_none()
    );
    assert!(
        db.input(&lrr::db::dependency::InputKey::DiscoverySeal(
            discovery.key().clone()
        ))
        .is_none()
    );
    let owned = serde_json::to_value(&error).unwrap();
    drop(run);
    drop(discovery);
    drop(request);
    drop(bound);
    assert_eq!(owned, serde_json::to_value(&error).unwrap());
    // Scoped panic/provenance acceptance. No full compiletest diagnostic match.
}
static WORK_SEQUENCE: AtomicU64 = AtomicU64::new(0);
fn allocate_work(base: &std::path::Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    base.join(format!(
        "m016-stock-attribute-panic-{}-{stamp}-{}",
        std::process::id(),
        WORK_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}
