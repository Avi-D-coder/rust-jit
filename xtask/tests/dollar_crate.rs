//! Native Cargo oracle; import/export routes come from parsed source. This
//! proves owned macro/item routing, not the future guest VM or Cargo collector.
use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::{
        key::{BucketKey, QueryKey},
        memo::Database,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    expand::scopes::MacroName,
    frontend::{
        hygiene::HygieneFacts, macro_names::PathLookup, source_macro_routes::SourceMacroRoutes,
    },
    source::{
        changes::{PreparedSourceInputs, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{outline::ScopeKey, parse::ParsedFile, token_wire as runtime},
};
use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite},
    hygiene::HygieneStore,
    token_wire::*,
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
fn name(text: &str) -> MacroName {
    MacroName::parse(text, 2024).unwrap()
}
fn helper(db: &mut Database, key: BucketKey) -> Digest {
    let fetched = db
        .fetch(&SyntaxProvider, QueryKey::ScopeBucket(key))
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::ScopeBucket(bucket) = fetched.value.as_ref() else {
        panic!("missing helper bucket")
    };
    assert!(bucket.blockers.is_empty());
    assert_eq!(bucket.candidates.len(), 1);
    bucket.candidates[0]
}
#[test]
fn dollar_crate_renamed_dependency_and_reexport_match_native_cargo() {
    oracle(false);
    oracle(true);
}
fn oracle(unicode: bool) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-dollar-crate-{}-{}",
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
    let provider_value = 17u64;
    let caller_value = 53u64;
    let facade_value = 67u64;
    let body = if unicode {
        "() => {$crate::café::r#gen()};"
    } else {
        "() => {$crate::helper()};"
    };
    let provider_source = format!(
        "pub fn helper()->u64 {{{provider_value}}} pub mod cafe\u{301} {{ pub fn r#gen()->u64 {{{provider_value}}} }} #[macro_export] macro_rules! exported {{{body}}}"
    );
    let facade_source = format!(
        "pub use inner_renamed::exported as forwarded; pub fn helper()->u64 {{{facade_value}}} pub mod café {{ pub fn r#gen()->u64 {{{facade_value}}} }}"
    );
    let caller_source = format!(
        "mod café {{ pub fn r#gen()->u64 {{{caller_value}}} }} fn helper()->u64 {{{caller_value}}} fn main() {{println!(\"[{{}},{{}},{{}}]\",renamed::exported!(),facade::forwarded!(),helper());}}"
    );
    fs::write(
        work.join("Cargo.toml"),
        "[workspace]\nmembers=[\"provider\",\"facade\",\"caller\"]\nresolver=\"3\"\n",
    )
    .unwrap();
    for (package, manifest, source, file) in [
        (
            "provider",
            "[package]\nname=\"origin_lib\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
            provider_source.as_str(),
            "lib.rs",
        ),
        (
            "facade",
            "[package]\nname=\"facade\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[dependencies]\ninner_renamed={package=\"origin_lib\",path=\"../provider\"}\n",
            facade_source.as_str(),
            "lib.rs",
        ),
        (
            "caller",
            "[package]\nname=\"caller\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[dependencies]\nrenamed={package=\"origin_lib\",path=\"../provider\"}\nfacade={path=\"../facade\"}\n",
            caller_source.as_str(),
            "main.rs",
        ),
    ] {
        fs::create_dir_all(work.join(package).join("src")).unwrap();
        fs::write(work.join(package).join("Cargo.toml"), manifest).unwrap();
        fs::write(work.join(package).join("src").join(file), source).unwrap();
    }
    let lock = Command::new("cargo")
        .args(["+1.99.0", "generate-lockfile", "--offline"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("lock.stderr"), &lock.stderr).unwrap();
    assert!(lock.status.success());
    let native = Command::new("cargo")
        .args(["+1.99.0", "run", "-p", "caller", "--locked", "--offline"])
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
    let provider = Digest([1; 32]);
    let caller = Digest([2; 32]);
    let facade = Digest([3; 32]);
    let catalogs = [
        source_routes(provider, &provider_source, BTreeMap::new()),
        source_routes(
            facade,
            &facade_source,
            BTreeMap::from([(name("inner_renamed"), provider)]),
        ),
        source_routes(
            caller,
            &caller_source,
            BTreeMap::from([(name("renamed"), provider), (name("facade"), facade)]),
        ),
    ];
    assert_eq!(catalogs[0].definitions().len(), 1);
    let definition = catalogs[0].definitions()[0].definition.clone();
    let mut namespaces = SourceMacroRoutes::candidate_graph(&catalogs).unwrap();
    namespaces
        .dependency(caller, name("renamed"), provider)
        .unwrap();
    namespaces
        .dependency(caller, name("facade"), facade)
        .unwrap();
    let mut db = Database::default();
    let inputs = [
        ("provider/src/lib.rs", provider, provider_source.as_str()),
        ("facade/src/lib.rs", facade, facade_source.as_str()),
        ("caller/src/main.rs", caller, caller_source.as_str()),
    ]
    .into_iter()
    .map(|(path, crate_key, source)| SourceUnitInput {
        path: SourcePath::new(path).unwrap(),
        crate_key,
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::default(),
        blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
    })
    .collect();
    let prepared = PreparedSourceInputs::refresh(None, inputs, BTreeMap::new()).unwrap();
    prepared.publish(&mut db).unwrap();
    let key = |unit| BucketKey {
        scope: ScopeKey {
            crate_key: unit,
            module_path: if unicode {
                vec!["café".into()]
            } else {
                vec![]
            },
            owner: None,
        },
        namespace: Namespace::Value,
        name: if unicode {
            "gen".into()
        } else {
            "helper".into()
        },
    };
    let provider_helper = helper(&mut db, key(provider));
    let caller_helper = helper(&mut db, key(caller));
    let facade_helper = helper(&mut db, key(facade));
    assert_ne!(provider_helper, caller_helper);
    assert_ne!(provider_helper, facade_helper);
    let caller_root = helper(
        &mut db,
        BucketKey {
            scope: ScopeKey {
                crate_key: caller,
                module_path: vec![],
                owner: None,
            },
            namespace: Namespace::Value,
            name: "helper".into(),
        },
    );
    let values = BTreeMap::from([
        (caller_root, caller_value),
        (provider_helper, provider_value),
        (caller_helper, caller_value),
        (facade_helper, facade_value),
    ]);
    let rules = DeclarativeMacro::new(fixture::definition(body, Edition::E2024)).unwrap();
    let input = fixture::input("");
    let mut observed = Vec::new();
    for (alias, member, expansion) in [("renamed", "exported", 8u8), ("facade", "forwarded", 9u8)] {
        let path = namespaces
            .dependency_path(caller, &name(alias), vec![name(member)])
            .unwrap();
        let PathLookup::Defined(selected) = namespaces.lookup(&path).unwrap() else {
            panic!("missing macro")
        };
        assert_eq!(selected, definition);
        let (output, stats) = rules
            .expand_hygienic_with_limits(
                &input,
                fixture::call_site(&input),
                DeclarativeSite {
                    expansion: Key([expansion; 32]),
                    defining_unit: Key(selected.defining_unit.0),
                    definition_edition: Edition::E2024,
                },
                100,
                4096,
            )
            .unwrap();
        let tokens = output.unwrap().tokens;
        assert_eq!(stats.admitted_bytes, tokens.encoded_len().unwrap());
        assert!(fixture::literal_texts(&tokens).is_empty());
        let mut stack = tokens.roots().iter().collect::<Vec<_>>();
        let mut dollar_span = None;
        while let Some(token) = stack.pop() {
            match token {
                TokenTree::Group { children, .. } => stack.extend(children),
                TokenTree::Ident { text, span, .. } if text == "$crate" => {
                    dollar_span = Some(*span)
                }
                _ => {}
            }
        }
        let emitting = tokens
            .spans()
            .iter()
            .find(|r| r.key() == dollar_span.unwrap())
            .unwrap()
            .context;
        let mut adapter = HygieneStore::default();
        adapter.import(tokens.contexts()).unwrap();
        let rows = adapter
            .resolution_contexts(
                &adapter
                    .records()
                    .iter()
                    .map(|r| r.key())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let contexts = runtime::TokenForest::decode(
            &TokenForest::new(vec![], adapter.records(), vec![])
                .unwrap()
                .encode()
                .unwrap(),
        )
        .unwrap();
        let facts = HygieneFacts::new(
            contexts.contexts().to_vec(),
            serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
        )
        .unwrap();
        let route = facts
            .dollar_crate_bucket(
                runtime::Key(emitting.0),
                &if unicode {
                    vec![name("café"), name("r#gen")]
                } else {
                    vec![name("helper")]
                },
                Namespace::Value,
            )
            .unwrap();
        assert_eq!(route.scope.crate_key, provider);
        observed.push(values[&helper(&mut db, route)]);
    }
    observed.push(values[&caller_root]);
    assert_eq!(observed, native);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","fixture":"MX011","unicode":unicode,"scope":"native Cargo plus actual pinned marked expansion, parsed original-source import/export routes, NFC source module/name bucket selection with raw r#gen; guest VM and Cargo source collector pending","observed":observed,"native":native,"passed":true})).unwrap()).unwrap();
}

fn source_routes(
    unit: Digest,
    source: &str,
    dependencies: BTreeMap<MacroName, Digest>,
) -> SourceMacroRoutes {
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key(unit.0),
        edition: Edition::E2024,
    };
    let context = root.key();
    let mut store = HygieneStore::default();
    store.import(&[root]).unwrap();
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
    let outline = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .outline(unit, vec![], &CfgSet::default())
    .unwrap();
    SourceMacroRoutes::from_outline(&outline, runtime::Key(context.0), facts, &dependencies)
        .unwrap()
}
