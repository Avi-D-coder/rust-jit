use lrr::{
    core::digest::Digest,
    expand::scopes::{MacroDefinition, MacroName, ScopeKey, ScopeLookup, TextualScopes},
};
#[test]
fn macro_names_use_nfc_identity_without_compatibility_folding() {
    for edition in [2015, 2018, 2021, 2024] {
        assert_eq!(
            MacroName::parse("café", edition).unwrap(),
            MacroName::parse("cafe\u{301}", edition).unwrap()
        );
        assert_eq!(
            MacroName::parse("r#cafe\u{301}", edition).unwrap().as_str(),
            "café"
        );
        assert_eq!(
            MacroName::parse("\u{1100}\u{1161}", edition)
                .unwrap()
                .as_str(),
            "가"
        );
        assert_eq!(
            MacroName::parse("x\u{315}\u{300}", edition).unwrap(),
            MacroName::parse("x\u{300}\u{315}", edition).unwrap()
        );
        assert_ne!(
            MacroName::parse("Ａ", edition).unwrap(),
            MacroName::parse("A", edition).unwrap()
        );
    }
    let unit = Digest([1; 32]);
    let scope = ScopeKey::root(unit);
    let mut scopes = TextualScopes::default();
    scopes.add_root(scope).unwrap();
    let definition = MacroDefinition {
        identity: Digest([2; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    scopes
        .define(
            scope,
            1,
            MacroName::parse("café", 2024).unwrap(),
            definition.clone(),
        )
        .unwrap();
    assert_eq!(
        scopes
            .lookup(scope, 2, &MacroName::parse("cafe\u{301}", 2024).unwrap())
            .unwrap(),
        ScopeLookup::Defined(definition)
    );
}
