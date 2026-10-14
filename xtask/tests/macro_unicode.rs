//! Controlled macOS compiler oracle for canonical macro namespace names.
use lrr::{
    core::digest::Digest,
    expand::scopes::{MacroDefinition, MacroName, ScopeKey, ScopeLookup, TextualScopes},
};
use lrr_expand_adapter::{declarative::DeclarativeMacro, token_wire::Edition};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
#[test]
fn canonical_unicode_macro_lookup_matches_macos_native() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m007-unicode-{}-{}",
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
    let body = "() => {37};";
    let source = format!(
        "macro_rules! café {{ {body} }} fn main() {{ println!(\"{{}}\",cafe\u{301}!()); }}"
    );
    let path = evidence.join("unicode.rs");
    let binary = evidence.join("unicode");
    fs::write(&path, source).unwrap();
    let compiled = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(&path)
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
    let native = Command::new(binary).output().unwrap();
    assert!(native.status.success());
    fs::write(evidence.join("native.stdout"), &native.stdout).unwrap();
    let unit = Digest([1; 32]);
    let scope = ScopeKey::root(unit);
    let mut scopes = TextualScopes::default();
    scopes.add_root(scope).unwrap();
    let def = MacroDefinition {
        identity: Digest([2; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    scopes
        .define(
            scope,
            1,
            MacroName::parse("café", 2024).unwrap(),
            def.clone(),
        )
        .unwrap();
    let ScopeLookup::Defined(selected) = scopes
        .lookup(scope, 2, &MacroName::parse("cafe\u{301}", 2024).unwrap())
        .unwrap()
    else {
        panic!("canonical macro definition missing")
    };
    assert_eq!(selected, def);
    let mac = DeclarativeMacro::new(fixture::definition(body, Edition::E2024)).unwrap();
    let input = fixture::input("");
    let result = mac
        .expand(&input, fixture::call_site(&input))
        .unwrap()
        .unwrap();
    let observed = fixture::literal_texts(&result.tokens);
    assert_eq!(observed.len(), 1);
    assert_eq!(
        observed[0].parse::<u64>().unwrap(),
        String::from_utf8(native.stdout)
            .unwrap()
            .trim()
            .parse::<u64>()
            .unwrap()
    );
}

#[test]
fn canonical_unicode_token_matching_matches_native_in_all_editions() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m007-unicode-tokens-{}-{}",
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
    let body = "(café) => {17}; (가) => {19}; (A) => {23}; ($other:ident) => {29};";
    let spellings = ["cafe\u{301}", "\u{1100}\u{1161}", "Ａ"];
    for (number, edition) in [
        (2015, Edition::E2015),
        (2018, Edition::E2018),
        (2021, Edition::E2021),
        (2024, Edition::E2024),
    ] {
        let source = format!(
            "macro_rules! pick {{ {body} }} fn main() {{ println!(\"[{{}},{{}},{{}}]\",pick!({}),pick!({}),pick!({})); }}",
            spellings[0], spellings[1], spellings[2]
        );
        let path = evidence.join(format!("unicode-{number}.rs"));
        let binary = evidence.join(format!("unicode-{number}"));
        fs::write(&path, source).unwrap();
        let compiled = Command::new("rustc")
            .args(["+1.99.0", &format!("--edition={number}")])
            .arg(&path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        fs::write(
            evidence.join(format!("compiler-{number}.stderr")),
            &compiled.stderr,
        )
        .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let native = Command::new(binary).output().unwrap();
        assert!(native.status.success());
        fs::write(
            evidence.join(format!("native-{number}.stdout")),
            &native.stdout,
        )
        .unwrap();
        let native: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
        let mac = DeclarativeMacro::new(fixture::definition(body, edition)).unwrap();
        let mut observed = Vec::new();
        for spelling in spellings {
            let input = fixture::input(spelling);
            let result = mac
                .expand(&input, fixture::call_site(&input))
                .unwrap()
                .unwrap();
            let literals = fixture::literal_texts(&result.tokens);
            assert_eq!(literals.len(), 1);
            observed.push(literals[0].parse::<u64>().unwrap());
        }
        assert_eq!(observed, native, "origin edition {number}");
    }
}
