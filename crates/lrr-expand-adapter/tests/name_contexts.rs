use lrr_expand_adapter::{
    hygiene::{HygieneStore, MarkRequest, ResolutionMode},
    token_wire::{Edition, Key, TokenForest, Transparency},
};
#[test]
fn owned_name_classes_match_pinned_equality_and_survive_private_id_remapping() {
    let mut store = HygieneStore::default();
    let a = store.root(Key([1; 32]), Edition::E2015).unwrap();
    let b = store.root(Key([2; 32]), Edition::E2024).unwrap();
    let mut keys = vec![a, b];
    for (index, transparency) in [
        Transparency::Opaque,
        Transparency::SemiTransparent,
        Transparency::Transparent,
    ]
    .into_iter()
    .enumerate()
    {
        let marked = store
            .apply(MarkRequest {
                parent: a,
                expansion: Key([u8::try_from(index).unwrap() + 3; 32]),
                defining_unit: Key([1; 32]),
                definition_edition: Edition::E2024,
                call_site: b,
                transparency,
            })
            .unwrap();
        keys.push(marked);
    }
    let nested = store
        .apply(MarkRequest {
            parent: keys[3],
            expansion: Key([9; 32]),
            defining_unit: Key([1; 32]),
            definition_edition: Edition::E2021,
            call_site: keys[2],
            transparency: Transparency::SemiTransparent,
        })
        .unwrap();
    keys.push(nested);
    let before = store.records();
    let rows = store.name_contexts(&keys).unwrap();
    assert_eq!(store.records(), before);
    for a in &rows {
        for b in &rows {
            assert_eq!(
                a.item == b.item,
                store
                    .name_equivalent(a.original, b.original, ResolutionMode::Item)
                    .unwrap()
            );
            assert_eq!(
                a.local == b.local,
                store
                    .name_equivalent(a.original, b.original, ResolutionMode::Local)
                    .unwrap()
            );
        }
    }
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let mut imported = HygieneStore::default();
    let unrelated = imported.root(Key([99; 32]), Edition::E2021).unwrap();
    imported
        .apply(MarkRequest {
            parent: unrelated,
            expansion: Key([98; 32]),
            defining_unit: Key([99; 32]),
            definition_edition: Edition::E2021,
            call_site: unrelated,
            transparency: Transparency::Opaque,
        })
        .unwrap();
    imported
        .import(TokenForest::decode(&wire).unwrap().contexts())
        .unwrap();
    assert_eq!(imported.name_contexts(&keys).unwrap(), rows);
    assert!(store.name_contexts(&[a, Key([255; 32])]).is_err());
    assert_eq!(store.records(), before);
}
