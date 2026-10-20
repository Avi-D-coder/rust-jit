//! Original import ambiguity component; no compiler diagnostics/runtime pass claim.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::parse::ParsedFile,
};
use std::collections::BTreeSet;
const ORIGINAL: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/issue-25396.rs");
fn graph(text: &str, edition: u16) -> (crate::syntax::outline::Outline, NameNamespaces) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let parsed =
        ParsedFile::parse(&SourceBlob::new(text.as_bytes().to_vec()).unwrap(), profile).unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"source module ambiguity"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let names = NameNamespaces::from_outlines(&[&outline]).unwrap();
    (outline, names)
}
#[test]
fn unchanged_distinct_module_imports_preserve_value_and_type_ambiguities_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (outline, mut names) = graph(ORIGINAL, edition);
        let resolution = names.resolve_imports().unwrap();
        assert!(!resolution.ambiguous.is_empty());
        assert!(resolution.pending.is_empty());
        assert!(resolution.missing.is_empty());
        for (namespace, name) in [
            (Namespace::Value, "WOMP"),
            (Namespace::Type, "baz"),
            (Namespace::Type, "Quux"),
            (Namespace::Type, "blah"),
        ] {
            let NameLookup::Ambiguous(found) = names
                .lookup_unqualified(namespace, name, &outline.root_scope, edition, None)
                .unwrap()
            else {
                panic!("exact original competing imports were not preserved")
            };
            let expected: BTreeSet<_> = outline
                .items
                .iter()
                .filter(|i| i.def_key.declared_name == name)
                .map(|i| i.def_key.fingerprint())
                .collect();
            let actual: BTreeSet<_> = found
                .iter()
                .map(|d| {
                    assert_eq!(d.namespace, namespace);
                    assert_eq!(d.definition_edition, edition);
                    assert_eq!(d.defining_unit, outline.root_scope.crate_key);
                    d.identity
                })
                .collect();
            assert_eq!(actual, expected);
            assert_eq!(actual.len(), 2);
        }
    }
}
const PREFIX: &str = "use foo::baz;use bar::baz;mod foo{pub mod baz{pub const X:u8=1;}}mod bar{pub mod baz{pub const X:u8=2;}}fn main(){}";
#[test]
fn qualified_imported_module_prefix_is_typed_ambiguous_not_first_route() {
    for edition in [2015, 2018, 2021, 2024] {
        let (outline, mut names) = graph(PREFIX, edition);
        names.resolve_imports().unwrap();
        let path =
            NameNamespaces::path(outline.root_scope.crate_key, &["baz", "X"], edition).unwrap();
        let NameLookup::AmbiguousModules(found) = names
            .lookup(Namespace::Value, &path, &outline.root_scope)
            .unwrap()
        else {
            panic!("competing source modules must not select X")
        };
        let expected = BTreeSet::from([
            NameNamespaces::path(outline.root_scope.crate_key, &["foo", "baz"], edition).unwrap(),
            NameNamespaces::path(outline.root_scope.crate_key, &["bar", "baz"], edition).unwrap(),
        ]);
        assert_eq!(found.into_iter().collect::<BTreeSet<_>>(), expected);
    }
}
#[test]
fn repeated_source_import_fixed_point_retains_routes_once() {
    let (_, mut names) = graph(ORIGINAL, 2024);
    let first = names.resolve_imports().unwrap();
    let before = (
        names.named_module_routes,
        names.named_module_bytes,
        names.import_state(),
    );
    for _ in 0..3 {
        let result = names.resolve_imports().unwrap();
        assert_eq!(result.ambiguous, first.ambiguous);
        assert_eq!(
            (
                names.named_module_routes,
                names.named_module_bytes,
                names.import_state()
            ),
            before
        );
    }
    assert_eq!(names.named_module_routes, 2);
}
#[test]
fn genuine_unknown_producer_on_either_import_route_remains_need_input() {
    for module in ["foo", "bar"] {
        let (outline, mut names) = graph(PREFIX, 2024);
        names.resolve_imports().unwrap();
        let producer = Digest::of_bytes(module.as_bytes());
        let scope = ScopeKey {
            crate_key: outline.root_scope.crate_key,
            module_path: vec![module.into(), "baz".into()],
            owner: None,
        };
        names.reserve_generated_frontier(scope, producer).unwrap();
        let path = NameNamespaces::path(outline.root_scope.crate_key, &["baz", "X"], 2024).unwrap();
        assert!(
            matches!(names.lookup(Namespace::Value,&path,&outline.root_scope).unwrap(),NameLookup::NeedInput(actual) if actual==producer)
        );
    }
}
#[test]
fn unresolved_original_import_does_not_become_completed_destination_ambiguity() {
    let (outline, mut names) = graph(
        "use missing::baz;use foo::baz;mod foo{pub mod baz{pub const X:u8=1;}}fn main(){}",
        2024,
    );
    let result = names.resolve_imports().unwrap();
    assert!(!result.pending.is_empty() || !result.missing.is_empty());
    let path = NameNamespaces::path(outline.root_scope.crate_key, &["baz", "X"], 2024).unwrap();
    assert!(matches!(
        names
            .lookup(Namespace::Value, &path, &outline.root_scope)
            .unwrap(),
        NameLookup::NeedInput(_)
    ));
}

#[test]
fn glob_imported_crate_root_alias_retains_its_target_and_frontier() {
    let source = "pub const VALUE: u8 = 1; mod a { pub use crate as root; } mod b { pub use crate::a::*; } fn main() {}";
    for edition in [2015, 2018, 2021, 2024] {
        let (outline, mut names) = graph(source, edition);
        let resolution = names.resolve_imports().unwrap();
        assert!(resolution.pending.is_empty(), "{edition}");
        assert!(resolution.missing.is_empty(), "{edition}");
        let unit = outline.root_scope.crate_key;
        let alias = NameNamespaces::path(unit, &["b", "root"], edition).unwrap();
        assert!(names.glob_module_links[&alias].contains(
            &super::super::module_paths::ModulePath {
                unit,
                names: vec![]
            }
        ));
        let value = NameNamespaces::path(unit, &["b", "root", "VALUE"], edition).unwrap();
        assert!(
            matches!(
                names
                    .lookup(Namespace::Value, &value, &outline.root_scope)
                    .unwrap(),
                NameLookup::Defined(_)
            ),
            "{edition}"
        );
        let producer = Digest::of_bytes(b"actual root namespace producer");
        names
            .reserve_generated_frontier(outline.root_scope.clone(), producer)
            .unwrap();
        assert!(
            matches!(names.lookup(Namespace::Value, &value, &outline.root_scope).unwrap(), NameLookup::NeedInput(actual) if actual == producer),
            "{edition}"
        );
    }
}

#[test]
fn disclosed_nonroot_module_alias_requires_no_synthetic_item_and_keeps_frontiers() {
    for edition in [2015,2018,2021,2024] {
        let (outline,mut names)=graph("use bridge::inner as chosen; fn main(){}",edition);
        let unit=outline.root_scope.crate_key;
        names.declare_module(unit,&["bridge"],edition,VisibilityScope::Public).unwrap();
        names.declare_module(unit,&["bridge","inner"],edition,VisibilityScope::Public).unwrap();
        let item=NameDefinition {namespace:Namespace::Value,identity:Digest([231;32]),defining_unit:unit,definition_edition:edition};
        names.define(NameNamespaces::path(unit,&["bridge","inner","known"],edition).unwrap(),item.clone(),VisibilityScope::Public).unwrap();
        let resolved=names.resolve_imports().unwrap();
        assert!(resolved.missing.is_empty() && resolved.pending.is_empty() && resolved.ambiguous.is_empty());
        assert_eq!(resolved.resolved.len(),1);
        let selected=NameNamespaces::path(unit,&["chosen","known"],edition).unwrap();
        assert_eq!(names.lookup(Namespace::Value,&selected,&outline.root_scope).unwrap(),NameLookup::Defined(item.clone()));
        // No item with the module's name was fabricated in the Type namespace.
        let module=NameNamespaces::path(unit,&["bridge","inner"],edition).unwrap();
        assert_eq!(names.types.lookup_from(&module,&outline.root_scope).unwrap(),PathLookup::Absent);
        let producer=Digest([232;32]);
        names.reserve_generated_frontier(outline.root_scope.clone(),producer).unwrap();
        let pending=names.resolve_imports().unwrap();
        assert_eq!(pending.pending.len(),1);
        let NameLookup::NeedInput(waiting)=names.lookup(Namespace::Value,&selected,&outline.root_scope).unwrap() else {
            panic!("an unresolved source/import frontier cannot authorize the old value");
        };
        // Either actual prerequisite may be first in the deterministic worklist.
        assert!(waiting==producer || pending.pending.contains(&waiting));
        names.release_generated_frontier(&outline.root_scope,producer);
        let resolved=names.resolve_imports().unwrap();assert!(resolved.pending.is_empty() && resolved.missing.is_empty());
        assert_eq!(names.lookup(Namespace::Value,&selected,&outline.root_scope).unwrap(),NameLookup::Defined(item));
    }
}
