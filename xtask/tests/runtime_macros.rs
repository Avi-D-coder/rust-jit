use lrr::{
    core::digest::Digest,
    expand::scopes::{MacroDefinition, MacroName},
    facade::macros::RuntimeMacroKind,
    frontend::{
        macro_names::{MacroPath, PathLookup},
        runtime_macro_catalog::RuntimeMacroCatalog,
        source_macro_routes::SourceMacroRoutes,
    },
};
fn path(unit: Digest, name: &str) -> MacroPath {
    MacroPath::new(unit, vec![MacroName::parse(name, 2024).unwrap()]).unwrap()
}
#[test]
fn runtime_macro_facet_has_six_stable_unit_and_edition_bound_definitions() {
    let unit = Digest([184; 32]);
    let catalog = RuntimeMacroCatalog::new(unit, 2024).unwrap();
    let rebuilt = RuntimeMacroCatalog::new(unit, 2024).unwrap();
    assert_eq!(
        catalog.definitions().collect::<Vec<_>>(),
        rebuilt.definitions().collect::<Vec<_>>()
    );
    let mut names = catalog
        .definitions()
        .map(|d| catalog.select(d).unwrap().unwrap().name())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        [
            "assert",
            "assert_eq",
            "assert_ne",
            "panic",
            "unimplemented",
            "unreachable"
        ]
    );
    for other in [
        RuntimeMacroCatalog::new(Digest([185; 32]), 2024).unwrap(),
        RuntimeMacroCatalog::new(unit, 2021).unwrap(),
    ] {
        assert!(
            other
                .definitions()
                .all(|d| catalog.select(d).unwrap().is_none())
        );
    }
    assert!(RuntimeMacroCatalog::new(unit, 2099).is_err());
}
#[test]
fn runtime_macro_selection_rejects_forged_provenance_and_preserves_user_identity() {
    let unit = Digest([184; 32]);
    let catalog = RuntimeMacroCatalog::new(unit, 2024).unwrap();
    for definition in catalog.definitions() {
        let mut forged = definition.clone();
        forged.defining_unit = Digest([185; 32]);
        assert_eq!(
            catalog.select(&forged).unwrap_err().code,
            "LRR-E-RUNTIME-MACRO"
        );
        let mut forged = definition.clone();
        forged.definition_edition = 2021;
        assert_eq!(
            catalog.select(&forged).unwrap_err().code,
            "LRR-E-RUNTIME-MACRO"
        );
    }
    let user = MacroDefinition {
        identity: Digest([186; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    assert_eq!(catalog.select(&user).unwrap(), None);
}
#[test]
fn runtime_macro_source_routes_preserve_aliases_user_names_and_ambiguous_lookup() {
    let core = Digest([184; 32]);
    let user_unit = Digest([185; 32]);
    let catalog = RuntimeMacroCatalog::new(core, 2024).unwrap();
    use lrr::{
        core::profile::ProfileIdentity,
        frontend::hygiene::HygieneFacts,
        source::snapshot::{CfgSet, SourceBlob},
        syntax::{outline::ScopeKey, parse::ParsedFile, token_wire as runtime},
    };
    use lrr_expand_adapter::{
        hygiene::HygieneStore,
        token_wire::{Edition, Key, TokenForest},
    };
    let source = "pub use core::{r#assert as check, panic as stop, assert_eq as equal, assert_ne as different}; pub use check as confirm; use core::panic as private_stop; pub use private_stop as denied; #[macro_export] macro_rules! assert { () => {}; } #[macro_export] macro_rules! panic { () => {}; } fn case(){confirm!(true, \"unvisited {}\", missing());panic!(custom => grammar);}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(user_unit, vec![], &CfgSet::default())
        .unwrap();
    let mut hygiene = HygieneStore::default();
    let context = hygiene.root(Key(user_unit.0), Edition::E2024).unwrap();
    let rows = hygiene.resolution_contexts(&[context]).unwrap();
    let comparisons = hygiene.name_contexts(&[context]).unwrap();
    let facts = HygieneFacts::new(
        runtime::TokenForest::decode(
            &TokenForest::new(vec![], hygiene.records(), vec![])
                .unwrap()
                .encode()
                .unwrap(),
        )
        .unwrap()
        .contexts()
        .to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&comparisons).unwrap()).unwrap())
    .unwrap();
    let source_routes = SourceMacroRoutes::from_outline(
        &outline,
        runtime::Key(context.0),
        facts,
        &std::collections::BTreeMap::from([(MacroName::parse("core", 2024).unwrap(), core)]),
    )
    .unwrap();
    assert!(source_routes.deferred().is_empty());
    let routes = catalog.source_routes().unwrap();
    let mut names = SourceMacroRoutes::candidate_graph(&[routes, source_routes]).unwrap();
    let caller = ScopeKey {
        crate_key: user_unit,
        module_path: vec![],
        owner: None,
    };
    for user_name in ["assert", "panic"] {
        let PathLookup::Defined(selected) = names
            .lookup_from(&path(user_unit, user_name), &caller)
            .unwrap()
        else {
            panic!("user definition absent")
        };
        assert_eq!(selected.defining_unit, user_unit);
        assert_eq!(catalog.select(&selected).unwrap(), None);
    }
    for (alias, kind) in [
        ("check", RuntimeMacroKind::Assert),
        ("stop", RuntimeMacroKind::Panic),
        ("equal", RuntimeMacroKind::AssertEq),
        ("different", RuntimeMacroKind::AssertNe),
        ("confirm", RuntimeMacroKind::Assert),
    ] {
        let PathLookup::Defined(selected) =
            names.lookup_from(&path(user_unit, alias), &caller).unwrap()
        else {
            panic!(
                "alias definition absent: {alias}: {:?}",
                names.lookup_from(&path(user_unit, alias), &caller).unwrap()
            )
        };
        assert_eq!(catalog.select(&selected).unwrap(), Some(kind));
        assert_eq!(selected.defining_unit, core);
    }
    use lrr::frontend::runtime_macro_plan::{
        MessageTiming, RuntimeMacroPreparation, prepare_runtime_macro,
    };
    use lrr::syntax::spans::SourceRange;
    for (text, name, builtin) in [
        (
            "confirm!(true, \"unvisited {}\", missing())",
            "confirm",
            true,
        ),
        ("panic!(custom => grammar)", "panic", false),
    ] {
        let start = source.find(text).unwrap() as u32;
        let call = parsed
            .macro_call_at(SourceRange {
                start,
                end: start + text.len() as u32,
            })
            .unwrap();
        let lookup = names.lookup_from(&path(user_unit, name), &caller).unwrap();
        match prepare_runtime_macro(&parsed, call, lookup, &catalog).unwrap() {
            RuntimeMacroPreparation::Intrinsic(plan) if builtin => {
                assert_eq!(plan.arguments().kind(), RuntimeMacroKind::Assert);
                assert_eq!(plan.message_timing(), MessageTiming::OnAssertionFailure);
                assert_eq!(plan.arguments().operands().len(), 1);
                assert!(plan.arguments().message().is_some());
            }
            RuntimeMacroPreparation::SourceExpansion { definition, .. } if !builtin => {
                assert_eq!(definition.defining_unit, user_unit);
            }
            _ => panic!("source alias/user preparation differs"),
        }
    }
    let PathLookup::Inaccessible(paths) = names
        .lookup_from(&path(user_unit, "denied"), &caller)
        .unwrap()
    else {
        panic!("private macro reexport must be inaccessible")
    };
    assert_eq!(paths, vec![path(user_unit, "private_stop")]);
    names
        .reexport(path(user_unit, "ambiguous"), path(core, "panic"))
        .unwrap();
    names
        .reexport(path(user_unit, "ambiguous"), path(core, "assert"))
        .unwrap();
    let PathLookup::Ambiguous(candidates) = names
        .lookup_from(&path(user_unit, "ambiguous"), &caller)
        .unwrap()
    else {
        panic!("ambiguity erased")
    };
    assert_eq!(candidates.len(), 2);
    assert!(matches!(
        names
            .lookup_from(&path(user_unit, "absent"), &caller)
            .unwrap(),
        PathLookup::Absent
    ));
}
