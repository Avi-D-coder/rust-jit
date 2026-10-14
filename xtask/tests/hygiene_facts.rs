use lrr::{frontend::hygiene::HygieneFacts, syntax::token_wire as runtime};
use lrr_expand_adapter::{
    hygiene::{HygieneStore, MarkRequest},
    token_wire::{Edition, Key, TokenForest, Transparency},
};
#[test]
fn pinned_normalization_facts_cross_the_runtime_boundary_as_owned_data() {
    let mut store = HygieneStore::default();
    let root = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let marked = store
        .apply(MarkRequest {
            parent: root,
            expansion: Key([2; 32]),
            defining_unit: Key([9; 32]),
            definition_edition: Edition::E2021,
            call_site: root,
            transparency: Transparency::SemiTransparent,
        })
        .unwrap();
    let rows = store.resolution_contexts(&[root, marked]).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let decoded = runtime::TokenForest::decode(&wire).unwrap();
    let rows: Vec<runtime::ResolutionContext> =
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap();
    let facts = HygieneFacts::new(decoded.contexts().to_vec(), rows).unwrap();
    assert_eq!(
        facts.item(runtime::Key(marked.0)).unwrap(),
        runtime::Key(root.0)
    );
    assert_eq!(
        facts.local(runtime::Key(marked.0)).unwrap(),
        runtime::Key(marked.0)
    );
    assert_eq!(
        facts.dollar_crate(runtime::Key(marked.0)).unwrap(),
        runtime::Key([9; 32])
    );
    assert_ne!(
        facts.dollar_crate(runtime::Key(marked.0)).unwrap(),
        facts.context(runtime::Key(root.0)).unwrap().defining_unit
    );
}
