use lrr::{
    core::digest::Digest,
    expand::scopes::{MacroName, ScopeKey},
    frontend::{
        hygiene::HygieneFacts,
        local_scopes::{LocalLookup, LocalScopes},
    },
    syntax::token_wire as runtime,
};
use lrr_expand_adapter::{
    hygiene::HygieneStore,
    token_wire::{Edition, Key, TokenForest},
};
use std::sync::Arc;
#[test]
fn owned_name_transport_resolves_root_unit_edition_differences_without_owner_rewrite() {
    let mut store = HygieneStore::default();
    let definition = store.root(Key([1; 32]), Edition::E2015).unwrap();
    let caller = store.root(Key([2; 32]), Edition::E2024).unwrap();
    let keys = [definition, caller];
    let rows = store.resolution_contexts(&keys).unwrap();
    let names = store.name_contexts(&keys).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let decoded = runtime::TokenForest::decode(&wire).unwrap();
    let facts = HygieneFacts::new(
        decoded.contexts().to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
    let definition = runtime::Key(definition.0);
    let caller = runtime::Key(caller.0);
    assert_ne!(
        facts.local(definition).unwrap(),
        facts.local(caller).unwrap()
    );
    assert_eq!(
        facts.name_local(definition).unwrap(),
        facts.name_local(caller).unwrap()
    );
    assert_eq!(
        facts.context(definition).unwrap().defining_unit,
        runtime::Key([1; 32])
    );
    assert_eq!(
        facts.context(caller).unwrap().edition,
        runtime::Edition::E2024
    );
    let mut scopes = LocalScopes::new(Arc::new(facts));
    let scope = ScopeKey::root(Digest([2; 32]));
    scopes.add_root(scope).unwrap();
    let name = MacroName::parse("x", 2024).unwrap();
    scopes
        .bind(scope, 1, name.clone(), definition, Digest([3; 32]))
        .unwrap();
    assert_eq!(
        scopes.lookup(scope, 2, &name, caller).unwrap(),
        LocalLookup::Binding(Digest([3; 32]))
    );
}
