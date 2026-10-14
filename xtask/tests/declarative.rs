//! Actual standalone Rust native oracle for matcher fixtures; no selected live Cargo unit.
use lrr_expand_adapter::{declarative::DeclarativeMacro, token_wire::Edition};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
fn rustc(args: &[&str], evidence: &Path, label: &str) -> std::process::Output {
    let output = Command::new("rustc")
        .arg("+1.99.0")
        .args(args)
        .output()
        .unwrap();
    fs::write(evidence.join(format!("{label}.json")),serde_json::to_vec_pretty(&json!({"command":"rustc","channel":"1.99.0","arguments":args,"exit_code":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)})).unwrap()).unwrap();
    output
}
#[test]
fn pinned_declarative_bindings_and_fragment_editions_match_native_oracle() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m006-native-oracle-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&evidence).unwrap();
    let version = rustc(&["-Vv"], &evidence, "version");
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("rustc 1.99.0"));
    let library = evidence.join("libm006_edition2021.rlib");
    let source = root.join("fixtures/macros/declarative/edition2021.rs");
    let result = rustc(
        &[
            "--edition=2021",
            "--crate-name=m006_edition2021",
            "--crate-type=rlib",
            source.to_str().unwrap(),
            "-o",
            library.to_str().unwrap(),
        ],
        &evidence,
        "definition-2021",
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let binary = evidence.join("native-oracle");
    let source = root.join("fixtures/macros/declarative/native-oracle.rs");
    let external = format!("m006_edition2021={}", library.display());
    let result = rustc(
        &[
            "--edition=2024",
            "--crate-name=m006_native_oracle",
            source.to_str().unwrap(),
            "--extern",
            &external,
            "-o",
            binary.to_str().unwrap(),
        ],
        &evidence,
        "call-2024",
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let executed = Command::new(&binary).output().unwrap();
    assert!(executed.status.success());
    fs::write(evidence.join("native.stdout"), &executed.stdout).unwrap();
    fs::write(evidence.join("native.stderr"), &executed.stderr).unwrap();
    let native = String::from_utf8(executed.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Vec<u64>>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(native.len(), 4);
    let mut editions = Vec::new();
    for (edition, fragment) in [
        (Edition::E2021, "expr"),
        (Edition::E2024, "expr"),
        (Edition::E2024, "expr_2021"),
    ] {
        let rules = DeclarativeMacro::new(fixture::definition(
            &format!("($value:{fragment}) => {{1}}; (const $value:block) => {{2}};"),
            edition,
        ))
        .unwrap();
        let args = fixture::input("const { 7 }");
        let output = rules
            .expand(&args, fixture::call_site(&args))
            .unwrap()
            .unwrap();
        let values = fixture::literal_texts(&output.tokens);
        assert_eq!(values.len(), 1);
        editions.push(values[0].parse::<u64>().unwrap());
    }
    assert_eq!(editions, native[0]);
    let mut observed = vec![editions];
    for (definition, input) in [
        ("($($value:literal),*) => {[$($value),*]};", "7,2,11"),
        (
            "($(($($a:literal),*; $($b:literal),*)),*) => { ($([$(( $a, $b )),*]),*) };",
            "(1,2;11,12),(3;13)",
        ),
    ] {
        let rules = DeclarativeMacro::new(fixture::definition(definition, Edition::E2024)).unwrap();
        let args = fixture::input(input);
        let output = rules
            .expand(&args, fixture::call_site(&args))
            .unwrap()
            .unwrap();
        observed.push(
            fixture::literal_texts(&output.tokens)
                .into_iter()
                .map(|text| text.parse::<u64>().unwrap())
                .collect(),
        );
    }
    let nested_rules = DeclarativeMacro::new(fixture::definition(
        "($($value:literal),*) => {values!($($value),*)};",
        Edition::E2024,
    ))
    .unwrap();
    let args = fixture::input("7,2,11");
    let nested = nested_rules
        .expand(&args, fixture::call_site(&args))
        .unwrap()
        .unwrap();
    let args = nested_input(&nested.tokens, "values");
    let values_rules = DeclarativeMacro::new(fixture::definition(
        "($($value:literal),*) => {[$($value),*]};",
        Edition::E2024,
    ))
    .unwrap();
    let expanded = values_rules
        .expand(&args, fixture::call_site(&args))
        .unwrap()
        .unwrap();
    observed.push(
        fixture::literal_texts(&expanded.tokens)
            .into_iter()
            .map(|text| text.parse::<u64>().unwrap())
            .collect(),
    );
    assert_eq!(observed, native);
    let source = root.join("fixtures/macros/declarative/cardinality-oracle.rs");
    let negative = rustc(
        &[
            "--edition=2024",
            "--crate-name=m006_cardinality_oracle",
            "--emit=metadata",
            source.to_str().unwrap(),
            "-o",
            evidence.join("negative.rmeta").to_str().unwrap(),
        ],
        &evidence,
        "cardinality-rejected",
    );
    assert_eq!(negative.status.code(), Some(1));
    let diagnostic = String::from_utf8_lossy(&negative.stderr);
    assert!(diagnostic.contains("repeats 2 times") && diagnostic.contains("repeats 1 time"));
    fs::write(evidence.join("report.json"),serde_json::to_vec_pretty(&json!({"schema":1,"task":"M006","platform":std::env::consts::OS,"passed":true,"scope":"native rule selection and literal binding order, not guest VM execution or expansion hygiene","native":native,"pinned":observed,"cardinality_rejection_observed":true,"selected_live_unit_compilation":false})).unwrap()).unwrap();
}

fn nested_input(
    tokens: &lrr_expand_adapter::token_wire::TokenForest,
    name: &str,
) -> lrr_expand_adapter::token_wire::TokenForest {
    use lrr_expand_adapter::token_wire::{TokenForest, TokenTree};
    let [TokenTree::Group { children, .. }] = tokens.roots() else {
        panic!("missing expansion root")
    };
    let [
        TokenTree::Ident { text, .. },
        TokenTree::Punct { scalar: '!', .. },
        args @ TokenTree::Group { .. },
    ] = children.as_slice()
    else {
        panic!("expected controlled nested invocation")
    };
    assert_eq!(text, name);
    TokenForest::new(
        vec![args.clone()],
        tokens.contexts().to_vec(),
        tokens.spans().to_vec(),
    )
    .unwrap()
}
#[test]
fn real_declarative_recursion_hits_reduced_frame_limit_before_next_backend_call() {
    use lrr::{
        core::digest::Digest,
        expand::declarative::{DeclarativeSession, ExpansionLimits},
        syntax::token_wire::TokenForest,
    };
    let limits = ExpansionLimits {
        frames: 4,
        ..ExpansionLimits::default()
    };
    let mut session = DeclarativeSession::new(limits).unwrap();
    let rules = DeclarativeMacro::new(fixture::definition(
        "($value:literal) => {again!($value)};",
        Edition::E2024,
    ))
    .unwrap();
    let definition = Digest(rules.definition().fingerprint().0);
    let mut input = fixture::input("0");
    let mut parent = None;
    let mut calls = 0;
    let error = loop {
        let frame = match session.begin(parent, definition) {
            Ok(frame) => frame,
            Err(error) => break error,
        };
        calls += 1;
        let result = rules
            .expand(&input, fixture::call_site(&input))
            .unwrap()
            .unwrap();
        let owned = TokenForest::decode(&result.tokens.encode().unwrap()).unwrap();
        session.adopt(frame, &owned).unwrap();
        input = nested_input(&result.tokens, "again");
        parent = Some(frame);
    };
    assert_eq!(error.code, "LRR-E-EXPANSION-CYCLE");
    assert_eq!(calls, 4);
    assert_eq!(session.counts().invocations, 4);
    assert_eq!(session.counts().max_frames, 4);
    assert_eq!(session.active_definitions().count(), 4);
    assert!(session.begin(parent, definition).is_err());
    let mut fresh = DeclarativeSession::new(limits).unwrap();
    let rules = DeclarativeMacro::new(fixture::definition(
        "(3)=>{walk!(2)};(2)=>{walk!(1)};(1)=>{walk!(0)};(0)=>{[7,2,11]};",
        Edition::E2024,
    ))
    .unwrap();
    let definition = Digest(rules.definition().fingerprint().0);
    let mut input = fixture::input("3");
    let mut frames = Vec::new();
    let output = loop {
        let frame = fresh.begin(frames.last().copied(), definition).unwrap();
        let result = rules
            .expand(&input, fixture::call_site(&input))
            .unwrap()
            .unwrap();
        fresh
            .adopt(
                frame,
                &TokenForest::decode(&result.tokens.encode().unwrap()).unwrap(),
            )
            .unwrap();
        frames.push(frame);
        if result.matched_arm == 3 {
            break result.tokens;
        }
        input = nested_input(&result.tokens, "walk");
    };
    assert_eq!(fixture::literal_texts(&output), ["7", "2", "11"]);
    while let Some(frame) = frames.pop() {
        fresh.finish(frame).unwrap()
    }
    assert_eq!(fresh.active_definitions().count(), 0);
    assert_eq!(fresh.counts().invocations, 4);
    let literal_rule =
        DeclarativeMacro::new(fixture::definition("($v:literal) => {$v};", Edition::E2024))
            .unwrap();
    let args = fixture::input("1 2");
    assert_eq!(
        literal_rule
            .expand(&args, fixture::call_site(&args))
            .unwrap()
            .err()
            .unwrap()
            .kind,
        lrr_expand_adapter::declarative::FailureKind::LeftoverTokens
    );
    let no_rules = DeclarativeMacro::new(fixture::definition("", Edition::E2024)).unwrap();
    assert_eq!(
        no_rules
            .expand(&args, fixture::call_site(&args))
            .unwrap()
            .err()
            .unwrap()
            .kind,
        lrr_expand_adapter::declarative::FailureKind::NoMatchingRule
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m006-recursion-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&evidence).unwrap();
    let source = root.join("fixtures/macros/declarative/recursive-oracle.rs");
    let native = rustc(
        &[
            "--edition=2024",
            "--crate-name=m006_recursion_oracle",
            "--emit=metadata",
            source.to_str().unwrap(),
            "-o",
            evidence.join("native.rmeta").to_str().unwrap(),
        ],
        &evidence,
        "native-recursion",
    );
    assert_eq!(native.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&native.stderr).contains("recursion limit reached"));
    fs::write(evidence.join("report.json"),serde_json::to_vec_pretty(&json!({"schema":1,"task":"M006","passed":true,"reduced_frames":4,"backend_calls_before_rejection":calls,"rejected_request_counts":session.counts(),"fresh_finite_counts":fresh.counts(),"native_reduced_recursion_rejection":true,"scope":"explicitly resolved fixture recursion, macro namespace integration remains M007"})).unwrap()).unwrap();
}

#[test]
fn native_oracle_accepts_large_repetition_and_rejects_missing_driver() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m006-repetition-driver-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&evidence).unwrap();
    let args = std::iter::repeat_n("7u8", 65_537)
        .collect::<Vec<_>>()
        .join(",");
    let source = evidence.join("large.rs");
    fs::write(&source,format!("macro_rules! values {{($($v:literal),*) => {{[$($v),*]}};}}\nfn main(){{let a=values!({args});assert!(a.iter().all(|v|*v==7));println!(\"{{}}\",a.len());}}\n")).unwrap();
    let binary = evidence.join("large");
    let result = rustc(
        &[
            "--edition=2024",
            source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ],
        &evidence,
        "native-large",
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let ran = Command::new(&binary).output().unwrap();
    assert!(ran.status.success());
    let count = String::from_utf8(ran.stdout)
        .unwrap()
        .trim()
        .parse::<usize>()
        .unwrap();
    let rules = DeclarativeMacro::new(fixture::definition(
        "($($v:literal),*) => {[$($v),*]};",
        Edition::E2024,
    ))
    .unwrap();
    let input = fixture::input(&args);
    let output = rules
        .expand(&input, fixture::call_site(&input))
        .unwrap()
        .unwrap();
    let actual = fixture::literal_texts(&output.tokens);
    assert_eq!(actual.len(), count);
    assert!(actual.iter().all(|value| value == "7u8"));
    for (label, definition, input) in [
        ("fragment-driver", "($v:literal) => {[$($v),*]};", "7"),
        ("no-driver", "() => {[$(7),*]};", ""),
    ] {
        let source = evidence.join(format!("{label}.rs"));
        fs::write(
            &source,
            format!("macro_rules! values {{{definition}}}\nfn main(){{let _=values!({input});}}\n"),
        )
        .unwrap();
        let metadata = evidence.join(format!("{label}.rmeta"));
        let native = rustc(
            &[
                "--edition=2024",
                "--emit=metadata",
                "--crate-name=driver_oracle",
                source.to_str().unwrap(),
                "-o",
                metadata.to_str().unwrap(),
            ],
            &evidence,
            label,
        );
        assert_eq!(native.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&native.stderr)
                .contains("no syntax variables matched as repeating at this depth")
        );
        let rules = DeclarativeMacro::new(fixture::definition(definition, Edition::E2024)).unwrap();
        let args = fixture::input(input);
        assert_eq!(
            rules
                .expand(&args, fixture::call_site(&args))
                .unwrap()
                .err()
                .unwrap()
                .kind,
            lrr_expand_adapter::declarative::FailureKind::Binding
        );
    }
    fs::write(evidence.join("report.json"), serde_json::to_vec_pretty(&json!({"schema":1,"task":"M006","native_elements":count,"backend_elements":actual.len(),"missing_drivers_rejected":true,"passed":true,"scope":"standalone native oracle and actual pinned transcription; allocation limits remain pending"})).unwrap()).unwrap();
}

#[test]
fn declarative_token_admission_records_real_zero_materialization_on_rejection() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m006-token-admission-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&evidence).unwrap();
    let suffix = std::iter::repeat_n("0", 1000).collect::<Vec<_>>().join(" ");
    let rules = DeclarativeMacro::new(fixture::definition(
        &format!("($($v:literal),*) => {{$($v {suffix})*}};"),
        Edition::E2024,
    ))
    .unwrap();
    let args = fixture::input(&std::iter::repeat_n("7", 2000).collect::<Vec<_>>().join(","));
    let (result, stats) = rules
        .expand_with_token_limit(&args, fixture::call_site(&args), 2_000_000)
        .unwrap();
    assert_eq!(
        result.err().unwrap().kind,
        lrr_expand_adapter::declarative::FailureKind::Limit
    );
    assert_eq!(stats.materializations, 0);
    assert!(stats.max_preflight_tokens > 2_000_000);
    fs::write(evidence.join("report.json"),serde_json::to_vec_pretty(&json!({"schema":1,"task":"M006","token_limit":2_000_000,"stats":stats,"passed":true,"scope":"final generated-token admission before output materialization; wire-byte and speculative scratch limits pending"})).unwrap()).unwrap();
}

#[test]
fn declarative_byte_admission_records_real_zero_materialization_on_rejection() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m006-byte-admission-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&evidence).unwrap();
    let rules = DeclarativeMacro::new(fixture::definition(
        "($v:literal) => {[$v,$v]}; ($v:literal) => {1};",
        Edition::E2024,
    ))
    .unwrap();
    let args = fixture::input(&format!("r#\"{}\"#", "é".repeat(9 * 1024 * 1024)));
    let (result, stats) = rules
        .expand_with_limits(
            &args,
            fixture::call_site(&args),
            2_000_000,
            32 * 1024 * 1024,
        )
        .unwrap();
    assert_eq!(
        result.err().unwrap().kind,
        lrr_expand_adapter::declarative::FailureKind::Limit
    );
    assert_eq!(stats.materializations, 0);
    assert_eq!(stats.preflight_calls, 1);
    assert!(stats.max_preflight_bytes > 32 * 1024 * 1024);
    assert!(stats.max_preflight_tokens < 10);
    fs::write(evidence.join("report.json"),serde_json::to_vec_pretty(&json!({"schema":1,"task":"M006","byte_limit":32*1024*1024,"stats":stats,"passed":true,"scope":"exact final schema2 output bytes including distinct source/context records before main output materialization; speculative scratch remains pending"})).unwrap()).unwrap();
}

#[test]
fn nested_replay_native_oracle_and_scratch_statistics_match_admitted_output() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m006-scratch-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&evidence).unwrap();
    let mut pattern = "$v:literal".to_owned();
    let mut template = "$v".to_owned();
    const DEPTH: usize = 80;
    for _ in 0..DEPTH {
        pattern = format!("$(({pattern}))*");
        template = format!("$({template})*");
    }
    let source = evidence.join("native.rs");
    let input = format!("{}7{}", "(".repeat(DEPTH), ")".repeat(DEPTH));
    fs::write(&source,format!("macro_rules! nested {{({pattern}) => {{{template}}};}}\nfn main(){{println!(\"{{}}\",nested!({input}));}}\n")).unwrap();
    let binary = evidence.join("native");
    let compiled = rustc(
        &[
            "--edition=2024",
            source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ],
        &evidence,
        "native-deep-repetition",
    );
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let ran = Command::new(&binary).output().unwrap();
    assert!(ran.status.success());
    let native = String::from_utf8(ran.stdout)
        .unwrap()
        .trim()
        .parse::<u64>()
        .unwrap();
    let rules = DeclarativeMacro::new(fixture::definition(
        &format!("({pattern}) => {{{template}}};"),
        Edition::E2024,
    ))
    .unwrap();
    let mut rows = Vec::new();
    for source in ["".to_owned(), input] {
        let args = fixture::input(&source);
        let (result, stats) = rules
            .expand_with_limits(
                &args,
                fixture::call_site(&args),
                2_000_000,
                32 * 1024 * 1024,
            )
            .unwrap();
        let output = result.unwrap();
        let values = fixture::literal_texts(&output.tokens)
            .into_iter()
            .map(|text| text.parse::<u64>().unwrap())
            .collect::<Vec<_>>();
        if source.is_empty() {
            assert!(values.is_empty());
            assert_eq!(stats.materialized_leaves, 0);
            assert_eq!(stats.materialized_raw_peak, 1);
        } else {
            assert_eq!(values, [native]);
            assert_eq!(stats.materialized_leaves, 1);
            assert_eq!(stats.materialized_raw_peak, DEPTH + 2);
        }
        assert!(stats.materialized_raw_peak <= stats.admitted_tokens + 256);
        rows.push(json!({"empty":source.is_empty(),"values":values,"stats":stats}));
    }
    fs::write(evidence.join("report.json"),serde_json::to_vec_pretty(&json!({"schema":1,"task":"M006","passed":true,"native_value":native,"depth":DEPTH,"rows":rows,"scope":"same pinned replay iteration recipe; empty/after-end trials and trailing separators skipped; temporary raw slots guarded before append; no guest VM or namespace/hygiene acceptance"})).unwrap()).unwrap();
}

#[test]
fn stable_macro_profile_matches_native_rejection_and_caches_definition_error() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m006-stable-profile-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&evidence).unwrap();
    let source = evidence.join("native.rs");
    fs::write(
        &source,
        "macro_rules! count {($($v:tt),*) => {${count($v)}};}\nfn main(){let _=count!(a,b);}\n",
    )
    .unwrap();
    let compiled = rustc(
        &[
            "--edition=2024",
            "--crate-name=m006_stable_profile",
            "--emit=metadata",
            source.to_str().unwrap(),
            "-o",
            evidence.join("native.rmeta").to_str().unwrap(),
        ],
        &evidence,
        "native-profile",
    );
    assert_eq!(compiled.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&compiled.stderr).contains("E0658"));
    assert!(
        String::from_utf8_lossy(&compiled.stderr)
            .contains("meta-variable expressions are unstable")
    );
    let rules = DeclarativeMacro::new(fixture::definition(
        "($($v:tt),*) => {${count($v)}};",
        Edition::E2024,
    ))
    .unwrap();
    let args = fixture::input("a,b");
    let mut failures = Vec::new();
    for _ in 0..2 {
        let (result, stats) = rules
            .expand_with_limits(
                &args,
                fixture::call_site(&args),
                2_000_000,
                32 * 1024 * 1024,
            )
            .unwrap();
        let failure = result.err().unwrap();
        assert_eq!(
            failure.kind,
            lrr_expand_adapter::declarative::FailureKind::DefinitionSyntax
        );
        assert_eq!(stats.preflight_calls, 0);
        assert_eq!(stats.materializations, 0);
        failures.push(json!({"failure":failure,"stats":stats}));
    }
    assert_eq!(failures[0], failures[1]);
    fs::write(evidence.join("report.json"),serde_json::to_vec_pretty(&json!({"schema":1,"task":"M006","passed":true,"native_unstable_rejection":true,"definition_error_reused":true,"invocations":failures,"scope":"ordinary stable user macro definitions; certified compiler-owned builtin handling remains M011"})).unwrap()).unwrap();
}
