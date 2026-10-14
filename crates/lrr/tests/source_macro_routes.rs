use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    expand::scopes::MacroName,
    frontend::{
        hygiene::HygieneFacts,
        macro_names::{MacroPath, PathLookup},
        source_macro_routes::{DeferredImportReason, SourceMacroRoutes},
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        outline::Outline,
        parse::ParsedFile,
        token_wire::{
            ContextRecord, Edition, Key, NameContextKey, NameResolutionContext, ResolutionContext,
            Transparency,
        },
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
fn parsed_export_and_named_import_routes_preserve_identity_across_units_and_scopes() {
    let provider = Digest([1; 32]);
    let facade = Digest([2; 32]);
    let caller = Digest([3; 32]);
    let p = outline(
        provider,
        "mod nested { #[macro_export] macro_rules! r#gen {()=>{17};} } #[cfg(any())] #[macro_export] macro_rules! hidden {()=>{99};}",
    );
    let exported = p
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "gen")
        .unwrap()
        .def_key
        .fingerprint();
    let f = outline(facade, "pub use renamed_provider::r#gen as forward;");
    let c = outline(
        caller,
        "macro_rules! local {()=>{29};} use local as renamed_local; use facade_dep::forward as chosen; mod inner {pub use crate::chosen as nested; mod deep {use super::nested as deep_name;} }",
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
            &BTreeMap::from([(MacroName::parse("facade_dep", 2024).unwrap(), facade)]),
        )
        .unwrap(),
    ];
    assert_eq!(catalogs[0].definitions().len(), 1);
    assert!(catalogs.iter().all(|c| c.deferred().is_empty()));
    let graph = SourceMacroRoutes::candidate_graph(&catalogs).unwrap();
    for route in [
        path(provider, &["r#gen"]),
        path(facade, &["forward"]),
        path(caller, &["chosen"]),
        path(caller, &["inner", "nested"]),
        path(caller, &["inner", "deep", "deep_name"]),
    ] {
        let PathLookup::Defined(def) = graph.lookup(&route).unwrap() else {
            panic!("missing parsed route {route:?}");
        };
        assert_eq!(def.identity, exported);
        assert_eq!(def.defining_unit, provider);
    }
    let PathLookup::Defined(local) = graph.lookup(&path(caller, &["renamed_local"])).unwrap()
    else {
        panic!("missing textual alias")
    };
    assert_eq!(local.defining_unit, caller);
    assert_ne!(local.identity, exported);
}
#[test]
fn source_routes_retain_deferred_producers_globs_module_aliases_and_path_errors() {
    let unit = Digest([4; 32]);
    let o = outline(
        unit,
        "macro_rules! m {()=>{17};} missing!(); use m as alias; use crate::some::*; use crate::some::{self as module_alias}; #[transform] use crate::m as transformed; use crate::m as _;",
    );
    let c = catalog(&o, &BTreeMap::new()).unwrap();
    assert_eq!(c.deferred().len(), 4);
    assert!(matches!(
        c.deferred()[0].reason,
        DeferredImportReason::TextualProducer(_)
    ));
    assert_eq!(c.deferred()[1].reason, DeferredImportReason::Glob);
    assert_eq!(c.deferred()[2].reason, DeferredImportReason::ModuleAlias);
    assert_eq!(
        c.deferred()[3].reason,
        DeferredImportReason::AttributeProducer
    );
    assert!(SourceMacroRoutes::candidate_graph(&[c]).is_err());
    let c = catalog(&o, &BTreeMap::new()).unwrap();
    assert!(c.definitions().is_empty());
    assert!(c.imports().is_empty());
    assert!(catalog(&outline(unit, "use super::m;"), &BTreeMap::new()).is_err());
    assert!(catalog(&outline(unit, "use ::unknown::m;"), &BTreeMap::new()).is_err());
    let dependencies = BTreeMap::from([(MacroName::parse("dep", 2024).unwrap(), Digest([5; 32]))]);
    let o = outline(
        unit,
        "mod dep {pub mod module {}} use dep::m; use ::dep as absolute_module; use crate as own_crate; use crate::dep::module as module_alias;",
    );
    let c = catalog(&o, &dependencies).unwrap();
    assert_eq!(
        c.deferred()
            .iter()
            .map(|d| d.reason.clone())
            .collect::<Vec<_>>(),
        vec![
            DeferredImportReason::DependencyModuleCollision,
            DeferredImportReason::ModuleAlias,
            DeferredImportReason::ModuleAlias,
            DeferredImportReason::ModuleAlias
        ]
    );
    let c = catalog(&outline(unit, "use unknown_module::m;"), &BTreeMap::new()).unwrap();
    assert_eq!(c.deferred()[0].reason, DeferredImportReason::ModuleLookup);
    assert!(MacroPath::new(unit, vec![MacroName::parse("m", 2024).unwrap(); 257]).is_err());
}

#[test]
fn module_aliases_resolve_forward_chains_root_aliases_raw_modules_and_macro_targets() {
    let provider = Digest([8; 32]);
    let consumer = Digest([9; 32]);
    let p = outline(
        provider,
        "#[macro_export] macro_rules! exported {()=>{17};} pub mod tools {pub use crate::exported as member;}",
    );
    let c = outline(
        consumer,
        "use second as first; use third as second; use ::dep as third; use first::tools as renamed; use renamed::member as chosen; use crate as own; mod r#gen {pub use super::chosen as raw_member;} use own::r#gen as raw_module; use raw_module::raw_member as final_name;",
    );
    let catalogs = [
        catalog(&p, &BTreeMap::new()).unwrap(),
        catalog(
            &c,
            &BTreeMap::from([(MacroName::parse("dep", 2024).unwrap(), provider)]),
        )
        .unwrap(),
    ];
    let graph = SourceMacroRoutes::candidate_graph(&catalogs).unwrap();
    let PathLookup::Defined(expected) = graph.lookup(&path(provider, &["exported"])).unwrap()
    else {
        panic!("missing source export")
    };
    for target in [
        path(consumer, &["chosen"]),
        path(consumer, &["r#gen", "raw_member"]),
        path(consumer, &["final_name"]),
    ] {
        assert_eq!(
            graph.lookup(&target).unwrap(),
            PathLookup::Defined(expected.clone())
        );
    }
    assert!(
        SourceMacroRoutes::candidate_graph(&[catalog(
            &outline(
                consumer,
                "use a as b; use b as a; use a::member as unreachable;"
            ),
            &BTreeMap::new()
        )
        .unwrap()])
        .is_err()
    );
}

#[test]
fn module_alias_worklist_handles_long_forward_chains_and_rejects_distinct_targets() {
    let unit = Digest([10; 32]);
    let mut source = String::from(
        "#[macro_export] macro_rules! exported {()=>{17};} mod origin {pub use crate::exported as member;}",
    );
    for index in 0..400 {
        source.push_str(&format!("use a{} as a{index};", index + 1));
    }
    source.push_str("use crate::origin as a400; use a0::member as chosen;");
    let graph =
        SourceMacroRoutes::candidate_graph(&[
            catalog(&outline(unit, &source), &BTreeMap::new()).unwrap()
        ])
        .unwrap();
    assert_eq!(
        graph.lookup(&path(unit, &["chosen"])).unwrap(),
        graph.lookup(&path(unit, &["exported"])).unwrap()
    );
    let c=catalog(&outline(unit,"mod left{} mod right{} use crate::left as alias; use crate::right as alias; use alias::m as chosen;"),&BTreeMap::new()).unwrap();
    assert!(SourceMacroRoutes::candidate_graph(&[c]).is_err());
    let c = catalog(&outline(unit, "use a as b; use b as a;"), &BTreeMap::new()).unwrap();
    assert_eq!(
        SourceMacroRoutes::candidate_graph(&[c])
            .unwrap()
            .lookup(&path(unit, &["a"]))
            .unwrap(),
        PathLookup::Absent
    );
}

#[test]
fn source_macro_binding_visibility_checks_callers_and_each_import_origin() {
    let unit = Digest([11; 32]);
    let other = Digest([12; 32]);
    let source = "#[macro_export] macro_rules! m {()=>{17};} pub mod area {pub mod inner {pub(in crate::area) use crate::m as limited; pub(super) use crate::m as parent; pub(crate) use crate::m as unit_only; use crate::m as private; pub(self) use crate::m as same; pub use crate::m as public; pub use private as escaped;}}";
    let graph =
        SourceMacroRoutes::candidate_graph(&[
            catalog(&outline(unit, source), &BTreeMap::new()).unwrap()
        ])
        .unwrap();
    let scope = |unit, modules: &[&str]| lrr::syntax::outline::ScopeKey {
        crate_key: unit,
        module_path: modules.iter().map(|s| (*s).into()).collect(),
        owner: None,
    };
    for (member, caller, visible) in [
        ("limited", scope(unit, &["area", "sibling"]), true),
        ("limited", scope(unit, &["outside"]), false),
        ("parent", scope(unit, &["area"]), true),
        ("parent", scope(unit, &[]), false),
        ("unit_only", scope(unit, &[]), true),
        ("unit_only", scope(other, &[]), false),
        ("private", scope(unit, &["area", "inner", "child"]), true),
        ("private", scope(unit, &["area"]), false),
        ("same", scope(unit, &["area", "inner"]), true),
        ("same", scope(other, &[]), false),
        ("public", scope(other, &[]), true),
        ("escaped", scope(other, &[]), false),
    ] {
        let result = graph
            .lookup_from(&path(unit, &["area", "inner", member]), &caller)
            .unwrap();
        if visible {
            assert!(
                matches!(result, PathLookup::Defined(_)),
                "{member}: {result:?}"
            );
        } else {
            assert!(
                matches!(result, PathLookup::Inaccessible(_)),
                "{member}: {result:?}"
            );
        }
    }
    let c=catalog(&outline(unit,"#[macro_export] macro_rules! m {()=>{17};} mod hidden {use crate::m as private;} pub use crate::hidden::private as invalid_escape;"),&BTreeMap::new()).unwrap();
    assert!(matches!(
        SourceMacroRoutes::candidate_graph(&[c])
            .unwrap()
            .lookup_from(&path(unit, &["invalid_escape"]), &scope(other, &[]))
            .unwrap(),
        PathLookup::Inaccessible(_)
    ));
}

#[test]
fn source_visibility_restrictions_use_canonical_ancestor_scopes_and_stable_encoding() {
    use lrr::syntax::{
        outline::{ScopeKey, Visibility},
        visibility::VisibilityScope,
    };
    let unit = Digest([31; 32]);
    let o = outline(
        unit,
        "mod café {mod inner {pub(in crate::cafe\u{301}) use crate::m as alias;}} ",
    );
    let import = &o.imports[0];
    let v = VisibilityScope::for_declaration(&import.visibility, &import.scope, 2024).unwrap();
    let caller = ScopeKey {
        crate_key: unit,
        module_path: vec!["cafe\u{301}".into(), "sibling".into()],
        owner: None,
    };
    assert!(v.allows(&caller).unwrap());
    let private =
        VisibilityScope::for_declaration(&Visibility::Private, &import.scope, 2024).unwrap();
    assert!(v.permits_reexport_as(&private));
    assert!(!private.permits_reexport_as(&v));
    assert_ne!(v.fingerprint(), private.fingerprint());
    let wrong = ScopeKey {
        crate_key: unit,
        module_path: vec!["other".into()],
        owner: None,
    };
    assert!(VisibilityScope::for_declaration(&import.visibility, &wrong, 2024).is_err());
    assert!(
        VisibilityScope::for_declaration(
            &Visibility::Super,
            &ScopeKey {
                crate_key: unit,
                module_path: vec![],
                owner: None
            },
            2024
        )
        .is_err()
    );
}

#[test]
fn direct_module_alias_macro_calls_preserve_prefix_privacy_and_public_escape_paths() {
    let unit = Digest([41; 32]);
    let other = Digest([42; 32]);
    let source = "#[macro_export] macro_rules! m {()=>{17};} mod hidden {pub mod visible {pub use crate::m as member;}} pub use crate::hidden::visible as exposed; use crate::hidden::visible as local_alias; pub mod outer {use crate::hidden::visible as inner_alias; pub use inner_alias::member as callable;}";
    let graph =
        SourceMacroRoutes::candidate_graph(&[
            catalog(&outline(unit, source), &BTreeMap::new()).unwrap()
        ])
        .unwrap();
    let caller = |unit| lrr::syntax::outline::ScopeKey {
        crate_key: unit,
        module_path: vec![],
        owner: None,
    };
    assert!(matches!(
        graph
            .lookup_from(&path(unit, &["exposed", "member"]), &caller(other))
            .unwrap(),
        PathLookup::Defined(_)
    ));
    assert!(matches!(
        graph
            .lookup_from(
                &path(unit, &["hidden", "visible", "member"]),
                &caller(other)
            )
            .unwrap(),
        PathLookup::Inaccessible(_)
    ));
    assert!(matches!(
        graph
            .lookup_from(&path(unit, &["local_alias", "member"]), &caller(other))
            .unwrap(),
        PathLookup::Inaccessible(_)
    ));
    assert!(matches!(
        graph
            .lookup_from(&path(unit, &["local_alias", "member"]), &caller(unit))
            .unwrap(),
        PathLookup::Defined(_)
    ));
    assert!(matches!(
        graph
            .lookup_from(&path(unit, &["outer", "callable"]), &caller(other))
            .unwrap(),
        PathLookup::Defined(_)
    ));
    assert!(matches!(
        graph.lookup(&path(unit, &["exposed", "member"])).unwrap(),
        PathLookup::Defined(_)
    ));
    let invalid = outline(unit, "mod hidden{} pub use crate::hidden as exposed;");
    let graph = SourceMacroRoutes::candidate_graph(&[catalog(&invalid, &BTreeMap::new()).unwrap()])
        .unwrap();
    assert!(matches!(
        graph
            .lookup_from(&path(unit, &["exposed", "missing"]), &caller(other))
            .unwrap(),
        PathLookup::Inaccessible(_)
    ));
}

#[test]
fn module_alias_prefixes_cannot_hide_private_macro_reexport_width() {
    let unit = Digest([43; 32]);
    let other = Digest([44; 32]);
    let source = "#[macro_export] macro_rules! m {()=>{17};} pub mod area {use crate::m as private; use crate::area as same_area; pub use same_area::private as invalid; }";
    let graph =
        SourceMacroRoutes::candidate_graph(&[
            catalog(&outline(unit, source), &BTreeMap::new()).unwrap()
        ])
        .unwrap();
    let caller = lrr::syntax::outline::ScopeKey {
        crate_key: other,
        module_path: vec![],
        owner: None,
    };
    assert!(matches!(
        graph
            .lookup_from(&path(unit, &["area", "invalid"]), &caller)
            .unwrap(),
        PathLookup::Inaccessible(_)
    ));
}

#[test]
fn macro_globs_propagate_identity_shadow_weak_candidates_and_keep_ambiguity() {
    let unit = Digest([51; 32]);
    let source = "#[macro_export] macro_rules! one {()=>{17};} #[macro_export] macro_rules! two {()=>{31};} pub mod a {pub use crate::one as chosen;} pub mod b {pub use crate::two as chosen;} pub mod forwarded {pub use crate::a::*;} pub mod collision {pub use crate::a::*;pub use crate::b::*;} pub mod explicit {pub use crate::a::*;pub use crate::b::chosen;} pub mod cyclic_a {pub use crate::cyclic_b::*;pub use crate::a::*;} pub mod cyclic_b {pub use crate::cyclic_a::*;}";
    let graph =
        SourceMacroRoutes::candidate_graph(&[
            catalog(&outline(unit, source), &BTreeMap::new()).unwrap()
        ])
        .unwrap();
    let caller = lrr::syntax::outline::ScopeKey {
        crate_key: Digest([52; 32]),
        module_path: vec![],
        owner: None,
    };
    let first = graph.lookup_from(&path(unit, &["one"]), &caller).unwrap();
    let second = graph.lookup_from(&path(unit, &["two"]), &caller).unwrap();
    for module in ["forwarded", "cyclic_a", "cyclic_b"] {
        assert_eq!(
            graph
                .lookup_from(&path(unit, &[module, "chosen"]), &caller)
                .unwrap(),
            first
        );
    }
    assert_eq!(
        graph
            .lookup_from(&path(unit, &["explicit", "chosen"]), &caller)
            .unwrap(),
        second
    );
    assert!(
        matches!(graph.lookup_from(&path(unit,&["collision","chosen"]),&caller).unwrap(),PathLookup::Ambiguous(defs) if defs.len()==2)
    );
}

#[test]
fn glob_member_visibility_narrows_public_reexports_and_deduplicates_diamonds() {
    let unit = Digest([53; 32]);
    let source = "#[macro_export] macro_rules! m {()=>{17};} pub mod restricted {pub(crate) use crate::m as local;pub use crate::m as visible;} pub mod copied {pub use crate::restricted::*;} pub mod left {pub use crate::restricted::*;} pub mod right {pub use crate::restricted::*;} pub mod diamond {pub use crate::left::*;pub use crate::right::*;} pub mod empty_a {pub use crate::empty_b::*;} pub mod empty_b {pub use crate::empty_a::*;}";
    let graph =
        SourceMacroRoutes::candidate_graph(&[
            catalog(&outline(unit, source), &BTreeMap::new()).unwrap()
        ])
        .unwrap();
    let caller = |unit| lrr::syntax::outline::ScopeKey {
        crate_key: unit,
        module_path: vec![],
        owner: None,
    };
    let outside = caller(Digest([54; 32]));
    assert!(matches!(
        graph
            .lookup_from(&path(unit, &["copied", "local"]), &caller(unit))
            .unwrap(),
        PathLookup::Defined(_)
    ));
    assert!(matches!(
        graph
            .lookup_from(&path(unit, &["copied", "local"]), &outside)
            .unwrap(),
        PathLookup::Inaccessible(_)
    ));
    assert!(matches!(
        graph
            .lookup_from(&path(unit, &["diamond", "visible"]), &outside)
            .unwrap(),
        PathLookup::Defined(_)
    ));
    assert_eq!(
        graph
            .lookup_from(&path(unit, &["empty_a", "missing"]), &outside)
            .unwrap(),
        PathLookup::Absent
    );
    let mut graph = graph;
    let producer = Digest([56; 32]);
    let member = path(unit, &["restricted", "visible"]);
    graph.pending(member.clone(), producer).unwrap();
    assert_eq!(
        graph
            .lookup_from(&path(unit, &["copied", "visible"]), &outside)
            .unwrap(),
        PathLookup::NeedInput(producer)
    );
    graph.publish(member, producer, vec![]).unwrap();
    assert!(matches!(
        graph
            .lookup_from(&path(unit, &["copied", "visible"]), &outside)
            .unwrap(),
        PathLookup::Defined(_)
    ));
}

#[test]
fn conflicting_strong_glob_cycles_never_choose_an_origin_from_traversal_order() {
    let unit = Digest([55; 32]);
    let prefix = "#[macro_export] macro_rules! one {()=>{17};} #[macro_export] macro_rules! two {()=>{31};} pub mod a {pub use crate::one as chosen;} pub mod b {pub use crate::two as chosen;}";
    let x = "pub mod x {pub use crate::y::chosen;pub use crate::a::*;}";
    let y = "pub mod y {pub use crate::x::chosen;pub use crate::b::*;}";
    let caller = lrr::syntax::outline::ScopeKey {
        crate_key: unit,
        module_path: vec![],
        owner: None,
    };
    for source in [format!("{prefix}{x}{y}"), format!("{prefix}{y}{x}")] {
        let graph = SourceMacroRoutes::candidate_graph(&[catalog(
            &outline(unit, &source),
            &BTreeMap::new(),
        )
        .unwrap()])
        .unwrap();
        for module in ["x", "y"] {
            let diagnostic = graph
                .lookup_from(&path(unit, &[module, "chosen"]), &caller)
                .unwrap_err();
            assert_eq!(
                diagnostic.message,
                "cyclic strong/glob candidate selection has no stable result"
            );
        }
    }
}
