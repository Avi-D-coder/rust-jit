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
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
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
fn unit(path: &str, module: &[&str], text: &str) -> SourceUnitInput {
    SourceUnitInput {
        path: SourcePath::new(path).unwrap(),
        crate_key: Digest([1; 32]),
        module_path: module.iter().map(|s| (*s).into()).collect(),
        profile: ProfileIdentity::current(),
        cfg: CfgSet::default(),
        blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
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
    let k = root.key();
    (
        k,
        HygieneFacts::new(
            vec![root],
            vec![ResolutionContext {
                original: k,
                item: k,
                local: k,
            }],
        )
        .unwrap()
        .with_name_contexts(vec![NameResolutionContext {
            original: k,
            item: NameContextKey(Key([2; 32])),
            local: NameContextKey(Key([2; 32])),
        }])
        .unwrap(),
    )
}
fn view(p: &PreparedSourceInputs, s: &SourceSelection) -> PreparedSourceView {
    let SourceSelectionOutcome::Selected(view) = p.select(s).unwrap() else {
        panic!("source absent")
    };
    view
}
fn defined(value: HygienicLookup) -> Digest {
    let HygienicLookup::Selection(MacroSelection::Defined(d)) = value else {
        panic!("not defined")
    };
    d.identity
}
#[test]
fn linked_external_modules_capture_parent_order_and_enter_recursive_file_bodies() {
    let root = unit(
        "main.rs",
        &[],
        "macro_rules! choose {()=>{17};} mod child; macro_rules! choose {()=>{29};}",
    );
    let child = unit(
        "child.rs",
        &["child"],
        "pub fn early(){choose!();} macro_rules! choose {()=>{41};} mod grand; macro_rules! choose {()=>{53};} pub fn late(){choose!();}",
    );
    let grand = unit(
        "child/grand.rs",
        &["child", "grand"],
        "pub fn early(){choose!();} macro_rules! choose {()=>{61};} pub fn late(){choose!();}",
    );
    let selections = [selection(&root), selection(&child), selection(&grand)];
    let child_text = child.blob.text().unwrap().to_owned();
    let grand_text = grand.blob.text().unwrap().to_owned();
    let prepared =
        PreparedSourceInputs::refresh(None, vec![root, child, grand], Default::default()).unwrap();
    let (context, owned) = facts();
    let root_index =
        PublishedMacroIndex::from_view(view(&prepared, &selections[0]), context, owned).unwrap();
    let child_view = view(&prepared, &selections[1]);
    let child_bodies: Vec<_> = child_view
        .outline()
        .items
        .iter()
        .filter_map(|i| {
            i.body
                .as_ref()
                .map(|b| (i.def_key.declared_name.clone(), b.key()))
        })
        .collect();
    let (context, owned) = facts();
    let child_index = root_index
        .link_module(
            &selections[0],
            root_index.module_input(&["child".into()]).unwrap(),
            child_view,
            context,
            owned,
        )
        .unwrap();
    let child_chain = vec![selections[1].clone(), selections[0].clone()];
    let earlier = child_index
        .enter_body(&child_chain, &child_bodies[0].1)
        .unwrap();
    let early = defined(
        earlier
            .lookup(
                &child_chain,
                &child_bodies[0].1,
                u64::try_from(child_text.find("choose!()").unwrap()).unwrap(),
                "choose",
            )
            .unwrap(),
    );
    let later = child_index
        .enter_body(&child_chain, &child_bodies[1].1)
        .unwrap();
    let late = defined(
        later
            .lookup(
                &child_chain,
                &child_bodies[1].1,
                u64::try_from(child_text.rfind("choose!()").unwrap()).unwrap(),
                "choose",
            )
            .unwrap(),
    );
    assert_ne!(early, late);
    let grand_view = view(&prepared, &selections[2]);
    let grand_bodies: Vec<_> = grand_view
        .outline()
        .items
        .iter()
        .filter_map(|i| i.body.as_ref().map(|b| b.key()))
        .collect();
    let (context, owned) = facts();
    let grand_index = child_index
        .link_module(
            &child_chain,
            child_index
                .module_input(&["child".into(), "grand".into()])
                .unwrap(),
            grand_view,
            context,
            owned,
        )
        .unwrap();
    let grand_chain = vec![
        selections[2].clone(),
        selections[1].clone(),
        selections[0].clone(),
    ];
    let first = grand_index
        .enter_body(&grand_chain, &grand_bodies[0])
        .unwrap();
    let inherited = defined(
        first
            .lookup(
                &grand_chain,
                &grand_bodies[0],
                u64::try_from(grand_text.find("choose!()").unwrap()).unwrap(),
                "choose",
            )
            .unwrap(),
    );
    assert_ne!(inherited, early);
    assert_ne!(inherited, late);
    let second = grand_index
        .enter_body(&grand_chain, &grand_bodies[1])
        .unwrap();
    let own = defined(
        second
            .lookup(
                &grand_chain,
                &grand_bodies[1],
                u64::try_from(grand_text.rfind("choose!()").unwrap()).unwrap(),
                "choose",
            )
            .unwrap(),
    );
    assert_ne!(own, inherited);
    assert_eq!(grand_index.sources().count(), 3);
    assert!(
        grand_index
            .lookup(
                &child_chain,
                &["child".into(), "grand".into()],
                u64::MAX,
                "choose"
            )
            .is_err()
    );
}
#[test]
fn module_link_rejects_foreign_publication_context_and_stale_parent() {
    let root = unit("main.rs", &[], "macro_rules! choose {()=>{17};} mod child;");
    let child = unit("child.rs", &["child"], "pub fn selected(){choose!();}");
    let r = selection(&root);
    let c = selection(&child);
    let prepared =
        PreparedSourceInputs::refresh(None, vec![root.clone(), child.clone()], Default::default())
            .unwrap();
    let (ctx, f) = facts();
    let index = PublishedMacroIndex::from_view(view(&prepared, &r), ctx, f).unwrap();
    let input = index.module_input(&["child".into()]).unwrap();
    let foreign =
        PreparedSourceInputs::refresh(None, vec![root, child], Default::default()).unwrap();
    let (ctx, f) = facts();
    assert!(
        index
            .link_module(&r, input, view(&foreign, &c), ctx, f)
            .is_err()
    );
    let (ctx, f) = facts();
    assert!(
        index
            .link_module(&r, input, view(&prepared, &r), ctx, f)
            .is_err()
    );
    let mut stale = r.clone();
    stale.outline.parse.content = Digest([99; 32]);
    let (ctx, f) = facts();
    assert!(
        index
            .link_module(&stale, input, view(&prepared, &c), ctx, f)
            .is_err()
    );
    assert!(matches!(
        index
            .lookup(&r, &["child".into()], u64::MAX, "choose")
            .unwrap(),
        HygienicLookup::NeedInput(_)
    ));
}

#[test]
fn linked_invocations_validate_full_chain_and_capture_parent_macro_order() {
    use lrr::{
        frontend::macro_names::{MacroNamespaces, PathLookup},
        syntax::spans::SourceRange,
    };
    let root = unit(
        "main.rs",
        &[],
        "macro_rules! choose {()=>{17};} mod child; macro_rules! choose {()=>{29};}",
    );
    let text = "fn selected(){let a=choose!(); {macro_rules! choose {()=>{41};} let b=choose!();}} choose!();";
    let child = unit("child.rs", &["child"], text);
    let selections = [selection(&root), selection(&child)];
    let prepared =
        PreparedSourceInputs::refresh(None, vec![root, child], Default::default()).unwrap();
    let (ctx, f) = facts();
    let parent = PublishedMacroIndex::from_view(view(&prepared, &selections[0]), ctx, f).unwrap();
    let child_view = view(&prepared, &selections[1]);
    let body = child_view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "selected")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (ctx, f) = facts();
    let child = parent
        .link_module(
            &selections[0],
            parent.module_input(&["child".into()]).unwrap(),
            child_view,
            ctx,
            f,
        )
        .unwrap();
    let chain = vec![selections[1].clone(), selections[0].clone()];
    let graph = MacroNamespaces::default();
    let calls: Vec<_> = text
        .match_indices("choose!()")
        .map(|(at, _)| SourceRange {
            start: u32::try_from(at).unwrap(),
            end: u32::try_from(at + 9).unwrap(),
        })
        .collect();
    let entered = child.enter_body(&chain, &body).unwrap();
    let first = entered
        .lookup_invocation(&chain, &body, calls[0], &graph)
        .unwrap();
    let second = entered
        .lookup_invocation(&chain, &body, calls[1], &graph)
        .unwrap();
    assert!(matches!(first, PathLookup::Defined(_)));
    assert_ne!(first, second);
    let header = SourceRange {
        end: calls[2].end + 1,
        ..calls[2]
    };
    assert_eq!(
        child.lookup_invocation(&chain, header, &graph).unwrap(),
        first
    );
    assert!(
        entered
            .lookup_invocation(&chain[..1], &body, calls[0], &graph)
            .is_err()
    );
    let mut wrong = chain.clone();
    wrong[1].outline.parse.content = Digest([44; 32]);
    assert!(
        entered
            .lookup_invocation(&wrong, &body, calls[0], &graph)
            .is_err()
    );
    assert!(child.lookup_invocation(&wrong, header, &graph).is_err());
    assert!(
        entered
            .lookup_invocation(&chain, &body, header, &graph)
            .is_err()
    );
}

#[test]
fn linked_nested_entries_validate_each_ancestor_and_capture_import_contexts() {
    use lrr::frontend::macro_names::{MacroNamespaces, PathLookup};
    let root = unit(
        "main.rs",
        &[],
        "macro_rules! choose {()=>{17};} mod child; macro_rules! choose {()=>{29};}",
    );
    let child = unit(
        "child.rs",
        &["child"],
        "macro_rules! choose {()=>{41};} mod grand; macro_rules! choose {()=>{53};}",
    );
    let grand = unit(
        "child/grand.rs",
        &["child", "grand"],
        "fn selected(){use choose as frozen;fn inner(){let a=choose!();let b=frozen!();fn leaf(){frozen!();}}macro_rules! choose {()=>{61};}}",
    );
    let selections = [selection(&root), selection(&child), selection(&grand)];
    let prepared =
        PreparedSourceInputs::refresh(None, vec![root, child, grand], Default::default()).unwrap();
    let (ctx, f) = facts();
    let main = PublishedMacroIndex::from_view(view(&prepared, &selections[0]), ctx, f).unwrap();
    let (ctx, f) = facts();
    let child = main
        .link_module(
            &selections[0],
            main.module_input(&["child".into()]).unwrap(),
            view(&prepared, &selections[1]),
            ctx,
            f,
        )
        .unwrap();
    let grand_view = view(&prepared, &selections[2]);
    let body = grand_view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "selected")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (ctx, f) = facts();
    let grand = child
        .link_module(
            &[selections[1].clone(), selections[0].clone()],
            child
                .module_input(&["child".into(), "grand".into()])
                .unwrap(),
            grand_view,
            ctx,
            f,
        )
        .unwrap();
    let chain = vec![
        selections[2].clone(),
        selections[1].clone(),
        selections[0].clone(),
    ];
    let outer = grand.enter_body(&chain, &body).unwrap();
    let inner = outer
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
    let entered = outer.enter_nested_body(&chain, &body, &inner).unwrap();
    let graph = MacroNamespaces::default();
    let mut definitions = Vec::new();
    for call in &entered.facts().invocations {
        let PathLookup::Defined(def) = entered
            .lookup_invocation(&chain, &inner, call.range, &graph)
            .unwrap()
        else {
            panic!("linked nested macro missing")
        };
        definitions.push(def.identity);
    }
    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions[0], definitions[1]);
    let expected = defined(
        child
            .lookup(
                &[selections[1].clone(), selections[0].clone()],
                &["child".into()],
                u64::try_from("macro_rules! choose {()=>{41};}".len()).unwrap(),
                "choose",
            )
            .unwrap(),
    );
    assert_eq!(definitions[0], expected);
    let leaf = entered
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
    let leaf_entered = entered.enter_nested_body(&chain, &inner, &leaf).unwrap();
    let call = leaf_entered.facts().invocations[0].range;
    let PathLookup::Defined(def) = leaf_entered
        .lookup_invocation(&chain, &leaf, call, &graph)
        .unwrap()
    else {
        panic!("linked grandchild macro missing")
    };
    assert_eq!(def.identity, expected);
    for ancestor in 0..chain.len() {
        let mut stale = chain.clone();
        stale[ancestor].outline.parse.content = Digest([9; 32]);
        assert!(outer.enter_nested_body(&stale, &body, &inner).is_err());
        assert!(
            leaf_entered
                .lookup_invocation(&stale, &leaf, call, &graph)
                .is_err()
        );
    }
    assert!(
        entered
            .enter_nested_body(&chain[..2], &inner, &leaf)
            .is_err()
    );
    assert!(
        outer
            .lookup_invocation(&chain, &body, call, &graph)
            .is_err()
    );
}

#[test]
fn linked_container_entries_validate_all_sources_and_capture_module_import_contexts() {
    use lrr::frontend::macro_names::{MacroNamespaces, PathLookup};
    let root = unit(
        "main.rs",
        &[],
        "macro_rules! choose {()=>{17};} mod child; macro_rules! choose {()=>{29};}",
    );
    let child = unit(
        "child.rs",
        &["child"],
        "macro_rules! choose {()=>{41};} mod grand; macro_rules! choose {()=>{53};}",
    );
    let grand = unit(
        "child/grand.rs",
        &["child", "grand"],
        "fn selected(){mod local {use choose as frozen;fn inner(){let a=choose!();let b=frozen!();} mod nested {use choose as leaf_alias;fn leaf(){let x=leaf_alias!();}}}macro_rules! choose {()=>{61};}}",
    );
    let selections = [selection(&root), selection(&child), selection(&grand)];
    let prepared =
        PreparedSourceInputs::refresh(None, vec![root, child, grand], Default::default()).unwrap();
    let (ctx, f) = facts();
    let main = PublishedMacroIndex::from_view(view(&prepared, &selections[0]), ctx, f).unwrap();
    let (ctx, f) = facts();
    let child = main
        .link_module(
            &selections[0],
            main.module_input(&["child".into()]).unwrap(),
            view(&prepared, &selections[1]),
            ctx,
            f,
        )
        .unwrap();
    let grand_view = view(&prepared, &selections[2]);
    let body = grand_view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "selected")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (ctx, f) = facts();
    let grand = child
        .link_module(
            &[selections[1].clone(), selections[0].clone()],
            child
                .module_input(&["child".into(), "grand".into()])
                .unwrap(),
            grand_view,
            ctx,
            f,
        )
        .unwrap();
    let chain = vec![
        selections[2].clone(),
        selections[1].clone(),
        selections[0].clone(),
    ];

    let outer = grand.enter_body(&chain, &body).unwrap();
    let key = &outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "local")
        .unwrap()
        .container
        .as_ref()
        .unwrap()
        .key;
    let local = outer.enter_container(&chain, &body, key).unwrap();
    assert_eq!(local.key(), key);
    let inner = local
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
    let entered = local.enter_body(&chain, key, &inner).unwrap();
    let expected = defined(
        child
            .lookup(
                &[selections[1].clone(), selections[0].clone()],
                &["child".into()],
                u64::try_from("macro_rules! choose {()=>{41};}".len()).unwrap(),
                "choose",
            )
            .unwrap(),
    );
    let graph = MacroNamespaces::default();
    assert_eq!(entered.facts().invocations.len(), 2);
    for call in &entered.facts().invocations {
        let PathLookup::Defined(definition) = entered
            .lookup_invocation(&chain, &inner, call.range, &graph)
            .unwrap()
        else {
            panic!("linked container selection failed")
        };
        assert_eq!(definition.identity, expected);
    }
    let nested_key = &local
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "nested")
        .unwrap()
        .container
        .as_ref()
        .unwrap()
        .key;
    let nested = local.enter_container(&chain, key, nested_key).unwrap();
    let leaf = nested
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
    let leaf_entered = nested.enter_body(&chain, nested_key, &leaf).unwrap();
    assert_eq!(leaf_entered.facts().invocations.len(), 1);
    let call = leaf_entered.facts().invocations[0].range;
    let PathLookup::Defined(definition) = leaf_entered
        .lookup_invocation(&chain, &leaf, call, &graph)
        .unwrap()
    else {
        panic!("linked recursive container selection failed")
    };
    assert_eq!(definition.identity, expected);
    for ancestor in 0..chain.len() {
        let mut stale = chain.clone();
        stale[ancestor].outline.parse.content = Digest([9; 32]);
        assert!(outer.enter_container(&stale, &body, key).is_err());
        assert!(local.enter_body(&stale, key, &inner).is_err());
        assert!(local.enter_container(&stale, key, nested_key).is_err());
        assert!(nested.enter_body(&stale, nested_key, &leaf).is_err());
        assert!(
            leaf_entered
                .lookup_invocation(&stale, &leaf, call, &graph)
                .is_err()
        );
    }
    assert!(outer.enter_container(&chain[..2], &body, key).is_err());
    assert!(local.enter_body(&chain[..2], key, &inner).is_err());
    assert!(local.enter_container(&chain[..2], key, nested_key).is_err());
    assert!(nested.enter_body(&chain[..2], nested_key, &leaf).is_err());
    let mut changed = key.clone();
    changed.content = Digest([9; 32]);
    assert!(local.enter_body(&chain, &changed, &inner).is_err());
    assert!(local.enter_container(&chain, &changed, nested_key).is_err());
    assert!(
        outer
            .lookup_invocation(&chain, &body, call, &graph)
            .is_err()
    );
}

#[test]
fn linked_local_lookup_validates_all_ancestors_and_retains_nested_container_body_guards() {
    let root = unit(
        "main.rs",
        &[],
        "macro_rules! choose {()=>{17};} mod child; macro_rules! choose {()=>{29};}",
    );
    let child = unit(
        "child.rs",
        &["child"],
        "macro_rules! choose {()=>{41};} mod grand; macro_rules! choose {()=>{53};}",
    );
    let grand = unit(
        "child/grand.rs",
        &["child", "grand"],
        "fn selected(mut p:u64){let mut x=17;{let mut x=31;let n=x;}let result=x;fn nested(){let mut local=53;let result=local;}mod local{fn member(){let mut z=61;let result=z;}}}",
    );
    let selections = [selection(&root), selection(&child), selection(&grand)];
    let prepared =
        PreparedSourceInputs::refresh(None, vec![root, child, grand], Default::default()).unwrap();
    let (ctx, f) = facts();
    let main = PublishedMacroIndex::from_view(view(&prepared, &selections[0]), ctx, f).unwrap();
    let (ctx, f) = facts();
    let child = main
        .link_module(
            &selections[0],
            main.module_input(&["child".into()]).unwrap(),
            view(&prepared, &selections[1]),
            ctx,
            f,
        )
        .unwrap();
    let grand_view = view(&prepared, &selections[2]);
    let body = grand_view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "selected")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (ctx, f) = facts();
    let grand = child
        .link_module(
            &[selections[1].clone(), selections[0].clone()],
            child
                .module_input(&["child".into(), "grand".into()])
                .unwrap(),
            grand_view,
            ctx,
            f,
        )
        .unwrap();
    let chain = vec![
        selections[2].clone(),
        selections[1].clone(),
        selections[0].clone(),
    ];

    use lrr::frontend::local_scopes::LocalLookup;
    let source = "fn selected(mut p:u64){let mut x=17;{let mut x=31;let n=x;}let result=x;fn nested(){let mut local=53;let result=local;}mod local{fn member(){let mut z=61;let result=z;}}}";
    let outer = grand.enter_body(&chain, &body).unwrap();
    let locals = outer
        .local_index(&chain, &body, &Default::default(), &[])
        .unwrap();
    let xs: Vec<_> = outer
        .facts()
        .bindings
        .iter()
        .filter(|b| b.name == "x")
        .collect();
    for (token, id) in [("x;}", xs[1].identity), ("x;fn", xs[0].identity)] {
        let at = u64::try_from(source.find(token).unwrap()).unwrap();
        assert_eq!(
            locals
                .lookup(&chain, &body, at, "x", locals.context())
                .unwrap(),
            LocalLookup::Binding(id)
        );
    }
    let at = u64::try_from(source.find("x;fn").unwrap()).unwrap();
    let p = outer
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "p")
        .unwrap()
        .identity;
    assert_eq!(
        locals
            .lookup(&chain, &body, at, "p", locals.context())
            .unwrap(),
        LocalLookup::Binding(p)
    );
    let nested_key = outer
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
    let nested = outer.enter_nested_body(&chain, &body, &nested_key).unwrap();
    let nested_locals = nested
        .local_index(&chain, &nested_key, &Default::default(), &[])
        .unwrap();
    let nested_at = u64::try_from(source.find("local;}").unwrap()).unwrap();
    assert_eq!(
        nested_locals
            .lookup(&chain, &nested_key, nested_at, "x", nested_locals.context())
            .unwrap(),
        LocalLookup::Absent
    );
    let key = &outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "local")
        .unwrap()
        .container
        .as_ref()
        .unwrap()
        .key;
    let catalogue = outer.enter_container(&chain, &body, key).unwrap();
    let member_key = catalogue.facts().items[0].item.body.as_ref().unwrap().key();
    let member = catalogue.enter_body(&chain, key, &member_key).unwrap();
    let member_locals = member
        .local_index(&chain, &member_key, &Default::default(), &[])
        .unwrap();
    let member_at = u64::try_from(source.find("z;}").unwrap()).unwrap();
    let z = member
        .facts()
        .bindings
        .iter()
        .find(|b| b.name == "z")
        .unwrap()
        .identity;
    assert_eq!(
        member_locals
            .lookup(&chain, &member_key, member_at, "z", member_locals.context())
            .unwrap(),
        LocalLookup::Binding(z)
    );
    for ancestor in 0..chain.len() {
        let mut stale = chain.clone();
        stale[ancestor].outline.parse.content = Digest([9; 32]);
        assert!(
            outer
                .local_index(&stale, &body, &Default::default(), &[])
                .is_err()
        );
        assert!(
            locals
                .lookup(&stale, &body, at, "x", locals.context())
                .is_err()
        );
        assert!(
            nested_locals
                .lookup(
                    &stale,
                    &nested_key,
                    nested_at,
                    "local",
                    nested_locals.context()
                )
                .is_err()
        );
        assert!(
            member_locals
                .lookup(&stale, &member_key, member_at, "z", member_locals.context())
                .is_err()
        );
    }
    assert!(
        outer
            .local_index(&chain[..2], &body, &Default::default(), &[])
            .is_err()
    );
    assert!(
        locals
            .lookup(&chain[..2], &body, at, "x", locals.context())
            .is_err()
    );
    assert!(
        member_locals
            .lookup(
                &chain[..2],
                &member_key,
                member_at,
                "z",
                member_locals.context()
            )
            .is_err()
    );
    let mut changed = body.clone();
    changed.exact_body_bytes = Digest([9; 32]);
    assert!(
        locals
            .lookup(&chain, &changed, at, "x", locals.context())
            .is_err()
    );
    assert!(
        locals
            .lookup(&chain, &body, member_at, "z", locals.context())
            .is_err()
    );
}

#[test]
fn linked_local_publication_requires_current_full_source_chain() {
    let root = unit(
        "main.rs",
        &[],
        "macro_rules! choose {()=>{17};} mod child; macro_rules! choose {()=>{29};}",
    );
    let child = unit(
        "child.rs",
        &["child"],
        "macro_rules! choose {()=>{41};} mod grand; macro_rules! choose {()=>{53};}",
    );
    let grand = unit(
        "child/grand.rs",
        &["child", "grand"],
        "fn selected(){macro_rules!emit{()=>{}}emit!();let result=x;}",
    );
    let selections = [selection(&root), selection(&child), selection(&grand)];
    let prepared =
        PreparedSourceInputs::refresh(None, vec![root, child, grand], Default::default()).unwrap();
    let (ctx, f) = facts();
    let main = PublishedMacroIndex::from_view(view(&prepared, &selections[0]), ctx, f).unwrap();
    let (ctx, f) = facts();
    let child = main
        .link_module(
            &selections[0],
            main.module_input(&["child".into()]).unwrap(),
            view(&prepared, &selections[1]),
            ctx,
            f,
        )
        .unwrap();
    let grand_view = view(&prepared, &selections[2]);
    let body = grand_view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "selected")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (ctx, f) = facts();
    let grand = child
        .link_module(
            &[selections[1].clone(), selections[0].clone()],
            child
                .module_input(&["child".into(), "grand".into()])
                .unwrap(),
            grand_view,
            ctx,
            f,
        )
        .unwrap();
    let chain = vec![
        selections[2].clone(),
        selections[1].clone(),
        selections[0].clone(),
    ];

    use lrr::frontend::local_scopes::LocalLookup;
    let source = "fn selected(){macro_rules!emit{()=>{}}emit!();let result=x;}";
    let outer = grand.enter_body(&chain, &body).unwrap();
    let locals = outer
        .local_index(&chain, &body, &Default::default(), &[])
        .unwrap();
    use lrr::{
        expand::scopes::MacroDefinition,
        frontend::source_locals::{EmittedLocalDeclaration, SourceLocalPublication},
        syntax::entered_macros::BodyMacroEventKind,
    };
    let producer = outer
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
    let definition = outer
        .facts()
        .events
        .iter()
        .find_map(|e| {
            if let BodyMacroEventKind::Definition { identity, .. } = e.kind {
                Some(MacroDefinition {
                    identity,
                    defining_unit: chain[0].outline.crate_key,
                    definition_edition: 2024,
                })
            } else {
                None
            }
        })
        .unwrap();
    let expansions = [(Key([9; 32]), definition)];
    let publication = SourceLocalPublication {
        source: chain[0].clone(),
        body: body.clone(),
        producer,
        expansion: expansions[0].0,
        output: Digest([7; 32]),
        declarations: vec![EmittedLocalDeclaration::Binding {
            name: "x".into(),
            context: locals.context(),
        }],
    };
    let supplied = outer
        .local_index_with_publications(
            &chain,
            &body,
            &Default::default(),
            &expansions,
            std::slice::from_ref(&publication),
        )
        .unwrap();
    let at = (source.find("result=x").unwrap() + 7) as u64;
    assert_eq!(
        supplied
            .lookup(&chain, &body, at, "x", supplied.context())
            .unwrap(),
        LocalLookup::Binding(publication.binding_identity(&body, 0).unwrap())
    );
    for ordinal in 0..chain.len() {
        let mut stale = chain.clone();
        stale[ordinal].outline.parse.content = Digest([8; 32]);
        assert!(
            outer
                .local_index_with_publications(
                    &stale,
                    &body,
                    &Default::default(),
                    &expansions,
                    std::slice::from_ref(&publication)
                )
                .is_err()
        );
        assert!(
            supplied
                .lookup(&stale, &body, at, "x", supplied.context())
                .is_err()
        );
    }
    assert!(
        outer
            .local_index_with_publications(
                &chain[..2],
                &body,
                &Default::default(),
                &expansions,
                &[publication]
            )
            .is_err()
    );
}
