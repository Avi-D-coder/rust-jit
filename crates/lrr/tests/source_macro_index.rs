use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    frontend::{
        hygiene::HygieneFacts, macro_origins::MacroSelection, macro_scopes::HygienicLookup,
        source_macros::SourceMacroIndex,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        parse::ParsedFile,
        token_wire::{
            ContextRecord, Edition, Key, NameContextKey, NameResolutionContext, ResolutionContext,
            Transparency,
        },
    },
};
fn build(source: &str) -> (SourceMacroIndex, lrr::syntax::outline::Outline) {
    let unit = Digest([1; 32]);
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key(unit.0),
        edition: Edition::E2024,
    };
    let context = root.key();
    let facts = HygieneFacts::new(
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
    .unwrap();
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed.outline(unit, vec![], &CfgSet::default()).unwrap();
    let mut wrong = outline.clone();
    wrong.root_scope.crate_key = Digest([2; 32]);
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key(unit.0),
        edition: Edition::E2024,
    };
    let wrong_facts = HygieneFacts::new(
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
    .unwrap();
    assert!(SourceMacroIndex::from_outline(&wrong, context, wrong_facts).is_err());
    (
        SourceMacroIndex::from_outline(&outline, context, facts).unwrap(),
        outline,
    )
}
#[test]
fn source_macro_index_discovers_cfg_raw_names_and_nested_parent_visibility_without_entering_bodies()
{
    let source = "macro_rules! r#choose {()=>{17};} mod inner { fn entered(){ choose!(); } } macro_rules! choose {()=>{29};} #[cfg(any())] macro_rules! choose {()=>{99};} fn cold(){ macro_rules! choose {()=>{31};} missing!(); }";
    let (index, outline) = build(source);
    let first = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "choose" && i.def_key.duplicate_ordinal == 0)
        .unwrap()
        .def_key
        .fingerprint();
    let second = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "choose" && i.def_key.duplicate_ordinal == 1)
        .unwrap()
        .def_key
        .fingerprint();
    assert_eq!(
        index.lookup(&["inner".into()], u64::MAX, "choose").unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(
            index.definition(first).unwrap().clone()
        ))
    );
    assert_eq!(
        index.lookup(&[], u64::MAX, "r#choose").unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(
            index.definition(second).unwrap().clone()
        ))
    );
    assert_eq!(index.definition_count(), 2);
    let (keywords, _) = build("macro_rules! r#gen {()=>{17};}");
    assert!(matches!(
        keywords.lookup(&[], u64::MAX, "r#gen").unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(_))
    ));
    assert_eq!(
        index.lookup(&[], 0, "choose").unwrap(),
        HygienicLookup::Selection(MacroSelection::Absent)
    );
}
#[test]
fn source_macro_index_tracks_item_producers_at_their_source_positions_and_rejects_wrong_authority()
{
    let source = "macro_rules! choose {()=>{17};} generate!(); mod later { fn use_it(){} }";
    let (index, _) = build(source);
    let call = u64::try_from(source.find("generate!").unwrap()).unwrap();
    assert!(matches!(
        index.lookup(&[], call, "choose").unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(_))
    ));
    assert!(matches!(
        index.lookup(&[], call + 1, "choose").unwrap(),
        HygienicLookup::NeedInput(_)
    ));
    assert!(matches!(
        index.lookup(&["later".into()], u64::MAX, "choose").unwrap(),
        HygienicLookup::NeedInput(_)
    ));
    assert!(
        index
            .lookup(&["absent".into()], u64::MAX, "choose")
            .is_err()
    );
}

#[test]
fn source_index_waits_for_transforming_scope_attributes_and_derive_output() {
    let (index, _) = build(
        "macro_rules! choose {()=>{17};} #[transform] mod altered { macro_rules! choose {()=>{29};} } #[derive(Generate)] struct Generated;",
    );
    assert!(matches!(
        index
            .lookup(&["altered".into()], u64::MAX, "choose")
            .unwrap(),
        HygienicLookup::NeedInput(_)
    ));
    assert!(matches!(
        index.lookup(&[], u64::MAX, "choose").unwrap(),
        HygienicLookup::NeedInput(_)
    ));
}

#[test]
fn unresolved_external_module_is_typed_source_demand_instead_of_empty_scope() {
    let (index, _) = build("macro_rules! choose {()=>{17};} mod external;");
    let HygienicLookup::NeedInput(input) = index
        .lookup(&["external".into()], u64::MAX, "choose")
        .unwrap()
    else {
        panic!("unloaded module reported complete")
    };
    assert_eq!(
        index.producer_kind(input),
        Some(lrr::syntax::outline::ProducerKind::ModuleSource)
    );
    assert!(matches!(
        index.lookup(&[], u64::MAX, "choose").unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(_))
    ));
}
