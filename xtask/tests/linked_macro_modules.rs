use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database},
    frontend::{
        hygiene::HygieneFacts,
        macro_names::{MacroNamespaces, PathLookup},
        source_macros::PublishedMacroIndex,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{parse::ParseKey, spans::SourceRange, token_wire as runtime},
};
use lrr_expand_adapter::{declarative::DeclarativeMacro, hygiene::HygieneStore, token_wire::*};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[allow(dead_code)]
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
fn facts() -> (runtime::Key, HygieneFacts) {
    let mut store = HygieneStore::default();
    let root = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let rows = store.resolution_contexts(&[root]).unwrap();
    let names = store.name_contexts(&[root]).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    (
        runtime::Key(root.0),
        HygieneFacts::new(
            runtime::TokenForest::decode(&wire)
                .unwrap()
                .contexts()
                .to_vec(),
            serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
        )
        .unwrap()
        .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
        .unwrap(),
    )
}
fn selected(value: PathLookup) -> Digest {
    let PathLookup::Defined(definition) = value else {
        panic!("macro selection failed")
    };
    definition.identity
}
fn expand(
    identity: Digest,
    definitions: &BTreeMap<Digest, (usize, SourceRange)>,
    forests: &[TokenForest],
) -> u64 {
    let (file, range) = definitions[&identity];
    let forest = &forests[file];
    let spans: BTreeMap<_, _> = forest
        .spans()
        .iter()
        .map(|span| (span.key(), span))
        .collect();
    let mut stack: Vec<_> = forest.roots().iter().rev().collect();
    let mut body = None;
    while let Some(tree) = stack.pop() {
        if let TokenTree::Group {
            delimiter,
            open,
            close,
            children,
            ..
        } = tree
        {
            if *delimiter == Delimiter::Brace
                && spans[open].start >= range.start
                && spans[close].end <= range.end
            {
                body = Some(tree.clone());
                break;
            }
            stack.extend(children.iter().rev());
        }
    }
    let definition = DeclarativeMacro::new(
        TokenForest::new(
            vec![body.unwrap()],
            forest.contexts().to_vec(),
            forest.spans().to_vec(),
        )
        .unwrap(),
    )
    .unwrap();
    let input = fixture::input("");
    let result = definition
        .expand(&input, fixture::call_site(&input))
        .unwrap()
        .unwrap();
    let literals = fixture::literal_texts(&result.tokens);
    assert_eq!(literals.len(), 1);
    literals[0].strip_suffix("_u64").unwrap().parse().unwrap()
}
#[test]
fn recursive_external_module_macro_capture_matches_native_macos() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-linked-module-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    fs::create_dir(work.join("child")).unwrap();
    let sources = [
        (
            "main.rs",
            vec![],
            "macro_rules! choose {()=>{17_u64};} mod child; macro_rules! choose {()=>{29_u64};} fn main(){let root=choose!();println!(\"{:?}\",[child::early(),child::late(),child::grand::early(),child::grand::late(),root]);}",
        ),
        (
            "child.rs",
            vec!["child"],
            "pub fn early()->u64{choose!()} macro_rules! choose {()=>{41_u64};} pub mod grand; macro_rules! choose {()=>{53_u64};} pub fn late()->u64{choose!()}",
        ),
        (
            "child/grand.rs",
            vec!["child", "grand"],
            "pub fn early()->u64{choose!()} macro_rules! choose {()=>{61_u64};} pub fn late()->u64{choose!()}",
        ),
    ];
    for (path, _, text) in &sources {
        fs::write(work.join(path), text).unwrap();
    }
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let binary = work.join("native");
    let compiler = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("main.rs"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    fs::write(work.join("compiler.stderr"), &compiler.stderr).unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let native = Command::new(binary).output().unwrap();
    assert!(native.status.success());
    fs::write(work.join("native.stdout"), &native.stdout).unwrap();
    let native_values: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let units: Vec<_> = sources
        .iter()
        .map(|(path, module, text)| SourceUnitInput {
            path: SourcePath::new(path).unwrap(),
            crate_key: Digest([1; 32]),
            module_path: module.iter().map(|s| (*s).to_owned()).collect(),
            profile: ProfileIdentity::current(),
            cfg: CfgSet::default(),
            blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        })
        .collect();
    let selections: Vec<_> = units
        .iter()
        .map(|unit| SourceSelection {
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
        })
        .collect();
    let prepared = PreparedSourceInputs::refresh(None, units, Default::default()).unwrap();
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let mut views: Vec<_> = selections
        .iter()
        .map(|s| {
            let SourceSelectionOutcome::Selected(view) = prepared.select(s).unwrap() else {
                panic!("source absent")
            };
            Some(view)
        })
        .collect();
    let mut definitions = BTreeMap::new();
    let mut bodies = Vec::new();
    let mut forests = Vec::new();
    for (file, view) in views.iter().enumerate() {
        let view = view.as_ref().unwrap();
        let mut file_bodies = BTreeMap::new();
        for item in &view.outline().items {
            if item.def_key.namespace == lrr::source::manifest::Namespace::Macro {
                definitions.insert(item.def_key.fingerprint(), (file, item.declaration_span));
            }
            if let Some(body) = &item.body {
                file_bodies.insert(item.def_key.declared_name.clone(), body.key());
            }
        }
        bodies.push(file_bodies);
        forests.push(
            parse_source(
                sources[file].2,
                SourceIdentity {
                    unit: Key([1; 32]),
                    content: Key(selections[file].outline.parse.content.0),
                    path: sources[file].0.into(),
                },
                ContextRecord {
                    parent: None,
                    expansion: None,
                    transparency: Transparency::Opaque,
                    defining_unit: Key([1; 32]),
                    edition: Edition::E2024,
                },
                Key([2; 32]),
            )
            .unwrap(),
        );
    }
    let (ctx, f) = facts();
    let main = PublishedMacroIndex::from_view(views[0].take().unwrap(), ctx, f).unwrap();
    let (ctx, f) = facts();
    let child = main
        .link_module(
            &selections[0],
            main.module_input(&["child".into()]).unwrap(),
            views[1].take().unwrap(),
            ctx,
            f,
        )
        .unwrap();
    let child_chain = vec![selections[1].clone(), selections[0].clone()];
    let (ctx, f) = facts();
    let grand = child
        .link_module(
            &child_chain,
            child
                .module_input(&["child".into(), "grand".into()])
                .unwrap(),
            views[2].take().unwrap(),
            ctx,
            f,
        )
        .unwrap();
    let grand_chain = vec![
        selections[2].clone(),
        selections[1].clone(),
        selections[0].clone(),
    ];
    let mut observed = Vec::new();
    for name in ["early", "late"] {
        let body = &bodies[1][name];
        let entered = child.enter_body(&child_chain, body).unwrap();
        let at = if name == "early" {
            sources[1].2.find("choose!()")
        } else {
            sources[1].2.rfind("choose!()")
        }
        .unwrap();
        observed.push(expand(
            selected(
                entered
                    .lookup_invocation(
                        &child_chain,
                        body,
                        SourceRange {
                            start: u32::try_from(at).unwrap(),
                            end: u32::try_from(at + 9).unwrap(),
                        },
                        &MacroNamespaces::default(),
                    )
                    .unwrap(),
            ),
            &definitions,
            &forests,
        ));
    }
    for name in ["early", "late"] {
        let body = &bodies[2][name];
        let entered = grand.enter_body(&grand_chain, body).unwrap();
        let at = if name == "early" {
            sources[2].2.find("choose!()")
        } else {
            sources[2].2.rfind("choose!()")
        }
        .unwrap();
        observed.push(expand(
            selected(
                entered
                    .lookup_invocation(
                        &grand_chain,
                        body,
                        SourceRange {
                            start: u32::try_from(at).unwrap(),
                            end: u32::try_from(at + 9).unwrap(),
                        },
                        &MacroNamespaces::default(),
                    )
                    .unwrap(),
            ),
            &definitions,
            &forests,
        ));
    }
    let entered = main.enter_body(&selections[0], &bodies[0]["main"]).unwrap();
    observed.push(expand(
        selected(
            entered
                .lookup_invocation(
                    &selections[0],
                    &bodies[0]["main"],
                    {
                        let at = sources[0].2.find("choose!()").unwrap();
                        SourceRange {
                            start: u32::try_from(at).unwrap(),
                            end: u32::try_from(at + 9).unwrap(),
                        }
                    },
                    &MacroNamespaces::default(),
                )
                .unwrap(),
        ),
        &definitions,
        &forests,
    ));
    assert_eq!(observed, native_values);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","passed":true,"native":native_values,"observed":observed,"scope":"same immutable publication, recursive external-module capture, exact entered invocation source-chain validation and pinned expansion selected from full original token forests with source content/offsets preserved; source file selections supplied, guest VM pending"})).unwrap()).unwrap();
}

#[test]
fn linked_nested_macro_capture_matches_native_macos() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-linked-nested-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    fs::create_dir(work.join("child")).unwrap();
    let sources = [
        (
            "main.rs",
            vec![],
            "macro_rules! choose {()=>{17_u64};} mod child; macro_rules! choose {()=>{29_u64};} fn main(){let root=choose!();println!(\"{:?}\",[child::early(),child::late(),child::grand::early()[0],child::grand::early()[1],child::grand::early()[2],child::grand::late()[0],child::grand::late()[1],child::grand::late()[2],root]);}",
        ),
        (
            "child.rs",
            vec!["child"],
            "pub fn early()->u64{choose!()} macro_rules! choose {()=>{41_u64};} pub mod grand; macro_rules! choose {()=>{53_u64};} pub fn late()->u64{choose!()}",
        ),
        (
            "child/grand.rs",
            vec!["child", "grand"],
            "pub fn early()->[u64;3]{use choose as frozen;fn nested()->[u64;3]{fn leaf()->u64{frozen!()}[choose!(),frozen!(),leaf()]}macro_rules! choose {()=>{99_u64};}nested()} macro_rules! choose {()=>{61_u64};} pub fn late()->[u64;3]{use choose as frozen;fn nested()->[u64;3]{fn leaf()->u64{frozen!()}[choose!(),frozen!(),leaf()]}macro_rules! choose {()=>{99_u64};}nested()}",
        ),
    ];
    for (path, _, text) in &sources {
        fs::write(work.join(path), text).unwrap();
    }
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let binary = work.join("native");
    let compiler = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("main.rs"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    fs::write(work.join("compiler.stderr"), &compiler.stderr).unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let native = Command::new(binary).output().unwrap();
    assert!(native.status.success());
    fs::write(work.join("native.stdout"), &native.stdout).unwrap();
    let native_values: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let units: Vec<_> = sources
        .iter()
        .map(|(path, module, text)| SourceUnitInput {
            path: SourcePath::new(path).unwrap(),
            crate_key: Digest([1; 32]),
            module_path: module.iter().map(|s| (*s).to_owned()).collect(),
            profile: ProfileIdentity::current(),
            cfg: CfgSet::default(),
            blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        })
        .collect();
    let selections: Vec<_> = units
        .iter()
        .map(|unit| SourceSelection {
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
        })
        .collect();
    let prepared = PreparedSourceInputs::refresh(None, units, Default::default()).unwrap();
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let mut views: Vec<_> = selections
        .iter()
        .map(|s| {
            let SourceSelectionOutcome::Selected(view) = prepared.select(s).unwrap() else {
                panic!("source absent")
            };
            Some(view)
        })
        .collect();
    let mut definitions = BTreeMap::new();
    let mut bodies = Vec::new();
    let mut forests = Vec::new();
    for (file, view) in views.iter().enumerate() {
        let view = view.as_ref().unwrap();
        let mut file_bodies = BTreeMap::new();
        for item in &view.outline().items {
            if item.def_key.namespace == lrr::source::manifest::Namespace::Macro {
                definitions.insert(item.def_key.fingerprint(), (file, item.declaration_span));
            }
            if let Some(body) = &item.body {
                file_bodies.insert(item.def_key.declared_name.clone(), body.key());
            }
        }
        bodies.push(file_bodies);
        forests.push(
            parse_source(
                sources[file].2,
                SourceIdentity {
                    unit: Key([1; 32]),
                    content: Key(selections[file].outline.parse.content.0),
                    path: sources[file].0.into(),
                },
                ContextRecord {
                    parent: None,
                    expansion: None,
                    transparency: Transparency::Opaque,
                    defining_unit: Key([1; 32]),
                    edition: Edition::E2024,
                },
                Key([2; 32]),
            )
            .unwrap(),
        );
    }
    let (ctx, f) = facts();
    let main = PublishedMacroIndex::from_view(views[0].take().unwrap(), ctx, f).unwrap();
    let (ctx, f) = facts();
    let child = main
        .link_module(
            &selections[0],
            main.module_input(&["child".into()]).unwrap(),
            views[1].take().unwrap(),
            ctx,
            f,
        )
        .unwrap();
    let child_chain = vec![selections[1].clone(), selections[0].clone()];
    let (ctx, f) = facts();
    let grand = child
        .link_module(
            &child_chain,
            child
                .module_input(&["child".into(), "grand".into()])
                .unwrap(),
            views[2].take().unwrap(),
            ctx,
            f,
        )
        .unwrap();
    let grand_chain = vec![
        selections[2].clone(),
        selections[1].clone(),
        selections[0].clone(),
    ];
    let mut observed = Vec::new();
    for name in ["early", "late"] {
        let body = &bodies[1][name];
        let entered = child.enter_body(&child_chain, body).unwrap();
        let at = if name == "early" {
            sources[1].2.find("choose!()")
        } else {
            sources[1].2.rfind("choose!()")
        }
        .unwrap();
        observed.push(expand(
            selected(
                entered
                    .lookup_invocation(
                        &child_chain,
                        body,
                        SourceRange {
                            start: u32::try_from(at).unwrap(),
                            end: u32::try_from(at + 9).unwrap(),
                        },
                        &MacroNamespaces::default(),
                    )
                    .unwrap(),
            ),
            &definitions,
            &forests,
        ));
    }
    for name in ["early", "late"] {
        let body = &bodies[2][name];
        let outer = grand.enter_body(&grand_chain, body).unwrap();
        let key = outer
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
        let nested = outer.enter_nested_body(&grand_chain, body, &key).unwrap();
        for call in &nested.facts().invocations {
            observed.push(expand(
                selected(
                    nested
                        .lookup_invocation(
                            &grand_chain,
                            &key,
                            call.range,
                            &MacroNamespaces::default(),
                        )
                        .unwrap(),
                ),
                &definitions,
                &forests,
            ));
        }
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
        let leaf_entered = nested.enter_nested_body(&grand_chain, &key, &leaf).unwrap();
        observed.push(expand(
            selected(
                leaf_entered
                    .lookup_invocation(
                        &grand_chain,
                        &leaf,
                        leaf_entered.facts().invocations[0].range,
                        &MacroNamespaces::default(),
                    )
                    .unwrap(),
            ),
            &definitions,
            &forests,
        ));
    }
    let entered = main.enter_body(&selections[0], &bodies[0]["main"]).unwrap();
    observed.push(expand(
        selected(
            entered
                .lookup_invocation(
                    &selections[0],
                    &bodies[0]["main"],
                    {
                        let at = sources[0].2.find("choose!()").unwrap();
                        SourceRange {
                            start: u32::try_from(at).unwrap(),
                            end: u32::try_from(at + 9).unwrap(),
                        }
                    },
                    &MacroNamespaces::default(),
                )
                .unwrap(),
        ),
        &definitions,
        &forests,
    ));
    assert_eq!(observed, native_values);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","passed":true,"native":native_values,"observed":observed,"scope":"same immutable publication, three-file external-module ancestry and explicit nested/grandchild entry, declaration-order captures and inherited frozen import contexts; exact full-source-chain validation and selected pinned expansion from original forests; guest VM pending"})).unwrap()).unwrap();
}

#[test]
fn container_body_capture_and_module_import_boundary_match_native_macos() {
    use lrr::source::manifest::ItemKind;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-container-body-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let source = "fn main(){macro_rules! m {()=>{17_u64};} use m as outer_alias; mod local {use m as frozen;pub fn early()->[u64;2]{[m!(),frozen!()]} macro_rules! m {()=>{41_u64};} pub mod nested {pub fn leaf()->u64{m!()}} macro_rules! m {()=>{61_u64};}} struct Local;trait Trait {fn method()->[u64;2]{[m!(),outer_alias!()]}} impl Trait for Local {} impl Local {fn inherent()->[u64;2]{[m!(),outer_alias!()]}} macro_rules! m {()=>{31_u64};} let a=local::early();let b=<Local as Trait>::method();let c=Local::inherent();println!(\"{:?}\",[a[0],a[1],local::nested::leaf(),b[0],b[1],c[0],c[1]]);}";
    fs::write(work.join("native.rs"), source).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), &version.stdout).unwrap();
    let compiled = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "native.rs", "-o", "native"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("native.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = Command::new(work.join("native")).output().unwrap();
    assert!(output.status.success());
    fs::write(work.join("native.stdout"), &output.stdout).unwrap();
    let native: Vec<u64> = serde_json::from_slice(&output.stdout).unwrap();
    let denied = source.replace("[m!(),frozen!()]", "[m!(),outer_alias!()]");
    fs::write(work.join("denied.rs"), denied).unwrap();
    let rejected = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "denied.rs", "-o", "denied"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("denied.stderr"), &rejected.stderr).unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr)
            .contains("cannot find macro `outer_alias` in this scope")
    );
    let unit = SourceUnitInput {
        path: SourcePath::new("source.rs").unwrap(),
        crate_key: Digest([1; 32]),
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::default(),
        blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
    };
    let selection = SourceSelection {
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
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("source missing")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (context, hygiene) = facts();
    let index = PublishedMacroIndex::from_view(view, context, hygiene).unwrap();
    let entered = index.enter_body(&selection, &body).unwrap();
    let forest = parse_source(
        source,
        SourceIdentity {
            unit: Key([1; 32]),
            content: Key(selection.outline.parse.content.0),
            path: "source.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([1; 32]),
            edition: Edition::E2024,
        },
        Key([2; 32]),
    )
    .unwrap();
    let forests = vec![forest];
    let mut definitions = BTreeMap::new();
    for item in &entered.facts().items {
        if item.item.def_key.item_kind == ItemKind::Macro {
            definitions.insert(
                item.item.def_key.fingerprint(),
                (0, item.item.declaration_span),
            );
        }
    }
    let graph = MacroNamespaces::default();
    let mut observed = Vec::new();
    for name in ["local", "Trait", "implLocal"] {
        let item = entered
            .facts()
            .items
            .iter()
            .find(|i| {
                if name == "implLocal" {
                    i.item.def_key.item_kind == ItemKind::Impl
                        && i.item
                            .header
                            .tokens
                            .iter()
                            .map(|t| t.text.as_str())
                            .collect::<String>()
                            == name
                } else {
                    i.item.def_key.declared_name == name
                }
            })
            .unwrap();
        let key = &item.container.as_ref().unwrap().key;
        let catalogue = entered.enter_container(&selection, &body, key).unwrap();
        for item in &catalogue.facts().items {
            if item.item.def_key.item_kind == ItemKind::Macro {
                definitions.insert(
                    item.item.def_key.fingerprint(),
                    (0, item.item.declaration_span),
                );
            }
        }
        let member = catalogue
            .facts()
            .items
            .iter()
            .find(|i| i.item.body.is_some())
            .unwrap();
        let member_key = member.item.body.as_ref().unwrap().key();
        let child = catalogue.enter_body(&selection, key, &member_key).unwrap();
        for call in &child.facts().invocations {
            observed.push(expand(
                selected(
                    child
                        .lookup_invocation(&selection, &member_key, call.range, &graph)
                        .unwrap(),
                ),
                &definitions,
                &forests,
            ));
        }
        if name == "local" {
            let nested = catalogue
                .facts()
                .items
                .iter()
                .find(|i| i.item.def_key.declared_name == "nested")
                .unwrap();
            let nested_key = &nested.container.as_ref().unwrap().key;
            let inner = catalogue
                .enter_container(&selection, key, nested_key)
                .unwrap();
            let leaf_key = inner.facts().items[0].item.body.as_ref().unwrap().key();
            let leaf = inner.enter_body(&selection, nested_key, &leaf_key).unwrap();
            assert_eq!(leaf.facts().invocations.len(), 1);
            observed.push(expand(
                selected(
                    leaf.lookup_invocation(
                        &selection,
                        &leaf_key,
                        leaf.facts().invocations[0].range,
                        &graph,
                    )
                    .unwrap(),
                ),
                &definitions,
                &forests,
            ));
        }
    }
    assert_eq!(observed.len(), 7);
    assert_eq!(observed, native);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"explicit container function entry, module/trait/impl capture, frozen imports and module import boundary; actual pinned hygiene and full original parsed-source macro expansion versus native macOS; no guest VM claim","native":native,"observed_expansions":observed,"native_module_import_rejected":true,"passed":true})).unwrap()).unwrap();
}

#[test]
fn linked_container_macro_capture_matches_native_macos() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-linked-container-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    fs::create_dir(work.join("child")).unwrap();
    let sources = [
        (
            "main.rs",
            vec![],
            "macro_rules! choose {()=>{17_u64};} mod child; macro_rules! choose {()=>{29_u64};} fn main(){let root=choose!();println!(\"{:?}\",[child::early(),child::late(),child::grand::early()[0],child::grand::early()[1],child::grand::early()[2],child::grand::late()[0],child::grand::late()[1],child::grand::late()[2],root]);}",
        ),
        (
            "child.rs",
            vec!["child"],
            "pub fn early()->u64{choose!()} macro_rules! choose {()=>{41_u64};} pub mod grand; macro_rules! choose {()=>{53_u64};} pub fn late()->u64{choose!()}",
        ),
        (
            "child/grand.rs",
            vec!["child", "grand"],
            "pub fn early()->[u64;3]{mod local {use choose as frozen;pub fn nested()->[u64;2]{[choose!(),frozen!()]}macro_rules! choose {()=>{73_u64};} pub mod deeper {pub fn leaf()->u64{choose!()}}}macro_rules! choose {()=>{99_u64};}[local::nested()[0],local::nested()[1],local::deeper::leaf()]} macro_rules! choose {()=>{61_u64};} pub fn late()->[u64;3]{mod local {use choose as frozen;pub fn nested()->[u64;2]{[choose!(),frozen!()]}macro_rules! choose {()=>{73_u64};} pub mod deeper {pub fn leaf()->u64{choose!()}}}macro_rules! choose {()=>{99_u64};}[local::nested()[0],local::nested()[1],local::deeper::leaf()]}",
        ),
    ];
    for (path, _, text) in &sources {
        fs::write(work.join(path), text).unwrap();
    }
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let binary = work.join("native");
    let compiler = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("main.rs"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    fs::write(work.join("compiler.stderr"), &compiler.stderr).unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let native = Command::new(binary).output().unwrap();
    assert!(native.status.success());
    fs::write(work.join("native.stdout"), &native.stdout).unwrap();
    let native_values: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let units: Vec<_> = sources
        .iter()
        .map(|(path, module, text)| SourceUnitInput {
            path: SourcePath::new(path).unwrap(),
            crate_key: Digest([1; 32]),
            module_path: module.iter().map(|s| (*s).to_owned()).collect(),
            profile: ProfileIdentity::current(),
            cfg: CfgSet::default(),
            blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        })
        .collect();
    let selections: Vec<_> = units
        .iter()
        .map(|unit| SourceSelection {
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
        })
        .collect();
    let prepared = PreparedSourceInputs::refresh(None, units, Default::default()).unwrap();
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let mut views: Vec<_> = selections
        .iter()
        .map(|s| {
            let SourceSelectionOutcome::Selected(view) = prepared.select(s).unwrap() else {
                panic!("source absent")
            };
            Some(view)
        })
        .collect();
    let mut definitions = BTreeMap::new();
    let mut bodies = Vec::new();
    let mut forests = Vec::new();
    for (file, view) in views.iter().enumerate() {
        let view = view.as_ref().unwrap();
        let mut file_bodies = BTreeMap::new();
        for item in &view.outline().items {
            if item.def_key.namespace == lrr::source::manifest::Namespace::Macro {
                definitions.insert(item.def_key.fingerprint(), (file, item.declaration_span));
            }
            if let Some(body) = &item.body {
                file_bodies.insert(item.def_key.declared_name.clone(), body.key());
            }
        }
        bodies.push(file_bodies);
        forests.push(
            parse_source(
                sources[file].2,
                SourceIdentity {
                    unit: Key([1; 32]),
                    content: Key(selections[file].outline.parse.content.0),
                    path: sources[file].0.into(),
                },
                ContextRecord {
                    parent: None,
                    expansion: None,
                    transparency: Transparency::Opaque,
                    defining_unit: Key([1; 32]),
                    edition: Edition::E2024,
                },
                Key([2; 32]),
            )
            .unwrap(),
        );
    }
    let (ctx, f) = facts();
    let main = PublishedMacroIndex::from_view(views[0].take().unwrap(), ctx, f).unwrap();
    let (ctx, f) = facts();
    let child = main
        .link_module(
            &selections[0],
            main.module_input(&["child".into()]).unwrap(),
            views[1].take().unwrap(),
            ctx,
            f,
        )
        .unwrap();
    let child_chain = vec![selections[1].clone(), selections[0].clone()];
    let (ctx, f) = facts();
    let grand = child
        .link_module(
            &child_chain,
            child
                .module_input(&["child".into(), "grand".into()])
                .unwrap(),
            views[2].take().unwrap(),
            ctx,
            f,
        )
        .unwrap();
    let grand_chain = vec![
        selections[2].clone(),
        selections[1].clone(),
        selections[0].clone(),
    ];
    let mut observed = Vec::new();
    for name in ["early", "late"] {
        let body = &bodies[1][name];
        let entered = child.enter_body(&child_chain, body).unwrap();
        let at = if name == "early" {
            sources[1].2.find("choose!()")
        } else {
            sources[1].2.rfind("choose!()")
        }
        .unwrap();
        observed.push(expand(
            selected(
                entered
                    .lookup_invocation(
                        &child_chain,
                        body,
                        SourceRange {
                            start: u32::try_from(at).unwrap(),
                            end: u32::try_from(at + 9).unwrap(),
                        },
                        &MacroNamespaces::default(),
                    )
                    .unwrap(),
            ),
            &definitions,
            &forests,
        ));
    }
    for name in ["early", "late"] {
        let body = &bodies[2][name];
        let outer = grand.enter_body(&grand_chain, body).unwrap();
        let local_key = &outer
            .facts()
            .items
            .iter()
            .find(|i| i.item.def_key.declared_name == "local")
            .unwrap()
            .container
            .as_ref()
            .unwrap()
            .key;
        let local = outer
            .enter_container(&grand_chain, body, local_key)
            .unwrap();
        for item in &local.facts().items {
            if item.item.def_key.namespace == lrr::source::manifest::Namespace::Macro {
                definitions.insert(
                    item.item.def_key.fingerprint(),
                    (2, item.item.declaration_span),
                );
            }
        }
        let key = local
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
        let nested = local.enter_body(&grand_chain, local_key, &key).unwrap();
        assert_eq!(nested.facts().invocations.len(), 2);
        for call in &nested.facts().invocations {
            observed.push(expand(
                selected(
                    nested
                        .lookup_invocation(
                            &grand_chain,
                            &key,
                            call.range,
                            &MacroNamespaces::default(),
                        )
                        .unwrap(),
                ),
                &definitions,
                &forests,
            ));
        }
        let deeper_key = &local
            .facts()
            .items
            .iter()
            .find(|i| i.item.def_key.declared_name == "deeper")
            .unwrap()
            .container
            .as_ref()
            .unwrap()
            .key;
        let deeper = local
            .enter_container(&grand_chain, local_key, deeper_key)
            .unwrap();
        let leaf = deeper
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
        let leaf_entered = deeper.enter_body(&grand_chain, deeper_key, &leaf).unwrap();
        assert_eq!(leaf_entered.facts().invocations.len(), 1);
        observed.push(expand(
            selected(
                leaf_entered
                    .lookup_invocation(
                        &grand_chain,
                        &leaf,
                        leaf_entered.facts().invocations[0].range,
                        &MacroNamespaces::default(),
                    )
                    .unwrap(),
            ),
            &definitions,
            &forests,
        ));
    }
    let entered = main.enter_body(&selections[0], &bodies[0]["main"]).unwrap();
    observed.push(expand(
        selected(
            entered
                .lookup_invocation(
                    &selections[0],
                    &bodies[0]["main"],
                    {
                        let at = sources[0].2.find("choose!()").unwrap();
                        SourceRange {
                            start: u32::try_from(at).unwrap(),
                            end: u32::try_from(at + 9).unwrap(),
                        }
                    },
                    &MacroNamespaces::default(),
                )
                .unwrap(),
        ),
        &definitions,
        &forests,
    ));
    assert_eq!(observed.len(), 9);
    assert_eq!(observed, native_values);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","passed":true,"native":native_values,"observed":observed,"scope":"same immutable publication, three-file external-module ancestry and explicit local/recursive-container function entry, declaration-order captures, local frozen imports and container-local macro definitions; exact full-source-chain validation and selected pinned expansion from original forests; guest VM pending"})).unwrap()).unwrap();
}

#[test]
fn linked_original_local_selection_matches_native_macos_across_function_and_container_entry() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-linked-locals-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    fs::create_dir(work.join("child")).unwrap();
    let sources = [
        (
            "main.rs",
            vec![],
            "mod child;fn main(){println!(\"{:?}\",child::grand::selected());}",
        ),
        ("child.rs", vec!["child"], "pub mod grand;"),
        (
            "child/grand.rs",
            vec!["child", "grand"],
            "pub fn selected()->[u64;4]{let mut x=17;let inner={let mut x=31;x};fn nested()->u64{let mut n=53;n}mod local{pub fn member()->u64{let mut z=61;z}}[x,inner,nested(),local::member()]}",
        ),
    ];
    for (path, _, source) in &sources {
        fs::write(work.join(path), source).unwrap();
    }
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), &version.stdout).unwrap();
    let compiled = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "main.rs", "-o", "native"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("native.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = Command::new(work.join("native")).output().unwrap();
    assert!(output.status.success());
    fs::write(work.join("native.stdout"), &output.stdout).unwrap();
    let native: Vec<u64> = serde_json::from_slice(&output.stdout).unwrap();
    let units: Vec<_> = sources
        .iter()
        .map(|(path, module, text)| SourceUnitInput {
            path: SourcePath::new(path).unwrap(),
            crate_key: Digest([1; 32]),
            module_path: module.iter().map(|s| (*s).to_owned()).collect(),
            profile: ProfileIdentity::current(),
            cfg: CfgSet::default(),
            blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        })
        .collect();
    let selections: Vec<_> = units
        .iter()
        .map(|u| SourceSelection {
            path: u.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: u.blob.content(),
                    profile: u.profile.clone(),
                },
                crate_key: u.crate_key,
                module_path: u.module_path.clone(),
                cfg: u.cfg.fingerprint(),
            },
        })
        .collect();
    let prepared = PreparedSourceInputs::refresh(None, units, Default::default()).unwrap();
    let mut views: Vec<_> = selections
        .iter()
        .map(|s| {
            let SourceSelectionOutcome::Selected(v) = prepared.select(s).unwrap() else {
                panic!("source missing")
            };
            Some(v)
        })
        .collect();
    let body = views[2]
        .as_ref()
        .unwrap()
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "selected")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (ctx, hygiene) = facts();
    let main = PublishedMacroIndex::from_view(views[0].take().unwrap(), ctx, hygiene).unwrap();
    let (ctx, hygiene) = facts();
    let child = main
        .link_module(
            &selections[0],
            main.module_input(&["child".into()]).unwrap(),
            views[1].take().unwrap(),
            ctx,
            hygiene,
        )
        .unwrap();
    let child_chain = vec![selections[1].clone(), selections[0].clone()];
    let (ctx, hygiene) = facts();
    let grand = child
        .link_module(
            &child_chain,
            child
                .module_input(&["child".into(), "grand".into()])
                .unwrap(),
            views[2].take().unwrap(),
            ctx,
            hygiene,
        )
        .unwrap();
    let chain = vec![
        selections[2].clone(),
        selections[1].clone(),
        selections[0].clone(),
    ];
    let outer = grand.enter_body(&chain, &body).unwrap();
    let locals = outer
        .local_index(&chain, &body, &Default::default(), &[])
        .unwrap();
    let source = sources[2].2;
    let mut values = BTreeMap::new();
    let mut add_values = |bindings: &[lrr::syntax::local_bindings::LocalBindingRecipe]| {
        for b in bindings.iter().filter(|b| {
            b.classification == lrr::syntax::local_bindings::PatternNameKind::ExplicitBinding
        }) {
            let bytes =
                &source[b.name_range.end as usize..usize::try_from(b.visible_from).unwrap()];
            values.insert(
                b.identity,
                bytes
                    .trim()
                    .strip_prefix('=')
                    .unwrap()
                    .trim()
                    .trim_end_matches(';')
                    .trim()
                    .parse::<u64>()
                    .unwrap(),
            );
        }
    };
    add_values(&outer.facts().bindings);
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
    add_values(&nested.facts().bindings);
    let local_key = &outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "local")
        .unwrap()
        .container
        .as_ref()
        .unwrap()
        .key;
    let catalogue = outer.enter_container(&chain, &body, local_key).unwrap();
    let member_key = catalogue.facts().items[0].item.body.as_ref().unwrap().key();
    let member = catalogue
        .enter_body(&chain, local_key, &member_key)
        .unwrap();
    add_values(&member.facts().bindings);
    let nested_locals = nested
        .local_index(&chain, &nested_key, &Default::default(), &[])
        .unwrap();
    let member_locals = member
        .local_index(&chain, &member_key, &Default::default(), &[])
        .unwrap();
    let mut observed = Vec::new();
    let mut identities = Vec::new();
    for (index, body, token, name) in [
        (&locals, &body, "[x,inner", "x"),
        (&locals, &body, "x};fn", "x"),
        (&nested_locals, &nested_key, "n}mod", "n"),
        (&member_locals, &member_key, "z}}[", "z"),
    ] {
        let at = source.find(token).unwrap() + usize::from(token.starts_with('['));
        let lrr::frontend::local_scopes::LocalLookup::Binding(binding) = index
            .lookup(
                &chain,
                body,
                u64::try_from(at).unwrap(),
                name,
                index.context(),
            )
            .unwrap()
        else {
            panic!("linked original local missing")
        };
        observed.push(values[&binding]);
        identities.push(binding.hex());
    }
    assert_eq!(observed.len(), 4);
    assert_eq!(observed, native);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","passed":true,"native":native,"observed_original_values":observed,"selected_binding_ids":identities,"scope":"three-file linked original local index, block shadowing, named-function and body-local container function entry; actual pinned root hygiene and source-derived initializer values versus native macOS; no guest VM or arbitrary constant evaluation claim"})).unwrap()).unwrap();
}

#[test]
fn original_for_binding_scope_selection_matches_native_macos() {
    use lrr::syntax::local_bindings::BindingForm;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-for-bindings-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let source = "fn main(){let mut x=17;let mut values=vec![];for (mut x,mut y) in {values.push(x);[(31,41)]} {values.push(x);values.push(y);for mut x in [53] {values.push(x);}values.push(x);}values.push(x);println!(\"{:?}\",values);}";
    fs::write(work.join("native.rs"), source).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), &version.stdout).unwrap();
    let compile = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "native.rs", "-o", "native"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("native.stderr"), &compile.stderr).unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = Command::new(work.join("native")).output().unwrap();
    assert!(output.status.success());
    fs::write(work.join("native.stdout"), &output.stdout).unwrap();
    let native: Vec<u64> = serde_json::from_slice(&output.stdout).unwrap();
    let unit = SourceUnitInput {
        path: SourcePath::new("native.rs").unwrap(),
        crate_key: Digest([1; 32]),
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::default(),
        blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
    };
    let selection = SourceSelection {
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
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("source missing")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (context, hygiene) = facts();
    let index = PublishedMacroIndex::from_view(view, context, hygiene).unwrap();
    let entered = index.enter_body(&selection, &body).unwrap();
    let locals = entered
        .local_index(&selection, &body, &Default::default(), &[])
        .unwrap();
    let forest = parse_source(
        source,
        SourceIdentity {
            unit: Key([1; 32]),
            content: Key(selection.outline.parse.content.0),
            path: "native.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([1; 32]),
            edition: Edition::E2024,
        },
        Key([2; 32]),
    )
    .unwrap();
    let spans: BTreeMap<_, _> = forest.spans().iter().map(|s| (s.key(), s)).collect();
    let mut literals = Vec::new();
    let mut stack: Vec<_> = forest.roots().iter().rev().collect();
    while let Some(token) = stack.pop() {
        match token {
            TokenTree::Group { children, .. } => stack.extend(children.iter().rev()),
            TokenTree::Literal { token_text, span } => {
                if let Ok(value) = token_text.parse::<u64>() {
                    literals.push((spans[span].start, spans[span].end, value));
                }
            }
            _ => {}
        }
    }
    let mut values = BTreeMap::new();
    let mut groups = BTreeMap::<u32, Vec<_>>::new();
    for binding in &entered.facts().bindings {
        if binding.form == BindingForm::For {
            groups.entry(binding.scope).or_default().push(binding);
        } else if binding.form == BindingForm::Let && binding.name == "x" {
            let bytes = &source
                [binding.name_range.end as usize..usize::try_from(binding.visible_from).unwrap()];
            values.insert(
                binding.identity,
                bytes
                    .trim()
                    .strip_prefix('=')
                    .unwrap()
                    .trim()
                    .trim_end_matches(';')
                    .trim()
                    .parse::<u64>()
                    .unwrap(),
            );
        }
    }
    assert_eq!(groups.len(), 2);
    for (scope, mut bindings) in groups {
        bindings.sort_by_key(|b| b.name_range.start);
        let start = bindings[0].pattern_range.end;
        let end = entered
            .facts()
            .scopes
            .iter()
            .find(|s| s.ordinal == scope)
            .unwrap()
            .range
            .start;
        let elements: Vec<_> = literals
            .iter()
            .filter(|(at, to, _)| *at >= start && *to <= end)
            .map(|(_, _, value)| *value)
            .collect();
        assert_eq!(bindings.len(), elements.len());
        for (binding, value) in bindings.into_iter().zip(elements) {
            values.insert(binding.identity, value);
        }
    }
    let mut observed = Vec::new();
    let mut identities = Vec::new();
    for (at, _) in source.match_indices("values.push(") {
        let at = at + "values.push(".len();
        let name = &source[at..at + 1];
        let lrr::frontend::local_scopes::LocalLookup::Binding(binding) = locals
            .lookup(
                &selection,
                &body,
                u64::try_from(at).unwrap(),
                name,
                locals.context(),
            )
            .unwrap()
        else {
            panic!("for variable binding missing")
        };
        observed.push(values[&binding]);
        identities.push(binding.hex());
    }
    assert_eq!(observed.len(), 6);
    assert_eq!(observed, native);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","passed":true,"native":native,"observed_original_values":observed,"selected_binding_ids":identities,"scope":"original for-pattern candidates and source local lookup, iterator block parent visibility, tuple and nested loop body scopes, post-loop restoration; pinned root hygiene and source literal elements versus native macOS; no guest loop execution or general iterator evaluation claim"})).unwrap()).unwrap();
}

#[test]
fn original_match_binding_scope_selection_matches_native_macos() {
    use lrr::syntax::local_bindings::BindingForm;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-match-bindings-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let source = "fn main(){let mut x=17;let mut values=vec![];match 31 {mut x @ 31 if {values.push(x);true}=>values.push(x),_=>{},}match 41 {mut y @ 41=>{values.push(x);values.push(y);},_=>{},}values.push(x);println!(\"{:?}\",values);}";
    fs::write(work.join("native.rs"), source).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), &version.stdout).unwrap();
    let compile = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "native.rs", "-o", "native"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("native.stderr"), &compile.stderr).unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = Command::new(work.join("native")).output().unwrap();
    assert!(output.status.success());
    fs::write(work.join("native.stdout"), &output.stdout).unwrap();
    let native: Vec<u64> = serde_json::from_slice(&output.stdout).unwrap();
    let unit = SourceUnitInput {
        path: SourcePath::new("native.rs").unwrap(),
        crate_key: Digest([1; 32]),
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::default(),
        blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
    };
    let selection = SourceSelection {
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
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("source missing")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (context, hygiene) = facts();
    let index = PublishedMacroIndex::from_view(view, context, hygiene).unwrap();
    let entered = index.enter_body(&selection, &body).unwrap();
    let locals = entered
        .local_index(&selection, &body, &Default::default(), &[])
        .unwrap();
    let mut values = BTreeMap::new();
    for binding in &entered.facts().bindings {
        let value = if binding.form == BindingForm::Match {
            let pattern =
                &source[binding.pattern_range.start as usize..binding.pattern_range.end as usize];
            pattern
                .split_once('@')
                .expect("fixture literal @ pattern")
                .1
                .trim()
                .parse::<u64>()
                .unwrap()
        } else if binding.form == BindingForm::Let && binding.name == "x" {
            let initializer =
                &source[binding.name_range.end as usize..binding.visible_from as usize];
            initializer
                .trim()
                .strip_prefix('=')
                .unwrap()
                .trim()
                .trim_end_matches(';')
                .trim()
                .parse::<u64>()
                .unwrap()
        } else {
            continue;
        };
        values.insert(binding.identity, value);
    }
    assert_eq!(values.len(), 3);
    let mut observed = Vec::new();
    let mut identities = Vec::new();
    for (at, _) in source.match_indices("values.push(") {
        let at = at + "values.push(".len();
        let name = &source[at..at + 1];
        let lrr::frontend::local_scopes::LocalLookup::Binding(binding) = locals
            .lookup(
                &selection,
                &body,
                u64::try_from(at).unwrap(),
                name,
                locals.context(),
            )
            .unwrap()
        else {
            panic!("match variable binding missing")
        };
        observed.push(values[&binding]);
        identities.push(binding.hex());
    }
    assert_eq!(observed.len(), 5);
    assert_eq!(observed, native);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","passed":true,"native":native,"observed_original_values":observed,"selected_binding_ids":identities,"scope":"original match-pattern bindings, guard and body scopes, unrelated arm and outer-name restoration; source literal @ patterns and initializer values versus actual native macOS, no guest match execution or general pattern evaluation claim"})).unwrap()).unwrap();
}

#[test]
fn original_receiver_binding_scope_selection_matches_native_macos() {
    use lrr::syntax::local_bindings::BindingForm;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-receiver-bindings-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    for (ordinal, (receiver, call)) in [
        ("self", "S(17).method()"),
        ("&self", "S(31).method()"),
        ("&mut self", "S(41).method()"),
        ("self: Box<Self>", "Box::new(S(53)).method()"),
    ]
    .into_iter()
    .enumerate()
    {
        let work = work.join(format!("receiver-{ordinal}"));
        fs::create_dir(&work).unwrap();
        let source = format!(
            "fn main(){{struct S(u64);impl S{{fn method({receiver})->u64{{self.0}}}}println!(\"[{{}}]\",{call});}}"
        );
        fs::write(work.join("native.rs"), &source).unwrap();
        let version = Command::new("rustc")
            .args(["+1.99.0", "-Vv"])
            .output()
            .unwrap();
        assert!(version.status.success());
        assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
        fs::write(work.join("rustc-version.txt"), &version.stdout).unwrap();
        let compile = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "native.rs", "-o", "native"])
            .current_dir(&work)
            .output()
            .unwrap();
        fs::write(work.join("native.stderr"), &compile.stderr).unwrap();
        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let output = Command::new(work.join("native")).output().unwrap();
        assert!(output.status.success());
        fs::write(work.join("native.stdout"), &output.stdout).unwrap();
        let native: Vec<u64> = serde_json::from_slice(&output.stdout).unwrap();
        let unit = SourceUnitInput {
            path: SourcePath::new("native.rs").unwrap(),
            crate_key: Digest([1; 32]),
            module_path: vec![],
            profile: ProfileIdentity::current(),
            cfg: CfgSet::default(),
            blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        };
        let selection = SourceSelection {
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
        let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
            panic!("source missing")
        };
        let body = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        let (context, hygiene) = facts();
        let index = PublishedMacroIndex::from_view(view, context, hygiene).unwrap();
        let parent = index.enter_body(&selection, &body).unwrap();
        let container = parent
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
        let catalogue = parent
            .enter_container(&selection, &body, &container)
            .unwrap();
        let method = catalogue.facts().items[0].item.body.as_ref().unwrap().key();
        let entered = catalogue
            .enter_body(&selection, &container, &method)
            .unwrap();
        let locals = entered
            .local_index(&selection, &method, &Default::default(), &[])
            .unwrap();
        let receiver_binding = entered
            .facts()
            .bindings
            .iter()
            .find(|b| b.form == BindingForm::Receiver)
            .unwrap();
        let value = source
            .rsplit_once("S(")
            .unwrap()
            .1
            .split_once(')')
            .unwrap()
            .0
            .parse::<u64>()
            .unwrap();
        let mut observed = Vec::new();
        let mut identities = Vec::new();
        let at = source.find("self.0").unwrap();
        let lrr::frontend::local_scopes::LocalLookup::Binding(binding) = locals
            .lookup(&selection, &method, at as u64, "self", locals.context())
            .unwrap()
        else {
            panic!("receiver binding missing")
        };
        assert_eq!(binding, receiver_binding.identity);
        observed.push(value);
        identities.push(binding.hex());
        assert_eq!(observed.len(), 1);
        assert_eq!(observed, native);
        fs::write(work.join(format!("report-{}.json",native[0])),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","passed":true,"native":native,"observed_original_values":observed,"selected_binding_ids":identities,"receiver":receiver,"scope":"original method receiver source identity selected by local lookup and fixture instance literal field versus actual native macOS; owned, borrowed, mutable and explicitly typed receiver forms; no guest method dispatch or general field evaluation claim"})).unwrap()).unwrap();
    }
}
