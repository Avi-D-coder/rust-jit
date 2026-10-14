//! Test-only matched adapter hygiene preparation; only owned runtime facts leave syntax.
use super::token_wire as runtime;
use crate::{core::digest::Digest, frontend::hygiene::HygieneFacts};
use lrr_expand_adapter::{
    hygiene::HygieneStore,
    token_wire::{Edition, Key, TokenForest},
};
pub(crate) fn root(unit: Digest, edition: u16) -> (runtime::Key, HygieneFacts) {
    let mut hygiene = HygieneStore::default();
    let context = hygiene
        .root(
            Key(unit.0),
            match edition {
                2015 => Edition::E2015,
                2018 => Edition::E2018,
                2021 => Edition::E2021,
                2024 => Edition::E2024,
                _ => unreachable!(),
            },
        )
        .unwrap();
    let keys = [context];
    let rows = hygiene.resolution_contexts(&keys).unwrap();
    let names = hygiene.name_contexts(&keys).unwrap();
    let wire = TokenForest::new(vec![], hygiene.records(), vec![])
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
    (runtime::Key(context.0), facts)
}
