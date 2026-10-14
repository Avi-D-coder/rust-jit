use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    expand::scopes::MacroName,
    frontend::{
        hygiene::HygieneFacts,
        macro_names::{MacroNamespaces, MacroPath, PathLookup},
        source_macro_routes::SourceMacroRoutes,
        source_macros::SourceMacroIndex,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{outline::ScopeKey, parse::ParsedFile, spans::SourceRange, token_wire::*},
};
use std::collections::BTreeMap;
fn facts(unit: Digest) -> (Key, HygieneFacts) {
    let row = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key(unit.0),
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
    (root, facts)
}
fn prepare(unit: Digest, source: &str) -> (ParsedFile, SourceMacroIndex, MacroNamespaces) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed.outline(unit, vec![], &CfgSet::default()).unwrap();
    let (root, f) = facts(unit);
    let index = SourceMacroIndex::from_outline(&outline, root, f).unwrap();
    let (root, f) = facts(unit);
    let routes = SourceMacroRoutes::from_outline(&outline, root, f, &BTreeMap::new()).unwrap();
    (
        parsed,
        index,
        SourceMacroRoutes::candidate_graph(&[routes]).unwrap(),
    )
}
fn range(source: &str, call: &str) -> SourceRange {
    let start = source.find(call).unwrap();
    SourceRange::checked(
        u32::try_from(start).unwrap(),
        u32::try_from(start + call.len()).unwrap(),
    )
    .unwrap()
}
fn scope(unit: Digest, module: &[&str]) -> ScopeKey {
    ScopeKey {
        crate_key: unit,
        module_path: module.iter().map(|s| s.to_string()).collect(),
        owner: None,
    }
}
#[test]
fn original_invocation_routes_relative_raw_paths_and_preserves_textual_priority() {
    let unit = Digest([95; 32]);
    let source = "#[macro_export] macro_rules! root {()=>{17};} mod first {pub use crate::root as r#gen; self::r#gen!();} mod second {super::root!();} mod third {crate::root!();} mod local {use crate::root as chosen; macro_rules! chosen {()=>{31};} chosen!();}";
    let (parsed, index, graph) = prepare(unit, source);
    let path = MacroPath::new(unit, vec![MacroName::parse("root", 2024).unwrap()]).unwrap();
    let expected = graph.lookup_from(&path, &scope(unit, &[])).unwrap();
    for (call, module) in [
        ("self::r#gen!();", "first"),
        ("super::root!();", "second"),
        ("crate::root!();", "third"),
    ] {
        let call = parsed.macro_call_at(range(source, call)).unwrap();
        assert_eq!(
            index
                .lookup_invocation(&call, &graph, &scope(unit, &[module]))
                .unwrap(),
            expected
        );
    }
    let call = parsed.macro_call_at(range(source, "chosen!();")).unwrap();
    let selected = index
        .lookup_invocation(&call, &graph, &scope(unit, &["local"]))
        .unwrap();
    assert!(matches!(selected, PathLookup::Defined(_)));
    assert_ne!(selected, expected);
}
#[test]
fn invocation_selection_rejects_stale_ranges_wrong_modules_and_unentered_bodies() {
    let unit = Digest([96; 32]);
    let source =
        "#[macro_export] macro_rules! m {()=>{};} mod area {crate::m!();} fn cold(){m!();}";
    let (parsed, index, graph) = prepare(unit, source);
    let selected = range(source, "crate::m!();");
    let call = parsed.macro_call_at(selected).unwrap();
    assert!(
        index
            .lookup_invocation(&call, &graph, &scope(unit, &[]))
            .is_err()
    );
    assert!(
        parsed
            .macro_call_at(SourceRange {
                start: selected.start + 1,
                end: selected.end
            })
            .is_err()
    );
    assert!(
        parsed
            .macro_call_at(SourceRange {
                start: 0,
                end: u32::MAX
            })
            .is_err()
    );
    let body = parsed.macro_call_at(range(source, "m!();}")).err();
    assert!(body.is_some());
    let start = source.rfind("m!();").unwrap();
    let body = parsed
        .macro_call_at(SourceRange {
            start: u32::try_from(start).unwrap(),
            end: u32::try_from(start + 4).unwrap(),
        })
        .unwrap();
    assert!(body.requires_body_entry());
    assert!(
        index
            .lookup_invocation(&body, &graph, &scope(unit, &[]))
            .is_err()
    );
    let changed = source.replace("crate::m!();", "self::m!();");
    let changed = ParsedFile::parse(
        &SourceBlob::new(changed.into_bytes()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let changed_call = changed
        .macro_call_at(SourceRange {
            start: selected.start,
            end: selected.end - 1,
        })
        .unwrap();
    assert!(
        index
            .lookup_invocation(&changed_call, &graph, &scope(unit, &["area"]))
            .is_err()
    );
}

#[test]
fn parsed_absolute_and_nested_dependency_calls_preserve_provider_identity() {
    let provider = Digest([97; 32]);
    let consumer = Digest([98; 32]);
    let provider_source = "#[macro_export] macro_rules! r#gen {()=>{};}";
    let source = "mod relative { renamed::r#gen!(); } mod absolute { ::renamed::r#gen!(); }";
    let (parsed, index, _) = prepare(consumer, source);
    let p = ParsedFile::parse(
        &SourceBlob::new(provider_source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let po = p.outline(provider, vec![], &CfgSet::default()).unwrap();
    let co = parsed
        .outline(consumer, vec![], &CfgSet::default())
        .unwrap();
    let (root, pfacts) = facts(provider);
    let pc = SourceMacroRoutes::from_outline(&po, root, pfacts, &BTreeMap::new()).unwrap();
    let (root, cfacts) = facts(consumer);
    let cc = SourceMacroRoutes::from_outline(
        &co,
        root,
        cfacts,
        &BTreeMap::from([(MacroName::parse("renamed", 2024).unwrap(), provider)]),
    )
    .unwrap();
    let graph = SourceMacroRoutes::candidate_graph(&[pc, cc]).unwrap();
    let expected = graph
        .lookup_from(
            &MacroPath::new(provider, vec![MacroName::parse("r#gen", 2024).unwrap()]).unwrap(),
            &scope(consumer, &[]),
        )
        .unwrap();
    for (call, module) in [
        ("renamed::r#gen!();", "relative"),
        ("::renamed::r#gen!();", "absolute"),
    ] {
        let start = if module == "relative" {
            source.find(call).unwrap()
        } else {
            source.rfind(call).unwrap()
        };
        let call = parsed
            .macro_call_at(SourceRange {
                start: u32::try_from(start).unwrap(),
                end: u32::try_from(start + call.len()).unwrap(),
            })
            .unwrap();
        assert_eq!(call.absolute(), module == "absolute");
        assert_eq!(
            index
                .lookup_invocation(&call, &graph, &scope(consumer, &[module]))
                .unwrap(),
            expected
        );
    }
}

#[test]
fn qualified_invocation_uses_path_namespace_and_keeps_templates_opaque() {
    let unit = Digest([100; 32]);
    let source = "#[macro_export] macro_rules! m {()=>{template::hidden!();};} produce!(); crate::m!(); m!();";
    let (parsed, index, graph) = prepare(unit, source);
    assert!(
        parsed
            .macro_call_at(range(source, "template::hidden!();"))
            .is_err()
    );
    let call = parsed.macro_call_at(range(source, "crate::m!();")).unwrap();
    assert_eq!(
        index
            .lookup_invocation(&call, &graph, &scope(unit, &[]))
            .unwrap(),
        graph
            .lookup_from(
                &MacroPath::new(unit, vec![MacroName::parse("m", 2024).unwrap()]).unwrap(),
                &scope(unit, &[]),
            )
            .unwrap()
    );
    let bare_start = source.rfind("m!();").unwrap();
    let bare = parsed
        .macro_call_at(
            SourceRange::checked(
                u32::try_from(bare_start).unwrap(),
                u32::try_from(bare_start + "m!();".len()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    assert!(matches!(
        index
            .lookup_invocation(&bare, &graph, &scope(unit, &[]))
            .unwrap(),
        PathLookup::NeedInput(_)
    ));
}

#[test]
fn disabled_item_calls_are_absent_and_own_transforming_attributes_suspend() {
    let unit = Digest([101; 32]);
    for (text, expected_pending) in [
        ("#[cfg(any())] crate::m!();", false),
        ("#[transform] crate::m!();", true),
        ("#[cfg_attr(all(), transform)] crate::m!();", true),
    ] {
        let source = format!("#[macro_export] macro_rules! m {{()=>{{}};}} {text}");
        let (parsed, index, graph) = prepare(unit, &source);
        let call = parsed.macro_call_at(range(&source, text)).unwrap();
        let actual = index
            .lookup_invocation(&call, &graph, &scope(unit, &[]))
            .unwrap();
        if expected_pending {
            assert!(matches!(actual, PathLookup::NeedInput(_)), "{actual:?}");
        } else {
            assert_eq!(actual, PathLookup::Absent);
        }
    }
}

#[test]
fn cfg_filtered_invocation_checks_own_attribute_before_path_selection() {
    let unit = Digest([102; 32]);
    let source = "#[macro_export] macro_rules! m {()=>{};} #[cfg(any())] mod off {crate::m!();} #[cfg_attr(any(), transform)] crate::m!(); produce!(); #[transform] crate::m!();";
    let (parsed, index, graph) = prepare(unit, source);
    let disabled = parsed.macro_call_at(range(source, "crate::m!();")).unwrap();
    assert_eq!(
        index
            .lookup_invocation(&disabled, &graph, &scope(unit, &["off"]))
            .unwrap(),
        PathLookup::Absent
    );
    let harmless_text = "#[cfg_attr(any(), transform)] crate::m!();";
    let harmless = parsed.macro_call_at(range(source, harmless_text)).unwrap();
    assert!(matches!(
        index
            .lookup_invocation(&harmless, &graph, &scope(unit, &[]))
            .unwrap(),
        PathLookup::Defined(_)
    ));
    let outline = parsed.outline(unit, vec![], &CfgSet::default()).unwrap();
    let target = parsed
        .macro_call_at(range(source, "#[transform] crate::m!();"))
        .unwrap();
    let prior = outline
        .item_invocations
        .iter()
        .find(|call| call.range == harmless.range())
        .unwrap()
        .item_input;
    let own = outline
        .item_invocations
        .iter()
        .find(|call| call.range == target.range())
        .unwrap()
        .attribute_input
        .unwrap();
    assert_ne!(prior, own);
    assert_eq!(
        index
            .lookup_invocation(&target, &graph, &scope(unit, &[]))
            .unwrap(),
        PathLookup::NeedInput(own)
    );
    assert_eq!(
        index.producer_kind(own),
        Some(lrr::syntax::outline::ProducerKind::Attribute)
    );
}
