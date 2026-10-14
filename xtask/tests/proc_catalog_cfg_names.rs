//! M015 source-backed cfg/name component candidate; no stock fixture pass claim.
//! Native preparation is explicit, macro-only, and uses the matched server.
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

const NAMES: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/issue-36768.rs");
const DISABLED_CRATE: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/issue-34932.rs");
static EVIDENCE_ID: AtomicU64 = AtomicU64::new(0);

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
            "m015-proc-catalog-cfg-names-{}-{id}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return fs::canonicalize(path).unwrap(),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("exclusive evidence directory: {error}"),
        }
    }
}
fn cfg() -> CfgSet {
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    cfg
}
fn captured(source: &str, unit: Digest) -> (PreparedSourceInputs, PreparedSourceView) {
    let profile = ProfileIdentity::current();
    let cfg = cfg();
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
        panic!("captured fixture absent")
    };
    (inputs, view)
}
fn emitted_source() -> String {
    // Disclosed adaptations outside licensed sources: retain the exact item
    // suffix of issue36768, and put issue34932's exact test suffix in an
    // explicitly cfg-disabled module. Stock crate feature/lint flags remain
    // source evidence, not generated item directives or execution claims.
    let names = &NAMES[NAMES.find("#[test] fn foo()").unwrap()..];
    let disabled = &DISABLED_CRATE[DISABLED_CRATE.find("#[test]").unwrap()..];
    format!(
        "#[lrr_fixture_macros::attribute_tokens(\n{names}\n#[cfg(any())] mod configured_away {{\n{disabled}\n#[lrr_fixture_macros::panic_attribute] #[test] fn must_not_expand() {{ deliberately_undefined(); }}\n}}\n)] fn placeholder() {{}}\n"
    )
}

#[test]
fn rustc_cfg_and_namespace_sources_preserve_exact_item_witnesses() {
    assert!(NAMES.contains("#[test] fn foo() {}\nmod foo {}"));
    assert!(NAMES.contains("#[test] fn core() {}\nextern crate core;"));
    assert!(DISABLED_CRATE.contains("#![cfg(false)]"));
    assert!(DISABLED_CRATE.contains("let () = \"this should not reach type-checking\";"));
    let source = emitted_source();
    assert!(source.contains(&NAMES[NAMES.find("#[test] fn foo()").unwrap()..]));
    assert!(source.contains(&DISABLED_CRATE[DISABLED_CRATE.find("#[test]").unwrap()..]));
    assert!(source.contains("#[cfg(any())] mod configured_away"));
}

#[test]
fn configured_away_original_crate_has_a_real_empty_sealed_catalog() {
    // Stock default2015 source is unchanged. Fixed Rust2024/testcfg capture is
    // a profile adaptation; no stock run-pass/body execution claim is made.
    let unit = Digest::of_bytes(b"closed original issue34932 cfg fixture unit");
    let (inputs, view) = captured(DISABLED_CRATE, unit);
    let retained: Vec<_> = view.outline().items.iter().map(|item| serde_json::json!({
        "name": item.def_key.declared_name,
        "identity": format!("{:?}", item.def_key.fingerprint()),
        "kind": format!("{:?}", item.def_key.item_kind),
        "module": item.scope.module_path,
        "attributes": item.header.attributes.iter().map(|attribute| attribute.name.as_str()).collect::<Vec<_>>()
    })).collect();
    let original_catalog = match lrr::engine::test_catalog::TestCatalog::original(&[view.share()]) {
        Ok(catalog) => {
            serde_json::json!({"names":catalog.cases().iter().map(|case|case.name.as_str()).collect::<Vec<_>>(),"sealed":catalog.seal().is_some()})
        }
        Err(error) => {
            serde_json::json!({"diagnostic_code":error.code,"diagnostic_message":error.message})
        }
    };
    eprintln!(
        "{}",
        serde_json::json!({"witness":"unchanged issue34932; fixed Rust2024/testcfg capture","source_content":format!("{:?}",view.selection().outline.parse.content),"retained":retained,"original_unsealed_catalog_observation":original_catalog})
    );
    // The stock behavior constrains the final active catalog, not the size of
    // an intermediate source inventory. Keep the observed inventory as evidence
    // and require an actual empty catalog after its real seal is published.
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Digest::of_bytes(b"closed fixture graph without macro tools"),
        1,
    )
    .unwrap()
    .with_declarative(
        Digest::of_bytes(b"closed fixture compiler environment without effects"),
        Digest::of_bytes(b"closed fixture declarative engine"),
    )
    .unwrap()
    .with_compiler(Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &ProfileIdentity::current(),
        )
        .unwrap(),
    ))
    .unwrap();
    assert_unsealed_catalog(&discovery, &mut db);
    assert!(
        matches!(discovery.poll().unwrap(), SourceDiscoveryStep::Sealed(_)),
        "a configured-away crate has no active producer; failure must not be substituted with no-match"
    );
    discovery.publish(&mut db).unwrap();
    let catalog = discovery.test_catalog(&mut db).unwrap();
    assert_no_body_demand(&db);
    eprintln!(
        "{}",
        serde_json::json!({"witness":"final public sealed issue34932 catalog","names":catalog.cases().iter().map(|case|case.name.as_str()).collect::<Vec<_>>(),"sealed":catalog.seal().is_some(),"body_signature_thunk_demands":0})
    );
    assert!(
        catalog.cases().is_empty(),
        "cfg(false) crate requires a genuinely empty final sealed catalog"
    );
    assert!(catalog.seal().is_some());
    for _ in 0..3 {
        catalog.require_current(&mut db).unwrap();
        assert!(discovery.test_catalog(&mut db).unwrap().cases().is_empty());
        assert!(catalog.select(Some("dummy"), true).is_empty());
    }
    assert_no_body_demand(&db);
}

#[test]
fn generated_test_names_do_not_collide_with_type_bindings_or_disabled_producers() {
    use lrr::{
        db::dependency::{InputKey, InputValue},
        expand::generated_items::{GeneratedHeaderKind, GeneratedItems},
    };
    let work = evidence();
    let macro_source = root().join("fixtures/macros/session-workspace");
    let source = emitted_source();
    let manifest = service_fixture::prepare_sources(
        &work.join("prepared"),
        &macro_source,
        &[("lib.rs", &source)],
    );
    let plan = lrr::project::tool_manifest::InputPlan::new(
        [work.join("prepared/app"), macro_source]
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
    let (inputs, view) = captured(&source, unit);
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
        request: Digest([231; 32]),
        session: Digest([232; 32]),
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
    .with_declarative_procedural(context.compile_environment, Digest([233; 32]), procedural)
    .unwrap()
    .with_compiler(Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &ProfileIdentity::current(),
        )
        .unwrap(),
    ))
    .unwrap()
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap();
    assert_unsealed_catalog(&discovery, &mut db);
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut origin = None;
    let mut sealed = false;
    for index in 0..16 {
        let step = match discovery.poll() {
            Ok(step) => step,
            Err(error) => {
                // Preserve a real discovery refusal and the actual callback state;
                // observation must not convert this selected component into a pass.
                let record = origin.and_then(|key| db.input(&InputKey::ExpansionRecord(key)));
                let record_observation = match record.as_deref() {
                    Some(InputValue::ExpansionRecord(record)) => {
                        let headers = match GeneratedItems::admit(
                            record.key(),
                            Arc::new(record.clone()),
                            context.epoch,
                        ) {
                            Ok(items) => serde_json::json!({
                                "headers": items.items().iter().map(|item| serde_json::json!({
                                    "kind": format!("{:?}", item.kind),
                                    "name": item.name.as_ref().map(|name| name.text.as_str()),
                                    "identity": item.key.fingerprint().hex(),
                                })).collect::<Vec<_>>()
                            }),
                            Err(admission) => serde_json::json!({
                                "admission_code": admission.code,
                                "admission_message": admission.message,
                            }),
                        };
                        serde_json::json!({
                            "key": record.key().fingerprint().hex(),
                            "output": record.output().hex(),
                            "tokens": record.token_count(),
                            "wire_bytes": record.descriptor().wire_bytes,
                            "generated_items": headers,
                        })
                    }
                    Some(_) => serde_json::json!({"wrong_input_variant": true}),
                    None => serde_json::Value::Null,
                };
                let observation = serde_json::json!({
                    "diagnostic_code": error.code,
                    "diagnostic_message": error.message,
                    "poll_index": index,
                    "original_root_unit": unit.hex(),
                    "original_source_content": view.selection().outline.parse.content.hex(),
                    "bound_origin": origin.map(|key| key.hex()),
                    "actual_unit_invocations": run.unit_invocations(unit),
                    "adopted_record": record_observation,
                    "body_queries": db.trace.events.iter().filter(|event| matches!(
                        event.key,
                        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
                    )).count(),
                    "compiler_catalog": "public CompileInputCatalog::standalone(current profile) installed through with_compiler",
                    "stock_full_fixture_passes": 0,
                });
                fs::write(
                    work.join("discovery-refusal.json"),
                    serde_json::to_vec_pretty(&observation).unwrap(),
                )
                .unwrap();
                eprintln!("{observation}");
                panic!("selected namespace/cfg component discovery refused: {error:?}");
            }
        };
        match step {
            SourceDiscoveryStep::NeedAttributeSelection { selection } => {
                assert!(
                    origin.is_none(),
                    "cfg-disabled failing attribute must not be selected"
                );
                let request = run
                    .prepare_attribute_selection(&mut db, &selection)
                    .unwrap();
                assert_eq!(request.request().export, "attribute_tokens");
                origin = Some(request.key().fingerprint());
                discovery.bind_attribute_request(request.clone()).unwrap();
                let SourceDiscoveryStep::NeedExpansion { request: bound } =
                    discovery.poll().unwrap()
                else {
                    panic!("bound producer absent")
                };
                assert_eq!(bound.key(), request.key());
                let phase = PhaseIdentity {
                    frame: Digest([234; 32]),
                    body: BodyKey {
                        definition: Digest([235; 32]),
                        exact_body_bytes: Digest([236; 32]),
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
                panic!("namespace/cfg discovery requires a real seal: {diagnostic:?}, {blocked:?}")
            }
            _ => panic!("namespace/cfg discovery requested an unexpected producer"),
        }
    }
    assert!(sealed);
    assert_eq!(run.unit_invocations(unit), 1);
    let records: Vec<_> = discovery.sealed_expansions().unwrap().cloned().collect();
    assert_eq!(records.len(), 1);
    assert_eq!(origin, Some(records[0].key().fingerprint()));
    let admitted =
        GeneratedItems::admit(records[0].key(), records[0].clone(), context.epoch).unwrap();
    assert!(
        admitted
            .items()
            .iter()
            .any(|item| item.kind == GeneratedHeaderKind::Module
                && item.name.as_ref().is_some_and(|name| name.text == "foo"))
    );
    assert!(
        admitted
            .items()
            .iter()
            .any(|item| item.kind == GeneratedHeaderKind::ExternCrate)
    );
    for name in ["dummy", "must_not_expand"] {
        assert!(
            admitted
                .items()
                .iter()
                .any(|item| item.kind == GeneratedHeaderKind::Function
                    && item.name.as_ref().is_some_and(|actual| actual.text == name)),
            "disabled witness must physically survive the actual producer output"
        );
    }
    run.publish_discovery(&mut db, &mut discovery).unwrap();
    let catalog = discovery.test_catalog(&mut db).unwrap();
    assert_eq!(
        catalog
            .cases()
            .iter()
            .map(|case| case.name.as_str())
            .collect::<Vec<_>>(),
        ["core", "foo"]
    );
    assert_ne!(catalog.cases()[0].definition, catalog.cases()[1].definition);
    for case in catalog.cases() {
        assert!(!case.ignored && !case.should_panic && !case.unsupported_signature);
        assert!(case.panic_contains.is_none() && case.attribute_error.is_none());
        assert_eq!(case.expansion_ancestry, [origin.unwrap()]);
        let function = admitted
            .items()
            .iter()
            .find(|item| {
                item.kind == GeneratedHeaderKind::Function
                    && item.key.fingerprint() == case.definition
            })
            .unwrap();
        assert_eq!(function.name.as_ref().unwrap().text, case.name);
        assert!(
            admitted
                .items()
                .iter()
                .filter(|item| item.kind != GeneratedHeaderKind::Function)
                .all(|item| item.key.fingerprint() != case.definition),
            "value test identity must remain distinct from module/extern-crate identities"
        );
        assert!(
            matches!(db.input(&InputKey::DeclarationContext(case.definition)).as_deref(), Some(InputValue::DeclarationContext(context)) if context.scope.crate_key == unit && context.scope.module_path.is_empty() && context.scope.owner.is_none())
        );
        assert!(
            matches!(discovery.lookup(&case.name).unwrap(), lrr::frontend::names::NameLookup::Defined(actual) if actual.identity == case.definition)
        );
        assert_eq!(
            catalog.select(Some(&case.name), true)[0].definition,
            case.definition
        );
        let source_input = db
            .input(&InputKey::DefinitionSource(case.definition))
            .unwrap();
        let InputValue::DefinitionSource(source_key) = source_input.as_ref() else {
            panic!("published test definition source variant differs")
        };
        let lrr::db::poll::QueryPoll::Ready(Ok(value)) = db.fetch(
            &lrr::db::provider::SyntaxProvider,
            QueryKey::ParseFile(source_key.clone()),
        ) else {
            panic!("owned published generated syntax must be ready")
        };
        let lrr::db::value::QueryValue::ParseFile(parsed) = value.value.as_ref() else {
            panic!("generated syntax query variant differs")
        };
        let active = parsed.outline(unit, vec![], &cfg()).unwrap();
        assert!(
            !active.items.iter().any(|item| matches!(
                item.def_key.declared_name.as_str(),
                "dummy" | "must_not_expand" | "configured_away"
            )),
            "actual cfg-disabled declarations must be absent from active owned syntax"
        );
    }
    assert!(catalog.select(Some("dummy"), true).is_empty());
    assert!(catalog.select(Some("must_not_expand"), true).is_empty());
    assert!(catalog.select(Some("configured_away"), false).is_empty());
    for _ in 0..3 {
        catalog.require_current(&mut db).unwrap();
        assert_eq!(
            discovery
                .test_catalog(&mut db)
                .unwrap()
                .cases()
                .iter()
                .map(|case| case.definition)
                .collect::<Vec<_>>(),
            catalog
                .cases()
                .iter()
                .map(|case| case.definition)
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(run.unit_invocations(unit), 1);
    assert_no_body_demand(&db);
    fs::write(work.join("component-report.json"), serde_json::to_vec_pretty(&serde_json::json!({"upstream_commit":"ae0fe511b15ab760d7e963b13389d1bc2ab42b18","case":"issue36768 item suffix plus issue34932 disabled-module suffix adaptation and LRR failing-attribute supplement","names":["core","foo"],"macro_invocations":1,"body_queries":0,"guest_execution":false,"stock_full_fixture_passes":0})).unwrap()).unwrap();
}

fn assert_no_body_demand(db: &Database) {
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
}

fn assert_unsealed_catalog(discovery: &SourceDiscovery, db: &mut Database) {
    let error = discovery.test_catalog(db).expect_err(
        "catalog before discovery seal must refuse with its exact typed incomplete outcome",
    );
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(
        error.message,
        "expansion input publication requires complete discovery"
    );
}
