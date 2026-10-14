//! M015/M016 edition supplement: pure owned expansion/declaration facts only.
//! The original rustc run-pass roots are retained; these component checks do not
//! execute check_async, main, a namespace seal or the full stock fixture.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    expand::{
        generated_declarations::GeneratedDeclarations,
        generated_items::GeneratedItems,
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
    token_wire::{
        ContextRecord, Delimiter, Edition, Key, SourceIdentity, TokenForest, TokenTree,
        Transparency, parse_source,
    },
};
use std::sync::Arc;
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
const ROOT2015: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/edition-keywords-2015-2015.rs"
);
const ROOT2018: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/edition-keywords-2018-2015.rs"
);
const AUX2015: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/auxiliary/edition-kw-macro-2015.rs"
);
fn original_rules(name: &str) -> &str {
    AUX2015
        .split_once(&format!("macro_rules! {name} {{\n"))
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0
}
fn edition(year: u16) -> Edition {
    match year {
        2015 => Edition::E2015,
        2018 => Edition::E2018,
        2021 => Edition::E2021,
        2024 => Edition::E2024,
        _ => panic!("test edition"),
    }
}
fn input(year: u16, text: &str) -> TokenForest {
    if year == 2024 {
        return fixture::input(text);
    }
    let text = format!("fixture!({text});");
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([2; 32]),
        edition: edition(year),
    };
    let forest = parse_source(
        &text,
        SourceIdentity {
            unit: context.defining_unit,
            content: Key(Digest::of_bytes(text.as_bytes()).0),
            path: "unit-2.rs".into(),
        },
        context,
        Key([3; 32]),
    )
    .unwrap();
    let [TokenTree::Group { children, .. }] = forest.roots() else {
        panic!("source root")
    };
    let group = children
        .iter()
        .find(|node| {
            matches!(
                node,
                TokenTree::Group {
                    delimiter: Delimiter::Parenthesis,
                    ..
                }
            )
        })
        .unwrap()
        .clone();
    TokenForest::new(
        vec![group],
        forest.contexts().to_vec(),
        forest.spans().to_vec(),
    )
    .unwrap()
}
fn parent(year: u16, text: &str) -> Arc<lrr::syntax::declaration_scan::DeclarationScan> {
    let mut profile = ProfileIdentity::current();
    profile.edition = year;
    Arc::new(
        ParsedFile::parse_declarations(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            profile,
        )
        .unwrap()
        .scan_declarations(
            SourcePath::new("unit-2.rs").unwrap(),
            ScopeKey {
                crate_key: Digest([2; 32]),
                module_path: vec![],
                owner: None,
            },
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    )
}
fn expand(
    year: u16,
    rules: &str,
    text: &str,
) -> (
    ExpansionKey,
    Arc<ExpansionRecord>,
    Arc<lrr::syntax::declaration_scan::DeclarationScan>,
) {
    let input = input(year, text);
    let site = fixture::call_site(&input);
    let context = input
        .spans()
        .iter()
        .find(|s| s.key() == site)
        .unwrap()
        .context;
    let parent = parent(year, &format!("fixture!({text});"));
    let rules = DeclarativeMacro::new(fixture::definition(rules, Edition::E2015)).unwrap();
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest::of_bytes(&rules.definition().encode().unwrap()),
        },
        invocation: parent.producers()[0].identity,
        input_tokens: Digest::of_bytes(&input.encode().unwrap()),
        consumer_compile_env: Digest([4; 32]),
        parent_context: Digest(context.0),
        definition_edition: 2015,
        call_edition: year,
        grammar: GrammarSlot::Items,
        engine_version: Digest::of_bytes(lrr_expand_adapter::UPSTREAM_COMMIT.as_bytes()),
        procedural_epoch: None,
    };
    let (result, stats) = rules
        .expand_hygienic_with_limits(
            &input,
            site,
            DeclarativeSite {
                expansion: Key(key.fingerprint().0),
                defining_unit: Key([1; 32]),
                definition_edition: Edition::E2015,
            },
            2_000_000,
            32 * 1024 * 1024,
        )
        .unwrap();
    assert_eq!(stats.materializations, 1);
    assert!(fixture::literal_texts(&result.as_ref().unwrap().tokens).is_empty());
    let record = Arc::new(
        ExpansionRecord::from_wire(key.clone(), result.unwrap().tokens.encode().unwrap()).unwrap(),
    );
    (key, record, parent)
}
#[test]
fn unchanged_official_keyword_producer_preserves_2015_function_tokens_in_all_call_editions() {
    assert!(ROOT2015.contains("//@ edition:2015"));
    assert!(ROOT2018.contains("//@ edition:2018"));
    for root in [ROOT2015, ROOT2018] {
        assert!(root.contains("//@ run-pass"));
        assert!(root.contains("//@ aux-build:edition-kw-macro-2015.rs"));
    }
    for year in [2015, 2018, 2021, 2024] {
        for (producer, name) in [
            ("produces_async", "async"),
            ("produces_async_raw", "r#async"),
        ] {
            let (key, record, _) = expand(year, original_rules(producer), "");
            let items = GeneratedItems::admit(&key, record, 1).unwrap();
            assert_eq!(items.items().len(), 1);
            let token = items.items()[0].name.as_ref().unwrap();
            assert_eq!(token.text, name);
            assert_eq!(
                items
                    .tokens()
                    .spans()
                    .iter()
                    .find(|s| s.key().0 == token.span.0)
                    .unwrap()
                    .edition,
                lrr::syntax::token_wire::Edition::E2015
            );
        }
    }
}
#[test]
fn definition_2015_keyword_macro_names_survive_newer_enclosing_call_editions() {
    for year in [2015, 2018, 2021, 2024] {
        for name in ["ordinary", "async", "r#async"] {
            // Explicit LRR adaptation: wrap the unchanged produces_async rule in
            // a generated macro definition carrying a definition2015 name.
            let rules = format!(
                "()=>{{macro_rules! {name} {{{}}}}};",
                original_rules("produces_async")
            );
            let (key, record, parent) = expand(year, &rules, "");
            let output = Arc::new(
                GeneratedDeclarations::admit(
                    &key,
                    record,
                    1,
                    &parent,
                    &CfgSet::guest_default(),
                    DeclarationScanLimits::default(),
                )
                .unwrap_or_else(|e| panic!("definition2015 {name} in caller{year}: {e:?}")),
            );
            assert_eq!(output.scan().parse().profile.edition, year);
            assert_eq!(output.scan().macro_definitions().len(), 1);
            let header = &output.scan().macro_definitions()[0];
            assert_eq!(header.name, name);
            let capture = output.macro_definition(header.identity).unwrap();
            assert_eq!(capture.edition, 2015);
            assert_eq!(
                output
                    .tokens()
                    .spans()
                    .iter()
                    .find(|s| s.key().0 == capture.name_span.0)
                    .unwrap()
                    .edition,
                lrr::syntax::token_wire::Edition::E2015
            );
            GeneratedDeclarationMacroIndex::new(
                output,
                DeclarationMacroParent::Source(Arc::new(
                    DeclarationMacroIndex::new(parent).unwrap(),
                )),
            )
            .unwrap();
        }
    }
}

#[test]
fn unchanged_official_roots_keep_full_cold_declaration_and_producer_inventory() {
    for (year, source, macros, attributes) in [(2015, ROOT2015, 8, 4), (2018, ROOT2018, 6, 2)] {
        let mut profile = ProfileIdentity::current();
        profile.edition = year;
        let parsed = ParsedFile::parse_declarations(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            profile,
        )
        .unwrap();
        assert!(
            parsed.errors.is_empty(),
            "unchanged stock caller{year}: {:?}",
            parsed.errors
        );
        let scan = parsed
            .scan_declarations(
                SourcePath::new("unit-2.rs").unwrap(),
                ScopeKey {
                    crate_key: Digest([2; 32]),
                    module_path: vec![],
                    owner: None,
                },
                &CfgSet::guest_default(),
                DeclarationScanLimits::default(),
            )
            .unwrap();
        assert_eq!(scan.producers().len(), macros + attributes);
        assert_eq!(
            scan.producers()
                .iter()
                .filter(|p| p.kind == lrr::syntax::declaration_scan::DeclarationProducerKind::Macro)
                .count(),
            macros
        );
        assert_eq!(
            scan.producers()
                .iter()
                .filter(
                    |p| p.kind == lrr::syntax::declaration_scan::DeclarationProducerKind::Attribute
                )
                .count(),
            attributes
        );
        assert_eq!(
            scan.modules()
                .iter()
                .map(|m| m.name.as_str())
                .collect::<Vec<_>>(),
            ["one_async", "two_async"]
        );
        assert!(scan.modules().iter().all(|m| m.name_edition == year));
        assert_eq!(scan.macro_definitions().len(), 0);
        assert_eq!(scan.impls().len(), 0);
        // check_async is intentionally inventoried while cold; no executable
        // acceptance or stock run-pass result follows from these observations.
    }
    let auxiliary = parent(2015, AUX2015);
    assert_eq!(auxiliary.macro_definitions().len(), 6);
    assert_eq!(auxiliary.producers().len(), 7);
    assert!(
        auxiliary
            .producers()
            .iter()
            .all(|p| p.kind == lrr::syntax::declaration_scan::DeclarationProducerKind::Attribute)
    );
    assert_eq!(auxiliary.impls().len(), 0);
}
