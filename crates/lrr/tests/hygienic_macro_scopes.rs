use lrr::{
    core::digest::Digest,
    expand::scopes::{MacroDefinition, MacroName, ScopeKey, TextualScopes},
    frontend::{
        hygiene::HygieneFacts,
        macro_origins::{ExpansionAncestry, MacroSelection},
        macro_scopes::{HygienicLookup, MacroDeclarations, MacroReference},
    },
    syntax::token_wire::{
        ContextRecord, Edition, Key, NameContextKey, NameResolutionContext, ResolutionContext,
        Transparency,
    },
};
fn facts() -> (HygieneFacts, Key, Key) {
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([1; 32]),
        edition: Edition::E2024,
    };
    let other = ContextRecord {
        defining_unit: Key([2; 32]),
        ..root.clone()
    };
    let keys = [root.key(), other.key()];
    let facts = HygieneFacts::new(
        vec![root, other],
        keys.iter()
            .map(|&k| ResolutionContext {
                original: k,
                item: k,
                local: k,
            })
            .collect(),
    )
    .unwrap()
    .with_name_contexts(
        keys.iter()
            .enumerate()
            .map(|(i, &k)| NameResolutionContext {
                original: k,
                item: NameContextKey(Key([i as u8; 32])),
                local: NameContextKey(Key([i as u8; 32])),
            })
            .collect(),
    )
    .unwrap();
    (facts, keys[0], keys[1])
}
fn definition(id: u8) -> MacroDefinition {
    MacroDefinition {
        identity: Digest([id; 32]),
        defining_unit: Digest([1; 32]),
        definition_edition: 2024,
    }
}
#[test]
fn textual_candidates_filter_hygiene_before_origin_ambiguity_and_capture_parent_order() {
    let (facts, context, other) = facts();
    let mut graph = ExpansionAncestry::default();
    let expansion = Key([4; 32]);
    graph.register(expansion, None).unwrap();
    let mut declarations = MacroDeclarations::default();
    for (id, ctx, origin) in [
        (10, context, None),
        (11, other, Some(expansion)),
        (12, context, Some(expansion)),
        (13, context, None),
    ] {
        declarations
            .register(definition(id), ctx, origin, &facts, &graph)
            .unwrap();
    }
    let mut scopes = TextualScopes::default();
    let root = ScopeKey::root(Digest([1; 32]));
    scopes.add_root(root).unwrap();
    let name = MacroName::parse("m", 2024).unwrap();
    scopes
        .define(root, 1, name.clone(), definition(10))
        .unwrap();
    scopes
        .define(root, 2, name.clone(), definition(11))
        .unwrap();
    let child = scopes.add_child(root, 3, 0).unwrap();
    scopes
        .define(root, 4, name.clone(), definition(13))
        .unwrap();
    assert_eq!(
        declarations
            .resolve(
                &scopes,
                MacroReference {
                    scope: child,
                    before: 10,
                    name: &name,
                    context,
                    origin: None
                },
                &facts,
                &graph
            )
            .unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(definition(10)))
    );
    scopes
        .define(child, 1, name.clone(), definition(12))
        .unwrap();
    assert!(matches!(
        declarations
            .resolve(
                &scopes,
                MacroReference {
                    scope: child,
                    before: 10,
                    name: &name,
                    context,
                    origin: None
                },
                &facts,
                &graph
            )
            .unwrap(),
        HygienicLookup::Selection(MacroSelection::Ambiguous(_))
    ));
    assert_eq!(
        declarations
            .resolve(
                &scopes,
                MacroReference {
                    scope: child,
                    before: 10,
                    name: &name,
                    context,
                    origin: Some(expansion)
                },
                &facts,
                &graph
            )
            .unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(definition(12)))
    );
}
#[test]
fn pending_outer_producers_suspend_even_with_nearer_binding_and_metadata_is_required() {
    let (facts, context, _) = facts();
    let graph = ExpansionAncestry::default();
    let mut declarations = MacroDeclarations::default();
    let mut scopes = TextualScopes::default();
    let root = ScopeKey::root(Digest([1; 32]));
    scopes.add_root(root).unwrap();
    let child = scopes.add_child(root, 5, 0).unwrap();
    let name = MacroName::parse("m", 2024).unwrap();
    scopes
        .define(child, 1, name.clone(), definition(10))
        .unwrap();
    scopes.block(root, 2, Digest([20; 32])).unwrap();
    assert_eq!(
        declarations
            .resolve(
                &scopes,
                MacroReference {
                    scope: child,
                    before: 3,
                    name: &name,
                    context,
                    origin: None
                },
                &facts,
                &graph
            )
            .unwrap(),
        HygienicLookup::NeedInput(Digest([20; 32]))
    );
    scopes.publish(root, 2, Digest([20; 32]), vec![]).unwrap();
    assert!(
        declarations
            .resolve(
                &scopes,
                MacroReference {
                    scope: child,
                    before: 3,
                    name: &name,
                    context,
                    origin: None
                },
                &facts,
                &graph
            )
            .is_err()
    );
    declarations
        .register(definition(10), context, None, &facts, &graph)
        .unwrap();
    assert!(
        declarations
            .register(definition(10), context, Some(Key([99; 32])), &facts, &graph)
            .is_err()
    );
    assert_eq!(
        declarations
            .resolve(
                &scopes,
                MacroReference {
                    scope: child,
                    before: 3,
                    name: &name,
                    context,
                    origin: None
                },
                &facts,
                &graph
            )
            .unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(definition(10)))
    );
    assert!(
        declarations
            .resolve(
                &scopes,
                MacroReference {
                    scope: child,
                    before: 3,
                    name: &name,
                    context: Key([99; 32]),
                    origin: None
                },
                &facts,
                &graph
            )
            .is_err()
    );
}

#[test]
fn generated_batch_selection_waits_for_publication_and_retains_declaration_origin() {
    let (facts, context, _) = facts();
    let mut graph = ExpansionAncestry::default();
    let expansion = Key([4; 32]);
    graph.register(expansion, None).unwrap();
    let mut declarations = MacroDeclarations::default();
    for (id, origin) in [(10, None), (11, Some(expansion)), (12, Some(expansion))] {
        // Substituted caller names all have the same name context; origins
        // still belong to the declarations that emitted them.
        declarations
            .register(definition(id), context, origin, &facts, &graph)
            .unwrap();
    }
    let mut scopes = TextualScopes::default();
    let scope = ScopeKey::root(Digest([1; 32]));
    scopes.add_root(scope).unwrap();
    let name = MacroName::parse("m", 2024).unwrap();
    scopes
        .define(scope, 1, name.clone(), definition(10))
        .unwrap();
    let invocation = Digest([20; 32]);
    scopes.block(scope, 2, invocation).unwrap();
    let lookup = |scopes: &TextualScopes, origin| {
        declarations
            .resolve(
                scopes,
                MacroReference {
                    scope,
                    before: 3,
                    name: &name,
                    context,
                    origin,
                },
                &facts,
                &graph,
            )
            .unwrap()
    };
    assert_eq!(lookup(&scopes, None), HygienicLookup::NeedInput(invocation));
    scopes
        .publish(
            scope,
            2,
            invocation,
            vec![
                (name.clone(), definition(11)),
                (name.clone(), definition(12)),
            ],
        )
        .unwrap();
    assert_eq!(
        lookup(&scopes, Some(expansion)),
        HygienicLookup::Selection(MacroSelection::Defined(definition(12)))
    );
    assert_eq!(
        lookup(&scopes, None),
        HygienicLookup::Selection(MacroSelection::Ambiguous(vec![
            definition(10),
            definition(12)
        ]))
    );
}

#[test]
fn same_output_incompatible_latest_name_does_not_erase_compatible_definition() {
    let (facts, context, other) = facts();
    let mut graph = ExpansionAncestry::default();
    let expansion = Key([4; 32]);
    graph.register(expansion, None).unwrap();
    let mut declarations = MacroDeclarations::default();
    declarations
        .register(definition(11), context, Some(expansion), &facts, &graph)
        .unwrap();
    declarations
        .register(definition(12), other, Some(expansion), &facts, &graph)
        .unwrap();
    let mut scopes = TextualScopes::default();
    let scope = ScopeKey::root(Digest([1; 32]));
    scopes.add_root(scope).unwrap();
    let name = MacroName::parse("m", 2024).unwrap();
    let invocation = Digest([20; 32]);
    scopes.block(scope, 1, invocation).unwrap();
    scopes
        .publish(
            scope,
            1,
            invocation,
            vec![
                (name.clone(), definition(11)),
                (name.clone(), definition(12)),
            ],
        )
        .unwrap();
    assert_eq!(
        declarations
            .resolve(
                &scopes,
                MacroReference {
                    scope,
                    before: 2,
                    name: &name,
                    context,
                    origin: Some(expansion)
                },
                &facts,
                &graph
            )
            .unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(definition(11)))
    );
}

#[test]
fn inherited_source_chain_checks_every_definition_metadata_row() {
    use lrr::frontend::macro_scopes::OuterMacroScope;
    let (facts, context, other) = facts();
    let graph = ExpansionAncestry::default();
    let name = MacroName::parse("m", 2024).unwrap();
    let root = ScopeKey::root(Digest([1; 32]));
    let mut scopes = TextualScopes::default();
    scopes.add_root(root).unwrap();
    scopes
        .define(root, 1, name.clone(), definition(10))
        .unwrap();
    let mut local = MacroDeclarations::default();
    local
        .register(definition(10), context, None, &facts, &graph)
        .unwrap();
    let mut compatible = MacroDeclarations::default();
    compatible
        .register(definition(10), context, None, &facts, &graph)
        .unwrap();
    let mut conflicting = MacroDeclarations::default();
    conflicting
        .register(definition(10), other, None, &facts, &graph)
        .unwrap();
    let outers = [
        OuterMacroScope {
            scopes: &scopes,
            scope: root,
            before: 2,
            declarations: &compatible,
        },
        OuterMacroScope {
            scopes: &scopes,
            scope: root,
            before: 2,
            declarations: &conflicting,
        },
    ];
    assert!(
        local
            .resolve_with_outers(
                &scopes,
                MacroReference {
                    scope: root,
                    before: 2,
                    name: &name,
                    context,
                    origin: None
                },
                &outers,
                &facts,
                &graph
            )
            .is_err()
    );
}
