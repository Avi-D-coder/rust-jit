use lrr::{
    core::digest::Digest,
    expand::scopes::{MacroDefinition, MacroName},
    frontend::macro_names::{MacroNamespaces, MacroPath, PathLookup},
};
fn path(unit: Digest, names: &[&str]) -> MacroPath {
    MacroPath::new(
        unit,
        names
            .iter()
            .map(|s| MacroName::parse(s, 2024).unwrap())
            .collect(),
    )
    .unwrap()
}
#[test]
fn reexports_and_dependency_renames_preserve_definition_unit_for_dollar_crate() {
    let provider = Digest([1; 32]);
    let facade = Digest([2; 32]);
    let caller = Digest([3; 32]);
    let definition = MacroDefinition {
        identity: Digest([4; 32]),
        defining_unit: provider,
        definition_edition: 2021,
    };
    let mut namespaces = MacroNamespaces::default();
    namespaces
        .define(path(provider, &["exported"]), definition.clone())
        .unwrap();
    namespaces
        .reexport(path(facade, &["renamed"]), path(provider, &["exported"]))
        .unwrap();
    namespaces
        .dependency(caller, MacroName::parse("alias", 2024).unwrap(), facade)
        .unwrap();
    let route = namespaces
        .dependency_path(
            caller,
            &MacroName::parse("alias", 2024).unwrap(),
            vec![MacroName::parse("renamed", 2024).unwrap()],
        )
        .unwrap();
    assert_eq!(
        namespaces.lookup(&route).unwrap(),
        PathLookup::Defined(definition.clone())
    );
    assert_eq!(
        MacroNamespaces::dollar_crate(&definition, vec![MacroName::parse("helper", 2024).unwrap()])
            .unwrap(),
        path(provider, &["helper"])
    );
    assert_ne!(
        MacroNamespaces::dollar_crate(&definition, vec![MacroName::parse("helper", 2024).unwrap()])
            .unwrap(),
        path(caller, &["helper"])
    );
    assert!(
        namespaces
            .dependency(caller, MacroName::parse("alias", 2024).unwrap(), provider)
            .is_err()
    );
    assert_eq!(
        namespaces
            .dependency_path(
                caller,
                &MacroName::parse("alias", 2024).unwrap(),
                vec![MacroName::parse("renamed", 2024).unwrap()]
            )
            .unwrap(),
        route
    );
}
#[test]
fn namespace_cycles_ambiguity_and_pending_inputs_are_explicit() {
    let unit = Digest([1; 32]);
    let a = path(unit, &["a"]);
    let b = path(unit, &["b"]);
    let mut namespaces = MacroNamespaces::default();
    namespaces.reexport(a.clone(), b.clone()).unwrap();
    namespaces.reexport(b.clone(), a.clone()).unwrap();
    assert_eq!(namespaces.lookup(&a).unwrap(), PathLookup::Absent);
    let invocation = Digest([9; 32]);
    let pending = path(unit, &["pending"]);
    namespaces.pending(pending.clone(), invocation).unwrap();
    assert_eq!(
        namespaces.lookup(&pending).unwrap(),
        PathLookup::NeedInput(invocation)
    );
    assert_eq!(
        namespaces.lookup(&path(unit, &["absent"])).unwrap(),
        PathLookup::Absent
    );
    let ambiguous = path(unit, &["ambiguous"]);
    for value in [2, 3] {
        namespaces
            .define(
                ambiguous.clone(),
                MacroDefinition {
                    identity: Digest([value; 32]),
                    defining_unit: unit,
                    definition_edition: 2024,
                },
            )
            .unwrap();
    }
    assert!(
        matches!(namespaces.lookup(&ambiguous).unwrap(), PathLookup::Ambiguous(definitions) if definitions.len() == 2)
    );
}

#[test]
fn long_reexport_chains_are_iterative_and_diamonds_deduplicate_identity() {
    let unit = Digest([1; 32]);
    let mut namespaces = MacroNamespaces::default();
    let definition = MacroDefinition {
        identity: Digest([2; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    let paths: Vec<_> = (0..3000)
        .map(|i| path(unit, &[&format!("name_{i}")]))
        .collect();
    for pair in paths.windows(2) {
        namespaces
            .reexport(pair[0].clone(), pair[1].clone())
            .unwrap();
    }
    namespaces
        .define(paths.last().unwrap().clone(), definition.clone())
        .unwrap();
    namespaces
        .reexport(paths[0].clone(), paths.last().unwrap().clone())
        .unwrap();
    assert_eq!(
        namespaces.lookup(&paths[0]).unwrap(),
        PathLookup::Defined(definition)
    );
}

#[test]
fn reexport_cycle_waits_for_producer_then_publishes_empty_or_definition_atomically() {
    let unit = Digest([1; 32]);
    let a = path(unit, &["a"]);
    let b = path(unit, &["b"]);
    let invocation = Digest([9; 32]);
    let mut namespaces = MacroNamespaces::default();
    namespaces.reexport(a.clone(), b.clone()).unwrap();
    namespaces.reexport(b.clone(), a.clone()).unwrap();
    namespaces.pending(b.clone(), invocation).unwrap();
    assert_eq!(
        namespaces.lookup(&a).unwrap(),
        PathLookup::NeedInput(invocation)
    );
    assert!(
        namespaces
            .publish(b.clone(), Digest([8; 32]), vec![])
            .is_err()
    );
    let definition = MacroDefinition {
        identity: Digest([2; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    let bad = MacroDefinition {
        definition_edition: 2025,
        ..definition.clone()
    };
    assert!(
        namespaces
            .publish(b.clone(), invocation, vec![definition.clone(), bad])
            .is_err()
    );
    assert_eq!(
        namespaces.lookup(&a).unwrap(),
        PathLookup::NeedInput(invocation)
    );
    namespaces.publish(b.clone(), invocation, vec![]).unwrap();
    assert_eq!(namespaces.lookup(&a).unwrap(), PathLookup::Absent);
    namespaces.pending(b.clone(), invocation).unwrap();
    namespaces
        .publish(b, invocation, vec![definition.clone()])
        .unwrap();
    assert_eq!(
        namespaces.lookup(&a).unwrap(),
        PathLookup::Defined(definition)
    );
}

#[test]
fn caller_aware_visibility_preserves_pending_outcomes_and_rejects_invalid_ranges_atomically() {
    use lrr::syntax::{outline::ScopeKey, visibility::VisibilityScope};
    let unit = Digest([21; 32]);
    let other = Digest([22; 32]);
    let path = path(unit, &["m"]);
    let caller = ScopeKey {
        crate_key: other,
        module_path: vec![],
        owner: None,
    };
    let mut graph = MacroNamespaces::default();
    let def = MacroDefinition {
        identity: Digest([23; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    assert!(
        graph
            .define_visible(
                path.clone(),
                def.clone(),
                VisibilityScope::Within {
                    unit: other,
                    module_path: vec![]
                }
            )
            .is_err()
    );
    assert_eq!(graph.lookup(&path).unwrap(), PathLookup::Absent);
    graph
        .define_visible(
            path.clone(),
            def,
            VisibilityScope::Within {
                unit,
                module_path: vec![],
            },
        )
        .unwrap();
    let producer = Digest([24; 32]);
    graph.pending(path.clone(), producer).unwrap();
    assert_eq!(
        graph.lookup_from(&path, &caller).unwrap(),
        PathLookup::NeedInput(producer)
    );
    graph.publish(path.clone(), producer, vec![]).unwrap();
    assert_eq!(
        graph.lookup_from(&path, &caller).unwrap(),
        PathLookup::Inaccessible(vec![path])
    );
}
