//! Standalone native comparison of the textual namespace component, before
//! M007's separate hygiene/local-binding integration is complete.
use lrr::{
    core::digest::Digest,
    expand::scopes::{MacroDefinition, MacroName, ScopeKey, ScopeLookup, TextualScopes},
};
use lrr_expand_adapter::{declarative::DeclarativeMacro, token_wire::Edition};
use serde_json::json;
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
fn textual_macro_namespace_expansions_match_macos_native_oracle() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m007-textual-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&evidence).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(evidence.join("rustc-version.txt"), version.stdout).unwrap();
    let definitions = ["() => {17};", "() => {29};", "() => {41};"];
    let source = format!(
        "fn main() {{ macro_rules! choose {{ {} }} let a=choose!(); let b={{ let x=choose!(); macro_rules! choose {{ {} }} [x,choose!()] }}; macro_rules! choose {{ {} }} let c={{ macro_rules! generate {{ () => {{ macro_rules! emitted {{ {} }} macro_rules! emitted {{ {} }} }}; }} generate!(); emitted!() }}; println!(\"[{{}},{{}},{{}},{{}},{{}}]\",a,b[0],b[1],choose!(),c); }}",
        definitions[0], definitions[2], definitions[1], definitions[0], definitions[2]
    );
    let source_path = evidence.join("textual.rs");
    let binary = evidence.join("textual");
    fs::write(&source_path, source).unwrap();
    let compiled = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(&source_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    fs::write(evidence.join("compiler.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = Command::new(&binary).output().unwrap();
    assert!(output.status.success());
    fs::write(evidence.join("native.stdout"), &output.stdout).unwrap();
    let native: Vec<u64> = serde_json::from_slice(&output.stdout).unwrap();
    let unit = Digest([1; 32]);
    let root_scope = ScopeKey::root(unit);
    let name = MacroName::parse("choose", 2024).unwrap();
    let mut scopes = TextualScopes::default();
    scopes.add_root(root_scope).unwrap();
    let mut macros = BTreeMap::new();
    for (index, body) in definitions.iter().enumerate() {
        let identity = Digest([u8::try_from(index).unwrap(); 32]);
        macros.insert(
            identity,
            DeclarativeMacro::new(fixture::definition(body, Edition::E2024)).unwrap(),
        );
    }
    let definition = |index: u8| MacroDefinition {
        identity: Digest([index; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    scopes
        .define(root_scope, 10, name.clone(), definition(0))
        .unwrap();
    let child = scopes.add_child(root_scope, 20, 0).unwrap();
    scopes
        .define(child, 30, name.clone(), definition(2))
        .unwrap();
    scopes
        .define(root_scope, 40, name.clone(), definition(1))
        .unwrap();
    let generated = scopes.add_child(root_scope, 50, 1).unwrap();
    let generated_name = MacroName::parse("emitted", 2024).unwrap();
    let invocation = Digest([9; 32]);
    scopes.block(generated, 10, invocation).unwrap();
    scopes
        .publish(
            generated,
            10,
            invocation,
            vec![
                (generated_name.clone(), definition(0)),
                (generated_name.clone(), definition(2)),
            ],
        )
        .unwrap();
    let mut observed = Vec::new();
    for (scope, position, lookup_name) in [
        (root_scope, 15, &name),
        (child, 25, &name),
        (child, 35, &name),
        (root_scope, 45, &name),
        (generated, 11, &generated_name),
    ] {
        let ScopeLookup::Defined(definition) = scopes.lookup(scope, position, lookup_name).unwrap()
        else {
            panic!("missing fixture macro");
        };
        let args = fixture::input("");
        let expansion = macros[&definition.identity]
            .expand(&args, fixture::call_site(&args))
            .unwrap()
            .unwrap();
        let literals = fixture::literal_texts(&expansion.tokens);
        assert_eq!(literals.len(), 1);
        observed.push(literals[0].parse::<u64>().unwrap());
    }
    assert_eq!(observed, native);
    let witness = root.join("fixtures/macros/hygiene/expanded-name-ambiguity.rs");
    let rejected = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(&witness)
        .arg("-o")
        .arg(evidence.join("rejected"))
        .output()
        .unwrap();
    fs::write(evidence.join("collision.stderr"), &rejected.stderr).unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("error[E0659]"));
    fs::write(evidence.join("report.json"), serde_json::to_vec_pretty(&json!({"task":"M007", "platform":"aarch64-apple-darwin", "scope":"textual namespace and actual pinned expansion component; local-binding hygiene pending", "native":native,"observed":observed,"passed":true,"expanded_collision_native_code":"E0659","expanded_collision_native_exit_code":rejected.status.code(),"expanded_collision_lrr_verified":false})).unwrap()).unwrap();
}
