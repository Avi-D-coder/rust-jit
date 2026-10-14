use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    expand::scopes::MacroName,
    frontend::{
        hygiene::HygieneFacts, macro_names::ModuleLookup, source_macro_routes::SourceMacroRoutes,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        outline::{Outline, ScopeKey},
        parse::ParsedFile,
        token_wire::{
            ContextRecord, Edition, Key, NameContextKey, NameResolutionContext, ResolutionContext,
            Transparency,
        },
        visibility::VisibilityScope,
    },
};
use std::collections::BTreeMap;
fn outline(unit: Digest, source: &str) -> Outline {
    ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .outline(unit, vec![], &CfgSet::default())
    .unwrap()
}
fn catalog(
    o: &Outline,
    deps: &BTreeMap<MacroName, Digest>,
) -> Result<SourceMacroRoutes, lrr::core::diagnostic::Diagnostic> {
    let row = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key(o.root_scope.crate_key.0),
        edition: Edition::E2024,
    };
    let root = row.key();
    let facts = HygieneFacts::new(
        vec![row],
        vec![ResolutionContext {
            original: root,
            item: root,
            local: root,
        }],
    )
    .unwrap()
    .with_name_contexts(vec![NameResolutionContext {
        original: root,
        item: NameContextKey(Key([7; 32])),
        local: NameContextKey(Key([7; 32])),
    }])
    .unwrap();
    SourceMacroRoutes::from_outline(o, root, facts, deps)
}
#[test]
fn direct_module_lookup_returns_canonical_identity_visibility_and_typed_absence() {
    let unit = Digest([82; 32]);
    let external = ScopeKey {
        crate_key: Digest([83; 32]),
        module_path: vec![],
        owner: None,
    };
    let source = outline(
        unit,
        "mod hidden { pub mod public {} pub(crate) mod local {} } pub use hidden::public as escape; use hidden::local as local_alias;",
    );
    let graph =
        SourceMacroRoutes::candidate_graph(&[catalog(&source, &Default::default()).unwrap()])
            .unwrap();
    let names = |parts: &[&str]| {
        parts
            .iter()
            .map(|p| MacroName::parse(p, 2024).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        graph
            .lookup_module_from(unit, &names(&["escape"]), &external)
            .unwrap(),
        ModuleLookup::Defined {
            unit,
            components: names(&["hidden", "public"]),
            visibility: VisibilityScope::Public
        }
    );
    assert!(matches!(
        graph
            .lookup_module_from(unit, &names(&["hidden", "public"]), &external)
            .unwrap(),
        ModuleLookup::Inaccessible { .. }
    ));
    let local = ScopeKey {
        crate_key: unit,
        module_path: vec![],
        owner: None,
    };
    assert_eq!(
        graph
            .lookup_module_from(unit, &names(&["local_alias"]), &local)
            .unwrap(),
        ModuleLookup::Defined {
            unit,
            components: names(&["hidden", "local"]),
            visibility: VisibilityScope::Within {
                unit,
                module_path: vec![]
            }
        }
    );
    assert_eq!(
        graph.lookup_module_from(unit, &[], &external).unwrap(),
        ModuleLookup::Defined {
            unit,
            components: vec![],
            visibility: VisibilityScope::Public
        }
    );
    assert_eq!(
        graph
            .lookup_module_from(unit, &names(&["missing"]), &external)
            .unwrap(),
        ModuleLookup::Absent
    );
    assert_eq!(
        graph
            .lookup_module_from(Digest([84; 32]), &[], &external)
            .unwrap(),
        ModuleLookup::Absent
    );
    assert!(
        graph
            .lookup_module_from(
                unit,
                &vec![MacroName::parse("m", 2024).unwrap(); 257],
                &external
            )
            .is_err()
    );
}

#[test]
fn module_globs_forward_canonical_members_shadow_weak_routes_and_keep_ambiguity() {
    let unit = Digest([86; 32]);
    let source = outline(
        unit,
        "#[macro_export] macro_rules! one {()=>{17};} #[macro_export] macro_rules! two {()=>{31};} mod a {pub mod member {pub use crate::one as call;}} mod b {pub mod member {pub use crate::two as call;}} pub mod forwarded {pub use crate::a::*;} pub mod chain {pub use crate::forwarded::*;} pub mod explicit {pub use crate::a::*; pub use crate::b::member;} pub mod conflict {pub use crate::a::*; pub use crate::b::*;} pub use chain::member as named;",
    );
    let graph =
        SourceMacroRoutes::candidate_graph(&[catalog(&source, &Default::default()).unwrap()])
            .unwrap();
    let caller = ScopeKey {
        crate_key: Digest([87; 32]),
        module_path: vec![],
        owner: None,
    };
    let names = |parts: &[&str]| {
        parts
            .iter()
            .map(|p| MacroName::parse(p, 2024).unwrap())
            .collect::<Vec<_>>()
    };
    for (route, target) in [
        (&["forwarded", "member"][..], &["a", "member"][..]),
        (&["chain", "member"], &["a", "member"]),
        (&["named"], &["a", "member"]),
        (&["explicit", "member"], &["b", "member"]),
    ] {
        assert_eq!(
            graph
                .lookup_module_from(unit, &names(route), &caller)
                .unwrap(),
            ModuleLookup::Defined {
                unit,
                components: names(target),
                visibility: VisibilityScope::Public
            }
        );
    }
    assert!(matches!(
        graph
            .lookup_module_from(unit, &names(&["conflict", "member"]), &caller)
            .unwrap(),
        ModuleLookup::Ambiguous(_)
    ));
    let macro_path =
        |parts: &[&str]| lrr::frontend::macro_names::MacroPath::new(unit, names(parts)).unwrap();
    let selected = graph
        .lookup_from(&macro_path(&["chain", "member", "call"]), &caller)
        .unwrap();
    assert_eq!(
        selected,
        graph.lookup_from(&macro_path(&["one"]), &caller).unwrap()
    );
    assert_eq!(
        graph
            .lookup_from(&macro_path(&["explicit", "member", "call"]), &caller)
            .unwrap(),
        graph.lookup_from(&macro_path(&["two"]), &caller).unwrap()
    );
}

#[test]
fn module_globs_intersect_visibility_deduplicate_diamonds_and_settle_cycles() {
    let unit = Digest([88; 32]);
    let source = outline(
        unit,
        "mod source {pub mod visible {} pub(crate) mod local {} mod private {}} pub mod copied {pub use crate::source::*;} pub mod left {pub use crate::copied::*;} pub mod right {pub use crate::copied::*;} pub mod diamond {pub use crate::left::*;pub use crate::right::*;} pub mod cycle_a {pub use crate::cycle_b::*;pub use crate::source::*;} pub mod cycle_b {pub use crate::cycle_a::*;} pub mod empty_a {pub use crate::empty_b::*;} pub mod empty_b {pub use crate::empty_a::*;}",
    );
    let graph =
        SourceMacroRoutes::candidate_graph(&[catalog(&source, &BTreeMap::new()).unwrap()]).unwrap();
    let names = |parts: &[&str]| {
        parts
            .iter()
            .map(|p| MacroName::parse(p, 2024).unwrap())
            .collect::<Vec<_>>()
    };
    let caller = ScopeKey {
        crate_key: Digest([89; 32]),
        module_path: vec![],
        owner: None,
    };
    for path in [
        &["copied", "visible"][..],
        &["diamond", "visible"],
        &["cycle_b", "visible"],
    ] {
        assert_eq!(
            graph
                .lookup_module_from(unit, &names(path), &caller)
                .unwrap(),
            ModuleLookup::Defined {
                unit,
                components: names(&["source", "visible"]),
                visibility: VisibilityScope::Public
            }
        );
    }
    assert!(matches!(
        graph
            .lookup_module_from(unit, &names(&["copied", "local"]), &caller)
            .unwrap(),
        ModuleLookup::Inaccessible { .. }
    ));
    assert_eq!(
        graph
            .lookup_module_from(unit, &names(&["copied", "private"]), &caller)
            .unwrap(),
        ModuleLookup::Absent
    );
    assert_eq!(
        graph
            .lookup_module_from(unit, &names(&["empty_b", "missing"]), &caller)
            .unwrap(),
        ModuleLookup::Absent
    );
    let own = ScopeKey {
        crate_key: unit,
        module_path: vec![],
        owner: None,
    };
    assert_eq!(
        graph
            .lookup_module_from(unit, &names(&["copied", "local"]), &own)
            .unwrap(),
        ModuleLookup::Defined {
            unit,
            components: names(&["source", "local"]),
            visibility: VisibilityScope::Within {
                unit,
                module_path: vec![]
            }
        }
    );
}

#[test]
fn module_globs_preserve_dependency_unit_identity_through_a_renamed_facade() {
    let provider = Digest([92; 32]);
    let facade = Digest([93; 32]);
    let caller = Digest([94; 32]);
    let p = outline(
        provider,
        "#[macro_export] macro_rules! value {()=>{17};} pub mod member {pub use crate::value as call;} pub(crate) mod local {}",
    );
    let f = outline(facade, "pub use renamed_provider::*;");
    let c = outline(
        caller,
        "pub use renamed_facade::*; pub use member as alias;",
    );
    let catalogs = [
        catalog(&p, &BTreeMap::new()).unwrap(),
        catalog(
            &f,
            &BTreeMap::from([(
                MacroName::parse("renamed_provider", 2024).unwrap(),
                provider,
            )]),
        )
        .unwrap(),
        catalog(
            &c,
            &BTreeMap::from([(MacroName::parse("renamed_facade", 2024).unwrap(), facade)]),
        )
        .unwrap(),
    ];
    let graph = SourceMacroRoutes::candidate_graph(&catalogs).unwrap();
    let scope = ScopeKey {
        crate_key: caller,
        module_path: vec![],
        owner: None,
    };
    let names = |parts: &[&str]| {
        parts
            .iter()
            .map(|p| MacroName::parse(p, 2024).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        graph
            .lookup_module_from(caller, &names(&["alias"]), &scope)
            .unwrap(),
        ModuleLookup::Defined {
            unit: provider,
            components: names(&["member"]),
            visibility: VisibilityScope::Public
        }
    );
    assert_eq!(
        graph
            .lookup_module_from(caller, &names(&["local"]), &scope)
            .unwrap(),
        ModuleLookup::Absent
    );
    let path = |unit, parts: &[&str]| {
        lrr::frontend::macro_names::MacroPath::new(unit, names(parts)).unwrap()
    };
    assert_eq!(
        graph
            .lookup_from(&path(caller, &["alias", "call"]), &scope)
            .unwrap(),
        graph
            .lookup_from(&path(provider, &["value"]), &scope)
            .unwrap()
    );
}
