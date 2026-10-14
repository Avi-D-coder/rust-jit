//! M015 source-backed private nested-test registration and ordinary glob routes.
//! Fixed Rust2024/test cfg metadata controls; no stock run-pass execution claim.
use lrr::{
    core::{
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::{OutlineKey, QueryKey},
        memo::Database,
    },
    expand::{
        generated_items::{GeneratedHeaderKind, GeneratedItems},
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
    syntax::parse::ParseKey,
};
use std::sync::Arc;

const UPSTREAM: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/issue-52557.rs");

fn generated_source() -> String {
    // Keep the exact upstream prefix at the consumer root, including its
    // inner allow attribute and directives, and the complete unchanged item
    // suffix in the transcriber. Only the declarative envelope is added.
    let split = UPSTREAM.find("mod a {").unwrap();
    let prefix = &UPSTREAM[..split];
    let items = &UPSTREAM[split..];
    format!("{prefix}macro_rules! emit {{ () => {{\n{items}\n}}; }}\nemit!();\n")
}

fn capture(text: &str, unit: Digest) -> (PreparedSourceInputs, PreparedSourceView) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
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
        panic!("exact captured fixture absent")
    };
    (inputs, view)
}

#[test]
fn unchanged_upstream_private_tests_keep_qualified_catalog_and_root_glob_route() {
    assert!(UPSTREAM.contains("//@ run-pass"));
    assert!(UPSTREAM.contains("//@ compile-flags: --test"));
    assert!(UPSTREAM.contains("use a::*;\nuse b::*;"));
    assert!(UPSTREAM.contains("let _: bool = foo();"));
    exercise(UPSTREAM, false);
}

#[test]
fn declarative_generated_private_tests_keep_qualified_catalog_and_root_glob_route() {
    let source = generated_source();
    let split = UPSTREAM.find("mod a {").unwrap();
    assert!(source.starts_with(&UPSTREAM[..split]));
    assert!(source.contains(&UPSTREAM[split..]));
    exercise(&source, true);
}

fn exercise(text: &str, generated: bool) {
    let unit = Digest::of_bytes(if generated {
        b"issue52557 declarative component unit"
    } else {
        b"issue52557 original component unit"
    });
    let (inputs, view) = capture(text, unit);
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Digest::of_bytes(b"issue52557 captured source graph"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"issue52557 effect-free compiler environment"),
        Digest::of_bytes(b"issue52557 explicit declarative engine"),
        Arc::new(CompileInputCatalog::standalone(&ProfileIdentity::current()).unwrap()),
    )
    .unwrap();
    let before = discovery
        .test_catalog(&mut db)
        .expect_err("unsealed catalog must refuse");
    assert_eq!(before.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(
        before.message,
        "expansion input publication requires complete discovery"
    );
    let mut expanded = None;
    let mut sealed = false;
    for _ in 0..8 {
        let step = discovery.poll().unwrap_or_else(|error| panic!(
            "issue52557 selected component typed refusal: {error:?}; generated={generated}; explicitly_expanded={}", expanded.is_some()));
        match step {
            SourceDiscoveryStep::NeedExpansion { request } => {
                assert!(
                    generated && expanded.is_none(),
                    "only the one explicit envelope may expand"
                );
                let record = Arc::new(request.expand().unwrap());
                let items = GeneratedItems::admit(request.key(), record.clone(), 1).unwrap();
                discovery.admit_expansion(record.clone()).unwrap();
                expanded = Some((record, items));
            }
            SourceDiscoveryStep::Sealed(_) => {
                sealed = true;
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!(
                "issue52557 requires real seal: {diagnostic:?}, {blocked:?}; generated={generated}"
            ),
            _ => panic!("issue52557 unexpectedly requests another producer"),
        }
    }
    assert!(sealed, "bounded complete discovery must seal");
    assert_eq!(expanded.is_some(), generated);
    assert_eq!(
        discovery.sealed_expansions().unwrap().count(),
        usize::from(generated)
    );
    discovery.publish(&mut db).unwrap();
    let catalog = discovery.test_catalog(&mut db).unwrap();
    assert!(catalog.seal().is_some());
    assert_eq!(
        catalog
            .cases()
            .iter()
            .map(|case| case.name.as_str())
            .collect::<Vec<_>>(),
        ["b::foo", "b::local_name"]
    );
    assert_ne!(catalog.cases()[0].definition, catalog.cases()[1].definition);
    for case in catalog.cases() {
        assert!(!case.ignored && !case.should_panic && !case.unsupported_signature);
        assert!(case.panic_contains.is_none() && case.attribute_error.is_none());
        let context_input = db
            .input(&InputKey::DeclarationContext(case.definition))
            .unwrap();
        let InputValue::DeclarationContext(context) = context_input.as_ref() else {
            panic!("test declaration context variant differs")
        };
        assert_eq!(context.scope.crate_key, unit);
        assert_eq!(context.scope.module_path, ["b"]);
        assert!(context.scope.owner.is_none());
        let source_input = db
            .input(&InputKey::DefinitionSource(case.definition))
            .unwrap();
        let InputValue::DefinitionSource(source) = source_input.as_ref() else {
            panic!("test source variant differs")
        };
        if let Some((record, items)) = &expanded {
            assert_eq!(case.expansion_ancestry, [record.key().fingerprint()]);
            assert!(
                items
                    .items()
                    .iter()
                    .any(|item| item.kind == GeneratedHeaderKind::Function
                        && item.key.fingerprint() == case.definition
                        && item.name.as_ref().unwrap().text
                            == case.name.rsplit("::").next().unwrap())
            );
            let mut encoded = StableEncoder::new("lrr/v2/generated-syntax-input");
            encoded.record(&record.descriptor());
            assert_eq!(source.content, encoded.finish());
            assert_eq!(source.profile.edition, record.key().call_edition);
        } else {
            assert!(case.expansion_ancestry.is_empty());
            assert_eq!(source, &view.selection().outline.parse);
            assert!(
                view.outline()
                    .items
                    .iter()
                    .any(|item| item.scope.module_path == ["b"]
                        && item.def_key.fingerprint() == case.definition)
            );
        }
    }
    // Private #[test] metadata must not make b::foo publicly glob-importable.
    let NameLookup::Defined(root_foo) = discovery.lookup("foo").unwrap() else {
        panic!("root foo must resolve uniquely to ordinary public a::foo")
    };
    assert_eq!(root_foo.defining_unit, unit);
    assert!(
        catalog
            .cases()
            .iter()
            .all(|case| case.definition != root_foo.identity)
    );
    let context_input = db
        .input(&InputKey::DeclarationContext(root_foo.identity))
        .unwrap();
    let InputValue::DeclarationContext(context) = context_input.as_ref() else {
        panic!("root import source context differs")
    };
    assert_eq!(context.scope.module_path, ["a"]);
    assert_eq!(context.scope.crate_key, unit);
    assert!(context.scope.owner.is_none());
    let root_source_input = db
        .input(&InputKey::DefinitionSource(root_foo.identity))
        .unwrap();
    let InputValue::DefinitionSource(root_source) = root_source_input.as_ref() else {
        panic!("ordinary root route source variant differs")
    };
    if let Some((record, items)) = &expanded {
        assert!(
            items
                .items()
                .iter()
                .any(|item| item.kind == GeneratedHeaderKind::Function
                    && item.key.fingerprint() == root_foo.identity
                    && item.name.as_ref().unwrap().text == "foo")
        );
        let mut encoded = StableEncoder::new("lrr/v2/generated-syntax-input");
        encoded.record(&record.descriptor());
        assert_eq!(root_source.content, encoded.finish());
        assert_eq!(root_source.profile.edition, record.key().call_edition);
    } else {
        assert_eq!(root_source, &view.selection().outline.parse);
        assert!(
            view.outline()
                .items
                .iter()
                .any(|item| item.scope.module_path == ["a"]
                    && item.def_key.declared_name == "foo"
                    && item.def_key.fingerprint() == root_foo.identity)
        );
    }
    assert_eq!(discovery.lookup("local_name").unwrap(), NameLookup::Absent);
    assert_eq!(catalog.select(Some("b::foo"), true).len(), 1);
    assert_eq!(
        catalog.select(Some("b::foo"), true)[0].definition,
        catalog.cases()[0].definition
    );
    assert_eq!(catalog.select(Some("foo"), false).len(), 1);
    assert_eq!(catalog.select(Some("b::"), false).len(), 2);
    assert!(catalog.select(Some("foo"), true).is_empty());
    let ids = catalog
        .cases()
        .iter()
        .map(|case| case.definition)
        .collect::<Vec<_>>();
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
            ids
        );
    }
    assert_eq!(
        discovery.sealed_expansions().unwrap().count(),
        usize::from(generated)
    );
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
    eprintln!(
        "{}",
        serde_json::json!({"fixture":"tests/ui/test-attrs/issue-52557.rs","generated":generated,"catalog":["b::foo","b::local_name"],"root_foo_origin_module":"a","explicit_declarative_expansions":usize::from(generated),"body_queries":0,"stock_full_fixture_passes":0})
    );
}
