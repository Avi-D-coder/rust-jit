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
