use lrr::{
    core::digest::Digest,
    expand::scopes::{MacroDefinition, MacroName, ScopeKey},
    frontend::{
        hygiene::HygieneFacts,
        local_scopes::{LocalLookup, LocalScopes},
    },
    syntax::token_wire::{
        ContextRecord, Edition, Key, NameContextKey, NameResolutionContext, ResolutionContext,
        Transparency,
    },
};
use std::sync::Arc;
fn facts() -> (Arc<HygieneFacts>, Key, Key) {
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([1; 32]),
        edition: Edition::E2024,
    };
    let marked = ContextRecord {
        parent: Some(root.key()),
        expansion: Some(Key([8; 32])),
        transparency: Transparency::SemiTransparent,
        defining_unit: Key([1; 32]),
        edition: Edition::E2024,
    };
    let rows = vec![
        ResolutionContext {
            original: root.key(),
            item: root.key(),
            local: root.key(),
        },
        ResolutionContext {
            original: marked.key(),
            item: root.key(),
            local: marked.key(),
        },
    ];
    (
        Arc::new(
            HygieneFacts::new(vec![root.clone(), marked.clone()], rows)
                .unwrap()
                .with_name_contexts(vec![
                    NameResolutionContext {
                        original: root.key(),
                        item: NameContextKey(Key([10; 32])),
                        local: NameContextKey(Key([10; 32])),
                    },
                    NameResolutionContext {
                        original: marked.key(),
                        item: NameContextKey(Key([10; 32])),
                        local: NameContextKey(Key([11; 32])),
                    },
                ])
                .unwrap(),
        ),
        root.key(),
        marked.key(),
    )
}
fn definition() -> MacroDefinition {
    MacroDefinition {
        identity: Digest([9; 32]),
        defining_unit: Digest([1; 32]),
        definition_edition: 2024,
    }
}
#[test]
fn local_hygiene_crosses_only_the_matching_definition_rib() {
    let (facts, root_ctx, marked) = facts();
    let mut scopes = LocalScopes::new(facts);
    let root = ScopeKey::root(Digest([1; 32]));
    let name = MacroName::parse("x", 2024).unwrap();
    scopes.add_root(root).unwrap();
    scopes
        .bind(root, 5, name.clone(), root_ctx, Digest([2; 32]))
        .unwrap();
    scopes.define_macro(root, 10, definition()).unwrap();
    scopes
        .register_expansion(Key([8; 32]), definition())
        .unwrap();
    let child = scopes.add_child(root, 20, 0, false).unwrap();
    scopes
        .bind(child, 25, name.clone(), root_ctx, Digest([3; 32]))
        .unwrap();
    assert_eq!(
        scopes.lookup(child, 30, &name, marked).unwrap(),
        LocalLookup::Binding(Digest([2; 32]))
    );
    assert_eq!(
        scopes.lookup(child, 30, &name, root_ctx).unwrap(),
        LocalLookup::Binding(Digest([3; 32]))
    );
    let generated = scopes.add_child(child, 30, 0, false).unwrap();
    scopes
        .bind(generated, 1, name.clone(), marked, Digest([4; 32]))
        .unwrap();
    assert_eq!(
        scopes.lookup(generated, 2, &name, marked).unwrap(),
        LocalLookup::Binding(Digest([4; 32]))
    );
    assert_eq!(
        scopes.lookup(generated, 2, &name, root_ctx).unwrap(),
        LocalLookup::Binding(Digest([3; 32]))
    );
    let before = scopes.add_child(root, 9, 1, false).unwrap();
    assert_eq!(
        scopes.lookup(before, 30, &name, marked).unwrap(),
        LocalLookup::Absent
    );
    let function = scopes.add_child(child, 30, 1, true).unwrap();
    assert_eq!(
        scopes.lookup(function, 1, &name, root_ctx).unwrap(),
        LocalLookup::Absent
    );
}
#[test]
fn local_scope_pending_and_invalid_hygiene_facts_never_become_absence() {
    let (facts, root_ctx, marked) = facts();
    let mut scopes = LocalScopes::new(facts);
    let root = ScopeKey::root(Digest([1; 32]));
    let name = MacroName::parse("x", 2024).unwrap();
    scopes.add_root(root).unwrap();
    assert!(scopes.lookup(root, 100, &name, marked).is_err());
    scopes
        .register_expansion(Key([8; 32]), definition())
        .unwrap();
    scopes
        .bind(root, 5, name.clone(), root_ctx, Digest([2; 32]))
        .unwrap();
    scopes.block(root, 10, Digest([8; 32])).unwrap();
    assert_eq!(
        scopes.lookup(root, 20, &name, root_ctx).unwrap(),
        LocalLookup::NeedInput(Digest([8; 32]))
    );
    assert!(scopes.publish(root, 10, Digest([99; 32]), vec![]).is_err());
    assert_eq!(
        scopes.lookup(root, 20, &name, root_ctx).unwrap(),
        LocalLookup::NeedInput(Digest([8; 32]))
    );
    scopes
        .bind(root, 15, name.clone(), root_ctx, Digest([3; 32]))
        .unwrap();
    assert_eq!(
        scopes.lookup(root, 20, &name, root_ctx).unwrap(),
        LocalLookup::Binding(Digest([3; 32]))
    );
    scopes.publish(root, 10, Digest([8; 32]), vec![]).unwrap();
    assert!(
        scopes
            .bind(root, 18, name.clone(), Key([255; 32]), Digest([4; 32]))
            .is_err()
    );
    assert_eq!(
        scopes.lookup(root, 20, &name, root_ctx).unwrap(),
        LocalLookup::Binding(Digest([3; 32]))
    );
    let mut bad = definition();
    bad.defining_unit = Digest([3; 32]);
    assert!(scopes.register_expansion(Key([8; 32]), bad).is_err());
}

#[test]
fn local_publication_is_atomic_and_retains_new_producer_demands() {
    use lrr::frontend::local_scopes::LocalDeclaration;
    let (facts, root_ctx, _) = facts();
    let mut scopes = LocalScopes::new(facts);
    let root = ScopeKey::root(Digest([1; 32]));
    scopes.add_root(root).unwrap();
    let name = MacroName::parse("x", 2024).unwrap();
    let invocation = Digest([8; 32]);
    scopes.block(root, 10, invocation).unwrap();
    let valid = LocalDeclaration::Binding {
        at: 15,
        name: name.clone(),
        context: root_ctx,
        identity: Digest([2; 32]),
    };
    let bad = LocalDeclaration::Binding {
        at: 18,
        name: name.clone(),
        context: Key([255; 32]),
        identity: Digest([3; 32]),
    };
    assert!(
        scopes
            .publish(root, 10, invocation, vec![valid.clone(), bad])
            .is_err()
    );
    assert_eq!(
        scopes.lookup(root, 20, &name, root_ctx).unwrap(),
        LocalLookup::NeedInput(invocation)
    );
    assert!(
        scopes
            .publish(root, 10, invocation, vec![valid.clone(), valid.clone()])
            .is_err()
    );
    assert_eq!(
        scopes.lookup(root, 20, &name, root_ctx).unwrap(),
        LocalLookup::NeedInput(invocation)
    );
    let next = Digest([9; 32]);
    scopes
        .publish(
            root,
            10,
            invocation,
            vec![
                valid,
                LocalDeclaration::Pending {
                    at: 16,
                    invocation: next,
                },
            ],
        )
        .unwrap();
    assert_eq!(
        scopes.lookup(root, 20, &name, root_ctx).unwrap(),
        LocalLookup::NeedInput(next)
    );
    scopes.publish(root, 16, next, vec![]).unwrap();
    assert_eq!(
        scopes.lookup(root, 20, &name, root_ctx).unwrap(),
        LocalLookup::Binding(Digest([2; 32]))
    );
}

#[test]
fn local_lookup_is_iterative_and_rejects_mismatched_mark_authority() {
    let (facts, root_ctx, marked) = facts();
    let root = ScopeKey::root(Digest([1; 32]));
    let name = MacroName::parse("x", 2024).unwrap();
    let mut scopes = LocalScopes::new(facts.clone());
    scopes.add_root(root).unwrap();
    scopes
        .bind(root, 5, name.clone(), root_ctx, Digest([2; 32]))
        .unwrap();
    scopes.define_macro(root, 10, definition()).unwrap();
    scopes
        .register_expansion(Key([8; 32]), definition())
        .unwrap();
    let mut deep = root;
    for _ in 0..4096 {
        deep = scopes.add_child(deep, 200, 99, false).unwrap();
    }
    assert_eq!(
        scopes.lookup(deep, 300, &name, marked).unwrap(),
        LocalLookup::Binding(Digest([2; 32]))
    );
    let function = scopes.add_child(deep, 300, 0, true).unwrap();
    assert_eq!(
        scopes.lookup(function, 1, &name, marked).unwrap(),
        LocalLookup::Absent
    );
    let mut invalid = LocalScopes::new(facts);
    invalid.add_root(root).unwrap();
    let mut wrong = definition();
    wrong.defining_unit = Digest([3; 32]);
    invalid.register_expansion(Key([8; 32]), wrong).unwrap();
    assert!(invalid.lookup(root, 100, &name, marked).is_err());
    assert_eq!(
        invalid.lookup(root, 100, &name, root_ctx).unwrap(),
        LocalLookup::Absent
    );
}

#[test]
fn local_publication_stays_before_the_following_source_frontier() {
    use lrr::frontend::local_scopes::LocalDeclaration;
    for child_frontier in [false, true] {
        let (facts, root_ctx, _) = facts();
        let mut scopes = LocalScopes::new(facts);
        let root = ScopeKey::root(Digest([1; 32]));
        let name = MacroName::parse("x", 2024).unwrap();
        scopes.add_root(root).unwrap();
        let producer = Digest([8; 32]);
        scopes.block(root, 10, producer).unwrap();
        if child_frontier {
            scopes.add_child(root, 20, 0, false).unwrap();
        } else {
            scopes
                .bind(
                    root,
                    20,
                    MacroName::parse("other", 2024).unwrap(),
                    root_ctx,
                    Digest([5; 32]),
                )
                .unwrap();
        }
        let declaration = |at| LocalDeclaration::Binding {
            at,
            name: name.clone(),
            context: root_ctx,
            identity: Digest([2; 32]),
        };
        assert!(
            scopes
                .publish(root, 10, producer, vec![declaration(15), declaration(25)])
                .is_err()
        );
        assert_eq!(
            scopes.lookup(root, 30, &name, root_ctx).unwrap(),
            LocalLookup::NeedInput(producer)
        );
        scopes
            .publish(root, 10, producer, vec![declaration(15)])
            .unwrap();
        assert_eq!(
            scopes.lookup(root, 30, &name, root_ctx).unwrap(),
            LocalLookup::Binding(Digest([2; 32]))
        );
    }
}
