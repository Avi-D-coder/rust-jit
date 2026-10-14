use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database},
    frontend::{
        hygiene::HygieneFacts, macro_origins::MacroSelection, macro_scopes::HygienicLookup,
        source_macros::PublishedMacroIndex,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        parse::ParseKey,
        token_wire::{
            ContextRecord, Edition, Key, NameContextKey, NameResolutionContext, ResolutionContext,
            Transparency,
        },
    },
};
fn unit(source: &str, cfg: CfgSet) -> SourceUnitInput {
    SourceUnitInput {
        path: SourcePath::new("source.rs").unwrap(),
        crate_key: Digest([1; 32]),
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg,
        blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
    }
}
fn selection(unit: &SourceUnitInput) -> SourceSelection {
    SourceSelection {
        path: unit.path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: unit.blob.content(),
                profile: unit.profile.clone(),
            },
            crate_key: unit.crate_key,
            module_path: unit.module_path.clone(),
            cfg: unit.cfg.fingerprint(),
        },
    }
}
fn facts() -> (Key, HygieneFacts) {
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([1; 32]),
        edition: Edition::E2024,
    };
    let context = root.key();
    (
        context,
        HygieneFacts::new(
            vec![root],
            vec![ResolutionContext {
                original: context,
                item: context,
                local: context,
            }],
        )
        .unwrap()
        .with_name_contexts(vec![NameResolutionContext {
            original: context,
            item: NameContextKey(Key([2; 32])),
            local: NameContextKey(Key([2; 32])),
        }])
        .unwrap(),
    )
}
fn index(prepared: &PreparedSourceInputs, selected: &SourceSelection) -> PublishedMacroIndex {
    let SourceSelectionOutcome::Selected(view) = prepared.select(selected).unwrap() else {
        panic!("exact source selection failed")
    };
    let (context, facts) = facts();
    PublishedMacroIndex::from_view(view, context, facts).unwrap()
}
#[test]
fn published_macro_index_retains_exact_old_source_and_rejects_cross_revision_requests() {
    let old = unit(
        "macro_rules! old {()=>{17};} fn cold(){ missing!(); }",
        CfgSet::default(),
    );
    let old_bytes = old.blob.bytes().len();
    let old_selection = selection(&old);
    let prepared = PreparedSourceInputs::refresh(None, vec![old], Default::default()).unwrap();
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let old_index = index(&prepared, &old_selection);
    let new = unit("macro_rules! new {()=>{29};}", CfgSet::default());
    let new_bytes = new.blob.bytes().len();
    let new_selection = selection(&new);
    let next =
        PreparedSourceInputs::refresh(Some(&prepared), vec![new], Default::default()).unwrap();
    next.publish(&mut db).unwrap();
    let new_index = index(&next, &new_selection);
    drop(prepared);
    assert!(matches!(
        old_index
            .lookup(&old_selection, &[], u64::MAX, "old")
            .unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(_))
    ));
    assert_eq!(
        old_index.source().outline.parse.content,
        old_selection.outline.parse.content
    );
    assert!(
        old_index
            .lookup(&new_selection, &[], u64::MAX, "old")
            .is_err()
    );
    assert!(
        new_index
            .lookup(&old_selection, &[], u64::MAX, "new")
            .is_err()
    );
    assert_eq!(
        new_index
            .lookup(&new_selection, &[], u64::MAX, "old")
            .unwrap(),
        HygienicLookup::Selection(MacroSelection::Absent)
    );
    assert!(matches!(
        next.select(&old_selection).unwrap(),
        SourceSelectionOutcome::Changed { .. }
    ));
    let deleted = PreparedSourceInputs::refresh(Some(&next), vec![], Default::default()).unwrap();
    deleted.publish(&mut db).unwrap();
    assert!(matches!(
        deleted.select(&new_selection).unwrap(),
        SourceSelectionOutcome::Absent
    ));
    drop(next);
    drop(deleted);
    assert!(matches!(
        new_index
            .lookup(&new_selection, &[], u64::MAX, "new")
            .unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(_))
    ));
    assert_eq!(
        db.cache_usage().unwrap().source_input_bytes,
        old_bytes + new_bytes
    );
    drop(old_index);
    assert_eq!(db.cache_usage().unwrap().source_input_bytes, new_bytes);
    drop(new_index);
    assert_eq!(db.cache_usage().unwrap().source_input_bytes, 0);
}
#[test]
fn publication_selection_checks_cfg_unit_path_and_canonical_module_context() {
    let mut source = unit(
        "#[cfg(active)] macro_rules! yes {()=>{17};}",
        CfgSet::default(),
    );
    source.module_path = vec!["cafe\u{301}".into()];
    let old_selection = selection(&source);
    let old =
        PreparedSourceInputs::refresh(None, vec![source.clone()], Default::default()).unwrap();
    let old_index = index(&old, &old_selection);
    assert_eq!(
        old_index
            .lookup(&old_selection, &["café".into()], u64::MAX, "yes")
            .unwrap(),
        HygienicLookup::Selection(MacroSelection::Absent)
    );
    source.cfg.atoms.insert("active".into());
    let mut new_selection = selection(&source);
    new_selection.outline.module_path = vec!["café".into()];
    let new = PreparedSourceInputs::refresh(Some(&old), vec![source], Default::default()).unwrap();
    assert!(matches!(
        new.select(&old_selection).unwrap(),
        SourceSelectionOutcome::Changed { .. }
    ));
    let new_index = index(&new, &new_selection);
    assert!(matches!(
        new_index
            .lookup(&new_selection, &["cafe\u{301}".into()], u64::MAX, "yes")
            .unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(_))
    ));
    assert!(
        old_index
            .lookup(&new_selection, &["café".into()], u64::MAX, "yes")
            .is_err()
    );
    let mut wrong = new_selection.clone();
    wrong.outline.crate_key = Digest([2; 32]);
    assert!(matches!(
        new.select(&wrong).unwrap(),
        SourceSelectionOutcome::Absent
    ));
    assert!(
        new_index
            .lookup(&wrong, &["café".into()], u64::MAX, "yes")
            .is_err()
    );
    wrong = new_selection.clone();
    wrong.path = SourcePath::new("different.rs").unwrap();
    assert!(matches!(
        new.select(&wrong).unwrap(),
        SourceSelectionOutcome::Absent
    ));
    for edition in [2015, 2018, 2021] {
        wrong = new_selection.clone();
        wrong.outline.parse.profile.edition = edition;
        assert!(matches!(
            new.select(&wrong).unwrap(),
            SourceSelectionOutcome::Changed { actual } if actual == new_selection
        ));
        assert!(new_index.lookup(&wrong, &[], 0, "yes").is_err());
    }
    wrong = new_selection.clone();
    wrong.outline.parse.profile.edition = 2099;
    assert!(new.select(&wrong).is_err());
    wrong = new_selection.clone();
    wrong.outline.module_path = vec!["x".into(); 257];
    assert!(new.select(&wrong).is_err());
    assert!(new_index.lookup(&wrong, &[], 0, "yes").is_err());
}
