use lrr_expand_adapter::{
    hygiene::{HygieneStore, MarkRequest, ResolutionMode},
    token_wire::{Edition, Key, Transparency},
};
#[test]
fn pinned_name_comparison_erases_root_edition_without_erasing_owner_records() {
    let mut store = HygieneStore::default();
    let a = store.root(Key([1; 32]), Edition::E2015).unwrap();
    let b = store.root(Key([2; 32]), Edition::E2024).unwrap();
    assert!(!store.equivalent(a, b, ResolutionMode::Item).unwrap());
    assert!(store.name_equivalent(a, b, ResolutionMode::Item).unwrap());
    assert!(store.name_equivalent(a, b, ResolutionMode::Local).unwrap());
    let mixed = store
        .apply(MarkRequest {
            parent: a,
            expansion: Key([3; 32]),
            defining_unit: Key([1; 32]),
            definition_edition: Edition::E2024,
            call_site: b,
            transparency: Transparency::SemiTransparent,
        })
        .unwrap();
    let before = store.records();
    assert!(
        store
            .name_equivalent(mixed, b, ResolutionMode::Item)
            .unwrap()
    );
    assert!(
        !store
            .name_equivalent(mixed, b, ResolutionMode::Local)
            .unwrap()
    );
    assert_eq!(store.records(), before);
    assert_eq!(
        store
            .records()
            .iter()
            .find(|r| r.key() == a)
            .unwrap()
            .defining_unit,
        Key([1; 32])
    );
    assert_eq!(
        store
            .records()
            .iter()
            .find(|r| r.key() == b)
            .unwrap()
            .edition,
        Edition::E2024
    );
}
