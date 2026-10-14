//! LRR-specific token-edition boundary supplement; no full upstream fixture claim.
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
    token_wire::{Edition, Key, TokenTree},
};
use std::sync::Arc;
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
#[test]
fn definition_edition_module_identifier_is_not_revalidated_in_caller_edition() {
    assert_definition_edition(false);
}
#[test]
fn relocated_module_identifier_retains_resolution_edition() {
    assert_definition_edition(true);
}
fn assert_definition_edition(relocate: bool) {
    let parent = Arc::new(
        ParsedFile::parse_declarations(
            &SourceBlob::new(b"fixture!();".to_vec()).unwrap(),
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
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    );
    let input = fixture::input("");
    let site = fixture::call_site(&input);
    let context = input
        .spans()
        .iter()
        .find(|row| row.key() == site)
        .unwrap()
        .context;
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest([1; 32]),
        },
        invocation: parent.producers()[0].identity,
        input_tokens: Digest::of_bytes(&input.encode().unwrap()),
        consumer_compile_env: Digest([4; 32]),
        parent_context: Digest(context.0),
        definition_edition: 2021,
        call_edition: 2024,
        grammar: GrammarSlot::Items,
        engine_version: Digest([6; 32]),
        procedural_epoch: None,
    };
    let rules =
        DeclarativeMacro::new(fixture::definition("()=>{mod gen {}}", Edition::E2021)).unwrap();
    let (expanded, _) = rules
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
    let expanded = expanded.unwrap().tokens;
    let expanded = if relocate {
        relocate_to_call_site(&expanded, &input, site)
    } else {
        expanded
    };
    let record =
        Arc::new(ExpansionRecord::from_wire(key.clone(), expanded.encode().unwrap()).unwrap());
    let tokens = lrr_expand_adapter::token_wire::TokenForest::decode(
        &record.tokens().unwrap().encode().unwrap(),
    )
    .unwrap();
    lrr_expand_adapter::declaration_output::materialize(
        &tokens,
        lrr_expand_adapter::declaration_output::DeclarationGrammar::Items,
    )
    .unwrap();
    assert!(fixture::literal_texts(&tokens).is_empty());
    let mut stack = tokens.roots().iter().collect::<Vec<_>>();
    let mut actual = Vec::new();
    while let Some(token) = stack.pop() {
        match token {
            TokenTree::Group { children, .. } => stack.extend(children),
            TokenTree::Ident { text, raw, span } if text == "gen" => {
                assert!(!*raw);
                let physical = tokens.spans().iter().find(|s| s.key() == *span).unwrap();
                assert_eq!(
                    physical.edition,
                    if relocate {
                        Edition::E2024
                    } else {
                        Edition::E2021
                    }
                );
                let context = tokens
                    .contexts()
                    .iter()
                    .find(|c| c.key() == physical.context)
                    .unwrap();
                assert_eq!(context.edition, Edition::E2021);
                assert_eq!(context.defining_unit, Key([1; 32]));
                actual.push(*span);
            }
            _ => {}
        }
    }
    assert_eq!(actual.len(), 1);
    assert_eq!(record.key().definition_edition, 2021);
    assert_eq!(record.key().call_edition, 2024);
    let output = Arc::new(
        GeneratedDeclarations::admit(
            &key,
            record,
            1,
            &parent,
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    );
    assert_eq!(output.scan().modules().len(), 1);
    let row = &output.scan().modules()[0];
    assert_eq!(row.name_edition, 2021);
    let tokens = output.module_name_tokens(row.identity).unwrap();
    let roots = match tokens.roots() {
        [TokenTree::Group { children, .. }] => children.as_slice(),
        roots => roots,
    };
    let [TokenTree::Ident { text, raw, span }] = roots else {
        panic!("exact generated module identifier required")
    };
    assert_eq!(text, "gen");
    assert!(!*raw);
    assert_eq!(
        tokens
            .spans()
            .iter()
            .find(|s| s.key() == *span)
            .unwrap()
            .edition,
        if relocate {
            Edition::E2024
        } else {
            Edition::E2021
        }
    );
    let index = GeneratedDeclarationMacroIndex::new(
        output,
        DeclarationMacroParent::Source(Arc::new(DeclarationMacroIndex::new(parent).unwrap())),
    )
    .unwrap();
    let bindings = index.module_bindings(1).unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].name.as_str(), "gen");
}

// Span::located_at changes physical source location while retaining resolution context.
// Exercise actual owned span transport; no expected output or macro invocation is faked.
fn relocate_to_call_site(
    tokens: &lrr_expand_adapter::token_wire::TokenForest,
    input: &lrr_expand_adapter::token_wire::TokenForest,
    site: Key,
) -> lrr_expand_adapter::token_wire::TokenForest {
    use std::collections::BTreeMap;
    let caller = input.spans().iter().find(|s| s.key() == site).unwrap();
    let mut remap = BTreeMap::new();
    let mut spans = BTreeMap::new();
    for original in tokens.spans() {
        let relocated = original.located_at(caller);
        remap.insert(original.key(), relocated.key());
        spans.insert(relocated.key(), relocated);
    }
    fn rewrite(token: &mut TokenTree, remap: &BTreeMap<Key, Key>) {
        match token {
            TokenTree::Group {
                open,
                close,
                children,
                ..
            } => {
                *open = remap[open];
                *close = remap[close];
                for child in children {
                    rewrite(child, remap);
                }
            }
            TokenTree::Ident { span, .. }
            | TokenTree::Punct { span, .. }
            | TokenTree::Literal { span, .. } => *span = remap[span],
        }
    }
    let mut roots = tokens.roots().to_vec();
    for token in &mut roots {
        rewrite(token, &remap);
    }
    lrr_expand_adapter::token_wire::TokenForest::new(
        roots,
        tokens.contexts().to_vec(),
        spans.into_values().collect(),
    )
    .unwrap()
}
