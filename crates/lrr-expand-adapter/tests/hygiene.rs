use lrr_expand_adapter::{
    hygiene::{HygieneStore, MarkRequest, ResolutionMode},
    token_wire::{Edition, Key, Transparency},
};
#[test]
fn actual_pinned_normalization_distinguishes_item_and_local_marks() {
    let mut store = HygieneStore::default();
    let root = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let mut parent = root;
    let mut marked = Vec::new();
    for (id, transparency) in [
        (2, Transparency::Opaque),
        (3, Transparency::SemiTransparent),
        (4, Transparency::Transparent),
    ] {
        parent = store
            .apply(MarkRequest {
                parent,
                expansion: Key([id; 32]),
                defining_unit: Key([1; 32]),
                definition_edition: Edition::E2024,
                call_site: root,
                transparency,
            })
            .unwrap();
        marked.push(parent);
    }
    assert!(
        store
            .equivalent(marked[2], marked[1], ResolutionMode::Local)
            .unwrap()
    );
    assert!(
        store
            .equivalent(marked[2], marked[0], ResolutionMode::Item)
            .unwrap()
    );
    assert!(
        !store
            .equivalent(marked[1], root, ResolutionMode::Local)
            .unwrap()
    );
    let rows = store.records();
    let final_row = rows.iter().find(|r| r.key() == marked[2]).unwrap();
    assert_eq!(final_row.parent, Some(marked[1]));
    assert_eq!(final_row.expansion, Some(Key([4; 32])));
}
#[test]
fn mixed_site_replays_definition_marks_in_opaque_call_site_context() {
    let mut store = HygieneStore::default();
    let definition_root = store.root(Key([1; 32]), Edition::E2021).unwrap();
    let call_root = store.root(Key([2; 32]), Edition::E2024).unwrap();
    let call_opaque = store
        .apply(MarkRequest {
            parent: call_root,
            expansion: Key([3; 32]),
            defining_unit: Key([2; 32]),
            definition_edition: Edition::E2024,
            call_site: call_root,
            transparency: Transparency::Opaque,
        })
        .unwrap();
    let definition_mixed = store
        .apply(MarkRequest {
            parent: definition_root,
            expansion: Key([4; 32]),
            defining_unit: Key([1; 32]),
            definition_edition: Edition::E2021,
            call_site: definition_root,
            transparency: Transparency::SemiTransparent,
        })
        .unwrap();
    let output = store
        .apply(MarkRequest {
            parent: definition_mixed,
            expansion: Key([5; 32]),
            defining_unit: Key([1; 32]),
            definition_edition: Edition::E2021,
            call_site: call_opaque,
            transparency: Transparency::SemiTransparent,
        })
        .unwrap();
    let rows = store.records();
    let mut key = output;
    let mut chain = Vec::new();
    loop {
        let row = rows.iter().find(|r| r.key() == key).unwrap();
        chain.push(row.clone());
        if let Some(parent) = row.parent {
            key = parent;
        } else {
            break;
        }
    }
    assert_eq!(
        chain.iter().filter_map(|r| r.expansion).collect::<Vec<_>>(),
        vec![Key([5; 32]), Key([4; 32]), Key([3; 32])]
    );
    assert_eq!(chain.last().unwrap().key(), call_root);
    assert_eq!(chain[0].edition, Edition::E2021);
    assert_eq!(chain[2].edition, Edition::E2024);
    assert_eq!(chain[0].defining_unit, Key([1; 32]));
    assert_eq!(chain[2].defining_unit, Key([2; 32]));
    assert!(
        store
            .equivalent(output, call_opaque, ResolutionMode::Item)
            .unwrap()
    );
    let mut restored = HygieneStore::default();
    restored.import(&rows).unwrap();
    assert!(
        restored
            .equivalent(output, call_opaque, ResolutionMode::Item)
            .unwrap()
    );
    assert!(
        !restored
            .equivalent(output, call_opaque, ResolutionMode::Local)
            .unwrap()
    );
}
#[test]
fn owned_hygiene_reimport_is_independent_of_private_salsa_slots() {
    let mut first = HygieneStore::default();
    let root = first.root(Key([1; 32]), Edition::E2024).unwrap();
    let marked = first
        .apply(MarkRequest {
            parent: root,
            expansion: Key([2; 32]),
            defining_unit: Key([1; 32]),
            definition_edition: Edition::E2024,
            call_site: root,
            transparency: Transparency::SemiTransparent,
        })
        .unwrap();
    let mut second = HygieneStore::default();
    let spare = second.root(Key([7; 32]), Edition::E2021).unwrap();
    second
        .apply(MarkRequest {
            parent: spare,
            expansion: Key([8; 32]),
            defining_unit: Key([7; 32]),
            definition_edition: Edition::E2021,
            call_site: spare,
            transparency: Transparency::Opaque,
        })
        .unwrap();
    second.import(&first.records()).unwrap();
    assert!(
        second
            .equivalent(marked, root, ResolutionMode::Item)
            .unwrap()
    );
    assert!(
        !second
            .equivalent(marked, root, ResolutionMode::Local)
            .unwrap()
    );
    let count = second.records().len();
    assert!(
        second
            .apply(MarkRequest {
                parent: root,
                expansion: Key([2; 32]),
                defining_unit: Key([9; 32]),
                definition_edition: Edition::E2024,
                call_site: root,
                transparency: Transparency::Opaque
            })
            .is_err()
    );
    assert_eq!(second.records().len(), count);
    assert!(
        second
            .normalize(Key([255; 32]), ResolutionMode::Local)
            .is_err()
    );
}

#[test]
fn hygiene_depth_boundary_refuses_without_publishing_partial_ancestors() {
    let mut store = HygieneStore::default();
    let root = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let mut parent = root;
    for index in 0_u32..256 {
        let mut key = [0; 32];
        key[..4].copy_from_slice(&index.to_le_bytes());
        parent = store
            .apply(MarkRequest {
                parent,
                expansion: Key(key),
                defining_unit: Key([1; 32]),
                definition_edition: Edition::E2024,
                call_site: root,
                transparency: Transparency::Opaque,
            })
            .unwrap();
    }
    let count = store.records().len();
    assert!(
        store
            .apply(MarkRequest {
                parent,
                expansion: Key([255; 32]),
                defining_unit: Key([1; 32]),
                definition_edition: Edition::E2024,
                call_site: root,
                transparency: Transparency::Opaque
            })
            .is_err()
    );
    assert_eq!(store.records().len(), count);
    store.import(&store.records()).unwrap();
}

#[test]
fn conflicting_import_and_nonopaque_root_refuse_atomically() {
    let mut store = HygieneStore::default();
    let root = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let marked = store
        .apply(MarkRequest {
            parent: root,
            expansion: Key([2; 32]),
            defining_unit: Key([1; 32]),
            definition_edition: Edition::E2024,
            call_site: root,
            transparency: Transparency::SemiTransparent,
        })
        .unwrap();
    let rows = store.records();
    let mut bad = rows.clone();
    bad.iter_mut()
        .find(|r| r.key() == marked)
        .unwrap()
        .defining_unit = Key([9; 32]);
    assert!(store.import(&bad).is_err());
    assert_eq!(store.records(), rows);
    let mut invalid_root = rows.iter().find(|r| r.key() == root).unwrap().clone();
    invalid_root.transparency = Transparency::Transparent;
    assert!(store.import(&[invalid_root]).is_err());
    assert_eq!(store.records(), rows);
}
