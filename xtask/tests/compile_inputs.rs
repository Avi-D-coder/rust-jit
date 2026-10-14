//! M011 immutable environment operands and real macOS native comparison.
use lrr::{
    core::digest::Digest,
    expand::builtins::{
        CapturedEnvironmentValue as Value, CompileStringPoll, FrozenCompileEnvironment,
    },
    project::tool_manifest::CompileEnv,
    source::snapshot::CfgSet,
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
fn environment(unit: Digest) -> CompileEnv {
    CompileEnv {
        unit,
        host: "aarch64-apple-darwin".into(),
        target: "aarch64-apple-darwin".into(),
        cfg: CfgSet::default(),
        cargo: BTreeMap::new(),
        rustc_env: BTreeMap::new(),
        declared: BTreeMap::new(),
        out_dir: None,
    }
}
#[test]
fn frozen_compile_environment_distinguishes_absence_capture_and_exact_unit() {
    let unit = Digest([1; 32]);
    let mut input = environment(unit);
    input
        .declared
        .insert("VALUE".into(), Some("declared".into()));
    input.declared.insert("ABSENT".into(), None);
    input.cargo.insert("VALUE".into(), "cargo".into());
    input
        .rustc_env
        .insert("VALUE".into(), "build-script".into());
    let frozen = FrozenCompileEnvironment::new(unit, &input).unwrap();
    assert_eq!(
        frozen.lookup("VALUE").unwrap(),
        Value::Value("build-script")
    );
    assert_eq!(frozen.lookup("ABSENT").unwrap(), Value::Absent);
    assert_eq!(frozen.lookup("UNCAPTURED").unwrap(), Value::NeedCapture);
    assert!(matches!(
        frozen.string_input("ABSENT", false).unwrap(),
        CompileStringPoll::Ready(Ok(None))
    ));
    assert!(
        matches!(frozen.string_input("ABSENT",true).unwrap(), CompileStringPoll::Ready(Err(e)) if e.code=="LRR-E-COMPILE-INPUT")
    );
    assert!(
        matches!(frozen.string_input("UNCAPTURED",false).unwrap(), CompileStringPoll::NeedCapture { unit:u,.. } if u==unit)
    );
    assert!(FrozenCompileEnvironment::new(Digest([2; 32]), &input).is_err());
    let before = frozen.dependency("ABSENT").unwrap();
    input.declared.insert("ABSENT".into(), Some(String::new()));
    let changed = FrozenCompileEnvironment::new(unit, &input).unwrap();
    assert_eq!(changed.lookup("ABSENT").unwrap(), Value::Value(""));
    assert_ne!(before, changed.dependency("ABSENT").unwrap());
    assert_eq!(frozen.lookup("ABSENT").unwrap(), Value::Absent);
    assert_ne!(
        frozen.dependency("ABSENT").unwrap(),
        frozen.dependency("UNCAPTURED").unwrap()
    );
    input.unit = Digest([2; 32]);
    let other = FrozenCompileEnvironment::new(input.unit, &input).unwrap();
    assert_ne!(
        changed.dependency("VALUE").unwrap(),
        other.dependency("VALUE").unwrap()
    );
}
#[test]
fn compile_environment_refuses_malformed_names_outdir_and_resource_excess() {
    let unit = Digest([3; 32]);
    let mut input = environment(unit);
    let frozen = FrozenCompileEnvironment::new(unit, &input).unwrap();
    for key in ["", "BAD=KEY", "BAD\0KEY"] {
        assert!(frozen.lookup(key).is_err());
    }
    input
        .declared
        .insert("LONG".into(), Some("x".repeat(65_537)));
    assert!(FrozenCompileEnvironment::new(unit, &input).is_err());
    input.declared.clear();
    input.out_dir = Some("/controlled/out".into());
    assert!(FrozenCompileEnvironment::new(unit, &input).is_err());
    input
        .cargo
        .insert("OUT_DIR".into(), "/controlled/out".into());
    assert_eq!(
        FrozenCompileEnvironment::new(unit, &input)
            .unwrap()
            .lookup("OUT_DIR")
            .unwrap(),
        Value::Value("/controlled/out")
    );
    input.rustc_env.insert("OUT_DIR".into(), "/other".into());
    assert!(FrozenCompileEnvironment::new(unit, &input).is_err());
    input = environment(unit);
    for i in 0..513 {
        input
            .declared
            .insert(format!("K{i}"), Some("x".repeat(65_536)));
    }
    assert!(FrozenCompileEnvironment::new(unit, &input).is_err());
}
#[test]
fn actual_native_env_and_option_env_match_frozen_per_unit_values_and_absence() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-env-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("oracle.rs");
    fs::write(&source,"fn main(){println!(\"{:?}|{:?}\", env!(\"LRR_M011_REQUIRED\"), option_env!(\"LRR_M011_OPTIONAL\"));}").unwrap();
    let mut rows = vec![];
    for (index, required, optional) in [
        (1, "first", Some("present")),
        (2, "second", None),
        (3, "", Some("")),
    ] {
        let unit = Digest([index; 32]);
        let mut env = environment(unit);
        env.declared
            .insert("LRR_M011_REQUIRED".into(), Some(required.into()));
        env.declared
            .insert("LRR_M011_OPTIONAL".into(), optional.map(str::to_owned));
        let frozen = FrozenCompileEnvironment::new(unit, &env).unwrap();
        let CompileStringPoll::Ready(Ok(Some(value))) =
            frozen.string_input("LRR_M011_REQUIRED", true).unwrap()
        else {
            panic!("required captured value did not evaluate")
        };
        let CompileStringPoll::Ready(Ok(option)) =
            frozen.string_input("LRR_M011_OPTIONAL", false).unwrap()
        else {
            panic!("optional captured value did not evaluate")
        };
        let executable = root.join(format!("oracle-{index}"));
        let mut cmd = Command::new("rustc");
        cmd.args(["+1.99.0", "--edition=2024", "--crate-name", "env_oracle"])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .env("LRR_M011_REQUIRED", required)
            .env_remove("LRR_M011_OPTIONAL");
        if let Some(value) = optional {
            cmd.env("LRR_M011_OPTIONAL", value);
        }
        let compiled = cmd.output().unwrap();
        fs::write(
            root.join(format!("oracle-{index}.stderr")),
            &compiled.stderr,
        )
        .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let native = Command::new(&executable).output().unwrap();
        assert!(native.status.success());
        let observed = String::from_utf8(native.stdout).unwrap();
        assert_eq!(observed.trim(), format!("{value:?}|{option:?}"));
        rows.push(serde_json::json!({"unit":unit,"environment":frozen.fingerprint(),"required_dependency":frozen.dependency("LRR_M011_REQUIRED").unwrap(),"optional_dependency":frozen.dependency("LRR_M011_OPTIONAL").unwrap(),"native_stdout":observed}));
    }
    let mut missing = environment(Digest([4; 32]));
    missing.declared.insert("LRR_M011_REQUIRED".into(), None);
    let frozen_missing = FrozenCompileEnvironment::new(missing.unit, &missing).unwrap();
    assert!(matches!(
        frozen_missing
            .string_input("LRR_M011_REQUIRED", true)
            .unwrap(),
        CompileStringPoll::Ready(Err(_))
    ));
    let rejected = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-name",
            "env_missing_oracle",
        ])
        .arg(&source)
        .arg("-o")
        .arg(root.join("missing-required"))
        .env_remove("LRR_M011_REQUIRED")
        .env_remove("LRR_M011_OPTIONAL")
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    let missing_diagnostic = String::from_utf8(rejected.stderr).unwrap();
    assert!(
        missing_diagnostic
            .contains("environment variable `LRR_M011_REQUIRED` not defined at compile time")
    );
    fs::write(root.join("missing-required.stderr"), &missing_diagnostic).unwrap();
    fs::write(root.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"frozen environment operand component; no builtin resolution/guest/include claim","observations":rows,"missing_required_diagnostic":missing_diagnostic})).unwrap()).unwrap();
}

#[test]
fn compile_input_facets_bind_pinned_identity_and_refuse_forged_provenance() {
    use lrr::{
        expand::compile_input_identity::descriptor,
        frontend::compile_input_catalog::CompileInputCatalog,
    };
    let unit = Digest([5; 32]);
    let descriptors: Vec<_> = [
        "env",
        "option_env",
        "concat",
        "include",
        "include_str",
        "include_bytes",
    ]
    .iter()
    .map(|n| descriptor(n).unwrap().unwrap())
    .collect();
    let mut identities = std::collections::BTreeSet::new();
    for d in &descriptors {
        assert!(identities.insert(d.definition_identity()));
        assert_eq!(
            d.definition_identity(),
            descriptor(d.name()).unwrap().unwrap().definition_identity()
        );
    }
    assert!(descriptor("user_env").unwrap().is_none());
    assert!(descriptor("cfg").unwrap().is_none());
    assert!(descriptor(&"x".repeat(65_537)).is_err());
    let catalog = CompileInputCatalog::new(unit, 2024, descriptors).unwrap();
    for d in catalog.definitions() {
        let mut forged = d.clone();
        forged.defining_unit = Digest([6; 32]);
        assert!(catalog.select(&forged).is_err());
        let mut forged = d.clone();
        forged.definition_edition = 2021;
        assert!(catalog.select(&forged).is_err());
    }
    let env = descriptor("env").unwrap().unwrap();
    assert!(CompileInputCatalog::new(unit, 2024, [env.clone(), env]).is_err());
    assert!(CompileInputCatalog::new(unit, 2099, []).is_err());
    let frozen = FrozenCompileEnvironment::new(unit, &environment(unit)).unwrap();
    assert!(
        frozen
            .environment_operand(&descriptor("concat").unwrap().unwrap(), "VALUE")
            .is_err()
    );
}

#[test]
fn actual_compile_input_alias_identity_preserves_user_env_shadowing_and_native_values() {
    use lrr::{
        core::profile::ProfileIdentity,
        expand::{
            compile_input_identity::{CompileInputKind, descriptor},
            scopes::MacroName,
        },
        frontend::{
            compile_input_catalog::CompileInputCatalog,
            hygiene::HygieneFacts,
            macro_names::{MacroPath, PathLookup},
            source_macro_routes::SourceMacroRoutes,
        },
        source::snapshot::SourceBlob,
        syntax::{outline::ScopeKey, parse::ParsedFile, token_wire as runtime},
    };
    use lrr_expand_adapter::{
        hygiene::HygieneStore,
        token_wire::{Edition, Key, TokenForest},
    };
    let consumer = Digest([7; 32]);
    let core = Digest([8; 32]);
    let source = "use core::{env as read_value, option_env as maybe_value}; #[macro_export] macro_rules! env { ($v:literal) => { \"user\" }; } fn main(){println!(\"{:?}|{:?}|{:?}\",read_value!(\"LRR_M011_IDENTITY\"),maybe_value!(\"LRR_M011_ABSENT\"),env!(\"LRR_M011_IDENTITY\"));}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(consumer, vec![], &CfgSet::default())
        .unwrap();
    let mut hygiene = HygieneStore::default();
    let context = hygiene.root(Key(consumer.0), Edition::E2024).unwrap();
    let rows = hygiene.resolution_contexts(&[context]).unwrap();
    let names = hygiene.name_contexts(&[context]).unwrap();
    let facts = HygieneFacts::new(
        runtime::TokenForest::decode(
            &TokenForest::new(vec![], hygiene.records(), vec![])
                .unwrap()
                .encode()
                .unwrap(),
        )
        .unwrap()
        .contexts()
        .to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
    let catalog = CompileInputCatalog::new(
        core,
        2024,
        [
            descriptor("env").unwrap().unwrap(),
            descriptor("option_env").unwrap().unwrap(),
        ],
    )
    .unwrap();
    let routes = [
        catalog.source_routes().unwrap(),
        SourceMacroRoutes::from_outline(
            &outline,
            runtime::Key(context.0),
            facts,
            &BTreeMap::from([(MacroName::parse("core", 2024).unwrap(), core)]),
        )
        .unwrap(),
    ];
    let graph = SourceMacroRoutes::candidate_graph(&routes).unwrap();
    let scope = ScopeKey {
        crate_key: consumer,
        module_path: vec![],
        owner: None,
    };
    let select = |parts: &[&str]| {
        let path = MacroPath::new(
            consumer,
            parts
                .iter()
                .map(|n| MacroName::parse(n, 2024).unwrap())
                .collect(),
        )
        .unwrap();
        let PathLookup::Defined(d) = graph.lookup_from(&path, &scope).unwrap() else {
            panic!("original source alias did not resolve")
        };
        d
    };
    let selected = select(&["read_value"]);
    let required = catalog.select(&selected).unwrap().unwrap();
    assert_eq!(required.kind(), CompileInputKind::Env);
    assert_eq!(selected, select(&["core", "env"]));
    let option_definition = select(&["maybe_value"]);
    let optional = catalog.select(&option_definition).unwrap().unwrap();
    assert_eq!(optional.kind(), CompileInputKind::OptionEnv);
    assert!(catalog.select(&select(&["env"])).unwrap().is_none());
    let mut env = environment(consumer);
    env.declared
        .insert("LRR_M011_IDENTITY".into(), Some("captured".into()));
    env.declared.insert("LRR_M011_ABSENT".into(), None);
    let frozen = FrozenCompileEnvironment::new(consumer, &env).unwrap();
    let CompileStringPoll::Ready(Ok(Some(value))) = frozen
        .environment_operand(required, "LRR_M011_IDENTITY")
        .unwrap()
    else {
        panic!("resolved required operand failed")
    };
    let CompileStringPoll::Ready(Ok(option)) = frozen
        .environment_operand(optional, "LRR_M011_ABSENT")
        .unwrap()
    else {
        panic!("resolved optional operand failed")
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-identity-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("oracle.rs");
    fs::write(&path, source).unwrap();
    let exe = root.join("native");
    let built = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-name",
            "identity_oracle",
        ])
        .arg(&path)
        .arg("-o")
        .arg(&exe)
        .env("LRR_M011_IDENTITY", "captured")
        .env_remove("LRR_M011_ABSENT")
        .output()
        .unwrap();
    fs::write(root.join("native.stderr"), &built.stderr).unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(&exe).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), format!("{value:?}|{option:?}|{:?}", "user"));
    fs::write(root.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"controlled compiler facet/source alias/env operand identity; no automatic compiler facet or guest claim","required_definition":selected.identity,"optional_definition":option_definition.identity,"user_definition":select(&["env"]).identity,"builtin_unit":selected.defining_unit,"builtin_edition":selected.definition_edition,"native_stdout":stdout})).unwrap()).unwrap();
}

fn argument_tokens(text: &str) -> lrr::syntax::token_wire::TokenForest {
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, TokenTree, Transparency,
    };
    let full = token_wire::parse_source(
        text,
        SourceIdentity {
            unit: Key([11; 32]),
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: "environment-arguments.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([11; 32]),
            edition: Edition::E2024,
        },
        Key([12; 32]),
    )
    .unwrap();
    let [TokenTree::Group { .. }] = full.roots() else {
        panic!("missing source token root")
    };
    lrr::syntax::token_wire::TokenForest::decode(&full.encode().unwrap()).unwrap()
}

#[test]
fn environment_token_arguments_decode_pinned_literals_and_preserve_original_spans() {
    use lrr::expand::{
        compile_input_identity::descriptor,
        environment_arguments::{EnvironmentArguments, StringOperand},
    };
    let d = descriptor("env").unwrap().unwrap();
    let text = r##"(r#"LRR_M011_ESCAPED"#, "diagnostic \u{1f642}",)"##;
    let forest = argument_tokens(text);
    let args = EnvironmentArguments::parse(&d, &forest).unwrap();
    let StringOperand::Literal { value, span } = args.name() else {
        panic!("name did not decode")
    };
    assert_eq!(value, "LRR_M011_ESCAPED");
    let record = forest.spans().iter().find(|r| r.key() == *span).unwrap();
    assert_eq!(
        &text[record.start as usize..record.end as usize],
        r##"r#"LRR_M011_ESCAPED"#"##
    );
    assert!(
        matches!(args.missing_message(),Some(StringOperand::Literal{value,..}) if value=="diagnostic 🙂")
    );
    assert_eq!(
        args.fingerprint(),
        EnvironmentArguments::parse(&d, &forest)
            .unwrap()
            .fingerprint()
    );
    for text in [
        r#"["LRR_M011_ESCAPED"]"#,
        r#"{"LRR_M011_ESCAPED"}"#,
        r#"("LRR_M011_\u{45}SCAPED",)"#,
    ] {
        assert!(
            matches!(EnvironmentArguments::parse(&d,&argument_tokens(text)).unwrap().name(),StringOperand::Literal{value,..} if value=="LRR_M011_ESCAPED")
        );
    }
    for invalid in [
        r#"()"#,
        r#"("A" "B")"#,
        r#"("A",,)"#,
        r#"("A","message","extra")"#,
        r#"(b"A")"#,
        r#"(c"A")"#,
        r#"(true)"#,
        r#"(("A"))"#,
        r#"("A"suffix)"#,
        r#"("\q")"#,
    ] {
        assert!(
            EnvironmentArguments::parse(&d, &argument_tokens(invalid)).is_err(),
            "{invalid}"
        );
    }
    let optional = descriptor("option_env").unwrap().unwrap();
    assert!(
        EnvironmentArguments::parse(&optional, &argument_tokens(r#"("A","message")"#)).is_err()
    );
    assert!(
        EnvironmentArguments::parse(&descriptor("include").unwrap().unwrap(), &forest).is_err()
    );
}

#[test]
fn environment_token_arguments_suspend_nested_expansion_before_lookup_and_bind_descriptor() {
    use lrr::expand::{
        compile_input_identity::descriptor,
        environment_arguments::{EnvironmentArguments, EnvironmentOperandPoll},
    };
    let unit = Digest([11; 32]);
    let mut env = environment(unit);
    env.declared
        .insert("LRR_M011_ESCAPED".into(), Some("present".into()));
    env.declared.insert("MISSING".into(), None);
    let frozen = FrozenCompileEnvironment::new(unit, &env).unwrap();
    let d = descriptor("env").unwrap().unwrap();
    for (text, index) in [
        (r#"(concat!("LRR_","M011_ESCAPED"))"#, 0),
        (r#"("LRR_M011_ESCAPED",stringify!(unexpanded))"#, 1),
    ] {
        let forest = argument_tokens(text);
        let args = EnvironmentArguments::parse(&d, &forest).unwrap();
        let EnvironmentOperandPoll::NeedExpansion {
            operand,
            first_span,
            last_span,
        } = frozen.evaluate_environment_arguments(&d, &args).unwrap()
        else {
            panic!("nested operand guessed a result")
        };
        assert_eq!(operand, index);
        assert!(forest.spans().iter().any(|s| s.key() == first_span));
        assert!(forest.spans().iter().any(|s| s.key() == last_span));
    }
    let args =
        EnvironmentArguments::parse(&d, &argument_tokens(r#"("MISSING","custom diagnostic")"#))
            .unwrap();
    assert!(
        matches!(frozen.evaluate_environment_arguments(&d,&args).unwrap(),EnvironmentOperandPoll::Ready(Err(e)) if e.message=="custom diagnostic")
    );
    assert!(
        frozen
            .evaluate_environment_arguments(&descriptor("option_env").unwrap().unwrap(), &args)
            .is_err()
    );
    let args = EnvironmentArguments::parse(&d, &argument_tokens(r#"("UNCAPTURED")"#)).unwrap();
    assert!(
        matches!(frozen.evaluate_environment_arguments(&d,&args).unwrap(),EnvironmentOperandPoll::NeedCapture{unit:u,name,..} if u==unit && name=="UNCAPTURED")
    );
}

#[test]
fn actual_native_environment_token_literals_and_custom_missing_message_match() {
    use lrr::expand::{
        compile_input_identity::descriptor,
        environment_arguments::{EnvironmentArguments, EnvironmentOperandPoll},
    };
    let unit = Digest([11; 32]);
    let mut env = environment(unit);
    env.declared
        .insert("LRR_M011_ESCAPED".into(), Some("captured".into()));
    env.declared.insert("MISSING".into(), None);
    let frozen = FrozenCompileEnvironment::new(unit, &env).unwrap();
    let required = descriptor("env").unwrap().unwrap();
    let optional = descriptor("option_env").unwrap().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-token-env-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&root).unwrap();
    let mut observations = vec![];
    for (index, d, args_text, success) in [
        (0, &required, r#"("LRR_M011_\u{45}SCAPED",)"#, true),
        (1, &optional, r##"(r#"MISSING"#,)"##, true),
        (
            2,
            &required,
            r#"("MISSING","controlled missing \u{1f642}")"#,
            false,
        ),
    ] {
        let args = EnvironmentArguments::parse(d, &argument_tokens(args_text)).unwrap();
        let poll = frozen.evaluate_environment_arguments(d, &args).unwrap();
        let path = root.join(format!("oracle-{index}.rs"));
        fs::write(
            &path,
            format!(
                "fn main(){{println!(\"{{:?}}\",{}!{});}}",
                d.name(),
                args_text
            ),
        )
        .unwrap();
        let exe = root.join(format!("native-{index}"));
        let compiled = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name",
                "token_env_oracle",
            ])
            .arg(&path)
            .arg("-o")
            .arg(&exe)
            .env("LRR_M011_ESCAPED", "captured")
            .env_remove("MISSING")
            .output()
            .unwrap();
        let stderr = String::from_utf8(compiled.stderr).unwrap();
        fs::write(root.join(format!("native-{index}.stderr")), &stderr).unwrap();
        assert_eq!(compiled.status.success(), success, "{stderr}");
        let observation = if success {
            let EnvironmentOperandPoll::Ready(Ok(value)) = poll else {
                panic!("operand did not evaluate")
            };
            let output = Command::new(exe).output().unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8(output.stdout).unwrap();
            let expected = if index == 0 {
                format!("{:?}", value.unwrap())
            } else {
                format!("{value:?}")
            };
            assert_eq!(stdout.trim(), expected);
            stdout
        } else {
            let EnvironmentOperandPoll::Ready(Err(error)) = poll else {
                panic!("missing required did not fail")
            };
            assert!(stderr.contains(&error.message));
            stderr
        };
        observations.push(serde_json::json!({"case":index,"arguments":args.fingerprint(),"native_success":success,"observed":observation}));
    }
    fs::write(root.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"original-token env argument parsing/operand evaluation; no nested expansion or guest claim","observations":observations})).unwrap()).unwrap();
}

#[test]
fn pinned_literal_concat_uses_registered_site_and_checks_descriptor_and_limits() {
    use lrr::{
        expand::{compile_input_identity::descriptor, concat},
        syntax::token_wire::TokenTree,
    };
    let input = argument_tokens(r##"("a",r#"b"#,'\u{1f642}',true,-12i64,)"##);
    let [TokenTree::Group { open, .. }] = input.roots() else {
        panic!("missing arguments")
    };
    let d = descriptor("concat").unwrap().unwrap();
    let value =
        concat::literal_value(&d, &input, *open, 64 * 1024 * 1024, 32 * 1024 * 1024).unwrap();
    assert_eq!(value.value(), "ab🙂true-12");
    assert_eq!(value.origin(), *open);
    assert_eq!(
        value.fingerprint(),
        concat::literal_value(&d, &input, *open, 64 * 1024 * 1024, 32 * 1024 * 1024)
            .unwrap()
            .fingerprint()
    );
    assert!(
        concat::literal_value(
            &d,
            &input,
            lrr::syntax::token_wire::Key([0; 32]),
            64 * 1024 * 1024,
            32 * 1024 * 1024
        )
        .is_err()
    );
    assert!(
        concat::literal_value(
            &descriptor("env").unwrap().unwrap(),
            &input,
            *open,
            64 * 1024 * 1024,
            32 * 1024 * 1024
        )
        .is_err()
    );
    assert!(concat::literal_value(&d, &input, *open, 1, 32 * 1024 * 1024).is_err());
    assert!(concat::literal_value(&d, &input, *open, 64 * 1024 * 1024, 1).is_err());
}

#[test]
fn actual_native_pinned_concat_normalizes_numeric_literals_and_matches_native() {
    use lrr::{
        expand::{compile_input_identity::descriptor, concat},
        syntax::token_wire::TokenTree,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-concat-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&root).unwrap();
    let d = descriptor("concat").unwrap().unwrap();
    let mut observations = vec![];
    for (index, text, equivalent) in [
        (0, r##"("a",r#"b"#,'\u{1f642}',true,-12i64,)"##, true),
        (1, "()", true),
        (2, "(0x10)", true),
        (3, "(0b1010,0o17,1_000u64,-0xffi32)", true),
        (4, "(1_2.5f64,-1_0.0e+2,1f32,500u8)", true),
        (5, "(340282366920938463463374607431768211455u128)", true),
    ] {
        let input = argument_tokens(text);
        let [TokenTree::Group { open, .. }] = input.roots() else {
            panic!("missing concat arguments")
        };
        let expanded =
            concat::literal_value(&d, &input, *open, 64 * 1024 * 1024, 32 * 1024 * 1024).unwrap();
        let path = root.join(format!("oracle-{index}.rs"));
        fs::write(
            &path,
            format!("fn main(){{println!(\"{{}}\",concat!{text});}}"),
        )
        .unwrap();
        let exe = root.join(format!("native-{index}"));
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--crate-name", "concat_oracle"])
            .arg(&path)
            .arg("-o")
            .arg(&exe)
            .output()
            .unwrap();
        fs::write(root.join(format!("native-{index}.stderr")), &built.stderr).unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = Command::new(exe).output().unwrap();
        assert!(output.status.success());
        let native = String::from_utf8(output.stdout).unwrap();
        let native = native.strip_suffix('\n').unwrap();
        assert_eq!(expanded.value() == native, equivalent);
        observations.push(serde_json::json!({"input":text,"pinned_output":expanded.value(),"native_output":native,"equivalent":equivalent,"numeric_scope":"exercised stable literal matrix"}));
    }
    fs::write(root.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"repaired pinned concat literal matrix; no complete error/nested/guest claim","observations":observations})).unwrap()).unwrap();
}

#[test]
fn actual_native_concat_numeric_overflow_and_invalid_suffixes_are_rejected() {
    use lrr::{
        expand::{compile_input_identity::descriptor, concat},
        syntax::token_wire::TokenTree,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-concat-errors-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&root).unwrap();
    let d = descriptor("concat").unwrap().unwrap();
    let mut observations = vec![];
    for (index, text, marker) in [
        (
            0,
            "(340282366920938463463374607431768211456)",
            "integer literal is too large",
        ),
        (1, "(1bad)", "invalid suffix"),
        (2, "(1.0u8)", "invalid suffix"),
    ] {
        let input = argument_tokens(text);
        let [TokenTree::Group { open, .. }] = input.roots() else {
            panic!("missing arguments")
        };
        let error =
            match concat::literal_value(&d, &input, *open, 64 * 1024 * 1024, 32 * 1024 * 1024) {
                Err(error) => error,
                Ok(_) => panic!("invalid numeric concat returned a value"),
            };
        assert_eq!(error.code, "LRR-E-COMPILE-INPUT");
        let path = root.join(format!("oracle-{index}.rs"));
        fs::write(&path, format!("fn main(){{let _=concat!{text};}}")).unwrap();
        let compiled = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name",
                "concat_error_oracle",
            ])
            .arg(&path)
            .arg("-o")
            .arg(root.join(format!("native-{index}")))
            .output()
            .unwrap();
        assert!(!compiled.status.success());
        let stderr = String::from_utf8(compiled.stderr).unwrap();
        assert!(stderr.contains(marker), "{stderr}");
        fs::write(root.join(format!("native-{index}.stderr")), &stderr).unwrap();
        observations.push(serde_json::json!({"input":text,"runtime_code":error.code,"runtime_message":error.message,"native_stderr":stderr}));
    }
    fs::write(root.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"numeric concat errors; no full literal/guest claim","observations":observations})).unwrap()).unwrap();
}

#[test]
fn include_token_paths_preserve_literal_spans_suspend_eager_macros_and_bind_identity() {
    use lrr::{
        expand::{
            compile_input_identity::descriptor,
            include_arguments::{IncludeArguments, IncludeOperand, IncludePathPoll},
        },
        source::{
            include_paths::{IncludeAuthorities, IncludeLocation, IncludeRoot, IncludeRootKind},
            snapshot::SourcePath,
        },
        syntax::token_wire::Key,
    };
    let unit = Digest([11; 32]);
    let root = Digest([61; 32]);
    let authorities = IncludeAuthorities::new(
        unit,
        Digest([62; 32]),
        vec![IncludeRoot {
            identity: root,
            path: PathBuf::from("/controlled/source"),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap();
    let from = IncludeLocation {
        unit,
        root,
        path: SourcePath::new("src/main.rs").unwrap(),
        origin: Key([63; 32]),
    };
    let text = r##"(r#"nested/file.rs"#,)"##;
    let tokens = argument_tokens(text);
    for name in ["include", "include_str", "include_bytes"] {
        let definition = descriptor(name).unwrap().unwrap();
        let args = IncludeArguments::parse(&definition, &tokens).unwrap();
        let IncludeOperand::Literal { value, span } = args.operand() else {
            panic!("literal did not decode")
        };
        assert_eq!(value, "nested/file.rs");
        let position = tokens.spans().iter().find(|r| r.key() == *span).unwrap();
        assert_eq!(
            &text[position.start as usize..position.end as usize],
            r##"r#"nested/file.rs"#"##
        );
        let IncludePathPoll::Capture(request) = args
            .route(&definition, &authorities, &from, Key([64; 32]))
            .unwrap()
        else {
            panic!("path did not route")
        };
        assert_eq!(request.path().as_str(), "src/nested/file.rs");
        assert_eq!(
            args.fingerprint(),
            IncludeArguments::parse(&definition, &tokens)
                .unwrap()
                .fingerprint()
        );
        assert!(
            args.route(
                &descriptor("env").unwrap().unwrap(),
                &authorities,
                &from,
                Key([64; 32])
            )
            .is_err()
        );
        let nested = IncludeArguments::parse(
            &definition,
            &argument_tokens(r#"(concat!(env!("OUT_DIR"),"/generated.rs"))"#),
        )
        .unwrap();
        assert!(matches!(
            nested
                .route(&definition, &authorities, &from, Key([64; 32]))
                .unwrap(),
            IncludePathPoll::NeedExpansion { .. }
        ));
    }
    let definition = descriptor("include").unwrap().unwrap();
    for invalid in [
        r#"()"#,
        r#"("a","b")"#,
        r#"("a",,)"#,
        r#"(b"a")"#,
        r#"(true)"#,
        r#"("\q")"#,
        r#"("a"suffix)"#,
    ] {
        assert!(
            IncludeArguments::parse(&definition, &argument_tokens(invalid)).is_err(),
            "{invalid}"
        );
    }
    assert!(IncludeArguments::parse(&descriptor("env").unwrap().unwrap(), &tokens).is_err());
}

#[test]
fn actual_native_include_token_paths_match_captured_syntax_utf8_and_bytes() {
    use lrr::{
        expand::{
            compile_input_identity::descriptor,
            include_arguments::{IncludeArguments, IncludePathPoll},
        },
        service::source_capture::SourceCapture,
        source::{
            include_paths::{IncludeAuthorities, IncludeLocation, IncludeRoot, IncludeRootKind},
            snapshot::SourcePath,
        },
        syntax::token_wire::Key,
    };
    let temp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-include-operands-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&temp).unwrap();
    let temp = fs::canonicalize(temp).unwrap();
    fs::write(temp.join("items.rs"), b"31").unwrap();
    fs::write(temp.join("text-é.txt"), "🙂").unwrap();
    fs::write(temp.join("binary.bin"), [255u8, 0]).unwrap();
    let unit = Digest([11; 32]);
    let root = Digest([65; 32]);
    let authorities = IncludeAuthorities::new(
        unit,
        Digest([66; 32]),
        vec![IncludeRoot {
            identity: root,
            path: temp.clone(),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap();
    let from = IncludeLocation {
        unit,
        root,
        path: SourcePath::new("oracle.rs").unwrap(),
        origin: Key([67; 32]),
    };
    let mut requests = Vec::new();
    for (name, operand) in [
        ("include", r##"(r#"items.rs"#,)"##),
        ("include_str", r#"("text-\u{e9}.txt")"#),
        ("include_bytes", r#"("binary.bin")"#),
    ] {
        let d = descriptor(name).unwrap().unwrap();
        let args = IncludeArguments::parse(&d, &argument_tokens(operand)).unwrap();
        let IncludePathPoll::Capture(request) =
            args.route(&d, &authorities, &from, Key([68; 32])).unwrap()
        else {
            panic!("path did not route")
        };
        requests.push(request);
    }
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let records = requests
        .iter()
        .map(|r| capture.capture(r).unwrap())
        .collect::<Vec<_>>();
    let expression = std::str::from_utf8(records[0].blob().bytes())
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let text = std::str::from_utf8(records[1].blob().bytes()).unwrap();
    let bytes = records[2].blob().bytes();
    let expected = format!("{expression}|{text}|{}:{}\n", bytes[0], bytes[1]);
    fs::write(temp.join("oracle.rs"),r###"fn main(){let value:i64=include!(r#"items.rs"#,);let text=include_str!("text-\u{e9}.txt");let bytes=include_bytes!("binary.bin");println!("{value}|{text}|{}:{}",bytes[0],bytes[1]);}"###).unwrap();
    let build = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(temp.join("oracle.rs"))
        .arg("-o")
        .arg(temp.join("oracle"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(temp.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout, expected);
    fs::write(temp.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"captured_stdout":expected,"scope":"original include token operands and coordinator capture; no eager expansion or guest claim"})).unwrap()).unwrap();
}

#[test]
fn nested_include_paths_resume_only_exact_adopted_expansion_and_preserve_original_tokens() {
    use lrr::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        db::{
            key::{ConstantKey, QueryKey},
            memo::Database,
            poll::{ExpansionRequest, InputRequest, QueryPoll},
        },
        expand::{
            compile_input_identity::descriptor,
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
            include_arguments::{IncludeArguments, IncludePathPoll},
        },
        frontend::compile_input_catalog::CompileInputCatalog,
        service::expansion::{
            DemandContext, ExpansionCoordinator, PhaseIdentity, Reply, StartDecision,
        },
        source::{
            include_paths::{IncludeAuthorities, IncludeLocation, IncludeRoot, IncludeRootKind},
            manifest::BodyKey,
            snapshot::SourcePath,
        },
        syntax::token_wire::Key,
    };
    let unit = Digest([11; 32]);
    let catalog = CompileInputCatalog::new(
        Digest([91; 32]),
        2024,
        [descriptor("concat").unwrap().unwrap()],
    )
    .unwrap();
    let selected = catalog.definitions().next().unwrap();
    let builtin = catalog.select(selected).unwrap().unwrap();
    let include = descriptor("include").unwrap().unwrap();
    let args = IncludeArguments::parse(
        &include,
        &argument_tokens(r#"(core::r#concat!("generated", ".rs"),)"#),
    )
    .unwrap();
    let nested = args.nested_invocation().unwrap();
    assert_eq!(nested.segments(), ["core", "r#concat"]);
    assert!(!nested.absolute());
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Builtin {
            definition: selected.identity,
        },
        invocation: nested.identity(),
        input_tokens: nested.argument_identity(),
        consumer_compile_env: Digest([92; 32]),
        parent_context: Digest([93; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Expression,
        engine_version: Digest([94; 32]),
        procedural_epoch: None,
    };
    let mut db = Database::default();
    assert!(matches!(
        args.resume_adopted(&mut db, &include, &key),
        QueryPoll::NeedInput(InputRequest::Expand(_))
    ));
    let mut wrong = key.clone();
    wrong.input_tokens = Digest([95; 32]);
    assert!(
        args.resume_adopted(&mut db, &include, &wrong)
            .into_ready()
            .unwrap()
            .is_err()
    );
    let first = nested.arguments().spans()[0].key();
    let value = lrr::expand::concat::literal_value(
        builtin,
        nested.arguments(),
        first,
        64 * 1024 * 1024,
        32 * 1024 * 1024,
    )
    .unwrap();
    let record = value
        .expansion_record(
            &catalog,
            selected,
            &key,
            nested.arguments(),
            256 * 1024 * 1024,
        )
        .unwrap();
    let context = DemandContext {
        request: Digest([96; 32]),
        session: Digest([97; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: Digest([98; 32]),
        native_generation: None,
        compile_environment: key.consumer_compile_env,
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let query = QueryKey::Constant(ConstantKey {
        definition: Digest([99; 32]),
        arguments: vec![],
        cfg: Digest::default(),
        profile: ProfileIdentity::current(),
    });
    let phase = PhaseIdentity {
        frame: Digest([100; 32]),
        body: BodyKey {
            definition: Digest([101; 32]),
            exact_body_bytes: Digest([102; 32]),
        },
        index: 0,
    };
    let token = coordinator
        .issue(
            query,
            phase,
            InputRequest::Expand(Box::new(ExpansionRequest { key: key.clone() })),
        )
        .unwrap();
    assert_eq!(
        coordinator.start(&context, &token).unwrap(),
        StartDecision::Start
    );
    coordinator
        .accept(&context, &token, Reply::success(&token, record.output()))
        .unwrap();
    assert!(matches!(
        args.resume_adopted(&mut db, &include, &key),
        QueryPoll::NeedInput(_)
    ));
    coordinator
        .adopt(&context, &token, &mut db, record)
        .unwrap();
    let resumed = args
        .resume_adopted(&mut db, &include, &key)
        .into_ready()
        .unwrap()
        .unwrap();
    assert_ne!(resumed.fingerprint(), args.fingerprint());
    let root = Digest([103; 32]);
    let authorities = IncludeAuthorities::new(
        unit,
        context.source_generation,
        vec![IncludeRoot {
            identity: root,
            path: PathBuf::from("/controlled/source"),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap();
    let from = IncludeLocation {
        unit,
        root,
        path: SourcePath::new("main.rs").unwrap(),
        origin: Key([104; 32]),
    };
    let IncludePathPoll::Capture(request) = resumed
        .route(&include, &authorities, &from, Key([105; 32]))
        .unwrap()
    else {
        panic!("resumed path did not route")
    };
    assert_eq!(request.path().as_str(), "generated.rs");
    assert_eq!(
        key.input_tokens,
        Digest::of_bytes(&nested.arguments().encode().unwrap())
    );
    assert_eq!(key.fingerprint(), key.clone().fingerprint());
    db.publish_inputs(BTreeMap::new()).unwrap();
    assert!(matches!(
        args.resume_adopted(&mut db, &include, &key),
        QueryPoll::NeedInput(_)
    ));
}

#[test]
fn nested_compile_input_resolution_preserves_alias_identity_user_shadowing_and_frontiers() {
    nested_resolution_fixture(false);
}
#[test]
fn nested_expansion_keys_bind_resolved_definition_original_context_and_consumer_environment() {
    nested_resolution_fixture(true);
}
fn nested_resolution_fixture(bind_keys: bool) {
    use lrr::{
        core::profile::ProfileIdentity,
        expand::{
            compile_input_identity::descriptor,
            include_arguments::IncludeArguments,
            scopes::{MacroDefinition, MacroName, ScopeKey as TextualScope, TextualScopes},
        },
        frontend::{
            compile_input_catalog::CompileInputCatalog,
            hygiene::HygieneFacts,
            macro_names::{MacroNamespaces, MacroPath},
            macro_origins::ExpansionAncestry,
            macro_scopes::MacroDeclarations,
            nested_compile_inputs::{
                NestedCompileInputResolver, NestedLexicalContext, NestedMacroLookup,
            },
        },
        syntax::{outline::ScopeKey, parse::ParseKey, token_wire::Key},
    };
    let unit = Digest([11; 32]);
    let core = Digest([111; 32]);
    let catalog =
        CompileInputCatalog::new(core, 2024, [descriptor("concat").unwrap().unwrap()]).unwrap();
    let compiler = catalog.definitions().next().unwrap().clone();
    let mut names = MacroNamespaces::default();
    let path = |unit, name: &str| {
        MacroPath::new(unit, vec![MacroName::parse(name, 2024).unwrap()]).unwrap()
    };
    names
        .define(path(core, "concat"), compiler.clone())
        .unwrap();
    names
        .dependency(unit, MacroName::parse("core", 2024).unwrap(), core)
        .unwrap();
    names
        .reexport(path(unit, "joined"), path(core, "concat"))
        .unwrap();
    let user = MacroDefinition {
        identity: Digest([112; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    names.define(path(unit, "concat"), user.clone()).unwrap();
    for (text, expected) in [
        (r#"(joined!("x"),)"#, true),
        (r#"(::core::r#concat!("x"))"#, true),
        (r#"(concat!("x"))"#, false),
    ] {
        let input = argument_tokens(text);
        let args =
            IncludeArguments::parse(&descriptor("include").unwrap().unwrap(), &input).unwrap();
        let nested = args.nested_invocation().unwrap();
        let span = nested
            .arguments()
            .spans()
            .iter()
            .find(|s| s.key() == nested.reference_span())
            .unwrap();
        let mut hygiene = lrr_expand_adapter::hygiene::HygieneStore::default();
        let adapter_contexts = input
            .contexts()
            .iter()
            .map(|c| serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap())
            .collect::<Vec<_>>();
        hygiene.import(&adapter_contexts).unwrap();
        let rows = hygiene
            .resolution_contexts(&[lrr_expand_adapter::token_wire::Key(span.context.0)])
            .unwrap();
        let rows = serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap();
        let comparisons = hygiene
            .name_contexts(&[lrr_expand_adapter::token_wire::Key(span.context.0)])
            .unwrap();
        let comparisons =
            serde_json::from_slice(&serde_json::to_vec(&comparisons).unwrap()).unwrap();
        let facts = HygieneFacts::new(input.contexts().to_vec(), rows)
            .unwrap()
            .with_name_contexts(comparisons)
            .unwrap();
        let root = TextualScope::root(unit);
        let mut scopes = TextualScopes::default();
        scopes.add_root(root).unwrap();
        let mut declarations = MacroDeclarations::default();
        let ancestry = ExpansionAncestry::default();
        let caller = NestedLexicalContext {
            source: ParseKey {
                content: Digest(span.source.content.0),
                profile: ProfileIdentity::current(),
            },
            scope: ScopeKey {
                crate_key: unit,
                module_path: vec![],
                owner: None,
            },
            textual_scope: root,
            origin: None,
        };
        let resolve = |scopes: &TextualScopes,
                       declarations: &MacroDeclarations,
                       caller: &NestedLexicalContext| {
            NestedCompileInputResolver {
                namespaces: &names,
                textual: scopes,
                declarations,
                hygiene: &facts,
                ancestry: &ancestry,
                compiler: &catalog,
            }
            .resolve(nested, caller)
        };
        if bind_keys {
            use lrr::expand::identity::{GrammarSlot, MacroIdentity};
            use lrr::frontend::nested_compile_inputs::NestedExpansionContext;
            let resolver = NestedCompileInputResolver {
                namespaces: &names,
                textual: &scopes,
                declarations: &declarations,
                hygiene: &facts,
                ancestry: &ancestry,
                compiler: &catalog,
            };
            let compile_env = environment(unit);
            let frozen = FrozenCompileEnvironment::new(unit, &compile_env).unwrap();
            let context = NestedExpansionContext {
                environment: &frozen,
                engine_version: Digest([202; 32]),
            };
            let bound = resolver
                .resolve_expansion(nested, &caller, &context)
                .unwrap();
            if expected {
                assert!(
                    matches!(bound.lookup(),NestedMacroLookup::Compiler{definition,..} if definition==&compiler)
                );
                let key = bound.key().unwrap();
                key.validate().unwrap();
                assert_eq!(
                    key.macro_identity,
                    MacroIdentity::Builtin {
                        definition: compiler.identity
                    }
                );
                assert_eq!(key.invocation, nested.identity());
                assert_eq!(key.input_tokens, nested.argument_identity());
                assert_eq!(key.parent_context, Digest(span.context.0));
                assert_eq!(key.consumer_compile_env, context.environment.fingerprint());
                assert_eq!(key.definition_edition, compiler.definition_edition);
                assert_eq!(key.call_edition, 2024);
                assert_eq!(key.grammar, GrammarSlot::Expression);
                assert!(key.procedural_epoch.is_none());
                let mut changed_env = compile_env.clone();
                changed_env
                    .declared
                    .insert("CHANGED".into(), Some("value".into()));
                let changed_frozen = FrozenCompileEnvironment::new(unit, &changed_env).unwrap();
                let changed = NestedExpansionContext {
                    environment: &changed_frozen,
                    engine_version: context.engine_version,
                };
                assert_ne!(
                    resolver
                        .resolve_expansion(nested, &caller, &changed)
                        .unwrap()
                        .key()
                        .unwrap(),
                    key
                );
            } else {
                assert!(
                    matches!(bound.lookup(),NestedMacroLookup::Other(definition) if definition==&user)
                );
                let key = bound.key().expect("owned declarative demand key");
                assert!(
                    matches!(&key.macro_identity,lrr::expand::identity::MacroIdentity::Declaration{definition} if *definition==user.identity)
                );
                assert_eq!(key.grammar, lrr::expand::identity::GrammarSlot::Expression);
            }
            let mut wrong = NestedLexicalContext {
                source: caller.source.clone(),
                scope: caller.scope.clone(),
                textual_scope: root,
                origin: None,
            };
            wrong.scope.crate_key = Digest([204; 32]);
            assert!(
                resolver
                    .resolve_expansion(nested, &wrong, &context)
                    .is_err()
            );
        }
        let selected = resolve(&scopes, &declarations, &caller).unwrap();
        if expected {
            assert!(
                matches!(selected,NestedMacroLookup::Compiler{definition,..} if definition==compiler)
            );
        } else {
            assert!(matches!(selected,NestedMacroLookup::Other(definition) if definition==user));
        }
        let mut wrong = NestedLexicalContext {
            source: caller.source.clone(),
            scope: caller.scope.clone(),
            textual_scope: root,
            origin: None,
        };
        wrong.source.content = Digest([113; 32]);
        assert!(resolve(&scopes, &declarations, &wrong).is_err());
        if !nested.absolute() && nested.segments().len() == 1 {
            declarations
                .register(user.clone(), Key(span.context.0), None, &facts, &ancestry)
                .unwrap();
            scopes
                .define(
                    root,
                    0,
                    MacroName::parse(&nested.segments()[0], 2024).unwrap(),
                    user.clone(),
                )
                .unwrap();
            if span.start > 0 {
                assert!(
                    matches!(resolve(&scopes,&declarations,&caller).unwrap(),NestedMacroLookup::Other(definition) if definition==user)
                );
            }
        }
        let mut blocked = TextualScopes::default();
        blocked.add_root(root).unwrap();
        blocked.block(root, 0, Digest([114; 32])).unwrap();
        assert!(
            matches!(resolve(&blocked,&declarations,&caller).unwrap(),NestedMacroLookup::NeedInput(input) if input==Digest([114;32]))
        );
    }
}

#[test]
fn actual_concat_string_emission_preserves_spans_native_values_and_exact_wire_budget() {
    use lrr::{
        expand::{
            compile_input_identity::descriptor,
            environment_arguments::{EnvironmentArguments, StringOperand},
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        },
        frontend::compile_input_catalog::CompileInputCatalog,
        syntax::token_wire::{Key, TokenTree},
    };
    let catalog = CompileInputCatalog::new(
        Digest([121; 32]),
        2024,
        [descriptor("concat").unwrap().unwrap()],
    )
    .unwrap();
    let selected = catalog.definitions().next().unwrap();
    let builtin = catalog.select(selected).unwrap().unwrap();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-string-emission-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&path).unwrap();
    let mut observations = Vec::new();
    for (index, text) in ["", "quote\"slash\\\n\t\r\0", "🙂é\u{85}", "a'b"]
        .into_iter()
        .enumerate()
    {
        let input = argument_tokens(&format!("({text:?})"));
        let site = input.spans()[0].key();
        let value = lrr::expand::concat::literal_value(
            builtin,
            &input,
            site,
            64 * 1024 * 1024,
            32 * 1024 * 1024,
        )
        .unwrap();
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Builtin {
                definition: selected.identity,
            },
            invocation: Digest([122; 32]),
            input_tokens: Digest::of_bytes(&input.encode().unwrap()),
            consumer_compile_env: Digest([123; 32]),
            parent_context: Digest([124; 32]),
            definition_edition: 2024,
            call_edition: 2024,
            grammar: GrammarSlot::Expression,
            engine_version: Digest([125; 32]),
            procedural_epoch: None,
        };
        let record = value
            .expansion_record(&catalog, selected, &key, &input, 256 * 1024 * 1024)
            .unwrap();
        let tokens = record.tokens().unwrap();
        let bytes = tokens.encode().unwrap().len();
        assert!(
            value
                .expansion_record(&catalog, selected, &key, &input, bytes - 1)
                .is_err()
        );
        assert_eq!(
            value
                .expansion_record(&catalog, selected, &key, &input, bytes)
                .unwrap()
                .output(),
            record.output()
        );
        let [
            TokenTree::Group {
                open,
                close,
                children,
                ..
            },
        ] = tokens.roots()
        else {
            panic!("emitted group absent")
        };
        assert_eq!(*open, site);
        assert_eq!(*close, site);
        assert!(matches!(children.as_slice(),[TokenTree::Literal{span,..}] if *span==site));
        assert_eq!(
            tokens.spans().iter().find(|r| r.key() == site),
            input.spans().iter().find(|r| r.key() == site)
        );
        let parsed =
            EnvironmentArguments::parse(&descriptor("option_env").unwrap().unwrap(), &tokens)
                .unwrap();
        let StringOperand::Literal { value: decoded, .. } = parsed.name() else {
            panic!("emitted value did not decode")
        };
        assert_eq!(decoded, text);
        let mut wrong = key.clone();
        wrong.input_tokens = Digest([126; 32]);
        assert!(
            value
                .expansion_record(&catalog, selected, &wrong, &input, bytes)
                .is_err()
        );
        let forged = lrr::expand::scopes::MacroDefinition {
            identity: selected.identity,
            defining_unit: Digest([127; 32]),
            definition_edition: 2024,
        };
        assert!(
            value
                .expansion_record(&catalog, &forged, &key, &input, bytes)
                .is_err()
        );
        fs::write(
            path.join("oracle.rs"),
            format!("fn main(){{println!(\"{{:?}}\",concat!({text:?}));}}"),
        )
        .unwrap();
        let binary = path.join(format!("oracle-{index}"));
        let build = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024"])
            .arg(path.join("oracle.rs"))
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        let native = Command::new(binary).output().unwrap();
        assert!(native.status.success());
        let stdout = String::from_utf8(native.stdout).unwrap();
        assert_eq!(stdout, format!("{decoded:?}\n"));
        observations.push(serde_json::json!({"native_stdout":stdout,"wire_bytes":bytes,"output":record.output().hex(),"span":Digest(site.0).hex()}));
        assert!(
            lrr_expand_adapter::string_output::emit(
                &lrr_expand_adapter::token_wire::TokenForest::decode(&input.encode().unwrap())
                    .unwrap(),
                lrr_expand_adapter::token_wire::Key(Key([128; 32]).0),
                text,
                256 * 1024 * 1024
            )
            .is_err()
        );
    }
    fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","observations":observations,"scope":"pinned concat string emission and exact wire budget; no recursive eager/guest claim"})).unwrap()).unwrap();
}

#[test]
fn environment_output_emits_frozen_values_and_preserves_absence_nested_and_context_demands() {
    use lrr::{
        expand::{
            compile_input_identity::descriptor,
            environment_arguments::{
                EnvironmentArguments, EnvironmentExpansion, EnvironmentExpansionPoll, StringOperand,
            },
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        },
        frontend::compile_input_catalog::CompileInputCatalog,
    };
    let unit = Digest([11; 32]);
    let catalog = CompileInputCatalog::new(
        Digest([131; 32]),
        2024,
        [
            descriptor("env").unwrap().unwrap(),
            descriptor("option_env").unwrap().unwrap(),
        ],
    )
    .unwrap();
    let selected = catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap().unwrap().name() == "env")
        .unwrap();
    let optional = catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap().unwrap().name() == "option_env")
        .unwrap();
    let mut input = environment(unit);
    input
        .declared
        .insert("VALUE".into(), Some("quote\" 🙂\n".into()));
    input.declared.insert("ABSENT".into(), None);
    let frozen = FrozenCompileEnvironment::new(unit, &input).unwrap();
    let tokens = argument_tokens(r#"("VALUE")"#);
    let site = tokens.spans()[0].key();
    let key = |selected: &lrr::expand::scopes::MacroDefinition,
               tokens: &lrr::syntax::token_wire::TokenForest| ExpansionKey {
        macro_identity: MacroIdentity::Builtin {
            definition: selected.identity,
        },
        invocation: Digest([132; 32]),
        input_tokens: Digest::of_bytes(&tokens.encode().unwrap()),
        consumer_compile_env: frozen.fingerprint(),
        parent_context: Digest([133; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Expression,
        engine_version: Digest([134; 32]),
        procedural_epoch: None,
    };
    let expand = |selected: &lrr::expand::scopes::MacroDefinition,
                  tokens: &lrr::syntax::token_wire::TokenForest,
                  key: &ExpansionKey| {
        frozen.expand_environment(EnvironmentExpansion {
            catalog: &catalog,
            selected,
            key,
            original: tokens,
            site: tokens.spans()[0].key(),
            max_wire_bytes: 256 * 1024 * 1024,
        })
    };
    let environment_key = key(selected, &tokens);
    let EnvironmentExpansionPoll::Ready(Ok(record)) =
        expand(selected, &tokens, &environment_key).unwrap()
    else {
        panic!("frozen env value did not emit")
    };
    let decoded = EnvironmentArguments::parse(
        &descriptor("option_env").unwrap().unwrap(),
        &record.tokens().unwrap(),
    )
    .unwrap();
    assert!(
        matches!(decoded.name(),StringOperand::Literal{value,span} if value=="quote\" 🙂\n" && *span==site)
    );
    let evidence = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-env-emission-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&evidence).unwrap();
    fs::write(
        evidence.join("oracle.rs"),
        "fn main(){println!(\"{:?}\",env!(\"VALUE\"));}",
    )
    .unwrap();
    let binary = evidence.join("oracle");
    let build = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .env("VALUE", "quote\" 🙂\n")
        .arg(evidence.join("oracle.rs"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(binary).output().unwrap();
    assert!(native.status.success());
    let StringOperand::Literal { value: emitted, .. } = decoded.name() else {
        panic!("emitted environment string did not decode")
    };
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout, format!("{emitted:?}\n"));
    fs::write(evidence.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"output":record.output().hex(),"scope":"frozen env string emission/native comparison; no option facade/recursive eager/guest claim"})).unwrap()).unwrap();
    let mut changed = input.clone();
    changed
        .declared
        .insert("VALUE".into(), Some("changed".into()));
    let next = FrozenCompileEnvironment::new(unit, &changed).unwrap();
    assert!(
        next.expand_environment(EnvironmentExpansion {
            catalog: &catalog,
            selected,
            key: &environment_key,
            original: &tokens,
            site,
            max_wire_bytes: 256 * 1024 * 1024
        })
        .is_err()
    );
    let mut wrong = environment_key.clone();
    wrong.definition_edition = 2021;
    assert!(expand(selected, &tokens, &wrong).is_err());
    for (text, kind) in [
        (r#"("ABSENT","custom missing")"#, 0),
        (r#"("UNKNOWN")"#, 1),
        (r#"(concat!("VA","LUE"))"#, 2),
    ] {
        let tokens = argument_tokens(text);
        let outcome = expand(selected, &tokens, &key(selected, &tokens)).unwrap();
        match kind {
            0 => assert!(
                matches!(outcome,EnvironmentExpansionPoll::Ready(Err(e)) if e.message=="custom missing")
            ),
            1 => assert!(
                matches!(outcome,EnvironmentExpansionPoll::NeedCapture{unit:u,name,..} if u==unit&&name=="UNKNOWN")
            ),
            _ => assert!(matches!(
                outcome,
                EnvironmentExpansionPoll::NeedExpansion { operand: 0, .. }
            )),
        }
    }
    for text in [r#"("VALUE")"#, r#"("ABSENT")"#] {
        let tokens = argument_tokens(text);
        let expected = if text.contains("VALUE") {
            Some("quote\" 🙂\n".to_owned())
        } else {
            None
        };
        assert!(
            matches!(expand(optional,&tokens,&key(optional,&tokens)).unwrap(),EnvironmentExpansionPoll::NeedOptionFacade{defining_unit,value,..} if defining_unit==optional.defining_unit&&value==expected)
        );
    }
}

#[test]
fn concat_operand_plans_preserve_literals_nested_calls_order_and_frozen_env_emission() {
    use lrr::{
        expand::{
            compile_input_identity::descriptor,
            concat_arguments::ConcatArguments,
            environment_arguments::{
                EnvironmentArguments, EnvironmentExpansion, EnvironmentExpansionPoll, StringOperand,
            },
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        },
        frontend::compile_input_catalog::CompileInputCatalog,
    };
    let concat = descriptor("concat").unwrap().unwrap();
    let source = r#"(env!("OUT_DIR"), "/generated.rs", alias::pick!(), 0x10u8,)"#;
    let input = argument_tokens(source);
    let plan = ConcatArguments::parse(&concat, &input).unwrap();
    assert_eq!(plan.operands().len(), 4);
    assert_eq!(
        plan.fingerprint(),
        ConcatArguments::parse(&concat, &input)
            .unwrap()
            .fingerprint()
    );
    let (index, nested) = plan.next_expansion().unwrap();
    assert_eq!(index, 0);
    assert_eq!(nested.segments(), ["env"]);
    assert_eq!(
        nested.argument_identity(),
        Digest::of_bytes(&nested.arguments().encode().unwrap())
    );
    assert_eq!(
        plan.operands()[2].nested().unwrap().segments(),
        ["alias", "pick"]
    );
    let literal = &plan.operands()[3];
    let site = literal.tokens().spans()[0].key();
    assert_eq!(
        lrr::expand::concat::literal_value(
            &concat,
            literal.tokens(),
            site,
            64 * 1024 * 1024,
            32 * 1024 * 1024
        )
        .unwrap()
        .value(),
        "16"
    );
    assert!(
        ConcatArguments::parse(&concat, &argument_tokens("()"))
            .unwrap()
            .operands()
            .is_empty()
    );
    let repeated =
        ConcatArguments::parse(&concat, &argument_tokens(r#"(env!("A"),env!("A"))"#)).unwrap();
    assert_ne!(
        repeated.operands()[0].nested().unwrap().identity(),
        repeated.operands()[1].nested().unwrap().identity()
    );
    for invalid in [
        r#"(,)"#,
        r#"(true,,false)"#,
        r#"("a" "b")"#,
        r#"(b"bad")"#,
        r#"(1bad)"#,
    ] {
        assert!(
            ConcatArguments::parse(&concat, &argument_tokens(invalid)).is_err(),
            "{invalid}"
        );
    }
    assert!(ConcatArguments::parse(&descriptor("env").unwrap().unwrap(), &input).is_err());
    let unit = Digest([11; 32]);
    let mut env = environment(unit);
    env.declared
        .insert("OUT_DIR".into(), Some("/controlled/out".into()));
    let frozen = FrozenCompileEnvironment::new(unit, &env).unwrap();
    let catalog = CompileInputCatalog::new(
        Digest([171; 32]),
        2024,
        [descriptor("env").unwrap().unwrap()],
    )
    .unwrap();
    let selected = catalog.definitions().next().unwrap();
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Builtin {
            definition: selected.identity,
        },
        invocation: nested.identity(),
        input_tokens: nested.argument_identity(),
        consumer_compile_env: frozen.fingerprint(),
        parent_context: Digest([172; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Expression,
        engine_version: Digest([173; 32]),
        procedural_epoch: None,
    };
    let EnvironmentExpansionPoll::Ready(Ok(record)) = frozen
        .expand_environment(EnvironmentExpansion {
            catalog: &catalog,
            selected,
            key: &key,
            original: nested.arguments(),
            site: nested.reference_span(),
            max_wire_bytes: 256 * 1024 * 1024,
        })
        .unwrap()
    else {
        panic!("planned env child did not emit")
    };
    let decoded = EnvironmentArguments::parse(
        &descriptor("option_env").unwrap().unwrap(),
        &record.tokens().unwrap(),
    )
    .unwrap();
    assert!(
        matches!(decoded.name(),StringOperand::Literal{value,span} if value=="/controlled/out" && *span==nested.reference_span())
    );
}

#[test]
fn actual_concat_adopted_env_join_matches_native_and_tracks_child_input_invalidation() {
    concat_join_fixture(false);
}
#[test]
fn actual_eager_concat_parent_record_preserves_original_key_and_matches_native_literal() {
    concat_join_fixture(true);
}
fn concat_join_fixture(emit_parent: bool) {
    use lrr::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        db::{
            dependency::{DependencyStamp, InputKey},
            key::{ConstantKey, QueryKey},
            memo::Database,
            poll::{ExpansionRequest, InputRequest, QueryPoll},
            provider::{Provided, Provider},
            value::QueryValue,
        },
        expand::{
            compile_input_identity::descriptor,
            concat_arguments::{
                ConcatArguments, ConcatExpansion, ConcatExpansionPoll, ConcatMaterializationPoll,
            },
            environment_arguments::{EnvironmentExpansion, EnvironmentExpansionPoll},
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        },
        frontend::compile_input_catalog::CompileInputCatalog,
        service::expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply},
        source::manifest::BodyKey,
    };
    let unit = Digest([11; 32]);
    let concat = descriptor("concat").unwrap().unwrap();
    let input = argument_tokens(r#"(env!("OUT_DIR"),"/generated.rs",0x10u8,)"#);
    let plan = ConcatArguments::parse(&concat, &input).unwrap();
    let (_, nested) = plan.next_expansion().unwrap();
    let mut env = environment(unit);
    env.declared
        .insert("OUT_DIR".into(), Some("/controlled/out".into()));
    let frozen = FrozenCompileEnvironment::new(unit, &env).unwrap();
    let catalog = CompileInputCatalog::new(
        Digest([181; 32]),
        2024,
        [
            descriptor("env").unwrap().unwrap(),
            descriptor("concat").unwrap().unwrap(),
        ],
    )
    .unwrap();
    let selected = catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap().unwrap().name() == "env")
        .unwrap();
    let child = ExpansionKey {
        macro_identity: MacroIdentity::Builtin {
            definition: selected.identity,
        },
        invocation: nested.identity(),
        input_tokens: nested.argument_identity(),
        consumer_compile_env: frozen.fingerprint(),
        parent_context: Digest([182; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Expression,
        engine_version: Digest([183; 32]),
        procedural_epoch: None,
    };
    let selections = BTreeMap::from([(0, child.clone())]);
    let concat_definition = catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap().unwrap().name() == "concat")
        .unwrap();
    let mut parent_key = child.clone();
    parent_key.macro_identity = MacroIdentity::Builtin {
        definition: concat_definition.identity,
    };
    parent_key.invocation = Digest([192; 32]);
    parent_key.input_tokens = Digest::of_bytes(&input.encode().unwrap());
    let emit = |db: &mut Database, key: &ExpansionKey, max_wire_bytes: usize| {
        plan.expand_adopted(
            db,
            ConcatExpansion {
                catalog: &catalog,
                selected: concat_definition,
                key,
                original: &input,
                site: input.spans()[0].key(),
                selections: &selections,
                max_input_bytes: 64 * 1024 * 1024,
                max_output_bytes: 32 * 1024 * 1024,
                max_wire_bytes,
            },
        )
    };
    let mut db = Database::default();
    assert!(matches!(
        emit(&mut db, &parent_key, 256 * 1024 * 1024),
        ConcatExpansionPoll::NeedInput(_)
    ));
    assert!(matches!(
        plan.materialize_adopted(
            &mut db,
            &concat,
            &input,
            frozen.fingerprint(),
            &BTreeMap::new()
        ),
        ConcatMaterializationPoll::NeedSelection { operand: 0 }
    ));
    assert!(matches!(
        plan.materialize_adopted(&mut db, &concat, &input, frozen.fingerprint(), &selections),
        ConcatMaterializationPoll::NeedInput(_)
    ));
    let wrong = BTreeMap::from([(1, child.clone())]);
    assert!(matches!(
        plan.materialize_adopted(&mut db, &concat, &input, frozen.fingerprint(), &wrong),
        ConcatMaterializationPoll::Ready(Err(_))
    ));
    let mut wrong = child.clone();
    wrong.consumer_compile_env = Digest([184; 32]);
    assert!(matches!(
        plan.materialize_adopted(
            &mut db,
            &concat,
            &input,
            frozen.fingerprint(),
            &BTreeMap::from([(0, wrong)])
        ),
        ConcatMaterializationPoll::Ready(Err(_))
    ));
    let EnvironmentExpansionPoll::Ready(Ok(record)) = frozen
        .expand_environment(EnvironmentExpansion {
            catalog: &catalog,
            selected,
            key: &child,
            original: nested.arguments(),
            site: nested.reference_span(),
            max_wire_bytes: 256 * 1024 * 1024,
        })
        .unwrap()
    else {
        panic!("env child did not emit")
    };
    let context = DemandContext {
        request: Digest([185; 32]),
        session: Digest([186; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: Digest([187; 32]),
        native_generation: None,
        compile_environment: frozen.fingerprint(),
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let query = QueryKey::Constant(ConstantKey {
        definition: Digest([188; 32]),
        arguments: vec![],
        cfg: Digest::default(),
        profile: ProfileIdentity::current(),
    });
    let phase = PhaseIdentity {
        frame: Digest([189; 32]),
        body: BodyKey {
            definition: Digest([190; 32]),
            exact_body_bytes: Digest([191; 32]),
        },
        index: 0,
    };
    let token = coordinator
        .issue(
            query.clone(),
            phase,
            InputRequest::Expand(Box::new(ExpansionRequest { key: child.clone() })),
        )
        .unwrap();
    coordinator.start(&context, &token).unwrap();
    coordinator
        .accept(&context, &token, Reply::success(&token, record.output()))
        .unwrap();
    assert!(matches!(
        plan.materialize_adopted(&mut db, &concat, &input, frozen.fingerprint(), &selections),
        ConcatMaterializationPoll::NeedInput(_)
    ));
    coordinator
        .adopt(&context, &token, &mut db, record)
        .unwrap();
    let ConcatMaterializationPoll::Ready(Ok(joined)) =
        plan.materialize_adopted(&mut db, &concat, &input, frozen.fingerprint(), &selections)
    else {
        panic!("adopted children did not join")
    };
    let site = input.spans()[0].key();
    let value = lrr::expand::concat::literal_value(
        &concat,
        &joined,
        site,
        64 * 1024 * 1024,
        32 * 1024 * 1024,
    )
    .unwrap();
    assert_eq!(
        joined
            .spans()
            .iter()
            .find(|s| s.key() == nested.reference_span()),
        input
            .spans()
            .iter()
            .find(|s| s.key() == nested.reference_span())
    );
    let emitted_record = if emit_parent {
        let ConcatExpansionPoll::Ready(Ok(record)) = emit(&mut db, &parent_key, 256 * 1024 * 1024)
        else {
            panic!("eager parent did not emit")
        };
        assert_eq!(record.key(), &parent_key);
        let output = record.tokens().unwrap();
        let bytes = output.encode().unwrap().len();
        assert!(matches!(
            emit(&mut db, &parent_key, bytes - 1),
            ConcatExpansionPoll::Ready(Err(_))
        ));
        let mut wrong = parent_key.clone();
        wrong.input_tokens = child.input_tokens;
        assert!(matches!(
            emit(&mut db, &wrong, bytes),
            ConcatExpansionPoll::Ready(Err(_))
        ));
        let token = coordinator
            .issue(
                query.clone(),
                PhaseIdentity {
                    frame: Digest([193; 32]),
                    body: BodyKey {
                        definition: Digest([194; 32]),
                        exact_body_bytes: Digest([195; 32]),
                    },
                    index: 1,
                },
                InputRequest::Expand(Box::new(ExpansionRequest {
                    key: parent_key.clone(),
                })),
            )
            .unwrap();
        coordinator.start(&context, &token).unwrap();
        coordinator
            .accept(&context, &token, Reply::success(&token, record.output()))
            .unwrap();
        coordinator
            .adopt(&context, &token, &mut db, record.clone())
            .unwrap();
        Some(record)
    } else {
        None
    };
    let label = if emit_parent { "parent" } else { "join" };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-concat-{label}-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&path).unwrap();
    let native_source = match &emitted_record {
        Some(record) => {
            let output = record.tokens().unwrap();
            let [lrr::syntax::token_wire::TokenTree::Group { children, .. }] = output.roots()
            else {
                panic!("parent output group absent")
            };
            let [lrr::syntax::token_wire::TokenTree::Literal { token_text, span }] =
                children.as_slice()
            else {
                panic!("parent output literal absent")
            };
            assert_eq!(*span, site);
            format!(
                "fn main(){{let native=concat!(env!(\"OUT_DIR\"),\"/generated.rs\",0x10u8,);let emitted={token_text};assert_eq!(native,emitted);println!(\"{{}}\",native);}}"
            )
        }
        None => {
            r#"fn main(){println!("{}",concat!(env!("OUT_DIR"),"/generated.rs",0x10u8,));}"#.into()
        }
    };
    fs::write(path.join("oracle.rs"), native_source).unwrap();
    let build = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .env("OUT_DIR", "/controlled/out")
        .arg(path.join("oracle.rs"))
        .arg("-o")
        .arg(path.join("oracle"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(path.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout, format!("{}\n", value.value()));
    struct Joined<'a> {
        plan: &'a ConcatArguments,
        descriptor: &'a lrr::expand::compile_input_identity::CompileInputDescriptor,
        original: &'a lrr::syntax::token_wire::TokenForest,
        environment: Digest,
        selections: &'a BTreeMap<usize, ExpansionKey>,
    }
    impl Provider for Joined<'_> {
        fn execute(&self, db: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
            match self.plan.materialize_adopted(
                db,
                self.descriptor,
                self.original,
                self.environment,
                self.selections,
            ) {
                ConcatMaterializationPoll::NeedInput(input) => QueryPoll::NeedInput(input),
                ConcatMaterializationPoll::NeedSelection { .. } => {
                    Provided::error("LRR-E-MACRO-SCOPE", "selection pending")
                }
                ConcatMaterializationPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                ConcatMaterializationPoll::Ready(Ok(tokens)) => {
                    let value = lrr::expand::concat::literal_value(
                        self.descriptor,
                        &tokens,
                        self.original.spans()[0].key(),
                        64 * 1024 * 1024,
                        32 * 1024 * 1024,
                    )
                    .unwrap();
                    Provided::pure(QueryValue::Constant(
                        i128::try_from(value.value().len()).unwrap(),
                    ))
                }
            }
        }
    }
    let provider = Joined {
        plan: &plan,
        descriptor: &concat,
        original: &input,
        environment: frozen.fingerprint(),
        selections: &selections,
    };
    db.fetch(&provider, query.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    assert!(db.memo(&query).unwrap().dependencies.iter().any(|d|matches!(d,DependencyStamp::Input{key:InputKey::ExpansionRecord(key),..} if *key==child.fingerprint())));
    db.publish_inputs(BTreeMap::new()).unwrap();
    assert!(matches!(
        db.fetch(&provider, query),
        QueryPoll::NeedInput(_)
    ));
    assert!(matches!(
        emit(&mut db, &parent_key, 256 * 1024 * 1024),
        ConcatExpansionPoll::NeedInput(_)
    ));
    fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"plan":plan.fingerprint().hex(),"scope":"adopted child joining and parent emission; no automatic recursive driver/guest claim", "parent_output":emitted_record.as_ref().map(|r|r.output().hex())})).unwrap()).unwrap();
}

#[test]
fn actual_environment_adopted_nested_operands_preserve_order_keys_and_native_results() {
    use lrr::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        db::{
            key::{ConstantKey, QueryKey},
            memo::Database,
            poll::{ExpansionRequest, InputRequest},
        },
        expand::{
            compile_input_identity::descriptor,
            environment_arguments::{EnvironmentExpansion, EnvironmentExpansionPoll},
            environment_plan::{
                AdoptedEnvironmentExpansionPoll, EnvironmentArgumentPlan,
                EnvironmentMaterializationPoll,
            },
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
            record::ExpansionRecord,
        },
        frontend::compile_input_catalog::CompileInputCatalog,
        service::expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply},
        source::manifest::BodyKey,
    };
    fn adopt(
        db: &mut Database,
        context: &DemandContext,
        coordinator: &mut ExpansionCoordinator,
        record: ExpansionRecord,
        index: u32,
    ) {
        let token = coordinator
            .issue(
                QueryKey::Constant(ConstantKey {
                    definition: Digest([212; 32]),
                    arguments: vec![],
                    cfg: Digest::default(),
                    profile: ProfileIdentity::current(),
                }),
                PhaseIdentity {
                    frame: Digest([213; 32]),
                    body: BodyKey {
                        definition: Digest([214; 32]),
                        exact_body_bytes: Digest([215; 32]),
                    },
                    index,
                },
                InputRequest::Expand(Box::new(ExpansionRequest {
                    key: record.key().clone(),
                })),
            )
            .unwrap();
        coordinator.start(context, &token).unwrap();
        coordinator
            .accept(context, &token, Reply::success(&token, record.output()))
            .unwrap();
        coordinator.adopt(context, &token, db, record).unwrap();
    }
    struct Materialized<'a> {
        plan: &'a EnvironmentArgumentPlan,
        descriptor: &'a lrr::expand::compile_input_identity::CompileInputDescriptor,
        original: &'a lrr::syntax::token_wire::TokenForest,
        environment: Digest,
        selections: &'a BTreeMap<usize, ExpansionKey>,
    }
    impl lrr::db::provider::Provider for Materialized<'_> {
        fn execute(
            &self,
            db: &mut Database,
            _: &QueryKey,
        ) -> lrr::db::poll::QueryPoll<lrr::db::provider::Provided> {
            use lrr::db::{poll::QueryPoll, provider::Provided, value::QueryValue};
            match self.plan.materialize_adopted(
                db,
                self.descriptor,
                self.original,
                self.environment,
                self.selections,
            ) {
                EnvironmentMaterializationPoll::NeedInput(input) => QueryPoll::NeedInput(input),
                EnvironmentMaterializationPoll::NeedSelection { .. } => {
                    Provided::error("LRR-E-MACRO-SCOPE", "selection pending")
                }
                EnvironmentMaterializationPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                EnvironmentMaterializationPoll::Ready(Ok(args)) => {
                    let lrr::expand::environment_arguments::StringOperand::Literal {
                        value, ..
                    } = args.name()
                    else {
                        panic!("name not materialized")
                    };
                    Provided::pure(QueryValue::Constant(i128::try_from(value.len()).unwrap()))
                }
            }
        }
    }
    let unit = Digest([11; 32]);
    let mut env = environment(unit);
    env.declared
        .insert("LRR_NESTED".into(), Some("captured🙂\n\"".into()));
    env.declared.insert("LRR_ABSENT".into(), None);
    let frozen = FrozenCompileEnvironment::new(unit, &env).unwrap();
    let catalog = CompileInputCatalog::new(
        Digest([210; 32]),
        2024,
        [
            descriptor("env").unwrap().unwrap(),
            descriptor("option_env").unwrap().unwrap(),
            descriptor("concat").unwrap().unwrap(),
        ],
    )
    .unwrap();
    let concat = catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap().unwrap().name() == "concat")
        .unwrap();
    let parent = catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap().unwrap().name() == "env")
        .unwrap();
    let descriptor = catalog.select(parent).unwrap().unwrap();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-env-adopted-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&path).unwrap();
    let mut observations = vec![];
    for (case, text) in [
        r#"(concat!("LRR_","NESTED"),concat!("missing ","message"))"#,
        r#"(concat!("LRR_","ABSENT"),concat!("missing ","message"))"#,
    ]
    .into_iter()
    .enumerate()
    {
        let original = argument_tokens(text);
        let plan = EnvironmentArgumentPlan::parse(descriptor, &original).unwrap();
        assert_eq!(plan.operands().len(), 2);
        assert_ne!(
            plan.operands()[0].nested().unwrap().identity(),
            plan.operands()[1].nested().unwrap().identity()
        );
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Builtin {
                definition: parent.identity,
            },
            invocation: Digest([211; 32]),
            input_tokens: Digest::of_bytes(&original.encode().unwrap()),
            consumer_compile_env: frozen.fingerprint(),
            parent_context: Digest([12; 32]),
            definition_edition: 2024,
            call_edition: 2024,
            grammar: GrammarSlot::Expression,
            engine_version: Digest([216; 32]),
            procedural_epoch: None,
        };
        let mut selections = BTreeMap::new();
        for (index, operand) in plan.operands().iter().enumerate() {
            let nested = operand.nested().unwrap();
            let mut child = key.clone();
            child.macro_identity = MacroIdentity::Builtin {
                definition: concat.identity,
            };
            child.invocation = nested.identity();
            child.input_tokens = nested.argument_identity();
            selections.insert(index, child);
        }
        let emit = |db: &mut Database, key: &ExpansionKey, max_wire_bytes| {
            frozen.expand_environment_adopted(
                db,
                &plan,
                EnvironmentExpansion {
                    catalog: &catalog,
                    selected: parent,
                    key,
                    original: &original,
                    site: original.spans()[0].key(),
                    max_wire_bytes,
                },
                &selections,
            )
        };
        let mut db = Database::default();
        assert!(matches!(
            plan.materialize_adopted(
                &mut db,
                descriptor,
                &original,
                frozen.fingerprint(),
                &BTreeMap::new()
            ),
            EnvironmentMaterializationPoll::NeedSelection { operand: 0 }
        ));
        assert!(
            matches!(emit(&mut db,&key,256*1024*1024).unwrap(),AdoptedEnvironmentExpansionPoll::NeedInput(InputRequest::Expand(r)) if r.key==selections[&0])
        );
        let context = DemandContext {
            request: Digest([217; 32]),
            session: Digest([218; 32]),
            unit,
            revision: db.source_revision(),
            epoch: 1,
            source_generation: Digest([219; 32]),
            native_generation: None,
            compile_environment: frozen.fingerprint(),
        };
        let mut coordinator = ExpansionCoordinator::new(context.clone());
        for (index, operand) in plan.operands().iter().enumerate() {
            let nested = operand.nested().unwrap();
            let value = lrr::expand::concat::literal_value(
                catalog.select(concat).unwrap().unwrap(),
                nested.arguments(),
                nested.reference_span(),
                64 * 1024 * 1024,
                32 * 1024 * 1024,
            )
            .unwrap();
            let record = value
                .expansion_record(
                    &catalog,
                    concat,
                    &selections[&index],
                    nested.arguments(),
                    256 * 1024 * 1024,
                )
                .unwrap();
            adopt(
                &mut db,
                &context,
                &mut coordinator,
                record,
                u32::try_from(index).unwrap(),
            );
            if index == 0 {
                assert!(
                    matches!(emit(&mut db,&key,256*1024*1024).unwrap(),AdoptedEnvironmentExpansionPoll::NeedInput(InputRequest::Expand(r)) if r.key==selections[&1])
                );
            }
        }
        let provider = Materialized {
            plan: &plan,
            descriptor,
            original: &original,
            environment: frozen.fingerprint(),
            selections: &selections,
        };
        let query = QueryKey::Constant(ConstantKey {
            definition: Digest([221; 32]),
            arguments: vec![],
            cfg: Digest::default(),
            profile: ProfileIdentity::current(),
        });
        db.fetch(&provider, query.clone())
            .into_ready()
            .unwrap()
            .unwrap();
        for child in selections.values() {
            assert!(db.memo(&query).unwrap().dependencies.iter().any(|d|matches!(d,lrr::db::dependency::DependencyStamp::Input{key:lrr::db::dependency::InputKey::ExpansionRecord(k),..} if *k==child.fingerprint())));
        }
        let mut wrong = selections.clone();
        wrong.get_mut(&0).unwrap().consumer_compile_env = Digest([220; 32]);
        assert!(matches!(
            plan.materialize_adopted(&mut db, descriptor, &original, frozen.fingerprint(), &wrong),
            EnvironmentMaterializationPoll::Ready(Err(_))
        ));
        let mut wrong = key.clone();
        wrong.input_tokens = selections[&0].input_tokens;
        assert!(emit(&mut db, &wrong, 256 * 1024 * 1024).is_err());
        let result = emit(&mut db, &key, 256 * 1024 * 1024).unwrap();
        let source = match result {
            AdoptedEnvironmentExpansionPoll::Output(EnvironmentExpansionPoll::Ready(Ok(
                record,
            ))) => {
                assert_eq!(record.key(), &key);
                let output = record.tokens().unwrap();
                let [lrr::syntax::token_wire::TokenTree::Group { children, .. }] = output.roots()
                else {
                    panic!("output group absent")
                };
                let [lrr::syntax::token_wire::TokenTree::Literal { token_text, span }] =
                    children.as_slice()
                else {
                    panic!("output literal absent")
                };
                assert_eq!(*span, original.spans()[0].key());
                assert!(emit(&mut db, &key, output.encode().unwrap().len() - 1).is_err());
                format!(
                    "fn main(){{let native=env!{text};let emitted={token_text};assert_eq!(native,emitted);println!(\"{{:?}}\",native);}}"
                )
            }
            AdoptedEnvironmentExpansionPoll::Output(EnvironmentExpansionPoll::Ready(Err(
                error,
            ))) => {
                assert_eq!(error.message, "missing message");
                format!("fn main(){{let _=env!{text};}}")
            }
            _ => panic!("adopted environment operands did not complete"),
        };
        let source_path = path.join(format!("oracle-{case}.rs"));
        let binary = path.join(format!("oracle-{case}"));
        fs::write(&source_path, source).unwrap();
        let mut command = Command::new("rustc");
        command
            .args(["+1.99.0", "--edition=2024"])
            .env("LRR_NESTED", env.declared["LRR_NESTED"].as_ref().unwrap())
            .env_remove("LRR_ABSENT")
            .arg(&source_path)
            .arg("-o")
            .arg(&binary);
        let build = command.output().unwrap();
        let observed = if case == 0 {
            assert!(
                build.status.success(),
                "{}",
                String::from_utf8_lossy(&build.stderr)
            );
            let run = Command::new(binary).output().unwrap();
            assert!(run.status.success());
            String::from_utf8(run.stdout).unwrap()
        } else {
            assert!(!build.status.success());
            let stderr = String::from_utf8(build.stderr).unwrap();
            assert!(stderr.contains("missing message"));
            stderr
        };
        observations.push(serde_json::json!({"case":case,"native_success":build.status.success(),"observed":observed,"original_key":key.fingerprint().hex()}));
        db.publish_inputs(BTreeMap::new()).unwrap();
        assert!(matches!(
            db.fetch(&provider, query),
            lrr::db::poll::QueryPoll::NeedInput(_)
        ));

        assert!(
            matches!(emit(&mut db,&key,256*1024*1024).unwrap(),AdoptedEnvironmentExpansionPoll::NeedInput(InputRequest::Expand(r)) if r.key==selections[&0])
        );
    }
    let optional = lrr::expand::compile_input_identity::descriptor("option_env")
        .unwrap()
        .unwrap();
    assert!(
        EnvironmentArgumentPlan::parse(
            &optional,
            &argument_tokens(r#"(concat!("LRR_","ABSENT"))"#)
        )
        .is_ok()
    );
    assert!(
        EnvironmentArgumentPlan::parse(
            &optional,
            &argument_tokens(r#"(concat!("LRR_","ABSENT"),"message")"#)
        )
        .is_err()
    );
    fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","observations":observations,"scope":"ordered adopted environment operands and parent string/diagnostic native agreement; no automatic recursive driver or Option output completion"})).unwrap()).unwrap();
}

#[test]
fn actual_eager_compiler_worklist_drives_nested_calls_only_after_adoption_and_matches_native() {
    use lrr::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        db::{
            key::{ConstantKey, QueryKey},
            memo::Database,
            poll::{ExpansionRequest, InputRequest},
        },
        expand::{
            compile_input_identity::descriptor,
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
            scopes::{MacroDefinition, MacroName, ScopeKey as TextualScope, TextualScopes},
        },
        frontend::{
            compile_input_catalog::CompileInputCatalog,
            hygiene::HygieneFacts,
            macro_names::{MacroNamespaces, MacroPath},
            macro_origins::ExpansionAncestry,
            macro_scopes::MacroDeclarations,
            nested_compile_inputs::{NestedCompileInputResolver, NestedLexicalContext},
        },
        service::{
            eager_compile_inputs::{
                EagerCompileInputPoll, EagerCompilerRequest, EagerLimits, poll_eager_compile_input,
            },
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply},
        },
        source::manifest::BodyKey,
        syntax::{outline::ScopeKey, parse::ParseKey},
    };
    let unit = Digest([11; 32]);
    let core = Digest([230; 32]);
    let catalog = CompileInputCatalog::new(
        core,
        2024,
        [
            descriptor("concat").unwrap().unwrap(),
            descriptor("env").unwrap().unwrap(),
            descriptor("option_env").unwrap().unwrap(),
            descriptor("include_str").unwrap().unwrap(),
        ],
    )
    .unwrap();
    let mut names = MacroNamespaces::default();
    for definition in catalog.definitions() {
        names
            .define(
                MacroPath::new(
                    core,
                    vec![
                        MacroName::parse(catalog.select(definition).unwrap().unwrap().name(), 2024)
                            .unwrap(),
                    ],
                )
                .unwrap(),
                definition.clone(),
            )
            .unwrap();
    }
    names
        .dependency(unit, MacroName::parse("core", 2024).unwrap(), core)
        .unwrap();
    let user = MacroDefinition {
        identity: Digest([231; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    names
        .define(
            MacroPath::new(unit, vec![MacroName::parse("user", 2024).unwrap()]).unwrap(),
            user.clone(),
        )
        .unwrap();
    let mut env = environment(unit);
    env.declared
        .insert("LRR_NESTED".into(), Some("value🙂\n".into()));
    env.declared.insert("LRR_ABSENT".into(), None);
    let frozen = FrozenCompileEnvironment::new(unit, &env).unwrap();
    let root = TextualScope::root(unit);
    let mut scopes = TextualScopes::default();
    scopes.add_root(root).unwrap();
    let declarations = MacroDeclarations::default();
    let ancestry = ExpansionAncestry::default();
    let cases = [
        (
            "concat",
            r#"(core::env!(core::concat!("LRR_","NESTED")),core::concat!(":",core::concat!(0x10,true)))"#,
        ),
        ("concat", r#"(user!("x"))"#),
        ("option_env", r#"(core::concat!("LRR_","ABSENT"))"#),
        ("concat", r#"(core::include_str!("external.txt"))"#),
    ];
    for (case, (kind, text)) in cases.into_iter().enumerate() {
        let original = argument_tokens(text);
        let site = original.spans()[0].key();
        let span = &original.spans()[0];
        let mut hygiene = lrr_expand_adapter::hygiene::HygieneStore::default();
        let contexts = original
            .contexts()
            .iter()
            .map(|c| serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap())
            .collect::<Vec<_>>();
        hygiene.import(&contexts).unwrap();
        let context_keys = original
            .contexts()
            .iter()
            .map(|c| lrr_expand_adapter::token_wire::Key(c.key().0))
            .collect::<Vec<_>>();
        let rows = serde_json::from_slice(
            &serde_json::to_vec(&hygiene.resolution_contexts(&context_keys).unwrap()).unwrap(),
        )
        .unwrap();
        let comparisons = serde_json::from_slice(
            &serde_json::to_vec(&hygiene.name_contexts(&context_keys).unwrap()).unwrap(),
        )
        .unwrap();
        let facts = HygieneFacts::new(original.contexts().to_vec(), rows)
            .unwrap()
            .with_name_contexts(comparisons)
            .unwrap();
        let caller = NestedLexicalContext {
            source: ParseKey {
                content: Digest(span.source.content.0),
                profile: ProfileIdentity::current(),
            },
            scope: ScopeKey {
                crate_key: unit,
                module_path: vec![],
                owner: None,
            },
            textual_scope: root,
            origin: None,
        };
        let resolver = NestedCompileInputResolver {
            namespaces: &names,
            textual: &scopes,
            declarations: &declarations,
            hygiene: &facts,
            ancestry: &ancestry,
            compiler: &catalog,
        };
        let selected = catalog
            .definitions()
            .find(|d| catalog.select(d).unwrap().unwrap().name() == kind)
            .unwrap();
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Builtin {
                definition: selected.identity,
            },
            invocation: Digest([232; 32]),
            input_tokens: Digest::of_bytes(&original.encode().unwrap()),
            consumer_compile_env: frozen.fingerprint(),
            parent_context: Digest(span.context.0),
            definition_edition: 2024,
            call_edition: 2024,
            grammar: GrammarSlot::Expression,
            engine_version: Digest([233; 32]),
            procedural_epoch: None,
        };
        if case == 0 {
            let mut staged = Database::default();
            let from_db = |db: &mut Database, revision| {
                lrr::service::eager_compile_inputs::poll_eager_compile_input_from_database(
                    db,
                    EagerCompilerRequest {
                        selected,
                        key: &key,
                        original: &original,
                        site,
                        source_revision: revision,
                        limits: EagerLimits::default(),
                    },
                    &resolver,
                    &caller,
                    None,
                )
            };
            let revision = staged.source_revision();
            assert!(
                matches!(from_db(&mut staged,revision).unwrap(),EagerCompileInputPoll::NeedInput(InputRequest::CaptureCompileEnvironment{unit:u}) if u==unit)
            );
            staged
                .publish_inputs(BTreeMap::from([(
                    lrr::db::dependency::InputKey::UnitCompileEnv(unit),
                    lrr::db::dependency::InputValue::UnitCompileEnv(std::sync::Arc::new(
                        frozen.clone(),
                    )),
                )]))
                .unwrap();
            let revision = staged.source_revision();
            let EagerCompileInputPoll::Produced(record) = from_db(&mut staged, revision).unwrap()
            else {
                panic!("prepared environment did not drive the worklist")
            };
            assert_eq!(record.key().consumer_compile_env, frozen.fingerprint());
            let mut changed = env.clone();
            changed.declared.insert("LRR_NESTED".into(), None);
            let changed = FrozenCompileEnvironment::new(unit, &changed).unwrap();
            staged
                .publish_inputs(BTreeMap::from([(
                    lrr::db::dependency::InputKey::UnitCompileEnv(unit),
                    lrr::db::dependency::InputValue::UnitCompileEnv(std::sync::Arc::new(changed)),
                )]))
                .unwrap();
            let revision = staged.source_revision();
            assert!(
                from_db(&mut staged, revision).is_err(),
                "old environment expansion key accepted new preparation"
            );
        }
        let mut db = Database::default();
        let revision = db.source_revision();
        let poll = |db: &mut Database, limits: EagerLimits, source_revision| {
            poll_eager_compile_input(
                db,
                EagerCompilerRequest {
                    selected,
                    key: &key,
                    original: &original,
                    site,
                    source_revision,
                    limits,
                },
                &resolver,
                &caller,
                &frozen,
            )
        };
        assert!(
            poll(
                &mut db,
                EagerLimits {
                    depth: 0,
                    ..EagerLimits::default()
                },
                revision
            )
            .is_err()
        );
        assert!(
            poll(
                &mut db,
                EagerLimits {
                    metadata_bytes: 1,
                    ..EagerLimits::default()
                },
                revision
            )
            .is_err()
        );
        assert!(poll(&mut db, EagerLimits::default(), revision + 1).is_err());
        let context = DemandContext {
            request: Digest([234; 32]),
            session: Digest([235; 32]),
            unit,
            revision,
            epoch: 1,
            source_generation: Digest([236; 32]),
            native_generation: None,
            compile_environment: frozen.fingerprint(),
        };
        let mut coordinator = ExpansionCoordinator::new(context.clone());
        let mut emitted = BTreeMap::new();
        let mut final_record = None;
        for index in 0..20 {
            let result = poll(&mut db, EagerLimits::default(), revision).unwrap();
            match result {
                EagerCompileInputPoll::Produced(record) => {
                    assert!(
                        emitted
                            .insert(record.key().fingerprint(), record.output())
                            .is_none(),
                        "already adopted child replayed"
                    );
                    let EagerCompileInputPoll::Produced(repeated) =
                        poll(&mut db, EagerLimits::default(), revision).unwrap()
                    else {
                        panic!("unadopted output advanced parent")
                    };
                    assert_eq!(repeated.output(), record.output());
                    assert_eq!(repeated.key(), record.key());
                    if case == 0 && record.key() == &key {
                        let error = poll(
                            &mut db,
                            EagerLimits {
                                work: 1,
                                ..EagerLimits::default()
                            },
                            revision,
                        )
                        .err()
                        .expect("adopted child traversal bypassed work budget");
                        assert!(error.message.contains("work limit"));
                        let EagerCompileInputPoll::Produced(bounded) = poll(
                            &mut db,
                            EagerLimits {
                                work: 3,
                                ..EagerLimits::default()
                            },
                            revision,
                        )
                        .unwrap() else {
                            panic!("exact three-step budget did not emit parent");
                        };
                        assert_eq!(bounded.key(), record.key());
                        assert_eq!(bounded.output(), record.output());
                    }
                    let token = coordinator
                        .issue(
                            QueryKey::Constant(ConstantKey {
                                definition: Digest([237; 32]),
                                arguments: vec![],
                                cfg: Digest::default(),
                                profile: ProfileIdentity::current(),
                            }),
                            PhaseIdentity {
                                frame: Digest([238; 32]),
                                body: BodyKey {
                                    definition: Digest([239; 32]),
                                    exact_body_bytes: Digest([240; 32]),
                                },
                                index,
                            },
                            InputRequest::Expand(Box::new(ExpansionRequest {
                                key: record.key().clone(),
                            })),
                        )
                        .unwrap();
                    coordinator.start(&context, &token).unwrap();
                    coordinator
                        .accept(&context, &token, Reply::success(&token, record.output()))
                        .unwrap();
                    let EagerCompileInputPoll::Produced(still_unadopted) =
                        poll(&mut db, EagerLimits::default(), revision).unwrap()
                    else {
                        panic!("accepted output advanced without adoption")
                    };
                    assert_eq!(still_unadopted.key(), record.key());
                    coordinator
                        .adopt(&context, &token, &mut db, record)
                        .unwrap();
                }
                EagerCompileInputPoll::Complete(record) => {
                    assert_eq!(case, 0);
                    assert_eq!(record.key(), &key);
                    final_record = Some(record);
                    break;
                }
                EagerCompileInputPoll::NeedExternal(call) if case == 1 => {
                    assert_eq!(call.selected(), &user);
                    assert!(
                        matches!(&call.key().macro_identity,lrr::expand::identity::MacroIdentity::Declaration{definition} if *definition==user.identity)
                    );
                    assert_eq!(
                        call.key().grammar,
                        lrr::expand::identity::GrammarSlot::Expression
                    );
                    assert_eq!(
                        call.key().input_tokens,
                        Digest::of_bytes(&call.original().encode().unwrap())
                    );
                    assert!(
                        call.original()
                            .spans()
                            .iter()
                            .any(|s| s.key() == call.site())
                    );
                    assert!(
                        db.input(&lrr::db::dependency::InputKey::ExpansionRecord(
                            call.key().fingerprint()
                        ))
                        .is_none()
                    );
                    break;
                }
                EagerCompileInputPoll::NeedOptionFacade {
                    value,
                    defining_unit,
                    ..
                } => {
                    assert_eq!(case, 2);
                    assert!(value.is_none());
                    assert_eq!(defining_unit, core);
                    assert_eq!(emitted.len(), 1);
                    break;
                }
                EagerCompileInputPoll::NeedExternal(call) => {
                    assert_eq!(case, 3);
                    assert_eq!(
                        catalog.select(call.selected()).unwrap().unwrap().name(),
                        "include_str"
                    );
                    let InputRequest::Expand(request) = call.input() else {
                        panic!("external request variant mismatch")
                    };
                    assert_eq!(request.key, *call.key());
                    assert_eq!(
                        request.key.input_tokens,
                        Digest::of_bytes(&call.original().encode().unwrap())
                    );
                    assert!(
                        call.original()
                            .spans()
                            .iter()
                            .any(|s| s.key() == call.site())
                    );
                    assert!(emitted.is_empty());
                    break;
                }
                _ => panic!("unexpected eager worklist demand"),
            }
            assert!(index < 19, "worklist did not terminate");
        }
        if case == 0 {
            assert!(
                poll(
                    &mut Database::default(),
                    EagerLimits {
                        depth: 1,
                        ..EagerLimits::default()
                    },
                    revision
                )
                .is_err()
            );

            assert_eq!(emitted.len(), 5);
            let record = final_record.unwrap();
            let output = record.tokens().unwrap();
            let [lrr::syntax::token_wire::TokenTree::Group { children, .. }] = output.roots()
            else {
                panic!("output root absent")
            };
            let [lrr::syntax::token_wire::TokenTree::Literal { token_text, span }] =
                children.as_slice()
            else {
                panic!("output literal absent")
            };
            assert_eq!(*span, site);
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join(format!(
                    "artifacts/macos-first/m011-eager-worklist-native-{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
            fs::create_dir_all(&path).unwrap();
            // core:: paths exercise LRR's exact compiler facet; native reexports use std for env/concat.
            let native = text.replace("core::", "std::");
            fs::write(path.join("oracle.rs"),format!("fn main(){{let native=concat!{native};let emitted={token_text};assert_eq!(native,emitted);println!(\"{{:?}}\",native);}}" )).unwrap();
            let build = Command::new("rustc")
                .args(["+1.99.0", "--edition=2024"])
                .env("LRR_NESTED", env.declared["LRR_NESTED"].as_ref().unwrap())
                .arg(path.join("oracle.rs"))
                .arg("-o")
                .arg(path.join("oracle"))
                .output()
                .unwrap();
            assert!(
                build.status.success(),
                "{}",
                String::from_utf8_lossy(&build.stderr)
            );
            let native = Command::new(path.join("oracle")).output().unwrap();
            assert!(native.status.success());
            assert!(
                poll(
                    &mut Database::default(),
                    EagerLimits {
                        work: 1,
                        ..EagerLimits::default()
                    },
                    revision
                )
                .is_err()
            );
            let mut fresh = Database::default();
            assert!(
                poll(
                    &mut fresh,
                    EagerLimits {
                        depth: 2,
                        ..EagerLimits::default()
                    },
                    revision
                )
                .is_err()
            );
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(poll(&mut db, EagerLimits::default(), revision).is_err());
            let new_revision = db.source_revision();
            assert!(matches!(
                poll(&mut db, EagerLimits::default(), new_revision),
                Ok(EagerCompileInputPoll::Produced(_))
            ));
            fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","adopted_outputs":emitted.len(),"root_output":record.output().hex(),"native_stdout":String::from_utf8(native.stdout).unwrap(),"scope":"iterative env/concat driver with real namespace resolution and coordinator adoption; external includes/user/Option remain explicit demands; no full guest acceptance"})).unwrap()).unwrap();
        }
    }
}

#[test]
fn prepared_unit_environment_inputs_invalidate_value_absence_and_missing_queries() {
    use lrr::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        db::{
            dependency::{DependencyStamp, InputKey, InputValue},
            key::{ConstantKey, QueryKey},
            memo::Database,
            poll::{InputRequest, QueryPoll},
            provider::{Provided, Provider, SyntaxProvider},
            value::QueryValue,
        },
        expand::compile_input_identity::descriptor,
        service::expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
        source::manifest::BodyKey,
    };
    use std::{cell::Cell, sync::Arc};
    struct Reader {
        unit: Digest,
        calls: Cell<usize>,
    }
    impl Provider for Reader {
        fn execute(&self, db: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
            self.calls.set(self.calls.get() + 1);
            let env = match SyntaxProvider::compile_environment(db, self.unit) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            match env
                .environment_operand(&descriptor("option_env").unwrap().unwrap(), "LRR_PREPARED")
                .unwrap()
            {
                CompileStringPoll::Ready(Ok(value)) => {
                    Provided::pure(QueryValue::Constant(i128::from(value.is_some())))
                }
                CompileStringPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                CompileStringPoll::NeedCapture { .. } => Provided::error(
                    "LRR-E-COMPILE-INPUT",
                    "variable was not explicitly captured",
                ),
            }
        }
    }
    let unit = Digest([61; 32]);
    let other = Digest([62; 32]);
    let mut prepared = environment(unit);
    prepared
        .declared
        .insert("LRR_PREPARED".into(), Some("captured".into()));
    let first = Arc::new(FrozenCompileEnvironment::new(unit, &prepared).unwrap());
    let reader = Reader {
        unit,
        calls: Cell::new(0),
    };
    let key = QueryKey::Constant(ConstantKey {
        definition: Digest([63; 32]),
        arguments: vec![],
        cfg: Digest::default(),
        profile: ProfileIdentity::current(),
    });
    let mut db = Database::default();
    let request = InputRequest::CaptureCompileEnvironment { unit };
    request.validate().unwrap();
    assert_ne!(
        request.fingerprint(),
        InputRequest::CaptureCompileEnvironment { unit: other }.fingerprint()
    );
    assert!(matches!(db.fetch(&reader,key.clone()),QueryPoll::NeedInput(input) if input==request));
    assert!(db.memo(&key).is_none());
    let mut inputs = BTreeMap::from([(
        InputKey::UnitCompileEnv(unit),
        InputValue::UnitCompileEnv(first.clone()),
    )]);
    assert!(
        db.publish_inputs(BTreeMap::from([(
            InputKey::UnitCompileEnv(other),
            InputValue::UnitCompileEnv(first.clone())
        )]))
        .is_err()
    );
    assert!(matches!(
        SyntaxProvider::compile_environment(&mut db, unit),
        QueryPoll::NeedInput(_)
    ));
    db.publish_inputs(inputs.clone()).unwrap();
    let value = db
        .fetch(&reader, key.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    assert!(matches!(value.value.as_ref(), QueryValue::Constant(1)));
    assert!(db.memo(&key).unwrap().dependencies.iter().any(
        |d| matches!(d,DependencyStamp::Input{key:InputKey::UnitCompileEnv(id),..} if *id==unit)
    ));
    let calls = reader.calls.get();
    db.fetch(&reader, key.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    assert_eq!(reader.calls.get(), calls);
    prepared.declared.insert("LRR_PREPARED".into(), None);
    let absent = Arc::new(FrozenCompileEnvironment::new(unit, &prepared).unwrap());
    assert_ne!(first.fingerprint(), absent.fingerprint());
    let mut other_prepared = environment(other);
    other_prepared
        .declared
        .insert("LRR_PREPARED".into(), Some("other-unit".into()));
    inputs.insert(
        InputKey::UnitCompileEnv(unit),
        InputValue::UnitCompileEnv(absent.clone()),
    );
    inputs.insert(
        InputKey::UnitCompileEnv(other),
        InputValue::UnitCompileEnv(Arc::new(
            FrozenCompileEnvironment::new(other, &other_prepared).unwrap(),
        )),
    );
    db.publish_inputs(inputs.clone()).unwrap();
    let value = db
        .fetch(&reader, key.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    assert!(matches!(value.value.as_ref(), QueryValue::Constant(0)));
    assert!(reader.calls.get() > calls);
    assert_eq!(
        first.lookup("LRR_PREPARED").unwrap(),
        Value::Value("captured")
    );
    inputs.remove(&InputKey::UnitCompileEnv(unit));
    db.publish_inputs(inputs).unwrap();
    assert!(matches!(db.fetch(&reader,key.clone()),QueryPoll::NeedInput(input) if input==request));
    assert!(db.memo(&key).is_none());
    let context = DemandContext {
        request: Digest([64; 32]),
        session: Digest([65; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: Digest([66; 32]),
        native_generation: None,
        compile_environment: absent.fingerprint(),
    };
    let mut coordinator = ExpansionCoordinator::new(context);
    let phase = PhaseIdentity {
        frame: Digest([67; 32]),
        body: BodyKey {
            definition: Digest([68; 32]),
            exact_body_bytes: Digest([69; 32]),
        },
        index: 0,
    };
    assert!(
        coordinator
            .issue(
                key.clone(),
                phase.clone(),
                InputRequest::CaptureCompileEnvironment { unit: other }
            )
            .is_err()
    );
    coordinator.issue(key, phase, request).unwrap();
}
