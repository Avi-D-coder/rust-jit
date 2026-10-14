//! Genuine cold preparation/current facade controls; no full original execution.
use super::*;
use crate::{
    expand::{
        identity::MacroIdentity,
        source_declarative::{SourceDeclarative, SourceDeclarativeDemand},
    },
    frontend::{
        compile_input_catalog::CompileInputCatalog,
        names::{NameLookup, NameNamespaces},
        source_macro_routes::SourceMacroRoutes,
    },
    source::dependencies::SourceDependency,
    syntax::{outline::ScopeKey, token_wire::TokenTree},
};
const AUX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/auxiliary/two_macros.rs"
);
pub(super) fn original_m4_selection(edition: u16) {
    let unit = Digest::of_bytes(b"original macro shadow fixture");
    let auxiliary = Digest::of_bytes(b"original two_macros auxiliary");
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let mut units = Vec::new();
    let mut selections = Vec::new();
    for (text, path, key) in [
        (SHADOW_BUILTIN, "shadow_builtin_macros.rs", unit),
        (AUX, "auxiliary/two_macros.rs", auxiliary),
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
                crate_key: key,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        });
        units.push(SourceUnitInput {
            path,
            crate_key: key,
            module_path: vec![],
            profile: profile.clone(),
            cfg: cfg.clone(),
            blob,
        });
    }
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        units,
        Default::default(),
        vec![SourceDependency {
            consumer: unit,
            alias: "two_macros".into(),
            provider: auxiliary,
        }],
    )
    .unwrap();
    let views: Vec<_> = selections
        .iter()
        .map(|s| {
            let SourceSelectionOutcome::Selected(v) = inputs.select(s).unwrap() else {
                panic!("original source absent")
            };
            v
        })
        .collect();
    let root = &views[0];
    let call = root
        .outline()
        .item_invocations
        .iter()
        .find(|c| c.scope.module_path == ["m4"])
        .unwrap();
    let selected = root
        .outline()
        .items
        .iter()
        .find(|i| i.scope.module_path == ["m4"] && i.def_key.declared_name == "panic")
        .unwrap();
    let mut providers: Vec<_> = views
        .iter()
        .map(|v| SourceDeclarative::new(v.share()).unwrap())
        .collect();
    SourceDeclarative::configure_captured_modules(&mut providers).unwrap();
    // The selected m4 invocation has an explicit local macro in the original;
    // a singleton index can already resolve that local identity. This does not
    // complete the other modules' import/producers or certify a full fixture.
    let root_hygiene = crate::expand::source_hygiene::source_root(unit, edition).unwrap();
    let facts =
        crate::frontend::hygiene::HygieneFacts::new(root_hygiene.contexts, root_hygiene.rows)
            .unwrap()
            .with_name_contexts(root_hygiene.names)
            .unwrap();
    let singleton = crate::frontend::source_macros::SourceMacroIndex::from_outline(
        root.outline(),
        root_hygiene.context,
        facts,
    )
    .unwrap();
    let singleton_selected = singleton
        .lookup(&["m4".into()], u64::from(call.range.start), "panic")
        .unwrap();
    let crate::frontend::macro_scopes::HygienicLookup::Selection(
        crate::frontend::macro_origins::MacroSelection::Defined(local),
    ) = &singleton_selected
    else {
        panic!("original explicit local macro not selected: {singleton_selected:?}")
    };
    assert_eq!(local.identity, selected.def_key.fingerprint());
    assert_eq!(local.defining_unit, unit);
    assert_eq!(local.definition_edition, edition);
    // Actual captured auxiliary inputs can resolve this name without claiming
    // completeness of the original's other modules/ambiguities.
    let raw = providers[0]
        .macro_index()
        .lookup(&["m4".into()], u64::from(call.range.start), "panic")
        .unwrap();
    let crate::frontend::macro_scopes::HygienicLookup::Selection(
        crate::frontend::macro_origins::MacroSelection::Defined(actual),
    ) = raw
    else {
        panic!(
            "genuine captured auxiliary did not establish the exact selected local macro: {raw:?}"
        )
    };
    assert_eq!(actual.identity, selected.def_key.fingerprint());
    assert_eq!(actual.defining_unit, unit);
    assert_eq!(actual.definition_edition, edition);
    let mut routes = Vec::new();
    for provider in &providers {
        routes.extend(provider.routes_in(&providers).unwrap());
    }
    let names = SourceMacroRoutes::discovery_graph(&routes).unwrap();
    // This existing API explicitly mints only a pure unsealed preparation
    // candidate. Genuine source tokens/definition ownership remain required;
    // this request is not a CurrentConstantMacroProof or a scope seal.
    let SourceDeclarativeDemand::Ready(request) = providers[0]
        .demand_for_discovery(
            call.item_input,
            Digest::of_bytes(b"original pure prep"),
            Digest::of_bytes(b"pinned declarative source engine"),
            &names,
            &providers,
        )
        .unwrap()
    else {
        panic!("genuine m4 preparation needs an unestablished operand")
    };
    assert_eq!(
        request.key().macro_identity,
        MacroIdentity::Declaration {
            definition: selected.def_key.fingerprint()
        }
    );
    assert_eq!(request.key().definition_edition, edition);
    assert_eq!(request.key().call_edition, edition);
    let definition = providers[0]
        .macro_index()
        .definition(selected.def_key.fingerprint())
        .unwrap();
    assert_eq!(definition.defining_unit, unit);
    assert_eq!(definition.definition_edition, edition);
    assert_eq!(
        crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard(&profile)
            .unwrap()
            .select(definition)
            .unwrap(),
        None
    );
    let expanded = request.expand().unwrap();
    assert_eq!(expanded.key(), request.key());
    let output = expanded.tokens().unwrap();
    assert!(matches!(output.roots(),[TokenTree::Group{children,..}] if children.is_empty()));
    // Expansion evidence does not retire a different/broader producer, publish
    // a full source seal, or authorize intrinsic Panic semantics.
    let after = providers[0]
        .macro_index()
        .lookup(&["m4".into()], u64::from(call.range.start), "panic")
        .unwrap();
    assert!(
        matches!(after,crate::frontend::macro_scopes::HygienicLookup::Selection(
        crate::frontend::macro_origins::MacroSelection::Defined(found)) if found == actual)
    );
    assert_eq!(
        singleton
            .lookup(&["m4".into()], u64::from(call.range.start), "panic")
            .unwrap(),
        singleton_selected
    );
}
#[test]
fn fixed_type_panic_module_is_distinct_from_macro_and_custom_catalogue_has_no_grant() {
    for edition in [2015, 2018, 2021, 2024] {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let catalog = CompileInputCatalog::standalone(&profile).unwrap();
        let mut graph = NameNamespaces::default();
        catalog.install_ordinary_module_facets(&mut graph).unwrap();
        let runtime = catalog.runtime_catalog().unwrap().unwrap();
        for (_, unit) in catalog.facade_aliases() {
            let caller = ScopeKey {
                crate_key: unit,
                module_path: vec![],
                owner: None,
            };
            let NameLookup::Defined(module) = graph
                .lookup_unqualified(Namespace::Type, "panic", &caller, edition, None)
                .unwrap()
            else {
                panic!("explicit synthetic Type module absent")
            };
            assert_eq!(module.namespace, Namespace::Type);
            assert_eq!(module.defining_unit, unit);
            assert_eq!(module.definition_edition, edition);
            assert_eq!(
                runtime
                    .select(&crate::expand::scopes::MacroDefinition {
                        identity: module.identity,
                        defining_unit: unit,
                        definition_edition: edition
                    })
                    .unwrap(),
                None
            );
            assert_eq!(
                graph
                    .lookup_unqualified(Namespace::Value, "panic", &caller, edition, None)
                    .unwrap(),
                NameLookup::Absent
            );
        }
        let custom =
            CompileInputCatalog::new(Digest::of_bytes(b"unconfigured custom facet"), edition, [])
                .unwrap();
        let mut empty = NameNamespaces::default();
        custom.install_ordinary_module_facets(&mut empty).unwrap();
        let caller = ScopeKey {
            crate_key: crate::facade::unit(&profile, "std"),
            module_path: vec![],
            owner: None,
        };
        assert_eq!(
            empty
                .lookup_unqualified(Namespace::Type, "panic", &caller, edition, None)
                .unwrap(),
            NameLookup::Absent
        );
    }
}
#[test]
fn explicit_standard_module_macro_rename_retains_real_type_route_under_source_policy() {
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "extern crate std;use std::panic as p;const X:()=p!(\"renamed\");fn main(){}",
            "#![no_std]extern crate core;use core::panic as p;const X:()=p!(\"core\");fn main(){}",
            "#![no_implicit_prelude]extern crate std;use std::panic as p;const X:()=p!(\"explicit\");fn main(){}",
        ] {
            let (db, view, _, base, proof) = proof(text, "X", edition);
            let current = ConstantSourceAdmission::capture_current_with_macro_proofs(
                &db,
                &view,
                base.plan().owner_definition(),
                ConstantRequest::Constant,
                MAX_BYTES,
                &[proof.clone()],
            )
            .unwrap();
            current.revalidate(&db, &view).unwrap();
            let type_key=current.dependencies().keys().find(|k|matches!(k,InputKey::DiscoverySeal(s) if matches!(&s.domain,SealDomain::Scope{namespace:Namespace::Type,..}))).unwrap();
            let published = db.published_input(type_key).unwrap();
            let InputValue::DiscoverySeal(seal) = published.0 else {
                panic!("actual Type seal absent")
            };
            let certified = seal.scope.as_ref().unwrap();
            let crate::frontend::names::NameLookup::Defined(module) = certified
                .lookup(&crate::db::key::BucketKey {
                    scope: certified.scope.clone(),
                    namespace: Namespace::Type,
                    name: "p".into(),
                })
                .unwrap()
            else {
                panic!("actual ordinary module alias p absent")
            };
            assert_ne!(module.identity, proof.definition());
            let Node::AdmittedPanic(panic) = selected_node(&current).unwrap() else {
                panic!("exact Macro proof not retained")
            };
            assert_eq!(panic.definition(), proof.definition());
            // No fake value/Type absence or blanket macro-only import success.
        }
    }
}
#[test]
fn renamed_proof_refuses_changed_source_cfg_and_epoch_without_query_work() {
    let text = "use std::panic as p;const X:()=p!(\"retained\");fn main(){}";
    let (mut db, view, _, _, admitted) = proof(text, "X", 2024);
    let (_, shadow, _) = world(
        "macro_rules! p{()=>{()}}const X:()=p!();fn main(){}",
        "X",
        2024,
        true,
    );
    assert!(admitted.revalidate(&db, &shadow).is_err());
    let selection = view.selection();
    let changed_cfg = CfgSet::default();
    assert_ne!(changed_cfg.fingerprint(), selection.outline.cfg);
    let changed_selection = SourceSelection {
        path: selection.path.clone(),
        outline: OutlineKey {
            cfg: changed_cfg.fingerprint(),
            ..selection.outline.clone()
        },
    };
    let changed = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path: selection.path.clone(),
            crate_key: selection.outline.crate_key,
            module_path: selection.outline.module_path.clone(),
            profile: selection.outline.parse.profile.clone(),
            cfg: changed_cfg,
            blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(changed) = changed.select(&changed_selection).unwrap()
    else {
        panic!("actual cfg source capture absent")
    };
    assert!(admitted.revalidate(&db, &changed).is_err());
    let before = db.trace.counts.clone();
    db.begin_expansion_epoch(2).unwrap();
    assert!(admitted.revalidate(&db, &view).is_err());
    assert_eq!(db.trace.counts, before);
    let profile = ProfileIdentity {
        edition: 2099,
        ..ProfileIdentity::current()
    };
    assert!(CompileInputCatalog::standalone(&profile).is_err());
}

#[test]
fn actual_source_std_module_shadow_keeps_ordinary_type_alias_identity() {
    let text = "mod std{pub mod panic{}}use std::panic as p;const X:()=();fn main(){}";
    for edition in [2015, 2018, 2021, 2024] {
        let (db, view, definition) = world(text, "X", edition, true);
        let current = admit(&db, &view, definition).unwrap();
        let expected = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "panic" && i.scope.module_path == ["std"])
            .unwrap()
            .def_key
            .fingerprint();
        let key=current.dependencies().keys().find(|k|matches!(k,InputKey::DiscoverySeal(s) if matches!(&s.domain,SealDomain::Scope{namespace:Namespace::Type,..}))).unwrap();
        let published = db.published_input(key).unwrap();
        let InputValue::DiscoverySeal(seal) = published.0 else {
            panic!("actual Type scope absent")
        };
        let scope = seal.scope.as_ref().unwrap();
        let NameLookup::Defined(module) = scope
            .lookup(&crate::db::key::BucketKey {
                scope: scope.scope.clone(),
                namespace: Namespace::Type,
                name: "p".into(),
            })
            .unwrap()
        else {
            panic!("original local std Type module was replaced")
        };
        assert_eq!(module.identity, expected);
        assert_eq!(module.defining_unit, view.selection().outline.crate_key);
        let runtime = CompileInputCatalog::standalone(&view.selection().outline.parse.profile)
            .unwrap()
            .runtime_catalog()
            .unwrap()
            .unwrap();
        assert_eq!(
            runtime
                .select(&crate::expand::scopes::MacroDefinition {
                    identity: module.identity,
                    defining_unit: module.defining_unit,
                    definition_edition: module.definition_edition
                })
                .unwrap(),
            None
        );
        current.revalidate(&db, &view).unwrap();
    }
}

#[test]
fn compiler_injected_import_is_policy_and_owner_bound_not_a_cargo_alias() {
    use crate::{
        expand::scopes::MacroName, frontend::macro_names::MacroNamespaces,
        syntax::imports::ImportSegment,
    };
    for edition in [2015, 2018, 2021, 2024] {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let catalog = CompileInputCatalog::standalone(&profile).unwrap();
        for (attrs, injected) in [
            ("", Some("std")),
            ("#![no_std]", Some("core")),
            ("#![no_implicit_prelude]", None),
        ] {
            let (_, view, _) = world(
                &format!("{attrs}const X:u8=1;fn main(){{}}"),
                "X",
                edition,
                true,
            );
            let caller = ScopeKey {
                crate_key: view.selection().outline.crate_key,
                module_path: vec![],
                owner: None,
            };
            let mut names = MacroNamespaces::default();
            for (alias, provider) in catalog.facade_aliases() {
                names
                    .dependency(
                        caller.crate_key,
                        MacroName::parse(alias, edition).unwrap(),
                        provider,
                    )
                    .unwrap();
            }
            catalog.install_injected_import(&mut names, &view).unwrap();
            for (alias, provider) in catalog.facade_aliases() {
                let parts = [
                    ImportSegment::Name(alias.into()),
                    ImportSegment::Name("panic".into()),
                ];
                for absolute in [false, true] {
                    let path = names
                        .import_segments_path(&parts, absolute, &caller, edition)
                        .unwrap()
                        .unwrap();
                    if edition == 2015 {
                        assert_eq!(
                            path.unit,
                            if injected == Some(alias) {
                                provider
                            } else {
                                caller.crate_key
                            }
                        );
                    }
                }
            }
            let mut custom = MacroNamespaces::default();
            let foreign = Digest::of_bytes(b"captured Cargo unit named std is not compiler-owned");
            custom
                .dependency(
                    caller.crate_key,
                    MacroName::parse("std", edition).unwrap(),
                    foreign,
                )
                .unwrap();
            catalog.install_injected_import(&mut custom, &view).unwrap();
            let parts = [
                ImportSegment::Name("std".into()),
                ImportSegment::Name("panic".into()),
            ];
            let path = custom
                .import_segments_path(&parts, false, &caller, edition)
                .unwrap()
                .unwrap();
            assert_eq!(
                path.unit,
                if edition == 2015 {
                    caller.crate_key
                } else {
                    foreign
                }
            );
            if edition == 2015 {
                let path = custom
                    .source_segments_path(&parts, true, &caller, edition)
                    .unwrap()
                    .unwrap();
                assert_eq!(path.unit, caller.crate_key);
            }
            let mut shadow = MacroNamespaces::default();
            let std = catalog
                .facade_aliases()
                .find(|(name, _)| *name == "std")
                .unwrap()
                .1;
            shadow
                .dependency(
                    caller.crate_key,
                    MacroName::parse("std", edition).unwrap(),
                    std,
                )
                .unwrap();
            shadow
                .module_paths
                .declare(
                    crate::frontend::module_paths::ModulePath {
                        unit: caller.crate_key,
                        names: vec![],
                    },
                    crate::syntax::visibility::VisibilityScope::Public,
                )
                .unwrap();
            shadow
                .module_paths
                .declare(
                    crate::frontend::module_paths::ModulePath {
                        unit: caller.crate_key,
                        names: vec![MacroName::parse("std", edition).unwrap()],
                    },
                    crate::syntax::visibility::VisibilityScope::Public,
                )
                .unwrap();
            catalog.install_injected_import(&mut shadow, &view).unwrap();
            let path = shadow
                .import_segments_path(&parts, false, &caller, edition)
                .unwrap()
                .unwrap();
            assert_eq!(path.unit, caller.crate_key);
        }
    }
}
