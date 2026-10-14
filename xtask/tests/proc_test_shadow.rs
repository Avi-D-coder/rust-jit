//! M015 procedural attributes spelled Test retain explicit procedural authority.
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

const UPSTREAM: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/ambiguous-builtin-attrs-test.rs"
);
const STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/ambiguous-builtin-attrs-test.stderr"
);
const AUXILIARY: &[u8] = include_bytes!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/builtin-attrs.rs"
);
const IDENTITY_SOURCE: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/test-followed-by-attr-macros.rs"
);
const IDENTITY_AUXILIARY: &[u8] = include_bytes!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/test-macros.rs"
);
static EVIDENCE_ID: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy)]
enum Case {
    NamedTest,
    RenamedTest,
}
#[derive(Clone, Copy)]
enum CatalogInstallation {
    Early,
    Late,
}
impl CatalogInstallation {
    fn label(self) -> &'static str {
        match self {
            Self::Early => "early",
            Self::Late => "late",
        }
    }
}
fn component_source(case: Case) -> String {
    match case {
        Case::NamedTest => {
            let start = UPSTREAM.find("#[test] // OK, shadowed").unwrap();
            let end = start + UPSTREAM[start..].find("\n\n#[bench]").unwrap();
            // Exact unchanged Test-shadow function component; omit unrelated
            // bench/unstable test crate/NonExistent body and adapt package alias.
            format!("use lrr_fixture_macros::test;\n{}\n", &UPSTREAM[start..end])
        }
        Case::RenamedTest => {
            // Explicit LRR alias supplement: unchanged identity_attr auxiliary
            // is imported under `test`, with a source-faithful empty function.
            "use lrr_fixture_macros::identity_attr as test;\n#[test] fn alias_target() {}\n".into()
        }
    }
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
        let path = base.join(format!("m015-proc-test-shadow-{}-{id}", std::process::id()));
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
fn official_proc_test_shadow_and_identity_alias_sources_remain_explicit() {
    assert!(UPSTREAM.contains("//@ proc-macro: builtin-attrs.rs"));
    assert!(UPSTREAM.contains("//@ compile-flags:--test"));
    assert!(STDERR.contains("cannot find value `NonExistent`"));
    assert!(!STDERR.contains("requires a free function"));
    let auxiliary = std::str::from_utf8(AUXILIARY).unwrap();
    assert!(auxiliary.contains("pub fn test(_: TokenStream, input: TokenStream) -> TokenStream"));
    assert!(auxiliary.contains("\"struct Test;\".parse().unwrap()"));
    assert!(IDENTITY_SOURCE.contains("#[test_macros::identity_attr]"));
    assert!(std::str::from_utf8(IDENTITY_AUXILIARY).unwrap().contains(
        "pub fn identity_attr(_: TokenStream, input: TokenStream) -> TokenStream {\n    input\n}"
    ));
    let adapted = component_source(Case::NamedTest);
    assert!(adapted.contains("#[test] // OK, shadowed\nfn test() {}"));
    assert!(!adapted.contains("extern crate test"));
    assert!(component_source(Case::RenamedTest).contains("identity_attr as test"));
}
#[test]
fn named_proc_test_runs_once_and_does_not_register_compiler_test() {
    component(Case::NamedTest, CatalogInstallation::Early);
}
#[test]
fn proc_attribute_renamed_to_test_runs_once_without_compiler_registration() {
    component(Case::RenamedTest, CatalogInstallation::Early);
}
#[test]
fn late_catalog_named_proc_test_retains_actual_attribute_authority() {
    component(Case::NamedTest, CatalogInstallation::Late);
}
#[test]
fn late_catalog_proc_attribute_renamed_to_test_retains_actual_authority() {
    component(Case::RenamedTest, CatalogInstallation::Late);
}
fn component(case: Case, installation: CatalogInstallation) {
    let work = evidence();
    let macro_source = work.join("macro");
    fs::create_dir_all(macro_source.join("src")).unwrap();
    fs::write(
        macro_source.join("src/lib.rs"),
        match case {
            Case::NamedTest => AUXILIARY,
            Case::RenamedTest => IDENTITY_AUXILIARY,
        },
    )
    .unwrap();
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
    let export_name = match case {
        Case::NamedTest => "test",
        Case::RenamedTest => "identity_attr",
    };
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
        CatalogInstallation::Late => discovery
            .with_declarative_compiler(context.compile_environment, Digest([224; 32]), compiler)
            .unwrap()
            .with_procedural_catalog(catalog.clone())
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
                let expected = match case {
                    Case::NamedTest => "test",
                    Case::RenamedTest => "identity_attr",
                };
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
    match case {
        Case::NamedTest => {
            assert_eq!(header.kind, GeneratedHeaderKind::Struct);
            assert_eq!(header.name.as_ref().unwrap().text, "Test");
        }
        Case::RenamedTest => {
            assert_eq!(header.kind, GeneratedHeaderKind::Function);
            assert_eq!(header.name.as_ref().unwrap().text, "alias_target");
        }
    }

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
        assert!(tests.select(Some("alias_target"), true).is_empty());
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
