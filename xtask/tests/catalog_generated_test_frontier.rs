//! M015 generated compiler Test frontier ownership diagnostics.
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
        declaration_scan::{DeclarationScan, DeclarationScanLimits},
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
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    let parent_scan = ParsedFile::parse_declarations(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .scan_declarations(
        SourcePath::new("lib.rs").unwrap(),
        view.outline().root_scope.clone(),
        &cfg,
        DeclarationScanLimits::default(),
    )
    .unwrap();
    report_scan(label, "captured original wrapper", &parent_scan);
    let mut producer_owners = std::collections::BTreeMap::new();
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
                    let items = GeneratedItems::admit(request.key(), record.clone(), 1)?;
                    // Public GeneratedDeclarations::admit cannot bridge this
                    // ordinary item invocation to the cold scan: preserved V2
                    // exact parent-producer-absent refusal. The coordinator owns
                    // that bridge. Observe actual item captures here and admit
                    // the unchanged record through the real public pipeline.
                    eprintln!(
                        "{}",
                        serde_json::json!({"label":label,
                        "actual_item_reply":record.key().fingerprint().hex(),
                        "request_invocation":request.key().invocation.hex(),
                        "cold_scan_has_exact_request_producer":parent_scan.producers().iter().any(|p|p.identity==request.key().invocation),
                        "captured_reply_contexts":items.tokens().contexts(),
                        "captured_reply_spans":items.tokens().spans(),
                        "materialized_origins":items.origins().iter().map(|origin|serde_json::json!({"start":origin.start,"end":origin.end,"span":origin.span})).collect::<Vec<_>>()})
                    );
                    producer_owners
                        .insert(record.key().fingerprint(), "actual envelope ExpansionKey");
                    for header in items.items() {
                        producer_owners
                            .insert(header.key.fingerprint(), "actual generated item header");
                        eprintln!(
                            "{}",
                            serde_json::json!({"label":label,"actual_item_header":header.key.fingerprint().hex(),"kind":format!("{:?}",header.kind),"name":format!("{:?}",header.name),"range":[header.start,header.end]})
                        );
                    }
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
                    eprintln!(
                        "{}",
                        serde_json::json!({"label":label,"actual_frontier":blocked.iter().map(|(node,needs)|serde_json::json!({"node":node.hex(),"owner":producer_owners.get(node),"needs":needs.iter().map(|key|serde_json::json!({"key":key.hex(),"owner":producer_owners.get(key)})).collect::<Vec<_>>()})).collect::<Vec<_>>(),"namespace_note":"SourceDiscovery exposes typed frontier, not its mutable generated compiler attribute namespace; no lookup authority fabricated"})
                    );
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
    if label == "positive generated free-function" {
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

fn report_scan(label: &str, role: &str, scan: &DeclarationScan) {
    eprintln!(
        "{}",
        serde_json::json!({"label":label,"role":role,"scan":scan.fingerprint().hex(),"generated_origin":scan.generated_origin().map(|key|key.hex()),"scope":scan.scope().fingerprint().hex(),"lexical_scopes":scan.lexical_scopes().iter().map(|scope|serde_json::json!({"identity":scope.identity.fingerprint().hex(),"parent":scope.parent.map(|p|p.fingerprint().hex()),"kind":format!("{:?}",scope.kind),"range":[scope.range.start,scope.range.end]})).collect::<Vec<_>>(),"producers":scan.producers().iter().map(|producer|serde_json::json!({"identity":producer.identity.hex(),"kind":format!("{:?}",producer.kind),"attribute":producer.unqualified_attribute,"scope":producer.scope.fingerprint().hex(),"lexical_scope":producer.lexical_scope.fingerprint().hex(),"grammar":format!("{:?}",producer.grammar),"range":[producer.range.start,producer.range.end],"containers":producer.containers.iter().map(|r|[r.start,r.end]).collect::<Vec<_>>()})).collect::<Vec<_>>()})
    );
}
fn free_function_component() -> String {
    let start = NON_FUNCTION.find("#[test]\nfn real_test()").unwrap();
    let end = start
        + NON_FUNCTION[start..]
            .find("\n\n// make sure it works with cfg test")
            .unwrap();
    let directives_end = NON_FUNCTION.find("#[test]").unwrap();
    format!(
        "{}{}\n",
        &NON_FUNCTION[..directives_end],
        &NON_FUNCTION[start..end]
    )
}
fn prefix_target(source: &str, attribute: &str) -> String {
    // Explicit LRR supplement outside unchanged licensed source. Retain every
    // target/header/body token and its lexical container; prefix its Test only.
    source.replacen("#[test]", &format!("{attribute}\n#[test]"), 1)
}
#[test]
fn generated_free_function_test_seals_with_exact_owned_ancestry() {
    let source = free_function_component();
    let catalog = discover(&source, true, "positive generated free-function").unwrap();
    assert_eq!(catalog.cases().len(), 1);
    assert_eq!(catalog.cases()[0].name, "real_test");
    assert!(catalog.cases()[0].attribute_error.is_none());
    assert_eq!(catalog.select(Some("real_test"), true).len(), 1);
    assert!(catalog.select(Some("absent"), true).is_empty());
}
#[test]
fn generated_single_module_test_reaches_target_refusal() {
    assert!(
        NON_FUNCTION_STDERR.contains("the `test` attribute may only be used on a free function")
    );
    require_ineligible(
        &non_function_component("mod test {}"),
        true,
        "minimal module",
    );
}
#[test]
fn generated_single_const_test_reaches_target_refusal() {
    require_ineligible(
        &non_function_component("const FOO: i32 = -1_i32;"),
        true,
        "minimal const",
    );
}
#[test]
fn generated_single_associated_test_reaches_target_refusal() {
    assert!(ASSOCIATED_STDERR.contains("the `test` attribute may only be used on a free function"));
    require_ineligible(
        &associated_component(),
        true,
        "single associated with surviving free Test",
    );
}
#[test]
fn generated_cfg_disabled_invalid_const_has_no_active_test_producer_and_seals_empty() {
    let source = prefix_target(
        &non_function_component("const FOO: i32 = -1_i32;"),
        "#[cfg(any())]",
    );
    let catalog = discover(&source, true, "cfg disabled const").unwrap();
    assert!(catalog.seal().is_some());
    assert!(catalog.cases().is_empty());
}
#[test]
fn generated_inert_prefixed_free_test_preserves_final_attachment() {
    let source = prefix_target(&free_function_component(), "#[allow(dead_code)]");
    let catalog = discover(&source, true, "inert prefixed free Test").unwrap();
    assert!(catalog.seal().is_some());
    assert_eq!(catalog.cases().len(), 1);
    assert_eq!(catalog.cases()[0].name, "real_test");
    assert!(catalog.cases()[0].attribute_error.is_none());
    assert_eq!(catalog.cases()[0].expansion_ancestry.len(), 1);
}
#[test]
fn generated_inert_prefixed_const_preserves_ineligible_target() {
    let source = prefix_target(
        &non_function_component("const FOO: i32 = -1_i32;"),
        "#[allow(dead_code)]",
    );
    require_ineligible(&source, true, "inert prefixed const Test");
}
