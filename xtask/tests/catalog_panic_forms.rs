//! M015 valid name-value expected-panic metadata, backed by unchanged rustc input/output.
//! Pure Rust2024/test-cfg component; stock run-fail bodies are never executed here.
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
    frontend::compile_input_catalog::CompileInputCatalog,
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

const UPSTREAM: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-should-panic-failed-show-span.rs"
);
const STOCK_STDOUT: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-should-panic-failed-show-span.run.stdout"
);
const STOCK_STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-should-panic-failed-show-span.run.stderr"
);

fn generated_source() -> String {
    let split = UPSTREAM.find("#[test]").unwrap();
    let prefix = &UPSTREAM[..split];
    let items = &UPSTREAM[split..];
    // Preserve every directive at the consumer root and the complete source
    // item suffix in one explicit declarative transcriber. No body is selected.
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

fn stock_names() -> Vec<&'static str> {
    STOCK_STDOUT
        .lines()
        .filter_map(|line| {
            line.strip_prefix("test ").and_then(|line| {
                line.split_once(" - should panic ... ")
                    .map(|(name, _)| name)
            })
        })
        .collect()
}

#[test]
fn original_name_value_should_panic_has_exact_final_message_without_attribute_error() {
    assert!(UPSTREAM.contains("//@ edition:2015..2021"));
    assert!(UPSTREAM.contains("//@ run-fail"));
    assert!(UPSTREAM.contains("//@ check-run-results"));
    assert_eq!(UPSTREAM.matches("#[should_panic = \"message\"]").count(), 4);
    assert!(STOCK_STDOUT.contains("panic did not contain expected string"));
    assert!(STOCK_STDOUT.contains("expected substring: \"message\""));
    assert!(STOCK_STDERR.contains("ZOMGWTFBBQ"));
    exercise(UPSTREAM, false);
}

#[test]
fn generated_name_value_should_panic_has_exact_final_message_without_attribute_error() {
    let source = generated_source();
    let split = UPSTREAM.find("#[test]").unwrap();
    assert!(source.starts_with(&UPSTREAM[..split]));
    assert!(source.contains(&UPSTREAM[split..]));
    exercise(&source, true);
}

fn exercise(text: &str, generated: bool) {
    let unit = Digest::of_bytes(if generated {
        b"shouldpanic namevalue generated unit"
    } else {
        b"shouldpanic namevalue original unit"
    });
    let (inputs, view) = capture(text, unit);
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Digest::of_bytes(b"shouldpanic namevalue captured graph"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"shouldpanic namevalue effect-free environment"),
        Digest::of_bytes(b"shouldpanic namevalue explicit declarative engine"),
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
        match discovery.poll().unwrap_or_else(|error| {
            panic!("valid panic-form discovery refusal: {error:?}; generated={generated}")
        }) {
            SourceDiscoveryStep::NeedExpansion { request } => {
                assert!(
                    generated && expanded.is_none(),
                    "only the one envelope may expand"
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
            } => panic!("valid panic forms require real seal: {diagnostic:?}, {blocked:?}"),
            _ => panic!("panic-form fixture unexpectedly requests another producer"),
        }
    }
    assert!(sealed);
    assert_eq!(expanded.is_some(), generated);
    discovery.publish(&mut db).unwrap();
    let catalog = discovery.test_catalog(&mut db).unwrap();
    assert!(catalog.seal().is_some());
    let names = stock_names();
    assert_eq!(names.len(), 6);
    assert_eq!(
        catalog
            .cases()
            .iter()
            .map(|case| case.name.as_str())
            .collect::<Vec<_>>(),
        names
    );
    for case in catalog.cases() {
        let context_input = db
            .input(&InputKey::DeclarationContext(case.definition))
            .unwrap();
        let InputValue::DeclarationContext(context) = context_input.as_ref() else {
            panic!("test declaration context variant differs")
        };
        assert_eq!(context.scope.crate_key, unit);
        assert!(context.scope.module_path.is_empty() && context.scope.owner.is_none());
        let source_input = db
            .input(&InputKey::DefinitionSource(case.definition))
            .unwrap();
        let InputValue::DefinitionSource(source) = source_input.as_ref() else {
            panic!("test definition source variant differs")
        };
        if let Some((record, items)) = &expanded {
            assert_eq!(case.expansion_ancestry, [record.key().fingerprint()]);
            assert!(
                items
                    .items()
                    .iter()
                    .any(|item| item.kind == GeneratedHeaderKind::Function
                        && item.key.fingerprint() == case.definition
                        && item.name.as_ref().unwrap().text == case.name)
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
                    .any(|item| item.def_key.fingerprint() == case.definition
                        && item.def_key.declared_name == case.name)
            );
        }
    }
    let body_queries = db
        .trace
        .events
        .iter()
        .filter(|event| {
            matches!(
                event.key,
                QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
            )
        })
        .count();
    assert_eq!(body_queries, 0);
    // Observe a genuinely sealed catalog before strict interpretation checks.
    // A seal with attribute_error is not successful interpretation of a valid form.
    eprintln!(
        "{}",
        serde_json::json!({
            "fixture":"test-should-panic-failed-show-span.rs", "generated":generated,
            "source_content":view.selection().outline.parse.content.hex(), "sealed":true,
            "body_queries":body_queries, "explicit_declarative_expansions":usize::from(generated),
            "cases":catalog.cases().iter().map(|case|serde_json::json!({
                "name":case.name,"definition":case.definition.hex(),"should_panic":case.should_panic,
                "ignored":case.ignored,"panic_contains":case.panic_contains,
                "attribute_error":case.attribute_error.as_ref().map(|error|serde_json::json!({"code":error.code,"message":error.message}))
            })).collect::<Vec<_>>(), "stock_full_fixture_passes":0
        })
    );
    for case in catalog.cases() {
        assert!(case.should_panic && !case.ignored && !case.unsupported_signature);
        assert!(
            case.attribute_error.is_none(),
            "valid source-backed panic attribute must have no error: {} {:?}",
            case.name,
            case.attribute_error
        );
        let expected = match case.name.as_str() {
            "should_panic_with_any_message" | "should_panic_with_any_message_does_not_panic" => {
                None
            }
            "should_panic_with_message"
            | "should_panic_with_message_does_not_panic"
            | "should_panic_with_substring_panics_with_incorrect_string"
            | "should_panic_with_substring_panics_with_non_string_value" => Some("message"),
            _ => panic!("unexpected case outside complete upstream source"),
        };
        assert_eq!(
            case.panic_contains.as_deref(),
            expected,
            "exact panic substring for {}",
            case.name
        );
    }
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
    assert_eq!(
        discovery.sealed_expansions().unwrap().count(),
        usize::from(generated)
    );
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
}
