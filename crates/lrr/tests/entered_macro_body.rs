use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    frontend::{
        hygiene::HygieneFacts, macro_origins::MacroSelection, macro_scopes::HygienicLookup,
        source_macros::PublishedMacroIndex,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        entered_macros::BodyMacroEventKind,
        parse::ParseKey,
        token_wire::{
            ContextRecord, Edition, Key, NameContextKey, NameResolutionContext, ResolutionContext,
            Transparency,
        },
    },
};
fn prepared(
    source: &str,
) -> (
    PublishedMacroIndex,
    SourceSelection,
    lrr::source::manifest::BodyKey,
) {
    let unit = SourceUnitInput {
        path: SourcePath::new("source.rs").unwrap(),
        crate_key: Digest([1; 32]),
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::default(),
        blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
    };
    let selected = SourceSelection {
        path: unit.path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: unit.blob.content(),
                profile: unit.profile.clone(),
            },
            crate_key: unit.crate_key,
            module_path: vec![],
            cfg: unit.cfg.fingerprint(),
        },
    };
    let prepared = PreparedSourceInputs::refresh(None, vec![unit], Default::default()).unwrap();
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selected).unwrap() else {
        panic!("missing source")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "selected")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([1; 32]),
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
    (
        PublishedMacroIndex::from_view(view, context, facts).unwrap(),
        selected,
        body,
    )
}
#[test]
fn entered_body_discovers_ordered_blocks_and_keeps_nested_executables_opaque() {
    let source = "macro_rules! choose {()=>{17};} fn selected(){ let a=choose!(); let b={ let x=choose!(); macro_rules! choose {()=>{41};} let y=choose!(); }; macro_rules! choose {()=>{29};} let c=choose!(); fn nested(){macro_rules! hidden {()=>{99};} missing!();} let closure=||{macro_rules! hidden {()=>{99};} missing!();}; } macro_rules! choose {()=>{53};} fn cold(){missing!();}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let calls: Vec<_> = source
        .match_indices("choose!()")
        .map(|(at, _)| u64::try_from(at).unwrap())
        .collect();
    assert_eq!(calls.len(), 4);
    let choices: Vec<_> = calls
        .iter()
        .map(|&at| {
            let HygienicLookup::Selection(MacroSelection::Defined(d)) =
                entered.lookup(&selected, &body, at, "choose").unwrap()
            else {
                panic!("selection failed")
            };
            d.identity
        })
        .collect();
    assert_eq!(choices[0], choices[1]);
    assert_ne!(choices[1], choices[2]);
    assert_ne!(choices[2], choices[3]);
    assert_eq!(entered.facts().scopes.len(), 2);
    assert_eq!(entered.facts().events.len(), 2);
    assert!(
        entered
            .facts()
            .events
            .iter()
            .all(|e| matches!(e.kind, BodyMacroEventKind::Definition { .. }))
    );
    assert!(entered.lookup(&selected, &body, 0, "choose").is_err());
    let nested = u64::try_from(source.find("macro_rules! hidden").unwrap()).unwrap();
    assert!(entered.lookup(&selected, &body, nested, "choose").is_err());
    let closure = u64::try_from(source.rfind("macro_rules! hidden").unwrap()).unwrap();
    assert!(entered.lookup(&selected, &body, closure, "choose").is_err());
    let mut wrong = body.clone();
    wrong.exact_body_bytes = Digest([99; 32]);
    assert!(index.enter_body(&selected, &wrong).is_err());
    assert!(
        entered
            .lookup(&selected, &wrong, calls[0], "choose")
            .is_err()
    );
}
#[test]
fn entered_statement_producers_suspend_without_expanding_token_trees() {
    let source = "fn selected(){ macro_rules! old {()=>{17};} generate!(); let x=old!(); }";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let call = u64::try_from(source.find("generate!").unwrap()).unwrap();
    assert!(matches!(
        entered.lookup(&selected, &body, call, "old").unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(_))
    ));
    let later = u64::try_from(source.find("let x").unwrap()).unwrap();
    let HygienicLookup::NeedInput(input) = entered.lookup(&selected, &body, later, "old").unwrap()
    else {
        panic!("producer ignored")
    };
    assert_eq!(
        entered.producer_kind(input),
        Some(lrr::syntax::outline::ProducerKind::ItemMacro)
    );
    assert_eq!(entered.facts().events.len(), 2);
}

#[test]
fn entered_macro_header_preserves_export_facets_and_cfg_filtering() {
    let source = "fn selected(){ #[cfg(any())] macro_rules! hidden {()=>{99};} #[macro_export] macro_rules! exported {()=>{17};} let x=exported!(); }";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    assert_eq!(entered.facts().events.len(), 1);
    let BodyMacroEventKind::Definition {
        name, attributes, ..
    } = &entered.facts().events[0].kind
    else {
        panic!("macro header lost")
    };
    assert_eq!(name, "exported");
    assert!(
        attributes
            .iter()
            .any(|attr| attr.class == lrr::syntax::outline::AttributeClass::MacroExport)
    );
    let at = u64::try_from(source.find("exported!()").unwrap()).unwrap();
    assert!(matches!(
        entered.lookup(&selected, &body, at, "exported").unwrap(),
        HygienicLookup::Selection(MacroSelection::Defined(_))
    ));
    assert_eq!(
        entered.lookup(&selected, &body, at, "hidden").unwrap(),
        HygienicLookup::Selection(MacroSelection::Absent)
    );
}

#[test]
fn entered_invocation_selection_uses_exact_calls_and_body_ownership() {
    use lrr::{
        frontend::macro_names::{MacroNamespaces, PathLookup},
        syntax::spans::SourceRange,
    };
    let source = "macro_rules! choose {()=>{17};} fn selected(){let a=choose!(); {macro_rules! choose {()=>{41};} let b=choose!();} let c=choose!(); fn nested(){choose!();} let closure=||choose!();} fn cold(){choose!();}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let graph = MacroNamespaces::default();
    let calls: Vec<_> = source
        .match_indices("choose!()")
        .map(|(at, _)| SourceRange {
            start: u32::try_from(at).unwrap(),
            end: u32::try_from(at + 9).unwrap(),
        })
        .collect();
    let values: Vec<_> = calls[..3]
        .iter()
        .map(|&range| {
            entered
                .lookup_invocation(&selected, &body, range, &graph)
                .unwrap()
        })
        .collect();
    assert!(values.iter().all(|v| matches!(v, PathLookup::Defined(_))));
    assert_eq!(values[0], values[2]);
    assert_ne!(values[0], values[1]);
    for &range in &calls[3..] {
        assert!(
            entered
                .lookup_invocation(&selected, &body, range, &graph)
                .is_err()
        );
    }
    let mut wrong = body.clone();
    wrong.exact_body_bytes = Digest([55; 32]);
    assert!(
        entered
            .lookup_invocation(&selected, &wrong, calls[0], &graph)
            .is_err()
    );
    assert!(
        entered
            .lookup_invocation(
                &selected,
                &body,
                SourceRange {
                    start: calls[0].start + 1,
                    ..calls[0]
                },
                &graph
            )
            .is_err()
    );
}

#[test]
fn entered_invocations_preserve_cfg_attribute_demand_and_qualified_routes() {
    use lrr::{
        expand::scopes::MacroName,
        frontend::macro_names::{MacroNamespaces, MacroPath, PathLookup},
        syntax::{spans::SourceRange, visibility::VisibilityScope},
    };
    let source = "macro_rules! choose {()=>{17};} fn selected(){let a=crate::choose!(); #[cfg(any())] choose!(); #[transform] choose!();}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let HygienicLookup::Selection(MacroSelection::Defined(root)) = index
        .lookup(
            &selected,
            &[],
            u64::try_from(source.find("fn selected").unwrap()).unwrap(),
            "choose",
        )
        .unwrap()
    else {
        panic!("source root macro missing");
    };
    let mut graph = MacroNamespaces::default();
    graph
        .define_visible(
            MacroPath::new(
                root.defining_unit,
                vec![MacroName::parse("choose", 2024).unwrap()],
            )
            .unwrap(),
            root.clone(),
            VisibilityScope::Within {
                unit: root.defining_unit,
                module_path: vec![],
            },
        )
        .unwrap();
    let selection = |text: &str| {
        let start = source.find(text).unwrap();
        SourceRange {
            start: u32::try_from(start).unwrap(),
            end: u32::try_from(start + text.len()).unwrap(),
        }
    };
    assert_eq!(
        entered
            .lookup_invocation(&selected, &body, selection("crate::choose!()"), &graph)
            .unwrap(),
        PathLookup::Defined(root)
    );
    assert_eq!(
        entered
            .lookup_invocation(
                &selected,
                &body,
                {
                    let start =
                        source.find("#[cfg(any())] choose!();").unwrap() + "#[cfg(any())] ".len();
                    SourceRange {
                        start: u32::try_from(start).unwrap(),
                        end: u32::try_from(start + 9).unwrap(),
                    }
                },
                &graph
            )
            .unwrap(),
        PathLookup::Absent
    );
    let pending = entered
        .lookup_invocation(
            &selected,
            &body,
            {
                let start = source.find("#[transform] choose!();").unwrap() + "#[transform] ".len();
                SourceRange {
                    start: u32::try_from(start).unwrap(),
                    end: u32::try_from(start + 9).unwrap(),
                }
            },
            &graph,
        )
        .unwrap();
    let PathLookup::NeedInput(input) = pending else {
        panic!("transforming body call must suspend");
    };
    assert_eq!(
        entered.producer_kind(input),
        Some(lrr::syntax::outline::ProducerKind::Attribute)
    );
}

#[test]
fn entered_import_facts_preserve_block_scopes_raw_paths_cfg_and_opacity() {
    use lrr::syntax::imports::{ImportKind, ImportSegment};
    let source = "fn selected(){use crate::tools::{r#gen as chosen, self as module}; {use super::r#gen as nested; use crate::tools::*;} #[cfg(any())] use crate::absent; #[transform] use crate::tools::r#gen as generated; fn cold(){use crate::secret;} let closure=||{use crate::secret;}; macro_rules! opaque {()=>{use crate::secret;};}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let imports = &entered.facts().imports;
    assert_eq!(imports.len(), 5);
    assert_eq!(imports[0].scope, 0);
    assert_eq!(
        imports[0].source.path,
        vec![
            ImportSegment::Crate,
            ImportSegment::Name("tools".into()),
            ImportSegment::Name("r#gen".into())
        ]
    );
    assert_eq!(
        imports[0].source.kind,
        ImportKind::Named {
            alias: Some("chosen".into())
        }
    );
    assert_eq!(imports[0].source.scope.owner, Some(body.definition));
    assert_eq!(
        imports[0].source.scope.crate_key,
        selected.outline.crate_key
    );
    assert_eq!(
        imports[1].source.kind,
        ImportKind::Named {
            alias: Some("module".into())
        }
    );
    assert_eq!(imports[2].scope, imports[3].scope);
    assert_ne!(imports[2].scope, 0);
    assert_eq!(imports[3].source.kind, ImportKind::Glob);
    assert!(imports[4].attribute_input.is_some());
    assert_eq!(
        entered.producer_kind(imports[4].attribute_input.unwrap()),
        Some(lrr::syntax::outline::ProducerKind::Attribute)
    );
    assert!(imports.iter().all(|i| {
        !i.source
            .path
            .iter()
            .any(|p| matches!(p,ImportSegment::Name(n) if n=="secret" || n=="absent"))
    }));
}

#[test]
fn body_named_imports_are_scope_wide_and_nested_aliases_shadow_parent_imports() {
    use lrr::{
        expand::scopes::MacroName,
        frontend::macro_names::{MacroNamespaces, MacroPath, PathLookup},
        syntax::spans::SourceRange,
    };
    let source = "macro_rules! one {()=>{17};} macro_rules! two {()=>{31};} fn selected(){let a=chosen!(); use crate::one as chosen; {use crate::two as chosen; let b=chosen!();} let c=chosen!();}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let mut graph = MacroNamespaces::default();
    let before = u64::try_from(source.find("fn selected").unwrap()).unwrap();
    let mut defs = Vec::new();
    for name in ["one", "two"] {
        let HygienicLookup::Selection(MacroSelection::Defined(def)) =
            index.lookup(&selected, &[], before, name).unwrap()
        else {
            panic!("source definition absent");
        };
        graph
            .define(
                MacroPath::new(
                    def.defining_unit,
                    vec![MacroName::parse(name, 2024).unwrap()],
                )
                .unwrap(),
                def.clone(),
            )
            .unwrap();
        defs.push(def);
    }
    let calls: Vec<_> = source
        .match_indices("chosen!()")
        .map(|(at, _)| SourceRange {
            start: u32::try_from(at).unwrap(),
            end: u32::try_from(at + 9).unwrap(),
        })
        .collect();
    let results: Vec<_> = calls
        .into_iter()
        .map(|range| {
            entered
                .lookup_invocation(&selected, &body, range, &graph)
                .unwrap()
        })
        .collect();
    assert_eq!(
        results,
        vec![
            PathLookup::Defined(defs[0].clone()),
            PathLookup::Defined(defs[1].clone()),
            PathLookup::Defined(defs[0].clone())
        ]
    );
}

#[test]
fn body_named_routes_follow_forward_aliases_module_aliases_and_complete_empty_cycles() {
    use lrr::{
        expand::scopes::MacroName,
        frontend::macro_names::{MacroNamespaces, MacroPath, PathLookup},
        syntax::spans::SourceRange,
    };
    let source = "macro_rules! one {()=>{17};} fn selected(){use forwarded as chosen; use crate::one as forwarded; use crate::tools::{self as module}; let a=chosen!(); let b=module::member!(); {use right as left; use left as right; let c=left!();}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let HygienicLookup::Selection(MacroSelection::Defined(def)) = index
        .lookup(
            &selected,
            &[],
            u64::try_from(source.find("fn selected").unwrap()).unwrap(),
            "one",
        )
        .unwrap()
    else {
        panic!("source macro missing");
    };
    let mut graph = MacroNamespaces::default();
    for names in [vec!["one"], vec!["tools", "member"]] {
        graph
            .define(
                MacroPath::new(
                    def.defining_unit,
                    names
                        .into_iter()
                        .map(|n| MacroName::parse(n, 2024).unwrap())
                        .collect(),
                )
                .unwrap(),
                def.clone(),
            )
            .unwrap();
    }
    for text in ["chosen!()", "module::member!()"] {
        let at = source.find(text).unwrap();
        assert_eq!(
            entered
                .lookup_invocation(
                    &selected,
                    &body,
                    SourceRange {
                        start: u32::try_from(at).unwrap(),
                        end: u32::try_from(at + text.len()).unwrap()
                    },
                    &graph
                )
                .unwrap(),
            PathLookup::Defined(def.clone())
        );
    }
    let text = "left!()";
    let at = source.find(text).unwrap();
    assert_eq!(
        entered
            .lookup_invocation(
                &selected,
                &body,
                SourceRange {
                    start: u32::try_from(at).unwrap(),
                    end: u32::try_from(at + text.len()).unwrap()
                },
                &graph
            )
            .unwrap(),
        PathLookup::Absent
    );
}

#[test]
fn body_textual_imports_capture_source_position_without_exposing_opaque_lookup() {
    use lrr::{
        frontend::macro_names::{MacroNamespaces, PathLookup},
        syntax::spans::SourceRange,
    };
    let source = "fn selected(){macro_rules! m {()=>{17};} use m as frozen; use frozen as forwarded; macro_rules! m {()=>{31};} {macro_rules! m {()=>{53};} let a=forwarded!();} let b=frozen!();}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let definition = entered
        .facts()
        .events
        .iter()
        .find_map(|event| match &event.kind {
            BodyMacroEventKind::Definition { identity, name, .. } if name == "m" => Some(*identity),
            _ => None,
        })
        .unwrap();
    let graph = MacroNamespaces::default();
    for needle in ["forwarded!()", "frozen!()"] {
        let at = source.find(needle).unwrap();
        let PathLookup::Defined(def) = entered
            .lookup_invocation(
                &selected,
                &body,
                SourceRange {
                    start: u32::try_from(at).unwrap(),
                    end: u32::try_from(at + needle.len()).unwrap(),
                },
                &graph,
            )
            .unwrap()
        else {
            panic!("frozen textual import missing");
        };
        assert_eq!(def.identity, definition);
    }
    for row in &entered.facts().imports {
        assert!(
            entered.lookup(&selected, &body, row.at, "m").is_err(),
            "public lookup must retain opaque use-item guard"
        );
    }
    let mut wrong = selected.clone();
    wrong.outline.parse.content = Digest([9; 32]);
    let at = source.find("frozen!()").unwrap();
    assert!(
        entered
            .lookup_invocation(
                &wrong,
                &body,
                SourceRange {
                    start: u32::try_from(at).unwrap(),
                    end: u32::try_from(at + 9).unwrap(),
                },
                &graph
            )
            .is_err()
    );
}

fn parsed_macro_graph(source: &str) -> lrr::frontend::macro_names::MacroNamespaces {
    use lrr::{frontend::source_macro_routes::SourceMacroRoutes, syntax::parse::ParsedFile};
    let outline = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .outline(Digest([1; 32]), vec![], &CfgSet::default())
    .unwrap();
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([1; 32]),
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
    SourceMacroRoutes::candidate_graph(&[SourceMacroRoutes::from_outline(
        &outline,
        context,
        facts,
        &Default::default(),
    )
    .unwrap()])
    .unwrap()
}
#[test]
fn body_glob_alias_targets_deduplicate_and_preserve_outer_scope_ambiguity() {
    use lrr::{frontend::macro_names::PathLookup, syntax::spans::SourceRange};
    let prefix = "#[macro_export] macro_rules! one {()=>{17};} #[macro_export] macro_rules! two {()=>{31};} mod a {pub use crate::one as chosen;} mod b {pub use crate::two as chosen;}";
    for (body_source, ambiguous) in [
        (
            "fn selected(){use crate::a::{self as module}; use module::*; use crate::a::*; let x=chosen!();}",
            false,
        ),
        (
            "fn selected(){use crate::a::chosen; {use crate::b::*; let x=chosen!();}}",
            true,
        ),
    ] {
        let source = format!("{prefix}{body_source}");
        let graph = parsed_macro_graph(&source);
        let (index, selected, body) = prepared(&source);
        let entered = index.enter_body(&selected, &body).unwrap();
        let at = source.find("chosen!()").unwrap();
        let result = entered
            .lookup_invocation(
                &selected,
                &body,
                SourceRange {
                    start: u32::try_from(at).unwrap(),
                    end: u32::try_from(at + 9).unwrap(),
                },
                &graph,
            )
            .unwrap();
        if ambiguous {
            let PathLookup::Ambiguous(defs) = result else {
                panic!("inner glob/outer import ambiguity lost")
            };
            assert_eq!(defs.len(), 2);
            assert_ne!(defs[0].identity, defs[1].identity);
        } else {
            let PathLookup::Defined(def) = result else {
                panic!("glob diamond identity lost")
            };
            let HygienicLookup::Selection(MacroSelection::Defined(original)) = index
                .lookup(
                    &selected,
                    &[],
                    u64::try_from(source.find("fn selected").unwrap()).unwrap(),
                    "one",
                )
                .unwrap()
            else {
                panic!("original missing")
            };
            assert_eq!(def, original);
        }
    }
}
#[test]
fn body_globs_filter_private_members_and_preserve_transforming_import_demand() {
    use lrr::{frontend::macro_names::PathLookup, syntax::spans::SourceRange};
    let prefix = "#[macro_export] macro_rules! one {()=>{17};} #[macro_export] macro_rules! two {()=>{31};} mod a {pub use crate::one as chosen;} mod b {use crate::two as chosen;}";
    for (body_source, pending) in [
        (
            "fn selected(){use crate::a::chosen; {use crate::b::*; let x=chosen!();}}",
            false,
        ),
        (
            "fn selected(){#[transform] use crate::a::*; let x=chosen!();}",
            true,
        ),
    ] {
        let source = format!("{prefix}{body_source}");
        let graph = parsed_macro_graph(&source);
        let (index, selected, body) = prepared(&source);
        let entered = index.enter_body(&selected, &body).unwrap();
        let at = source.find("chosen!()").unwrap();
        let result = entered
            .lookup_invocation(
                &selected,
                &body,
                SourceRange {
                    start: u32::try_from(at).unwrap(),
                    end: u32::try_from(at + 9).unwrap(),
                },
                &graph,
            )
            .unwrap();
        if pending {
            assert!(matches!(result, PathLookup::NeedInput(_)));
        } else {
            let PathLookup::Defined(def) = result else {
                panic!("private glob member replaced accessible outer import")
            };
            let HygienicLookup::Selection(MacroSelection::Defined(original)) = index
                .lookup(
                    &selected,
                    &[],
                    u64::try_from(source.find("fn selected").unwrap()).unwrap(),
                    "one",
                )
                .unwrap()
            else {
                panic!("original missing")
            };
            assert_eq!(def, original);
        }
    }
}

#[test]
fn body_glob_modules_support_chains_qualified_calls_and_separate_macro_facets() {
    use lrr::{frontend::macro_names::PathLookup, syntax::spans::SourceRange};
    let source = "#[macro_export] macro_rules! one {()=>{17};} #[macro_export] macro_rules! two {()=>{31};} mod a {pub mod nested {pub use crate::one as chosen;}} mod origin {mod hidden {pub mod nested {pub use crate::one as chosen;}} pub use hidden::nested as escaped;} fn selected(){use crate::a::*; use nested::*; use nested as nested; use nested as forwarded; let a=chosen!(); let b=forwarded::chosen!(); {use crate::two as nested; use crate::a::*; let c=nested::chosen!();} {use crate::origin::escaped as route; use route::*; let d=chosen!();}}";
    let graph = parsed_macro_graph(source);
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let HygienicLookup::Selection(MacroSelection::Defined(original)) = index
        .lookup(
            &selected,
            &[],
            u64::try_from(source.find("fn selected").unwrap()).unwrap(),
            "one",
        )
        .unwrap()
    else {
        panic!("original missing")
    };
    for call in &entered.facts().invocations {
        let PathLookup::Defined(def) = entered
            .lookup_invocation(
                &selected,
                &body,
                SourceRange {
                    start: call.range.start,
                    end: call.range.end,
                },
                &graph,
            )
            .unwrap()
        else {
            panic!("body module route did not select a macro")
        };
        assert_eq!(def, original);
    }
    assert_eq!(entered.facts().invocations.len(), 4);
}
#[test]
fn body_module_tables_settle_empty_cycles_without_weakly_seeding_strong_cycles() {
    use lrr::{frontend::macro_names::PathLookup, syntax::spans::SourceRange};
    let prefix = "#[macro_export] macro_rules! one {()=>{17};} mod a {pub mod nested {pub use crate::one as chosen;}}";
    for body_source in [
        "fn selected(){use left::*; use right::*; let x=chosen!();}",
        "fn selected(){use forward as nested; use nested as forward; use crate::a::*; let x=nested::chosen!();}",
    ] {
        let source = format!("{prefix}{body_source}");
        let graph = parsed_macro_graph(&source);
        let (index, selected, body) = prepared(&source);
        let entered = index.enter_body(&selected, &body).unwrap();
        let call = &entered.facts().invocations[0];
        assert_eq!(
            entered
                .lookup_invocation(
                    &selected,
                    &body,
                    SourceRange {
                        start: call.range.start,
                        end: call.range.end
                    },
                    &graph
                )
                .unwrap(),
            PathLookup::Absent
        );
    }
}

#[test]
fn body_glob_module_ambiguity_survives_equal_macro_definition_identity() {
    let source = "#[macro_export] macro_rules! one {()=>{17};} mod a {pub mod nested {pub use crate::one as chosen;}} mod b {pub mod nested {pub use crate::one as chosen;}} fn selected(){use crate::a::*;use crate::b::*;let x=nested::chosen!();}";
    let graph = parsed_macro_graph(source);
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let error = entered
        .lookup_invocation(
            &selected,
            &body,
            entered.facts().invocations[0].range,
            &graph,
        )
        .unwrap_err();
    assert!(error.message.contains("ambiguous body module prefix"));
}

#[test]
fn body_module_queries_do_not_resolve_unselected_child_scope_imports() {
    use lrr::frontend::macro_names::PathLookup;
    let source = "#[macro_export] macro_rules! one {()=>{17};} mod a {pub use crate::one as chosen;} fn selected(){use crate::a as route; let x=route::chosen!(); {use super::outside::*; let y=chosen!();}}";
    let graph = parsed_macro_graph(source);
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    assert_eq!(entered.facts().invocations.len(), 2);
    assert!(matches!(
        entered
            .lookup_invocation(
                &selected,
                &body,
                entered.facts().invocations[0].range,
                &graph
            )
            .unwrap(),
        PathLookup::Defined(_)
    ));
    let error = entered
        .lookup_invocation(
            &selected,
            &body,
            entered.facts().invocations[1].range,
            &graph,
        )
        .unwrap_err();
    assert!(error.message.contains("escapes crate root"));
}

#[test]
fn body_item_headers_preserve_namespaces_cfg_attributes_and_opaque_contents() {
    use lrr::{
        source::manifest::{ItemKind, Namespace},
        syntax::ast_adapter::token_text,
    };
    let source = "fn selected(){struct Unit; struct Tuple(u64); struct Record{field:u64} enum E{A} union U{field:u64} type Alias=u64; const C:u64=17; static S:u64=31; trait Trait{fn method(){cold!();}} impl Trait for Unit{fn method(){cold!();}} mod module{fn hidden(){cold!();}} unsafe extern \"C\" {fn imported();} fn local(){cold!();} #[cfg(any())] struct Hidden; #[derive(Unknown)] struct Pending; macro_rules! m {()=>{cold!()};} {struct Unit;} let x=m!();}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let items = &entered.facts().items;
    assert!(
        items
            .iter()
            .all(|i| i.item.def_key.declared_name != "Hidden")
    );
    let units: Vec<_> = items
        .iter()
        .filter(|i| i.item.def_key.declared_name == "Unit")
        .collect();
    assert_eq!(units.len(), 2);
    assert_ne!(
        units[0].item.def_key.fingerprint(),
        units[1].item.def_key.fingerprint()
    );
    for name in ["Unit", "Tuple"] {
        let item = items
            .iter()
            .find(|i| i.item.def_key.declared_name == name)
            .unwrap();
        assert_eq!(item.namespaces, vec![Namespace::Type, Namespace::Value]);
    }
    let record = items
        .iter()
        .find(|i| i.item.def_key.declared_name == "Record")
        .unwrap();
    assert_eq!(record.namespaces, vec![Namespace::Type]);
    assert!(token_text(&record.item.header.tokens).contains("field"));
    for name in ["local", "module", "Trait"] {
        let item = items
            .iter()
            .find(|i| i.item.def_key.declared_name == name)
            .unwrap();
        assert!(!token_text(&item.item.header.tokens).contains("cold"));
    }
    assert!(
        items
            .iter()
            .any(|i| i.item.def_key.item_kind == ItemKind::ExternBlock
                && i.container_range.is_some())
    );
    assert!(
        items
            .iter()
            .any(|i| i.item.def_key.item_kind == ItemKind::Impl && i.container_range.is_some())
    );
    let pending = items
        .iter()
        .find(|i| i.item.def_key.declared_name == "Pending")
        .unwrap();
    assert!(pending.attribute_input.is_some());
    let m = items
        .iter()
        .find(|i| i.item.def_key.declared_name == "m")
        .unwrap();
    let event = entered
        .facts()
        .events
        .iter()
        .find_map(|e| match &e.kind {
            BodyMacroEventKind::Definition { identity, name, .. } if name == "m" => Some(*identity),
            _ => None,
        })
        .unwrap();
    assert_eq!(m.item.def_key.fingerprint(), event);
    assert_eq!(entered.facts().invocations.len(), 1);
}
#[test]
fn body_item_identity_and_header_ignore_cold_body_edits_but_keep_exact_handles() {
    fn item(source: &str) -> lrr::syntax::body_items::BodyItem {
        let (index, selected, body) = prepared(source);
        let entered = index.enter_body(&selected, &body).unwrap();
        entered
            .facts()
            .items
            .iter()
            .find(|i| i.item.def_key.declared_name == "local")
            .unwrap()
            .clone()
    }
    let a = item("fn selected(){fn local()->u64 {17}} ");
    let b = item("fn selected(){fn local()->u64 {31}} ");
    assert_eq!(a.item.def_key, b.item.def_key);
    assert_eq!(a.item.header.fingerprint(), b.item.header.fingerprint());
    assert_ne!(
        a.item.body.as_ref().unwrap().body_hash,
        b.item.body.as_ref().unwrap().body_hash
    );
    let blocked = item("fn selected(){#[transform] fn local()->u64 {17}} ");
    assert!(blocked.attribute_input.is_some());
    assert!(blocked.item.body.unwrap().require_entry().is_err());
}

#[test]
fn nested_function_entry_captures_textual_order_and_inherited_import_context() {
    use lrr::frontend::macro_names::{MacroNamespaces, PathLookup};
    let source = "fn selected(){macro_rules! m {()=>{17};} use m as frozen; fn inner(){let a=m!();let b=frozen!();fn leaf(){frozen!();}} macro_rules! m {()=>{31};}}";
    let (index, selected, body) = prepared(source);
    let parent = index.enter_body(&selected, &body).unwrap();
    let old = parent
        .facts()
        .events
        .iter()
        .find_map(|e| match &e.kind {
            BodyMacroEventKind::Definition { identity, name, .. } if name == "m" => Some(*identity),
            _ => None,
        })
        .unwrap();
    let key = parent
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "inner")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let child = parent.enter_nested_body(&selected, &body, &key).unwrap();
    let graph = MacroNamespaces::default();
    for call in &child.facts().invocations {
        let PathLookup::Defined(def) = child
            .lookup_invocation(&selected, &key, call.range, &graph)
            .unwrap()
        else {
            panic!("nested capture missing")
        };
        assert_eq!(def.identity, old);
        assert!(
            parent
                .lookup_invocation(&selected, &body, call.range, &graph)
                .is_err()
        );
    }
    let leaf = child
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "leaf")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let grandchild = child.enter_nested_body(&selected, &key, &leaf).unwrap();
    let PathLookup::Defined(def) = grandchild
        .lookup_invocation(
            &selected,
            &leaf,
            grandchild.facts().invocations[0].range,
            &graph,
        )
        .unwrap()
    else {
        panic!("grandchild capture missing")
    };
    assert_eq!(def.identity, old);
    let mut wrong = key.clone();
    wrong.exact_body_bytes = Digest([9; 32]);
    assert!(parent.enter_nested_body(&selected, &body, &wrong).is_err());
    assert!(parent.enter_nested_body(&selected, &key, &key).is_err());
    let mut changed = selected.clone();
    changed.outline.parse.content = Digest([9; 32]);
    assert!(parent.enter_nested_body(&changed, &body, &key).is_err());
}
#[test]
fn nested_import_frames_preserve_glob_ambiguity_and_transforming_entry_guards() {
    use lrr::frontend::macro_names::PathLookup;
    let source = "#[macro_export] macro_rules! one {()=>{17};} #[macro_export] macro_rules! two {()=>{31};} mod a {pub use crate::one as chosen;} mod b {pub use crate::two as chosen;} fn selected(){use crate::a::chosen;fn inner(){use crate::b::*;let x=chosen!();} #[transform] fn blocked(){chosen!();}}";
    let graph = parsed_macro_graph(source);
    let (index, selected, body) = prepared(source);
    let parent = index.enter_body(&selected, &body).unwrap();
    let key = parent
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "inner")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let child = parent.enter_nested_body(&selected, &body, &key).unwrap();
    let PathLookup::Ambiguous(defs) = child
        .lookup_invocation(&selected, &key, child.facts().invocations[0].range, &graph)
        .unwrap()
    else {
        panic!("nested glob/outer alias ambiguity lost")
    };
    assert_eq!(defs.len(), 2);
    let key = parent
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "blocked")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    assert!(parent.enter_nested_body(&selected, &body, &key).is_err());
}

#[test]
fn explicit_container_entry_keeps_associated_context_and_executable_contents_opaque() {
    use lrr::{source::manifest::ItemKind, syntax::ast_adapter::token_text};
    let source = "fn selected(){mod local {struct S;mod nested{fn cold(){unresolved!();}} macro_rules! m {()=>{17};}} trait Trait<T>{type Output;const C:u64=17;fn method()->u64{unresolved!();}} impl<T> Trait<T> for Local {type Output=T;const C:u64=31;fn method()->u64{unresolved!();}} unsafe extern \"C\" {fn foreign();}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    for kind in [
        ItemKind::Module,
        ItemKind::Trait,
        ItemKind::Impl,
        ItemKind::ExternBlock,
    ] {
        let item = entered
            .facts()
            .items
            .iter()
            .find(|i| i.item.def_key.item_kind == kind)
            .unwrap();
        let key = &item.container.as_ref().unwrap().key;
        let catalogue = entered.enter_container(&selected, &body, key).unwrap();
        assert_eq!(catalogue.key(), key);
        assert!(!catalogue.facts().items.is_empty());
        assert!(catalogue.facts().invocations.is_empty());
        assert!(
            catalogue
                .facts()
                .items
                .iter()
                .all(|i| !token_text(&i.item.header.tokens).contains("unresolved"))
        );
        if matches!(
            kind,
            ItemKind::Trait | ItemKind::Impl | ItemKind::ExternBlock
        ) {
            assert!(
                catalogue
                    .facts()
                    .items
                    .iter()
                    .all(|i| !i.item.header.context.is_empty())
            );
        }
        if kind == ItemKind::Module {
            let nested = catalogue
                .facts()
                .items
                .iter()
                .find(|i| i.item.def_key.declared_name == "nested")
                .unwrap();
            let nested_key = &nested.container.as_ref().unwrap().key;
            let inner = catalogue
                .enter_container(&selected, key, nested_key)
                .unwrap();
            assert_eq!(inner.facts().items.len(), 1);
            assert_eq!(inner.facts().items[0].item.def_key.declared_name, "cold");
            assert_eq!(
                inner.facts().items[0].item.scope.module_path,
                vec!["local", "nested"]
            );
            assert!(inner.facts().items[0].item.body.is_some());
            let template = catalogue
                .facts()
                .items
                .iter()
                .find(|i| i.item.def_key.declared_name == "m")
                .unwrap();
            assert!(
                catalogue
                    .enter_container(&selected, key, &template.container.as_ref().unwrap().key)
                    .is_err()
            );
        }
    }
}
#[test]
fn container_keys_reject_changed_source_content_parent_and_transforming_entry() {
    let source = "fn selected(){mod local {fn f(){17;}} #[transform] mod blocked {fn f(){31;}}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let key = entered
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "local")
        .unwrap()
        .container
        .as_ref()
        .unwrap()
        .key
        .clone();
    let mut changed = key.clone();
    changed.content = Digest([9; 32]);
    assert!(entered.enter_container(&selected, &body, &changed).is_err());
    let mut source_changed = selected.clone();
    source_changed.outline.parse.content = Digest([9; 32]);
    assert!(
        entered
            .enter_container(&source_changed, &body, &key)
            .is_err()
    );
    let mut wrong = body.clone();
    wrong.exact_body_bytes = Digest([9; 32]);
    assert!(entered.enter_container(&selected, &wrong, &key).is_err());
    let blocked = entered
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "blocked")
        .unwrap()
        .container
        .as_ref()
        .unwrap()
        .key
        .clone();
    assert!(entered.enter_container(&selected, &body, &blocked).is_err());
    let (index2, selected2, body2) = prepared("fn selected(){mod local {fn f(){31;}}}");
    let entered2 = index2.enter_body(&selected2, &body2).unwrap();
    let key2 = entered2.facts().items[0]
        .container
        .as_ref()
        .unwrap()
        .key
        .clone();
    assert_eq!(key.definition, key2.definition);
    assert_ne!(key.content, key2.content);
}

#[test]
fn container_function_entry_captures_textual_macros_and_module_import_boundaries() {
    use lrr::frontend::macro_names::{MacroNamespaces, PathLookup};
    let source = "fn selected(){macro_rules! m {()=>{17};} use m as outer_alias; mod local {use m as frozen;fn early(){let a=m!();let b=frozen!();let c=outer_alias!();} macro_rules! m {()=>{41};} mod nested {fn leaf(){let a=m!();}} macro_rules! m {()=>{61};}} trait Trait {fn method(){let a=m!();let b=outer_alias!();}} impl Local {fn method(){let a=m!();let b=outer_alias!();}} macro_rules! m {()=>{31};}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let original = entered
        .facts()
        .events
        .iter()
        .find_map(|e| match &e.kind {
            BodyMacroEventKind::Definition { identity, name, .. } if name == "m" => Some(*identity),
            _ => None,
        })
        .unwrap();
    let graph = MacroNamespaces::default();
    for name in ["local", "Trait", ""] {
        let item = entered
            .facts()
            .items
            .iter()
            .find(|i| i.item.def_key.declared_name == name && i.container.is_some())
            .unwrap();
        let key = &item.container.as_ref().unwrap().key;
        let container = entered.enter_container(&selected, &body, key).unwrap();
        let member = container
            .facts()
            .items
            .iter()
            .find(|i| i.item.body.is_some())
            .unwrap();
        let member_key = member.item.body.as_ref().unwrap().key();
        let child = container.enter_body(&selected, key, &member_key).unwrap();
        for (ordinal, call) in child.facts().invocations.iter().enumerate() {
            let result = child
                .lookup_invocation(&selected, &member_key, call.range, &graph)
                .unwrap();
            if name == "local" && ordinal == 2 {
                assert_eq!(
                    result,
                    PathLookup::Absent,
                    "module captured enclosing body import"
                );
            } else {
                let PathLookup::Defined(definition) = result else {
                    panic!("captured macro missing in {name:?} at {ordinal}: {result:?}")
                };
                assert_eq!(definition.identity, original);
            }
        }
        if name == "local" {
            let nested = container
                .facts()
                .items
                .iter()
                .find(|i| i.item.def_key.declared_name == "nested")
                .unwrap();
            let nested_key = &nested.container.as_ref().unwrap().key;
            let inner = container
                .enter_container(&selected, key, nested_key)
                .unwrap();
            let leaf_key = inner.facts().items[0].item.body.as_ref().unwrap().key();
            let leaf = inner.enter_body(&selected, nested_key, &leaf_key).unwrap();
            let PathLookup::Defined(definition) = leaf
                .lookup_invocation(
                    &selected,
                    &leaf_key,
                    leaf.facts().invocations[0].range,
                    &graph,
                )
                .unwrap()
            else {
                panic!("nested module capture missing")
            };
            let expected = container
                .facts()
                .events
                .iter()
                .find_map(|e| match &e.kind {
                    BodyMacroEventKind::Definition { identity, name, .. } if name == "m" => {
                        Some(*identity)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(definition.identity, expected);
        }
    }
}

#[test]
fn container_function_entry_requires_exact_declared_parent_body_and_source() {
    let source = "fn selected(){mod local {fn good(){17;} #[transform] fn blocked(){31;} mod nested {fn other(){41;}}}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let key = &entered.facts().items[0].container.as_ref().unwrap().key;
    let catalogue = entered.enter_container(&selected, &body, key).unwrap();
    let good = catalogue
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "good")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    assert!(catalogue.enter_body(&selected, key, &good).is_ok());
    let mut changed = key.clone();
    changed.content = Digest([9; 32]);
    assert!(catalogue.enter_body(&selected, &changed, &good).is_err());
    let mut stale = good.clone();
    stale.exact_body_bytes = Digest([9; 32]);
    assert!(catalogue.enter_body(&selected, key, &stale).is_err());
    let mut stale_source = selected.clone();
    stale_source.outline.parse.content = Digest([9; 32]);
    assert!(catalogue.enter_body(&stale_source, key, &good).is_err());
    let blocked = catalogue
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "blocked")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    assert!(catalogue.enter_body(&selected, key, &blocked).is_err());
    let nested_key = &catalogue
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "nested")
        .unwrap()
        .container
        .as_ref()
        .unwrap()
        .key;
    let nested = catalogue
        .enter_container(&selected, key, nested_key)
        .unwrap();
    let other = nested.facts().items[0].item.body.as_ref().unwrap().key();
    assert!(catalogue.enter_body(&selected, key, &other).is_err());
}

#[test]
fn container_function_entry_preserves_prior_expansion_frontier_and_producer_identity() {
    use lrr::{
        frontend::macro_names::{MacroNamespaces, PathLookup},
        syntax::outline::ProducerKind,
    };
    let source = "fn selected(){macro_rules! m {()=>{17};} mod local {emit!();macro_rules! m {()=>{41};} fn inner(){let x=m!();}}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let key = &entered
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "local")
        .unwrap()
        .container
        .as_ref()
        .unwrap()
        .key;
    let catalogue = entered.enter_container(&selected, &body, key).unwrap();
    let producer = catalogue
        .facts()
        .events
        .iter()
        .find_map(|e| match &e.kind {
            BodyMacroEventKind::Producer {
                identity,
                kind: ProducerKind::ItemMacro,
            } => Some(*identity),
            _ => None,
        })
        .unwrap();
    let child_key = catalogue
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "inner")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let child = catalogue.enter_body(&selected, key, &child_key).unwrap();
    assert_eq!(child.producer_kind(producer), Some(ProducerKind::ItemMacro));
    assert_eq!(
        child
            .lookup_invocation(
                &selected,
                &child_key,
                child.facts().invocations[0].range,
                &MacroNamespaces::default()
            )
            .unwrap(),
        PathLookup::NeedInput(producer)
    );
}

#[test]
fn original_container_item_calls_all_register_expansion_frontiers() {
    use lrr::{source::manifest::ItemKind, syntax::outline::ProducerKind};
    let source = "fn selected(){mod local {emit!();} trait Trait {emit!();} impl Local {emit!();} unsafe extern \"C\" {emit!();}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    for kind in [
        ItemKind::Module,
        ItemKind::Trait,
        ItemKind::Impl,
        ItemKind::ExternBlock,
    ] {
        let key = &entered
            .facts()
            .items
            .iter()
            .find(|i| i.item.def_key.item_kind == kind)
            .unwrap()
            .container
            .as_ref()
            .unwrap()
            .key;
        let catalogue = entered.enter_container(&selected, &body, key).unwrap();
        assert_eq!(catalogue.facts().invocations.len(), 1);
        assert_eq!(catalogue.facts().events.len(), 1);
        let BodyMacroEventKind::Producer {
            identity: _,
            kind: ProducerKind::ItemMacro,
        } = &catalogue.facts().events[0].kind
        else {
            panic!("item macro frontier absent in {kind:?}")
        };
        assert_eq!(
            catalogue.facts().events[0].range,
            catalogue.facts().invocations[0].range
        );
    }
}

#[test]
fn original_binding_candidates_preserve_patterns_visibility_cfg_and_opaque_boundaries() {
    use lrr::syntax::local_bindings::{BindingForm, PatternNameKind};
    let source = "fn selected((mut a,r#gen):(u64,u64),#[cfg(any())] gone:u64){let (x,mut y)=(1,2);let z @ Some(ref w)=None;let x=x;{let x=3;}#[cfg(any())]let hidden=missing!();#[cfg_attr(all(),cfg(any()))]let hidden2=missing!();let c=||{let closure_only=1;};fn nested(param:u64){let cold=param;}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let bindings = &entered.facts().bindings;
    assert_eq!(bindings.len(), 9);
    assert_eq!(
        bindings.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
        vec!["a", "gen", "x", "y", "z", "w", "x", "x", "c"]
    );
    let raw = bindings.iter().find(|b| b.name == "gen").unwrap();
    assert_eq!(raw.raw_name, "r#gen");
    assert_eq!(raw.form, BindingForm::Parameter);
    assert_eq!(
        &source[raw.name_range.start as usize..raw.name_range.end as usize],
        raw.raw_name
    );
    for binding in &bindings[..2] {
        assert_eq!(
            binding.visible_from,
            u64::from(entered.facts().scopes[0].range.start)
        );
    }
    for name in ["a", "y", "z", "w"] {
        assert_eq!(
            bindings
                .iter()
                .find(|b| b.name == name)
                .unwrap()
                .classification,
            PatternNameKind::ExplicitBinding
        );
    }
    let xs: Vec<_> = bindings.iter().filter(|b| b.name == "x").collect();
    assert_eq!(xs.len(), 3);
    assert!(
        xs.iter()
            .all(|b| b.classification == PatternNameKind::BindingOrConstant)
    );
    assert_eq!(xs[0].scope, 0);
    assert_eq!(xs[1].scope, 0);
    assert_ne!(xs[2].scope, 0);
    assert!(xs[1].visible_from > u64::try_from(source.find("let x=x;").unwrap() + 6).unwrap());
    assert!(entered.facts().invocations.is_empty());
    let ids: std::collections::BTreeSet<_> = bindings.iter().map(|b| b.identity).collect();
    assert_eq!(ids.len(), bindings.len());
    let child_key = entered
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "nested")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let child = entered
        .enter_nested_body(&selected, &body, &child_key)
        .unwrap();
    assert_eq!(
        child
            .facts()
            .bindings
            .iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>(),
        vec!["param", "cold"]
    );
}

#[test]
fn original_binding_identities_ignore_initializer_bytes_and_normalize_source_names() {
    let first = "fn selected(){let r#café=1;let cafe\u{301}=2;}";
    let second = "fn selected(){let r#café=999;let cafe\u{301}=222;}";
    let (index1, selected1, body1) = prepared(first);
    let a = index1.enter_body(&selected1, &body1).unwrap();
    let (index2, selected2, body2) = prepared(second);
    let b = index2.enter_body(&selected2, &body2).unwrap();
    assert_ne!(body1.exact_body_bytes, body2.exact_body_bytes);
    assert_eq!(a.facts().bindings.len(), 2);
    assert_eq!(b.facts().bindings.len(), 2);
    for (x, y) in a.facts().bindings.iter().zip(&b.facts().bindings) {
        assert_eq!(x.name, "café");
        assert_eq!(x.name, y.name);
        assert_eq!(x.identity, y.identity);
        assert_eq!(x.raw_name, y.raw_name);
    }
    assert_ne!(
        a.facts().bindings[0].identity,
        a.facts().bindings[1].identity
    );
    assert!(index1.enter_body(&selected2, &body2).is_err());
}

#[test]
fn original_binding_patterns_preserve_record_shorthand_and_alternative_occurrences() {
    let source = "fn selected(){let S {field: renamed, shorthand, ref borrowed,..}=value;let (Some(a)|Some(a))=value;let const {let hidden=1;hidden}=1;}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let root: Vec<_> = entered
        .facts()
        .bindings
        .iter()
        .filter(|b| b.scope == 0)
        .collect();
    assert_eq!(
        root.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
        vec!["renamed", "shorthand", "borrowed", "a", "a"]
    );
    assert_eq!(
        root[1].classification,
        lrr::syntax::local_bindings::PatternNameKind::ExplicitBinding
    );
    assert_eq!(root[3].pattern_range, root[4].pattern_range);
    assert_ne!(root[3].name_range, root[4].name_range);
    assert_ne!(root[3].identity, root[4].identity);
    assert!(
        entered
            .facts()
            .bindings
            .iter()
            .filter(|b| b.name == "hidden")
            .all(|b| b.scope != 0)
    );
}

#[test]
fn source_local_lookup_preserves_shadowing_initializer_destructuring_and_scope_order() {
    use lrr::frontend::{local_scopes::LocalLookup, source_locals::PatternResolution};
    let source = "fn selected((a,b):(u64,u64)){let mut x=17;let mut y=19;let mut x=x;{let mut x=31;let nested=x;}let result=x;}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selected, &body, &resolutions, &[])
        .unwrap();
    let context = locals.context();
    let xs: Vec<_> = entered
        .facts()
        .bindings
        .iter()
        .filter(|b| b.name == "x")
        .collect();
    for (token, expected) in [
        ("x;{", xs[0].identity),
        ("x;}let", xs[2].identity),
        ("x;}", xs[2].identity),
    ] {
        let at = u64::try_from(source.find(token).unwrap()).unwrap();
        assert_eq!(
            locals.lookup(&selected, &body, at, "x", context).unwrap(),
            LocalLookup::Binding(expected)
        );
    }
    let at = u64::try_from(source.rfind("x;").unwrap()).unwrap();
    assert_eq!(
        locals.lookup(&selected, &body, at, "x", context).unwrap(),
        LocalLookup::Binding(xs[1].identity)
    );
    for name in ["a", "b"] {
        let id = entered
            .facts()
            .bindings
            .iter()
            .find(|b| b.name == name)
            .unwrap()
            .identity;
        assert_eq!(
            locals.lookup(&selected, &body, at, name, context).unwrap(),
            LocalLookup::Binding(id)
        );
    }
    assert_eq!(
        locals
            .lookup(&selected, &body, at, "missing", context)
            .unwrap(),
        LocalLookup::Absent
    );
}

#[test]
fn source_local_pattern_demands_are_name_specific_and_retained_source_checked() {
    use lrr::frontend::{local_scopes::LocalLookup, source_locals::PatternResolution};
    let source =
        "fn selected(){let mut y=17;let x=31;let z=41;let result=y;fn cold(){let mut y=99;}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let x = entered
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "x")
        .unwrap()
        .identity;
    let z = entered
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "z")
        .unwrap()
        .identity;
    let y = entered
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "y")
        .unwrap()
        .identity;
    let at = u64::try_from(source.find("y;fn").unwrap()).unwrap();
    let resolutions = std::collections::BTreeMap::from([(z, PatternResolution::Constant)]);
    let locals = entered
        .local_index(&selected, &body, &resolutions, &[])
        .unwrap();
    let context = locals.context();
    assert_eq!(
        locals.lookup(&selected, &body, at, "y", context).unwrap(),
        LocalLookup::Binding(y)
    );
    assert_eq!(
        locals.lookup(&selected, &body, at, "x", context).unwrap(),
        LocalLookup::NeedInput(x)
    );
    assert_eq!(
        locals.lookup(&selected, &body, at, "z", context).unwrap(),
        LocalLookup::Absent
    );
    let mut stale = selected.clone();
    stale.outline.parse.content = Digest([9; 32]);
    assert!(locals.lookup(&stale, &body, at, "y", context).is_err());
    assert!(
        entered
            .local_index(&stale, &body, &resolutions, &[])
            .is_err()
    );
    let mut wrong = body.clone();
    wrong.exact_body_bytes = Digest([9; 32]);
    assert!(locals.lookup(&selected, &wrong, at, "y", context).is_err());
    assert!(
        locals
            .lookup(
                &selected,
                &body,
                u64::try_from(source.find("99").unwrap()).unwrap(),
                "y",
                context
            )
            .is_err()
    );
    assert!(
        entered
            .local_index(
                &selected,
                &body,
                &std::collections::BTreeMap::from([(Digest([9; 32]), PatternResolution::Binding)]),
                &[]
            )
            .is_err()
    );
    assert!(
        entered
            .local_index(
                &selected,
                &body,
                &std::collections::BTreeMap::from([(y, PatternResolution::Constant)]),
                &[]
            )
            .is_err()
    );
}

#[test]
fn source_local_lookup_keeps_prior_producers_and_excludes_the_current_call_frontier() {
    use lrr::frontend::{local_scopes::LocalLookup, source_locals::PatternResolution};
    let source = "fn selected(){let mut y=17;emit!();let mut x=31;println!(\"{}\",x);}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let locals = entered
        .local_index(
            &selected,
            &body,
            &std::collections::BTreeMap::<Digest, PatternResolution>::new(),
            &[],
        )
        .unwrap();
    let at = u64::try_from(source.find("println!").unwrap()).unwrap();
    let x = entered
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "x")
        .unwrap()
        .identity;
    let producer = entered
        .facts()
        .events
        .iter()
        .find_map(|e| match e.kind {
            BodyMacroEventKind::Producer { identity, .. } => Some(identity),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        locals
            .lookup(&selected, &body, at, "x", locals.context())
            .unwrap(),
        LocalLookup::Binding(x)
    );
    assert_eq!(
        locals
            .lookup(&selected, &body, at, "y", locals.context())
            .unwrap(),
        LocalLookup::NeedInput(producer)
    );
}

#[test]
fn source_local_lookup_requires_pattern_equivalence_and_isolates_named_function_locals() {
    use lrr::frontend::{local_scopes::LocalLookup, source_locals::PatternResolution};
    let source = "fn selected(){let mut outer=17;let (Some(a)|Some(a))=value;fn nested(){let mut local=31;let result=local;}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let alternatives: std::collections::BTreeMap<_, _> = entered
        .facts()
        .bindings
        .iter()
        .filter(|b| b.name == "a")
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    assert_eq!(alternatives.len(), 2);
    assert!(
        entered
            .local_index(&selected, &body, &alternatives, &[])
            .is_err()
    );
    assert!(
        entered
            .local_index(&selected, &body, &Default::default(), &[])
            .is_ok()
    );
    let key = entered
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "nested")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let child = entered.enter_nested_body(&selected, &body, &key).unwrap();
    let locals = child
        .local_index(&selected, &key, &Default::default(), &[])
        .unwrap();
    let at = u64::try_from(source.find("local;").unwrap()).unwrap();
    let local = child
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "local")
        .unwrap()
        .identity;
    assert_eq!(
        locals
            .lookup(&selected, &key, at, "local", locals.context())
            .unwrap(),
        LocalLookup::Binding(local)
    );
    assert_eq!(
        locals
            .lookup(&selected, &key, at, "outer", locals.context())
            .unwrap(),
        LocalLookup::Absent
    );
}

#[test]
fn for_binding_candidates_and_lookup_keep_iterators_in_the_parent_scope() {
    use lrr::{frontend::local_scopes::LocalLookup, syntax::local_bindings::BindingForm};
    let source = "fn selected(){let mut x=17;for (mut x,mut y) in {let iterator=x;[(31,41)]} {let body=x;for mut x in [x] {let nested=x;}let after_nested=x;}let after=x;#[cfg(any())]for mut hidden in missing!(){missing!();}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let loops: Vec<_> = entered
        .facts()
        .bindings
        .iter()
        .filter(|b| b.form == BindingForm::For)
        .collect();
    assert_eq!(loops.len(), 3);
    assert_eq!(loops[0].name, "x");
    assert_eq!(loops[1].name, "y");
    assert_eq!(loops[2].name, "x");
    assert_eq!(loops[0].scope, loops[1].scope);
    assert_ne!(loops[0].scope, loops[2].scope);
    let outer = entered
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "x" && b.form == BindingForm::Let)
        .unwrap()
        .identity;
    let locals = entered
        .local_index(&selected, &body, &Default::default(), &[])
        .unwrap();
    for (token, offset, expected) in [
        ("iterator=x", 9, outer),
        ("body=x", 5, loops[0].identity),
        ("in [x]", 4, loops[0].identity),
        ("nested=x", 7, loops[2].identity),
        ("after_nested=x", 13, loops[0].identity),
        ("after=x", 6, outer),
    ] {
        let at = u64::try_from(source.find(token).unwrap() + offset).unwrap();
        assert_eq!(
            locals
                .lookup(&selected, &body, at, "x", locals.context())
                .unwrap(),
            LocalLookup::Binding(expected)
        );
    }
    assert!(entered.facts().invocations.is_empty());
    assert!(entered.facts().bindings.iter().all(|b| b.name != "hidden"));
}

#[test]
fn for_pattern_macros_preserve_body_name_demand_and_enclosing_invocation_scope() {
    use lrr::frontend::local_scopes::LocalLookup;
    let source = "fn selected(){let mut x=17;for pattern!() in [31] {let value=x;}let after=x;}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    assert_eq!(entered.facts().invocations.len(), 1);
    assert_eq!(entered.facts().invocations[0].scope, 0);
    let event = entered
        .facts()
        .events
        .iter()
        .find(|e| matches!(e.kind, BodyMacroEventKind::Producer { .. }))
        .unwrap();
    assert_ne!(event.scope, 0);
    assert_eq!(event.range, entered.facts().invocations[0].range);
    let BodyMacroEventKind::Producer { identity, .. } = event.kind else {
        panic!("pattern demand missing")
    };
    let locals = entered
        .local_index(&selected, &body, &Default::default(), &[])
        .unwrap();
    let at = u64::try_from(source.find("value=x").unwrap() + 6).unwrap();
    assert_eq!(
        locals
            .lookup(&selected, &body, at, "x", locals.context())
            .unwrap(),
        LocalLookup::NeedInput(identity)
    );
    let outer = entered
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "x")
        .unwrap()
        .identity;
    let at = u64::try_from(source.find("after=x").unwrap() + 6).unwrap();
    assert_eq!(
        locals
            .lookup(&selected, &body, at, "x", locals.context())
            .unwrap(),
        LocalLookup::Binding(outer)
    );
}

#[test]
fn for_binding_entry_rejects_missing_iterator_or_body_syntax_boundaries() {
    for source in [
        "fn selected(){for mut x in {17;}}",
        "fn selected(){for mut x in [17]}",
    ] {
        let (index, selected, body) = prepared(source);
        let result = index.enter_body(&selected, &body);
        let Err(error) = result else {
            panic!("malformed for boundary admitted")
        };
        assert!(error.message.contains("for iterator/body boundary missing"));
    }
}

#[test]
fn match_bindings_are_visible_in_guards_and_bodies_and_isolated_between_arms() {
    use lrr::{frontend::local_scopes::LocalLookup, syntax::local_bindings::BindingForm};
    let source = "fn selected(){let mut x=17;match x {mut x if x>0=>{let first=x;},mut y=>{let second=x;let own=y;},#[cfg(any())]mut hidden=>missing!(),}let after=x;}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let arms: Vec<_> = entered
        .facts()
        .bindings
        .iter()
        .filter(|b| b.form == BindingForm::Match)
        .collect();
    assert_eq!(arms.len(), 2);
    assert_ne!(arms[0].scope, arms[1].scope);
    let outer = entered
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "x" && b.form == BindingForm::Let)
        .unwrap()
        .identity;
    let locals = entered
        .local_index(&selected, &body, &Default::default(), &[])
        .unwrap();
    for (token, offset, name, expected) in [
        ("match x", 6, "x", outer),
        ("if x>0", 3, "x", arms[0].identity),
        ("first=x", 6, "x", arms[0].identity),
        ("second=x", 7, "x", outer),
        ("own=y", 4, "y", arms[1].identity),
        ("after=x", 6, "x", outer),
    ] {
        let at = u64::try_from(source.find(token).unwrap() + offset).unwrap();
        assert_eq!(
            locals
                .lookup(&selected, &body, at, name, locals.context())
                .unwrap(),
            LocalLookup::Binding(expected)
        );
    }
    assert!(entered.facts().invocations.is_empty());
    assert!(entered.facts().bindings.iter().all(|b| b.name != "hidden"));
}

#[test]
fn match_pattern_macro_demands_names_in_guard_and_body_only() {
    use lrr::frontend::local_scopes::LocalLookup;
    let source = "fn selected(){let mut x=17;match x {pattern!() if x>0=>x,_=>x,}let after=x;}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    assert_eq!(entered.facts().invocations.len(), 1);
    let event = entered
        .facts()
        .events
        .iter()
        .find(|e| matches!(e.kind, BodyMacroEventKind::Producer { .. }))
        .unwrap();
    let BodyMacroEventKind::Producer { identity, .. } = event.kind else {
        panic!("missing pattern demand")
    };
    assert_eq!(event.range, entered.facts().invocations[0].range);
    let outer = entered
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "x")
        .unwrap()
        .identity;
    let locals = entered
        .local_index(&selected, &body, &Default::default(), &[])
        .unwrap();
    for (token, offset, expected) in [
        ("match x", 6, LocalLookup::Binding(outer)),
        ("if x>0", 3, LocalLookup::NeedInput(identity)),
        ("=>x,_", 2, LocalLookup::NeedInput(identity)),
        ("_=>x", 3, LocalLookup::Binding(outer)),
        ("after=x", 6, LocalLookup::Binding(outer)),
    ] {
        let at = u64::try_from(source.find(token).unwrap() + offset).unwrap();
        assert_eq!(
            locals
                .lookup(&selected, &body, at, "x", locals.context())
                .unwrap(),
            expected
        );
    }
}

#[test]
fn match_entry_rejects_missing_arm_expression() {
    let (index, selected, body) = prepared("fn selected(){match 17 {mut x=>,}}");
    let Err(error) = index.enter_body(&selected, &body) else {
        panic!("missing match body admitted")
    };
    assert!(
        error
            .message
            .contains("match pattern/body boundary missing")
    );
}

#[test]
fn method_receiver_bindings_use_original_header_ranges_and_body_visibility() {
    use lrr::{
        frontend::local_scopes::LocalLookup,
        syntax::local_bindings::{BindingForm, PatternNameKind},
    };
    for receiver in [
        "self",
        "mut self",
        "&self",
        "&mut self",
        "&'a self",
        "self: Box<Self>",
    ] {
        let source = format!(
            "fn selected(){{struct S;impl S{{fn method<'a>({receiver}){{let value=self;{{let nested=self;}}}}}}}}"
        );
        let (index, selected, body) = prepared(&source);
        let entered = index.enter_body(&selected, &body).unwrap();
        let key = entered
            .facts()
            .items
            .iter()
            .find(|i| {
                i.container
                    .as_ref()
                    .is_some_and(|c| c.key.kind == lrr::source::manifest::ItemKind::Impl)
            })
            .unwrap()
            .container
            .as_ref()
            .unwrap()
            .key
            .clone();
        let catalogue = entered.enter_container(&selected, &body, &key).unwrap();
        let method = catalogue
            .facts()
            .items
            .iter()
            .find(|i| i.item.def_key.declared_name == "method")
            .unwrap()
            .item
            .body
            .as_ref()
            .unwrap()
            .key();
        let method_index = catalogue.enter_body(&selected, &key, &method).unwrap();
        let binding = method_index
            .facts()
            .bindings
            .iter()
            .find(|b| b.form == BindingForm::Receiver)
            .unwrap();
        assert_eq!(binding.name, "self");
        assert_eq!(binding.classification, PatternNameKind::ExplicitBinding);
        assert_eq!(
            &source[binding.name_range.start as usize..binding.name_range.end as usize],
            "self"
        );
        assert_eq!(
            &source[binding.pattern_range.start as usize..binding.pattern_range.end as usize],
            receiver
        );
        assert_eq!(
            binding.visible_from,
            u64::from(method_index.facts().scopes[0].range.start)
        );
        let locals = method_index
            .local_index(&selected, &method, &Default::default(), &[])
            .unwrap();
        for token in ["value=self", "nested=self"] {
            let at = source.find(token).unwrap() + token.find('=').unwrap() + 1;
            assert_eq!(
                locals
                    .lookup(&selected, &method, at as u64, "self", locals.context())
                    .unwrap(),
                LocalLookup::Binding(binding.identity)
            );
        }
        assert!(
            locals
                .lookup(
                    &selected,
                    &method,
                    source.find("value=self").unwrap() as u64,
                    "r#self",
                    locals.context()
                )
                .is_err()
        );
        assert!(lrr::expand::scopes::MacroName::parse("self", 2024).is_err());
    }
}

#[test]
fn receiver_binding_does_not_cross_named_function_boundary() {
    use lrr::frontend::local_scopes::LocalLookup;
    let source = "fn selected(){struct S;impl S{fn method(&self){fn nested(){let value=self;}let outer=self;}}}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let key = entered
        .facts()
        .items
        .iter()
        .find(|i| {
            i.container
                .as_ref()
                .is_some_and(|c| c.key.kind == lrr::source::manifest::ItemKind::Impl)
        })
        .unwrap()
        .container
        .as_ref()
        .unwrap()
        .key
        .clone();
    let catalogue = entered.enter_container(&selected, &body, &key).unwrap();
    let method = catalogue.facts().items[0].item.body.as_ref().unwrap().key();
    let method_index = catalogue.enter_body(&selected, &key, &method).unwrap();
    let nested = method_index.facts().items[0]
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let nested_index = method_index
        .enter_nested_body(&selected, &method, &nested)
        .unwrap();
    assert!(
        nested_index
            .facts()
            .bindings
            .iter()
            .all(|b| b.name != "self")
    );
    let locals = nested_index
        .local_index(&selected, &nested, &Default::default(), &[])
        .unwrap();
    let at = source.find("value=self").unwrap() + 6;
    assert_eq!(
        locals
            .lookup(&selected, &nested, at as u64, "self", locals.context())
            .unwrap(),
        LocalLookup::Absent
    );
}

#[test]
fn receiver_cfg_filters_candidates_and_body_edits_preserve_identity() {
    use lrr::syntax::local_bindings::BindingForm;
    let mut identities = Vec::new();
    for (cfg, statement, expected) in [
        ("all()", "let value=self;", true),
        ("all()", "let value=self;17;", true),
        ("any()", "let value=self;", false),
    ] {
        let source = format!(
            "fn selected(){{struct S;impl S{{fn method(#[cfg({cfg})] &self){{{statement}}}}}}}"
        );
        let (index, selected, body) = prepared(&source);
        let entered = index.enter_body(&selected, &body).unwrap();
        let key = entered
            .facts()
            .items
            .iter()
            .find(|i| {
                i.container
                    .as_ref()
                    .is_some_and(|c| c.key.kind == lrr::source::manifest::ItemKind::Impl)
            })
            .unwrap()
            .container
            .as_ref()
            .unwrap()
            .key
            .clone();
        let catalogue = entered.enter_container(&selected, &body, &key).unwrap();
        let method = catalogue.facts().items[0].item.body.as_ref().unwrap().key();
        let method_index = catalogue.enter_body(&selected, &key, &method).unwrap();
        let candidates: Vec<_> = method_index
            .facts()
            .bindings
            .iter()
            .filter(|b| b.form == BindingForm::Receiver)
            .collect();
        assert_eq!(candidates.len(), usize::from(expected));
        if expected {
            identities.push(candidates[0].identity);
        }
    }
    assert_eq!(identities.len(), 2);
    assert_eq!(identities[0], identities[1]);
}

#[test]
fn supplied_statement_local_publication_replaces_demand_in_source_order() {
    use lrr::{
        expand::scopes::MacroDefinition,
        frontend::{
            local_scopes::LocalLookup,
            source_locals::{EmittedLocalDeclaration, SourceLocalPublication},
        },
    };
    let source = "fn selected(){let mut x=17;macro_rules!emit{()=>{}}emit!(x);let result=x;}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let (producer, definition) =
        entered
            .facts()
            .events
            .iter()
            .fold((None, None), |(mut p, mut d), e| {
                match &e.kind {
                    BodyMacroEventKind::Producer { identity, .. } => p = Some(*identity),
                    BodyMacroEventKind::Definition { identity, .. } => {
                        d = Some(MacroDefinition {
                            identity: *identity,
                            defining_unit: selected.outline.crate_key,
                            definition_edition: 2024,
                        })
                    }
                };
                (p, d)
            });
    let producer = producer.unwrap();
    let expansion = Key([9; 32]);
    let expansions = [(expansion, definition.unwrap())];
    let original = entered
        .local_index(&selected, &body, &Default::default(), &expansions)
        .unwrap();
    let at = (source.find("result=x").unwrap() + 7) as u64;
    assert_eq!(
        original
            .lookup(&selected, &body, at, "x", original.context())
            .unwrap(),
        LocalLookup::NeedInput(producer)
    );
    let publication = SourceLocalPublication {
        source: selected.clone(),
        body: body.clone(),
        producer,
        expansion,
        output: Digest([7; 32]),
        declarations: vec![
            EmittedLocalDeclaration::Binding {
                name: "x".into(),
                context: original.context(),
            },
            EmittedLocalDeclaration::Binding {
                name: "x".into(),
                context: original.context(),
            },
        ],
    };
    let supplied = entered
        .local_index_with_publications(
            &selected,
            &body,
            &Default::default(),
            &expansions,
            std::slice::from_ref(&publication),
        )
        .unwrap();
    let expected = publication.binding_identity(&body, 1).unwrap();
    assert_eq!(
        supplied
            .lookup(&selected, &body, at, "x", original.context())
            .unwrap(),
        LocalLookup::Binding(expected)
    );
    let before = source.find("emit!(x)").unwrap() as u64;
    let outer = entered
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "x")
        .unwrap()
        .identity;
    assert_eq!(
        supplied
            .lookup(&selected, &body, before + 6, "x", original.context())
            .unwrap(),
        LocalLookup::Binding(outer)
    );

    assert_ne!(
        supplied
            .lookup(&selected, &body, before, "x", original.context())
            .unwrap(),
        LocalLookup::Binding(expected)
    );
    let empty = SourceLocalPublication {
        declarations: vec![],
        ..publication
    };
    let cleared = entered
        .local_index_with_publications(&selected, &body, &Default::default(), &expansions, &[empty])
        .unwrap();
    let outer = entered
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "x")
        .unwrap()
        .identity;
    assert_eq!(
        cleared
            .lookup(&selected, &body, at, "x", original.context())
            .unwrap(),
        LocalLookup::Binding(outer)
    );
    assert_eq!(
        original
            .lookup(&selected, &body, at, "x", original.context())
            .unwrap(),
        LocalLookup::NeedInput(producer)
    );
}

#[test]
fn supplied_local_publication_rejects_stale_unknown_duplicate_and_invalid_batches() {
    use lrr::{
        expand::scopes::MacroDefinition,
        frontend::source_locals::{EmittedLocalDeclaration, SourceLocalPublication},
    };
    let source = "fn selected(){macro_rules!emit{()=>{}}emit!();let result=x;}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let producer = entered
        .facts()
        .events
        .iter()
        .find_map(|e| {
            if let BodyMacroEventKind::Producer { identity, .. } = e.kind {
                Some(identity)
            } else {
                None
            }
        })
        .unwrap();
    let definition = entered
        .facts()
        .events
        .iter()
        .find_map(|e| {
            if let BodyMacroEventKind::Definition { identity, .. } = e.kind {
                Some(MacroDefinition {
                    identity,
                    defining_unit: selected.outline.crate_key,
                    definition_edition: 2024,
                })
            } else {
                None
            }
        })
        .unwrap();
    let expansions = [(Key([9; 32]), definition)];
    let original = entered
        .local_index(&selected, &body, &Default::default(), &expansions)
        .unwrap();
    let good = SourceLocalPublication {
        source: selected.clone(),
        body: body.clone(),
        producer,
        expansion: expansions[0].0,
        output: Digest([7; 32]),
        declarations: vec![EmittedLocalDeclaration::Binding {
            name: "x".into(),
            context: original.context(),
        }],
    };
    for bad in [
        SourceLocalPublication {
            producer: Digest([8; 32]),
            ..good.clone()
        },
        SourceLocalPublication {
            expansion: Key([8; 32]),
            ..good.clone()
        },
        SourceLocalPublication {
            declarations: vec![EmittedLocalDeclaration::Binding {
                name: "r#self".into(),
                context: original.context(),
            }],
            ..good.clone()
        },
        SourceLocalPublication {
            declarations: vec![EmittedLocalDeclaration::Binding {
                name: "x".into(),
                context: Key([8; 32]),
            }],
            ..good.clone()
        },
    ] {
        assert!(
            entered
                .local_index_with_publications(
                    &selected,
                    &body,
                    &Default::default(),
                    &expansions,
                    &[bad]
                )
                .is_err()
        );
    }
    assert!(
        entered
            .local_index_with_publications(
                &selected,
                &body,
                &Default::default(),
                &expansions,
                &[good.clone(), good.clone()]
            )
            .is_err()
    );
    let mut stale = selected.clone();
    stale.outline.parse.content = Digest([8; 32]);
    assert!(
        entered
            .local_index_with_publications(
                &stale,
                &body,
                &Default::default(),
                &expansions,
                std::slice::from_ref(&good)
            )
            .is_err()
    );
    let mut stale_body = body.clone();
    stale_body.exact_body_bytes = Digest([8; 32]);
    assert!(
        entered
            .local_index_with_publications(
                &selected,
                &stale_body,
                &Default::default(),
                &expansions,
                &[good]
            )
            .is_err()
    );
}

#[test]
fn supplied_local_publication_preserves_nested_demand_and_checks_its_own_provenance() {
    use lrr::{
        expand::scopes::MacroDefinition,
        frontend::{
            local_scopes::LocalLookup,
            source_locals::{EmittedLocalDeclaration, SourceLocalPublication},
        },
    };
    let source = "fn selected(){macro_rules!emit{()=>{}}emit!();let result=x;}";
    let (index, selected, body) = prepared(source);
    let entered = index.enter_body(&selected, &body).unwrap();
    let producer = entered
        .facts()
        .events
        .iter()
        .find_map(|e| {
            if let BodyMacroEventKind::Producer { identity, .. } = e.kind {
                Some(identity)
            } else {
                None
            }
        })
        .unwrap();
    let definition = entered
        .facts()
        .events
        .iter()
        .find_map(|e| {
            if let BodyMacroEventKind::Definition { identity, .. } = e.kind {
                Some(MacroDefinition {
                    identity,
                    defining_unit: selected.outline.crate_key,
                    definition_edition: 2024,
                })
            } else {
                None
            }
        })
        .unwrap();
    let expansions = [(Key([9; 32]), definition.clone())];
    let original = entered
        .local_index(&selected, &body, &Default::default(), &expansions)
        .unwrap();
    let next = Digest([6; 32]);
    let publication = SourceLocalPublication {
        source: selected.clone(),
        body: body.clone(),
        producer,
        expansion: expansions[0].0,
        output: Digest([7; 32]),
        declarations: vec![
            EmittedLocalDeclaration::Macro { definition },
            EmittedLocalDeclaration::Binding {
                name: "x".into(),
                context: original.context(),
            },
            EmittedLocalDeclaration::Pending { invocation: next },
        ],
    };
    let supplied = entered
        .local_index_with_publications(
            &selected,
            &body,
            &Default::default(),
            &expansions,
            std::slice::from_ref(&publication),
        )
        .unwrap();
    let at = (source.find("result=x").unwrap() + 7) as u64;
    assert_eq!(
        supplied
            .lookup(&selected, &body, at, "x", supplied.context())
            .unwrap(),
        LocalLookup::NeedInput(next)
    );
    let changed_source = source.replace("let result=x;", "let result=x;17;");
    let (changed_index, changed_selection, changed_body) = prepared(&changed_source);
    let changed = changed_index
        .enter_body(&changed_selection, &changed_body)
        .unwrap();
    assert!(
        changed.facts().events.iter().any(
            |e| matches!(e.kind,BodyMacroEventKind::Producer{identity,..} if identity==producer)
        )
    );
    assert!(
        changed
            .local_index_with_publications(
                &changed_selection,
                &changed_body,
                &Default::default(),
                &expansions,
                std::slice::from_ref(&publication)
            )
            .is_err()
    );
    let mut invalid = publication.clone();
    invalid.declarations.push(EmittedLocalDeclaration::Binding {
        name: "x".into(),
        context: Key([8; 32]),
    });
    assert!(
        entered
            .local_index_with_publications(
                &selected,
                &body,
                &Default::default(),
                &expansions,
                &[invalid]
            )
            .is_err()
    );
    assert_eq!(
        original
            .lookup(&selected, &body, at, "x", original.context())
            .unwrap(),
        LocalLookup::NeedInput(producer)
    );
    let oversized = vec![
        SourceLocalPublication {
            declarations: vec![],
            ..publication
        };
        100_001
    ];
    assert!(
        entered
            .local_index_with_publications(
                &selected,
                &body,
                &Default::default(),
                &expansions,
                &oversized
            )
            .is_err()
    );
}

#[test]
fn supplied_statement_publication_refuses_pattern_and_attribute_frontiers() {
    use lrr::{expand::scopes::MacroDefinition, frontend::source_locals::SourceLocalPublication};
    for source in [
        "fn selected(){for pattern!() in [17] {let result=x;}}",
        "fn selected(){#[transform] fn pending(){}let result=x;}",
    ] {
        let (index, selected, body) = prepared(source);
        let entered = index.enter_body(&selected, &body).unwrap();
        let producer = entered
            .facts()
            .events
            .iter()
            .find_map(|e| {
                if let BodyMacroEventKind::Producer { identity, .. } = e.kind {
                    Some(identity)
                } else {
                    None
                }
            })
            .unwrap();
        let expansion = Key([9; 32]);
        let metadata = [(
            expansion,
            MacroDefinition {
                identity: Digest([7; 32]),
                defining_unit: selected.outline.crate_key,
                definition_edition: 2024,
            },
        )];
        let publication = SourceLocalPublication {
            source: selected.clone(),
            body: body.clone(),
            producer,
            expansion,
            output: Digest([6; 32]),
            declarations: vec![],
        };
        let Err(error) = entered.local_index_with_publications(
            &selected,
            &body,
            &Default::default(),
            &metadata,
            &[publication],
        ) else {
            panic!("nonstatement publication cleared demand")
        };
        assert!(
            error
                .message
                .contains("requires generated declaration discovery")
        );
    }
}
