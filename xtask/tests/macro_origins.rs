//! Native expansion-order matrix through owned name transport and textual lookup.
//! Declaration/source-order/origin facts are supplied; discovery and VM are later.
use lrr::{
    core::digest::Digest,
    expand::scopes::{MacroDefinition, MacroName, ScopeKey, TextualScopes},
    frontend::{
        hygiene::HygieneFacts,
        macro_origins::{ExpansionAncestry, MacroCandidate, MacroSelection},
        macro_scopes::{HygienicLookup, MacroDeclarations, MacroReference},
    },
    syntax::token_wire as runtime,
};
use lrr_expand_adapter::{
    declarative::DeclarativeMacro,
    hygiene::{HygieneStore, MarkRequest, ResolutionMode},
    token_wire::{Edition, Key, Transparency},
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
#[test]
fn expansion_origin_shadowing_and_ambiguity_match_macos_native_matrix() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-origin-matrix-{}-{}",
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
    let a = runtime::Key([1; 32]);
    let b = runtime::Key([2; 32]);
    let mut graph = ExpansionAncestry::default();
    graph.register(a, None).unwrap();
    graph.register(b, None).unwrap();
    let make = |id, origin| MacroCandidate {
        definition: MacroDefinition {
            identity: Digest([id; 32]),
            defining_unit: Digest([1; 32]),
            definition_edition: 2024,
        },
        origin,
    };
    let source = make(10, None);
    let first = make(11, Some(a));
    let second_same = make(12, Some(a));
    let second_other = make(12, Some(b));
    let macros = BTreeMap::from([
        (
            Digest([10; 32]),
            DeclarativeMacro::new(fixture::definition("()=>{29};", Edition::E2024)).unwrap(),
        ),
        (
            Digest([11; 32]),
            DeclarativeMacro::new(fixture::definition("()=>{41};", Edition::E2024)).unwrap(),
        ),
        (
            Digest([12; 32]),
            DeclarativeMacro::new(fixture::definition("()=>{43};", Edition::E2024)).unwrap(),
        ),
    ]);
    let cases = [
        (
            "generated_after_source",
            None,
            vec![first.clone(), source.clone()],
        ),
        (
            "source_after_generated",
            None,
            vec![source.clone(), first.clone()],
        ),
        (
            "generated_twice_same_output",
            None,
            vec![second_same, first.clone()],
        ),
        (
            "generated_twice_distinct_outputs",
            None,
            vec![second_other, first.clone()],
        ),
        (
            "within_same_output",
            Some(a),
            vec![first.clone(), source.clone()],
        ),
        ("argument_name", None, vec![first.clone(), source.clone()]),
        ("nested_block", None, vec![first.clone(), source.clone()]),
    ];
    let mut hygiene = HygieneStore::default();
    let root_context = hygiene.root(Key([1; 32]), Edition::E2024).unwrap();
    let marked = hygiene
        .apply(MarkRequest {
            parent: root_context,
            expansion: Key(a.0),
            defining_unit: Key([1; 32]),
            definition_edition: Edition::E2024,
            call_site: root_context,
            transparency: Transparency::SemiTransparent,
        })
        .unwrap();
    assert!(
        hygiene
            .name_equivalent(marked, root_context, ResolutionMode::Item)
            .unwrap()
    );
    let keys = [root_context, marked];
    let rows = hygiene.resolution_contexts(&keys).unwrap();
    let names = hygiene.name_contexts(&keys).unwrap();
    let wire = lrr_expand_adapter::token_wire::TokenForest::new(vec![], hygiene.records(), vec![])
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
    let macro_name = MacroName::parse("m", 2024).unwrap();
    let mut reports = Vec::new();
    let input = fixture::input("");
    for (name, reference, candidates) in cases {
        let source = root.join(format!("fixtures/macros/hygiene/origins/{name}.rs"));
        let binary = work.join(name);
        let compiler = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        fs::write(work.join(format!("{name}.stderr")), &compiler.stderr).unwrap();
        let mut scopes = TextualScopes::default();
        let scope = ScopeKey::root(Digest([1; 32]));
        scopes.add_root(scope).unwrap();
        let mut declarations = MacroDeclarations::default();
        for (i, candidate) in candidates.iter().rev().enumerate() {
            // The argument-name fixture retains the caller token context even
            // though the declaration itself originates in an expansion.
            let context = if candidate.origin.is_some() && name != "argument_name" {
                runtime::Key(marked.0)
            } else {
                runtime::Key(root_context.0)
            };
            declarations
                .register(
                    candidate.definition.clone(),
                    context,
                    candidate.origin,
                    &facts,
                    &graph,
                )
                .unwrap();
            scopes
                .define(
                    scope,
                    u64::try_from(i).unwrap() + 1,
                    macro_name.clone(),
                    candidate.definition.clone(),
                )
                .unwrap();
        }
        let selected = match declarations
            .resolve(
                &scopes,
                MacroReference {
                    scope,
                    before: 10,
                    name: &macro_name,
                    context: runtime::Key(root_context.0),
                    origin: reference,
                },
                &facts,
                &graph,
            )
            .unwrap()
        {
            HygienicLookup::Selection(selected) => selected,
            HygienicLookup::NeedInput(_) => panic!("complete fixture suspended"),
        };
        match selected {
            MacroSelection::Ambiguous(definitions) => {
                assert!(!compiler.status.success());
                assert!(String::from_utf8_lossy(&compiler.stderr).contains("error[E0659]"));
                assert!(
                    MacroSelection::Ambiguous(definitions.clone())
                        .require_definition()
                        .is_err()
                );
                reports.push(serde_json::json!({"case":name,"native":"E0659","lrr":"ambiguous","candidates":definitions.iter().map(|d|d.identity.to_string()).collect::<Vec<_>>() }));
            }
            MacroSelection::Defined(definition) => {
                assert!(
                    compiler.status.success(),
                    "{}",
                    String::from_utf8_lossy(&compiler.stderr)
                );
                let native = Command::new(binary).output().unwrap();
                assert!(native.status.success());
                fs::write(work.join(format!("{name}.stdout")), &native.stdout).unwrap();
                let result = macros[&definition.identity]
                    .expand(&input, fixture::call_site(&input))
                    .unwrap()
                    .unwrap();
                let literals = fixture::literal_texts(&result.tokens);
                assert_eq!(literals.len(), 1);
                let observed = literals[0].parse::<u64>().unwrap();
                let native = String::from_utf8(native.stdout)
                    .unwrap()
                    .trim()
                    .parse::<u64>()
                    .unwrap();
                assert_eq!(observed, native);
                reports.push(serde_json::json!({"case":name,"observed":observed,"native":native,"lrr":"defined"}));
            }
            MacroSelection::Absent => panic!("fixture macro disappeared"),
        }
    }
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","passed":true,"scope":"textual source-order lookup with actual pinned owned name classes and declaration origins; source facts supplied, discovery and guest VM pending","cases":reports})).unwrap()).unwrap();
}
