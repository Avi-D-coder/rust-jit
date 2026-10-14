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
    syntax::{parse::ParseKey, token_wire as runtime},
};
use lrr_expand_adapter::{
    declarative::DeclarativeMacro,
    hygiene::HygieneStore,
    token_wire::{self, Delimiter, Edition, Key, TokenForest, TokenTree},
};
use std::collections::BTreeMap;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[allow(dead_code)]
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
#[test]
fn source_index_selected_macro_definitions_match_native_module_shadowing() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-source-index-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let source = "macro_rules! r#gen {()=>{17};} mod inner { pub fn selected()->u64 {r#gen!()} } macro_rules! r#gen {()=>{29};} #[cfg(any())] macro_rules! r#gen {()=>{99};} fn main(){println!(\"[{},{}]\",inner::selected(),r#gen!());}";
    let path = work.join("source.rs");
    fs::write(&path, source).unwrap();
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
        .arg(&path)
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
    let values: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let mut store = HygieneStore::default();
    let context = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let rows = store.resolution_contexts(&[context]).unwrap();
    let names = store.name_contexts(&[context]).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let facts = HygieneFacts::new(
        runtime::TokenForest::decode(&wire)
            .unwrap()
            .contexts()
            .to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
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
            module_path: unit.module_path.clone(),
            cfg: unit.cfg.fingerprint(),
        },
    };
    let prepared = PreparedSourceInputs::refresh(None, vec![unit], Default::default()).unwrap();
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selected).unwrap() else {
        panic!("published source selection failed")
    };
    let outline = view.outline().clone();
    let index = PublishedMacroIndex::from_view(view, runtime::Key(context.0), facts).unwrap();
    let mut observed = Vec::new();
    for modules in [vec!["inner".to_owned()], vec![]] {
        let HygienicLookup::Selection(MacroSelection::Defined(definition)) = index
            .lookup(&selected, &modules, u64::MAX, "r#gen")
            .unwrap()
        else {
            panic!("source lookup failed")
        };
        let item = outline
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == definition.identity)
            .unwrap();
        let text = &source[usize::try_from(item.declaration_span.start).unwrap()
            ..usize::try_from(item.declaration_span.end).unwrap()];
        let forest = token_wire::parse_source(
            text,
            token_wire::SourceIdentity {
                unit: Key([1; 32]),
                content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
                path: "source.rs".into(),
            },
            token_wire::ContextRecord {
                parent: None,
                expansion: None,
                transparency: token_wire::Transparency::Opaque,
                defining_unit: Key([1; 32]),
                edition: Edition::E2024,
            },
            Key([2; 32]),
        )
        .unwrap();
        let [TokenTree::Group { children, .. }] = forest.roots() else {
            panic!("source root missing")
        };
        let body = children
            .iter()
            .find(|tree| {
                matches!(
                    tree,
                    TokenTree::Group {
                        delimiter: Delimiter::Brace,
                        ..
                    }
                )
            })
            .unwrap()
            .clone();
        let definition = DeclarativeMacro::new(
            TokenForest::new(
                vec![body],
                forest.contexts().to_vec(),
                forest.spans().to_vec(),
            )
            .unwrap(),
        )
        .unwrap();
        let input = fixture::input("");
        let output = definition
            .expand(&input, fixture::call_site(&input))
            .unwrap()
            .unwrap();
        let literals = fixture::literal_texts(&output.tokens);
        assert_eq!(literals.len(), 1);
        observed.push(literals[0].parse::<u64>().unwrap());
    }
    assert_eq!(observed, values);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","passed":true,"native":values,"observed":observed,"scope":"exact immutable source publication/view, actual outlining, owned name facts, module-parent textual selection and pinned expansion of selected original source slices; guest VM and generated-source publication pending"})).unwrap()).unwrap();
}

#[test]
fn entered_body_discovered_shadowing_matches_native_macos() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-entered-body-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let source = "macro_rules! choose {()=>{17_u64};} fn main(){let a=choose!(); let b={let x=choose!(); macro_rules! choose {()=>{41_u64};} [x,choose!()]}; macro_rules! choose {()=>{29_u64};} let c=choose!(); println!(\"{:?}\",[a,b[0],b[1],c]);} macro_rules! choose {()=>{53_u64};} fn cold(){macro_rules! hidden {()=>{99_u64};}}";
    let path = work.join("source.rs");
    fs::write(&path, source).unwrap();
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
        .arg(&path)
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
    let values: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let mut store = HygieneStore::default();
    let context = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let rows = store.resolution_contexts(&[context]).unwrap();
    let names = store.name_contexts(&[context]).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let facts = HygieneFacts::new(
        runtime::TokenForest::decode(&wire)
            .unwrap()
            .contexts()
            .to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
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
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selected).unwrap() else {
        panic!("source absent")
    };
    let outline = view.outline().clone();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let index = PublishedMacroIndex::from_view(view, runtime::Key(context.0), facts).unwrap();
    let entered = index.enter_body(&selected, &body).unwrap();
    let mut observed = Vec::new();
    let calls: Vec<_> = source
        .match_indices("choose!()")
        .map(|(at, _)| u64::try_from(at).unwrap())
        .collect();
    assert_eq!(calls.len(), 4);
    for at in calls {
        let lrr::frontend::macro_names::PathLookup::Defined(definition) = entered
            .lookup_invocation(
                &selected,
                &body,
                lrr::syntax::spans::SourceRange {
                    start: u32::try_from(at).unwrap(),
                    end: u32::try_from(at + 9).unwrap(),
                },
                &lrr::frontend::macro_names::MacroNamespaces::default(),
            )
            .unwrap()
        else {
            panic!("body selection failed")
        };
        let span = outline
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == definition.identity)
            .map(|item| item.declaration_span)
            .or_else(|| {
                entered
                    .facts()
                    .events
                    .iter()
                    .find_map(|event| match event.kind {
                        lrr::syntax::entered_macros::BodyMacroEventKind::Definition {
                            identity,
                            ..
                        } if identity == definition.identity => Some(event.range),
                        _ => None,
                    })
            })
            .unwrap();
        let text =
            &source[usize::try_from(span.start).unwrap()..usize::try_from(span.end).unwrap()];
        let forest = token_wire::parse_source(
            text,
            token_wire::SourceIdentity {
                unit: Key([1; 32]),
                content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
                path: "source.rs".into(),
            },
            token_wire::ContextRecord {
                parent: None,
                expansion: None,
                transparency: token_wire::Transparency::Opaque,
                defining_unit: Key([1; 32]),
                edition: Edition::E2024,
            },
            Key([2; 32]),
        )
        .unwrap();
        let [TokenTree::Group { children, .. }] = forest.roots() else {
            panic!("root absent")
        };
        let rule = children
            .iter()
            .find(|t| {
                matches!(
                    t,
                    TokenTree::Group {
                        delimiter: Delimiter::Brace,
                        ..
                    }
                )
            })
            .unwrap()
            .clone();
        let definition = DeclarativeMacro::new(
            TokenForest::new(
                vec![rule],
                forest.contexts().to_vec(),
                forest.spans().to_vec(),
            )
            .unwrap(),
        )
        .unwrap();
        let input = fixture::input("");
        let output = definition
            .expand(&input, fixture::call_site(&input))
            .unwrap()
            .unwrap();
        let literals = fixture::literal_texts(&output.tokens);
        assert_eq!(literals.len(), 1);
        observed.push(
            literals[0]
                .strip_suffix("_u64")
                .unwrap()
                .parse::<u64>()
                .unwrap(),
        );
    }
    assert_eq!(observed, values);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","passed":true,"native":values,"observed":observed,"scope":"exact published body key, actual pinned AST lexical discovery, exact retained source invocation selection and name-class selection followed by selected original-source pinned expansion; generated-source and guest VM pending"})).unwrap()).unwrap();
}

#[test]
fn body_named_import_scope_and_shadowing_matches_native_macos() {
    use lrr::frontend::{macro_names::PathLookup, source_macro_routes::SourceMacroRoutes};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-body-import-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let source = "#[macro_export] macro_rules! one {()=>{17_u64};} #[macro_export] macro_rules! two {()=>{31_u64};} fn main(){let a=chosen!(); use crate::one as chosen; let b={use crate::two as chosen; chosen!()}; let c=chosen!();println!(\"{:?}\",[a,b,c]);}";
    fs::write(work.join("native.rs"), source).unwrap();
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
    let mut store = HygieneStore::default();
    let context = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let rows = store.resolution_contexts(&[context]).unwrap();
    let names = store.name_contexts(&[context]).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let facts = || {
        HygieneFacts::new(
            runtime::TokenForest::decode(&wire)
                .unwrap()
                .contexts()
                .to_vec(),
            serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
        )
        .unwrap()
        .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
        .unwrap()
    };
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
        panic!("source missing");
    };
    let outline = view.outline().clone();
    let body = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let graph = SourceMacroRoutes::candidate_graph(&[SourceMacroRoutes::from_outline(
        &outline,
        runtime::Key(context.0),
        facts(),
        &BTreeMap::new(),
    )
    .unwrap()])
    .unwrap();
    let index = PublishedMacroIndex::from_view(view, runtime::Key(context.0), facts()).unwrap();
    let entered = index.enter_body(&selected, &body).unwrap();
    let mut selections = Vec::new();
    for (ordinal, (at, _)) in source.match_indices("chosen!()").enumerate() {
        let result = entered
            .lookup_invocation(&selected, &body, runtime_range(at, 9), &graph)
            .unwrap();
        let PathLookup::Defined(definition) = result else {
            panic!("body import macro selection failed");
        };
        let name = if ordinal == 1 { "two" } else { "one" };
        let expected = outline
            .items
            .iter()
            .find(|i| i.def_key.declared_name == name)
            .unwrap()
            .def_key
            .fingerprint();
        assert_eq!(definition.identity, expected);
        selections.push(definition.identity.hex());
    }
    assert_eq!(selections.len(), 3);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"actual pinned hygiene, parsed source export graph and retained entered-body named imports, scope-wide use and nested alias shadowing; actual native macOS execution; selected definition identity comparison, no guest VM claim","native":native,"selected":selections,"passed":true})).unwrap()).unwrap();
}
#[test]
fn body_glob_scope_and_shadowing_matches_native_macos() {
    use lrr::frontend::{macro_names::PathLookup, source_macro_routes::SourceMacroRoutes};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-body-glob-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let source = "#[macro_export] macro_rules! one {()=>{17_u64};} #[macro_export] macro_rules! two {()=>{31_u64};} mod a {pub use crate::one as chosen;} mod b {pub use crate::two as chosen;} mod facade {pub use crate::a::*;} fn main(){let a=chosen!(); use a::*; use crate::facade::*; let b={use crate::b::chosen; use crate::a::*; chosen!()}; let c=chosen!();println!(\"{:?}\",[a,b,c]);}";
    fs::write(work.join("native.rs"), source).unwrap();
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
    let ambiguity = "#[macro_export] macro_rules! one {()=>{17};} #[macro_export] macro_rules! two {()=>{31};} mod a {pub use crate::one as chosen;} mod b {pub use crate::two as chosen;} fn main(){use crate::a::chosen; {use crate::b::*; chosen!();}}";
    fs::write(work.join("ambiguous.rs"), ambiguity).unwrap();
    let rejected = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "ambiguous.rs",
            "-o",
            "ambiguous",
        ])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("ambiguous.stderr"), &rejected.stderr).unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("E0659"));
    let private = ambiguity
        .replace("pub use crate::two as chosen", "use crate::two as chosen")
        .replace("chosen!();", "println!(\"{}\",chosen!());");
    fs::write(work.join("private.rs"), private).unwrap();
    let compiled = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "private.rs", "-o", "private"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("private.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new(work.join("private")).output().unwrap();
    assert!(executed.status.success());
    fs::write(work.join("private.stdout"), &executed.stdout).unwrap();
    let private_native: u64 = serde_json::from_slice(&executed.stdout).unwrap();
    assert_eq!(private_native, native[0]);
    let mut store = HygieneStore::default();
    let context = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let rows = store.resolution_contexts(&[context]).unwrap();
    let names = store.name_contexts(&[context]).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let facts = || {
        HygieneFacts::new(
            runtime::TokenForest::decode(&wire)
                .unwrap()
                .contexts()
                .to_vec(),
            serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
        )
        .unwrap()
        .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
        .unwrap()
    };
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
        panic!("source missing");
    };
    let outline = view.outline().clone();
    let body = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let graph = SourceMacroRoutes::candidate_graph(&[SourceMacroRoutes::from_outline(
        &outline,
        runtime::Key(context.0),
        facts(),
        &BTreeMap::new(),
    )
    .unwrap()])
    .unwrap();
    let index = PublishedMacroIndex::from_view(view, runtime::Key(context.0), facts()).unwrap();
    let entered = index.enter_body(&selected, &body).unwrap();
    let mut selections = Vec::new();
    for (ordinal, (at, _)) in source.match_indices("chosen!()").enumerate() {
        let result = entered
            .lookup_invocation(&selected, &body, runtime_range(at, 9), &graph)
            .unwrap();
        let PathLookup::Defined(definition) = result else {
            panic!("body import macro selection failed");
        };
        let name = if ordinal == 1 { "two" } else { "one" };
        let expected = outline
            .items
            .iter()
            .find(|i| i.def_key.declared_name == name)
            .unwrap()
            .def_key
            .fingerprint();
        assert_eq!(definition.identity, expected);
        selections.push(definition.identity.hex());
    }
    assert_eq!(selections.len(), 3);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"actual pinned hygiene, parsed source graph and retained body glob imports, scope-wide use, identity diamond, nested named-over-glob priority; actual native macOS execution; selected definition identity comparison, no guest VM claim","native":native,"private_native":private_native,"outer_glob_ambiguity":"E0659","selected":selections,"passed":true})).unwrap()).unwrap();
}
#[test]
fn body_glob_module_membership_matches_native_macos() {
    use lrr::frontend::{macro_names::PathLookup, source_macro_routes::SourceMacroRoutes};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-body-module-membership-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let source = "#[macro_export] macro_rules! one {()=>{17_u64};} #[macro_export] macro_rules! two {()=>{31_u64};} mod a {pub mod nested {pub use crate::one as chosen;}} mod b {pub use crate::two as chosen;} mod origin {mod hidden {pub mod nested {pub use crate::one as chosen;}} pub use hidden::nested as escaped;} pub use origin::escaped; fn main(){let a=chosen!(); use crate::a::*; use nested::*; use nested as nested; let b={use crate::b::chosen; use nested::*; chosen!()}; let d={use crate::two as nested; use crate::a::*; nested::chosen!()}; let c={use crate::escaped::*; chosen!()};println!(\"{:?}\",[a,b,d,c]);}";
    fs::write(work.join("native.rs"), source).unwrap();
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
    let cyclic = "#[macro_export] macro_rules! one {()=>{17};} mod a {pub mod nested {pub use crate::one as chosen;}} fn main(){use forward as nested; use nested as forward; use crate::a::*; println!(\"{}\",nested::chosen!());}";
    fs::write(work.join("cyclic.rs"), cyclic).unwrap();
    let rejected = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "cyclic.rs", "-o", "cyclic"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("cyclic.stderr"), &rejected.stderr).unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("E0432"));
    let ambiguous = "#[macro_export] macro_rules! one {()=>{17};} mod a {pub mod nested {pub use crate::one as chosen;}} mod b {pub mod nested {pub use crate::one as chosen;}} fn main(){use crate::a::*;use crate::b::*;println!(\"{}\",nested::chosen!());}";
    fs::write(work.join("ambiguous.rs"), ambiguous).unwrap();
    let rejected = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "ambiguous.rs",
            "-o",
            "ambiguous",
        ])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("ambiguous.stderr"), &rejected.stderr).unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("E0659"));
    let mut store = HygieneStore::default();
    let context = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let rows = store.resolution_contexts(&[context]).unwrap();
    let names = store.name_contexts(&[context]).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let facts = || {
        HygieneFacts::new(
            runtime::TokenForest::decode(&wire)
                .unwrap()
                .contexts()
                .to_vec(),
            serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
        )
        .unwrap()
        .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
        .unwrap()
    };
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
        panic!("source missing");
    };
    let outline = view.outline().clone();
    let body = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let graph = SourceMacroRoutes::candidate_graph(&[SourceMacroRoutes::from_outline(
        &outline,
        runtime::Key(context.0),
        facts(),
        &BTreeMap::new(),
    )
    .unwrap()])
    .unwrap();
    let index = PublishedMacroIndex::from_view(view, runtime::Key(context.0), facts()).unwrap();
    let entered = index.enter_body(&selected, &body).unwrap();
    let mut selections = Vec::new();
    for (ordinal, (at, _)) in source.match_indices("chosen!()").enumerate() {
        let result = entered
            .lookup_invocation(
                &selected,
                &body,
                runtime_range(
                    if source[..at].ends_with("nested::") {
                        at - 8
                    } else {
                        at
                    },
                    if source[..at].ends_with("nested::") {
                        17
                    } else {
                        9
                    },
                ),
                &graph,
            )
            .unwrap();
        let PathLookup::Defined(definition) = result else {
            panic!("body import macro selection failed");
        };
        let name = if ordinal == 1 { "two" } else { "one" };
        let expected = outline
            .items
            .iter()
            .find(|i| i.def_key.declared_name == name)
            .unwrap()
            .def_key
            .fingerprint();
        assert_eq!(definition.identity, expected);
        selections.push(definition.identity.hex());
    }
    assert_eq!(selections.len(), 4);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"actual pinned hygiene, parsed source graph and retained body-glob module membership, chained glob targets, named precedence and origin-aware escaped module access; actual native macOS execution; selected definition identity comparison, no guest VM claim","native":native,"strong_cycle_native":"E0432","module_ambiguity_native":"E0659","selected":selections,"passed":true})).unwrap()).unwrap();
}
#[test]
fn body_textual_import_captures_declaration_position_matches_native_macos() {
    use lrr::frontend::{macro_names::PathLookup, source_macro_routes::SourceMacroRoutes};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-body-textual-import-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let source = "fn main(){macro_rules! m {()=>{17_u64};} use m as chosen; macro_rules! m {()=>{31_u64};} let a=chosen!(); let b=m!(); let c={macro_rules! m {()=>{53_u64};} chosen!()};println!(\"{:?}\",[a,b,c]);}";
    fs::write(work.join("native.rs"), source).unwrap();
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
    let mut store = HygieneStore::default();
    let context = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let rows = store.resolution_contexts(&[context]).unwrap();
    let names = store.name_contexts(&[context]).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let facts = || {
        HygieneFacts::new(
            runtime::TokenForest::decode(&wire)
                .unwrap()
                .contexts()
                .to_vec(),
            serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
        )
        .unwrap()
        .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
        .unwrap()
    };
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
        panic!("source missing");
    };
    let outline = view.outline().clone();
    let body = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let graph = SourceMacroRoutes::candidate_graph(&[SourceMacroRoutes::from_outline(
        &outline,
        runtime::Key(context.0),
        facts(),
        &BTreeMap::new(),
    )
    .unwrap()])
    .unwrap();
    let index = PublishedMacroIndex::from_view(view, runtime::Key(context.0), facts()).unwrap();
    let entered = index.enter_body(&selected, &body).unwrap();
    let definitions: Vec<_> = entered
        .facts()
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            lrr::syntax::entered_macros::BodyMacroEventKind::Definition {
                identity, name, ..
            } if name == "m" => Some(*identity),
            _ => None,
        })
        .collect();
    assert_eq!(definitions.len(), 3);
    let mut selections = Vec::new();
    for (needle, expected) in [("chosen!()", definitions[0]), ("m!()", definitions[1])] {
        for (at, _) in source.match_indices(needle) {
            let PathLookup::Defined(definition) = entered
                .lookup_invocation(&selected, &body, runtime_range(at, needle.len()), &graph)
                .unwrap()
            else {
                panic!("body textual import macro selection failed");
            };
            assert_eq!(definition.identity, expected);
            selections.push(definition.identity.hex());
        }
    }
    assert_eq!(selections.len(), 3);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"actual pinned hygiene and retained body textual import targets frozen at their declaration scope and position, later redefinition and nested textual shadow; actual native macOS execution; selected definition identity comparison, no guest VM claim","native":native,"selected":selections,"passed":true})).unwrap()).unwrap();
}
#[test]
fn nested_body_capture_matches_native_macos() {
    use lrr::frontend::{macro_names::PathLookup, source_macro_routes::SourceMacroRoutes};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-nested-body-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let source = "fn main(){macro_rules! m {()=>{17_u64};} use m as frozen; fn inner()->[u64;3]{let a=m!();let b=frozen!();fn leaf()->u64{frozen!()}[a,b,leaf()]} macro_rules! m {()=>{31_u64};} println!(\"{:?}\",inner());}";
    fs::write(work.join("native.rs"), source).unwrap();
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
    let mut store = HygieneStore::default();
    let context = store.root(Key([1; 32]), Edition::E2024).unwrap();
    let rows = store.resolution_contexts(&[context]).unwrap();
    let names = store.name_contexts(&[context]).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let facts = || {
        HygieneFacts::new(
            runtime::TokenForest::decode(&wire)
                .unwrap()
                .contexts()
                .to_vec(),
            serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
        )
        .unwrap()
        .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
        .unwrap()
    };
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
        panic!("source missing");
    };
    let outline = view.outline().clone();
    let body = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let graph = SourceMacroRoutes::candidate_graph(&[SourceMacroRoutes::from_outline(
        &outline,
        runtime::Key(context.0),
        facts(),
        &BTreeMap::new(),
    )
    .unwrap()])
    .unwrap();
    let index = PublishedMacroIndex::from_view(view, runtime::Key(context.0), facts()).unwrap();
    let entered = index.enter_body(&selected, &body).unwrap();
    let original = entered
        .facts()
        .events
        .iter()
        .find_map(|e| match &e.kind {
            lrr::syntax::entered_macros::BodyMacroEventKind::Definition {
                identity, name, ..
            } if name == "m" => Some(*identity),
            _ => None,
        })
        .unwrap();
    let key = entered
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
    let child = entered.enter_nested_body(&selected, &body, &key).unwrap();
    let mut selections = Vec::new();
    for call in &child.facts().invocations {
        let PathLookup::Defined(definition) = child
            .lookup_invocation(&selected, &key, call.range, &graph)
            .unwrap()
        else {
            panic!("nested source macro selection failed")
        };
        assert_eq!(definition.identity, original);
        selections.push(definition.identity.hex());
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
    let PathLookup::Defined(definition) = grandchild
        .lookup_invocation(
            &selected,
            &leaf,
            grandchild.facts().invocations[0].range,
            &graph,
        )
        .unwrap()
    else {
        panic!("grandchild source macro selection failed")
    };
    assert_eq!(definition.identity, original);
    selections.push(definition.identity.hex());
    assert_eq!(selections.len(), 3);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"actual pinned hygiene, explicit nested function entry, captured declaration-order macro frames and inherited frozen aliases through a grandchild; actual native macOS execution; selected definition identity comparison, no guest VM claim","native":native,"selected":selections,"passed":true})).unwrap()).unwrap();
}
fn runtime_range(at: usize, len: usize) -> lrr::syntax::spans::SourceRange {
    lrr::syntax::spans::SourceRange {
        start: u32::try_from(at).unwrap(),
        end: u32::try_from(at.checked_add(len).unwrap()).unwrap(),
    }
}
