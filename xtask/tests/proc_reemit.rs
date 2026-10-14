//! M015 source-origin component controls; no stock compiletest or guest-tier claim.
//! The native controls require explicit macro-only preparation and the matched server.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
    },
    expand::{
        cache::ExpansionOutputCache,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    service::{
        expansion::{DemandContext, PhaseIdentity},
        run::MacroRun,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::BodyKey,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{outline::AttributeClass, parse::ParseKey},
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
#[allow(dead_code)]
#[path = "support/macro_service.rs"]
mod service_fixture;

const UPSTREAM: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/test-re-emit.rs");
const AUXILIARY: &[u8] = include_bytes!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/test-re-emit.rs"
);
// Adapt only the package alias outside the licensed directory. The official
// consumer bytes remain an unchanged suffix in an original Rust2024 source unit.
const ALIAS: &str = "extern crate lrr_fixture_macros as test_re_emit;\n";
const WRONG_ORIGIN: &str = "use lrr_fixture_macros::attribute_tokens;\n#[test] #[attribute_tokens()] fn meow1() {}\nfn meow1() {}\n";
const MOVED_OUTPUT: &str = "mod moved { pub fn meow1() {} }";
const WRONG_SCOPE: &str = "use lrr_fixture_macros::attribute_tokens;\n#[test] #[attribute_tokens(mod moved { pub fn meow1() {} })] fn meow1() {}\n";
const QUALIFIED_UPSTREAM: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/issue-53675-a-test-called-panic.rs"
);
const NESTED_UPSTREAM: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/test-followed-by-attr-macros.rs"
);
const NESTED_AUXILIARY: &[u8] = include_bytes!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/test-macros.rs"
);
const ROOT_ALIAS_UPSTREAM: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/rust-2018/extern-crate-alias-shadowed-by-module-55759.rs"
);
const QUALIFIED_NAMES: [&str; 4] = [
    "in_expression_position::panic",
    "in_statement_position::panic",
    "what_if_we_use_panic_directly_in_expr::panic",
    "what_if_we_use_panic_directly_in_stmt::panic",
];
static EVIDENCE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum Case {
    Reemit,
    WrongOrigin,
    WrongScope,
    Qualified,
    NestedRetained,
}

fn stable_procedural_branch() -> &'static str {
    // The unchanged stock case also exercises unstable macro_attr. Retain only
    // its exact stable procedural module as a disclosed component adaptation.
    let start = NESTED_UPSTREAM.find("mod proc_macro_attr {").unwrap();
    let end = NESTED_UPSTREAM.find("mod macro_rules_attr {").unwrap();
    &NESTED_UPSTREAM[start..end]
}

fn qualified_source() -> String {
    // Original test bytes are an unchanged argument payload. The existing
    // explicitly prepared attribute emits them without compiling the consumer.
    format!(
        "#[lrr_fixture_macros::attribute_tokens(\n{QUALIFIED_UPSTREAM}\n)] fn placeholder() {{}}\n"
    )
}

fn nested_source() -> String {
    // compiletest names the dependency test_macros in the extern prelude. Our
    // existing public preparation helper instead fixes lrr_fixture_macros.
    // Explicitly address its root alias, as in the unchanged edition2018
    // extern-crate-alias-shadowed-by-module witness, without changing fixtures.
    format!(
        "extern crate lrr_fixture_macros as test_macros;\n{}",
        stable_procedural_branch().replace(
            "#[test_macros::identity_attr]",
            "#[crate::test_macros::identity_attr]"
        )
    )
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
fn evidence() -> PathBuf {
    let base = root().join("artifacts/macos-first");
    fs::create_dir_all(&base).unwrap();
    loop {
        let id = EVIDENCE_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("evidence counter exhausted");
        let path = base.join(format!("m015-proc-reemit-{}-{id}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return fs::canonicalize(path).unwrap(),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("exclusive evidence directory: {error}"),
        }
    }
}
fn test_cfg() -> CfgSet {
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    cfg
}
fn outline(text: &str, unit: Digest) -> lrr::syntax::outline::Outline {
    lrr::syntax::parse::ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .outline(unit, vec![], &test_cfg())
    .unwrap()
}

fn test_markers(forest: &lrr_expand_adapter::token_wire::TokenForest) -> usize {
    use lrr_expand_adapter::token_wire::{Delimiter, TokenTree};
    fn count(trees: &[TokenTree]) -> usize {
        let here = trees.windows(2).filter(|pair| matches!(&pair[0], TokenTree::Punct { scalar: '#', .. }) && matches!(&pair[1], TokenTree::Group { delimiter: Delimiter::Bracket, children, .. } if matches!(children.as_slice(), [TokenTree::Ident { text, .. }] if text == "test"))).count();
        here + trees
            .iter()
            .map(|tree| match tree {
                TokenTree::Group { children, .. } => count(children),
                _ => 0,
            })
            .sum::<usize>()
    }
    count(forest.roots())
}

#[test]
fn fixture_reemit_source_and_wrong_origin_witness_are_pure() {
    assert_eq!(
        UPSTREAM
            .lines()
            .filter(|s| s.starts_with("//@"))
            .collect::<Vec<_>>(),
        vec![
            "//@ check-pass",
            "//@ proc-macro: test-re-emit.rs",
            "//@ compile-flags: --test"
        ]
    );
    assert!(
        std::str::from_utf8(AUXILIARY)
            .unwrap()
            .contains("item.to_string().parse().unwrap()")
    );
    let adapted = format!("{ALIAS}{UPSTREAM}");
    assert!(adapted.ends_with(UPSTREAM));
    let parsed = outline(&adapted, Digest([221; 32]));
    let functions: Vec<_> = parsed
        .items
        .iter()
        .filter(|i| i.def_key.declared_name.starts_with("meow"))
        .collect();
    assert_eq!(functions.len(), 2);
    assert_eq!(
        functions[0]
            .header
            .attributes
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        vec!["test", "test_re_emit::remove_span"]
    );
    assert_eq!(
        functions[1]
            .header
            .attributes
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        vec!["test_re_emit::remove_span", "test"]
    );
    let decoys = outline(WRONG_ORIGIN, Digest([221; 32]));
    let functions: Vec<_> = decoys
        .items
        .iter()
        .filter(|i| i.def_key.declared_name == "meow1")
        .collect();
    assert_eq!(functions.len(), 2);
    assert_eq!(functions[0].scope, functions[1].scope);
    assert_ne!(
        functions[0].def_key.fingerprint(),
        functions[1].def_key.fingerprint()
    );
    assert_ne!(
        functions[0].def_key.duplicate_ordinal,
        functions[1].def_key.duplicate_ordinal
    );
    assert!(
        functions[0]
            .header
            .attributes
            .iter()
            .any(|a| a.class == AttributeClass::Test)
    );
    assert!(functions[1].header.attributes.is_empty());
    // Exercise the observer on original structured tokens without executing a macro.
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, Transparency,
    };
    let forest = |text: &str| {
        token_wire::parse_source(
            text,
            SourceIdentity {
                unit: Key([221; 32]),
                content: Key(Digest::of_bytes(text.as_bytes()).0),
                path: "pure-reemit.rs".into(),
            },
            ContextRecord {
                parent: None,
                expansion: None,
                transparency: Transparency::Opaque,
                defining_unit: Key([221; 32]),
                edition: Edition::E2024,
            },
            Key([228; 32]),
        )
        .unwrap()
    };
    assert_eq!(test_markers(&forest(UPSTREAM)), 2);
    assert_eq!(test_markers(&forest(WRONG_ORIGIN)), 1);
    assert_eq!(test_markers(&forest("fn meow2() {}")), 0);
}

#[test]
fn rustc_reemit_preserves_consumed_test_registration_after_actual_span_loss() {
    component(Case::Reemit);
}
#[test]
fn consumed_test_same_name_unrelated_survivor_cannot_claim_registration() {
    component(Case::WrongOrigin);
}
#[test]
fn wrong_module_scope_witness_keeps_the_same_name_and_root_authority() {
    let unit = Digest([229; 32]);
    assert!(WRONG_SCOPE.contains(MOVED_OUTPUT));
    let input = outline(WRONG_SCOPE, unit);
    let original = input
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "meow1")
        .unwrap();
    assert!(
        original
            .header
            .attributes
            .iter()
            .any(|a| a.name == "test" && a.class == AttributeClass::Test)
    );
    assert!(original.scope.module_path.is_empty());
    let output = outline(MOVED_OUTPUT, unit);
    let function = output
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "meow1")
        .unwrap();
    assert_eq!(
        function.def_key.declared_name,
        original.def_key.declared_name
    );
    assert_eq!(function.scope.crate_key, original.scope.crate_key);
    assert_eq!(function.scope.owner, original.scope.owner);
    assert_eq!(function.scope.module_path, vec!["moved"]);
    assert_ne!(function.scope.module_path, original.scope.module_path);
    assert!(function.header.attributes.is_empty());
}
#[test]
fn consumed_test_same_producer_and_name_in_wrong_module_cannot_claim_registration() {
    component(Case::WrongScope);
}

#[test]
fn qualified_catalog_source_witness_has_distinct_scopes_and_final_flags() {
    assert_eq!(
        QUALIFIED_UPSTREAM
            .lines()
            .filter(|s| s.starts_with("//@"))
            .collect::<Vec<_>>(),
        vec!["//@ check-pass", "//@ compile-flags: --test"]
    );
    assert!(qualified_source().contains(QUALIFIED_UPSTREAM));
    let parsed = outline(QUALIFIED_UPSTREAM, Digest([230; 32]));
    let functions: Vec<_> = parsed
        .items
        .iter()
        .filter(|i| i.def_key.declared_name == "panic")
        .collect();
    assert_eq!(functions.len(), 4);
    for (index, function) in functions.iter().enumerate() {
        assert_eq!(
            format!("{}::panic", function.scope.module_path.join("::")),
            QUALIFIED_NAMES[index]
        );
        assert!(function.scope.owner.is_none());
        assert_eq!(
            function
                .header
                .attributes
                .iter()
                .any(|a| a.name == "should_panic"),
            index >= 2
        );
        assert!(
            !function
                .header
                .attributes
                .iter()
                .any(|a| a.name == "ignore")
        );
    }
    assert_eq!(
        functions
            .iter()
            .map(|f| f.def_key.fingerprint())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
    assert!(NESTED_UPSTREAM.contains("#![feature(macro_attr)]"));
    assert!(NESTED_UPSTREAM.contains(stable_procedural_branch()));
    assert!(ROOT_ALIAS_UPSTREAM.contains("extern crate time as std_time;"));
    assert!(ROOT_ALIAS_UPSTREAM.contains("crate::std_time::now()"));
    assert!(nested_source().contains("#[crate::test_macros::identity_attr]"));
    let branch = outline(stable_procedural_branch(), Digest([230; 32]));
    let function = branch
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "test")
        .unwrap();
    assert_eq!(function.scope.module_path, ["proc_macro_attr"]);
    assert_eq!(
        function
            .header
            .attributes
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        ["test", "test_macros::identity_attr"]
    );
    assert!(std::str::from_utf8(NESTED_AUXILIARY).unwrap().contains(
        "pub fn identity_attr(_: TokenStream, input: TokenStream) -> TokenStream {\n    input\n}"
    ));
}

#[test]
fn generated_qualified_same_leaf_tests_keep_identities_flags_and_filters() {
    component(Case::Qualified);
}

#[test]
fn retained_registration_in_original_module_keeps_qualified_identity() {
    component(Case::NestedRetained);
}

fn component(case: Case) {
    let work = evidence();
    let (macro_source, source) = match case {
        Case::Reemit => {
            let path = work.join("macro");
            fs::create_dir_all(path.join("src")).unwrap();
            fs::write(path.join("src/lib.rs"), AUXILIARY).unwrap();
            fs::write(path.join("Cargo.toml"), "[package]\nname=\"lrr_fixture_macros\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
            (path, format!("{ALIAS}{UPSTREAM}"))
        }
        Case::WrongOrigin => (
            root().join("fixtures/macros/session-workspace"),
            WRONG_ORIGIN.to_owned(),
        ),
        Case::WrongScope => (
            root().join("fixtures/macros/session-workspace"),
            WRONG_SCOPE.to_owned(),
        ),
        Case::Qualified => (
            root().join("fixtures/macros/session-workspace"),
            qualified_source(),
        ),
        Case::NestedRetained => {
            let path = work.join("macro");
            fs::create_dir_all(path.join("src")).unwrap();
            fs::write(path.join("src/lib.rs"), NESTED_AUXILIARY).unwrap();
            fs::write(path.join("Cargo.toml"), "[package]\nname=\"lrr_fixture_macros\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
            (path, nested_source())
        }
    };
    let manifest = service_fixture::prepare_sources(
        &work.join("prepared"),
        &macro_source,
        &[("lib.rs", &source)],
    );
    let plan = lrr::project::tool_manifest::InputPlan::new(
        [work.join("prepared/app"), macro_source]
            .into_iter()
            .map(|p| {
                lrr::project::tool_manifest::InputRoot::open(
                    lrr::project::tool_manifest::InputKind::Source,
                    &p,
                    Default::default(),
                    Default::default(),
                    true,
                    Default::default(),
                )
                .unwrap()
            })
            .collect(),
        Default::default(),
    )
    .unwrap();
    let unit = *manifest.graph.roots.first().unwrap();
    let profile = ProfileIdentity::current();
    let cfg = test_cfg();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("main.rs").unwrap();
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let environment =
        lrr::expand::builtins::FrozenCompileEnvironment::new(unit, &manifest.compile_env[&unit])
            .unwrap();
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap()
    .with_compile_environment(Arc::new(environment))
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("reemission source absent")
    };
    let originals: Vec<_> = view
        .outline()
        .items
        .iter()
        .filter(|i| {
            i.def_key.declared_name.starts_with("meow")
                || i.header
                    .attributes
                    .iter()
                    .any(|a| a.class == AttributeClass::Test)
        })
        .map(|i| i.def_key.fingerprint())
        .collect();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let context = DemandContext {
        request: Digest([222; 32]),
        session: Digest([223; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut denied = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, false).unwrap();
    assert_eq!(
        denied.catalog(&mut db, unit).unwrap_err().code,
        "LRR-E-MACRO-PERMISSION"
    );
    assert_eq!(denied.unit_invocations(unit), 0);
    let mut run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
    let catalog = run.catalog(&mut db, unit).unwrap();
    let compiler = Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &ProfileIdentity::current(),
        )
        .unwrap(),
    );
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_declarative_procedural(
        context.compile_environment,
        Digest([224; 32]),
        catalog.clone(),
    )
    .unwrap()
    .with_compiler(compiler)
    .unwrap()
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap();
    assert!(discovery.test_catalog(&mut db).is_err());
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut observations = BTreeMap::new();
    let mut sealed = false;
    for index in 0..16 {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedAttributeSelection { selection } => {
                let request = run
                    .prepare_attribute_selection(&mut db, &selection)
                    .unwrap();
                let invocation = request.request();
                let materialized =
                    lrr_expand_adapter::item_output::materialize(&invocation.input).unwrap();
                let function = materialized
                    .items
                    .iter()
                    .find(|i| i.kind == lrr_expand_adapter::item_output::ItemKind::Function)
                    .unwrap();
                let name = function.name.as_ref().unwrap();
                // Read actual token boundaries, never reparsed display text.
                let markers = test_markers(&invocation.input);
                let original_name_span = name.span;
                if matches!(case, Case::Reemit | Case::NestedRetained) {
                    assert_eq!(
                        markers,
                        usize::from(matches!(case, Case::Reemit) && name.text == "meow2")
                    );
                    let definition = invocation
                        .input
                        .spans()
                        .iter()
                        .find(|s| s.key() == invocation.definition_site)
                        .unwrap();
                    let call = invocation
                        .input
                        .spans()
                        .iter()
                        .find(|s| s.key() == invocation.call_site)
                        .unwrap();
                    assert_eq!(
                        definition.edition,
                        lrr_expand_adapter::token_wire::Edition::E2015
                    );
                    assert_eq!(call.edition, lrr_expand_adapter::token_wire::Edition::E2024);
                    assert_eq!(call.source.unit.0, unit.0);
                } else {
                    assert_eq!(markers, 0);
                }
                assert!(
                    observations
                        .insert(
                            name.text.clone(),
                            (request.key().fingerprint(), original_name_span)
                        )
                        .is_none()
                );
                discovery.bind_attribute_request(request.clone()).unwrap();
                let SourceDiscoveryStep::NeedExpansion { request: bound } =
                    discovery.poll().unwrap()
                else {
                    panic!("bound reemission request absent")
                };
                assert_eq!(bound.key(), request.key());
                let phase = PhaseIdentity {
                    frame: Digest([225; 32]),
                    body: BodyKey {
                        definition: Digest([226; 32]),
                        exact_body_bytes: Digest([227; 32]),
                    },
                    index,
                };
                run.fulfil_discovery(&mut db, &mut discovery, phase, &request, &mut cache)
                    .unwrap();
            }
            SourceDiscoveryStep::Sealed(_) => {
                sealed = true;
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                fs::write(
                    work.join("discovery-failure.json"),
                    serde_json::to_vec_pretty(&serde_json::json!({
                        "diagnostic_code": diagnostic.code,
                        "diagnostic_message": diagnostic.message,
                        "blocked": format!("{blocked:?}"),
                        "source": source,
                        "source_content": format!("{:?}", view.selection().outline.parse.content),
                        "source_path": view.selection().path.as_str(),
                        "root_unit": format!("{unit:?}"),
                        "source_revision": db.source_revision(),
                        "macro_invocations": run.unit_invocations(unit),
                        "actual_bound_selections": observations.len(),
                        "published_positive_catalog_assertions_reached": false,
                        "stock_full_fixture_passes": 0
                    }))
                    .unwrap(),
                )
                .unwrap();
                panic!("reemission unresolved: {diagnostic:?}, {blocked:?}")
            }
            _ => panic!("unexpected reemission discovery demand"),
        }
    }
    assert!(sealed, "bounded reemission traversal did not seal");
    let expected_invocations = if matches!(case, Case::Reemit) { 2 } else { 1 };
    assert_eq!(run.unit_invocations(unit), expected_invocations);
    assert_eq!(observations.len() as u64, expected_invocations);
    let records: Vec<_> = discovery.sealed_expansions().unwrap().cloned().collect();
    assert_eq!(records.len() as u64, expected_invocations);
    if matches!(case, Case::Reemit) {
        for record in &records {
            let forest = lrr_expand_adapter::token_wire::TokenForest::decode(
                &record.tokens().unwrap().encode().unwrap(),
            )
            .unwrap();
            let items = lrr_expand_adapter::item_output::materialize(&forest).unwrap();
            let function = items
                .items
                .iter()
                .find(|i| i.kind == lrr_expand_adapter::item_output::ItemKind::Function)
                .unwrap();
            let name = function.name.as_ref().unwrap();
            let (origin, input_span) = observations[&name.text];
            assert_eq!(record.key().fingerprint(), origin);
            assert_ne!(
                name.span, input_span,
                "unchanged auxiliary must actually erase the original name span"
            );
            let span = forest
                .spans()
                .iter()
                .find(|s| s.key() == name.span)
                .unwrap();
            assert_eq!(span.source.unit.0, unit.0);
            assert_eq!(
                span.source.content.0,
                view.selection().outline.parse.content.0
            );
            assert_eq!(span.source.path, view.selection().path.as_str());
            let context = forest
                .contexts()
                .iter()
                .find(|c| c.key() == span.context)
                .unwrap();
            assert_eq!(context.defining_unit.0, unit.0);
        }
    } else if matches!(case, Case::WrongOrigin) {
        let forest = lrr_expand_adapter::token_wire::TokenForest::decode(
            &records[0].tokens().unwrap().encode().unwrap(),
        )
        .unwrap();
        assert!(
            lrr_expand_adapter::item_output::materialize(&forest)
                .unwrap()
                .items
                .is_empty()
        );
    }
    run.publish_discovery(&mut db, &mut discovery).unwrap();
    match case {
        Case::Qualified | Case::NestedRetained => {
            let nested = matches!(case, Case::NestedRetained);
            assert_eq!(originals.len(), usize::from(nested));
            let record = records[0].clone();
            assert_eq!(
                record.key().fingerprint(),
                observations[if nested { "test" } else { "placeholder" }].0
            );
            assert_qualified_catalog(&mut discovery, &mut db, &record, case, &view, context.epoch);
        }
        Case::Reemit => {
            let catalog = discovery.test_catalog(&mut db).unwrap();
            assert_eq!(
                catalog
                    .cases()
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["meow1", "meow2"]
            );
            assert_ne!(catalog.cases()[0].definition, catalog.cases()[1].definition);
            for case in catalog.cases() {
                assert!(
                    matches!(discovery.lookup(&case.name).unwrap(), lrr::frontend::names::NameLookup::Defined(actual) if actual.identity == case.definition)
                );
                assert!(!originals.contains(&case.definition));
                assert_eq!(case.expansion_ancestry, vec![observations[&case.name].0]);
                assert!(!case.ignored && !case.should_panic && !case.unsupported_signature);
                assert!(case.panic_contains.is_none() && case.attribute_error.is_none());
            }
            for _ in 0..3 {
                catalog.require_current(&mut db).unwrap();
                let again = discovery.test_catalog(&mut db).unwrap();
                assert_eq!(
                    again
                        .cases()
                        .iter()
                        .map(|c| c.definition)
                        .collect::<Vec<_>>(),
                    catalog
                        .cases()
                        .iter()
                        .map(|c| c.definition)
                        .collect::<Vec<_>>()
                );
            }
        }
        Case::WrongOrigin => {
            assert_eq!(originals.len(), 2);
            assert!(
                matches!(discovery.lookup("meow1").unwrap(), lrr::frontend::names::NameLookup::Defined(actual) if actual.identity == originals[1])
            );
            for _ in 0..3 {
                let error = match discovery.test_catalog(&mut db) {
                    Ok(_) => panic!(
                        "unrelated same-name survivor claimed registration or discovery returned zero-tests success"
                    ),
                    Err(error) => error,
                };
                assert_eq!(error.code, "LRR-E-TEST-DISCOVERY");
                assert!(error.message.contains("removed or renamed"));
            }
        }
        Case::WrongScope => {
            use lrr::db::{
                dependency::{InputKey, InputValue},
                key::{BucketKey, SealedNameKey},
                poll::QueryPoll,
                provider::SyntaxProvider,
                value::QueryValue,
            };
            use lrr::expand::generated_items::{GeneratedHeaderKind, GeneratedItems};
            assert_eq!(originals.len(), 1);
            let record = discovery
                .sealed_expansions()
                .unwrap()
                .next()
                .unwrap()
                .clone();
            assert_eq!(
                record.key().fingerprint(),
                observations["meow1"].0,
                "the moved output must belong to the original registered function's exact producer"
            );
            let items = GeneratedItems::admit(record.key(), record.clone(), context.epoch).unwrap();
            let function = items
                .items()
                .iter()
                .find(|i| {
                    i.kind == GeneratedHeaderKind::Function
                        && i.name.as_ref().is_some_and(|n| n.text == "meow1")
                })
                .unwrap();
            let identity = function.key.fingerprint();
            assert_ne!(identity, originals[0]);
            let retained = db
                .input(&InputKey::DeclarationContext(identity))
                .expect("moved declaration must actually be published");
            let InputValue::DeclarationContext(published) = retained.as_ref() else {
                panic!("moved declaration context differs")
            };
            let original = view
                .outline()
                .items
                .iter()
                .find(|i| i.def_key.fingerprint() == originals[0])
                .unwrap();
            assert_eq!(
                function.name.as_ref().unwrap().text,
                original.def_key.declared_name
            );
            assert_eq!(published.scope.crate_key, original.scope.crate_key);
            assert_eq!(published.scope.owner, original.scope.owner);
            assert_eq!(published.scope.module_path, vec!["moved"]);
            assert_ne!(published.scope.module_path, original.scope.module_path);
            // Bind positive declaration evidence through the original public
            // generated syntax inputs. A TestCatalog certificate is not a
            // scoped-name certificate and cannot authorize child name lookup.
            let source_input = db
                .input(&InputKey::DefinitionSource(identity))
                .expect("moved definition source must be published");
            let InputValue::DefinitionSource(source_key) = source_input.as_ref() else {
                panic!("moved definition source variant differs")
            };
            let QueryPoll::Ready(Ok(value)) =
                db.fetch(&SyntaxProvider, QueryKey::ParseFile(source_key.clone()))
            else {
                panic!("published generated declaration syntax must be ready")
            };
            let QueryValue::ParseFile(parsed) = value.value.as_ref() else {
                panic!("generated parse query variant differs")
            };
            assert_eq!(&parsed.key(), source_key);
            let output_outline = parsed.outline(unit, vec![], &test_cfg()).unwrap();
            let output_function = output_outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "meow1")
                .unwrap();
            assert_eq!(published.scope, output_function.scope);
            assert_eq!(published.range, output_function.declaration_span);
            assert_eq!(
                published.range,
                lrr::syntax::spans::SourceRange {
                    start: function.start,
                    end: function.end
                }
            );
            assert!(
                matches!(db.input(&InputKey::DefinitionSource(identity)).as_deref(), Some(InputValue::DefinitionSource(actual)) if *actual == parsed.key())
            );
            assert!(
                matches!(db.input(&InputKey::DefinitionHeader(identity)).as_deref(), Some(InputValue::DefinitionHeader(actual)) if *actual == output_function.header)
            );
            assert!(output_function.header.attributes.is_empty());
            let key = QueryKey::SealedName(Box::new(SealedNameKey {
                seal: discovery.key().clone(),
                bucket: BucketKey {
                    scope: published.scope.clone(),
                    namespace: lrr::source::manifest::Namespace::Value,
                    name: "meow1".into(),
                },
            }));
            let QueryPoll::Ready(Err(error)) = db.fetch(&SyntaxProvider, key) else {
                panic!("test-catalog certificate must refuse scoped-name query")
            };
            assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
            assert_eq!(error.message, "sealed name query domain/owner differs");
            assert!(matches!(
                discovery.lookup("meow1").unwrap(),
                lrr::frontend::names::NameLookup::Absent
            ));
            for _ in 0..3 {
                let error = match discovery.test_catalog(&mut db) {
                    Ok(_) => panic!(
                        "same producer/name in a different module claimed registration or returned zero-tests success"
                    ),
                    Err(error) => error,
                };
                assert_eq!(error.code, "LRR-E-TEST-DISCOVERY");
                assert!(error.message.contains("removed or renamed"));
            }
        }
    }
    assert_eq!(run.unit_invocations(unit), expected_invocations);
    assert!(db.trace.events.iter().all(|e| !matches!(
        e.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
    fs::write(work.join("component-report.json"), serde_json::to_vec_pretty(&serde_json::json!({"upstream_commit":"ae0fe511b15ab760d7e963b13389d1bc2ab42b18", "case":match case { Case::Reemit => "unchanged upstream source plus alias adaptation", Case::WrongOrigin => "LRR same-name wrong-origin supplement", Case::WrongScope => "LRR same-producer same-name wrong-module-scope supplement", Case::Qualified => "unchanged issue-53675 source emitted by attribute_tokens; fixed Rust2024 adaptation", Case::NestedRetained => "stable procedural branch of test-followed-by-attr-macros plus explicit crate:: root-alias path adaptation; unchanged auxiliary2015; unstable branch unexecuted" }, "macro_invocations":expected_invocations,"sealed":sealed,"body_queries":0,"stock_full_fixture_passes":0,"guest_execution":false})).unwrap()).unwrap();
}

fn assert_qualified_catalog(
    discovery: &mut SourceDiscovery,
    db: &mut Database,
    record: &Arc<lrr::expand::record::ExpansionRecord>,
    variant: Case,
    view: &lrr::source::changes::PreparedSourceView,
    epoch: u64,
) {
    use lrr::{
        db::{
            dependency::{InputKey, InputValue},
            poll::QueryPoll,
            provider::SyntaxProvider,
            value::QueryValue,
        },
        expand::generated_items::{GeneratedHeaderKind, GeneratedItems},
    };
    let nested = matches!(variant, Case::NestedRetained);
    let names: &[&str] = if nested {
        &["proc_macro_attr::test"]
    } else {
        &QUALIFIED_NAMES
    };
    let expected_panic: &[bool] = if nested {
        &[false]
    } else {
        &[false, false, true, true]
    };
    let unit = view.selection().outline.crate_key;
    let admitted = GeneratedItems::admit(record.key(), record.clone(), epoch).unwrap();
    let functions: Vec<_> = admitted
        .items()
        .iter()
        .filter(|header| header.kind == GeneratedHeaderKind::Function)
        .collect();
    assert_eq!(functions.len(), names.len());
    let catalog = discovery.test_catalog(db).unwrap();
    assert_eq!(
        catalog
            .cases()
            .iter()
            .map(|case| case.name.as_str())
            .collect::<Vec<_>>(),
        names
    );
    assert_eq!(
        catalog
            .cases()
            .iter()
            .map(|case| case.definition)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        names.len()
    );
    for (index, case) in catalog.cases().iter().enumerate() {
        let (module, leaf) = names[index].rsplit_once("::").unwrap();
        assert_eq!(case.expansion_ancestry, vec![record.key().fingerprint()]);
        assert_eq!(case.should_panic, expected_panic[index]);
        assert!(!case.ignored && !case.unsupported_signature);
        assert!(case.panic_contains.is_none() && case.attribute_error.is_none());
        let header = functions
            .iter()
            .find(|header| header.key.fingerprint() == case.definition)
            .expect("catalog identity must belong to an actually admitted output function");
        assert_eq!(header.name.as_ref().unwrap().text, leaf);
        let context = db
            .input(&InputKey::DeclarationContext(case.definition))
            .unwrap();
        let InputValue::DeclarationContext(published) = context.as_ref() else {
            panic!("qualified declaration context variant differs")
        };
        assert_eq!(published.scope.crate_key, unit);
        assert!(published.scope.owner.is_none());
        assert_eq!(published.scope.module_path, [module]);
        assert_eq!(
            published.range,
            lrr::syntax::spans::SourceRange {
                start: header.start,
                end: header.end
            }
        );
        let source = db
            .input(&InputKey::DefinitionSource(case.definition))
            .unwrap();
        let InputValue::DefinitionSource(source_key) = source.as_ref() else {
            panic!("qualified definition source variant differs")
        };
        let QueryPoll::Ready(Ok(value)) =
            db.fetch(&SyntaxProvider, QueryKey::ParseFile(source_key.clone()))
        else {
            panic!("actual published syntax must be ready")
        };
        let QueryValue::ParseFile(parsed) = value.value.as_ref() else {
            panic!("qualified parse query variant differs")
        };
        assert_eq!(&parsed.key(), source_key);
        // A retained replacement is inserted into its proven original module;
        // a whole generated payload retains the modules in its actual syntax.
        let insertion = if nested {
            let original = view
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "test")
                .unwrap();
            assert_eq!(published.scope, original.scope);
            assert_ne!(case.definition, original.def_key.fingerprint());
            original.scope.module_path.clone()
        } else {
            vec![]
        };
        let parsed_outline = parsed.outline(unit, insertion, &test_cfg()).unwrap();
        let output = parsed_outline
            .items
            .iter()
            .find(|item| {
                item.def_key.declared_name == leaf
                    && item.scope == published.scope
                    && item.declaration_span == published.range
            })
            .expect("published scope/range must select its exact actual generated function");
        assert!(
            matches!(db.input(&InputKey::DefinitionHeader(case.definition)).as_deref(), Some(InputValue::DefinitionHeader(actual)) if *actual == output.header)
        );
        if nested {
            assert!(
                output.header.attributes.is_empty(),
                "leading builtin test must have been consumed before identity_attr"
            );
        }
        let exact = catalog.select(Some(names[index]), true);
        assert_eq!(exact.len(), 1);
        assert_eq!(exact[0].definition, case.definition);
    }
    let leaf = if nested { "test" } else { "panic" };
    assert!(catalog.select(Some(leaf), true).is_empty());
    assert_eq!(
        catalog
            .select(Some(leaf), false)
            .iter()
            .map(|case| case.name.as_str())
            .collect::<Vec<_>>(),
        names
    );
    assert!(catalog.select(Some("missing::panic"), true).is_empty());
    assert!(catalog.select(Some("missing"), false).is_empty());
    if !nested {
        assert_eq!(
            catalog
                .select(Some("what_if_we_use_panic_directly"), false)
                .iter()
                .map(|case| case.name.as_str())
                .collect::<Vec<_>>(),
            &names[2..]
        );
    }
    let identities = catalog
        .cases()
        .iter()
        .map(|case| case.definition)
        .collect::<Vec<_>>();
    for _ in 0..3 {
        catalog.require_current(db).unwrap();
        let again = discovery.test_catalog(db).unwrap();
        assert_eq!(
            again
                .cases()
                .iter()
                .map(|case| case.definition)
                .collect::<Vec<_>>(),
            identities
        );
        assert_eq!(
            again
                .cases()
                .iter()
                .map(|case| case.name.as_str())
                .collect::<Vec<_>>(),
            names
        );
        assert_eq!(
            again
                .cases()
                .iter()
                .map(|case| case.should_panic)
                .collect::<Vec<_>>(),
            expected_panic
        );
        assert!(again.cases().iter().all(|case| case.expansion_ancestry
            == [record.key().fingerprint()]
            && !case.ignored
            && !case.unsupported_signature
            && case.attribute_error.is_none()));
    }
}
