use lrr::{
    core::digest::Digest,
    expand::scopes::{MacroDefinition, MacroName, ScopeKey, ScopeLookup, TextualScopes},
};

fn name(text: &str) -> MacroName {
    MacroName::parse(text, 2024).unwrap()
}
fn definition(value: u8, unit: Digest) -> MacroDefinition {
    MacroDefinition {
        identity: Digest([value; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    }
}

#[test]
fn textual_shadowing_obeys_source_order_and_captured_parent_position() {
    let unit = Digest([1; 32]);
    let root = ScopeKey::root(unit);
    let mut scopes = TextualScopes::default();
    scopes.add_root(root).unwrap();
    scopes
        .define(root, 10, name("choose"), definition(2, unit))
        .unwrap();
    let child = scopes.add_child(root, 20, 0).unwrap();
    scopes
        .define(root, 30, name("choose"), definition(3, unit))
        .unwrap();
    assert_eq!(
        scopes.lookup(root, 9, &name("choose")).unwrap(),
        ScopeLookup::Absent
    );
    assert_eq!(
        scopes.lookup(root, 20, &name("choose")).unwrap(),
        ScopeLookup::Defined(definition(2, unit))
    );
    assert_eq!(
        scopes.lookup(root, 31, &name("choose")).unwrap(),
        ScopeLookup::Defined(definition(3, unit))
    );
    assert_eq!(
        scopes.lookup(child, 99, &name("choose")).unwrap(),
        ScopeLookup::Defined(definition(2, unit))
    );
    scopes
        .define(child, 40, name("choose"), definition(4, unit))
        .unwrap();
    assert_eq!(
        scopes.lookup(child, 41, &name("choose")).unwrap(),
        ScopeLookup::Defined(definition(4, unit))
    );
}

#[test]
fn pending_generated_declarations_suspend_lookup_without_caching_absence() {
    let unit = Digest([1; 32]);
    let root = ScopeKey::root(unit);
    let mut scopes = TextualScopes::default();
    scopes.add_root(root).unwrap();
    let invocation = Digest([9; 32]);
    scopes.block(root, 10, invocation).unwrap();
    assert_eq!(
        scopes.lookup(root, 9, &name("missing")).unwrap(),
        ScopeLookup::Absent
    );
    assert_eq!(
        scopes.lookup(root, 11, &name("missing")).unwrap(),
        ScopeLookup::NeedInput(invocation)
    );
    scopes
        .publish(
            root,
            10,
            invocation,
            vec![(name("missing"), definition(2, unit))],
        )
        .unwrap();
    assert_eq!(
        scopes.lookup(root, 11, &name("missing")).unwrap(),
        ScopeLookup::Defined(definition(2, unit))
    );
    assert!(scopes.publish(root, 10, invocation, vec![]).is_err());
}

#[test]
fn macro_names_use_pinned_identifier_parsing_and_normalize_raw_spelling() {
    assert_eq!(name("r#choose"), name("choose"));
    assert_eq!(name("r#gen").as_str(), "gen");
    assert!(MacroName::parse("gen", 2024).is_err());
    assert!(MacroName::parse("gen", 2021).is_ok());
    assert!(MacroName::parse("foo() {} fn injected", 2024).is_err());
    assert!(MacroName::parse("r#self", 2024).is_err());
    assert!(MacroName::parse("foo bar", 2024).is_err());
    assert!(MacroName::parse("foo", 2025).is_err());
    assert_eq!(name("δ").as_str(), "δ");
}

#[test]
fn rejected_generated_batch_keeps_blocker_and_publishes_no_partial_names() {
    let unit = Digest([1; 32]);
    let root = ScopeKey::root(unit);
    let mut scopes = TextualScopes::default();
    scopes.add_root(root).unwrap();
    let invocation = Digest([9; 32]);
    scopes.block(root, 10, invocation).unwrap();
    let batch = vec![
        (name("first"), definition(2, unit)),
        (
            name("second"),
            MacroDefinition {
                definition_edition: 2025,
                ..definition(3, unit)
            },
        ),
    ];
    assert!(scopes.publish(root, 10, invocation, batch).is_err());
    assert_eq!(
        scopes.lookup(root, 11, &name("first")).unwrap(),
        ScopeLookup::NeedInput(invocation)
    );
    assert!(scopes.publish(root, 10, Digest([8; 32]), vec![]).is_err());
    scopes.publish(root, 10, invocation, vec![]).unwrap();
    assert_eq!(
        scopes.lookup(root, 11, &name("first")).unwrap(),
        ScopeLookup::Absent
    );
}

#[test]
fn generated_outward_bindings_preserve_repeated_name_shadowing() {
    let unit = Digest([1; 32]);
    let root = ScopeKey::root(unit);
    let mut scopes = TextualScopes::default();
    scopes.add_root(root).unwrap();
    let invocation = Digest([9; 32]);
    scopes.block(root, 10, invocation).unwrap();
    scopes
        .publish(
            root,
            10,
            invocation,
            vec![
                (name("choose"), definition(2, unit)),
                (name("choose"), definition(3, unit)),
            ],
        )
        .unwrap();
    assert_eq!(
        scopes.lookup(root, 11, &name("choose")).unwrap(),
        ScopeLookup::Defined(definition(3, unit))
    );
}
