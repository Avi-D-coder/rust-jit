//! M015 compiler-owned Attribute aliases versus actual source bang aliases.
//! Unchanged rustc alias ambiguity evidence; disclosed Rust2024/test-cfg
//! supplements. Pure public discovery, no consumer compile or fixture pass.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::InputKey,
        key::{OutlineKey, QueryKey},
        memo::Database,
    },
    expand::{
        identity::GrammarSlot,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        declaration_scan::{DeclarationProducerKind, DeclarationScanLimits},
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
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/ambiguous-builtin-attrs.rs"
);
const STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/ambiguous-builtin-attrs.stderr"
);
static EVIDENCE_ID: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy, Debug)]
enum Placement {
    Original,
    Generated,
    Cold,
}
fn source(attribute: &str, placement: Placement, bang: bool) -> String {
    let import = if bang {
        format!(
            "#[macro_export] macro_rules! bang_identity {{ ($($t:tt)*) => {{ $($t)* }}; }}\nuse crate::bang_identity as {attribute};\n"
        )
    } else {
        format!("use core::prelude::rust_2024::test as {attribute};\n")
    };
    let component = format!("#[test]\n#[{attribute}]\nfn test() {{}}\n");
    let body = match placement {
        Placement::Original => component,
        Placement::Generated => {
            format!("macro_rules! emit {{ () => {{ {component} }}; }}\nemit!();\n")
        }
        Placement::Cold => format!("fn cold() {{ {component} }}\n"),
    };
    format!("{import}{body}")
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
            "m015-compiler-inert-alias-{}-{id}",
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

#[test]
fn unchanged_official_alias_ambiguity_evidence() {
    assert!(
        UPSTREAM.contains("use deny as allow;\n#[allow(unused)] //~ ERROR `allow` is ambiguous")
    );
    assert!(STDERR.contains("`allow` is ambiguous"));
    assert!(UPSTREAM.contains("#[test] // OK, shadowed\nfn test() {}"));
    let compiler = CompileInputCatalog::standalone(&ProfileIdentity::current()).unwrap();
    let attrs = compiler.attribute_catalog().unwrap();
    assert_eq!(
        attrs
            .definitions()
            .filter(|d| attrs.select(d).unwrap().unwrap().name() == "test")
            .count(),
        1
    );
}
#[test]
fn original_ignore_compiler_alias_must_refuse() {
    exercise("ignore", Placement::Original, false);
}
#[test]
fn original_should_panic_compiler_alias_must_refuse() {
    exercise("should_panic", Placement::Original, false);
}
#[test]
fn generated_ignore_compiler_alias_must_refuse() {
    exercise("ignore", Placement::Generated, false);
}
#[test]
fn generated_should_panic_compiler_alias_must_refuse() {
    exercise("should_panic", Placement::Generated, false);
}
#[test]
fn cold_ignore_compiler_alias_must_refuse() {
    exercise("ignore", Placement::Cold, false);
}
#[test]
fn cold_should_panic_compiler_alias_must_refuse() {
    exercise("should_panic", Placement::Cold, false);
}
#[test]
fn original_ignore_source_bang_alias_preserves_final_metadata() {
    exercise("ignore", Placement::Original, true);
}
#[test]
fn original_should_panic_source_bang_alias_preserves_final_metadata() {
    exercise("should_panic", Placement::Original, true);
}
fn exercise(attribute: &str, placement: Placement, bang: bool) {
    let text = source(attribute, placement, bang);
    let unit = Digest::of_bytes(text.as_bytes());
    let (inputs, view) = capture(&text, unit);
    let work = evidence();
    let scan = ParsedFile::parse_declarations(
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
    let cold = scan.producers().iter().find(|p| {
        p.kind == DeclarationProducerKind::Attribute
            && p.unqualified_attribute.as_deref() == Some(attribute)
    });
    if matches!(placement, Placement::Cold) {
        assert_eq!(cold.unwrap().grammar, GrammarSlot::Statements);
    }
    let original = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "test")
        .map(|i| i.def_key.fingerprint());
    let compiler = Arc::new(CompileInputCatalog::standalone(&ProfileIdentity::current()).unwrap());
    let attrs = compiler.attribute_catalog().unwrap();
    let definition = attrs
        .definitions()
        .find(|d| attrs.select(d).unwrap().unwrap().name() == "test")
        .unwrap()
        .clone();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Digest::of_bytes(b"compiler inert alias captured graph"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"pure compiler alias environment"),
        Digest::of_bytes(b"pinned compiler alias engine"),
        compiler,
    )
    .unwrap();
    assert_eq!(
        discovery.test_catalog(&mut db).unwrap_err().code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    let mut primary = None;
    let mut sealed = false;
    let mut expanded = None;
    let mut stage = String::new();
    for _ in 0..16 {
        match discovery.poll() {
            Err(error) => {
                stage = "poll_error".into();
                primary = Some(error);
                break;
            }
            Ok(SourceDiscoveryStep::NeedExpansion { request }) => {
                assert!(matches!(placement, Placement::Generated) && expanded.is_none());
                assert_eq!(request.key().grammar, GrammarSlot::Items);
                let retained: Vec<_> = view
                    .outline()
                    .item_invocations
                    .iter()
                    .filter(|call| call.attribute_input.is_none())
                    .collect();
                assert_eq!(retained.len(), 1);
                assert_eq!(request.key().invocation, retained[0].item_input);
                let record = Arc::new(request.expand().unwrap());
                expanded = Some(record.key().clone());
                if let Err(error) = discovery.admit_expansion(record) {
                    stage = "admit_error".into();
                    primary = Some(error);
                    break;
                }
            }
            Ok(SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            }) => {
                stage = format!("Incomplete {blocked:?}");
                primary = Some(diagnostic);
                break;
            }
            Ok(SourceDiscoveryStep::Sealed(_)) => {
                sealed = true;
                stage = "Sealed".into();
                break;
            }
            Ok(SourceDiscoveryStep::NeedAttributeSelection { selection }) => {
                stage = format!("NeedAttributeSelection scope={:?}", selection.scope());
                break;
            }
            Ok(other) => {
                stage = format!("other {:?}", std::mem::discriminant(&other));
                break;
            }
        }
    }
    let publication = discovery.publish(&mut db);
    let catalog = discovery.test_catalog(&mut db);
    no_bodies(&db);
    let observation = serde_json::json!({"source":text,"source_content":view.selection().outline.parse.content.hex(),"placement":format!("{placement:?}"),"attribute":attribute,"bang":bang,"compiler_test_definition":definition.identity.hex(),"compiler_defining_unit":definition.defining_unit.hex(),"compiler_definition_edition":definition.definition_edition,"cold_producer":cold.map(|p|serde_json::json!({"identity":p.identity.hex(),"grammar":p.grammar,"scope":format!("{:?}",p.scope)})),"original_test_identity":original.map(|d|d.hex()),"actual_mbe_record":expanded,"stage":stage,"sealed":sealed,"primary":primary,"publication_error":publication.as_ref().err(),"catalog":match &catalog { Ok(t)=>serde_json::json!({"seal":t.seal().is_some(),"cases":t.cases().iter().map(|c|serde_json::json!({"name":c.name,"definition":c.definition.hex(),"ignored":c.ignored,"should_panic":c.should_panic,"attribute_error":c.attribute_error})).collect::<Vec<_>>(),"known":t.select(Some("test"),true).len(),"absent":t.select(Some("absent"),true).len()}),Err(e)=>serde_json::json!({"error":e})},"expansions":discovery.accounted_expansion_count().unwrap(),"procedural_callbacks":0,"body_queries":0,"live_consumer_compilations":0,"stock_full_fixture_passes":0});
    fs::write(
        work.join("actual-before-strict-assertions.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
    eprintln!("{observation}");
    if bang {
        assert!(
            sealed,
            "source FunctionLike alias cannot compete with inert Attribute"
        );
        assert!(primary.is_none());
        publication.unwrap();
        for _ in 0..3 {
            let tests = discovery.test_catalog(&mut db).unwrap();
            tests.require_current(&mut db).unwrap();
            assert!(tests.seal().is_some());
            assert_eq!(tests.cases().len(), 1);
            let c = &tests.cases()[0];
            assert_eq!(c.name, "test");
            assert_eq!(Some(c.definition), original);
            assert!(c.expansion_ancestry.is_empty());
            assert_eq!(c.ignored, attribute == "ignore");
            assert_eq!(c.should_panic, attribute == "should_panic");
            assert!(
                c.panic_contains.is_none()
                    && c.attribute_error.is_none()
                    && !c.unsupported_signature
            );
            assert_eq!(tests.select(Some("test"), true).len(), 1);
            assert_eq!(tests.select(Some("tes"), false).len(), 1);
            assert!(tests.select(Some("absent"), true).is_empty());
            assert!(tests.select(Some("absent"), false).is_empty());
        }
    } else {
        assert!(
            !sealed,
            "compiler Attribute alias ambiguity must precede catalog seal"
        );
        let error = primary
            .expect("compiler Attribute ambiguity absent; typed prerequisite is not success");
        assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
        assert!(publication.is_err() && catalog.is_err());
        assert!(
            db.input(&InputKey::DiscoverySeal(discovery.key().clone()))
                .is_none()
        );
        for _ in 0..3 {
            assert_eq!(discovery.poll().err().unwrap().code, error.code);
            assert!(discovery.publish(&mut db).is_err());
            assert!(discovery.test_catalog(&mut db).is_err());
        }
    }
    assert_eq!(
        discovery.accounted_expansion_count().unwrap(),
        usize::from(matches!(placement, Placement::Generated))
    );
    no_bodies(&db);
    fs::write(
        work.join("accepted.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
}

#[test]
fn original_late_ignore_compiler_alias_must_invalidate_negative_facet() {
    exercise_late_alias(false, "ignore");
}
#[test]
fn original_late_should_panic_compiler_alias_must_invalidate_negative_facet() {
    exercise_late_alias(false, "should_panic");
}
#[test]
fn generated_late_ignore_compiler_alias_must_invalidate_negative_facet() {
    exercise_late_alias(true, "ignore");
}
#[test]
fn generated_late_should_panic_compiler_alias_must_invalidate_negative_facet() {
    exercise_late_alias(true, "should_panic");
}
fn exercise_late_alias(generated: bool, attribute: &str) {
    let component = format!("#[test]\n#[{attribute}]\nfn test() {{}}\n");
    let declaration = format!(
        "macro_rules! add_alias {{ () => {{ use core::prelude::rust_2024::test as {attribute}; }}; }}\n"
    );
    let text = if generated {
        format!(
            "{declaration}macro_rules! emit {{ () => {{ {component} }}; }}\nemit!();\nadd_alias!();\n"
        )
    } else {
        format!("{declaration}{component}add_alias!();\n")
    };
    let unit = Digest::of_bytes(text.as_bytes());
    let (inputs, view) = capture(&text, unit);
    let work = evidence();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Digest::of_bytes(b"actual late compiler alias graph"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"late alias pure captured environment"),
        Digest::of_bytes(b"late alias pinned engine"),
        Arc::new(CompileInputCatalog::standalone(&ProfileIdentity::current()).unwrap()),
    )
    .unwrap();
    assert_eq!(
        discovery.test_catalog(&mut db).unwrap_err().code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    let calls: Vec<_> = view
        .outline()
        .item_invocations
        .iter()
        .filter(|call| call.attribute_input.is_none())
        .collect();
    assert_eq!(calls.len(), 1 + usize::from(generated));
    let mut records = Vec::new();
    let mut primary = None;
    let mut sealed = false;
    let mut stage = String::new();
    for _ in 0..16 {
        match discovery.poll() {
            Err(error) => {
                stage = "poll_error".into();
                primary = Some(error);
                break;
            }
            Ok(SourceDiscoveryStep::NeedExpansion { request }) => {
                assert_eq!(request.key().grammar, GrammarSlot::Items);
                let original = calls
                    .iter()
                    .find(|call| call.item_input == request.key().invocation)
                    .expect("request must match actual original Items invocation");
                assert_eq!(original.scope, view.outline().root_scope);
                let identity = request.key().fingerprint();
                assert!(
                    !records.iter().any(|(key, _)| *key == identity),
                    "no MBE reexecution"
                );
                let record = Arc::new(request.expand().unwrap());
                assert_eq!(record.key(), request.key());
                records.push((identity,serde_json::json!({"request":request.key(),"original_invocation":original.item_input.hex(),"tokens":record.tokens().unwrap().roots()})));
                if let Err(error) = discovery.admit_expansion(record) {
                    stage = "admit_error".into();
                    primary = Some(error);
                    break;
                }
                assert_eq!(
                    discovery.test_catalog(&mut db).unwrap_err().code,
                    "LRR-E-CATALOG-INCOMPLETE"
                );
            }
            Ok(SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            }) => {
                stage = format!("Incomplete {blocked:?}");
                primary = Some(diagnostic);
                break;
            }
            Ok(SourceDiscoveryStep::Sealed(_)) => {
                stage = "Sealed".into();
                sealed = true;
                break;
            }
            Ok(other) => {
                stage = format!("unexpected {:?}", std::mem::discriminant(&other));
                break;
            }
        }
    }
    let publication = discovery.publish(&mut db);
    let catalog = discovery.test_catalog(&mut db);
    no_bodies(&db);
    let observation = serde_json::json!({"source":text,"source_content":view.selection().outline.parse.content.hex(),"generated":generated,"attribute":attribute,"records":records.iter().map(|(_,row)|row).collect::<Vec<_>>(),"stage":stage,"sealed":sealed,"primary":primary,"publication_error":publication.as_ref().err(),"catalog_error":catalog.as_ref().err(),"catalog_sealed":catalog.as_ref().ok().map(|t|t.seal().is_some()),"actual_expansion_count":discovery.accounted_expansion_count().unwrap(),"procedural_callbacks":0,"body_queries":0,"live_consumer_compilations":0,"stock_full_fixture_passes":0});
    fs::write(
        work.join("late-alias-before-strict-assertions.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
    eprintln!("{observation}");
    assert!(
        !sealed,
        "namespace-changing alias output cannot leave a sealed negative facet proof"
    );
    let error = primary.expect("late compiler alias must cause exact ambiguity");
    assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
    assert_eq!(
        records.len(),
        1 + usize::from(generated),
        "real alias and generated Test output must both be observed"
    );
    assert_eq!(
        discovery.accounted_expansion_count().unwrap(),
        records.len()
    );
    assert!(publication.is_err() && catalog.is_err());
    assert!(
        db.input(&InputKey::DiscoverySeal(discovery.key().clone()))
            .is_none()
    );
    for _ in 0..3 {
        assert_eq!(discovery.poll().err().unwrap().code, error.code);
        assert!(discovery.publish(&mut db).is_err());
        assert!(discovery.test_catalog(&mut db).is_err());
    }
    no_bodies(&db);
    fs::write(
        work.join("late-alias-accepted.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
}
