use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    expand::scopes::MacroName,
    frontend::{
        hygiene::HygieneFacts,
        macro_names::{MacroPath, PathLookup},
        source_macro_routes::SourceMacroRoutes,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{parse::ParsedFile, token_wire as runtime},
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
fn name(s: &str) -> MacroName {
    MacroName::parse(s, 2024).unwrap()
}
#[test]
fn parsed_module_alias_selection_matches_native_macos_and_original_token_expansion() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-module-alias-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let provider = Digest([8; 32]);
    let consumer = Digest([9; 32]);
    let p = "#[macro_export] macro_rules! other {()=>{31_u64};} #[macro_export] macro_rules! exported {()=>{17_u64};} pub mod tools {pub use crate::exported as member;}";
    let c = "use second as first; use third as second; use ::dep as third; use first::tools as renamed; pub use renamed::member as chosen; use crate as own; mod r#gen {pub use super::chosen as raw_member;} use own::r#gen as raw_module; use raw_module::raw_member as final_name; fn main(){println!(\"{:?}\",[chosen!(),r#gen::raw_member!(),final_name!(),first::tools::member!(),renamed::member!(),raw_module::raw_member!()]);}";
    fs::write(
        work.join("Cargo.toml"),
        "[workspace]\nmembers=[\"provider\",\"consumer\"]\nresolver=\"3\"\n",
    )
    .unwrap();
    for (dir, manifest, source, file) in [
        (
            "provider",
            "[package]\nname=\"origin\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
            p,
            "lib.rs",
        ),
        (
            "consumer",
            "[package]\nname=\"consumer\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[dependencies]\ndep={package=\"origin\",path=\"../provider\"}\n",
            c,
            "main.rs",
        ),
    ] {
        fs::create_dir_all(work.join(dir).join("src")).unwrap();
        fs::write(work.join(dir).join("Cargo.toml"), manifest).unwrap();
        fs::write(work.join(dir).join("src").join(file), source).unwrap();
    }
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let lock = Command::new("cargo")
        .args(["+1.99.0", "generate-lockfile", "--offline"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("lock.stderr"), &lock.stderr).unwrap();
    assert!(lock.status.success());
    let native = Command::new("cargo")
        .args(["+1.99.0", "run", "-p", "consumer", "--locked", "--offline"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("native.stdout"), &native.stdout).unwrap();
    fs::write(work.join("native.stderr"), &native.stderr).unwrap();
    assert!(
        native.status.success(),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    let native: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let mut catalogs = Vec::new();
    let mut outlines = Vec::new();
    let mut forests = Vec::new();
    for (unit, source, path, deps) in [
        (provider, p, "provider/src/lib.rs", BTreeMap::new()),
        (
            consumer,
            c,
            "consumer/src/main.rs",
            BTreeMap::from([(name("dep"), provider)]),
        ),
    ] {
        let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
        let outline = ParsedFile::parse(&blob, ProfileIdentity::current())
            .unwrap()
            .outline(unit, vec![], &CfgSet::default())
            .unwrap();
        let mut store = HygieneStore::default();
        let ctx = store.root(Key(unit.0), Edition::E2024).unwrap();
        let rows = store.resolution_contexts(&[ctx]).unwrap();
        let names = store.name_contexts(&[ctx]).unwrap();
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
        catalogs.push(
            SourceMacroRoutes::from_outline(&outline, runtime::Key(ctx.0), facts, &deps).unwrap(),
        );
        forests.push(
            parse_source(
                source,
                SourceIdentity {
                    unit: Key(unit.0),
                    content: Key(blob.content().0),
                    path: path.into(),
                },
                ContextRecord {
                    parent: None,
                    expansion: None,
                    transparency: Transparency::Opaque,
                    defining_unit: Key(unit.0),
                    edition: Edition::E2024,
                },
                Key([2; 32]),
            )
            .unwrap(),
        );
        outlines.push(outline);
    }
    let graph = SourceMacroRoutes::candidate_graph(&catalogs).unwrap();
    let mut observed = Vec::new();
    let mut selected = Vec::new();
    for names in [
        vec![name("chosen")],
        vec![name("r#gen"), name("raw_member")],
        vec![name("final_name")],
        vec![name("first"), name("tools"), name("member")],
        vec![name("renamed"), name("member")],
        vec![name("raw_module"), name("raw_member")],
    ] {
        let PathLookup::Defined(def) = graph
            .lookup_from(
                &MacroPath::new(consumer, names).unwrap(),
                &lrr::syntax::outline::ScopeKey {
                    crate_key: consumer,
                    module_path: vec![],
                    owner: None,
                },
            )
            .unwrap()
        else {
            panic!("missing selected macro")
        };
        let item = outlines[0]
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == def.identity)
            .unwrap();
        assert_eq!(def.defining_unit, provider);
        let forest = &forests[0];
        let spans: BTreeMap<_, _> = forest.spans().iter().map(|r| (r.key(), r)).collect();
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
                    && spans[open].start >= item.declaration_span.start
                    && spans[close].end <= item.declaration_span.end
                {
                    body = Some(tree.clone());
                    break;
                }
                stack.extend(children.iter().rev());
            }
        }
        let rules = DeclarativeMacro::new(
            TokenForest::new(
                vec![body.unwrap()],
                forest.contexts().to_vec(),
                forest.spans().to_vec(),
            )
            .unwrap(),
        )
        .unwrap();
        let input = fixture::input("");
        let output = rules
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
        selected.push(def.identity);
    }
    assert_eq!(observed, native);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"parsed module alias fixed point, actual pinned hygiene, selected expansion from full original source token forest preserving unit/content/path/offsets; no guest VM or visibility seal claim","selected":selected,"observed":observed,"native":native,"passed":true})).unwrap()).unwrap();
}
