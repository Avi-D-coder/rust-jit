//! Retained import resolution: an internal check that a retained resolution is
//! reused exactly when a full re-run would repeat it. Not language acceptance.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::parse::ParsedFile,
};
const SOURCE: &str = "\
mod a { pub fn f() {} pub struct S; pub mod inner { pub fn h() {} } }
mod b {
    use crate::a::f;
    use crate::a::S as T;
    use crate::a::inner::h;
    pub fn g() { f(); h(); }
}
fn main() {}";

fn graph(edition: u16) -> (crate::syntax::outline::Outline, NameNamespaces) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let parsed = ParsedFile::parse(
        &SourceBlob::new(SOURCE.as_bytes().to_vec()).unwrap(),
        profile,
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"import memo"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let names = NameNamespaces::from_outlines(&[&outline]).unwrap();
    (outline, names)
}
fn scope(outline: &crate::syntax::outline::Outline, path: &[&str]) -> ScopeKey {
    ScopeKey {
        crate_key: outline.root_scope.crate_key,
        module_path: path.iter().map(|part| (*part).to_owned()).collect(),
        owner: None,
    }
}
fn summary(resolution: &ImportResolution) -> String {
    let mut sorted = resolution.clone();
    for ids in [
        &mut sorted.resolved,
        &mut sorted.pending,
        &mut sorted.missing,
        &mut sorted.inaccessible,
        &mut sorted.ambiguous,
    ] {
        ids.sort();
    }
    format!("{sorted:?}")
}

#[test]
fn a_fixed_point_is_retained_and_a_repeat_run_changes_no_structure() {
    for edition in [2015, 2018, 2021, 2024] {
        let (_, mut names) = graph(edition);
        let first = names.resolve_imports().unwrap();
        assert_eq!(first.resolved.len(), 3, "edition {edition}");
        assert!(names.import_memo.is_some(), "edition {edition}");
        // Running the named-import phase again re-establishes only facts the
        // graph already holds: the structure epoch (and so the memo) survives.
        let epoch = names.structure_epoch;
        let repeat = names.resolve_named_imports().unwrap();
        assert_eq!(names.structure_epoch, epoch, "edition {edition}");
        assert_eq!(summary(&repeat), summary(&first));
        // An unchanged graph answers from the generation cache.
        let generation = names.import_generation;
        assert_eq!(summary(&names.resolve_imports().unwrap()), summary(&first));
        assert_eq!(names.import_generation, generation);
    }
}

#[test]
fn a_producer_frontier_edit_off_every_read_path_reuses_the_resolution() {
    let (outline, mut names) = graph(2024);
    let first = names.resolve_imports().unwrap();
    let producer = Digest::of_bytes(b"producer elsewhere");
    let elsewhere = scope(&outline, &["unrelated"]);
    names
        .reserve_generated_frontier(elsewhere.clone(), producer)
        .unwrap();
    // The edit is a new graph state, yet not one the resolution read.
    let generation = names.import_generation;
    let retained = names.resolve_imports().unwrap();
    assert_eq!(names.import_generation, generation);
    assert_eq!(summary(&retained), summary(&first));
    assert_eq!(
        names.import_memo.as_ref().unwrap().log_len,
        names.frontier_log.len()
    );
    names.release_generated_frontier(&elsewhere, producer);
    assert_eq!(summary(&names.resolve_imports().unwrap()), summary(&first));
}

#[test]
fn a_producer_frontier_edit_on_a_read_path_reruns_to_the_fresh_outcome() {
    // For every scope any import path walks through, the retained answer after
    // the edit equals a fresh graph that had the producer from the start.
    let (outline, probe) = graph(2024);
    drop(probe);
    for path in [&[][..], &["a"], &["a", "inner"], &["b"]] {
        let at = scope(&outline, path);
        let producer = Digest::of_bytes(b"producer on a path");
        let (_, mut late) = graph(2024);
        late.resolve_imports().unwrap();
        assert!(late.import_memo.is_some());
        late.reserve_generated_frontier(at.clone(), producer).unwrap();
        let after = late.resolve_imports().unwrap();
        let (_, mut early) = graph(2024);
        early.reserve_generated_frontier(at.clone(), producer).unwrap();
        let fresh = early.resolve_imports().unwrap();
        assert_eq!(summary(&after), summary(&fresh), "scope {path:?}");
        // Releasing it restores the original outcome through a full re-run.
        late.release_generated_frontier(&at, producer);
        let restored = late.resolve_imports().unwrap();
        let (_, mut clean) = graph(2024);
        assert_eq!(
            summary(&restored),
            summary(&clean.resolve_imports().unwrap()),
            "scope {path:?}"
        );
    }
}

#[test]
fn a_structural_change_discards_the_retained_resolution() {
    let (outline, mut names) = graph(2024);
    names.resolve_imports().unwrap();
    let epoch = names.structure_epoch;
    // A pending macro producer at a path is a new binding fact.
    names
        .pending(
            Namespace::Macro,
            NameNamespaces::path(outline.root_scope.crate_key, &["a", "late"], 2024).unwrap(),
            Digest::of_bytes(b"late macro"),
        )
        .unwrap();
    assert_ne!(names.structure_epoch, epoch);
    names.resolve_imports().unwrap();
    assert!(names.import_memo.is_some());
    assert_eq!(names.import_memo.as_ref().unwrap().structure_epoch, names.structure_epoch);
}
