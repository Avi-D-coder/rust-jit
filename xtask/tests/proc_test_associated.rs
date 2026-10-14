//! M015 procedural Test on an associated method, preserving lexical grammar/ownership.
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
        identity::{GrammarSlot, MacroIdentity, ProceduralKind},
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
        token_wire::{Delimiter, TokenTree},
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
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-attr-non-associated-functions.rs"
);
const STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-attr-non-associated-functions.stderr"
);
const AUXILIARY: &[u8] = include_bytes!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/test-macros.rs"
);
static EVIDENCE_ID: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy)]
enum Case {
    Associated,
}
#[derive(Clone, Copy)]
enum CatalogInstallation {
    Early,
}
impl CatalogInstallation {
    fn label(self) -> &'static str {
        "early-associated"
    }
}
fn component_source(_case: Case) -> String {
    let method = UPSTREAM.find("fn new()").unwrap();
    let omitted = method + UPSTREAM[method..].find("    #[test]\n").unwrap();
    let end = omitted
        + UPSTREAM[omitted..]
            .find("\n}\n\n#[test]\nfn test()")
            .unwrap();
    // Select the exact struct/impl/first method token component. The second
    // method and later root tests/main remain unchanged/imported, unexecuted.
    format!(
        "use lrr_fixture_macros::identity_attr as test;\n{}{}",
        &UPSTREAM[..omitted],
        &UPSTREAM[end..end + 3]
    )
}
// Unwrap only owned invisible transport groups. Retain every visible method
// delimiter, span and token, with the same maximum depth as the wire schema.
fn method_tokens(mut roots: &[TokenTree]) -> Result<&[TokenTree], &'static str> {
    let mut remaining = 256_usize;
    loop {
        let [
            TokenTree::Group {
                delimiter: Delimiter::Invisible,
                children,
                ..
            },
        ] = roots
        else {
            return Ok(roots);
        };
        remaining = remaining
            .checked_sub(1)
            .ok_or("owned invisible method wrapper depth exceeds 256")?;
        roots = children;
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
        let path = base.join(format!(
            "m015-proc-test-associated-{}-{id}",
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
fn unchanged_associated_method_container_and_identity_auxiliary_are_retained() {
    let source = component_source(Case::Associated);
    assert!(source.contains("impl A {\n    #[test]"));
    assert!(source.contains("fn new() -> A {\n        A {}\n    }"));
    assert!(!source.contains("fn recovery_witness") && !source.contains("fn main"));
    assert!(STDERR.contains("the `test` attribute may only be used on a free function"));
    assert!(std::str::from_utf8(AUXILIARY).unwrap().contains(
        "pub fn identity_attr(_: TokenStream, input: TokenStream) -> TokenStream {\n    input\n}"
    ));
}
#[test]
fn associated_proc_test_preserves_impl_grammar_and_member_without_compiler_registration() {
    component(Case::Associated, CatalogInstallation::Early);
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
    let attributes: Vec<_> = scan
        .producers()
        .iter()
        .filter(|p| p.unqualified_attribute.as_deref() == Some("test"))
        .collect();
    assert_eq!(attributes.len(), 1);
    let original_producer = attributes[0].identity;
    let lexical = scan
        .lexical_scopes()
        .iter()
        .find(|scope| scope.identity == attributes[0].lexical_scope)
        .unwrap();
    assert_eq!(lexical.kind, DeclarationScopeKind::AssociatedItems);
    assert_eq!(scan.impls().len(), 1);
    assert_eq!(scan.impls()[0].associated.len(), 1);
    eprintln!(
        "{}",
        serde_json::json!({"retained_associated_scope":lexical.identity.fingerprint().hex(),"retained_producer":original_producer.hex(),"producer_grammar":format!("{:?}",attributes[0].grammar),"retained_impl_identity":scan.impls()[0].identity.hex(),"retained_impl_header_identity":scan.impls()[0].header_identity.hex(),"associated_range":[scan.impls()[0].associated[0].range.start,scan.impls()[0].associated[0].range.end]})
    );
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
    let export_name = "identity_attr";
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
                let expected = "identity_attr";
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
                eprintln!(
                    "{}",
                    serde_json::json!({"actual_request":request.key(),"retained_original_attribute_producer":original_producer.hex(),"request_invocation_matches_retained_producer":request.key().invocation==original_producer})
                );
                assert_eq!(request.key().invocation, original_producer);
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
    let mut records: Vec<_> = discovery.sealed_expansions().unwrap().cloned().collect();
    records.extend(discovery.sealed_declaration_expansions().unwrap());
    let inventory = discovery.impl_header_inventory().unwrap();
    let root_lookup = discovery.lookup("new");
    let publication = run.publish_discovery(&mut db, &mut discovery);
    let observed_catalog = if publication.is_ok() {
        Some(discovery.test_catalog(&mut db))
    } else {
        None
    };
    let observation = serde_json::json!({"catalog_installation":installation.label(),"source":source,"root_unit":unit.hex(),"source_content":view.selection().outline.parse.content.hex(),
        "actual_proc_export":owned_export.identity(),"selected_procedural":selected.map(|key|key.hex()),
        "callbacks":run.unit_invocations(unit),"actual_records":records.iter().map(|record|serde_json::json!({"key":record.key(),"fingerprint":record.key().fingerprint().hex(),"tokens":record.tokens().unwrap().roots()})).collect::<Vec<_>>(),
        "root_new_lookup":format!("{root_lookup:?}"),
        "impl_inventory":inventory.headers().iter().map(|header|serde_json::json!({"identity":header.identity.hex(),"header_identity":header.header_identity.hex(),"scope":format!("{:?}",header.scope),"associated":header.associated.iter().map(|member|serde_json::json!({"range":[member.range.start,member.range.end],"header":member.header})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
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
    assert_eq!(
        records[0].key().grammar,
        GrammarSlot::ImplItems,
        "associated output must retain actual impl-item grammar; do not reclassify as a free item"
    );
    let output = records[0].tokens().unwrap();
    let tokens = method_tokens(output.roots()).unwrap();
    assert!(matches!(tokens.first(), Some(TokenTree::Ident { text, .. }) if text=="fn"));
    assert!(matches!(tokens.get(1), Some(TokenTree::Ident { text, .. }) if text=="new"));
    // The public inventory exposes structural attachment/range/header, not a
    // certified member DefKey or a child-scope name seal.
    assert_eq!(inventory.headers().len(), 1);
    let captured_impl = &inventory.headers()[0];
    assert_eq!(captured_impl.identity, scan.impls()[0].identity);
    assert_eq!(captured_impl.associated.len(), 1);
    assert_eq!(
        captured_impl.associated[0].range,
        scan.impls()[0].associated[0].range
    );
    assert!(
        captured_impl.associated[0]
            .header
            .iter()
            .any(|token| token.text == "new")
    );
    assert!(matches!(
        discovery.lookup("new").unwrap(),
        lrr::frontend::names::NameLookup::Absent
    ));
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
        assert!(tests.select(Some("new"), true).is_empty());
    }
    assert_eq!(run.unit_invocations(unit), 1);
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
    let proof = serde_json::json!({"catalog_installation":installation.label(),"source":source,"root_unit":unit.hex(),"source_content":view.selection().outline.parse.content.hex(),"actual_proc_identity":records[0].key().macro_identity,"actual_record":records[0].key().fingerprint().hex(),"output_digest":records[0].output().hex(),"associated_impl_identity":captured_impl.identity.hex(),"associated_range":[captured_impl.associated[0].range.start,captured_impl.associated[0].range.end],"callbacks":1,"sealed_tests":0,"body_queries":0,"stock_full_fixture_passes":0});
    fs::write(
        work.join("accepted.json"),
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .unwrap();
    eprintln!("{proof}");
}
