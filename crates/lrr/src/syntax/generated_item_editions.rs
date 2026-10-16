//! Item-arena edition provenance after a real mixed-edition declarative expansion.
use super::*;
use crate::expand::declarative_test_fixture as fixture;
use crate::{
    expand::{
        generated_items::GeneratedItems,
        identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        record::ExpansionRecord,
    },
    source::snapshot::{CfgSet, SourcePath},
    syntax::{declaration_scan::DeclarationScanLimits, outline::ScopeKey},
};
use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite},
    token_wire::{Edition, Key},
};
use std::sync::Arc;

fn materialized(rules: &str, argument: &str) -> GeneratedItems {
    let input = fixture::input(argument);
    let site = fixture::call_site(&input);
    let context = input
        .spans()
        .iter()
        .find(|s| s.key() == site)
        .unwrap()
        .context;
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest([1; 32]),
        },
        invocation: Digest([3; 32]),
        input_tokens: Digest::of_bytes(&input.encode().unwrap()),
        consumer_compile_env: Digest([4; 32]),
        parent_context: Digest(context.0),
        definition_edition: 2021,
        call_edition: 2024,
        grammar: GrammarSlot::Items,
        engine_version: Digest([6; 32]),
        procedural_epoch: None,
    };
    let rules = DeclarativeMacro::new(fixture::definition(rules, Edition::E2021)).unwrap();
    let (result, _) = rules
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
    let result = result.unwrap();
    assert!(fixture::literal_texts(&result.tokens).is_empty());
    let record =
        Arc::new(ExpansionRecord::from_wire(key.clone(), result.tokens.encode().unwrap()).unwrap());
    GeneratedItems::admit(&key, record, 1).unwrap()
}
fn scan(items: &GeneratedItems) -> crate::syntax::declaration_scan::DeclarationScan {
    let parsed = ParsedFile::from_generated_items(items).unwrap();
    assert!(Arc::ptr_eq(
        parsed.hygiene_facts().unwrap(),
        &items.hygiene_facts()
    ));
    assert_eq!(parsed.profile.edition, 2024);
    assert!(parsed.errors.is_empty());
    parsed
        .scan_declarations(
            SourcePath::new("generated.rs").unwrap(),
            ScopeKey {
                crate_key: Digest([2; 32]),
                module_path: vec![],
                owner: None,
            },
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap()
}
#[test]
fn generated_item_arena_preserves_definition_edition_module_names() {
    let items = materialized("()=>{mod gen {}}", "");
    let scan = scan(&items);
    assert_eq!(scan.modules().len(), 1);
    assert_eq!(scan.modules()[0].name, "gen");
    assert_eq!(scan.modules()[0].name_edition, 2021);
}
#[test]
fn generated_item_arena_keeps_mixed_definition_and_call_token_editions() {
    let items = materialized("($name:ident)=>{mod gen {} mod $name {}}", "r#gen");
    let scan = scan(&items);
    assert_eq!(scan.modules().len(), 2);
    assert_eq!(scan.modules()[0].name, "gen");
    assert_eq!(scan.modules()[0].name_edition, 2021);
    assert_eq!(scan.modules()[1].name, "r#gen");
    assert_eq!(scan.modules()[1].name_edition, 2024);
    assert_ne!(scan.modules()[0].identity, scan.modules()[1].identity);
}
