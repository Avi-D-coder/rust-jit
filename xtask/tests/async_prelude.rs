use lrr::{
    core::digest::Digest,
    facade::prelude::{AsyncTrait, AsyncTraitCatalog},
    frontend::names::{NameDefinition, NameLookup, NameNamespaces},
    source::manifest::Namespace,
    syntax::{outline::ScopeKey, visibility::VisibilityScope},
};
fn setup() -> (NameNamespaces, AsyncTraitCatalog, ScopeKey) {
    let mut g = NameNamespaces::default();
    let caller = ScopeKey {
        crate_key: Digest([241; 32]),
        module_path: vec![],
        owner: None,
    };
    g.declare_module(caller.crate_key, &[], 2024, VisibilityScope::Public)
        .unwrap();
    let catalog = AsyncTraitCatalog::new();
    catalog.install(&mut g).unwrap();
    (g, catalog, caller)
}
#[test]
fn async_prelude_edition_sets_preserve_canonical_core_and_std_identities() {
    let (g, c, caller) = setup();
    for edition in [2015, 2018, 2021, 2024] {
        let p = c.prelude_scope(edition, false).unwrap();
        let s = c.prelude_scope(edition, true).unwrap();
        for kind in AsyncTrait::ALL {
            for prelude in [&p, &s] {
                let found = g
                    .lookup_unqualified(
                        Namespace::Type,
                        kind.name(),
                        &caller,
                        edition,
                        Some(prelude),
                    )
                    .unwrap();
                if kind.in_prelude(edition) {
                    assert_eq!(found, NameLookup::Defined(c.definition(kind)));
                } else {
                    assert_eq!(found, NameLookup::Absent);
                }
            }
            for unit in [c.core_unit(), c.std_unit()] {
                let path =
                    NameNamespaces::path(unit, &[kind.module(), kind.name()], edition).unwrap();
                assert_eq!(
                    g.lookup(Namespace::Type, &path, &caller).unwrap(),
                    NameLookup::Defined(c.definition(kind))
                );
            }
        }
    }
    assert_eq!(
        c.prelude_scope(2027, false).unwrap_err().code,
        "LRR-E-TOOLCHAIN-CAPABILITY"
    );
}
#[test]
fn selected_async_facade_execution_refuses_and_provenance_spoofing_is_not_ordinary() {
    let (g, c, caller) = setup();
    let p = c.prelude_scope(2024, true).unwrap();
    for kind in AsyncTrait::ALL {
        let NameLookup::Defined(d) = g
            .lookup_unqualified(Namespace::Type, kind.name(), &caller, 2024, Some(&p))
            .unwrap()
        else {
            panic!("async identity absent")
        };
        assert_eq!(c.select(&d).unwrap(), Some(kind));
        assert_eq!(
            c.require_execution(&d).unwrap_err().code,
            "LRR-E-FEATURE-ASYNC"
        );
        let mut spoof = d;
        spoof.defining_unit = caller.crate_key;
        assert_eq!(c.select(&spoof).unwrap_err().code, "LRR-E-FACADE-IDENTITY");
    }
}
#[test]
fn user_declarations_and_manual_globs_shadow_async_prelude_without_facade_admission() {
    let (mut g, c, caller) = setup();
    let prelude = c.prelude_scope(2024, false).unwrap();
    for (i, kind) in AsyncTrait::ALL.into_iter().enumerate() {
        let ordinary = NameDefinition {
            namespace: Namespace::Type,
            identity: Digest([i as u8 + 100; 32]),
            defining_unit: caller.crate_key,
            definition_edition: 2024,
        };
        let local = NameNamespaces::path(caller.crate_key, &[kind.name()], 2024).unwrap();
        if i % 2 == 0 {
            g.define(local, ordinary.clone(), VisibilityScope::Public)
                .unwrap();
        } else {
            g.declare_module(
                caller.crate_key,
                &["exports"],
                2024,
                VisibilityScope::Public,
            )
            .unwrap();
            let target =
                NameNamespaces::path(caller.crate_key, &["exports", kind.name()], 2024).unwrap();
            g.define(target.clone(), ordinary.clone(), VisibilityScope::Public)
                .unwrap();
            g.glob_member(Namespace::Type, local, target, VisibilityScope::Public)
                .unwrap();
        }
        assert_eq!(
            g.lookup_unqualified(Namespace::Type, kind.name(), &caller, 2024, Some(&prelude))
                .unwrap(),
            NameLookup::Defined(ordinary.clone())
        );
        assert_eq!(c.select(&ordinary).unwrap(), None);
        c.require_execution(&ordinary).unwrap();
    }
}
#[test]
fn async_facade_aliases_preserve_identity_and_unknown_producers_precede_prelude_fallback() {
    let (mut g, c, caller) = setup();
    let prelude = c.prelude_scope(2024, true).unwrap();
    let original = NameNamespaces::path(c.std_unit(), &["future", "Future"], 2024).unwrap();
    let alias = NameNamespaces::path(caller.crate_key, &["Renamed"], 2024).unwrap();
    g.named_import(
        Namespace::Type,
        alias.clone(),
        original,
        VisibilityScope::Public,
    )
    .unwrap();
    let NameLookup::Defined(d) = g.lookup(Namespace::Type, &alias, &caller).unwrap() else {
        panic!("alias absent")
    };
    assert_eq!(c.select(&d).unwrap(), Some(AsyncTrait::Future));
    assert_eq!(
        c.require_execution(&d).unwrap_err().code,
        "LRR-E-FEATURE-ASYNC"
    );
    let path = NameNamespaces::path(caller.crate_key, &["Future"], 2024).unwrap();
    g.pending(Namespace::Type, path, Digest([199; 32])).unwrap();
    assert_eq!(
        g.lookup_unqualified(Namespace::Type, "Future", &caller, 2024, Some(&prelude))
            .unwrap(),
        NameLookup::NeedInput(Digest([199; 32]))
    );
    assert_eq!(
        g.lookup_unqualified(Namespace::Value, "Future", &caller, 2024, Some(&prelude))
            .unwrap(),
        NameLookup::Absent
    );
}
