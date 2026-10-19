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
fn differing_visibility_for_same_import_route_retains_both_records() {
    let (mut registry, unit, a, _, dest) = fixture_registry();
    registry
        .imported_alias(dest.clone(), a.clone(), VisibilityScope::Public)
        .unwrap();
    let private = VisibilityScope::Within {
        unit,
        module_path: vec![],
    };
    registry
        .imported_alias(dest.clone(), a.clone(), private.clone())
        .unwrap();
    let Binding::ImportedAliases(aliases) = registry.bindings.get(&dest).unwrap() else {
        panic!()
    };
    assert_eq!(aliases.len(), 2);
    assert!(aliases.iter().any(|row| row.visibility == private));
    assert!(
        aliases
            .iter()
            .any(|row| row.visibility == VisibilityScope::Public)
    );
    assert_eq!(registry.imported_aliases, 2);
    assert!(matches!(registry.resolve_module(&dest, None).unwrap(),
        ModuleResolution::Known { path, .. } if path == a));
}

#[test]
fn completed_source_routes_to_one_module_keep_both_imports() {
    // Same Type/Value import configuration as unchanged core/src/slice/mod.rs
    // and slice/index.rs. No raw-path equality substitutes for module selection.
    let source = "pub mod range {}
mod slice {
 use crate::range;
 pub mod index { use crate::range; pub fn range() {} }
 pub use self::index::range;
}
fn main() {}";
    for edition in [2015, 2018, 2021, 2024] {
        let (provider, _, _) =
            crate::expand::source_declarative::assembly::tests::world(source, edition);
        let routes = crate::frontend::source_macro_routes::SourceMacroRoutes::from_completed_index(
            provider.module_source().outline(),
            provider.macro_index(),
            &BTreeMap::new(),
            &[],
        )
        .unwrap();
        let graph = crate::frontend::source_macro_routes::SourceMacroRoutes::discovery_graph(&[
            std::sync::Arc::new(routes),
        ])
        .unwrap();
        let unit = provider.module_source().outline().root_scope.crate_key;
        let destination = path(unit, &["slice", "range"]);
        let canonical = path(unit, &["range"]);
        let Binding::ImportedAliases(aliases) =
            graph.module_paths.bindings.get(&destination).unwrap()
        else {
            panic!()
        };
        assert_eq!(aliases.len(), 2);
        assert_ne!(aliases[0].target, aliases[1].target);
        assert!(
            aliases
                .iter()
                .all(|alias| alias.canonical.as_ref() == Some(&canonical))
        );
        assert!(
            matches!(graph.module_paths.resolve_module(&destination, None).unwrap(),
            ModuleResolution::Known { path, .. } if path == canonical)
        );
    }
}

#[test]
fn root_extern_prelude_is_shared_by_file_catalogs_and_local_modules_still_win() {
    use crate::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        db::key::OutlineKey,
        expand::source_declarative::SourceDeclarative,
        frontend::source_macro_routes::SourceMacroRoutes,
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    for edition in [2015, 2018, 2021, 2024] {
        for shadow in [false, true] {
            let unit = Digest::of_bytes(b"actual multi-file root extern source control");
            let profile = ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            };
            let cfg = CfgSet::guest_default();
            let root = "extern crate self as renamed; pub mod target {} mod child;";
            let child = if shadow {
                "pub mod renamed { pub mod target {} } pub use renamed::target as picked;"
            } else {
                "pub use renamed as whole; pub use renamed::target as picked;"
            };
            let mut units = Vec::new();
            let mut selections = Vec::new();
            for (path, module, text) in [
                ("root.rs", vec![], root),
                ("child.rs", vec!["child".into()], child),
            ] {
                let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
                let path = SourcePath::new(path).unwrap();
                selections.push(SourceSelection {
                    path: path.clone(),
                    outline: OutlineKey {
                        parse: ParseKey {
                            content: blob.content(),
                            profile: profile.clone(),
                        },
                        crate_key: unit,
                        module_path: module.clone(),
                        cfg: cfg.fingerprint(),
                    },
                });
                units.push(SourceUnitInput {
                    path,
                    crate_key: unit,
                    module_path: module,
                    profile: profile.clone(),
                    cfg: cfg.clone(),
                    blob,
                });
            }
            let inputs = PreparedSourceInputs::refresh(None, units, BTreeMap::new()).unwrap();
            let mut routes = Vec::new();
            for selection in selections {
                let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap()
                else {
                    panic!()
                };
                let provider = SourceDeclarative::new(view).unwrap();
                routes.push(std::sync::Arc::new(
                    SourceMacroRoutes::from_completed_index(
                        provider.module_source().outline(),
                        provider.macro_index(),
                        &BTreeMap::new(),
                        &[],
                    )
                    .unwrap(),
                ));
            }
            let graph = SourceMacroRoutes::discovery_graph(&routes).unwrap();
            let destination = path(unit, &["child", "picked"]);
            let expected = if shadow && edition >= 2018 {
                path(unit, &["child", "renamed", "target"])
            } else {
                path(unit, &["target"])
            };
            assert!(
                matches!(graph.module_paths.resolve_module(&destination, None).unwrap(),
                ModuleResolution::Known { path, .. } if path == expected),
                "{edition} shadow={shadow}"
            );
        }
    }
}
