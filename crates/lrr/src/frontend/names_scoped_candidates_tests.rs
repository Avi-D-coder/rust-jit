//! Source namespace equivalence controls, pending Rust execution. These do not
//! claim guest execution, macro execution, compiler acceptance or performance.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{outline::Outline, parse::ParsedFile},
};

fn world(text: &str, edition: u16) -> (Outline, NameNamespaces) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"scoped namespace source-equivalence controls"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
    (outline, graph)
}
fn scope(outline: &Outline, path: &[&str]) -> ScopeKey {
    ScopeKey {
        crate_key: outline.root_scope.crate_key,
        module_path: path.iter().map(|name| (*name).to_owned()).collect(),
        owner: None,
    }
}
fn equivalent(graph: &NameNamespaces, scope: &ScopeKey, ns: Namespace, edition: u16) {
    let old =
        graph.certified_scope_from_candidates(scope, ns, edition, graph.table(ns).member_names());
    let new = graph.certified_scope(scope, ns, edition);
    match (old, new) {
        (Ok(old), Ok(new)) => {
            // Equality of every input field to the unchanged explicit stable
            // encoder establishes identical encoded content, not just a hash.
            assert_eq!(old.scope, new.scope);
            assert_eq!(old.namespace, new.namespace);
            assert_eq!(old.edition, new.edition);
            assert_eq!(old.names, new.names);
            assert_eq!(old.fingerprint(), new.fingerprint());
            assert_eq!(old.metadata_bytes().unwrap(), new.metadata_bytes().unwrap());
            for name in old.names.keys() {
                assert_eq!(
                    graph
                        .lookup_unqualified(ns, name, scope, edition, None)
                        .unwrap(),
                    new.names.get(name).cloned().unwrap_or(NameLookup::Absent)
                );
            }
        }
        (Err(old), Err(new)) => {
            assert_eq!(old.code, new.code);
            assert_eq!(old.message, new.message);
        }
        (old, new) => {
            panic!("scoped candidate changed full namespace outcome: old={old:?}, new={new:?}")
        }
    }
}

#[test]
fn many_actual_unrelated_modules_do_not_become_scope_candidates_all_editions() {
    let mut text = String::from("pub mod target {pub fn wanted(){}} ");
    for ordinal in 0..1040 {
        text.push_str(&format!(
            "pub mod m{ordinal} {{pub fn unrelated_{ordinal}(){{}}}}"
        ));
    }
    for edition in [2015, 2018, 2021, 2024] {
        let (outline, graph) = world(&text, edition);
        let selected = scope(&outline, &["target"]);
        let candidates = graph
            .scoped_certification_candidates(&selected, Namespace::Value, edition)
            .expect("actual plain scope should use prefix candidates");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates.first().unwrap().as_str(), "wanted");
        assert!(graph.values.member_names().len() > 1040);
        for ns in [Namespace::Value, Namespace::Type, Namespace::Macro] {
            equivalent(&graph, &selected, ns, edition);
        }
    }
}

#[test]
fn actual_named_renamed_glob_shadow_and_visibility_worlds_equal_old_global() {
    for text in [
        "mod origin{pub const V:u8=1;pub fn f(){}}mod destination{use super::origin::V as Renamed;pub use super::origin::f as chosen;}fn main(){}",
        "mod origin{pub const V:u8=1;pub const K:u8=2;}mod destination{use super::origin::*;pub const V:u8=3;}fn main(){}",
        "mod origin{pub const V:u8=1;pub const K:u8=2;}mod destination{use super::origin::*;}fn main(){}",
        "mod origin{fn hidden(){}}mod destination{use super::origin::hidden as denied;}fn main(){}",
        "mod origin{pub const V:u8=1;}mod destination{use super::origin::V;use super::origin::V;}fn main(){}",
    ] {
        for edition in [2015, 2018, 2021, 2024] {
            let (outline, mut graph) = world(text, edition);
            let _outcomes = graph.resolve_imports().unwrap();
            for path in [vec![], vec!["origin"], vec!["destination"]] {
                let selected = scope(&outline, &path);
                for ns in [Namespace::Value, Namespace::Type, Namespace::Macro] {
                    equivalent(&graph, &selected, ns, edition);
                }
            }
        }
    }
}

#[test]
fn aliases_module_roots_ambiguity_and_missing_scope_keep_old_fallback() {
    for text in [
        "mod origin{pub mod nested{pub const V:u8=1;}}use origin::nested as alias;fn main(){}",
        "mod left{pub mod alias{pub const V:u8=1;}}mod right{pub mod alias{pub const V:u8=2;}}use left::alias;use right::alias;fn main(){}",
        "mod origin{mod hidden{pub const V:u8=1;}}use origin::hidden as alias;fn main(){}",
    ] {
        let (outline, mut graph) = world(text, 2024);
        graph.resolve_imports().unwrap();
        for path in [vec![], vec!["alias"], vec!["not_disclosed"]] {
            let selected = scope(&outline, &path);
            assert!(
                graph
                    .scoped_certification_candidates(&selected, Namespace::Type, 2024)
                    .is_none()
            );
            for ns in [Namespace::Value, Namespace::Type, Namespace::Macro] {
                equivalent(&graph, &selected, ns, 2024);
            }
        }
    }
    let (outline, _) = world("pub const V:u8=1;fn main(){}", 2024);
    let provider = Digest::of_bytes(b"actual second source unit");
    let parsed_provider = ParsedFile::parse(
        &SourceBlob::new(b"pub type renamed=u8;".to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let provider_outline = parsed_provider
        .outline(provider, vec![], &CfgSet::guest_default())
        .unwrap();
    let mut graph = NameNamespaces::from_outlines(&[&outline, &provider_outline]).unwrap();
    graph
        .dependency(outline.root_scope.crate_key, "renamed", provider, 2024)
        .unwrap();
    // A module-only alias to a crate root has no destination item row. The
    // same spelling in the provider's genuine source is in the old universe.
    assert!(
        graph
            .scoped_certification_candidates(&outline.root_scope, Namespace::Type, 2024)
            .is_none()
    );
    equivalent(&graph, &outline.root_scope, Namespace::Type, 2024);
    assert!(
        matches!(graph.certified_scope(&outline.root_scope, Namespace::Type, 2024).unwrap().names.get("renamed"), Some(NameLookup::ModuleRoot(actual)) if *actual == provider)
    );
    equivalent(
        &graph,
        &scope(&outline, &["renamed"]),
        Namespace::Type,
        2024,
    );
}

#[test]
fn genuine_frontier_and_pending_binding_refusal_equal_old_global() {
    let (outline, mut graph) = world("pub mod target {pub fn wanted(){}}pub mod sibling{}", 2024);
    let selected = scope(&outline, &["target"]);
    let producer = Digest::of_bytes(b"actual unresolved source producer");
    graph
        .reserve_generated_frontier(outline.root_scope.clone(), producer)
        .unwrap();
    assert!(
        graph
            .scoped_certification_candidates(&selected, Namespace::Value, 2024)
            .is_none()
    );
    equivalent(&graph, &selected, Namespace::Value, 2024);
    assert_eq!(
        graph
            .certified_scope(&selected, Namespace::Value, 2024)
            .unwrap_err()
            .code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    graph
        .disclose_module_source_scope(producer, scope(&outline, &["sibling"]))
        .unwrap();
    assert!(
        graph
            .scoped_certification_candidates(&selected, Namespace::Value, 2024)
            .is_some()
    );
    equivalent(&graph, &selected, Namespace::Value, 2024);
    assert!(
        graph
            .scoped_certification_candidates(&selected, Namespace::Macro, 2024)
            .is_none()
    );
    equivalent(&graph, &selected, Namespace::Macro, 2024);
    graph.release_generated_frontier(&outline.root_scope, producer);
    let path = NameNamespaces::path(selected.crate_key, &["target", "pending"], 2024).unwrap();
    graph.pending(Namespace::Value, path, producer).unwrap();
    equivalent(&graph, &selected, Namespace::Value, 2024);
    assert_eq!(
        graph
            .certified_scope(&selected, Namespace::Value, 2024)
            .unwrap_err()
            .code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
}

#[test]
fn weak_destination_spelling_requires_real_old_binding_universe_evidence() {
    let (outline, mut graph) = world(
        "mod origin{pub const V:u8=1;}pub mod destination{}fn main(){}",
        2024,
    );
    let selected = scope(&outline, &["destination"]);
    let target = NameNamespaces::path(selected.crate_key, &["origin", "V"], 2024).unwrap();
    let same = NameNamespaces::path(selected.crate_key, &["destination", "V"], 2024).unwrap();
    graph
        .glob_member(
            Namespace::Value,
            same,
            target.clone(),
            VisibilityScope::Public,
        )
        .unwrap();
    assert!(
        graph
            .scoped_certification_candidates(&selected, Namespace::Value, 2024)
            .is_some()
    );
    equivalent(&graph, &selected, Namespace::Value, 2024);
    let renamed = NameNamespaces::path(
        selected.crate_key,
        &["destination", "weak_only_new_spelling"],
        2024,
    )
    .unwrap();
    graph
        .glob_member(Namespace::Value, renamed, target, VisibilityScope::Public)
        .unwrap();
    assert!(
        graph
            .scoped_certification_candidates(&selected, Namespace::Value, 2024)
            .is_none()
    );
    equivalent(&graph, &selected, Namespace::Value, 2024);
}

#[test]
fn raw_keyword_module_names_and_empty_universe_preserve_old_diagnostics() {
    for edition in [2015, 2018, 2021, 2024] {
        let (outline, graph) = world("pub mod r#gen{pub fn r#type(){}}", edition);
        let selected = outline
            .items
            .iter()
            .find(|item| {
                item.def_key.declared_name == "r#type" || item.def_key.declared_name == "type"
            })
            .unwrap()
            .scope
            .clone();
        for ns in [Namespace::Value, Namespace::Type, Namespace::Macro] {
            equivalent(&graph, &selected, ns, edition);
        }
        let missing = scope(&outline, &["not_disclosed"]);
        equivalent(&graph, &missing, Namespace::Macro, edition);
        equivalent(&graph, &selected, Namespace::Value, 2099);
        equivalent(&graph, &outline.root_scope, Namespace::Value, 2099);
    }
}
