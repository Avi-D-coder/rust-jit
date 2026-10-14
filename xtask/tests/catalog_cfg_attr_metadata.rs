//! M015 cfg_attr Test flags and invalid ordinary attribute/signature metadata.
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
    syntax::parse::ParseKey,
};
use std::sync::Arc;
const PANIC_FORMS: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-should-panic-attr.rs"
);
const PANIC_STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-should-panic-attr.stderr"
);
const MALFORMED: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/attributes/malformed-attrs.rs");
const MALFORMED_STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/attributes/malformed-attrs.stderr"
);
const IGNORE_CFG: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/run-make/test-harness/test-ignore-cfg.rs"
);
const RUN_MAKE: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/run-make/test-harness/rmake.rs");
const SIGNATURES: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/test-function-signature.rs"
);
fn selected_metadata_components() -> String {
    // Exact panic source retained in full, exact ignore-name-value component
    // from malformed attrs (its other unstable/ineligible attributes are not
    // selected), exact six free signature prefix. No stock body is executed.
    let ignore = MALFORMED.find("#[ignore = 1]").unwrap();
    let end = ignore + MALFORMED[ignore..].find("\nfn main()").unwrap();
    let signatures_end = SIGNATURES.find("// Regression test").unwrap();
    let cfg = IGNORE_CFG.replacen("#[test]", "#[cfg_attr(test, test)]", 1);
    format!(
        "{PANIC_FORMS}\n#[test]\n{}\n{cfg}\n{}",
        &MALFORMED[ignore..end],
        &SIGNATURES[..signatures_end]
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
    cfg.atoms.insert("ignorecfg".into());
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
    let mut generated_definitions = std::collections::BTreeSet::new();
    let result = (|| {
        for _ in 0..32 {
            match discovery.poll()? {
                SourceDiscoveryStep::NeedExpansion { request } => {
                    let record = Arc::new(request.expand()?);
                    let items = GeneratedItems::admit(request.key(), record.clone(), 1)?;
                    generated_definitions
                        .extend(items.items().iter().map(|item| item.key.fingerprint()));
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
    if let Ok(catalog) = &result {
        assert!(catalog.seal().is_some());
        catalog.require_current(&mut db).unwrap();
        assert_eq!(expanded.len(), usize::from(generated));
        for case in catalog.cases() {
            if generated {
                assert_eq!(case.expansion_ancestry, expanded);
                assert!(generated_definitions.contains(&case.definition));
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

fn require_metadata(generated: bool) {
    assert!(RUN_MAKE.contains(".cfg(\"ignorecfg\")"));
    assert!(RUN_MAKE.contains("shouldignore ... ignored"));
    assert_eq!(
        PANIC_STDERR
            .matches("error[E0539]: malformed `should_panic` attribute input")
            .count(),
        3
    );
    assert_eq!(
        PANIC_STDERR
            .matches("error[E0805]: malformed `should_panic` attribute input")
            .count(),
        1
    );
    assert!(MALFORMED_STDERR.contains("#[ignore = 1]"));
    let source = selected_metadata_components();
    let catalog = discover(&source, generated, "cfg_attr and invalid ordinary metadata").unwrap();
    assert_eq!(catalog.cases().len(), 14);
    let case = |name: &str| {
        catalog
            .cases()
            .iter()
            .find(|case| case.name == name)
            .unwrap()
    };
    for name in ["test2", "test3", "test4", "test5"] {
        let bad = case(name);
        assert!(bad.should_panic);
        assert!(bad.panic_contains.is_none());
        let error = bad.attribute_error.as_ref().unwrap();
        assert_eq!(error.code, "LRR-E-TEST-DISCOVERY");
        assert_eq!(error.message, "invalid expected-panic attribute");
    }
    let bad = case("thing");
    assert!(bad.ignored);
    assert_eq!(
        bad.attribute_error.as_ref().unwrap().code,
        "LRR-E-TEST-DISCOVERY"
    );
    assert_eq!(
        bad.attribute_error.as_ref().unwrap().message,
        "invalid ignore attribute"
    );
    assert!(case("test1").should_panic);
    assert_eq!(case("test1").panic_contains.as_deref(), Some("foo"));
    assert!(case("test1").attribute_error.is_none());
    assert!(case("shouldignore").ignored);
    assert!(!case("shouldnotignore").ignored);
    assert!(!case("shouldnotignore").should_panic);
    assert!(case("shouldnotignore").attribute_error.is_none());
    assert_eq!(catalog.select(Some("shouldnotignore"), true).len(), 1);
    assert_eq!(catalog.select(Some("shouldnot"), false).len(), 1);
    assert!(catalog.select(Some("absent"), true).is_empty());
    assert!(catalog.select(Some("absent"), false).is_empty());
    // Selecting bad metadata is observable, but no runner/body/signature query
    // is invoked; unsupported syntax belongs to that selected test only.
    assert_eq!(catalog.select(Some("thing"), true).len(), 1);
    assert!(
        catalog.select(Some("thing"), true)[0]
            .attribute_error
            .is_some()
    );
    for name in [
        "bar",
        "baz",
        "const_generic",
        "foo",
        "lifetime_generic",
        "type_generic",
    ] {
        assert!(case(name).attribute_error.is_none());
    }
}
#[test]
fn original_cfg_attr_flags_and_bad_metadata_allow_unrelated_and_empty_selection() {
    require_metadata(false);
}
#[test]
fn generated_cfg_attr_flags_and_bad_metadata_allow_unrelated_and_empty_selection() {
    require_metadata(true);
}
#[test]
fn original_inactive_cfg_attr_test_does_not_register_or_set_ignore() {
    let source = IGNORE_CFG.replacen("#[test]", "#[cfg_attr(any(), test)]", 1);
    let catalog = discover(&source, false, "inactive cfg_attr Test").unwrap();
    assert_eq!(catalog.cases().len(), 1);
    assert_eq!(catalog.cases()[0].name, "shouldnotignore");
    assert!(!catalog.cases()[0].ignored);
    assert!(catalog.cases()[0].attribute_error.is_none());
}
#[test]
fn generated_inactive_cfg_attr_test_does_not_register_or_set_ignore() {
    let source = IGNORE_CFG.replacen("#[test]", "#[cfg_attr(any(), test)]", 1);
    let catalog = discover(&source, true, "inactive cfg_attr Test").unwrap();
    assert_eq!(catalog.cases().len(), 1);
    assert_eq!(catalog.cases()[0].name, "shouldnotignore");
    assert!(!catalog.cases()[0].ignored);
    assert!(catalog.cases()[0].attribute_error.is_none());
}
fn require_unknown_transform(generated: bool) {
    // Explicit LRR refusal supplement: an active cfg_attr on the unchanged
    // shouldnotignore target introduces an unresolved mandatory annotation.
    let source = IGNORE_CFG.replacen(
        "fn shouldnotignore()",
        "#[cfg_attr(test, missing_annotation)]\nfn shouldnotignore()",
        1,
    );
    let error = discover(
        &source,
        generated,
        "unknown cfg_attr annotation before filter",
    )
    .expect_err("unknown transformation cannot yield final catalog");
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(
        error.message,
        "fixed point retains unresolved import or macro producers"
    );
    // The catalog cannot exist to apply either known or no-match filtering.
    // This is a completeness refusal control, not language fixture acceptance.
}
#[test]
fn original_unknown_cfg_attr_transform_refuses_catalog_before_any_filter() {
    require_unknown_transform(false);
}
#[test]
fn generated_unknown_cfg_attr_transform_refuses_catalog_before_any_filter() {
    require_unknown_transform(true);
}
