//! M015 procedural Test on non-function targets, preserving mandatory admission.
//! The native controls require explicit macro-only preparation and the matched server.
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
        generated_items::{GeneratedHeaderKind, GeneratedItems},
        identity::{MacroIdentity, ProceduralKind},
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
};
#[allow(dead_code)]
#[path = "support/macro_service.rs"]
mod service_fixture;

const UPSTREAM: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-on-not-fn.rs");
const STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-on-not-fn.stderr"
);
const AUXILIARY: &[u8] = include_bytes!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/builtin-attrs.rs"
);
static EVIDENCE_ID: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy)]
enum Case {
    Module,
    Constant,
}
#[derive(Clone, Copy)]
enum CatalogInstallation {
    Early,
}
impl CatalogInstallation {
    fn label(self) -> &'static str {
        "early"
    }
}
fn component_source(case: Case) -> String {
    let target = match case {
        Case::Module => "mod test {}",
        Case::Constant => "const FOO: i32 = -1_i32;",
    };
    let target_start = UPSTREAM.find(target).unwrap();
    let start = UPSTREAM[..target_start].rfind("#[test]").unwrap();
    let end = target_start + target.len();
    // Exact original annotated non-function component, with an explicit LRR
    // procedural-import supplement shadowing compiler Test. No target tokens,
    // lexical attachment or unchanged auxiliary output are translated.
    format!("use lrr_fixture_macros::test;\n{}\n", &UPSTREAM[start..end])
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
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("evidence counter exhausted");
        let path = base.join(format!(
            "m015-proc-test-non-function-{}-{id}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return fs::canonicalize(path).unwrap(),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("exclusive evidence directory: {error}"),
        }
    }
}
fn test_cfg() -> CfgSet {
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    cfg
}

#[test]
fn unchanged_non_function_components_and_procedural_auxiliary_are_retained() {
    assert!(UPSTREAM.contains("//@ compile-flags: --test"));
    assert!(STDERR.contains("the `test` attribute may only be used on a free function"));
    assert!(component_source(Case::Module).ends_with("mod test {}\n"));
    assert!(component_source(Case::Constant).ends_with("const FOO: i32 = -1_i32;\n"));
    let auxiliary = std::str::from_utf8(AUXILIARY).unwrap();
    assert!(auxiliary.contains("pub fn test(_: TokenStream, input: TokenStream) -> TokenStream"));
    assert!(auxiliary.contains("\"struct Test;\".parse().unwrap()"));
}
#[test]
fn original_module_proc_test_invokes_once_and_seals_empty_catalog() {
    component(Case::Module, CatalogInstallation::Early);
}
#[test]
fn original_const_proc_test_invokes_once_and_seals_empty_catalog() {
    component(Case::Constant, CatalogInstallation::Early);
}
fn component(case: Case, installation: CatalogInstallation) {
    let work = evidence();
    let macro_source = work.join("macro");
    fs::create_dir_all(macro_source.join("src")).unwrap();
    fs::write(macro_source.join("src/lib.rs"), AUXILIARY).unwrap();
    fs::write(macro_source.join("Cargo.toml"),"[package]\nname=\"lrr_fixture_macros\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    let source = component_source(case);
    let manifest = service_fixture::prepare_sources(
        &work.join("prepared"),
        &macro_source,
        &[("lib.rs", &source)],
    );
    let plan = lrr::project::tool_manifest::InputPlan::new(
        [work.join("prepared/app"), macro_source]
            .into_iter()
            .map(|p| {
                lrr::project::tool_manifest::InputRoot::open(
                    lrr::project::tool_manifest::InputKind::Source,
                    &p,
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
    let cfg = test_cfg();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("main.rs").unwrap();
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
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap()
    .with_compile_environment(Arc::new(environment))
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("reemission source absent")
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let context = DemandContext {
        request: Digest([222; 32]),
        session: Digest([223; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut denied = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, false).unwrap();
    assert_eq!(
        denied.catalog(&mut db, unit).unwrap_err().code,
        "LRR-E-MACRO-PERMISSION"
    );
    assert_eq!(denied.unit_invocations(unit), 0);
    let mut run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
    let catalog = run.catalog(&mut db, unit).unwrap();
    let export_name = "test";
    let owned_export=catalog.exports().find(|export|matches!(export.identity(),MacroIdentity::Procedural{export,..} if export==export_name)).unwrap();
    assert!(matches!(
        owned_export.identity(),
        MacroIdentity::Procedural {
            kind: ProceduralKind::Attribute,
            ..
        }
    ));
    eprintln!(
        "{}",
        serde_json::json!({"catalog_installation":installation.label(),"captured_export":owned_export.identity(),"owned_definition":owned_export.definition().identity.hex(),"defining_unit":owned_export.definition().defining_unit.hex(),"consumer_unit":unit.hex(),"source_content":view.selection().outline.parse.content.hex()})
    );
    let compiler = Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &ProfileIdentity::current(),
        )
        .unwrap(),
    );
    let discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap();
    // Exercise distinct public catalog lifecycles without changing source,
    // alias authority, returned-output or compiler-registration expectations.
    let mut discovery = match installation {
        CatalogInstallation::Early => discovery
            .with_declarative_procedural(
                context.compile_environment,
                Digest([224; 32]),
                catalog.clone(),
            )
            .unwrap()
            .with_compiler(compiler)
            .unwrap(),
    }
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap();
    assert_eq!(
        discovery.test_catalog(&mut db).unwrap_err().code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut selected = None;
    let mut sealed = false;
    for index in 0..16 {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedAttributeSelection { selection } => {
                assert!(
                    selected.is_none(),
                    "exactly one actual procedural selection"
                );
                assert_eq!(selection.scope().crate_key, unit);
                assert!(selection.scope().owner.is_none());
                assert!(selection.scope().module_path.is_empty());
                let request = run
                    .prepare_attribute_selection(&mut db, &selection)
                    .unwrap();
                let expected = "test";
                assert!(
                    matches!(&request.key().macro_identity,MacroIdentity::Procedural{export,kind:ProceduralKind::Attribute,..} if export==expected)
                );
                assert_eq!(request.key().definition_edition, 2015);
                assert_eq!(request.key().call_edition, 2024);
                let call = request
                    .request()
                    .input
                    .spans()
                    .iter()
                    .find(|span| span.key() == request.request().call_site)
                    .unwrap();
                assert_eq!(call.source.unit.0, unit.0);
                assert_eq!(
                    call.source.content.0,
                    view.selection().outline.parse.content.0
                );
                assert_eq!(call.source.path, view.selection().path.as_str());
                selected = Some(request.key().fingerprint());
                discovery.bind_attribute_request(request.clone()).unwrap();
                let SourceDiscoveryStep::NeedExpansion { request: bound } =
                    discovery.poll().unwrap()
                else {
                    panic!("bound Attribute request absent")
                };
                assert_eq!(bound.key(), request.key());
                let phase = PhaseIdentity {
                    frame: Digest([231; 32]),
                    body: BodyKey {
                        definition: Digest([232; 32]),
                        exact_body_bytes: Digest([233; 32]),
                    },
                    index,
                };
                run.fulfil_discovery(&mut db, &mut discovery, phase, &request, &mut cache)
                    .unwrap();
            }
            SourceDiscoveryStep::Sealed(_) => {
                sealed = true;
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                eprintln!(
                    "{}",
                    serde_json::json!({"code":diagnostic.code,"message":diagnostic.message,"blocked":format!("{blocked:?}"),"callbacks":run.unit_invocations(unit),"source":source,"source_content":view.selection().outline.parse.content.hex(),"root_unit":unit.hex(),"actual_selected_procedural":selected.map(|key|key.hex()),"stock_full_fixture_passes":0})
                );
                panic!("actual procedural Test prerequisite: {diagnostic:?}");
            }
            _ => panic!("unexpected effectful procedural Test discovery stage"),
        }
    }
    assert!(sealed);
    let records: Vec<_> = discovery.sealed_expansions().unwrap().cloned().collect();
    let publication = run.publish_discovery(&mut db, &mut discovery);
    let observed_catalog = if publication.is_ok() {
        Some(discovery.test_catalog(&mut db))
    } else {
        None
    };
    let observation = serde_json::json!({"catalog_installation":installation.label(),"source":source,"root_unit":unit.hex(),"source_content":view.selection().outline.parse.content.hex(),
        "actual_proc_export":owned_export.identity(),"selected_procedural":selected.map(|key|key.hex()),
        "callbacks":run.unit_invocations(unit),"actual_records":records.iter().map(|record|serde_json::json!({"key":record.key().fingerprint().hex(),"identity":record.key().macro_identity})).collect::<Vec<_>>(),
        "publication_error":publication.as_ref().err().map(|error|(&error.code,&error.message)),
        "actual_catalog":observed_catalog.as_ref().map(|result|match result{
            Ok(tests)=>serde_json::json!({"sealed":tests.seal().is_some(),"cases":tests.cases().iter().map(|test|serde_json::json!({"name":test.name,"definition":test.definition.hex(),"ancestry":test.expansion_ancestry.iter().map(|key|key.hex()).collect::<Vec<_>>()})).collect::<Vec<_>>()}),
            Err(error)=>serde_json::json!({"code":error.code,"message":error.message})}),
        "original_definitions":view.outline().items.iter().map(|item|serde_json::json!({"name":item.def_key.declared_name,"identity":item.def_key.fingerprint().hex()})).collect::<Vec<_>>(),
        "body_queries":db.trace.events.iter().filter(|event|matches!(event.key,QueryKey::FunctionSignature(_)|QueryKey::BodyStructure(_)|QueryKey::Thunk(_))).count(),"stock_full_fixture_passes":0});
    fs::write(
        work.join("actual-before-strict-assertions.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
    eprintln!("{observation}");
    publication.unwrap();
    assert_eq!(run.unit_invocations(unit), 1);
    assert_eq!(records.len(), 1);
    assert_eq!(Some(records[0].key().fingerprint()), selected);
    let items = GeneratedItems::admit(records[0].key(), records[0].clone(), 1).unwrap();
    assert_eq!(items.items().len(), 1);
    let header = &items.items()[0];
    assert_eq!(header.kind, GeneratedHeaderKind::Struct);
    assert_eq!(header.name.as_ref().unwrap().text, "Test");
    assert!(
        matches!(discovery.lookup(&header.name.as_ref().unwrap().text).unwrap(), lrr::frontend::names::NameLookup::Defined(actual) if actual.identity == header.key.fingerprint())
    );
    for _ in 0..3 {
        let tests = discovery.test_catalog(&mut db).unwrap();
        assert!(tests.seal().is_some());
        tests.require_current(&mut db).unwrap();
        assert!(
            tests.cases().is_empty(),
            "procedural spelling test must not confer compiler registration"
        );
        assert!(tests.select(None, false).is_empty());
        assert!(tests.select(Some("test"), false).is_empty());
        assert!(tests.select(Some("FOO"), true).is_empty());
    }
    assert_eq!(run.unit_invocations(unit), 1);
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
    let proof = serde_json::json!({"catalog_installation":installation.label(),"source":source,"root_unit":unit.hex(),"source_content":view.selection().outline.parse.content.hex(),"actual_proc_identity":records[0].key().macro_identity,"actual_record":records[0].key().fingerprint().hex(),"output_identity":header.key.fingerprint().hex(),"output_kind":format!("{:?}",header.kind),"callbacks":1,"sealed_tests":0,"body_queries":0,"stock_full_fixture_passes":0});
    fs::write(
        work.join("accepted.json"),
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .unwrap();
    eprintln!("{proof}");
}
