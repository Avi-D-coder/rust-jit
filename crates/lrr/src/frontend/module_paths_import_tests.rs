//! Private registry invariants complement actual original source import controls.
use super::*;
fn path(unit: Digest, names: &[&str]) -> ModulePath {
    ModulePath {
        unit,
        names: names
            .iter()
            .map(|n| MacroName::parse(n, 2024).unwrap())
            .collect(),
    }
}
fn fixture_registry() -> (ModulePaths, Digest, ModulePath, ModulePath, ModulePath) {
    let unit = Digest([98; 32]);
    let a = path(unit, &["a"]);
    let b = path(unit, &["b"]);
    let dest = path(unit, &["alias"]);
    let mut registry = ModulePaths::default();
    for p in [path(unit, &[]), a.clone(), b.clone()] {
        registry.declare(p, VisibilityScope::Public).unwrap();
    }
    (registry, unit, a, b, dest)
}
#[test]
fn actual_import_routes_and_original_origin_retained_exactly_once() {
    let (mut registry, unit, a, b, dest) = fixture_registry();
    registry
        .imported_alias(dest.clone(), a.clone(), VisibilityScope::Public)
        .unwrap();
    let bytes = registry.bytes;
    registry
        .imported_alias(dest.clone(), a.clone(), VisibilityScope::Public)
        .unwrap();
    assert_eq!(bytes, registry.bytes);
    assert_eq!(registry.imported_aliases, 1);
    registry
        .imported_alias(dest.clone(), b.clone(), VisibilityScope::Public)
        .unwrap();
    let bytes = registry.bytes;
    registry
        .imported_alias(dest.clone(), b.clone(), VisibilityScope::Public)
        .unwrap();
    assert_eq!(bytes, registry.bytes);
    let Binding::ImportedAliases(aliases) = registry.bindings.get(&dest).unwrap() else {
        panic!()
    };
    assert_eq!(aliases.len(), 2);
    for alias in aliases {
        assert_eq!(
            alias.origin,
            ScopeKey {
                crate_key: unit,
                module_path: vec![],
                owner: None
            }
        );
    }
    let ModuleResolution::Ambiguous(found) = registry.resolve_module(&dest, None).unwrap() else {
        panic!()
    };
    assert_eq!(
        found.into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([a, b])
    );
}
#[test]
fn compiler_alias_and_declaration_provenance_are_still_strict() {
    let (mut registry, _, a, b, dest) = fixture_registry();
    registry
        .alias(dest.clone(), a.clone(), VisibilityScope::Public)
        .unwrap();
    assert!(
        registry
            .alias(dest.clone(), b.clone(), VisibilityScope::Public)
            .is_err()
    );
    assert!(
        registry
            .imported_alias(dest, b, VisibilityScope::Public)
            .is_err()
    );
    assert!(
        registry
            .imported_alias(a.clone(), a, VisibilityScope::Public)
            .is_err()
    );
}
#[test]
fn competing_private_import_does_not_grant_outside_access() {
    let (mut registry, unit, a, b, dest) = fixture_registry();
    registry
        .imported_alias(dest.clone(), a.clone(), VisibilityScope::Public)
        .unwrap();
    registry
        .imported_alias(
            dest.clone(),
            b,
            VisibilityScope::Within {
                unit,
                module_path: vec![],
            },
        )
        .unwrap();
    let outside = ScopeKey {
        crate_key: Digest([99; 32]),
        module_path: vec![],
        owner: None,
    };
    assert!(
        matches!(registry.resolve_module(&dest,Some(&outside)).unwrap(),ModuleResolution::Known{path,..} if path==a)
    );
}
#[test]
fn imported_alias_still_checks_original_target_reexport_width() {
    let (mut registry, unit, _, _, dest) = fixture_registry();
    let hidden = path(unit, &["private"]);
    registry
        .declare(
            hidden.clone(),
            VisibilityScope::Within {
                unit,
                module_path: vec![],
            },
        )
        .unwrap();
    registry
        .imported_alias(dest.clone(), hidden, VisibilityScope::Public)
        .unwrap();
    let root = ScopeKey {
        crate_key: unit,
        module_path: vec![],
        owner: None,
    };
    assert!(matches!(
        registry.resolve_module(&dest, Some(&root)).unwrap(),
        ModuleResolution::Denied(_)
    ));
}
#[test]
fn metadata_and_count_refusal_install_no_unowned_import_route() {
    let (mut registry, _, a, _, dest) = fixture_registry();
    registry.bytes = 32 * 1024 * 1024;
    let before = registry.bindings.len();
    assert!(
        registry
            .imported_alias(dest.clone(), a, VisibilityScope::Public)
            .is_err()
    );
    assert_eq!(registry.bindings.len(), before);
    assert_eq!(registry.imported_aliases, 0);
    assert!(!registry.bindings.contains_key(&dest));
    assert_eq!(registry.bytes, 32 * 1024 * 1024);
    let (mut registry, _, a, _, dest) = fixture_registry();
    registry.imported_aliases = 100_000;
    let bytes = registry.bytes;
    assert!(
        registry
            .imported_alias(dest.clone(), a, VisibilityScope::Public)
            .is_err()
    );
    assert_eq!(registry.bytes, bytes);
    assert!(!registry.bindings.contains_key(&dest));
}
#[test]
fn differing_visibility_for_same_import_route_is_not_silently_rewritten() {
    let (mut registry, unit, a, _, dest) = fixture_registry();
    registry
        .imported_alias(dest.clone(), a.clone(), VisibilityScope::Public)
        .unwrap();
    let bytes = registry.bytes;
    assert!(
        registry
            .imported_alias(
                dest,
                a,
                VisibilityScope::Within {
                    unit,
                    module_path: vec![]
                }
            )
            .is_err()
    );
    assert_eq!(bytes, registry.bytes);
    assert_eq!(registry.imported_aliases, 1);
}
