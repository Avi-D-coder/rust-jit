//! M015 pure module-route edition supplement backed by an unchanged official closure.
//! No namespace/catalog seal, native preparation, selected body or stock fixture pass.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    expand::{
        generated_declarations::GeneratedDeclarations,
        identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        record::ExpansionRecord,
    },
    frontend::impl_index::{
        DeclarationMacroIndex, DeclarationMacroParent, GeneratedDeclarationMacroIndex,
    },
    source::snapshot::{CfgSet, SourceBlob, SourcePath},
    syntax::{declaration_scan::DeclarationScanLimits, outline::ScopeKey, parse::ParsedFile},
};
use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite},
    token_wire::{Edition, Key},
};
use std::sync::Arc;
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
const UPSTREAM: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/nested-macro-rules-edition.rs"
);
const AUX2021: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/auxiliary/nested_macro_rules_dep_2021.rs"
);
const AUX2024: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/auxiliary/nested_macro_rules_dep_2024.rs"
);
const STDERR2021: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/nested-macro-rules-edition.e2021.stderr"
);
fn original_inner_rules() -> &'static str {
    AUX2021
        .rsplit_once("macro_rules! macro_inner {")
        .unwrap()
        .1
        .strip_suffix("        }\n    };\n}\n")
        .unwrap()
}
fn adapted_rules(ancestor: &str) -> String {
    // LRR scope supplement: apply the original2021 inner macro's literal `gen`
    // identifier to a module header, and capture one caller2024 child identifier.
    // The original nested macro/fn fixture remains unchanged and unexecuted.
    original_inner_rules()
        .replacen("() =>", "($child:ident) =>", 1)
        .replacen(
            "pub fn gen() {}",
            &format!("pub mod {ancestor} {{ pub mod $child {{}} }}"),
            1,
        )
}
#[test]
fn official_revision_closure_and_explicit_module_adaptation_are_retained() {
    assert!(UPSTREAM.contains("//@ revisions: e2021 e2024"));
    assert!(UPSTREAM.contains("//@[e2024] check-pass"));
    assert!(UPSTREAM.contains("nested_macro_rules_dep_2021.rs"));
    assert!(UPSTREAM.contains("nested_macro_rules_dep_2024.rs"));
    assert!(AUX2021.starts_with("//@ edition: 2021"));
    assert!(AUX2024.starts_with("//@ edition: 2024"));
    assert!(STDERR2021.contains("found reserved keyword `gen`"));
    let adapted = adapted_rules("gen");
    assert_eq!(
        adapted.replacen("($child:ident) =>", "() =>", 1).replacen(
            "pub mod gen { pub mod $child {} }",
            "pub fn gen() {}",
            1
        ),
        original_inner_rules()
    );
}
#[test]
fn ordinary_definition_2021_ancestor_retains_caller_2024_child_route() {
    exercise("ordinary");
}
#[test]
fn keyword_definition_2021_ancestor_retains_caller_2024_child_route() {
    exercise("gen");
}
fn exercise(ancestor: &str) {
    let source = "fixture!(r#gen);";
    let cfg = CfgSet::guest_default();
    let parent = Arc::new(
        ParsedFile::parse_declarations(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap()
        .scan_declarations(
            SourcePath::new("unit-2.rs").unwrap(),
            ScopeKey {
                crate_key: Digest([2; 32]),
                module_path: vec![],
                owner: None,
            },
            &cfg,
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    );
    assert_eq!(parent.producers().len(), 1);
    let input = fixture::input("r#gen");
    let site = fixture::call_site(&input);
    let context = input
        .spans()
        .iter()
        .find(|row| row.key() == site)
        .unwrap()
        .context;
    let rules = DeclarativeMacro::new(fixture::definition(
        &adapted_rules(ancestor),
        Edition::E2021,
    ))
    .unwrap();
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest::of_bytes(&rules.definition().encode().unwrap()),
        },
        invocation: parent.producers()[0].identity,
        input_tokens: Digest::of_bytes(&input.encode().unwrap()),
        consumer_compile_env: Digest::of_bytes(b"pure captured module-edition environment"),
        parent_context: Digest(context.0),
        definition_edition: 2021,
        call_edition: 2024,
        grammar: GrammarSlot::Items,
        engine_version: Digest::of_bytes(lrr_expand_adapter::UPSTREAM_COMMIT.as_bytes()),
        procedural_epoch: None,
    };
    let (actual, stats) = rules
        .expand_hygienic_with_limits(
            &input,
            site,
            DeclarativeSite {
                expansion: Key(key.fingerprint().0),
                defining_unit: Key([1; 32]),
                definition_edition: Edition::E2021,
            },
            2_000_000,
            32 * 1024 * 1024,
        )
        .unwrap();
    assert_eq!(stats.materializations, 1);
    let actual = actual.unwrap();
    assert!(fixture::literal_texts(&actual.tokens).is_empty());
    if ancestor == "gen" {
        let refusal = lrr::expand::scopes::MacroName::parse("gen", 2024)
            .expect_err("bare caller-edition gen is a reserved identifier");
        assert_eq!(refusal.code, "LRR-E-MACRO-SCOPE");
        assert_eq!(refusal.message, "invalid macro identifier or edition");
    }
    assert_eq!(actual.matched_arm, 0);
    let record =
        Arc::new(ExpansionRecord::from_wire(key.clone(), actual.tokens.encode().unwrap()).unwrap());
    let output = Arc::new(
        GeneratedDeclarations::admit(
            &key,
            record,
            1,
            &parent,
            &cfg,
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    );
    let scan = output.scan();
    assert_eq!(scan.parse().profile.edition, 2024);
    assert_eq!(scan.modules().len(), 2);
    assert_eq!(scan.modules()[0].name, ancestor);
    assert_eq!(scan.modules()[0].name_edition, 2021);
    assert_eq!(scan.modules()[1].name, "r#gen");
    assert_eq!(scan.modules()[1].name_edition, 2024);
    assert_eq!(scan.modules()[1].scope.module_path, [ancestor]);
    assert_ne!(scan.modules()[0].identity, scan.modules()[1].identity);
    let identities = scan
        .modules()
        .iter()
        .map(|r| r.identity)
        .collect::<Vec<_>>();
    let index = GeneratedDeclarationMacroIndex::new(
        output,
        DeclarationMacroParent::Source(Arc::new(DeclarationMacroIndex::new(parent).unwrap())),
    )
    .unwrap();
    let bindings = index.module_bindings(1).unwrap_or_else(|error| {
        panic!("valid mixed-edition nested module route refused: ancestor={ancestor}; {error:?}")
    });
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].identity, identities[0]);
    assert_eq!(bindings[1].identity, identities[1]);
    assert_eq!(bindings[0].name.as_str(), ancestor);
    assert_eq!(bindings[1].name.as_str(), "gen");
    assert_eq!(bindings[0].route_unit, Digest([2; 32]));
    assert_eq!(bindings[1].route_unit, Digest([2; 32]));
    assert_eq!(
        bindings[1]
            .route_components
            .iter()
            .map(|n| n.as_str())
            .collect::<Vec<_>>(),
        [ancestor, "gen"]
    );
    assert!(bindings.iter().all(|b| b.pending.is_none()));
    // Inspect owned route facts only. No registry publication, positive child
    // Scope lookup or catalog seal is granted by this component.
}
