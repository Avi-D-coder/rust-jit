use lrr_expand_adapter::{
    hygiene::{HygieneStore, MarkRequest},
    token_wire::{Edition, Key, TokenForest, Transparency},
};
#[test]
fn owned_resolution_rows_use_pinned_item_and_local_forms_without_frontend_ids() {
    let mut store = HygieneStore::default();
    let root = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let mixed = store
        .apply(MarkRequest {
            parent: root,
            expansion: Key([2; 32]),
            defining_unit: Key([1; 32]),
            definition_edition: Edition::E2024,
            call_site: root,
            transparency: Transparency::SemiTransparent,
        })
        .unwrap();
    let transparent = store
        .apply(MarkRequest {
            parent: mixed,
            expansion: Key([3; 32]),
            defining_unit: Key([1; 32]),
            definition_edition: Edition::E2024,
            call_site: root,
            transparency: Transparency::Transparent,
        })
        .unwrap();
    let rows = store
        .resolution_contexts(&[transparent, root, mixed, transparent])
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert!(
        rows.windows(2)
            .all(|rows| rows[0].original < rows[1].original)
    );
    let last = rows.iter().find(|row| row.original == transparent).unwrap();
    assert_eq!(last.item, root);
    assert_eq!(last.local, mixed);
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let decoded = TokenForest::decode(&wire).unwrap();
    let mut imported = HygieneStore::default();
    imported.import(decoded.contexts()).unwrap();
    assert_eq!(
        imported
            .resolution_contexts(&[root, mixed, transparent])
            .unwrap(),
        rows
    );
    let before = store.records();
    assert!(store.resolution_contexts(&[root, Key([255; 32])]).is_err());
    assert_eq!(store.records(), before);
}
