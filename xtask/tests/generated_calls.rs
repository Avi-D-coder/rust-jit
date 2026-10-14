use lrr::{
    core::digest::{Digest, StableEncode},
    expand::{
        generated_items::GeneratedItems,
        identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        record::ExpansionRecord,
    },
};
use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite},
    token_wire::{Edition, Key},
};
use std::sync::Arc;
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
fn key() -> ExpansionKey {
    ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest([1; 32]),
        },
        invocation: Digest([2; 32]),
        input_tokens: Digest([3; 32]),
        consumer_compile_env: Digest([4; 32]),
        parent_context: Digest(owner_context().0),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Items,
        engine_version: Digest([6; 32]),
        procedural_epoch: None,
    }
}
fn output(rules: &str) -> Arc<ExpansionRecord> {
    let d = DeclarativeMacro::new(fixture::definition(rules, Edition::E2024)).unwrap();
    let input = fixture::input("");
    let (out, _) = d
        .expand_hygienic_with_limits(
            &input,
            fixture::call_site(&input),
            DeclarativeSite {
                expansion: Key(key().fingerprint().0),
                defining_unit: Key([1; 32]),
                definition_edition: Edition::E2024,
            },
            2_000_000,
            32 * 1024 * 1024,
        )
        .unwrap();
    Arc::new(ExpansionRecord::from_wire(key(), out.unwrap().tokens.encode().unwrap()).unwrap())
}

fn owner_context() -> lrr::syntax::token_wire::Key {
    use lrr::syntax::token_wire::{ContextRecord, Edition, Key, Transparency};
    ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([2; 32]),
        edition: Edition::E2024,
    }
    .key()
}

fn items(rules: &str) -> GeneratedItems {
    GeneratedItems::admit(&key(), output(rules), 1).unwrap()
}
#[test]
fn generated_call_arguments_preserve_real_tokens_and_canonical_header_identity() {
    let g = items("()=>{forward!(7_i64,(1+2));};");
    let call = &g.calls()[0];
    assert_eq!(call.key, g.items()[0].key);
    assert_eq!(call.edition, 2024);
    assert_eq!(
        fixture::literal_texts(
            &lrr_expand_adapter::token_wire::TokenForest::decode(&call.input.encode().unwrap())
                .unwrap()
        ),
        vec!["7_i64", "1", "2"]
    );
    assert!(call.input.spans().iter().any(|s| s.key() == call.site));
    assert!(
        call.input
            .spans()
            .iter()
            .all(|s| g.tokens().spans().iter().any(|original| original == s))
    );
}
#[test]
fn repeated_generated_calls_have_distinct_structural_keys() {
    let g = items("()=>{forward!(7);forward!(7);};");
    assert_eq!(g.calls().len(), 2);
    assert_ne!(
        g.calls()[0].key.fingerprint(),
        g.calls()[1].key.fingerprint()
    );
    assert_eq!(g.calls()[0].key.structural_path, vec![0]);
    assert_eq!(g.calls()[1].key.structural_path, vec![1]);
}
#[test]
fn generated_qualified_call_path_retains_owned_punctuation_and_origins() {
    let g = items("()=>{crate::module::forward!{9}};");
    let call = &g.calls()[0];
    let mut names = vec![];
    let mut punctuation = vec![];
    let mut stack = call.path.roots().iter().rev().collect::<Vec<_>>();
    while let Some(t) = stack.pop() {
        match t {
            lrr::syntax::token_wire::TokenTree::Group { children, .. } => {
                stack.extend(children.iter().rev())
            }
            lrr::syntax::token_wire::TokenTree::Ident { text, .. } => names.push(text.as_str()),
            lrr::syntax::token_wire::TokenTree::Punct { scalar, .. } => punctuation.push(*scalar),
            _ => {}
        }
    }
    assert_eq!(names, vec!["crate", "module", "forward"]);
    assert_eq!(punctuation, vec![':', ':', ':', ':']);
    assert!(
        call.path
            .spans()
            .iter()
            .all(|s| g.tokens().spans().iter().any(|original| original == s))
    );
}

#[test]
fn generated_call_argument_spacing_matches_original_token_subtree() {
    let g = items("()=>{forward!(a : : b, a > > b);};");
    let lrr::syntax::token_wire::TokenTree::Group { children, .. } = &g.tokens().roots()[0] else {
        panic!("output root absent")
    };
    let original = children
        .iter()
        .find(|t| {
            matches!(
                t,
                lrr::syntax::token_wire::TokenTree::Group {
                    delimiter: lrr::syntax::token_wire::Delimiter::Parenthesis,
                    ..
                }
            )
        })
        .unwrap();
    assert_eq!(&g.calls()[0].input.roots()[0], original);
}
