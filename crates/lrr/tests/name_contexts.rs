use lrr::{
    frontend::hygiene::HygieneFacts,
    syntax::token_wire::{
        ContextRecord, Edition, Key, NameContextKey, NameResolutionContext, ResolutionContext,
        Transparency,
    },
};
#[test]
fn owned_name_facts_reject_unknown_duplicate_and_missing_rows() {
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([1; 32]),
        edition: Edition::E2024,
    };
    let facts = || {
        HygieneFacts::new(
            vec![root.clone()],
            vec![ResolutionContext {
                original: root.key(),
                item: root.key(),
                local: root.key(),
            }],
        )
        .unwrap()
    };
    let row = NameResolutionContext {
        original: root.key(),
        item: NameContextKey(Key([2; 32])),
        local: NameContextKey(Key([2; 32])),
    };
    assert!(facts().name_local(root.key()).is_err());
    assert!(
        facts()
            .with_name_contexts(vec![row.clone(), row.clone()])
            .is_err()
    );
    let mut unknown = row.clone();
    unknown.original = Key([255; 32]);
    assert!(facts().with_name_contexts(vec![unknown]).is_err());
    let names = facts().with_name_contexts(vec![row]).unwrap();
    assert_eq!(
        names.name_local(root.key()).unwrap(),
        names.name_item(root.key()).unwrap()
    );
    assert_eq!(
        names.context(root.key()).unwrap().defining_unit,
        Key([1; 32])
    );
    assert!(names.dollar_crate(root.key()).is_err());
}
