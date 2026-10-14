//! M015 compiler Test target eligibility follows actual generated insertion scope.
//! Exact rustc token components with disclosed Rust2024 cold MBE envelopes;
//! pure public discovery only, no stock fixture pass or body execution.
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
        generated_declarations::GeneratedDeclarations,
        identity::GrammarSlot,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::{compile_input_catalog::CompileInputCatalog, names::NameLookup},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        declaration_scan::{DeclarationProducerKind, DeclarationScanLimits, DeclarationScopeKind},
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
const UPSTREAM: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-attr-non-associated-functions.rs"
);
const STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-attr-non-associated-functions.stderr"
);
static EVIDENCE_ID: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy)]
enum Slot {
    Impl,
    Statements,
}
impl Slot {
    fn grammar(self) -> GrammarSlot {
        match self {
            Self::Impl => GrammarSlot::ImplItems,
            Self::Statements => GrammarSlot::Statements,
        }
    }
    fn kind(self) -> DeclarationScopeKind {
        match self {
            Self::Impl => DeclarationScopeKind::AssociatedItems,
            Self::Statements => DeclarationScopeKind::Block,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Impl => "impl-method",
            Self::Statements => "statement-local-function",
        }
    }
}
fn method_component() -> &'static str {
    let start = UPSTREAM.find("    #[test]\n").unwrap();
    let method = UPSTREAM.find("fn new()").unwrap();
    let end = method + UPSTREAM[method..].find("    #[test]\n").unwrap();
    &UPSTREAM[start..end]
}
fn free_function_component() -> &'static str {
    let start = UPSTREAM.find("#[test]\nfn test()").unwrap();
    let end = start + UPSTREAM[start..].find("\nfn main()").unwrap();
    &UPSTREAM[start..end]
}
fn source(slot: Slot) -> String {
    let prefix = &UPSTREAM[..UPSTREAM.find("struct A {}").unwrap()];
    let struct_source =
        &UPSTREAM[UPSTREAM.find("struct A {}").unwrap()..UPSTREAM.find("\n\nimpl A {").unwrap()];
    let (component, caller) = match slot {
        Slot::Impl => (method_component(), "fn cold() { impl A { emit!(); } }"),
        Slot::Statements => (free_function_component(), "fn cold() { emit!(); }"),
    };
    format!(
        "{prefix}{struct_source}\nmacro_rules! emit {{ () => {{\n{component}\n}}; }}\n{caller}\n"
    )
}
fn cfg() -> CfgSet {
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    cfg
}
fn capture(text: &str, unit: Digest) -> (PreparedSourceInputs, PreparedSourceView) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = cfg();
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
        panic!("exact component source absent")
    };
    (inputs, view)
}
fn evidence() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("artifacts/macos-first");
    fs::create_dir_all(&root).unwrap();
    loop {
        let id = EVIDENCE_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("evidence counter exhausted");
        let path = root.join(format!(
            "m015-associated-macro-slot-{}-{id}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return path,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("exclusive evidence directory: {e}"),
        }
    }
}
fn no_bodies(db: &Database) {
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
}
fn same_primary(actual: &Diagnostic, expected: &Diagnostic) {
    assert_eq!(actual.code, expected.code);
    assert_eq!(actual.message, expected.message);
}
#[test]
fn unchanged_method_and_free_function_components_preserve_official_tokens() {
    assert!(method_component().contains("#[test]\n    //~^ ERROR the `test` attribute may only be used on a free function\n    fn new() -> A {\n        A {}\n    }"));
    assert_eq!(
        free_function_component(),
        "#[test]\nfn test() {\n    let _ = A::new();\n}\n"
    );
    assert!(STDERR.contains("the `test` attribute may only be used on a free function"));
    for slot in [Slot::Impl, Slot::Statements] {
        let text = source(slot);
        assert!(text.starts_with(&UPSTREAM[..UPSTREAM.find("struct A {}").unwrap()]));
        assert!(text.contains(match slot {
            Slot::Impl => method_component(),
            Slot::Statements => free_function_component(),
        }));
    }
}
#[test]
fn captured_impl_items_test_method_must_refuse_before_catalog_seal() {
    exercise(Slot::Impl);
}
#[test]
fn captured_statements_local_free_function_can_complete_without_body_demand() {
    exercise(Slot::Statements);
}
fn exercise(slot: Slot) {
    let text = source(slot);
    let unit = Digest::of_bytes(text.as_bytes());
    let (inputs, view) = capture(&text, unit);
    let work = evidence();
    let parent_scan = ParsedFile::parse_declarations(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .scan_declarations(
        view.selection().path.clone(),
        view.outline().root_scope.clone(),
        &cfg(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    let producers: Vec<_> = parent_scan
        .producers()
        .iter()
        .filter(|p| p.kind == DeclarationProducerKind::Macro)
        .collect();
    assert_eq!(producers.len(), 1);
    let original = producers[0];
    assert_eq!(original.grammar, slot.grammar());
    let lexical = parent_scan
        .lexical_scopes()
        .iter()
        .find(|scope| scope.identity == original.lexical_scope)
        .unwrap();
    assert_eq!(lexical.kind, slot.kind());
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Digest::of_bytes(b"associated macro slot captured graph"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"associated macro slot pure compile environment"),
        Digest::of_bytes(b"associated macro slot pinned declarative engine"),
        Arc::new(CompileInputCatalog::standalone(&ProfileIdentity::current()).unwrap()),
    )
    .unwrap();
    assert_eq!(
        discovery.test_catalog(&mut db).unwrap_err().code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    let mut expanded = None;
    let mut primary = None;
    let mut sealed = false;
    for _ in 0..16 {
        let step = match discovery.poll() {
            Ok(step) => step,
            Err(error) => {
                primary = Some(error);
                break;
            }
        };
        match step {
            SourceDiscoveryStep::NeedExpansion { request } => {
                assert!(expanded.is_none(), "exactly one captured MBE demand");
                assert_eq!(request.key().grammar, slot.grammar());
                assert_eq!(request.key().invocation, original.identity);
                let record = Arc::new(request.expand().unwrap());
                assert_eq!(record.key(), request.key());
                let generated = GeneratedDeclarations::admit(
                    request.key(),
                    record.clone(),
                    1,
                    &parent_scan,
                    &cfg(),
                    DeclarationScanLimits::default(),
                )
                .unwrap();
                assert_eq!(generated.scan().lexical_scopes()[0].kind, slot.kind());
                eprintln!(
                    "{}",
                    serde_json::json!({"label":slot.label(),"actual_request":request.key(),"actual_generated_text":generated.text(),"actual_generated_root_kind":format!("{:?}",generated.scan().lexical_scopes()[0].kind),"actual_generated_test_producers":generated.scan().producers().iter().filter(|p|p.unqualified_attribute.as_deref()==Some("test")).map(|p|serde_json::json!({"identity":p.identity.hex(),"lexical_scope":p.lexical_scope.fingerprint().hex(),"grammar":p.grammar})).collect::<Vec<_>>(),"retained_original_producer":original.identity.hex(),"retained_original_scope":format!("{:?}",original.scope)})
                );
                expanded = Some(record.clone());
                if let Err(error) = discovery.admit_expansion(record) {
                    primary = Some(error);
                    break;
                }
            }
            SourceDiscoveryStep::Sealed(_) => {
                sealed = true;
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                eprintln!("actual typed frontier: {blocked:?}");
                primary = Some(diagnostic);
                break;
            }
            _ => panic!("unexpected procedural/effectful prerequisite for pure slot control"),
        }
    }
    let record = expanded.expect("actual captured declarative expansion absent");
    let publication = discovery.publish(&mut db);
    let catalog = discovery.test_catalog(&mut db);
    no_bodies(&db);
    let observation = serde_json::json!({"label":slot.label(),"source":text,"source_content":view.selection().outline.parse.content.hex(),"root_unit":unit.hex(),"actual_record":record.key(),"record_fingerprint":record.key().fingerprint().hex(),"tokens":record.tokens().unwrap().roots(),"original_producer":original.identity.hex(),"retained_lexical_scope":lexical.identity.fingerprint().hex(),"retained_lexical_kind":format!("{:?}",lexical.kind),"sealed":sealed,"primary":primary,"publication_error":publication.as_ref().err(),"catalog":match &catalog {Ok(tests)=>serde_json::json!({"sealed":tests.seal().is_some(),"cases":tests.cases().iter().map(|case|serde_json::json!({"name":case.name,"definition":case.definition.hex(),"ancestry":case.expansion_ancestry})).collect::<Vec<_>>()}),Err(error)=>serde_json::json!({"error":error})},"db_seal_present":db.input(&InputKey::DiscoverySeal(discovery.key().clone())).is_some(),"expansions":discovery.accounted_expansion_count().unwrap(),"body_queries":0,"stock_full_fixture_passes":0});
    fs::write(
        work.join("actual-before-strict-assertions.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
    eprintln!("{observation}");
    assert_eq!(discovery.accounted_expansion_count().unwrap(), 1);
    match slot {
        Slot::Impl => {
            assert!(
                !sealed,
                "associated ImplItems Test must refuse before sealing"
            );
            let error = primary.expect("compiler Test target refusal absent");
            assert_eq!(error.code, "LRR-E-TEST-DISCOVERY");
            assert_eq!(
                error.message,
                "compiler Test attribute requires a free function"
            );
            same_primary(&publication.unwrap_err(), &error);
            same_primary(&catalog.unwrap_err(), &error);
            assert!(
                db.input(&InputKey::DiscoverySeal(discovery.key().clone()))
                    .is_none()
            );
            for _ in 0..3 {
                same_primary(&discovery.poll().err().unwrap(), &error);
                same_primary(&discovery.publish(&mut db).unwrap_err(), &error);
                same_primary(&discovery.test_catalog(&mut db).unwrap_err(), &error);
            }
        }
        Slot::Statements => {
            assert!(sealed);
            assert!(primary.is_none());
            publication.unwrap();
            let tests = catalog.unwrap();
            assert!(tests.seal().is_some());
            tests.require_current(&mut db).unwrap();
            assert!(tests.cases().is_empty());
            assert!(matches!(
                discovery.lookup("test").unwrap(),
                NameLookup::Absent
            ));
            for _ in 0..3 {
                let tests = discovery.test_catalog(&mut db).unwrap();
                tests.require_current(&mut db).unwrap();
                assert!(tests.select(None, false).is_empty());
                assert!(tests.select(Some("test"), true).is_empty());
            }
        }
    }
    no_bodies(&db);
    assert_eq!(discovery.accounted_expansion_count().unwrap(), 1);
    fs::write(
        work.join("accepted.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
}
