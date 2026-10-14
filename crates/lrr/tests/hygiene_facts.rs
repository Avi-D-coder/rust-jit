use lrr::{
    frontend::hygiene::HygieneFacts,
    syntax::token_wire::{ContextRecord, Edition, Key, ResolutionContext, Transparency},
};
#[test]
fn owned_hygiene_facts_preserve_definition_authority_and_reject_missing_rows() {
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([1; 32]),
        edition: Edition::E2024,
    };
    let mixed = ContextRecord {
        parent: Some(root.key()),
        expansion: Some(Key([2; 32])),
        transparency: Transparency::SemiTransparent,
        defining_unit: Key([9; 32]),
        edition: Edition::E2021,
    };
    let rows = vec![
        ResolutionContext {
            original: root.key(),
            item: root.key(),
            local: root.key(),
        },
        ResolutionContext {
            original: mixed.key(),
            item: root.key(),
            local: mixed.key(),
        },
    ];
    let facts = HygieneFacts::new(vec![root.clone(), mixed.clone()], rows.clone()).unwrap();
    assert_eq!(facts.item(mixed.key()).unwrap(), root.key());
    assert_eq!(facts.local(mixed.key()).unwrap(), mixed.key());
    assert_eq!(facts.dollar_crate(mixed.key()).unwrap(), Key([9; 32]));
    assert!(facts.dollar_crate(root.key()).is_err());
    assert!(facts.local(Key([255; 32])).is_err());
    let mut bad = rows.clone();
    bad[0].item = Key([255; 32]);
    assert!(HygieneFacts::new(vec![root.clone(), mixed.clone()], bad).is_err());
    let mut wrong_form = rows.clone();
    wrong_form[1].item = mixed.key();
    assert!(HygieneFacts::new(vec![root.clone(), mixed.clone()], wrong_form).is_err());
    let mut duplicate = rows;
    duplicate.push(duplicate[0].clone());
    assert!(HygieneFacts::new(vec![root, mixed], duplicate).is_err());
}
