//! M015 built-in inert attributes resolve only the Attribute facet before sealing.
//! The native controls require explicit macro-only preparation and the matched server.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::{OutlineKey, QueryKey},
        memo::Database,
    },
    expand::{
        cache::ExpansionOutputCache,
        generated_items::{GeneratedHeaderKind, GeneratedItems},
        identity::GrammarSlot,
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
    syntax::{
        declaration_scan::{DeclarationScanLimits, DeclarationScopeKind},
        parse::{ParseKey, ParsedFile},
    },
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
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/ambiguous-builtin-attrs.rs"
);
const STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/ambiguous-builtin-attrs.stderr"
);
const BUILTIN_AUXILIARY: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/builtin-attrs.rs"
);
const IDENTITY_SOURCE: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/test-followed-by-attr-macros.rs"
);
const IDENTITY_AUXILIARY: &[u8] = include_bytes!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/test-macros.rs"
);
static EVIDENCE_ID: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy, Debug)]
enum Facet {
    Attribute,
    FunctionLike,
}
fn component_source(attribute: &str, facet: Facet) -> String {
    assert!(matches!(attribute, "ignore" | "should_panic"));
    let function = UPSTREAM
        .split("#[test] // OK, shadowed\n")
        .nth(1)
        .unwrap()
        .split("\n\n#[bench]")
        .next()
        .unwrap();
    let export = match facet {
        Facet::Attribute => "identity_attr",
        Facet::FunctionLike => "identity",
    };
    // LRR supplement: test cfg/Rust2024, legitimate captured alias, an extra
    // inert marker before exact original fn tokens. No decl_macro translation.
    format!(
        "use lrr_fixture_macros::{export} as {attribute};\n#[test]\n#[{attribute}]\n{function}\n"
    )
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
            "m015-proc-inert-attribute-ambiguity-{}-{id}",
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
fn upstream_builtin_ambiguity_and_actual_identity_facets_are_preserved() {
    assert!(UPSTREAM.contains("//@ edition:2018"));
    assert!(UPSTREAM.contains("//@ proc-macro: builtin-attrs.rs"));
    assert!(UPSTREAM.contains("#[repr(C)] //~ ERROR `repr` is ambiguous"));
    assert!(UPSTREAM.contains("use deny as allow;"));
    assert!(STDERR.contains("ambiguous because of a name conflict with a builtin attribute"));
    assert!(BUILTIN_AUXILIARY.contains("#[proc_macro_attribute]\npub fn repr"));
    assert!(IDENTITY_SOURCE.contains("#[test_macros::identity_attr]"));
    let auxiliary = std::str::from_utf8(IDENTITY_AUXILIARY).unwrap();
    assert!(auxiliary.contains("#[proc_macro_attribute]\npub fn identity_attr"));
    assert!(auxiliary.contains("#[proc_macro]\npub fn identity"));
    for attribute in ["ignore", "should_panic"] {
        for facet in [Facet::Attribute, Facet::FunctionLike] {
            let source = component_source(attribute, facet);
            assert!(source.ends_with("fn test() {}\n"));
            assert!(source.contains(&format!("#[{attribute}]")));
        }
    }
}
#[test]
fn attribute_ignore_alias_must_refuse_builtin_ambiguity_before_catalog() {
    component("ignore", Facet::Attribute);
}
#[test]
fn attribute_should_panic_alias_must_refuse_builtin_ambiguity_before_catalog() {
    component("should_panic", Facet::Attribute);
}
#[test]
fn function_like_ignore_alias_does_not_compete_with_builtin_attribute() {
    component("ignore", Facet::FunctionLike);
}
#[test]
fn function_like_should_panic_alias_does_not_compete_with_builtin_attribute() {
    component("should_panic", Facet::FunctionLike);
}
#[test]
fn generated_attribute_ignore_alias_must_refuse_builtin_ambiguity_before_catalog() {
    exercise("ignore", Facet::Attribute, true);
}
#[test]
fn generated_attribute_should_panic_alias_must_refuse_builtin_ambiguity_before_catalog() {
    exercise("should_panic", Facet::Attribute, true);
}
fn component(attribute: &str, facet: Facet) {
    exercise(attribute, facet, false);
}
fn exercise(attribute: &str, facet: Facet, generated: bool) {
    exercise_prefix(attribute, facet, generated, false);
}
#[test]
fn generated_replacing_prefix_runs_once_before_later_inert_alias_ambiguity() {
    exercise_prefix("ignore", Facet::Attribute, true, true);
}
fn exercise_prefix(attribute: &str, facet: Facet, generated: bool, prefix: bool) {
    exercise_scope(attribute, facet, generated, prefix, false);
}
#[test]
fn cold_attribute_ignore_alias_must_refuse_builtin_ambiguity_before_catalog() {
    exercise_scope("ignore", Facet::Attribute, false, false, true);
}
#[test]
fn cold_attribute_should_panic_alias_must_refuse_builtin_ambiguity_before_catalog() {
    exercise_scope("should_panic", Facet::Attribute, false, false, true);
}
#[test]
fn generated_replacing_prefix_removes_later_inert_alias_before_binding() {
    for attribute in ["ignore", "should_panic"] {
        exercise_case(attribute, Facet::Attribute, true, true, false, true);
    }
}
#[test]
fn original_replacing_prefix_removes_later_inert_alias_before_binding() {
    for attribute in ["ignore", "should_panic"] {
        exercise_case(attribute, Facet::Attribute, false, true, false, true);
    }
}
fn exercise_scope(attribute: &str, facet: Facet, generated: bool, prefix: bool, cold: bool) {
    exercise_case(attribute, facet, generated, prefix, cold, false);
}
fn exercise_case(
    attribute: &str,
    facet: Facet,
    generated: bool,
    prefix: bool,
    cold: bool,
    removes: bool,
) {
    let work = evidence();
    let macro_source = work.join("macro");
    fs::create_dir_all(macro_source.join("src")).unwrap();
    fs::write(macro_source.join("src/lib.rs"), IDENTITY_AUXILIARY).unwrap();
    fs::write(macro_source.join("Cargo.toml"),"[package]\nname=\"lrr_fixture_macros\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    let mut source = component_source(attribute, facet);
    if generated {
        let (import, body) = source.split_once('\n').unwrap();
        source = format!("{import}\nmacro_rules! emit {{ () => {{ {body} }}; }}\nemit!();\n");
    }
    if prefix {
        // Ordering supplement outside licensed source: real replacing attr
        // must execute before the later competing ignore/Test chain is visited.
        source = "use lrr_fixture_macros::{identity_attr, identity_attr as ignore};\nmacro_rules! emit { () => { #[identity_attr] #[ignore] #[test] fn test() {} }; }\nemit!();\n".into();
    }
    if removes {
        assert!(prefix && !cold);
        let body = format!(
            "#[empty_attr] #[{attribute}] #[test] fn test() {{}} #[test] fn survivor() {{}}"
        );
        let imports =
            format!("use lrr_fixture_macros::{{empty_attr, identity_attr as {attribute}}};\n");
        source = if generated {
            format!("{imports}macro_rules! emit {{ () => {{ {body} }}; }}\nemit!();\n")
        } else {
            format!("{imports}{body}\n")
        };
    }
    if cold {
        assert!(!generated && !prefix);
        let (import, body) = source.split_once('\n').unwrap();
        source = format!("{import}\nfn cold() {{\n{body}}}\n");
    }
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
    let export_name = match facet {
        Facet::Attribute => "identity_attr",
        Facet::FunctionLike => "identity",
    };
    let owned_export = catalog.exports().find(|entry| matches!(entry.identity(), MacroIdentity::Procedural{export,..} if export == export_name)).unwrap();
    assert!(
        matches!(owned_export.identity(), MacroIdentity::Procedural{kind,..} if *kind == match facet { Facet::Attribute => ProceduralKind::Attribute, Facet::FunctionLike => ProceduralKind::Function })
    );
    let compiler = Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &ProfileIdentity::current(),
        )
        .unwrap(),
    );
    let original = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "test")
        .map(|item| item.def_key.fingerprint());
    let scan = ParsedFile::parse_declarations(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .scan_declarations(
        view.selection().path.clone(),
        view.outline().root_scope.clone(),
        &test_cfg(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    let cold_producer = if cold {
        let producers: Vec<_> = scan
            .producers()
            .iter()
            .filter(|p| p.unqualified_attribute.as_deref() == Some(attribute))
            .collect();
        assert_eq!(producers.len(), 1);
        let producer = producers[0];
        let lexical = scan
            .lexical_scopes()
            .iter()
            .find(|scope| scope.identity == producer.lexical_scope)
            .unwrap();
        assert_eq!(lexical.kind, DeclarationScopeKind::Block);
        assert_eq!(producer.grammar, GrammarSlot::Statements);
        assert!(producer.scope.owner.is_some());
        let tests: Vec<_> = scan
            .producers()
            .iter()
            .filter(|p| p.unqualified_attribute.as_deref() == Some("test"))
            .collect();
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].scope, producer.scope);
        assert_eq!(tests[0].lexical_scope, producer.lexical_scope);
        fs::write(work.join("actual-retained-cold-producers.json"),serde_json::to_vec_pretty(&serde_json::json!({"source":source,"source_unit":unit.hex(),"source_content":view.selection().outline.parse.content.hex(),"attribute":attribute,"producer":producer.identity.hex(),"producer_range":[producer.range.start,producer.range.end],"scope":format!("{:?}",producer.scope),"lexical_scope":producer.lexical_scope.fingerprint().hex(),"grammar":producer.grammar,"retained_test_producer":tests[0].identity.hex()})).unwrap()).unwrap();
        Some(producer)
    } else {
        None
    };
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_declarative_procedural(
        context.compile_environment,
        Digest([224; 32]),
        catalog.clone(),
    )
    .unwrap()
    .with_compiler(compiler)
    .unwrap()
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap();
    assert_eq!(
        discovery.test_catalog(&mut db).unwrap_err().code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut actual_callbacks = Vec::new();
    let mut pending_selection = None;
    let mut parent = None;
    let mut sealed = false;
    let mut primary = None;
    let mut stage = "unpolled".to_owned();
    for _ in 0..16 {
        match discovery.poll() {
            Err(error) => {
                primary = Some(error);
                stage = "error".into();
                break;
            }
            Ok(SourceDiscoveryStep::Incomplete { diagnostic, .. }) => {
                primary = Some(diagnostic);
                stage = "Incomplete".into();
                break;
            }
            Ok(SourceDiscoveryStep::Sealed(_)) => {
                sealed = true;
                stage = "Sealed".into();
                break;
            }
            Ok(SourceDiscoveryStep::NeedExpansion { request }) => {
                assert!(generated && parent.is_none(), "one captured MBE only");
                let invocations: Vec<_> = view
                    .outline()
                    .item_invocations
                    .iter()
                    .filter(|call| call.attribute_input.is_none())
                    .collect();
                assert_eq!(invocations.len(), 1);
                assert_eq!(request.key().invocation, invocations[0].item_input);
                assert_eq!(request.key().grammar, GrammarSlot::Items);
                let record = Arc::new(request.expand().unwrap());
                assert_eq!(record.key(), request.key());
                let output = GeneratedItems::admit(request.key(), record.clone(), 1).unwrap();
                assert_eq!(output.items().len(), if removes { 2 } else { 1 });
                let function = &output.items()[0];
                assert_eq!(function.kind, GeneratedHeaderKind::Function);
                assert_eq!(function.name.as_ref().unwrap().text, "test");
                fs::write(work.join("actual-mbe-parent.json"), serde_json::to_vec_pretty(&serde_json::json!({"key":request.key(),"retained_original_invocation":invocations[0].item_input.hex(),"original_invocation_scope":format!("{:?}",invocations[0].scope),"output":output.text(),"generated_test_identity":function.key.fingerprint().hex(),"source":source})).unwrap()).unwrap();
                parent = Some(record.key().fingerprint());
                if let Err(error) = discovery.admit_expansion(record) {
                    primary = Some(error);
                    stage = "admit_error".into();
                    break;
                }
            }
            Ok(SourceDiscoveryStep::NeedAttributeSelection { selection }) => {
                let prepared = match run.prepare_attribute_selection(&mut db, &selection) {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        primary = Some(error);
                        stage = "prepare_attribute_error".into();
                        break;
                    }
                };
                if let Some(producer) = cold_producer {
                    assert_eq!(selection.scope(), &producer.scope);
                    assert_eq!(prepared.key().invocation, producer.identity);
                    assert_eq!(prepared.key().grammar, producer.grammar);
                    assert_eq!(prepared.key().grammar, GrammarSlot::Statements);
                    assert_eq!(prepared.request().export, "identity_attr");
                }
                let actual = serde_json::json!({"stage":"NeedAttributeSelection","key":prepared.key(),"scope":format!("{:?}",selection.scope()),"export":prepared.request().export,"input_roots":prepared.request().input.roots(),"input_spans":prepared.request().input.spans(),"input_contexts":prepared.request().input.contexts(),"callbacks":run.unit_invocations(unit)});
                fs::write(
                    work.join(format!(
                        "actual-attribute-selection-{}.json",
                        actual_callbacks.len()
                    )),
                    serde_json::to_vec_pretty(&actual).unwrap(),
                )
                .unwrap();
                if !prefix || !actual_callbacks.is_empty() {
                    pending_selection = Some(actual);
                    stage = "NeedAttributeSelection".into();
                    break;
                }
                assert_eq!(
                    prepared.request().export,
                    if removes {
                        "empty_attr"
                    } else {
                        "identity_attr"
                    }
                );
                assert_eq!(prepared.key().grammar, GrammarSlot::Items);
                let input = lrr_expand_adapter::item_output::materialize(&prepared.request().input)
                    .unwrap();
                assert!(
                    input.text.contains(attribute) && input.text.contains("test"),
                    "earlier attribute must receive the later inert/Test annotations"
                );
                discovery.bind_attribute_request(prepared.clone()).unwrap();
                let SourceDiscoveryStep::NeedExpansion { request: bound } =
                    discovery.poll().unwrap()
                else {
                    panic!("bound prefix Attribute request absent");
                };
                assert_eq!(bound.key(), prepared.key());
                let phase = PhaseIdentity {
                    frame: Digest([239; 32]),
                    body: BodyKey {
                        definition: prepared.key().invocation,
                        exact_body_bytes: prepared.key().input_tokens,
                    },
                    index: 1,
                };
                if let Err(error) =
                    run.fulfil_discovery(&mut db, &mut discovery, phase, &prepared, &mut cache)
                {
                    primary = Some(error);
                    stage = "fulfil_attribute_error".into();
                    break;
                }
                let Some(value) =
                    db.input(&InputKey::ExpansionRecord(prepared.key().fingerprint()))
                else {
                    panic!("actual prefix DB record absent");
                };
                let InputValue::ExpansionRecord(record) = value.as_ref() else {
                    panic!("prefix reply not expansionrecord");
                };
                assert_eq!(record.key(), prepared.key());
                let output =
                    GeneratedItems::admit(prepared.key(), Arc::new(record.clone()), 1).unwrap();
                assert_eq!(output.items().len(), usize::from(!removes));
                if !removes {
                    assert_eq!(output.items()[0].kind, GeneratedHeaderKind::Function);
                    assert_eq!(output.items()[0].name.as_ref().unwrap().text, "test");
                    assert!(output.text().contains(attribute) && output.text().contains("test"));
                } else {
                    assert!(output.text().trim().is_empty());
                }
                actual_callbacks.push(prepared.key().fingerprint());
                assert_eq!(run.unit_invocations(unit), 1);
                fs::write(work.join("actual-prefix-output.json"), serde_json::to_vec_pretty(&serde_json::json!({"record":record.key(),"actual_output":output.text(),"actual_output_function_identity":output.items().first().map(|item|item.key.fingerprint().hex()),"input":actual,"callbacks":run.unit_invocations(unit)})).unwrap()).unwrap();
            }
            Ok(SourceDiscoveryStep::NeedGeneratedExpansion { request })
                if prefix && actual_callbacks.is_empty() =>
            {
                assert_eq!(request.parent_record().key().fingerprint(), parent.unwrap());
                let native = request
                    .procedural_request()
                    .expect("actual prefix procedural request");
                assert_eq!(
                    native.export,
                    if removes {
                        "empty_attr"
                    } else {
                        "identity_attr"
                    }
                );
                assert_eq!(request.key().grammar, GrammarSlot::Items);
                let input = lrr_expand_adapter::item_output::materialize(&native.input).unwrap();
                assert!(input.text.contains(attribute) && input.text.contains("test"));
                let phase = PhaseIdentity {
                    frame: Digest([239; 32]),
                    body: BodyKey {
                        definition: request.key().invocation,
                        exact_body_bytes: request.key().input_tokens,
                    },
                    index: 1,
                };
                if let Err(error) = run.fulfil_generated_discovery(
                    &mut db,
                    &mut discovery,
                    phase,
                    &request,
                    &mut cache,
                ) {
                    primary = Some(error);
                    stage = "fulfil_generated_error".into();
                    break;
                }
                let Some(value) = db.input(&InputKey::ExpansionRecord(request.key().fingerprint()))
                else {
                    panic!("actual generated prefix DBrecord absent");
                };
                let InputValue::ExpansionRecord(record) = value.as_ref() else {
                    panic!("generated prefix reply not expansionrecord");
                };
                assert_eq!(record.key(), request.key());
                let output =
                    GeneratedItems::admit(request.key(), Arc::new(record.clone()), 1).unwrap();
                assert_eq!(output.items().len(), usize::from(!removes));
                if !removes {
                    assert_eq!(output.items()[0].kind, GeneratedHeaderKind::Function);
                    assert_eq!(output.items()[0].name.as_ref().unwrap().text, "test");
                    assert!(output.text().contains(attribute) && output.text().contains("test"));
                } else {
                    assert!(output.text().trim().is_empty());
                }
                actual_callbacks.push(request.key().fingerprint());
                assert_eq!(run.unit_invocations(unit), 1);
                fs::write(work.join("actual-prefix-output.json"), serde_json::to_vec_pretty(&serde_json::json!({"stage":"NeedGeneratedExpansion","record":record.key(),"actual_parent_record":request.parent_record().key(),"actual_output":output.text(),"actual_output_function_identity":output.items().first().map(|item|item.key.fingerprint().hex()),"callbacks":run.unit_invocations(unit)})).unwrap()).unwrap();
            }
            Ok(step) => {
                stage = format!("unexpected {:?}", std::mem::discriminant(&step));
                break;
            }
        }
    }
    let publication = run.publish_discovery(&mut db, &mut discovery);
    let tests = discovery.test_catalog(&mut db);
    let observation = serde_json::json!({"attribute":attribute,"facet":format!("{facet:?}"),"source":source,"source_content":view.selection().outline.parse.content.hex(),"root_unit":unit.hex(),"captured_export":owned_export.identity(),"captured_definition":owned_export.definition().identity.hex(),"generated":generated,"cold":cold,"retained_cold_producer":cold_producer.map(|p|p.identity.hex()),"replacing_prefix":prefix,"removes_later_annotation":removes,"actual_prefix_records":actual_callbacks.iter().map(|key|key.hex()).collect::<Vec<_>>(),"actual_pending_selection":pending_selection,"actual_mbe_parent":parent.map(|key|key.hex()),"original_test_identity":original.map(|key|key.hex()),"stage":stage,"primary":primary.as_ref().map(|error|serde_json::json!({"code":error.code,"message":error.message})),"publication_error":publication.as_ref().err().map(|error|serde_json::json!({"code":error.code,"message":error.message})),"actual_catalog":match &tests { Ok(tests)=>serde_json::json!({"seal":tests.seal().is_some(),"cases":tests.cases().iter().map(|test|serde_json::json!({"name":test.name,"definition":test.definition.hex(),"ignored":test.ignored,"should_panic":test.should_panic,"panic_contains":test.panic_contains,"attribute_error":test.attribute_error.as_ref().map(|error|error.code)})).collect::<Vec<_>>(),"known_exact":tests.select(Some("test"),true).len(),"known_substring":tests.select(Some("tes"),false).len(),"absent_exact":tests.select(Some("absent"),true).len(),"absent_substring":tests.select(Some("absent"),false).len()}), Err(error)=>serde_json::json!({"code":error.code,"message":error.message}) },"callbacks":run.unit_invocations(unit),"body_queries":db.trace.events.iter().filter(|event|matches!(event.key,QueryKey::FunctionSignature(_)|QueryKey::BodyStructure(_)|QueryKey::Thunk(_))).count(),"stock_full_fixture_passes":0});
    fs::write(
        work.join("actual-before-strict-assertions.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
    eprintln!("{observation}");
    assert_eq!(run.unit_invocations(unit), u64::from(prefix));
    assert_eq!(actual_callbacks.len(), usize::from(prefix));
    assert_eq!(parent.is_some(), generated);
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
    if removes {
        assert!(
            sealed,
            "earlier removal must retire the later ambiguous annotation"
        );
        assert!(primary.is_none());
        assert!(pending_selection.is_none());
        publication.unwrap();
        for _ in 0..3 {
            let tests = discovery.test_catalog(&mut db).unwrap();
            tests.require_current(&mut db).unwrap();
            assert!(tests.seal().is_some());
            assert_eq!(tests.cases().len(), 1);
            let case = &tests.cases()[0];
            assert_eq!(case.name, "survivor");
            assert!(!case.ignored && !case.should_panic && !case.unsupported_signature);
            assert!(case.attribute_error.is_none() && case.panic_contains.is_none());
            assert!(tests.select(Some("test"), true).is_empty());
            assert!(tests.select(Some("tes"), false).is_empty());
            assert_eq!(tests.select(Some("survivor"), true).len(), 1);
            assert_eq!(tests.select(Some("surv"), false).len(), 1);
            assert!(tests.select(Some("absent"), true).is_empty());
            assert_eq!(!case.expansion_ancestry.is_empty(), generated);
        }
    } else {
        match facet {
            Facet::Attribute => {
                assert!(!sealed, "Attribute-facet collision must not seal a catalog");
                let error = primary.expect("builtin Attribute-facet ambiguity must be explicit");
                assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
                assert!(publication.is_err());
                assert!(tests.is_err());
            }
            Facet::FunctionLike => {
                assert!(
                    sealed,
                    "bang-only export must not compete with an Attribute"
                );
                assert!(primary.is_none());
                publication.unwrap();
                for _ in 0..3 {
                    let tests = discovery.test_catalog(&mut db).unwrap();
                    assert!(tests.seal().is_some());
                    tests.require_current(&mut db).unwrap();
                    assert_eq!(tests.cases().len(), 1);
                    let test = &tests.cases()[0];
                    assert_eq!(test.name, "test");
                    assert_eq!(Some(test.definition), original);
                    assert!(test.expansion_ancestry.is_empty());
                    assert_eq!(test.ignored, attribute == "ignore");
                    assert_eq!(test.should_panic, attribute == "should_panic");
                    assert!(test.panic_contains.is_none());
                    assert!(test.attribute_error.is_none());
                    assert!(!test.unsupported_signature);
                    assert_eq!(tests.select(Some("test"), true).len(), 1);
                    assert_eq!(tests.select(Some("tes"), false).len(), 1);
                    assert!(tests.select(Some("absent"), true).is_empty());
                    assert!(tests.select(Some("absent"), false).is_empty());
                }
            }
        }
    }
    assert_eq!(run.unit_invocations(unit), u64::from(prefix));
    fs::write(
        work.join("accepted.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
}
