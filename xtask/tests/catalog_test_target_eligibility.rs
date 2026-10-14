//! M015 Test target eligibility versus deferred ordinary signature metadata.
//! Source-backed Rust2024/test-cfg component only; no selected body or stock pass.
use lrr::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
    },
    engine::test_catalog::TestCatalog,
    expand::{
        generated_items::GeneratedItems,
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
        declaration_scan::DeclarationScanLimits,
        parse::{ParseKey, ParsedFile},
    },
};
use std::sync::Arc;
const ASSOCIATED: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-attr-non-associated-functions.rs"
);
const ASSOCIATED_STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-attr-non-associated-functions.stderr"
);
const NON_FUNCTION: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-on-not-fn.rs");
const NON_FUNCTION_STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-on-not-fn.stderr"
);
const SIGNATURES: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-function-signature.rs"
);
const SIGNATURES_STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-function-signature.stderr"
);
fn signature_prefix() -> &'static str {
    // Only this contiguous original prefix is selected for the positive metadata
    // component; the later nested-item regression remains imported/unexecuted.
    &SIGNATURES[..SIGNATURES.find("// Regression test").unwrap()]
}
// Adaptations select exact contiguous source components, retaining original
// annotation/comments, target tokens and lexical container. These components
// do not replace the unchanged whole-source negative controls below.
fn associated_component() -> String {
    let method = ASSOCIATED.find("fn new()").unwrap();
    let omitted = method + ASSOCIATED[method..].find("    #[test]\n").unwrap();
    let tail = omitted
        + ASSOCIATED[omitted..]
            .find("\n}\n\n#[test]\nfn test()")
            .unwrap();
    format!("{}{}", &ASSOCIATED[..omitted], &ASSOCIATED[tail..])
}
fn non_function_component(target: &str) -> String {
    let target_start = NON_FUNCTION.find(target).unwrap();
    let start = NON_FUNCTION[..target_start].rfind("#[test]").unwrap();
    let end = target_start + target.len();
    let directives_end = NON_FUNCTION.find("#[test]").unwrap();
    format!(
        "{}{}\n",
        &NON_FUNCTION[..directives_end],
        &NON_FUNCTION[start..end]
    )
}
fn generated_source(source: &str) -> String {
    let split = source
        .lines()
        .take_while(|line| line.trim_start().starts_with("//@") || line.trim().is_empty())
        .map(|line| line.len() + 1)
        .sum::<usize>();
    let (directives, items) = source.split_at(split);
    format!("{directives}macro_rules! emit {{ () => {{\n{items}\n}}; }}\nemit!();\n")
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
        panic!("exact source absent")
    };
    (inputs, view)
}
fn no_bodies(db: &Database) {
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
}
fn discover(source: &str, generated: bool, label: &str) -> Result<TestCatalog, Diagnostic> {
    let text = if generated {
        generated_source(source)
    } else {
        source.into()
    };
    let unit = Digest::of_bytes(text.as_bytes());
    let (inputs, view) = capture(&text, unit);
    if label.starts_with("focused ") {
        let mut cfg = CfgSet::guest_default();
        cfg.atoms.insert("test".into());
        let parsed = ParsedFile::parse_declarations(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let scan = parsed
            .scan_declarations(
                SourcePath::new("lib.rs").unwrap(),
                view.outline().root_scope.clone(),
                &cfg,
                DeclarationScanLimits::default(),
            )
            .unwrap();
        eprintln!(
            "{}",
            serde_json::json!({"label":label,"component_source":source,
            "component_sha256":Digest::of_bytes(source.as_bytes()).hex(),
            "retained_original_scope_kinds":scan.lexical_scopes().iter().map(|scope|format!("{:?}",scope.kind)).collect::<Vec<_>>(),
            "retained_original_attributes":scan.producers().iter().map(|producer|serde_json::json!({"identity":producer.identity.hex(),"kind":format!("{:?}",producer.kind),"attribute":producer.unqualified_attribute,"grammar":format!("{:?}",producer.grammar),"scope_owner":producer.scope.owner.map(|owner|owner.hex()),"container_count":producer.containers.len()})).collect::<Vec<_>>() })
        );
    }
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Digest::of_bytes(b"closed target eligibility fixture graph"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"pure test target environment"),
        Digest::of_bytes(b"explicit pure declaration engine"),
        Arc::new(CompileInputCatalog::standalone(&ProfileIdentity::current()).unwrap()),
    )
    .unwrap();
    let error = discovery
        .test_catalog(&mut db)
        .expect_err("no early catalog");
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    let mut expanded = Vec::new();
    let result = (|| {
        for _ in 0..32 {
            match discovery.poll()? {
                SourceDiscoveryStep::NeedExpansion { request } => {
                    let record = Arc::new(request.expand()?);
                    GeneratedItems::admit(request.key(), record.clone(), 1)?;
                    expanded.push(record.key().fingerprint());
                    discovery.admit_expansion(record)?;
                }
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                    let record = Arc::new(request.expand()?);
                    expanded.push(record.key().fingerprint());
                    discovery.admit_expansion(record)?;
                }
                SourceDiscoveryStep::Sealed(_) => {
                    discovery.publish(&mut db)?;
                    return discovery.test_catalog(&mut db);
                }
                SourceDiscoveryStep::Incomplete {
                    diagnostic,
                    blocked,
                } => {
                    eprintln!("{label} blocked={blocked:?}");
                    return Err(diagnostic);
                }
                SourceDiscoveryStep::NeedSourceDeriveSelection { .. } => panic!(
                    "{label}: actual prerequisite NeedSourceDeriveSelection; no native authority"
                ),
                SourceDiscoveryStep::NeedDeriveDeclaration { .. } => panic!(
                    "{label}: actual prerequisite NeedDeriveDeclaration; no native authority"
                ),
                SourceDiscoveryStep::NeedAttributeSelection { .. } => panic!(
                    "{label}: actual prerequisite NeedAttributeSelection; no native authority"
                ),
                _ => panic!("{label}: unexpected effectful typed discovery stage"),
            }
        }
        panic!("bounded discovery did not settle {label}");
    })();
    no_bodies(&db);
    let observation = serde_json::json!({"label":label,"generated":generated,"root_unit":unit.hex(),"source_content":view.selection().outline.parse.content.hex(),
    "expanded":expanded.iter().map(|k|k.hex()).collect::<Vec<_>>(),"body_queries":0,"stock_full_fixture_passes":0,
    "actual":match &result {
        Ok(catalog)=>serde_json::json!({"sealed":catalog.seal().is_some(),"cases":catalog.cases().iter().map(|case|serde_json::json!({"name":case.name,"definition":case.definition.hex(),"ancestry":case.expansion_ancestry.iter().map(|k|k.hex()).collect::<Vec<_>>(),"unsupported_signature":case.unsupported_signature,"attribute_error":case.attribute_error.as_ref().map(|e|(&e.code,&e.message))})).collect::<Vec<_>>()}),
        Err(error)=>serde_json::json!({"code":error.code,"message":error.message}),
    }});
    eprintln!("{observation}");
    if label == "ordinary free-signature prefix" {
        let catalog = result.as_ref().unwrap();
        assert!(catalog.seal().is_some());
        catalog.require_current(&mut db).unwrap();
        assert_eq!(expanded.len(), usize::from(generated));
        for case in catalog.cases() {
            if generated {
                assert_eq!(case.expansion_ancestry, expanded);
            } else {
                let source_item = view
                    .outline()
                    .items
                    .iter()
                    .find(|item| item.def_key.declared_name == case.name)
                    .unwrap();
                assert_eq!(case.definition, source_item.def_key.fingerprint());
                assert!(source_item.scope.owner.is_none());
                assert!(case.expansion_ancestry.is_empty());
            }
        }
    }
    no_bodies(&db);
    result
}
fn require_ineligible(source: &str, generated: bool, label: &str) {
    let outcome = discover(source, generated, label);
    let error = match outcome {
        Err(error) => error,
        Ok(_) => {
            panic!("ineligible compiler Test target must not yield a selectable final catalog")
        }
    };
    assert_eq!(
        error.code, "LRR-E-TEST-DISCOVERY",
        "preserve unrelated prerequisite as a failing witness"
    );
    assert!(
        error.message.contains("free function"),
        "target eligibility must be the actual refusal: {error:?}"
    );
}
fn require_signature_metadata(generated: bool) {
    assert!(SIGNATURES_STDERR.contains("functions used as tests can not have any arguments"));
    let catalog = discover(
        signature_prefix(),
        generated,
        "ordinary free-signature prefix",
    )
    .unwrap();
    assert_eq!(
        catalog
            .cases()
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        [
            "bar",
            "baz",
            "const_generic",
            "foo",
            "lifetime_generic",
            "type_generic"
        ]
    );
    assert!(
        catalog
            .cases()
            .iter()
            .all(|case| case.attribute_error.is_none())
    );
    assert_eq!(catalog.select(Some("foo"), true).len(), 1);
    assert_eq!(catalog.select(Some("baz"), true).len(), 1);
    assert!(catalog.select(Some("absent"), true).is_empty());
}
#[test]
fn original_associated_test_targets_refuse_final_catalog() {
    assert_eq!(
        ASSOCIATED_STDERR
            .matches("error: the `test` attribute may only be used on a free function")
            .count(),
        2
    );
    require_ineligible(ASSOCIATED, false, "associated Test targets");
}
#[test]
fn generated_associated_test_targets_refuse_final_catalog() {
    require_ineligible(ASSOCIATED, true, "associated Test targets");
}
#[test]
fn original_non_function_test_targets_refuse_final_catalog() {
    assert!(
        NON_FUNCTION_STDERR.contains("the `test` attribute may only be used on a free function")
    );
    require_ineligible(NON_FUNCTION, false, "non-function Test targets");
}
#[test]
fn generated_non_function_test_targets_refuse_final_catalog() {
    require_ineligible(NON_FUNCTION, true, "non-function Test targets");
}
#[test]
fn original_free_signature_errors_remain_unselected_metadata() {
    require_signature_metadata(false);
}
#[test]
fn generated_free_signature_errors_remain_unselected_metadata() {
    require_signature_metadata(true);
}

#[test]
fn focused_original_single_associated_target_refuses_final_catalog() {
    let source = associated_component();
    assert!(source.contains("impl A {\n    #[test]"));
    assert!(!source.contains("fn recovery_witness"));
    assert!(source.ends_with("fn main() {}\n"));
    require_ineligible(&source, false, "focused single associated Test target");
}
#[test]
fn focused_generated_single_associated_target_refuses_final_catalog() {
    require_ineligible(
        &associated_component(),
        true,
        "focused single associated Test target",
    );
}
#[test]
fn focused_original_module_target_refuses_final_catalog() {
    require_ineligible(
        &non_function_component("mod test {}"),
        false,
        "focused module Test target",
    );
}
#[test]
fn focused_generated_module_target_refuses_final_catalog() {
    require_ineligible(
        &non_function_component("mod test {}"),
        true,
        "focused module Test target",
    );
}
#[test]
fn focused_original_const_target_refuses_final_catalog() {
    require_ineligible(
        &non_function_component("const FOO: i32 = -1_i32;"),
        false,
        "focused const Test target",
    );
}
#[test]
fn focused_generated_const_target_refuses_final_catalog() {
    require_ineligible(
        &non_function_component("const FOO: i32 = -1_i32;"),
        true,
        "focused const Test target",
    );
}
