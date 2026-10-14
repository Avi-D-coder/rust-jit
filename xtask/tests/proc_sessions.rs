//! Explicit preparation of standalone synthetic proc tools and actual external execution.
use lrr::project::toolchain;
use lrr_expand_adapter::{
    proc_client::{
        MacroArtifact, ProcForm, ProcGrammar, ProcInvocation, ProcSession, ServerIdentity,
        SessionConfig,
    },
    token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, TokenForest, TokenTree, Transparency,
    },
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    sync::{Arc, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Prepared {
    config: SessionConfig,
    work: PathBuf,
}
#[path = "support/macro_service.rs"]
mod service_fixture;
fn prepared() -> &'static Prepared {
    static PREPARED: OnceLock<Prepared> = OnceLock::new();
    PREPARED.get_or_init(|| {
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_owned();
        let source=root.join("fixtures/macros/session-workspace").canonicalize().unwrap();
        let work=root.join("artifacts/macos-first").canonicalize().unwrap().join(format!("m009-proc-session-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&work).unwrap();
        let tools=toolchain::capture("1.99.0").unwrap();
        assert_eq!(tools.host,"aarch64-apple-darwin");
        let output=Command::new("cargo").args(["+1.99.0","build","--locked","--offline","--message-format=json","--manifest-path"])
            .arg(source.join("Cargo.toml")).arg("--target-dir").arg(work.join("target"))
            .env("CARGO_PROFILE_DEV_DEBUG","0").env("CARGO_INCREMENTAL","0").output().unwrap();
        fs::write(work.join("prepare.stdout"),&output.stdout).unwrap();
        fs::write(work.join("prepare.stderr"),&output.stderr).unwrap();
        assert!(output.status.success(),"explicit synthetic tool preparation failed: {}",String::from_utf8_lossy(&output.stderr));
        let messages:Vec<Value>=String::from_utf8(output.stdout).unwrap().lines().map(|l|serde_json::from_str(l).unwrap()).collect();
        let artifacts:Vec<_>=messages.iter().filter(|m|m["reason"]=="compiler-artifact").collect();
        assert_eq!(artifacts.len(),1,"this controlled workspace must compile only the synthetic proc tool");
        assert_eq!(artifacts[0]["target"]["kind"],serde_json::json!(["proc-macro"]));
        let paths=artifacts[0]["filenames"].as_array().unwrap();
        assert_eq!(paths.len(),1);
        let artifact=PathBuf::from(paths[0].as_str().unwrap());
        assert!(artifact.is_absolute());
        let artifact_hash=blake3::hash(&fs::read(&artifact).unwrap()).to_hex().to_string();
        let config=SessionConfig{allow_macro_execution:true,required_span_operations:lrr_expand_adapter::proc_client::ProcSpanOperation::stable_set(),unit:Key([2;32]),epoch:1,
            server:ServerIdentity{path:tools.server.clone(),raw_blake3:tools.server_raw_blake3.clone(),compiler_commit:tools.compiler_commit.clone(),protocol:6},
            artifacts:vec![MacroArtifact{identity:Key([3;32]),path:artifact,raw_blake3:artifact_hash}],environment:BTreeMap::new(),directory:source,
            timeout:Duration::from_secs(10),wire_limit:256*1024*1024,output_limit:4*1024*1024};
        fs::write(work.join("identity.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":tools.host,"compiler_commit":tools.compiler_commit,"server":tools.server,"server_raw_blake3":tools.server_raw_blake3,"artifact":config.artifacts[0].path,"artifact_raw_blake3":config.artifacts[0].raw_blake3,"preparation_role":"synthetic_proc_macro_host_only"})).unwrap()).unwrap();
        Prepared{config,work}
    })
}
/// Explicit fixture tool preparation, separate from macro execution authority.
fn prepared_matched_server() -> &'static ServerIdentity {
    static SERVER: OnceLock<ServerIdentity> = OnceLock::new();
    SERVER.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_owned();
        let result = Command::new("python3")
            .arg(root.join("tools/prepare_macro_server.py"))
            .arg("--allow-tool-preparation")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "explicit matched server preparation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let receipt: Value = serde_json::from_slice(
            &fs::read(root.join("artifacts/macos-first/m013-prepared-server-preparation.json"))
                .unwrap(),
        )
        .unwrap();
        let tools = toolchain::capture("1.99.0").unwrap();
        assert_eq!(receipt["platform"], tools.host);
        assert_eq!(receipt["compiler_commit"], tools.compiler_commit);
        assert_eq!(receipt["protocol"], 6);
        assert_eq!(receipt["diagnostic_stdout"], "stderr");
        assert_eq!(receipt["live_runtime_unit_compiled"], false);
        assert_eq!(receipt["production_adoption"], false);
        let path = PathBuf::from(receipt["server"].as_str().unwrap());
        assert_eq!(
            path,
            root.join("tools/prepared_macro_server/target/debug/rust-analyzer-proc-macro-srv")
                .canonicalize()
                .unwrap()
        );
        ServerIdentity {
            raw_blake3: blake3::hash(&fs::read(&path).unwrap()).to_hex().to_string(),
            path,
            compiler_commit: tools.compiler_commit,
            protocol: 6,
        }
    })
}
fn invocation(
    config: &SessionConfig,
    id: u8,
    name: &str,
    form: ProcForm,
    grammar: ProcGrammar,
    text: &str,
) -> ProcInvocation {
    let input = token_wire::parse_source(
        text,
        SourceIdentity {
            unit: config.unit,
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: "fixture.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: config.unit,
            edition: Edition::E2024,
        },
        Key([17; 32]),
    )
    .unwrap();
    let [TokenTree::Group { open, .. }] = input.roots() else {
        panic!("missing source root")
    };
    let site = *open;
    let source = lrr_expand_adapter::proc_client::CapturedSpanSource::new(
        input.spans()[0].source.clone(),
        text.into(),
        lrr_expand_adapter::proc_client::SourceDigestEncoding::RawBlake3,
        None,
    )
    .unwrap();
    ProcInvocation {
        identity: Key([id; 32]),
        unit: config.unit,
        epoch: config.epoch,
        artifact: config.artifacts[0].identity,
        export: name.into(),
        form,
        grammar,
        scope: Key([19; 32]),
        parent_origin: Key([20; 32]),
        input,
        attributes: None,
        definition_site: site,
        call_site: site,
        mixed_site: site,
        sources: vec![source],
    }
}
fn literals(forest: &TokenForest) -> Vec<String> {
    let mut result = vec![];
    let mut pending: Vec<_> = forest.roots().iter().collect();
    while let Some(token) = pending.pop() {
        match token {
            TokenTree::Group { children, .. } => pending.extend(children.iter().rev()),
            TokenTree::Literal { token_text, .. } => result.push(token_text.clone()),
            _ => {}
        }
    }
    result
}
fn identifiers(forest: &TokenForest) -> Vec<String> {
    let mut result = vec![];
    let mut pending: Vec<_> = forest.roots().iter().collect();
    while let Some(token) = pending.pop() {
        match token {
            TokenTree::Group { children, .. } => pending.extend(children.iter().rev()),
            TokenTree::Ident { text, .. } => result.push(text.clone()),
            _ => {}
        }
    }
    result
}

fn compare_captured_span_native(export: &str, input: &str, id: u8) {
    let prepared = prepared();
    let config = prepared.config.clone();
    let source_path = prepared.work.join(format!("{export}-native.rs"));
    let owned_source = format!(
        "fn main() {{\n    let _é = 0; println!(\"{{}}\", lrr_fixture_macros::{export}!({input}));\n}}\n"
    );
    let source = owned_source.as_str();
    fs::write(&source_path, source).unwrap();
    let mut request = invocation(
        &config,
        id,
        export,
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        source,
    );
    fn arguments(tokens: &[TokenTree], export: &str) -> Option<TokenTree> {
        for (index, token) in tokens.iter().enumerate() {
            if matches!(token, TokenTree::Ident { text, .. } if text == export)
                && let Some(TokenTree::Group {
                    open,
                    close,
                    children,
                    ..
                }) = tokens.get(index + 2)
            {
                return Some(TokenTree::Group {
                    delimiter: token_wire::Delimiter::Invisible,
                    open: *open,
                    close: *close,
                    children: children.clone(),
                });
            }
            if let TokenTree::Group { children, .. } = token
                && let Some(found) = arguments(children, export)
            {
                return Some(found);
            }
        }
        None
    }
    let mut identity = request.sources[0].source().clone();
    identity.path = source_path.to_str().unwrap().into();
    request.sources = vec![
        lrr_expand_adapter::proc_client::CapturedSpanSource::new(
            identity.clone(),
            source.into(),
            lrr_expand_adapter::proc_client::SourceDigestEncoding::RawBlake3,
            Some(source_path.to_str().unwrap().into()),
        )
        .unwrap(),
    ];
    let full = token_wire::parse_source(
        source,
        identity,
        request.input.contexts()[0].clone(),
        Key([17; 32]),
    )
    .unwrap();
    request.input = TokenForest::new(
        vec![arguments(full.roots(), export).expect("actual macro argument group")],
        full.contexts().to_vec(),
        full.spans().to_vec(),
    )
    .unwrap();
    let [TokenTree::Group { open, .. }] = request.input.roots() else {
        unreachable!()
    };
    request.definition_site = *open;
    request.call_site = *open;
    request.mixed_site = *open;
    let mut session = ProcSession::new(config.clone()).unwrap();
    let expanded = session.expand(&request).unwrap();
    let values = literals(&expanded);
    assert_eq!(values.len(), 1);
    let observed: String = serde_json::from_str(&values[0]).unwrap();
    let executable = prepared.work.join(format!("{export}-native"));
    let external = format!("lrr_fixture_macros={}", config.artifacts[0].path.display());
    let compiler = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--extern"])
        .arg(external)
        .arg(&source_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    fs::write(
        prepared.work.join(format!("{export}-native-build.stderr")),
        &compiler.stderr,
    )
    .unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let native = Command::new(executable).output().unwrap();
    assert!(native.status.success());
    fs::write(
        prepared.work.join(format!("{export}-native.stdout")),
        &native.stdout,
    )
    .unwrap();
    assert_eq!(
        observed,
        String::from_utf8(native.stdout).unwrap().trim_end()
    );
    assert_eq!(session.invocations(), 1);
    let previous = lrr::service::macro_session::procedural_input_fingerprint(&request).unwrap();
    request.sources[0] = lrr_expand_adapter::proc_client::CapturedSpanSource::new(
        request.sources[0].source().clone(),
        source.into(),
        lrr_expand_adapter::proc_client::SourceDigestEncoding::RawBlake3,
        None,
    )
    .unwrap();
    assert_ne!(
        previous,
        lrr::service::macro_session::procedural_input_fingerprint(&request).unwrap()
    );
    assert_eq!(
        session.expand(&request).unwrap_err().code,
        "LRR-E-MACRO-CRASH"
    );
    assert_eq!(
        session.invocations(),
        1,
        "changed callback capability must never reuse or retry"
    );
}

#[test]
fn actual_captured_span_callbacks_match_native_unicode_source() {
    compare_captured_span_native("span_info", "\"hé\"", 101);
}
#[test]
fn actual_resolved_and_located_span_operations_match_native_source() {
    compare_captured_span_native("span_relocate_info", "\"hé\"\n        \"other\"", 102);
}

#[test]
fn actual_span_relocation_preserves_owned_context_across_source_units() {
    let config = prepared().config.clone();
    let mut request = invocation(
        &config,
        103,
        "relocate_tokens",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        "first",
    );
    let mut other_config = config.clone();
    other_config.unit = Key([104; 32]);
    let other = invocation(
        &other_config,
        104,
        "identity",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        "second",
    );
    let [
        TokenTree::Group {
            open,
            close,
            children: first,
            ..
        },
    ] = request.input.roots()
    else {
        unreachable!()
    };
    let [
        TokenTree::Group {
            children: second, ..
        },
    ] = other.input.roots()
    else {
        unreachable!()
    };
    let first_span = match &first[0] {
        TokenTree::Ident { span, .. } => *span,
        _ => unreachable!(),
    };
    let second_span = match &second[0] {
        TokenTree::Ident { span, .. } => *span,
        _ => unreachable!(),
    };
    let original = request
        .input
        .spans()
        .iter()
        .find(|s| s.key() == first_span)
        .unwrap()
        .clone();
    let location = other
        .input
        .spans()
        .iter()
        .find(|s| s.key() == second_span)
        .unwrap()
        .clone();
    request.input = TokenForest::new(
        vec![TokenTree::Group {
            delimiter: token_wire::Delimiter::Invisible,
            open: *open,
            close: *close,
            children: first.iter().chain(second).cloned().collect(),
        }],
        request
            .input
            .contexts()
            .iter()
            .chain(other.input.contexts())
            .cloned()
            .collect(),
        request
            .input
            .spans()
            .iter()
            .chain(other.input.spans())
            .cloned()
            .collect(),
    )
    .unwrap();
    request.sources.extend(other.sources);
    let mut session = ProcSession::new(config).unwrap();
    let output = session.expand(&request).unwrap();
    let mut pending: Vec<_> = output.roots().iter().collect();
    let mut checked = 0;
    while let Some(token) = pending.pop() {
        match token {
            TokenTree::Group { children, .. } => pending.extend(children),
            TokenTree::Ident { text, span, .. } => {
                assert!(text == "first" || text == "second");
                let returned = output.spans().iter().find(|s| s.key() == *span).unwrap();
                assert_eq!(returned.source, location.source);
                assert_eq!(
                    (returned.start, returned.end),
                    (location.start, location.end)
                );
                assert_eq!(
                    returned.context, original.context,
                    "located_at and resolved_at must preserve the selected context independently of location"
                );
                checked += 1;
            }
            _ => {}
        }
    }
    assert_eq!(checked, 2);
    assert_eq!(session.invocations(), 1);
}

#[test]
fn actual_proc_identity_uses_each_grammar_and_preserves_owned_source_spans() {
    let config = prepared().config.clone();
    let mut session = ProcSession::new(config.clone()).unwrap();
    for (id, grammar, text) in [
        (21, ProcGrammar::Expression, "2_i64+3_i64"),
        (22, ProcGrammar::Type, "[i64; 3]"),
        (23, ProcGrammar::Items, "fn generated() {}"),
        (24, ProcGrammar::Pattern, "Some(value)"),
        (25, ProcGrammar::Statements, "let value=7_i64;"),
        (26, ProcGrammar::TraitItems, "fn value(&self)->i64;"),
        (27, ProcGrammar::ImplItems, "fn value(&self)->i64{43_i64}"),
    ] {
        let request = invocation(
            &config,
            id,
            "identity",
            ProcForm::FunctionLike,
            grammar,
            text,
        );
        let output = session.expand(&request).unwrap();
        assert_eq!(output.as_ref(), &request.input);
        assert!(Arc::ptr_eq(&output, &session.expand(&request).unwrap()));
    }
    assert_eq!(session.invocations(), 7);
}

#[test]
fn actual_attribute_replaces_input_and_derive_appends_unrelated_items() {
    let config = prepared().config.clone();
    let mut session = ProcSession::new(config.clone()).unwrap();
    let attr = invocation(
        &config,
        31,
        "make_tests",
        ProcForm::Attribute,
        ProcGrammar::Items,
        "fn dummy() {}",
    );
    let output = session.expand(&attr).unwrap();
    let names = identifiers(&output);
    assert!(names.contains(&"generated_one".into()) && names.contains(&"generated_two".into()));
    assert!(!names.contains(&"dummy".into()));
    let derive = invocation(
        &config,
        32,
        "EmitItems",
        ProcForm::Derive,
        ProcGrammar::Items,
        "struct Marker;",
    );
    let output = session.expand(&derive).unwrap();
    let names = identifiers(&output);
    for name in ["generated_helper", "Mark", "Other"] {
        assert!(names.contains(&name.into()));
    }
    assert!(!names.contains(&"Marker".into()));
    assert_eq!(literals(&output), ["41_i64", "43_i64"]);
    assert_eq!(session.invocations(), 2);
}

#[test]
fn expired_startup_deadline_preserves_authority_guard_and_prevents_tool_access() {
    let mut config = prepared().config.clone();
    config.artifacts[0].path = PathBuf::from("unavailable-relative-artifact");
    config.server.path = PathBuf::from("unavailable-relative-server");
    let expired = std::time::Instant::now() - Duration::from_secs(1);
    config.allow_macro_execution = false;
    assert_eq!(
        ProcSession::new_with_deadline(config.clone(), expired)
            .err()
            .unwrap()
            .code,
        "LRR-E-MACRO-PERMISSION"
    );
    config.allow_macro_execution = true;
    assert_eq!(
        ProcSession::new_with_deadline(config, expired)
            .err()
            .unwrap()
            .code,
        "LRR-E-MACRO-TIMEOUT"
    );
}

#[test]
fn actual_startup_version_probe_timeout_is_typed_and_reaped() {
    startup_deadline_refusal("version", "--version");
}

#[test]
fn actual_startup_protocol_probe_timeout_is_typed_and_reaped() {
    startup_deadline_refusal("protocol", "");
}

#[test]
fn actual_startup_inventory_timeout_is_typed_and_reaped() {
    startup_deadline_refusal("inventory", "--format");
}

fn startup_deadline_refusal(name: &str, argument: &str) {
    use std::os::unix::fs::PermissionsExt;
    let prepared = prepared();
    let work = prepared.work.join(format!("startup-deadline-{name}"));
    fs::create_dir(&work).unwrap();
    let server = work.join("server.sh");
    let pid_file = work.join("stalled.pid");
    let steps = work.join("steps.txt");
    let quote =
        |path: &std::path::Path| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
    // Successful phases delegate to the actual matched server. Only the
    // selected startup phase hangs; no protocol success is manufactured.
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"${{1-}}\" >> {}\nif [ \"${{1-}}\" = '{}' ]; then\n printf '%s\\n' \"$$\" > {}\n exec /bin/sleep 30\nfi\nexec {} \"$@\"\n",
        quote(&steps),
        argument,
        quote(&pid_file),
        quote(&prepared.config.server.path)
    );
    fs::write(&server, script).unwrap();
    fs::set_permissions(&server, fs::Permissions::from_mode(0o700)).unwrap();
    let mut config = prepared.config.clone();
    config.server.path = server;
    config.server.raw_blake3 = blake3::hash(&fs::read(&config.server.path).unwrap())
        .to_hex()
        .to_string();
    let start = std::time::Instant::now();
    let error = ProcSession::new_with_deadline(config, start + Duration::from_secs(4))
        .err()
        .unwrap();
    let elapsed = start.elapsed();
    assert_eq!(error.code, "LRR-E-MACRO-TIMEOUT", "{name}: {error:?}");
    assert!(elapsed < Duration::from_secs(6), "{name}: {elapsed:?}");
    let pid: i32 = fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(
        rustix::process::test_kill_process(rustix::process::Pid::from_raw(pid).unwrap()),
        Err(rustix::io::Errno::SRCH),
        "stalled startup helper must be terminated and reaped"
    );
    let observed = fs::read_to_string(&steps).unwrap();
    assert_eq!(observed.lines().last().unwrap(), argument);
    fs::write(
        work.join("timeout.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "task":"M013","platform":"aarch64-apple-darwin","rust":"1.99.0",
            "phase":name,"deadline_ms":4000,"elapsed_ms":elapsed.as_millis(),
            "diagnostic":error.code,"helper_pid":pid,"helper_reaped":true,
            "session_created":false,"successful_probes":"delegated to matched server",
            "scope":"controlled startup stall; no guest macro invocation",
            "release_complete":false
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn actual_session_execution_deadline_resets_for_each_invocation() {
    let config = prepared().config.clone();
    let mut session = ProcSession::new(config.clone()).unwrap();
    let pid = session.helper_pid().unwrap();
    for id in [43, 44] {
        session
            .cap_execution_deadline(std::time::Instant::now() + Duration::from_millis(500))
            .unwrap();
        let request = invocation(
            &config,
            id,
            "session_counter",
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            "",
        );
        assert_eq!(
            literals(&session.expand(&request).unwrap()),
            [if id == 43 { "1_u64" } else { "2_u64" }]
        );
        if id == 43 {
            std::thread::sleep(Duration::from_millis(600));
        }
    }
    assert_eq!(session.invocations(), 2);
    assert_eq!(session.helper_pid().unwrap(), pid);
    assert!(!session.helper_terminated().unwrap());
}

#[test]
fn actual_session_counter_is_exact_once_and_separate_per_unit_and_epoch() {
    let config = prepared().config.clone();
    let mut session = ProcSession::new(config.clone()).unwrap();
    let request = invocation(
        &config,
        41,
        "session_counter",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        "",
    );
    let one = session.expand(&request).unwrap();
    assert_eq!(literals(&one), ["1_u64"]);
    assert!(Arc::ptr_eq(&one, &session.expand(&request).unwrap()));
    assert_eq!(session.invocations(), 1);
    let other = invocation(
        &config,
        42,
        "session_counter",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        "",
    );
    assert_eq!(literals(&session.expand(&other).unwrap()), ["2_u64"]);
    let mut changed = invocation(
        &config,
        41,
        "session_counter",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        "43",
    );
    assert!(
        session
            .expand(&changed)
            .unwrap_err()
            .message
            .contains("different request")
    );
    changed.epoch = 2;
    assert!(
        session
            .expand(&changed)
            .unwrap_err()
            .message
            .contains("epoch")
    );
    assert_eq!(session.invocations(), 2);
    for (unit, epoch) in [(Key([2; 32]), 2), (Key([9; 32]), 1)] {
        let mut config = config.clone();
        config.unit = unit;
        config.epoch = epoch;
        let mut fresh = ProcSession::new(config.clone()).unwrap();
        let request = invocation(
            &config,
            41,
            "session_counter",
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            "",
        );
        assert_eq!(literals(&fresh.expand(&request).unwrap()), ["1_u64"]);
        assert_ne!(session.helper_pid().unwrap(), fresh.helper_pid().unwrap());
    }
}

#[test]
fn actual_macro_panic_crash_timeout_and_oversize_fail_once_and_close_session() {
    for (name, code, timeout, limit) in [
        (
            "deliberately_panics",
            "LRR-E-MACRO-PANIC",
            Duration::from_secs(10),
            4096,
        ),
        (
            "deliberately_crashes",
            "LRR-E-MACRO-CRASH",
            Duration::from_secs(10),
            4096,
        ),
        (
            "deliberately_hangs",
            "LRR-E-MACRO-TIMEOUT",
            Duration::from_millis(200),
            4096,
        ),
        (
            "oversized",
            "LRR-E-MACRO-OUTPUT",
            Duration::from_secs(10),
            1024,
        ),
        (
            "malformed",
            "LRR-E-MACRO-OUTPUT",
            Duration::from_secs(10),
            4096,
        ),
    ] {
        let mut config = prepared().config.clone();
        config.timeout = timeout;
        config.output_limit = limit;
        let mut session = ProcSession::new(config.clone()).unwrap();
        let request = invocation(
            &config,
            51,
            name,
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            "",
        );
        let error = session.expand(&request).unwrap_err();
        assert_eq!(error.code, code, "{name}: {error}");
        let again = session.expand(&request).unwrap_err();
        assert_eq!(again.code, error.code);
        assert_eq!(again.message, error.message);
        assert_eq!(session.invocations(), 1);
        assert!(
            session.helper_terminated().unwrap(),
            "{name}: failed helper must be terminated and reaped"
        );
        let other = invocation(
            &config,
            52,
            "identity",
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            "7_i64",
        );
        assert!(session.expand(&other).is_err());
        assert_eq!(session.invocations(), 1);
        let (_, errors) = session.diagnostics().unwrap();
        fs::write(prepared().work.join(format!("{name}.stderr")), errors).unwrap();
    }
}

#[test]
fn missing_session_authority_precedes_tool_filesystem_access() {
    let mut config = prepared().config.clone();
    config.allow_macro_execution = false;
    config.server.path = PathBuf::from("missing-relative");
    config.artifacts[0].path = PathBuf::from("also-missing");
    let error = ProcSession::new(config).err().unwrap();
    assert_eq!(error.code, "LRR-E-MACRO-PERMISSION");
}

#[test]
fn actual_proc_outputs_and_transformed_test_names_match_native_macos() {
    let prepared = prepared();
    let config = prepared.config.clone();
    let mut session = ProcSession::new(config.clone()).unwrap();
    let mut observed = vec![];
    for (id, name, form, text, grammar) in [
        (
            61,
            "identity",
            ProcForm::FunctionLike,
            "7_i64",
            ProcGrammar::Expression,
        ),
        (
            62,
            "EmitItems",
            ProcForm::Derive,
            "struct Marker;",
            ProcGrammar::Items,
        ),
        (
            63,
            "identity",
            ProcForm::FunctionLike,
            "fn item_value()->i64{43_i64}",
            ProcGrammar::Items,
        ),
    ] {
        let request = invocation(&config, id, name, form, grammar, text);
        observed.extend(
            literals(&session.expand(&request).unwrap())
                .into_iter()
                .map(|text| text.strip_suffix("_i64").unwrap().parse::<i64>().unwrap()),
        );
    }
    let source = prepared.work.join("native.rs");
    fs::write(&source,r#"use lrr_fixture_macros::{identity,make_tests,EmitItems};
trait Mark {fn mark(&self)->i64;} struct Other;
#[derive(EmitItems)] struct Marker;
identity!(fn item_value()->i64{43_i64});
#[make_tests] fn dummy(){panic!("must be replaced");}
fn main(){let value:identity!(i64)=identity!(7_i64);println!("{value},{},{},{}",generated_helper(),Other.mark(),item_value());}
"#).unwrap();
    let executable = prepared.work.join("native");
    let external = format!("lrr_fixture_macros={}", config.artifacts[0].path.display());
    let compiler = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--extern"])
        .arg(&external)
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    fs::write(prepared.work.join("native-build.stderr"), &compiler.stderr).unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let result = Command::new(&executable).output().unwrap();
    assert!(result.status.success());
    fs::write(prepared.work.join("native.stdout"), &result.stdout).unwrap();
    let native: Vec<i64> = String::from_utf8(result.stdout)
        .unwrap()
        .trim()
        .split(',')
        .map(|value| value.parse().unwrap())
        .collect();
    assert_eq!(observed, native);
    let executable = prepared.work.join("native-tests");
    let compiler = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--test", "--extern"])
        .arg(&external)
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    fs::write(
        prepared.work.join("native-test-build.stderr"),
        &compiler.stderr,
    )
    .unwrap();
    assert!(compiler.status.success());
    let result = Command::new(&executable)
        .args(["--list", "--format=terse"])
        .output()
        .unwrap();
    assert!(result.status.success());
    fs::write(
        prepared.work.join("native-test-list.stdout"),
        &result.stdout,
    )
    .unwrap();
    let native_names: Vec<String> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_suffix(": test").map(str::to_owned))
        .collect();
    let request = invocation(
        &config,
        64,
        "make_tests",
        ProcForm::Attribute,
        ProcGrammar::Items,
        "fn dummy() {}",
    );
    let names: Vec<String> = identifiers(&session.expand(&request).unwrap())
        .into_iter()
        .filter(|name| name.starts_with("generated_"))
        .collect();
    assert_eq!(names, native_names);
    for name in &native_names {
        let result = Command::new(&executable)
            .args(["--exact", name])
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
    }
}

#[test]
fn coordinator_cancellation_terminates_active_macro_without_retry() {
    let config = prepared().config.clone();
    let mut session = ProcSession::new(config.clone()).unwrap();
    let cancellation = session.cancellation().unwrap();
    let request = invocation(
        &config,
        71,
        "deliberately_hangs",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        "",
    );
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        cancellation.cancel().unwrap();
    });
    let started = std::time::Instant::now();
    let error = session.expand(&request).unwrap_err();
    canceller.join().unwrap();
    assert_eq!(error.code, "LRR-E-MACRO-CRASH");
    assert!(session.helper_terminated().unwrap());
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(session.invocations(), 1);
    assert_eq!(session.expand(&request).unwrap_err().code, error.code);
    assert_eq!(session.invocations(), 1);
}

#[test]
fn noisy_native_macro_channels_are_bounded_and_fail_without_retry() {
    for name in ["noisy_stdout", "noisy_stderr"] {
        let mut config = prepared().config.clone();
        if name == "noisy_stdout" {
            config.wire_limit = 8 * 1024;
        }
        let mut session = ProcSession::new(config.clone()).unwrap();
        let request = invocation(
            &config,
            81,
            name,
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            "",
        );
        let error = session.expand(&request).unwrap_err();
        assert_eq!(error.code, "LRR-E-MACRO-OUTPUT", "{name}: {error}");
        assert_eq!(session.invocations(), 1);
        assert!(session.helper_terminated().unwrap());
        assert_eq!(session.expand(&request).unwrap_err().code, error.code);
        assert_eq!(session.invocations(), 1);
        let (stdout, stderr) = session.diagnostics().unwrap();
        assert!(stdout.len() <= 64 * 1024);
        assert!(stderr.len() <= 64 * 1024);
        if name == "noisy_stderr" {
            assert_eq!(stderr.len(), 64 * 1024);
        }
        fs::write(
            prepared().work.join(format!("{name}.protocol-preview")),
            stdout,
        )
        .unwrap();
        fs::write(
            prepared().work.join(format!("{name}.stderr-preview")),
            stderr,
        )
        .unwrap();
    }
}

#[test]
fn cancellation_handle_invalidates_already_completed_session_cache() {
    let config = prepared().config.clone();
    let mut session = ProcSession::new(config.clone()).unwrap();
    let request = invocation(
        &config,
        82,
        "session_counter",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        "",
    );
    assert_eq!(literals(&session.expand(&request).unwrap()), ["1_u64"]);
    session.cancellation().unwrap().cancel().unwrap();
    assert_eq!(
        session.expand(&request).unwrap_err().code,
        "LRR-E-MACRO-CRASH"
    );
    assert_eq!(session.invocations(), 1);
    assert!(session.helper_terminated().unwrap());
}

#[test]
fn actual_prepared_service_fulfils_adopts_and_reuses_proc_record_without_reexecution() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        db::{
            key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider,
            value::QueryValue,
        },
        engine::dispatch::{ResumePoll, SavedPhase},
        expand::identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
        service::{
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
            macro_session::{
                MacroSession, macro_artifact_identity, procedural_engine_version,
                procedural_input_fingerprint,
            },
        },
        source::manifest::BodyKey,
    };
    let prepared = prepared();
    let evidence = prepared.work.join("service");
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let (tool, artifact) = manifest.artifacts.iter().next().unwrap();
    let mut config = prepared.config.clone();
    config.unit = Key(unit.0);
    config.artifacts[0].identity = Key(macro_artifact_identity(*tool, artifact).0);
    let mut request = invocation(
        &config,
        91,
        "session_counter",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        "sentinel + 1_i64",
    );
    let mut db = Database::default();
    let context = DemandContext {
        request: Digest::of_bytes(b"service-request"),
        session: Digest::of_bytes(b"service-session"),
        unit,
        revision: db.source_revision(),
        epoch: config.epoch,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Procedural {
            artifact: Digest(request.artifact.0),
            export: request.export.clone(),
            kind: ProceduralKind::Function,
        },
        invocation: Digest(request.identity.0),
        input_tokens: procedural_input_fingerprint(&request).unwrap(),
        consumer_compile_env: context.compile_environment,
        parent_context: Digest(
            request
                .input
                .spans()
                .iter()
                .find(|s| s.key() == request.call_site)
                .unwrap()
                .context
                .0,
        ),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Expression,
        engine_version: procedural_engine_version(),
        procedural_epoch: Some(context.epoch),
    };
    let directory = PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]);
    let mut service = MacroSession::open(
        context.clone(),
        &manifest,
        true,
        directory,
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .unwrap();
    assert_eq!(service.request_key(&context, &request).unwrap(), key);
    let original_scope = request.scope;
    request.scope = Key([221; 32]);
    assert_ne!(service.request_key(&context, &request).unwrap(), key);
    request.scope = original_scope;
    let original_origin = request.parent_origin;
    request.parent_origin = Key([222; 32]);
    assert_ne!(service.request_key(&context, &request).unwrap(), key);
    request.parent_origin = original_origin;
    let alternative_site = request
        .input
        .spans()
        .iter()
        .find(|span| span.key() != request.call_site)
        .unwrap()
        .key();
    let original_call = request.call_site;
    request.call_site = alternative_site;
    assert_ne!(service.request_key(&context, &request).unwrap(), key);
    request.call_site = original_call;
    let original_definition = request.definition_site;
    request.definition_site = alternative_site;
    assert_ne!(service.request_key(&context, &request).unwrap(), key);
    request.definition_site = original_definition;
    let original_mixed = request.mixed_site;
    request.mixed_site = alternative_site;
    assert_ne!(service.request_key(&context, &request).unwrap(), key);
    request.mixed_site = original_mixed;
    assert_eq!(service.request_key(&context, &request).unwrap(), key);
    let mut foreign_context = context.clone();
    foreign_context.epoch += 1;
    assert!(service.request_key(&foreign_context, &request).is_err());
    let original_export = request.export.clone();
    request.export = "absent_export".into();
    assert_eq!(
        service.request_key(&context, &request).unwrap_err().code,
        "LRR-E-MACRO-NOT-FOUND"
    );
    request.export = original_export;
    let original_mixed = request.mixed_site;
    request.mixed_site = Key([255; 32]);
    assert!(service.request_key(&context, &request).is_err());
    request.mixed_site = original_mixed;
    let original_epoch = request.epoch;
    request.epoch += 1;
    assert!(service.request_key(&context, &request).is_err());
    request.epoch = original_epoch;
    request.form = ProcForm::Derive;
    assert!(service.request_key(&context, &request).is_err());
    request.form = ProcForm::FunctionLike;
    assert_eq!(service.request_key(&context, &request).unwrap(), key);
    assert_eq!(service.invocations(), 0);
    let exports: Vec<_> = service.exports(&context).unwrap().collect();
    let derive_export = exports
        .iter()
        .find(|e| e.name == "MemberCfg" && e.form == ProcForm::Derive)
        .unwrap();
    assert_eq!(
        derive_export.helpers,
        lrr_expand_adapter::proc_client::DeriveHelperMetadata::Unavailable { protocol: 6 }
    );
    let mut helper_key = key.clone();
    helper_key.macro_identity = MacroIdentity::Procedural {
        artifact: Digest(derive_export.artifact.0),
        export: derive_export.name.into(),
        kind: ProceduralKind::Derive,
    };
    helper_key.grammar = GrammarSlot::Items;
    assert_eq!(
        service
            .derive_helpers(&context, &helper_key)
            .unwrap_err()
            .code,
        "LRR-E-MACRO-SERVER-MISMATCH"
    );
    let mut stale_inventory = context.clone();
    stale_inventory.revision += 1;
    assert!(service.exports(&stale_inventory).is_err());
    assert_eq!(service.invocations(), 0);
    use lrr::{
        core::profile::ProfileIdentity,
        expand::derive::{HelperLookup, ItemHelperScope},
        project::cargo_units::UnitRole,
        source::snapshot::SourceBlob,
        syntax::parse::ParsedFile,
    };
    let producer = *manifest
        .graph
        .units
        .iter()
        .find(|(_, u)| u.key.role == UnitRole::ProcMacroHost)
        .unwrap()
        .0;
    let macro_path = PathBuf::from(&manifest.compile_env[&producer].cargo["CARGO_MANIFEST_DIR"])
        .join("src/lib.rs");
    let blob = SourceBlob::new(fs::read(&macro_path).unwrap()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let selected = parsed.source_function_range("member_cfg").unwrap();
    assert!(
        service
            .register_source_derive(
                &stale_inventory,
                &manifest,
                producer,
                &macro_path,
                &blob,
                selected
            )
            .is_err()
    );
    let changed = SourceBlob::new(
        b"#[proc_macro_derive(MemberCfg,attributes(wrong))] pub fn member_cfg(){}".to_vec(),
    )
    .unwrap();
    assert!(
        service
            .register_source_derive(
                &context,
                &manifest,
                producer,
                &macro_path,
                &changed,
                selected
            )
            .is_err()
    );
    assert!(
        service
            .register_source_derive(
                &context,
                &manifest,
                context.unit,
                &macro_path,
                &blob,
                selected
            )
            .is_err()
    );
    let catalog_admission = service
        .register_source_derive_catalog(&context, &manifest, producer, &macro_path, &blob)
        .unwrap();
    let lrr::expand::derive_declaration::SourceCatalogAdmission::Ready(ids) = catalog_admission
    else {
        panic!("original source catalog must be complete")
    };
    let loaded_count = service
        .exports(&context)
        .unwrap()
        .filter(|e| e.form == ProcForm::Derive)
        .count();
    assert_eq!(ids.len(), loaded_count);
    assert_eq!(service.invocations(), 0);
    let declaration = service
        .register_source_derive(&context, &manifest, producer, &macro_path, &blob, selected)
        .unwrap();
    assert_eq!(
        service
            .register_source_derive(&context, &manifest, producer, &macro_path, &blob, selected)
            .unwrap(),
        declaration
    );
    assert_eq!(
        service.derive_helpers(&context, &helper_key).unwrap(),
        ["member_helper"]
    );
    let binding = service
        .derive_helper_binding(&context, &helper_key)
        .unwrap();
    let helper_scope = ItemHelperScope::new(
        Digest([245; 32]),
        context.compile_environment,
        vec![binding],
    )
    .unwrap();
    assert!(
        matches!(helper_scope.lookup_helpers(Digest([245;32]),context.compile_environment,std::slice::from_ref(&helper_key),"member_helper").unwrap(),HelperLookup::Ready(found) if found==[helper_key.clone()])
    );
    assert_eq!(service.invocations(), 0);
    let raw_selected = parsed
        .source_function_range("source_facet_metadata")
        .unwrap();
    service
        .register_source_derive(
            &context,
            &manifest,
            producer,
            &macro_path,
            &blob,
            raw_selected,
        )
        .unwrap();
    let mut raw_key = helper_key.clone();
    if let MacroIdentity::Procedural { export, .. } = &mut raw_key.macro_identity {
        *export = "type".into();
    }
    assert_eq!(
        service.derive_helpers(&context, &raw_key).unwrap(),
        ["first", "type"]
    );
    let prepared_artifact = &manifest.artifacts[&producer];
    let oracle_root = evidence.join("source-helper-native");
    fs::create_dir_all(&oracle_root).unwrap();
    for (name, attribute, accepted) in [
        ("first", "first", true),
        ("raw", "r#type", true),
        ("missing", "not_a_helper", false),
    ] {
        let path = oracle_root.join(format!("{name}.rs"));
        fs::write(
            &path,
            format!("#[derive(lrr_fixture_macros::r#type)] #[{attribute}] struct Marker;"),
        )
        .unwrap();
        let native = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-type=lib",
                "--emit=metadata",
                "--extern",
            ])
            .arg(format!("lrr_fixture_macros={}", prepared_artifact.path))
            .arg(&path)
            .arg("-o")
            .arg(oracle_root.join(format!("{name}.rmeta")))
            .output()
            .unwrap();
        fs::write(oracle_root.join(format!("{name}.stderr")), &native.stderr).unwrap();
        assert_eq!(
            native.status.success(),
            accepted,
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
        if !accepted {
            assert!(String::from_utf8_lossy(&native.stderr).contains("cannot find attribute"));
        }
    }
    assert_eq!(service.invocations(), 0);
    let query = QueryKey::ExpansionInput(Box::new(key.clone()));
    let QueryPoll::NeedInput(input) = db.fetch(&SyntaxProvider, query.clone()) else {
        panic!("missing procedural output did not suspend")
    };
    assert!(db.memo(&query).is_none());
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let phase = PhaseIdentity {
        frame: Digest::of_bytes(b"frame"),
        body: BodyKey {
            definition: Digest::of_bytes(b"body"),
            exact_body_bytes: Digest::of_bytes(b"body-bytes"),
        },
        index: 3,
    };
    let token = coordinator
        .issue(query.clone(), phase.clone(), input)
        .unwrap();
    let saved = SavedPhase::new(token.clone(), vec![17u64, 31]);
    let mut stale = context.clone();
    stale.source_generation = Digest::of_bytes(b"stale");
    assert!(
        service
            .fulfil(&stale, &mut coordinator, &token, &request)
            .is_err()
    );
    assert_eq!(service.invocations(), 0);
    request.mixed_site = alternative_site;
    assert_eq!(
        service
            .fulfil(&context, &mut coordinator, &token, &request)
            .unwrap_err()
            .code,
        "LRR-E-MACRO-OUTPUT"
    );
    assert_eq!(service.invocations(), 0);
    request.mixed_site = original_mixed;
    let record = service
        .fulfil(&context, &mut coordinator, &token, &request)
        .unwrap();
    assert_eq!(service.invocations(), 1);
    let output = record.output();
    let owned = record.tokens().unwrap();
    let adapter = token_wire::TokenForest::decode(&owned.encode().unwrap()).unwrap();
    assert_eq!(literals(&adapter), ["1_u64"]);
    let ResumePoll::Waiting(saved) = saved.resume(&mut coordinator, &context) else {
        panic!("reply resumed before record adoption")
    };
    db.begin_run().unwrap();
    coordinator
        .adopt(&context, &token, &mut db, record)
        .unwrap();
    let ready = db
        .fetch(&SyntaxProvider, query.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::ExpansionInput(descriptor) = ready.value.as_ref() else {
        panic!("wrong adopted output")
    };
    assert_eq!(descriptor.output, output);
    let ResumePoll::Ready {
        state,
        output: resumed,
    } = saved.resume(&mut coordinator, &context)
    else {
        panic!("adopted record did not resume")
    };
    assert_eq!(state, vec![17, 31]);
    assert_eq!(resumed, output);
    let reused = service
        .fulfil(&context, &mut coordinator, &token, &request)
        .unwrap();
    assert_eq!(reused.output(), output);
    assert_eq!(service.invocations(), 1);
    coordinator
        .adopt(&context, &token, &mut db, reused)
        .unwrap();
    let mut wrong_engine = key.clone();
    wrong_engine.engine_version = Digest::of_bytes(b"unmatched-engine");
    let wrong_query = QueryKey::ExpansionInput(Box::new(wrong_engine));
    let QueryPoll::NeedInput(input) = db.fetch(&SyntaxProvider, wrong_query.clone()) else {
        panic!("wrong engine should lack a record")
    };
    let token = coordinator.issue(wrong_query, phase, input).unwrap();
    assert!(
        service
            .fulfil(&context, &mut coordinator, &token, &request)
            .is_err()
    );
    assert_eq!(service.invocations(), 1);
    coordinator.cancel();
    assert!(
        service
            .fulfil(&context, &mut coordinator, &token, &request)
            .is_err()
    );
    assert_eq!(service.invocations(), 1);
    db.finish_run().unwrap();
    fs::write(evidence.join("service-report.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","prepared_live_compiler_invocations":0,"macro_invocations":service.invocations(),"output":output,"owned_literal":literals(&adapter),"coordinator_record_adopted":true,"saved_payload_resumed_once":true,"pure_query_reentry":true,"stale_context_and_wrong_engine_refused":true,"scope":"actual schema4 planning, macro-only preparation, authorized external session, immutable output adoption and saved payload; guest VM/discovery remain later integration"})).unwrap()).unwrap();
}

#[test]
fn actual_service_macro_faults_reject_saved_phase_and_preserve_origin_without_retry() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        db::{key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider},
        engine::dispatch::{ResumePoll, SavedPhase},
        expand::identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
        service::{
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
            macro_session::{
                MacroFaultPhase, MacroSession, macro_artifact_identity, procedural_engine_version,
                procedural_input_fingerprint,
            },
        },
        source::manifest::BodyKey,
    };
    let prepared = prepared();
    let evidence = prepared.work.join("service-faults");
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let (tool, artifact) = manifest.artifacts.iter().next().unwrap();
    let mut config = prepared.config.clone();
    config.unit = Key(unit.0);
    config.artifacts[0].identity = Key(macro_artifact_identity(*tool, artifact).0);
    for (id, name, code, timeout, output_limit) in [
        (
            111,
            "deliberately_panics",
            "LRR-E-MACRO-PANIC",
            Duration::from_secs(10),
            4 * 1024 * 1024,
        ),
        (
            112,
            "deliberately_crashes",
            "LRR-E-MACRO-CRASH",
            Duration::from_secs(10),
            4 * 1024 * 1024,
        ),
        (
            113,
            "deliberately_hangs",
            "LRR-E-MACRO-TIMEOUT",
            Duration::from_millis(500),
            4 * 1024 * 1024,
        ),
        (
            114,
            "oversized",
            "LRR-E-MACRO-OUTPUT",
            Duration::from_secs(10),
            128,
        ),
        (
            115,
            "panics_with_diagnostic",
            "LRR-E-MACRO-PANIC",
            Duration::from_secs(10),
            4 * 1024 * 1024,
        ),
        (
            116,
            "noisy_stderr",
            "LRR-E-MACRO-OUTPUT",
            Duration::from_secs(10),
            4 * 1024 * 1024,
        ),
    ] {
        let request = invocation(
            &config,
            id,
            name,
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            "",
        );
        let mut db = Database::default();
        let context = DemandContext {
            request: Digest::of_bytes(name.as_bytes()),
            session: Digest::of_bytes(name.as_bytes()),
            unit,
            revision: db.source_revision(),
            epoch: config.epoch,
            source_generation: manifest.inputs.fingerprint(),
            native_generation: None,
            compile_environment: manifest.compile_env[&unit].fingerprint(),
        };
        let call = request
            .input
            .spans()
            .iter()
            .find(|span| span.key() == request.call_site)
            .unwrap();
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Procedural {
                artifact: Digest(request.artifact.0),
                export: name.into(),
                kind: ProceduralKind::Function,
            },
            invocation: Digest(request.identity.0),
            input_tokens: procedural_input_fingerprint(&request).unwrap(),
            consumer_compile_env: context.compile_environment,
            parent_context: Digest(call.context.0),
            definition_edition: 2024,
            call_edition: 2024,
            grammar: GrammarSlot::Expression,
            engine_version: procedural_engine_version(),
            procedural_epoch: Some(context.epoch),
        };
        let query = QueryKey::ExpansionInput(Box::new(key));
        let QueryPoll::NeedInput(input) = db.fetch(&SyntaxProvider, query.clone()) else {
            panic!("macro fault requires expansion demand")
        };
        let mut coordinator = ExpansionCoordinator::new(context.clone());
        let phase = PhaseIdentity {
            frame: Digest::of_bytes(b"suspended-frame"),
            body: BodyKey {
                definition: Digest::of_bytes(b"test-with-should-panic"),
                exact_body_bytes: Digest::of_bytes(b"unentered-body"),
            },
            index: 7,
        };
        let token = coordinator
            .issue(query.clone(), phase.clone(), input)
            .unwrap();
        let saved = SavedPhase::new(token.clone(), vec![17, 31]);
        let directory = PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]);
        let mut service = MacroSession::open(
            context.clone(),
            &manifest,
            true,
            directory,
            timeout,
            output_limit,
        )
        .unwrap();
        assert!(service.primary_failure().is_none());
        let exports_before: Vec<_> = service
            .export_catalog(&context)
            .unwrap()
            .exports()
            .map(|e| e.definition().clone())
            .collect();
        let error = service
            .fulfil(&context, &mut coordinator, &token, &request)
            .unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(
            exports_before,
            service
                .export_catalog(&context)
                .unwrap()
                .exports()
                .map(|e| e.definition().clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(service.invocations(), 1);
        let report = service.primary_failure().unwrap();
        assert_eq!(report.phase, MacroFaultPhase::MacroExpand);
        assert_eq!(report.demand_context, context.fingerprint());
        assert_eq!(report.suspended_phase, phase.fingerprint());
        assert_eq!(report.invocation, Digest(request.identity.0));
        assert_eq!(report.call_site, Digest(request.call_site.0));
        assert_eq!(report.call_site_origin, Digest(call.origin.0));
        assert_eq!(report.parent_origin, Digest(request.parent_origin.0));
        assert_eq!(report.diagnostic.code, code);
        assert_eq!(report.native_attempts, 1);
        assert_eq!(report.helper_reaped, Some(true));
        assert!(report.capture_errors.is_empty());
        assert!(
            report.protocol_preview.len() <= 64 * 1024 && report.stderr_preview.len() <= 64 * 1024
        );
        let before = serde_json::to_vec(report).unwrap();
        if name == "deliberately_panics" {
            assert!(
                report
                    .diagnostic
                    .message
                    .contains("controlled fixture macro panic")
            );
        }
        if name == "panics_with_diagnostic" {
            assert!(
                String::from_utf8_lossy(&report.stderr_preview)
                    .contains("controlled macro stderr diagnostic")
            );
        }
        if name == "noisy_stderr" {
            assert_eq!(report.stderr_preview.len(), 64 * 1024);
            assert_eq!(report.stderr_truncated, Some(true));
        }
        let ResumePoll::Rejected {
            saved,
            error: rejected,
        } = saved.resume(&mut coordinator, &context)
        else {
            panic!("external macro failure must reject the phase, never resume guest execution")
        };
        assert_eq!(rejected.code, code);
        assert_eq!(saved.state(), &vec![17, 31]);
        assert!(db.memo(&query).is_none());
        assert_eq!(
            service
                .fulfil(&context, &mut coordinator, &token, &request)
                .unwrap_err()
                .code,
            code
        );
        assert_eq!(service.invocations(), 1);
        assert_eq!(
            serde_json::to_vec(service.primary_failure().unwrap()).unwrap(),
            before
        );
        fs::write(evidence.join(format!("{name}-fault.json")), before).unwrap();
    }
}

#[test]
fn matched_rust199_span_capabilities_are_measured_without_loading_macro_library() {
    let prepared = prepared();
    let root = prepared.work.join("span-capabilities");
    fs::create_dir_all(&root).unwrap();
    let mut observations = Vec::new();
    for (name, expression, supported) in [
        ("call_site", "proc_macro::Span::call_site()", true),
        ("mixed_site", "proc_macro::Span::mixed_site()", true),
        ("source_text", "span.source_text()", true),
        (
            "resolved_at",
            "span.resolved_at(proc_macro::Span::mixed_site())",
            true,
        ),
        (
            "located_at",
            "span.located_at(proc_macro::Span::call_site())",
            true,
        ),
        ("start", "span.start()", true),
        ("end", "span.end()", true),
        ("line", "span.line()", true),
        ("column", "span.column()", true),
        ("file", "span.file()", true),
        ("local_file", "span.local_file()", true),
        ("def_site", "proc_macro::Span::def_site()", false),
        ("source", "span.source()", false),
        ("parent", "span.parent()", false),
        ("join", "span.join(proc_macro::Span::call_site())", false),
        ("byte_range", "span.byte_range()", false),
    ] {
        let source = root.join(format!("{name}.rs"));
        fs::write(&source, format!("extern crate proc_macro;\n#[proc_macro] pub fn probe(input: proc_macro::TokenStream) -> proc_macro::TokenStream {{ let span = proc_macro::Span::call_site(); let _ = {expression}; input }}\n")).unwrap();
        let output = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-type=proc-macro",
                "--emit=metadata",
                "--error-format=json",
            ])
            .arg(&source)
            .arg("-o")
            .arg(root.join(format!("{name}.rmeta")))
            .output()
            .unwrap();
        fs::write(root.join(format!("{name}.stderr")), &output.stderr).unwrap();
        fs::write(root.join(format!("{name}.stdout")), &output.stdout).unwrap();
        let diagnostics: Vec<Value> = String::from_utf8(output.stderr)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d["level"] == "error")
            .collect();
        let codes: Vec<_> = errors
            .iter()
            .filter_map(|d| d["code"]["code"].as_str())
            .collect();
        assert_eq!(
            output.status.success(),
            supported,
            "{name}: {diagnostics:?}"
        );
        if supported {
            assert!(errors.is_empty());
        } else {
            assert!(
                !codes.is_empty(),
                "missing instability diagnostic for {name}"
            );
            assert!(
                codes.iter().all(|code| *code == "E0658"),
                "{name}: {codes:?}"
            );
            assert!(errors.iter().all(|d| {
                d["code"]["code"] == "E0658"
                    || d["message"]
                        .as_str()
                        .unwrap()
                        .starts_with("aborting due to")
            }));
        }
        observations.push(serde_json::json!({"operation":name,"stable":supported,"status":output.status.code(),"diagnostic_codes":codes}));
    }
    let measured: std::collections::BTreeSet<String> = observations
        .iter()
        .filter(|row| row["stable"] == true)
        .map(|row| row["operation"].as_str().unwrap().into())
        .collect();
    let admitted: std::collections::BTreeSet<String> =
        lrr_expand_adapter::proc_client::ProcSpanOperation::stable_set()
            .into_iter()
            .map(|op| serde_json::to_value(op).unwrap().as_str().unwrap().into())
            .collect();
    assert_eq!(
        measured, admitted,
        "production admission must match the measured pinned compiler API"
    );
    fs::write(root.join("capabilities.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "platform":"aarch64-apple-darwin", "compiler_commit":prepared.config.server.compiler_commit,
        "server_raw_blake3":prepared.config.server.raw_blake3, "protocol":prepared.config.server.protocol,
        "role":"synthetic_proc_macro_host_metadata_only", "macro_library_loads":0,
        "operations":observations, "scope":"stable compiler API admission; actual supported callback semantics have separate native fixtures"})).unwrap()).unwrap();
}

#[test]
fn unsupported_span_requirements_are_refused_after_authority_and_before_tool_io() {
    use lrr_expand_adapter::proc_client::ProcSpanOperation as Op;
    let mut config = SessionConfig {
        allow_macro_execution: false,
        required_span_operations: Op::stable_set(),
        unit: Key([2; 32]),
        epoch: 1,
        server: ServerIdentity {
            path: "missing-relative-server".into(),
            raw_blake3: "0".repeat(64),
            compiler_commit: "0".repeat(40),
            protocol: 6,
        },
        artifacts: vec![MacroArtifact {
            identity: Key([3; 32]),
            path: "missing-relative-library".into(),
            raw_blake3: "0".repeat(64),
        }],
        environment: BTreeMap::new(),
        directory: std::env::current_dir().unwrap(),
        timeout: Duration::from_secs(10),
        wire_limit: 1024,
        output_limit: 1024,
    };
    for operation in [Op::DefSite, Op::Source, Op::Parent, Op::Join, Op::ByteRange] {
        config.required_span_operations = std::collections::BTreeSet::from([operation]);
        config.allow_macro_execution = false;
        assert_eq!(
            ProcSession::new(config.clone()).err().unwrap().code,
            "LRR-E-MACRO-PERMISSION"
        );
        config.allow_macro_execution = true;
        let error = ProcSession::new(config.clone()).err().unwrap();
        assert_eq!(error.code, "LRR-E-MACRO-SERVER-MISMATCH");
        assert!(
            error.message.contains("required span operation")
                && error.message.contains("unavailable")
        );
    }
}

#[test]
fn actual_native_derive_helper_scope_matches_exact_active_item_bindings() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        expand::{
            derive::{DeriveHelperBinding, ItemHelperScope},
            identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
        },
    };
    let prepared = prepared();
    let config = &prepared.config;
    let root = prepared.work.join("helper-scope");
    fs::create_dir_all(&root).unwrap();
    let owner = Digest::of_bytes(b"Marker item identity");
    let cfg = Digest::of_bytes(b"captured cfg");
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Procedural {
            artifact: Digest(config.artifacts[0].identity.0),
            export: "EmitItems".into(),
            kind: ProceduralKind::Derive,
        },
        invocation: Digest::of_bytes(b"Marker derive invocation"),
        input_tokens: Digest::of_bytes(b"struct Marker;"),
        consumer_compile_env: cfg,
        parent_context: Digest::of_bytes(b"call context"),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Items,
        engine_version: lrr::service::macro_session::procedural_engine_version(),
        procedural_epoch: Some(config.epoch),
    };
    let declaration = Digest::of_bytes(&fs::read(config.directory.join("src/lib.rs")).unwrap());
    let binding =
        DeriveHelperBinding::new(key.clone(), declaration, ["lrr_helper".into()]).unwrap();
    let scope = ItemHelperScope::new(owner, cfg, vec![binding]).unwrap();
    assert_eq!(
        scope
            .helper_owners(owner, cfg, std::slice::from_ref(&key), "lrr_helper")
            .unwrap(),
        vec![key.clone()]
    );
    assert!(
        scope
            .helper_owners(owner, cfg, &[], "lrr_helper")
            .unwrap()
            .is_empty()
    );
    let sibling = Digest::of_bytes(b"Sibling item identity");
    assert!(
        scope
            .helper_owners(sibling, cfg, std::slice::from_ref(&key), "lrr_helper")
            .is_err()
    );
    let mut observations = Vec::new();
    for (name, items, valid) in [
        (
            "valid",
            "#[derive(lrr_fixture_macros::EmitItems)] #[lrr_helper] struct Marker;",
            true,
        ),
        ("uninvoked", "#[lrr_helper] struct Marker;", false),
        (
            "sibling",
            "#[derive(lrr_fixture_macros::EmitItems)] #[lrr_helper] struct Marker; #[lrr_helper] struct Sibling;",
            false,
        ),
        (
            "inactive",
            "#[cfg(any())] #[derive(lrr_fixture_macros::EmitItems)] #[lrr_helper] struct Marker; #[lrr_helper] struct Sibling;",
            false,
        ),
    ] {
        let source = root.join(format!("{name}.rs"));
        fs::write(
            &source,
            format!("trait Mark{{fn mark(&self)->i64;}} struct Other; {items}"),
        )
        .unwrap();
        let output = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-type=lib",
                "--emit=metadata",
                "--extern",
            ])
            .arg(format!(
                "lrr_fixture_macros={}",
                config.artifacts[0].path.display()
            ))
            .arg(&source)
            .arg("-o")
            .arg(root.join(format!("{name}.rmeta")))
            .output()
            .unwrap();
        fs::write(root.join(format!("{name}.stderr")), &output.stderr).unwrap();
        assert_eq!(
            output.status.success(),
            valid,
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if !valid {
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("cannot find attribute `lrr_helper`")
            );
        }
        observations.push(serde_json::json!({"case":name,"native_status":output.status.code(),"expected_helper_admission":valid,"native_accepts":output.status.success()}));
    }
    let duplicate_source = root.join("duplicate-declaration.rs");
    fs::write(&duplicate_source,"extern crate proc_macro; #[proc_macro_derive(Duplicate, attributes(helper, r#helper))] pub fn duplicate(_:proc_macro::TokenStream)->proc_macro::TokenStream{proc_macro::TokenStream::new()}").unwrap();
    let duplicate = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-type=proc-macro",
            "--emit=metadata",
        ])
        .arg(&duplicate_source)
        .arg("-o")
        .arg(root.join("duplicate.rmeta"))
        .output()
        .unwrap();
    fs::write(root.join("duplicate-declaration.stderr"), &duplicate.stderr).unwrap();
    assert!(
        duplicate.status.success(),
        "{}",
        String::from_utf8_lossy(&duplicate.stderr)
    );
    let duplicate_binding = DeriveHelperBinding::new(
        key.clone(),
        declaration,
        ["helper".into(), "r#helper".into()],
    )
    .unwrap();
    assert_eq!(duplicate_binding.helpers().len(), 1);
    fs::write(root.join("scope-observations.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","declaration":declaration,"scope":scope.fingerprint(),"macro_artifact":config.artifacts[0].path,"cases":observations,"scope_note":"explicit exact-export metadata and scoped admission; automatic helper metadata acquisition, full transformation queue and guest integration remain pending"})).unwrap()).unwrap();
}

#[test]
fn actual_native_derive_paths_accept_alias_absolute_raw_and_crate_qualification() {
    let prepared = prepared();
    let config = &prepared.config;
    let text = "(First, ::renamed::r#gen, crate::Last,)";
    let input = invocation(
        config,
        141,
        "identity",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        text,
    )
    .input;
    let args = if matches!(
        input.roots(),
        [TokenTree::Group {
            delimiter: token_wire::Delimiter::Parenthesis,
            ..
        }]
    ) {
        input
    } else {
        let [TokenTree::Group { children, .. }] = input.roots() else {
            unreachable!()
        };
        TokenForest::new(
            children.clone(),
            input.contexts().to_vec(),
            input.spans().to_vec(),
        )
        .unwrap()
    };
    let paths = lrr_expand_adapter::derive_paths::parse(&args).unwrap();
    assert_eq!(paths.len(), 3);
    let root = prepared.work.join("derive-paths");
    fs::create_dir_all(&root).unwrap();
    let source = root.join("valid.rs");
    fs::write(&source,"use renamed::Empty as First; pub use renamed::Empty as Last; #[derive(First, ::renamed::r#gen, crate::Last,)] struct Marker;").unwrap();
    let output = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-type=lib",
            "--emit=metadata",
            "--extern",
        ])
        .arg(format!("renamed={}", config.artifacts[0].path.display()))
        .arg(&source)
        .arg("-o")
        .arg(root.join("valid.rmeta"))
        .output()
        .unwrap();
    fs::write(root.join("valid.stderr"), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(root.join("paths.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","native_status":output.status.code(),"paths":paths.iter().map(|p|serde_json::json!({"absolute":p.absolute,"segments":p.segments})).collect::<Vec<_>>(),"scope":"pinned token path syntax plus native qualified macro acceptance; full attribute transformation queue remains pending"})).unwrap()).unwrap();
}

#[test]
fn actual_attribute_arguments_use_original_contents_and_match_native_replacement() {
    use lrr::{
        core::{digest::Digest, profile::ProfileIdentity},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::{parse::ParsedFile, spans::SourceRange},
    };
    let prepared = prepared();
    let config = prepared.config.clone();
    let source = "#[lrr_fixture_macros::attribute_tokens(fn generated()->i64{7_i64})] fn discarded(){missing();}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let facts = parsed
        .attribute_inputs_at(
            SourceRange {
                start: 0,
                end: source.len().try_into().unwrap(),
            },
            &CfgSet::default(),
        )
        .unwrap();
    let identity = SourceIdentity {
        unit: config.unit,
        content: Key(Digest::of_bytes(source.as_bytes()).0),
        path: "attribute-original.rs".into(),
    };
    let original = token_wire::parse_source(
        source,
        identity.clone(),
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: config.unit,
            edition: Edition::E2024,
        },
        Key([142; 32]),
    )
    .unwrap();
    let runtime =
        lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let source_identity = runtime.spans()[0].source.clone();
    let delimited = lrr::expand::attributes::argument_forest(&facts, 0, &source_identity, &runtime)
        .unwrap()
        .unwrap();
    let contents = lrr::expand::attributes::macro_argument_contents(&delimited).unwrap();
    let mut request = invocation(
        &config,
        143,
        "attribute_tokens",
        ProcForm::Attribute,
        ProcGrammar::Items,
        "fn discarded(){missing();}",
    );
    request.attributes = Some(TokenForest::decode(&contents.encode().unwrap()).unwrap());
    request.sources.push(
        lrr_expand_adapter::proc_client::CapturedSpanSource::new(
            identity,
            source.into(),
            lrr_expand_adapter::proc_client::SourceDigestEncoding::LrrSourceBytes,
            None,
        )
        .unwrap(),
    );
    let mut session = ProcSession::new(config.clone()).unwrap();
    let output = session.expand(&request).unwrap();
    assert!(identifiers(&output).contains(&"generated".into()));
    assert!(!identifiers(&output).contains(&"discarded".into()));
    let observed = literals(&output);
    assert_eq!(observed.len(), 1);
    let value: i64 = observed[0].strip_suffix("_i64").unwrap().parse().unwrap();
    let root = prepared.work.join("attribute-arguments");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("native.rs");
    fs::write(
        &path,
        format!("{source} fn main(){{println!(\"{{}}\",generated());}}"),
    )
    .unwrap();
    let executable = root.join("native");
    let compiler = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--extern"])
        .arg(format!(
            "lrr_fixture_macros={}",
            config.artifacts[0].path.display()
        ))
        .arg(&path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    fs::write(root.join("build.stderr"), &compiler.stderr).unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let native = Command::new(executable).output().unwrap();
    assert!(native.status.success());
    fs::write(root.join("native.stdout"), &native.stdout).unwrap();
    assert_eq!(
        value,
        String::from_utf8(native.stdout)
            .unwrap()
            .trim()
            .parse::<i64>()
            .unwrap()
    );
    assert_eq!(session.invocations(), 1);
}

#[test]
fn actual_applied_attribute_is_removed_while_later_attributes_match_native_input() {
    use lrr::{
        core::{digest::Digest, profile::ProfileIdentity},
        expand::attributes::{AttributeItemInput, item_without_applied_attribute},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::{parse::ParsedFile, spans::SourceRange},
    };
    let prepared = prepared();
    let config = prepared.config.clone();
    let source = "#[lrr_fixture_macros::check_applied_absent] #[inline] fn value()->i64{7_i64}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let facts = parsed
        .attribute_inputs_at(
            SourceRange {
                start: 0,
                end: source.len().try_into().unwrap(),
            },
            &CfgSet::default(),
        )
        .unwrap();
    let identity = SourceIdentity {
        unit: config.unit,
        content: Key(Digest::of_bytes(source.as_bytes()).0),
        path: "applied-original.rs".into(),
    };
    let original = token_wire::parse_source(
        source,
        identity.clone(),
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: config.unit,
            edition: Edition::E2024,
        },
        Key([151; 32]),
    )
    .unwrap();
    let runtime =
        lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let source_identity = runtime.spans()[0].source.clone();
    let AttributeItemInput::Ready(input) =
        item_without_applied_attribute(&facts, 0, &source_identity, &runtime).unwrap()
    else {
        panic!("original outer attribute should be removable")
    };
    let mut request = invocation(
        &config,
        152,
        "check_applied_absent",
        ProcForm::Attribute,
        ProcGrammar::Items,
        "fn value()->i64{7_i64}",
    );
    request.input = TokenForest::decode(&input.encode().unwrap()).unwrap();
    let [TokenTree::Group { open, .. }] = request.input.roots() else {
        unreachable!()
    };
    request.definition_site = *open;
    request.call_site = *open;
    request.mixed_site = *open;
    request.sources = vec![
        lrr_expand_adapter::proc_client::CapturedSpanSource::new(
            identity,
            source.into(),
            lrr_expand_adapter::proc_client::SourceDigestEncoding::LrrSourceBytes,
            None,
        )
        .unwrap(),
    ];
    let mut session = ProcSession::new(config.clone()).unwrap();
    let output = session.expand(&request).unwrap();
    assert_eq!(output.as_ref(), &request.input);
    assert_eq!(session.invocations(), 1);
    let root = prepared.work.join("applied-attribute");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("native.rs");
    fs::write(
        &path,
        format!("{source} fn main(){{println!(\"{{}}\",value());}}"),
    )
    .unwrap();
    let executable = root.join("native");
    let compiler = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--extern"])
        .arg(format!(
            "lrr_fixture_macros={}",
            config.artifacts[0].path.display()
        ))
        .arg(&path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    fs::write(root.join("build.stderr"), &compiler.stderr).unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let native = Command::new(executable).output().unwrap();
    assert!(native.status.success());
    fs::write(root.join("native.stdout"), &native.stdout).unwrap();
    let observed = literals(&output);
    assert_eq!(observed.len(), 1);
    assert_eq!(
        observed[0]
            .strip_suffix("_i64")
            .unwrap()
            .parse::<i64>()
            .unwrap(),
        String::from_utf8(native.stdout)
            .unwrap()
            .trim()
            .parse::<i64>()
            .unwrap()
    );
}

#[test]
fn actual_cfg_materialized_attribute_input_matches_native_preprocessing() {
    use lrr::{
        core::{digest::Digest, profile::ProfileIdentity},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::{parse::ParsedFile, spans::SourceRange},
    };
    let prepared = prepared();
    let config = prepared.config.clone();
    let source = "#[cfg(all())] #[cfg_attr(any(), unavailable)] #[cfg_attr(all(), lrr_fixture_macros::check_applied_absent, cfg_attr(all(), inline))] fn value()->i64{7_i64}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let facts = parsed
        .attribute_inputs_at(
            SourceRange {
                start: 0,
                end: source.len().try_into().unwrap(),
            },
            &CfgSet::default(),
        )
        .unwrap();
    let identity = SourceIdentity {
        unit: config.unit,
        content: Key(Digest::of_bytes(source.as_bytes()).0),
        path: "applied-original.rs".into(),
    };
    let original = token_wire::parse_source(
        source,
        identity.clone(),
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: config.unit,
            edition: Edition::E2024,
        },
        Key([154; 32]),
    )
    .unwrap();
    let runtime =
        lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let source_identity = runtime.spans()[0].source.clone();
    let prepared_input = lrr::service::item_macro_inputs::attribute(
        Digest([19; 32]),
        &facts,
        1,
        &source_identity,
        &runtime,
    )
    .unwrap();
    assert!(prepared_input.arguments.is_none());
    let input = prepared_input.item;
    let mut request = invocation(
        &config,
        153,
        "check_applied_absent",
        ProcForm::Attribute,
        ProcGrammar::Items,
        "fn value()->i64{7_i64}",
    );
    request.input = TokenForest::decode(&input.encode().unwrap()).unwrap();
    let [TokenTree::Group { open, .. }] = request.input.roots() else {
        unreachable!()
    };
    request.definition_site = *open;
    request.call_site = *open;
    request.mixed_site = *open;
    request.sources = vec![
        lrr_expand_adapter::proc_client::CapturedSpanSource::new(
            identity,
            source.into(),
            lrr_expand_adapter::proc_client::SourceDigestEncoding::LrrSourceBytes,
            None,
        )
        .unwrap(),
    ];
    let mut session = ProcSession::new(config.clone()).unwrap();
    let output = session.expand(&request).unwrap();
    assert_eq!(output.as_ref(), &request.input);
    assert_eq!(session.invocations(), 1);
    let root = prepared.work.join("cfg-materialized-attribute");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("native.rs");
    fs::write(
        &path,
        format!("{source} fn main(){{println!(\"{{}}\",value());}}"),
    )
    .unwrap();
    let executable = root.join("native");
    let compiler = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--extern"])
        .arg(format!(
            "lrr_fixture_macros={}",
            config.artifacts[0].path.display()
        ))
        .arg(&path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    fs::write(root.join("build.stderr"), &compiler.stderr).unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let native = Command::new(executable).output().unwrap();
    assert!(native.status.success());
    fs::write(root.join("native.stdout"), &native.stdout).unwrap();
    let observed = literals(&output);
    assert_eq!(observed.len(), 1);
    assert_eq!(
        observed[0]
            .strip_suffix("_i64")
            .unwrap()
            .parse::<i64>()
            .unwrap(),
        String::from_utf8(native.stdout)
            .unwrap()
            .trim()
            .parse::<i64>()
            .unwrap()
    );
}

#[test]
fn actual_macro_child_holding_pipes_is_terminated_with_owned_helper_group() {
    let prepared = prepared();
    let pid_file = prepared.work.join("descendant.pid");
    let mut config = prepared.config.clone();
    // This fixed budget also covers matched-server startup/library inventory.
    // Keep the five-second assertion below and require an actual native attempt.
    config.timeout = Duration::from_secs(2);
    config.environment.insert(
        "LRR_MACRO_CHILD_PID_FILE".into(),
        pid_file.to_str().unwrap().into(),
    );
    let mut session = ProcSession::new(config.clone()).unwrap();
    let request = invocation(
        &config,
        92,
        "child_hangs",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        "",
    );
    let started = std::time::Instant::now();
    let error = session.expand(&request).unwrap_err();
    assert_eq!(error.code, "LRR-E-MACRO-TIMEOUT", "{error:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(session.helper_terminated().unwrap());
    assert_eq!(session.invocations(), 1);
    assert_eq!(session.expand(&request).unwrap_err().code, error.code);
    assert_eq!(session.invocations(), 1);
    let pid = fs::read_to_string(&pid_file)
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let child = rustix::process::Pid::from_raw(i32::try_from(pid).unwrap()).unwrap();
        assert_ne!(child, rustix::process::Pid::INIT);
        match rustix::process::test_kill_process(child) {
            Err(rustix::io::Errno::SRCH) => break,
            Ok(()) => {}
            Err(error) => panic!("owned descendant liveness probe failed: {error}"),
        }
        assert!(
            std::time::Instant::now() < until,
            "macro descendant {pid} has not terminated and been reaped"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    fs::write(prepared.work.join("descendant-report.json"),serde_json::to_vec_pretty(&serde_json::json!({"helper_pid":session.helper_pid().unwrap(),"descendant_pid":pid,"helper_reaped":true,"descendant_not_running":true,"native_attempts":session.invocations(),"session_timeout_ms":config.timeout.as_millis(),"diagnostic":error.code,"scope":"owned helper process group; native macros are not contained against deliberate group escape"})).unwrap()).unwrap();
}

#[test]
fn actual_native_derive_input_removes_derives_retains_helpers_after_cfg() {
    let prepared = prepared();
    let root = prepared.work.join("derive-input-attributes");
    fs::create_dir_all(&root).unwrap();
    let source = "#[derive(lrr_fixture_macros::Empty)] #[cfg(all())] #[cfg_attr(any(), unavailable)] #[cfg_attr(all(), derive(lrr_fixture_macros::CheckDeriveInput), repr(C), lrr_derive_helper)] struct Marker;";
    let path = root.join("native.rs");
    fs::write(
        &path,
        format!(
            "{source} fn main() {{ let _ = Marker; println!(\"{{}}\", DERIVE_INPUT_ATTRIBUTES); }}"
        ),
    )
    .unwrap();
    let executable = root.join("native");
    let compiler = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--extern"])
        .arg(format!(
            "lrr_fixture_macros={}",
            prepared.config.artifacts[0].path.display()
        ))
        .arg(&path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    fs::write(root.join("build.stderr"), &compiler.stderr).unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let native = Command::new(executable).output().unwrap();
    assert!(native.status.success());
    fs::write(root.join("native.stdout"), &native.stdout).unwrap();
    let native_count: usize = String::from_utf8(native.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    use lrr::{
        core::{digest::Digest, profile::ProfileIdentity},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::{parse::ParsedFile, spans::SourceRange},
    };
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let facts = parsed
        .attribute_inputs_at(
            SourceRange {
                start: 0,
                end: source.len().try_into().unwrap(),
            },
            &CfgSet::default(),
        )
        .unwrap();
    let identity = SourceIdentity {
        unit: prepared.config.unit,
        content: Key(Digest::of_bytes(source.as_bytes()).0),
        path: "derive-input.rs".into(),
    };
    let original = token_wire::parse_source(
        source,
        identity.clone(),
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: prepared.config.unit,
            edition: Edition::E2024,
        },
        Key([161; 32]),
    )
    .unwrap();
    let runtime =
        lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let input = lrr::expand::attribute_materialize::derive_input(
        &facts,
        &runtime.spans()[0].source,
        &runtime,
    )
    .unwrap();
    let mut request = invocation(
        &prepared.config,
        162,
        "CheckDeriveInput",
        ProcForm::Derive,
        ProcGrammar::Items,
        source,
    );
    request.input = TokenForest::decode(&input.encode().unwrap()).unwrap();
    let [TokenTree::Group { open, .. }] = request.input.roots() else {
        unreachable!()
    };
    request.call_site = *open;
    request.definition_site = *open;
    request.mixed_site = *open;
    request.sources = vec![
        lrr_expand_adapter::proc_client::CapturedSpanSource::new(
            identity,
            source.into(),
            lrr_expand_adapter::proc_client::SourceDigestEncoding::LrrSourceBytes,
            None,
        )
        .unwrap(),
    ];
    let mut session = ProcSession::new(prepared.config.clone()).unwrap();
    let output = session.expand(&request).unwrap();
    assert_eq!(session.invocations(), 1);
    let values = literals(&output);
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0]
            .strip_suffix("_usize")
            .unwrap()
            .parse::<usize>()
            .unwrap(),
        native_count
    );
    assert!(identifiers(&request.input).contains(&"Marker".into()));
    assert!(identifiers(&output).contains(&"DERIVE_INPUT_ATTRIBUTES".into()));
    assert!(!identifiers(&output).contains(&"Marker".into()));
}

#[test]
fn actual_attribute_chain_replans_then_appends_sibling_derives_matching_native() {
    use lrr::{
        core::digest::Digest,
        expand::{
            identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
            record::ExpansionRecord,
            worklist::{AttributeSlot, DeriveSlot, ItemPlan, ItemTransformWorklist, TransformStep},
        },
    };
    let prepared = prepared();
    let config = &prepared.config;
    let env = Digest([181; 32]);
    let owner = Digest([182; 32]);
    let cfg = Digest([183; 32]);
    let key_for = |request: &ProcInvocation, kind| {
        let span = request
            .input
            .spans()
            .iter()
            .find(|s| s.key() == request.call_site)
            .unwrap();
        ExpansionKey {
            macro_identity: MacroIdentity::Procedural {
                artifact: Digest(request.artifact.0),
                export: request.export.clone(),
                kind,
            },
            invocation: Digest(request.identity.0),
            input_tokens: lrr::service::macro_session::procedural_input_fingerprint(request)
                .unwrap(),
            consumer_compile_env: env,
            parent_context: Digest(span.context.0),
            definition_edition: 2024,
            call_edition: 2024,
            grammar: GrammarSlot::Items,
            engine_version: lrr::service::macro_session::procedural_engine_version(),
            procedural_epoch: Some(config.epoch),
        }
    };
    let runtime = |tokens: &TokenForest| {
        Arc::new(lrr::syntax::token_wire::TokenForest::decode(&tokens.encode().unwrap()).unwrap())
    };
    let strip_first_attribute = |tokens: &TokenForest| {
        let [
            TokenTree::Group {
                open,
                close,
                children,
                ..
            },
        ] = tokens.roots()
        else {
            panic!("single root expected")
        };
        assert!(matches!(&children[0], TokenTree::Punct { scalar: '#', .. }));
        assert!(matches!(
            &children[1],
            TokenTree::Group {
                delimiter: token_wire::Delimiter::Bracket,
                ..
            }
        ));
        TokenForest::new(
            vec![TokenTree::Group {
                delimiter: token_wire::Delimiter::Invisible,
                open: *open,
                close: *close,
                children: children[2..].to_vec(),
            }],
            tokens.contexts().to_vec(),
            tokens.spans().to_vec(),
        )
        .unwrap()
    };
    let first = invocation(
        config,
        184,
        "chain_begin",
        ProcForm::Attribute,
        ProcGrammar::Items,
        "#[lrr_fixture_macros::chain_finish] fn discarded(){undefined_cold();}",
    );
    let first_key = key_for(&first, ProceduralKind::Attribute);
    let original = runtime(&first.input);
    let mut work = ItemTransformWorklist::new(
        ItemPlan::new(
            owner,
            cfg,
            original,
            vec![
                AttributeSlot::expand(first_key.clone()),
                AttributeSlot::Unresolved(Digest([185; 32])),
            ],
            vec![],
        )
        .unwrap(),
        env,
        config.epoch,
    )
    .unwrap();
    let mut session = ProcSession::new(config.clone()).unwrap();
    use lrr::{
        db::{
            key::QueryKey,
            memo::Database,
            poll::{ExpansionRequest, InputRequest},
        },
        service::expansion::{
            DemandContext, ExpansionCoordinator, PhaseIdentity, Reply, StartDecision,
        },
        source::manifest::BodyKey,
    };
    let mut db = Database::default();
    let context = DemandContext {
        request: Digest([189; 32]),
        session: Digest([190; 32]),
        unit: Digest(config.unit.0),
        revision: db.source_revision(),
        epoch: config.epoch,
        source_generation: Digest([191; 32]),
        native_generation: None,
        compile_environment: env,
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let mut phase_index = 0u32;
    let mut execute =
        |work: &mut ItemTransformWorklist, request: &ProcInvocation, key: ExpansionKey| {
            let token = coordinator
                .issue(
                    QueryKey::ExpansionInput(Box::new(key.clone())),
                    PhaseIdentity {
                        frame: owner,
                        body: BodyKey {
                            definition: owner,
                            exact_body_bytes: cfg,
                        },
                        index: phase_index,
                    },
                    InputRequest::Expand(Box::new(ExpansionRequest { key: key.clone() })),
                )
                .unwrap();
            phase_index += 1;
            assert!(matches!(
                coordinator.start(&context, &token).unwrap(),
                StartDecision::Start
            ));
            let output = session.expand(request).unwrap();
            let record = ExpansionRecord::from_tokens(key, &runtime(&output)).unwrap();
            coordinator
                .accept(&context, &token, Reply::success(&token, record.output()))
                .unwrap();
            assert!(
                coordinator
                    .admit_transform(&context, &token, &db, work, record.clone())
                    .is_err()
            );
            coordinator
                .adopt(&context, &token, &mut db, record.clone())
                .unwrap();
            let mut stale = context.clone();
            stale.source_generation = Digest([192; 32]);
            assert!(
                coordinator
                    .admit_transform(&stale, &token, &db, work, record.clone())
                    .is_err()
            );
            let mut changed_db = Database::default();
            changed_db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(
                coordinator
                    .admit_transform(&context, &token, &changed_db, work, record.clone())
                    .is_err()
            );
            coordinator
                .admit_transform(&context, &token, &db, work, record.clone())
                .unwrap();
            assert!(
                coordinator
                    .admit_transform(&context, &token, &db, work, record)
                    .is_err()
            );
            output
        };

    assert!(matches!(work.poll(),TransformStep::Expand(k) if k==&first_key));
    let first_output = execute(&mut work, &first, first_key);
    let ticket = match work.poll() {
        TransformStep::NeedReanalysis { ticket, .. } => *ticket,
        _ => panic!("replacement must reanalyse"),
    };
    let mut second = first;
    second.identity = Key([186; 32]);
    second.export = "chain_finish".into();
    second.input = strip_first_attribute(&first_output);
    let second_key = key_for(&second, ProceduralKind::Attribute);
    work.replan(
        ticket,
        ItemPlan::new(
            owner,
            cfg,
            runtime(&first_output),
            vec![AttributeSlot::expand(second_key.clone())],
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    let generated_key = second_key.clone();
    let second_output = execute(&mut work, &second, second_key);
    let generated = lrr::expand::generated_items::GeneratedItems::admit(
        &generated_key,
        Arc::new(
            ExpansionRecord::from_tokens(generated_key.clone(), runtime(&second_output).as_ref())
                .unwrap(),
        ),
        config.epoch,
    )
    .unwrap();
    let generated_cfg = lrr::source::snapshot::CfgSet::guest_default();
    let annotations = lrr::expand::generated_derives::GeneratedDeriveAnnotations::new(
        &generated,
        0,
        &generated_cfg,
        [(
            0,
            lrr::expand::builtin_attributes::descriptor("derive")
                .unwrap()
                .unwrap(),
        )],
    )
    .unwrap();
    let generated_plan = lrr::expand::generated_derives::GeneratedDerivePlan::prepare(
        &generated,
        0,
        &generated_cfg,
        &annotations,
    )
    .unwrap();
    assert_eq!(generated_plan.annotations()[0].paths().len(), 2);
    assert_eq!(generated_plan.unresolved_attributes(), [2]);
    let generated_input = generated_plan
        .materialize_input(&generated, 0, &generated_cfg)
        .unwrap();
    let ticket = match work.poll() {
        TransformStep::NeedReanalysis { ticket, .. } => *ticket,
        _ => panic!("generated derives must reanalyse"),
    };
    let mut derive = second;
    derive.identity = Key([187; 32]);
    derive.export = "CheckDeriveInput".into();
    derive.form = ProcForm::Derive;
    derive.input = TokenForest::decode(&generated_input.encode().unwrap()).unwrap();
    let [TokenTree::Group { open, .. }] = derive.input.roots() else {
        unreachable!()
    };
    derive.call_site = *open;
    derive.definition_site = *open;
    derive.mixed_site = *open;
    let derive_key = key_for(&derive, ProceduralKind::Derive);
    let mut sibling = invocation(
        config,
        188,
        "Empty",
        ProcForm::Derive,
        ProcGrammar::Items,
        "",
    );
    sibling.input = derive.input.clone();
    sibling.sources = derive.sources.clone();
    sibling.call_site = derive.call_site;
    sibling.definition_site = derive.definition_site;
    sibling.mixed_site = derive.mixed_site;
    sibling.identity = Key([188; 32]);
    sibling.export = "Empty".into();
    let sibling_key = key_for(&sibling, ProceduralKind::Derive);
    assert_eq!(derive_key.input_tokens, sibling_key.input_tokens);
    work.replan(
        ticket,
        ItemPlan::new(
            owner,
            cfg,
            runtime(&second_output),
            vec![],
            vec![
                DeriveSlot::expand(derive_key.clone()),
                DeriveSlot::expand(sibling_key.clone()),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    let derived_output = execute(&mut work, &derive, derive_key);
    assert!(matches!(work.poll(),TransformStep::Expand(k) if k==&sibling_key));
    let sibling_output = execute(&mut work, &sibling, sibling_key);
    match work.poll() {
        TransformStep::Complete { item, appended } => {
            assert_eq!(item.as_ref(), runtime(&second_output).as_ref());
            assert_eq!(appended.len(), 2);
            assert_eq!(
                appended[0].tokens().unwrap(),
                runtime(&derived_output).as_ref().clone()
            );
            assert_eq!(
                appended[1].tokens().unwrap(),
                runtime(&sibling_output).as_ref().clone()
            );
        }
        _ => panic!("complete after all derivatives"),
    }
    assert_eq!(session.invocations(), 4);
    let root = prepared.work.join("transform-chain");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("native.rs");
    fs::write(&path,"#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish] fn discarded(){undefined_cold();} fn main(){let _=Marker;println!(\"{}\",DERIVE_INPUT_ATTRIBUTES);}").unwrap();
    let executable = root.join("native");
    let compiler = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--extern"])
        .arg(format!(
            "lrr_fixture_macros={}",
            config.artifacts[0].path.display()
        ))
        .arg(&path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    fs::write(root.join("build.stderr"), &compiler.stderr).unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let native = Command::new(executable).output().unwrap();
    assert!(native.status.success());
    fs::write(root.join("native.stdout"), &native.stdout).unwrap();
    let values = literals(&derived_output);
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0]
            .strip_suffix("_usize")
            .unwrap()
            .parse::<usize>()
            .unwrap(),
        String::from_utf8(native.stdout)
            .unwrap()
            .trim()
            .parse::<usize>()
            .unwrap()
    );
    let input_wire = generated_input.encode().unwrap();
    fs::write(root.join("generated-derive-input.lrrtok"), &input_wire).unwrap();
    assert!(generated_input.spans().iter().all(|row| {
        generated
            .tokens()
            .spans()
            .iter()
            .any(|original| original == row)
    }));
    assert!(generated_input.contexts().iter().all(|row| {
        generated
            .tokens()
            .contexts()
            .iter()
            .any(|original| original == row)
    }));
    fs::write(root.join("worklist.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","role":"synthetic_macro_chain_native_oracle","invocations":session.invocations(),"replacement_reanalyses":2,"appended_derives":2,"generated_derive_materialization":true,"original_span_context_records_preserved":true,"derive_input_blake3":blake3::hash(&input_wire).to_hex().to_string(),"guest_execution":false,"publication_scope":"worklist_fragments_only"})).unwrap()).unwrap();
}

#[test]
fn actual_native_derive_member_cfg_preprocessing() {
    let prepared = prepared();
    let samples = [
        (
            "record",
            "#[derive(lrr_fixture_macros::MemberCfg)] struct Record { #[cfg(any())] DROP: Missing, #[cfg_attr(all(), member_helper)] kept: i64, #[cfg(all())] last: i64 }",
        ),
        (
            "tuple",
            "#[derive(lrr_fixture_macros::MemberCfg)] struct Tuple(#[cfg(any())] Missing, #[cfg_attr(all(), member_helper)] i64, #[cfg(any())] Missing);",
        ),
        (
            "enum",
            "#[derive(lrr_fixture_macros::MemberCfg)] enum Choice { #[cfg(any())] DROP(#[cfg(not(a,a))] Missing), #[cfg_attr(all(), member_helper)] Kept { #[cfg(any())] DROP: Missing, #[cfg_attr(all(), member_helper)] value: i64 }, Last(#[cfg(any())] Missing, i64) }",
        ),
        (
            "record_empty",
            "#[derive(lrr_fixture_macros::MemberCfg)] struct Empty { #[cfg(any())] gone: Missing }",
        ),
        (
            "tuple_empty",
            "#[derive(lrr_fixture_macros::MemberCfg)] struct Empty(#[cfg(any())] Missing);",
        ),
        (
            "enum_empty",
            "#[derive(lrr_fixture_macros::MemberCfg)] enum Empty { #[cfg(any())] Gone(#[cfg(not(a,a))] Missing) }",
        ),
    ];
    let root = prepared.work.join("derive-member-cfg");
    fs::create_dir_all(&root).unwrap();
    let mut session = ProcSession::new(prepared.config.clone()).unwrap();
    for (case, (name, source)) in samples.into_iter().enumerate() {
        let path = root.join(format!("{name}.rs"));
        let executable = root.join(name);
        fs::write(
            &path,
            format!("{source} fn main(){{println!(\"{{}}\",MEMBER_NAMES);}}"),
        )
        .unwrap();
        let compiler = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--extern"])
            .arg(format!(
                "lrr_fixture_macros={}",
                prepared.config.artifacts[0].path.display()
            ))
            .arg(&path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        fs::write(root.join(format!("{name}.stderr")), &compiler.stderr).unwrap();
        assert!(
            compiler.status.success(),
            "{}",
            String::from_utf8_lossy(&compiler.stderr)
        );
        let native = Command::new(executable).output().unwrap();
        assert!(native.status.success());
        fs::write(root.join(format!("{name}.stdout")), &native.stdout).unwrap();
        let observed = String::from_utf8(native.stdout).unwrap();
        assert!(
            !observed.contains("DROP") && !observed.contains("Missing"),
            "{observed}"
        );
        assert!(!observed.contains("cfg_attr"), "{observed}");
        if !name.ends_with("empty") {
            assert!(observed.contains("member_helper"), "{observed}");
        }
        use lrr::{
            core::{digest::Digest, profile::ProfileIdentity},
            source::snapshot::{CfgSet, SourceBlob},
            syntax::{parse::ParsedFile, spans::SourceRange},
        };
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let facts = parsed
            .derive_item_inputs_at(
                SourceRange {
                    start: 0,
                    end: source.len().try_into().unwrap(),
                },
                &CfgSet::default(),
            )
            .unwrap();
        let identity = SourceIdentity {
            unit: prepared.config.unit,
            content: Key(Digest::of_bytes(source.as_bytes()).0),
            path: format!("{name}.rs"),
        };
        let original = token_wire::parse_source(
            source,
            identity.clone(),
            ContextRecord {
                parent: None,
                expansion: None,
                transparency: Transparency::Opaque,
                defining_unit: prepared.config.unit,
                edition: Edition::E2024,
            },
            Key([201; 32]),
        )
        .unwrap();
        let runtime =
            lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
        let prepared_input = lrr::service::item_macro_inputs::derive(
            Digest([20; 32]),
            &facts,
            0,
            &runtime.spans()[0].source,
            &runtime,
        )
        .unwrap();
        assert_eq!(prepared_input.selections.len(), 1);
        assert_eq!(
            prepared_input.selections[0].segments,
            ["lrr_fixture_macros", "MemberCfg"]
        );
        let input = prepared_input.item;
        assert!(input.spans().iter().all(|s| runtime.spans().contains(s)));
        assert!(
            input
                .contexts()
                .iter()
                .all(|c| runtime.contexts().contains(c))
        );
        let mut request = invocation(
            &prepared.config,
            202 + u8::try_from(case).unwrap(),
            "MemberCfg",
            ProcForm::Derive,
            ProcGrammar::Items,
            source,
        );
        request.input = TokenForest::decode(&input.encode().unwrap()).unwrap();
        let [TokenTree::Group { open, .. }] = request.input.roots() else {
            unreachable!()
        };
        request.definition_site = *open;
        request.call_site = *open;
        request.mixed_site = *open;
        request.sources = vec![
            lrr_expand_adapter::proc_client::CapturedSpanSource::new(
                identity,
                source.into(),
                lrr_expand_adapter::proc_client::SourceDigestEncoding::LrrSourceBytes,
                None,
            )
            .unwrap(),
        ];
        let output = session.expand(&request).unwrap();
        let values = literals(&output);
        assert_eq!(values.len(), 1);
        let actual: String = serde_json::from_str(&values[0]).unwrap();
        let mut grammar = invocation(
            &prepared.config,
            212 + u8::try_from(case).unwrap(),
            "identity",
            ProcForm::FunctionLike,
            ProcGrammar::Items,
            source,
        );
        grammar.input = request.input.clone();
        grammar.sources = request.sources.clone();
        grammar.definition_site = request.definition_site;
        grammar.call_site = request.call_site;
        grammar.mixed_site = request.mixed_site;
        let checked = session.expand(&grammar).unwrap();
        assert_eq!(
            checked.as_ref(),
            &request.input,
            "pinned item parser must accept retained member separators"
        );
        assert_eq!(
            actual,
            observed.trim(),
            "native derive member input differs for {name}"
        );
    }
    assert_eq!(session.invocations(), 12);
}

#[test]
fn matched_server_export_inventory_cannot_infer_generated_derive_helpers() {
    use lrr_expand_adapter::proc_client::DeriveHelperMetadata;
    let prepared = prepared();
    let root = prepared.work.join("helper-metadata-witness");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"),"[package]\nname=\"helper_metadata_witness\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\nproc-macro=true\n[features]\nfirst=[]\nsecond=[]\n[workspace]\n").unwrap();
    fs::write(root.join("src/lib.rs"),"use proc_macro::TokenStream; macro_rules! declare { ($helper:ident) => { #[proc_macro_derive(Identical, attributes($helper))] pub fn identical(_: TokenStream) -> TokenStream { TokenStream::new() } }; } #[cfg(feature=\"first\")] declare!(first_helper); #[cfg(feature=\"second\")] declare!(second_helper);").unwrap();
    let lock = Command::new("cargo")
        .args([
            "+1.99.0",
            "generate-lockfile",
            "--offline",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .output()
        .unwrap();
    fs::write(root.join("lock.stdout"), &lock.stdout).unwrap();
    fs::write(root.join("lock.stderr"), &lock.stderr).unwrap();
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    let mut inventories = vec![];
    let mut rows = vec![];
    for (case, feature) in ["first", "second"].into_iter().enumerate() {
        let build = Command::new("cargo")
            .args([
                "+1.99.0",
                "build",
                "--locked",
                "--offline",
                "--message-format=json",
                "--features",
                feature,
                "--manifest-path",
            ])
            .arg(root.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(root.join(format!("target-{feature}")))
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_INCREMENTAL", "0")
            .output()
            .unwrap();
        fs::write(
            root.join(format!("{feature}-prepare.stdout")),
            &build.stdout,
        )
        .unwrap();
        fs::write(
            root.join(format!("{feature}-prepare.stderr")),
            &build.stderr,
        )
        .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        let messages: Vec<Value> = String::from_utf8(build.stdout)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let artifacts: Vec<_> = messages
            .iter()
            .filter(|m| m["reason"] == "compiler-artifact")
            .collect();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(
            artifacts[0]["target"]["kind"],
            serde_json::json!(["proc-macro"])
        );
        let paths = artifacts[0]["filenames"].as_array().unwrap();
        assert_eq!(paths.len(), 1);
        let artifact = PathBuf::from(paths[0].as_str().unwrap());
        let raw = blake3::hash(&fs::read(&artifact).unwrap())
            .to_hex()
            .to_string();
        let mut config = prepared.config.clone();
        config.directory = root.clone();
        config.artifacts = vec![MacroArtifact {
            identity: Key([241 + u8::try_from(case).unwrap(); 32]),
            path: artifact.clone(),
            raw_blake3: raw.clone(),
        }];
        let session = ProcSession::new(config.clone()).unwrap();
        let inventory: Vec<_> = session.exports().collect();
        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].artifact, config.artifacts[0].identity);
        assert_eq!(inventory[0].name, "Identical");
        assert_eq!(inventory[0].form, ProcForm::Derive);
        assert_eq!(
            inventory[0].helpers,
            DeriveHelperMetadata::Unavailable { protocol: 6 }
        );
        inventories.push((
            inventory[0].name.to_owned(),
            inventory[0].form,
            inventory[0].helpers,
        ));
        assert_eq!(session.invocations(), 0);
        let mut membership = vec![];
        for helper in ["first_helper", "second_helper"] {
            let path = root.join(format!("{feature}-{helper}.rs"));
            fs::write(
                &path,
                format!("#[derive(helper_metadata_witness::Identical)] #[{helper}] struct Marker;"),
            )
            .unwrap();
            let native = Command::new("rustc")
                .args([
                    "+1.99.0",
                    "--edition=2024",
                    "--crate-type=lib",
                    "--emit=metadata",
                    "--extern",
                ])
                .arg(format!("helper_metadata_witness={}", artifact.display()))
                .arg(&path)
                .arg("-o")
                .arg(root.join(format!("{feature}-{helper}.rmeta")))
                .output()
                .unwrap();
            fs::write(
                root.join(format!("{feature}-{helper}.stderr")),
                &native.stderr,
            )
            .unwrap();
            let expected = helper == format!("{feature}_helper");
            assert_eq!(
                native.status.success(),
                expected,
                "{}",
                String::from_utf8_lossy(&native.stderr)
            );
            if !expected {
                assert!(String::from_utf8_lossy(&native.stderr).contains("cannot find attribute"));
            }
            membership.push(serde_json::json!({"helper":helper,"accepted":native.status.success(),"exit_code":native.status.code()}));
        }
        rows.push(serde_json::json!({"feature":feature,"artifact":artifact,"artifact_raw_blake3":raw,"export_name":inventory[0].name,"kind":"derive","helper_metadata":"unavailable","protocol":6,"native_membership":membership,"session_invocations":session.invocations()}));
    }
    assert_eq!(inventories[0], inventories[1]);
    assert_ne!(
        rows[0]["artifact_raw_blake3"],
        rows[1]["artifact_raw_blake3"]
    );
    fs::write(root.join("witness.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","server":prepared.config.server.path,"server_raw_blake3":prepared.config.server.raw_blake3,"compiler_commit":prepared.config.server.compiler_commit,"scope":"compiled-export helper metadata capability witness; not full helper acceptance","rows":rows})).unwrap()).unwrap();
}

#[test]
fn actual_generated_macro_host_catalog_suspends_without_partial_helper_admission() {
    use lrr::{
        core::{digest::Digest, profile::ProfileIdentity},
        expand::{
            derive_declaration::SourceCatalogAdmission,
            identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
        },
        project::cargo_units::UnitRole,
        service::{
            expansion::DemandContext,
            macro_session::{MacroSession, macro_artifact_identity, procedural_engine_version},
        },
        source::snapshot::SourceBlob,
        syntax::parse::ParsedFile,
    };
    let prepared = prepared();
    let root = prepared.work.join("generated-source-catalog");
    let macro_root = root.join("macros");
    fs::create_dir_all(macro_root.join("src")).unwrap();
    fs::write(macro_root.join("Cargo.toml"),"[package]\nname=\"lrr_fixture_macros\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    fs::write(macro_root.join("src/lib.rs"),"use proc_macro::TokenStream; #[proc_macro_derive(Static,attributes(static_helper))] pub fn original(_:TokenStream)->TokenStream{TokenStream::new()} macro_rules! declare{()=>{#[proc_macro_derive(Generated,attributes(generated_helper))] pub fn generated(_:TokenStream)->TokenStream{TokenStream::new()}}} declare!();").unwrap();
    fs::write(macro_root.join("src/unused.rs"),"#[proc_macro_derive(Static,attributes(wrong))] pub fn fake(_:TokenStream)->TokenStream{TokenStream::new()}").unwrap();
    let manifest = service_fixture::prepare(&root.join("preparation"), &macro_root);
    let unit = *manifest.graph.roots.first().unwrap();
    let producer = *manifest
        .graph
        .units
        .iter()
        .find(|(_, u)| u.key.role == UnitRole::ProcMacroHost)
        .unwrap()
        .0;
    let context = DemandContext {
        request: Digest([251; 32]),
        session: Digest([252; 32]),
        unit,
        revision: 1,
        epoch: 1,
        source_generation: Digest([253; 32]),
        native_generation: None,
        compile_environment: lrr::core::digest::StableEncode::fingerprint(
            &manifest.compile_env[&unit],
        ),
    };
    let directory = PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]);
    let mut service = MacroSession::open(
        context.clone(),
        &manifest,
        true,
        directory,
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .unwrap();
    let source_path = macro_root.join("src/lib.rs");
    let blob = SourceBlob::new(fs::read(&source_path).unwrap()).unwrap();
    let artifact = macro_artifact_identity(producer, &manifest.artifacts[&producer]);
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Procedural {
            artifact,
            export: "Static".into(),
            kind: ProceduralKind::Derive,
        },
        invocation: Digest([254; 32]),
        input_tokens: Digest([255; 32]),
        consumer_compile_env: context.compile_environment,
        parent_context: Digest([1; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Items,
        engine_version: procedural_engine_version(),
        procedural_epoch: Some(context.epoch),
    };
    assert_eq!(
        service
            .exports(&context)
            .unwrap()
            .filter(|e| e.form == ProcForm::Derive)
            .count(),
        2
    );
    assert!(service.derive_helpers(&context, &key).is_err());
    let decoy_path = macro_root.join("src/unused.rs");
    let decoy = SourceBlob::new(fs::read(&decoy_path).unwrap()).unwrap();
    assert_eq!(
        manifest.inputs.source_files[decoy_path.to_str().unwrap()],
        Some(decoy.content()),
        "decoy is captured but not the compiled source root"
    );
    let decoy_parsed = ParsedFile::parse(&decoy, ProfileIdentity::current()).unwrap();
    let decoy_range = decoy_parsed.source_function_range("fake").unwrap();
    assert_eq!(
        service
            .register_source_derive(
                &context,
                &manifest,
                producer,
                &decoy_path,
                &decoy,
                decoy_range
            )
            .unwrap_err()
            .code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
    assert!(
        service
            .register_source_derive_catalog(&context, &manifest, producer, &decoy_path, &decoy)
            .is_err()
    );

    let SourceCatalogAdmission::NeedExpansion(pending) = service
        .register_source_derive_catalog(&context, &manifest, producer, &source_path, &blob)
        .unwrap()
    else {
        panic!("enabled producer must remain pending")
    };
    assert_eq!(pending.len(), 1);
    assert!(
        service.derive_helpers(&context, &key).is_err(),
        "pending catalog must not partially commit Static"
    );
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let selected = parsed.source_function_range("original").unwrap();
    service
        .register_source_derive(&context, &manifest, producer, &source_path, &blob, selected)
        .unwrap();
    assert_eq!(
        service.derive_helpers(&context, &key).unwrap(),
        ["static_helper"]
    );
    let mut generated = key.clone();
    if let MacroIdentity::Procedural { export, .. } = &mut generated.macro_identity {
        *export = "Generated".into();
    }
    assert_eq!(
        service
            .derive_helpers(&context, &generated)
            .unwrap_err()
            .code,
        "LRR-E-MACRO-SERVER-MISMATCH"
    );
    assert_eq!(service.invocations(), 0);
}

#[test]
fn actual_older_macro_source_facets_preserve_definition_edition_for_keyword_helpers() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        expand::{
            derive_declaration::SourceCatalogAdmission,
            identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
        },
        project::cargo_units::UnitRole,
        service::{
            expansion::DemandContext,
            macro_session::{MacroSession, macro_artifact_identity, procedural_engine_version},
        },
        source::snapshot::SourceBlob,
    };
    let prepared = prepared();
    let root = prepared.work.join("older-source-facets");
    fs::create_dir_all(&root).unwrap();
    let mut rows = vec![];
    for edition in [2015, 2018, 2021] {
        let case = root.join(edition.to_string());
        let macro_root = case.join("macros");
        fs::create_dir_all(macro_root.join("src")).unwrap();
        fs::write(macro_root.join("Cargo.toml"),format!("[package]\nname=\"lrr_fixture_macros\"\nversion=\"0.1.0\"\nedition=\"{edition}\"\n[lib]\nproc-macro=true\n[workspace]\n")).unwrap();
        fs::write(macro_root.join("src/lib.rs"),"extern crate proc_macro; use proc_macro::TokenStream; #[proc_macro_derive(gen,attributes(gen))] pub fn implementation(_:TokenStream)->TokenStream{TokenStream::new()}").unwrap();
        let manifest = service_fixture::prepare(&case.join("preparation"), &macro_root);
        let unit = *manifest.graph.roots.first().unwrap();
        let producer = *manifest
            .graph
            .units
            .iter()
            .find(|(_, u)| u.key.role == UnitRole::ProcMacroHost)
            .unwrap()
            .0;
        assert_eq!(manifest.graph.units[&unit].key.edition, 2024);
        assert_eq!(manifest.graph.units[&producer].key.edition, edition);
        let context = DemandContext {
            request: Digest([11; 32]),
            session: Digest([12; 32]),
            unit,
            revision: 1,
            epoch: 1,
            source_generation: Digest([13; 32]),
            native_generation: None,
            compile_environment: manifest.compile_env[&unit].fingerprint(),
        };
        let directory = PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]);
        let mut service = MacroSession::open(
            context.clone(),
            &manifest,
            true,
            directory,
            Duration::from_secs(10),
            4 * 1024 * 1024,
        )
        .unwrap();
        let compiled_exports = service.export_catalog(&context).unwrap();
        for export in compiled_exports.exports() {
            assert_eq!(export.definition().defining_unit, producer);
            assert_eq!(export.definition().definition_edition, edition);
        }
        let source_path = macro_root.join("src/lib.rs");
        let blob = SourceBlob::new(fs::read(&source_path).unwrap()).unwrap();
        let SourceCatalogAdmission::Ready(ids) = service
            .register_source_derive_catalog(&context, &manifest, producer, &source_path, &blob)
            .unwrap()
        else {
            panic!("older source metadata must complete")
        };
        assert_eq!(ids.len(), 1);
        let artifact = macro_artifact_identity(producer, &manifest.artifacts[&producer]);
        let mut config = prepared.config.clone();
        config.unit = Key(unit.0);
        config.epoch = context.epoch;
        config.artifacts[0].identity = Key(artifact.0);
        let mut request = invocation(
            &config,
            14,
            "gen",
            ProcForm::Derive,
            ProcGrammar::Items,
            "struct Marker;",
        );
        assert_eq!(
            service.request_key(&context, &request).unwrap_err().code,
            "LRR-E-MACRO-SERVER-MISMATCH"
        );
        let definition = token_wire::parse_source(
            "",
            SourceIdentity {
                unit: Key(producer.0),
                content: Key(*blake3::hash(b"").as_bytes()),
                path: "definition.rs".into(),
            },
            ContextRecord {
                parent: None,
                expansion: None,
                transparency: Transparency::Opaque,
                defining_unit: Key(producer.0),
                edition: match edition {
                    2015 => Edition::E2015,
                    2018 => Edition::E2018,
                    2021 => Edition::E2021,
                    _ => unreachable!(),
                },
            },
            Key([18; 32]),
        )
        .unwrap();
        let [TokenTree::Group { open, .. }] = definition.roots() else {
            panic!("missing definition root")
        };
        request.definition_site = *open;
        let mut contexts = request.input.contexts().to_vec();
        contexts.extend_from_slice(definition.contexts());
        let mut spans = request.input.spans().to_vec();
        spans.extend_from_slice(definition.spans());
        request.input = TokenForest::new(request.input.roots().to_vec(), contexts, spans).unwrap();
        let request_key = service.request_key(&context, &request).unwrap();
        assert_eq!(request_key.definition_edition, edition);
        assert_eq!(request_key.call_edition, 2024);
        assert_eq!(service.invocations(), 0);
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Procedural {
                artifact,
                export: "gen".into(),
                kind: ProceduralKind::Derive,
            },
            invocation: Digest([14; 32]),
            input_tokens: Digest([15; 32]),
            consumer_compile_env: context.compile_environment,
            parent_context: Digest([16; 32]),
            definition_edition: edition,
            call_edition: 2024,
            grammar: GrammarSlot::Items,
            engine_version: procedural_engine_version(),
            procedural_epoch: Some(context.epoch),
        };
        assert_eq!(service.derive_helpers(&context, &key).unwrap(), ["gen"]);
        service.derive_helper_binding(&context, &key).unwrap();
        let mut wrong = key.clone();
        wrong.definition_edition = 2024;
        assert_eq!(
            service.derive_helpers(&context, &wrong).unwrap_err().code,
            "LRR-E-MACRO-SERVER-MISMATCH"
        );
        let path = case.join("native.rs");
        fs::write(
            &path,
            "#[derive(lrr_fixture_macros::r#gen)] #[r#gen] struct Marker;",
        )
        .unwrap();
        let native = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-type=lib",
                "--emit=metadata",
                "--extern",
            ])
            .arg(format!(
                "lrr_fixture_macros={}",
                manifest.artifacts[&producer].path
            ))
            .arg(&path)
            .arg("-o")
            .arg(case.join("native.rmeta"))
            .output()
            .unwrap();
        fs::write(case.join("native.stderr"), &native.stderr).unwrap();
        assert!(
            native.status.success(),
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert_eq!(service.invocations(), 0);
        rows.push(serde_json::json!({"definition_edition":edition,"consumer_edition":2024,"producer":producer,"artifact":artifact,"declaration":ids[0],"helpers":service.derive_helpers(&context,&key).unwrap(),"native_exit_code":native.status.code(),"session_invocations":service.invocations()}));
    }
    fs::write(root.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"source-facet metadata and native helper membership; not guest execution","rows":rows})).unwrap()).unwrap();
}

#[test]
fn actual_original_item_requests_admit_exact_worklist_selections_and_match_native() {
    use lrr::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        db::{key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider},
        expand::{
            record::ExpansionRecord,
            worklist::{AttributeSlot, DeriveSlot, ItemPlan, ItemTransformWorklist, TransformStep},
        },
        service::{
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
            item_macro_inputs,
            item_macro_requests::{PreparedItemRequest, ResolvedItemTarget},
            macro_session::{MacroSession, macro_artifact_identity},
        },
        source::{
            manifest::BodyKey,
            snapshot::{CfgSet, SourceBlob},
        },
        syntax::{parse::ParsedFile, spans::SourceRange},
    };
    fn execute(
        service: &mut MacroSession,
        context: &DemandContext,
        db: &mut Database,
        coordinator: &mut ExpansionCoordinator,
        worklist: &mut ItemTransformWorklist,
        request: &PreparedItemRequest,
    ) -> ExpansionRecord {
        let TransformStep::Expand(key) = worklist.poll() else {
            panic!("request did not admit")
        };
        assert_eq!(key, request.key());
        let query = QueryKey::ExpansionInput(Box::new(key.clone()));
        let QueryPoll::NeedInput(input) = db.fetch(&SyntaxProvider, query.clone()) else {
            panic!("missing output must suspend")
        };
        let phase = PhaseIdentity {
            frame: key.invocation,
            body: BodyKey {
                definition: Digest([41; 32]),
                exact_body_bytes: Digest([42; 32]),
            },
            index: 0,
        };
        let token = coordinator.issue(query, phase, input).unwrap();
        let record = service
            .fulfil(context, coordinator, &token, request.request())
            .unwrap();
        assert!(
            coordinator
                .admit_transform(context, &token, db, worklist, record.clone())
                .is_err()
        );
        coordinator
            .adopt(context, &token, db, record.clone())
            .unwrap();
        coordinator
            .admit_transform(context, &token, db, worklist, record.clone())
            .unwrap();
        record
    }
    let prepared = prepared();
    let evidence = prepared.work.join("original-item-requests");
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let (producer, artifact) = manifest.artifacts.iter().next().unwrap();
    let artifact_key = Key(macro_artifact_identity(*producer, artifact).0);
    let mut db = Database::default();
    db.begin_run().unwrap();
    let context = DemandContext {
        request: Digest([31; 32]),
        session: Digest([32; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut service = MacroSession::open(
        context.clone(),
        &manifest,
        true,
        PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]),
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .unwrap();
    let catalog = service.export_catalog(&context).unwrap();
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let mut cfg = CfgSet::default();
    cfg.atoms.insert("lrr_fixture_active".into());
    let mut rows = vec![];
    for (case, text, main) in [
        (
            "attribute",
            "#[cfg_attr(lrr_fixture_inactive, lrr_fixture_macros::panic_attribute)] #[cfg_attr(lrr_fixture_active, lrr_fixture_macros::attribute_tokens(struct Replaced;))] #[unresolved_later_attribute] struct Removed;",
            "#[cfg(lrr_fixture_inactive)] #[lrr_fixture_macros::panic_attribute] fn disabled(){missing();} fn main(){let _ = Replaced; println!(\"Replaced\");}",
        ),
        (
            "derive",
            "#[derive(lrr_fixture_macros::MemberCfg, lrr_fixture_macros::Empty, lrr_fixture_macros::Empty)] struct Marker { #[cfg(any())] missing: Missing, kept: i64 }",
            "fn main(){println!(\"{}\",MEMBER_NAMES);}",
        ),
    ] {
        let invocations_before = service.invocations();
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let range = SourceRange {
            start: 0,
            end: u32::try_from(text.len()).unwrap(),
        };
        let source = SourceIdentity {
            unit: Key(unit.0),
            content: Key(Digest::of_bytes(text.as_bytes()).0),
            path: format!("{case}.rs"),
        };
        let original = token_wire::parse_source(
            text,
            source.clone(),
            ContextRecord {
                parent: None,
                expansion: None,
                transparency: Transparency::Opaque,
                defining_unit: Key(unit.0),
                edition: Edition::E2024,
            },
            Key([43; 32]),
        )
        .unwrap();
        let runtime =
            lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
        let [TokenTree::Group { open, .. }] = original.roots() else {
            panic!("missing item root")
        };
        let definition_site = *open;
        let target = |export: &str, call_site: Key| ResolvedItemTarget {
            artifact: artifact_key,
            export: export.into(),
            scope: Key([44; 32]),
            parent_origin: Key([43; 32]),
            registry: &original,
            definition_site,
            call_site,
            mixed_site: call_site,
            sources: vec![
                lrr_expand_adapter::proc_client::CapturedSpanSource::new(
                    source.clone(),
                    text.into(),
                    lrr_expand_adapter::proc_client::SourceDigestEncoding::LrrSourceBytes,
                    None,
                )
                .unwrap(),
            ],
        };
        let owner = Digest::of_bytes(case.as_bytes());
        use lrr::{
            expand::{
                identity::{MacroIdentity, ProceduralKind},
                scopes::MacroName,
            },
            frontend::{
                hygiene::HygieneFacts,
                macro_names::{MacroPath, PathLookup},
                source_macro_routes::SourceMacroRoutes,
            },
        };
        let mut hygiene = lrr_expand_adapter::hygiene::HygieneStore::default();
        let ctx = hygiene.root(Key(unit.0), Edition::E2024).unwrap();
        let resolution = hygiene.resolution_contexts(&[ctx]).unwrap();
        let names = hygiene.name_contexts(&[ctx]).unwrap();
        let hygiene_facts = || {
            HygieneFacts::new(
                lrr::syntax::token_wire::TokenForest::decode(
                    &TokenForest::new(vec![], hygiene.records(), vec![])
                        .unwrap()
                        .encode()
                        .unwrap(),
                )
                .unwrap()
                .contexts()
                .to_vec(),
                serde_json::from_slice(&serde_json::to_vec(&resolution).unwrap()).unwrap(),
            )
            .unwrap()
            .with_name_contexts(
                serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap(),
            )
            .unwrap()
        };
        let outline = parsed.outline(unit, vec![], &cfg).unwrap();
        let source_index = lrr::frontend::source_macros::SourceMacroIndex::from_outline(
            &outline,
            lrr::syntax::token_wire::Key(ctx.0),
            hygiene_facts(),
        )
        .unwrap();
        let mut routes = SourceMacroRoutes::from_procedural_catalog(&catalog).unwrap();
        routes.push(
            SourceMacroRoutes::from_outline(
                &outline,
                lrr::syntax::token_wire::Key(ctx.0),
                hygiene_facts(),
                &BTreeMap::from([(
                    MacroName::parse("lrr_fixture_macros", 2024).unwrap(),
                    *producer,
                )]),
            )
            .unwrap(),
        );
        let namespaces = SourceMacroRoutes::candidate_graph(&routes).unwrap();
        let resolved_target = |segments: &[String], kind: ProceduralKind, call_site| {
            let path = MacroPath::new(
                unit,
                segments
                    .iter()
                    .map(|s| MacroName::parse(s, 2024).unwrap())
                    .collect(),
            )
            .unwrap();
            let caller = lrr::syntax::outline::ScopeKey {
                crate_key: unit,
                module_path: vec![],
                owner: None,
            };
            let PathLookup::Defined(definition) = namespaces.lookup_from(&path, &caller).unwrap()
            else {
                panic!("source item macro did not resolve")
            };
            let selected = catalog.select(&definition, kind).unwrap().unwrap();
            let MacroIdentity::Procedural {
                artifact, export, ..
            } = selected.identity()
            else {
                panic!("catalog identity differs")
            };
            let mut result = target(export, call_site);
            result.artifact = Key(artifact.0);
            result
        };
        let item = Arc::new(runtime.clone());
        let output_names;
        if case == "attribute" {
            let facts = parsed.attribute_inputs_at(range, &cfg).unwrap();
            let input = item_macro_inputs::attribute(
                owner,
                &facts,
                0,
                &runtime.spans()[0].source,
                &runtime,
            )
            .unwrap();
            let mut pending: Vec<_> = original.roots().iter().collect();
            let call_site = loop {
                match pending.pop().expect("attribute token absent") {
                    TokenTree::Group { children, .. } => pending.extend(children),
                    TokenTree::Ident { text, span, .. } if text == "attribute_tokens" => {
                        break *span;
                    }
                    _ => {}
                }
            };
            assert!(!input.item.spans().iter().any(|s| s.key().0 == call_site.0));
            let mut missing_site = target("attribute_tokens", call_site);
            missing_site.mixed_site = Key([255; 32]);
            assert!(
                PreparedItemRequest::attribute(&service, &context, &input, missing_site).is_err()
            );
            assert_eq!(
                PreparedItemRequest::attribute(
                    &service,
                    &context,
                    &input,
                    target("absent", call_site)
                )
                .err()
                .unwrap()
                .code,
                "LRR-E-MACRO-NOT-FOUND"
            );
            let mut excessive = target("attribute_tokens", call_site);
            let mut excessive_export = String::with_capacity(64 * 1024 * 1024 + 1);
            excessive_export.push_str("attribute_tokens");
            excessive.export = excessive_export;
            assert_eq!(
                PreparedItemRequest::attribute(&service, &context, &input, excessive)
                    .err()
                    .unwrap()
                    .code,
                "LRR-E-MACRO-TRANSFORM"
            );
            let mut stale = context.clone();
            stale.epoch += 1;
            assert!(
                PreparedItemRequest::attribute(
                    &service,
                    &stale,
                    &input,
                    target("attribute_tokens", call_site)
                )
                .is_err()
            );
            assert_eq!(service.invocations(), 0);
            let request = PreparedItemRequest::attribute(
                &service,
                &context,
                &input,
                resolved_target(
                    &["lrr_fixture_macros".into(), "attribute_tokens".into()],
                    ProceduralKind::Attribute,
                    call_site,
                ),
            )
            .unwrap();
            let mut altered = item_macro_inputs::attribute(
                owner,
                &facts,
                0,
                &runtime.spans()[0].source,
                &runtime,
            )
            .unwrap();
            altered.identity = Digest([253; 32]);
            assert!(
                PreparedItemRequest::attribute(
                    &service,
                    &context,
                    &altered,
                    target("attribute_tokens", call_site)
                )
                .is_err()
            );
            assert!(
                request
                    .request()
                    .input
                    .spans()
                    .iter()
                    .any(|s| s.key() == call_site)
            );
            let wrong_plan = ItemPlan::new(
                owner,
                cfg.fingerprint(),
                item.clone(),
                vec![AttributeSlot::Unresolved(Digest([255; 32]))],
                vec![],
            )
            .unwrap();
            let mut wrong_worklist =
                ItemTransformWorklist::new(wrong_plan, context.compile_environment, context.epoch)
                    .unwrap();
            assert!(request.admit(&mut wrong_worklist, 0, 0).is_err());
            let plan = ItemPlan::new(
                owner,
                cfg.fingerprint(),
                item,
                vec![AttributeSlot::Unresolved(input.identity)],
                vec![],
            )
            .unwrap();
            let mut worklist =
                ItemTransformWorklist::new(plan, context.compile_environment, context.epoch)
                    .unwrap();
            let helpers =
                lrr::expand::derive::ItemHelperScope::new(owner, cfg.fingerprint(), vec![])
                    .unwrap();
            let builtin =
                lrr::frontend::builtin_attributes::BuiltinAttributeCatalog::new(unit, 2024, [])
                    .unwrap();
            let resolver = lrr::frontend::original_attributes::OriginalAttributeResolver {
                source: &source_index,
                namespaces: &namespaces,
                procedural: &catalog,
                builtin: &builtin,
                helpers: &helpers,
                owner,
                cfg: cfg.fingerprint(),
                active_derives: &[],
            };
            let cursor = lrr::frontend::original_attributes::OriginalItemAttributeCursor::new(
                &resolver,
                &parsed,
                lrr::syntax::outline::ScopeKey {
                    crate_key: unit,
                    module_path: vec![],
                    owner: None,
                },
                range,
            )
            .unwrap()
            .unwrap();
            assert!(
                request
                    .admit_original_attribute(&cursor, &resolver, &parsed, &mut worklist, 1)
                    .is_err()
            );
            let disabled_text = "#[cfg(lrr_fixture_inactive)] #[lrr_fixture_macros::panic_attribute] fn disabled(){missing();}";
            let disabled = ParsedFile::parse(
                &SourceBlob::new(disabled_text.as_bytes().to_vec()).unwrap(),
                ProfileIdentity::current(),
            )
            .unwrap();
            let disabled_range = SourceRange {
                start: 0,
                end: u32::try_from(disabled_text.len()).unwrap(),
            };
            let disabled_inputs = disabled.attribute_inputs_at(disabled_range, &cfg).unwrap();
            assert!(!disabled_inputs.enabled);
            assert!(disabled_inputs.attributes.is_empty());
            let disabled_outline = disabled.outline(unit, vec![], &cfg).unwrap();
            assert!(disabled_outline.items.is_empty());
            let disabled_index = lrr::frontend::source_macros::SourceMacroIndex::from_outline(
                &disabled_outline,
                lrr::syntax::token_wire::Key(ctx.0),
                hygiene_facts(),
            )
            .unwrap();
            let disabled_resolver = lrr::frontend::original_attributes::OriginalAttributeResolver {
                source: &disabled_index,
                namespaces: &namespaces,
                procedural: &catalog,
                builtin: &builtin,
                helpers: &helpers,
                owner,
                cfg: cfg.fingerprint(),
                active_derives: &[],
            };
            assert!(
                lrr::frontend::original_attributes::OriginalItemAttributeCursor::new(
                    &disabled_resolver,
                    &disabled,
                    lrr::syntax::outline::ScopeKey {
                        crate_key: unit,
                        module_path: vec![],
                        owner: None
                    },
                    disabled_range,
                )
                .unwrap()
                .is_none()
            );
            assert_eq!(service.invocations(), 0);
            let different = PreparedItemRequest::attribute(
                &service,
                &context,
                &input,
                target("make_tests", call_site),
            )
            .unwrap();
            assert!(
                different
                    .admit_original_attribute(&cursor, &resolver, &parsed, &mut worklist, 0)
                    .is_err()
            );
            let later_input = item_macro_inputs::attribute(
                owner,
                &facts,
                1,
                &runtime.spans()[0].source,
                &runtime,
            )
            .unwrap();
            let later = PreparedItemRequest::attribute(
                &service,
                &context,
                &later_input,
                target("attribute_tokens", call_site),
            )
            .unwrap();
            assert!(
                later
                    .admit_original_attribute(&cursor, &resolver, &parsed, &mut worklist, 0)
                    .is_err()
            );
            let foreign = ParsedFile::parse(
                &SourceBlob::new(b"struct Other;".to_vec()).unwrap(),
                ProfileIdentity::current(),
            )
            .unwrap();
            assert!(
                request
                    .admit_original_attribute(&cursor, &resolver, &foreign, &mut worklist, 0)
                    .is_err()
            );
            assert_eq!(service.invocations(), 0);
            for (other_owner, other_cfg) in [
                (Digest([254; 32]), cfg.fingerprint()),
                (owner, Digest([254; 32])),
            ] {
                let mut foreign_worklist = ItemTransformWorklist::new(
                    ItemPlan::new(
                        other_owner,
                        other_cfg,
                        Arc::new(runtime.clone()),
                        vec![AttributeSlot::Unresolved(input.identity)],
                        vec![],
                    )
                    .unwrap(),
                    context.compile_environment,
                    context.epoch,
                )
                .unwrap();
                assert!(
                    request
                        .admit_original_attribute(
                            &cursor,
                            &resolver,
                            &parsed,
                            &mut foreign_worklist,
                            0
                        )
                        .is_err()
                );
            }
            request
                .admit_original_attribute(&cursor, &resolver, &parsed, &mut worklist, 0)
                .unwrap();
            assert!(
                request
                    .admit_original_attribute(&cursor, &resolver, &parsed, &mut worklist, 0)
                    .is_err()
            );
            let record = execute(
                &mut service,
                &context,
                &mut db,
                &mut coordinator,
                &mut worklist,
                &request,
            );
            let TransformStep::NeedReanalysis { tokens, .. } = worklist.poll() else {
                panic!("attribute must replace")
            };
            let adapter = TokenForest::decode(&tokens.encode().unwrap()).unwrap();
            output_names = identifiers(&adapter);
            assert!(output_names.contains(&"Replaced".into()));
            assert!(!output_names.contains(&"Removed".into()));
            assert_eq!(record.key(), request.key());
        } else {
            let facts = parsed.derive_item_inputs_at(range, &cfg).unwrap();
            let input =
                item_macro_inputs::derive(owner, &facts, 0, &runtime.spans()[0].source, &runtime)
                    .unwrap();
            let ids: Vec<_> = (0..input.selections.len())
                .map(|i| input.selection_identity(i).unwrap())
                .collect();
            assert_ne!(ids[1], ids[2]);
            let plan = ItemPlan::new(
                owner,
                cfg.fingerprint(),
                item.clone(),
                vec![AttributeSlot::Inert],
                ids.into_iter().map(DeriveSlot::Unresolved).collect(),
            )
            .unwrap();
            let mut worklist =
                ItemTransformWorklist::new(plan, context.compile_environment, context.epoch)
                    .unwrap();
            let wrong = PreparedItemRequest::derive(
                &service,
                &context,
                &input,
                1,
                target("Empty", Key(input.selections[1].first_span.0)),
            )
            .unwrap();
            assert!(wrong.admit(&mut worklist, 0, 0).is_err());
            assert!(
                PreparedItemRequest::derive(
                    &service,
                    &context,
                    &input,
                    99,
                    target("Empty", definition_site)
                )
                .is_err()
            );
            let mut first_output = None;
            for (i, selection) in input.selections.iter().enumerate() {
                let request = PreparedItemRequest::derive(
                    &service,
                    &context,
                    &input,
                    i,
                    resolved_target(
                        &selection.segments,
                        ProceduralKind::Derive,
                        Key(selection.first_span.0),
                    ),
                )
                .unwrap();
                request.admit(&mut worklist, i, 0).unwrap();
                let record = execute(
                    &mut service,
                    &context,
                    &mut db,
                    &mut coordinator,
                    &mut worklist,
                    &request,
                );
                if i == 0 {
                    first_output = Some(record);
                }
            }
            let TransformStep::Complete {
                item: retained,
                appended,
            } = worklist.poll()
            else {
                panic!("derive siblings must finish")
            };
            assert_eq!(retained.encode().unwrap(), item.encode().unwrap());
            assert_eq!(appended.len(), 3);
            let adapter =
                TokenForest::decode(&first_output.unwrap().tokens().unwrap().encode().unwrap())
                    .unwrap();
            output_names = literals(&adapter);
        }
        let native_source = evidence.join(format!("{case}-native.rs"));
        fs::write(&native_source, format!("{text}\n{main}\n")).unwrap();
        let executable = evidence.join(format!("{case}-native"));
        let compiled = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name",
                "item_oracle",
                "--cfg=lrr_fixture_active",
                "--check-cfg=cfg(lrr_fixture_active,lrr_fixture_inactive)",
                "--extern",
            ])
            .arg(format!("lrr_fixture_macros={}", artifact.path))
            .arg(&native_source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        fs::write(
            evidence.join(format!("{case}-native.stderr")),
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
        let value = String::from_utf8(native.stdout).unwrap().trim().to_owned();
        if case == "attribute" {
            assert!(output_names.contains(&value));
            let enabled_failure = Command::new("rustc")
                .args([
                    "+1.99.0",
                    "--edition=2024",
                    "--crate-name",
                    "cfg_failure_control",
                    "--cfg=lrr_fixture_active",
                    "--cfg=lrr_fixture_inactive",
                    "--check-cfg=cfg(lrr_fixture_active,lrr_fixture_inactive)",
                    "--extern",
                ])
                .arg(format!("lrr_fixture_macros={}", artifact.path))
                .arg(&native_source)
                .arg("-o")
                .arg(evidence.join("cfg-failure-control"))
                .output()
                .unwrap();
            fs::write(
                evidence.join("cfg-failure-control.stderr"),
                &enabled_failure.stderr,
            )
            .unwrap();
            assert!(!enabled_failure.status.success());
            assert!(
                String::from_utf8_lossy(&enabled_failure.stderr)
                    .contains("controlled inactive attribute must not run")
            );
        } else {
            assert!(output_names.contains(&format!("{value:?}")));
        }
        let invocation_delta = service.invocations() - invocations_before;
        assert_eq!(invocation_delta, if case == "attribute" { 1 } else { 3 });
        rows.push(serde_json::json!({"case":case,"native_observed":value,"macro_output_tokens":output_names,"session_invocations":service.invocations(),"invocation_delta":invocation_delta,"cfg_atoms":cfg.atoms,"cfg_digest":cfg.fingerprint()}));
    }
    assert_eq!(service.invocations(), 4);
    fs::write(evidence.join("evidence.json"), serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"original input/request/worklist/coordinator integration with native oracle; no guest or generated-source discovery claim","rows":rows})).unwrap()).unwrap();
}

#[test]
fn actual_prepared_procedural_exports_follow_source_import_aliases_and_exact_kinds() {
    use lrr::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        expand::{
            identity::{MacroIdentity, ProceduralKind},
            scopes::MacroName,
        },
        frontend::{
            hygiene::HygieneFacts,
            macro_names::{MacroPath, PathLookup},
            source_macro_routes::SourceMacroRoutes,
        },
        service::{
            expansion::DemandContext,
            macro_session::{MacroSession, macro_artifact_identity},
        },
        source::snapshot::{CfgSet, SourceBlob},
        syntax::{outline::ScopeKey, parse::ParsedFile, token_wire as runtime},
    };
    let prepared = prepared();
    let evidence = prepared.work.join("prepared-procedural-catalog");
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let consumer = *manifest.graph.roots.first().unwrap();
    let (producer, artifact) = manifest.artifacts.iter().next().unwrap();
    let context = DemandContext {
        request: Digest([61; 32]),
        session: Digest([62; 32]),
        unit: consumer,
        revision: 0,
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&consumer].fingerprint(),
    };
    let service = MacroSession::open(
        context.clone(),
        &manifest,
        true,
        PathBuf::from(&manifest.compile_env[&consumer].cargo["CARGO_MANIFEST_DIR"]),
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .unwrap();
    let catalog = service.export_catalog(&context).unwrap();
    assert_eq!(
        catalog.exports().count(),
        service.exports(&context).unwrap().count()
    );
    let stable: Vec<_> = catalog.exports().map(|e| e.definition().clone()).collect();
    assert_eq!(
        stable,
        service
            .export_catalog(&context)
            .unwrap()
            .exports()
            .map(|e| e.definition().clone())
            .collect::<Vec<_>>()
    );
    let mut stale = context.clone();
    stale.epoch += 1;
    assert!(service.export_catalog(&stale).is_err());
    let source = "use lrr_fixture_macros as renamed; use renamed::Empty as Chosen; use renamed::attribute_tokens as transform; #[derive(Chosen, renamed::r#gen)] struct Original; #[transform(struct Selected;)] struct Removed; fn main(){let _=Selected;let _=Original;println!(\"{}\",std::any::type_name::<Selected>());}";
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let outline = ParsedFile::parse(&blob, ProfileIdentity::current())
        .unwrap()
        .outline(consumer, vec![], &CfgSet::default())
        .unwrap();
    let mut hygiene = lrr_expand_adapter::hygiene::HygieneStore::default();
    let ctx = hygiene.root(Key(consumer.0), Edition::E2024).unwrap();
    let rows = hygiene.resolution_contexts(&[ctx]).unwrap();
    let names = hygiene.name_contexts(&[ctx]).unwrap();
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
    let dependencies = BTreeMap::from([(
        MacroName::parse("lrr_fixture_macros", 2024).unwrap(),
        *producer,
    )]);
    let mut routes = SourceMacroRoutes::from_procedural_catalog(&catalog).unwrap();
    routes.push(
        SourceMacroRoutes::from_outline(&outline, runtime::Key(ctx.0), facts, &dependencies)
            .unwrap(),
    );
    let graph = SourceMacroRoutes::candidate_graph(&routes).unwrap();
    let caller = ScopeKey {
        crate_key: consumer,
        module_path: vec![],
        owner: None,
    };
    let mut observations = vec![];
    for (path, expected, export_name) in [
        (vec!["Chosen"], ProceduralKind::Derive, "Empty"),
        (
            vec!["transform"],
            ProceduralKind::Attribute,
            "attribute_tokens",
        ),
        (
            vec!["renamed", "identity"],
            ProceduralKind::Function,
            "identity",
        ),
        (vec!["renamed", "r#gen"], ProceduralKind::Derive, "gen"),
    ] {
        let path = MacroPath::new(
            consumer,
            path.iter()
                .map(|s| MacroName::parse(s, 2024).unwrap())
                .collect(),
        )
        .unwrap();
        let PathLookup::Defined(definition) = graph.lookup_from(&path, &caller).unwrap() else {
            panic!("source procedural import did not resolve")
        };
        assert_eq!(definition.defining_unit, *producer);
        let selected = catalog.select(&definition, expected).unwrap().unwrap();
        assert_eq!(
            selected.identity(),
            &MacroIdentity::Procedural {
                artifact: macro_artifact_identity(*producer, artifact),
                export: export_name.into(),
                kind: expected
            }
        );
        let wrong_kind = if expected == ProceduralKind::Attribute {
            ProceduralKind::Derive
        } else {
            ProceduralKind::Attribute
        };
        assert!(catalog.select(&definition, wrong_kind).is_err());
        let mut forged = definition.clone();
        forged.defining_unit = consumer;
        assert!(catalog.select(&forged, expected).is_err());
        observations.push(serde_json::json!({"path":path.components().iter().map(|n|n.as_str()).collect::<Vec<_>>(),"identity":selected.identity(),"definition":definition.identity}));
    }
    assert_eq!(service.invocations(), 0);
    let native_path = evidence.join("native.rs");
    fs::write(&native_path, source).unwrap();
    let executable = evidence.join("native");
    let compiled = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-name",
            "catalog_oracle",
            "--extern",
        ])
        .arg(format!("lrr_fixture_macros={}", artifact.path))
        .arg(&native_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    fs::write(evidence.join("native.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let native = Command::new(executable).output().unwrap();
    assert!(native.status.success());
    let native_stdout = String::from_utf8(native.stdout).unwrap();
    assert!(!native_stdout.trim().is_empty());
    assert_eq!(service.invocations(), 0);
    fs::write(evidence.join("evidence.json"), serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"compiled export facet into existing source namespace; native alias acceptance; no guest execution or helper inference","exports":catalog.exports().count(),"observations":observations,"native_stdout":native_stdout,"session_invocations":service.invocations()})).unwrap()).unwrap();
}

#[test]
fn actual_custom_attribute_classification_matches_native_helper_precedence() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        expand::{derive::ItemHelperScope, identity::ProceduralKind, scopes::MacroName},
        frontend::{
            item_attributes::{CustomAttributeDecision, classify_custom_attribute},
            macro_names::PathLookup,
        },
        service::{
            expansion::DemandContext,
            macro_session::{MacroSession, macro_artifact_identity},
        },
        source::snapshot::SourceBlob,
    };
    let prepared = prepared();
    let root = prepared.work.join("helper-attribute-precedence");
    let manifest = service_fixture::prepare(&root, &prepared.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let (producer, artifact) = manifest.artifacts.iter().next().unwrap();
    let context = DemandContext {
        request: Digest([71; 32]),
        session: Digest([72; 32]),
        unit,
        revision: 0,
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut service = MacroSession::open(
        context.clone(),
        &manifest,
        true,
        PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]),
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .unwrap();
    let source_path = PathBuf::from(&manifest.graph.units[producer].source_path);
    let blob = SourceBlob::new(fs::read(&source_path).unwrap()).unwrap();
    assert!(matches!(
        service
            .register_source_derive_catalog(&context, &manifest, *producer, &source_path, &blob)
            .unwrap(),
        lrr::expand::derive_declaration::SourceCatalogAdmission::Ready(_)
    ));
    let mut config = prepared.config.clone();
    config.unit = Key(unit.0);
    config.epoch = context.epoch;
    config.artifacts[0].identity = Key(macro_artifact_identity(*producer, artifact).0);
    let request = invocation(
        &config,
        97,
        "MemberCfg",
        ProcForm::Derive,
        ProcGrammar::Items,
        "struct Marker;",
    );
    let key = service.request_key(&context, &request).unwrap();
    let binding = service.derive_helper_binding(&context, &key).unwrap();
    let owner = Digest([73; 32]);
    let cfg = Digest([74; 32]);
    let scope = ItemHelperScope::new(owner, cfg, vec![binding]).unwrap();
    let catalog = service.export_catalog(&context).unwrap();
    let find = |name: &str| {
        catalog.exports().find(|e|matches!(e.identity(),lrr::expand::identity::MacroIdentity::Procedural{export,..} if export==name)).unwrap().definition().clone()
    };
    let attribute = find("make_tests");
    let function = find("identity");
    let helper_path = vec![MacroName::parse("member_helper", 2024).unwrap()];
    let active = vec![key.clone()];
    let incomplete = ItemHelperScope::new(owner, cfg, vec![]).unwrap();
    assert!(
        matches!(classify_custom_attribute(&incomplete,owner,cfg,&active,&helper_path,PathLookup::Defined(attribute.clone()),&catalog).unwrap(),CustomAttributeDecision::NeedHelpers(keys) if keys==active)
    );
    assert!(
        matches!(classify_custom_attribute(&scope,owner,cfg,&active,&helper_path,PathLookup::NeedInput(Digest([75;32])),&catalog).unwrap(),CustomAttributeDecision::Helpers(keys) if keys==active)
    );
    let other_path = vec![MacroName::parse("another", 2024).unwrap()];
    let unknown_definition = lrr::expand::scopes::MacroDefinition {
        identity: Digest([76; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    assert!(
        matches!(classify_custom_attribute(&scope,owner,cfg,&active,&other_path,PathLookup::Defined(unknown_definition.clone()),&catalog).unwrap(),CustomAttributeDecision::NeedDefinition(d) if d==unknown_definition)
    );
    assert!(matches!(
        classify_custom_attribute(
            &scope,
            owner,
            cfg,
            &active,
            &other_path,
            PathLookup::NeedInput(Digest([75; 32])),
            &catalog
        )
        .unwrap(),
        CustomAttributeDecision::NeedNamespace(_)
    ));
    assert!(
        classify_custom_attribute(
            &scope,
            Digest([255; 32]),
            cfg,
            &active,
            &helper_path,
            PathLookup::Absent,
            &catalog
        )
        .is_err()
    );
    let mut wrong = key.clone();
    wrong.macro_identity = lrr::expand::identity::MacroIdentity::Procedural {
        artifact: Digest(request.artifact.0),
        export: "identity".into(),
        kind: ProceduralKind::Function,
    };
    assert!(
        scope
            .lookup_helpers(owner, cfg, &[wrong], "member_helper")
            .is_err()
    );
    let mut rows = vec![];
    for (name, source) in [
        (
            "helper",
            "use lrr_fixture_macros::MemberCfg; #[derive(MemberCfg)] #[member_helper] struct Marker;",
        ),
        (
            "attribute_collision",
            "use lrr_fixture_macros::{MemberCfg,make_tests as member_helper}; #[derive(MemberCfg)] #[member_helper] struct Marker;",
        ),
        (
            "function_collision",
            "use lrr_fixture_macros::{MemberCfg,identity as member_helper}; #[derive(MemberCfg)] #[member_helper] struct Marker;",
        ),
        (
            "qualified",
            "use lrr_fixture_macros::MemberCfg; #[derive(MemberCfg)] #[lrr_fixture_macros::make_tests] struct Marker;",
        ),
        (
            "no_active_derive",
            "use lrr_fixture_macros::make_tests as member_helper; #[member_helper] struct Marker;",
        ),
        ("absent", "#[member_helper] struct Marker;"),
    ] {
        let (path_names, namespace, active_derives) = match name {
            "helper" => (helper_path.clone(), PathLookup::Absent, active.as_slice()),
            "attribute_collision" => (
                helper_path.clone(),
                PathLookup::Defined(attribute.clone()),
                active.as_slice(),
            ),
            "function_collision" => (
                helper_path.clone(),
                PathLookup::Defined(function.clone()),
                active.as_slice(),
            ),
            "qualified" => (
                vec![
                    MacroName::parse("lrr_fixture_macros", 2024).unwrap(),
                    MacroName::parse("make_tests", 2024).unwrap(),
                ],
                PathLookup::Defined(attribute.clone()),
                active.as_slice(),
            ),
            "no_active_derive" => (
                helper_path.clone(),
                PathLookup::Defined(attribute.clone()),
                [].as_slice(),
            ),
            "absent" => (helper_path.clone(), PathLookup::Absent, [].as_slice()),
            _ => unreachable!(),
        };
        let decision = classify_custom_attribute(
            &scope,
            owner,
            cfg,
            active_derives,
            &path_names,
            namespace,
            &catalog,
        );
        let classification = match &decision {
            Ok(CustomAttributeDecision::Helpers(keys)) => {
                assert_eq!(keys, &active);
                "helpers"
            }
            Ok(CustomAttributeDecision::Procedural(identity)) => {
                assert_eq!(
                    identity,
                    &lrr::expand::identity::MacroIdentity::Procedural {
                        artifact: Digest(request.artifact.0),
                        export: "make_tests".into(),
                        kind: ProceduralKind::Attribute
                    }
                );
                "attribute"
            }
            Err(_) if name == "absent" => "absent",
            _ => panic!("unexpected custom attribute classification"),
        };
        let path = root.join(format!("{name}.rs"));
        let source = if classification == "helpers" {
            format!(
                "{source} #[test] fn retained(){{let _=Marker;assert!(!MEMBER_NAMES.is_empty());}}"
            )
        } else {
            source.into()
        };
        fs::write(&path, source).unwrap();
        let output = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--test", "--extern"])
            .arg(format!("lrr_fixture_macros={}", artifact.path))
            .arg(&path)
            .arg("-o")
            .arg(root.join(format!("{name}-native")))
            .output()
            .unwrap();
        fs::write(root.join(format!("{name}.stderr")), &output.stderr).unwrap();
        assert_eq!(
            output.status.success(),
            decision.is_ok(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut native_list = String::new();
        if output.status.success() {
            let executable = root.join(format!("{name}-native"));
            let listing = Command::new(&executable).arg("--list").output().unwrap();
            assert!(listing.status.success());
            native_list = String::from_utf8(listing.stdout).unwrap();
            assert_eq!(
                native_list.contains("retained: test"),
                classification == "helpers"
            );
            let run = Command::new(executable).output().unwrap();
            assert!(
                run.status.success(),
                "{}",
                String::from_utf8_lossy(&run.stdout)
            );
        } else {
            assert!(String::from_utf8_lossy(&output.stderr).contains("cannot find attribute"));
        }
        assert_eq!(service.invocations(), 0);
        let row = serde_json::json!({"case":name,"classification":classification,"native_list":native_list,"success":output.status.success(),"exit_code":output.status.code(),"stderr":String::from_utf8_lossy(&output.stderr),"session_invocations":service.invocations()});
        rows.push(row);
    }
    fs::write(
        root.join("evidence.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
}

#[test]
fn actual_builtin_derive_helpers_require_exact_identity_and_match_native_default_scope() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        expand::{
            derive::{DeriveHelperBinding, ItemHelperScope},
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        },
        service::{
            expansion::DemandContext,
            macro_session::{MacroSession, macro_artifact_identity},
        },
        source::snapshot::SourceBlob,
    };
    let prepared = prepared();
    let root = prepared.work.join("builtin-derive-helpers");
    let manifest = service_fixture::prepare(&root, &prepared.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let (producer, artifact) = manifest.artifacts.iter().next().unwrap();
    let context = DemandContext {
        request: Digest([81; 32]),
        session: Digest([82; 32]),
        unit,
        revision: 0,
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut service = MacroSession::open(
        context.clone(),
        &manifest,
        true,
        PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]),
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .unwrap();
    let source_path = PathBuf::from(&manifest.graph.units[producer].source_path);
    let blob = SourceBlob::new(fs::read(&source_path).unwrap()).unwrap();
    assert!(matches!(
        service
            .register_source_derive_catalog(&context, &manifest, *producer, &source_path, &blob)
            .unwrap(),
        lrr::expand::derive_declaration::SourceCatalogAdmission::Ready(_)
    ));
    let owner = Digest([83; 32]);
    let cfg = Digest([84; 32]);
    let mut bindings = vec![];
    let mut builtins = vec![];
    for name in [
        "Copy",
        "Clone",
        "Default",
        "Debug",
        "Hash",
        "Ord",
        "PartialOrd",
        "Eq",
        "PartialEq",
    ] {
        let descriptor = lrr_expand_adapter::builtin_derives::descriptor(name)
            .unwrap()
            .unwrap();
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Builtin {
                definition: Digest(descriptor.definition_identity().0),
            },
            invocation: Digest::of_bytes(name.as_bytes()),
            input_tokens: Digest([85; 32]),
            consumer_compile_env: context.compile_environment,
            parent_context: Digest([86; 32]),
            definition_edition: 2024,
            call_edition: 2024,
            grammar: GrammarSlot::Items,
            engine_version: Digest(descriptor.engine_identity().0),
            procedural_epoch: None,
        };
        bindings.push(DeriveHelperBinding::builtin(key.clone(), &descriptor).unwrap());
        builtins.push((name, key));
    }
    let default = builtins
        .iter()
        .find(|(n, _)| *n == "Default")
        .unwrap()
        .1
        .clone();
    let debug = builtins
        .iter()
        .find(|(n, _)| *n == "Debug")
        .unwrap()
        .1
        .clone();
    let descriptor = lrr_expand_adapter::builtin_derives::descriptor("Default")
        .unwrap()
        .unwrap();
    let mut wrong = default.clone();
    wrong.macro_identity = MacroIdentity::Builtin {
        definition: Digest([255; 32]),
    };
    assert!(DeriveHelperBinding::builtin(wrong, &descriptor).is_err());
    let mut wrong = default.clone();
    wrong.engine_version = Digest([255; 32]);
    assert!(DeriveHelperBinding::builtin(wrong, &descriptor).is_err());
    assert!(DeriveHelperBinding::builtin(debug.clone(), &descriptor).is_err());
    let mut config = prepared.config.clone();
    config.unit = Key(unit.0);
    config.epoch = context.epoch;
    config.artifacts[0].identity = Key(macro_artifact_identity(*producer, artifact).0);
    let request = invocation(
        &config,
        98,
        "Empty",
        ProcForm::Derive,
        ProcGrammar::Items,
        "enum Value{Selected}",
    );
    let procedural = service.request_key(&context, &request).unwrap();
    assert!(DeriveHelperBinding::builtin(procedural.clone(), &descriptor).is_err());
    bindings.push(
        service
            .derive_helper_binding(&context, &procedural)
            .unwrap(),
    );
    let scope = ItemHelperScope::new(owner, cfg, bindings).unwrap();
    let catalog = service.export_catalog(&context).unwrap();
    let all_active: Vec<_> = builtins.iter().map(|(_, key)| key.clone()).collect();
    assert_eq!(
        scope
            .helper_owners(owner, cfg, &all_active, "default")
            .unwrap(),
        std::slice::from_ref(&default)
    );
    assert!(
        scope
            .helper_owners(owner, cfg, &[debug.clone(), procedural.clone()], "default")
            .unwrap()
            .is_empty()
    );
    let mut rows = vec![];
    for (case, source, active) in [
        (
            "builtin",
            "#[derive(Default,Copy,Clone,Debug,Hash,Ord,PartialOrd,Eq,PartialEq)] enum Value{#[default] Selected,Other} #[test]fn observed(){assert!(matches!(Value::default(),Value::Selected));}",
            all_active,
        ),
        (
            "genuine_alias",
            "use core::default::Default as Genuine; #[derive(Genuine)] enum Value{#[default] Selected} #[test]fn observed(){assert!(matches!(Value::default(),Value::Selected));}",
            vec![default.clone()],
        ),
        (
            "no_default",
            "#[derive(Debug)] enum Value{#[default] Selected}",
            vec![debug.clone()],
        ),
        (
            "inactive_default",
            "#[cfg_attr(any(),derive(Default))] #[derive(Debug)] enum Value{#[default] Selected}",
            vec![debug],
        ),
        (
            "procedural_alias",
            "use lrr_fixture_macros::Empty as Default; #[derive(Default)] enum Value{#[default] Selected}",
            vec![procedural],
        ),
    ] {
        let owners = scope.helper_owners(owner, cfg, &active, "default").unwrap();
        let decision = lrr::frontend::item_attributes::classify_custom_attribute(
            &scope,
            owner,
            cfg,
            &active,
            &[lrr::expand::scopes::MacroName::parse("r#default", 2024).unwrap()],
            lrr::frontend::macro_names::PathLookup::Absent,
            &catalog,
        );
        if !owners.is_empty() {
            assert!(
                matches!(&decision,Ok(lrr::frontend::item_attributes::CustomAttributeDecision::Helpers(keys)) if keys==&owners)
            );
        } else {
            assert!(decision.is_err());
        }
        let path = root.join(format!("{case}.rs"));
        fs::write(&path, source).unwrap();
        let executable = root.join(format!("{case}-native"));
        let compiled = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--test", "--extern"])
            .arg(format!("lrr_fixture_macros={}", artifact.path))
            .arg(&path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        fs::write(root.join(format!("{case}.stderr")), &compiled.stderr).unwrap();
        assert_eq!(
            compiled.status.success(),
            decision.is_ok(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let mut native_stdout = String::new();
        if compiled.status.success() {
            let run = Command::new(executable).output().unwrap();
            assert!(
                run.status.success(),
                "{}",
                String::from_utf8_lossy(&run.stdout)
            );
            native_stdout = String::from_utf8(run.stdout).unwrap();
        } else {
            assert!(
                String::from_utf8_lossy(&compiled.stderr)
                    .contains("cannot find attribute `default`")
            );
        }
        assert_eq!(service.invocations(), 0);
        rows.push(serde_json::json!({"case":case,"native_success":compiled.status.success(),"helper_owners":owners.iter().map(|k|k.invocation).collect::<Vec<_>>(),"native_stdout":native_stdout,"session_invocations":service.invocations()}));
    }
    fs::write(root.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"builtin/procedural helper metadata and native scope acceptance; no builtin expansion or guest execution claim","rows":rows})).unwrap()).unwrap();
}

#[test]
fn actual_builtin_attribute_aliases_classify_by_identity_and_prepare_resolved_derives() {
    use lrr::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        expand::{derive::ItemHelperScope, scopes::MacroName},
        frontend::{
            builtin_attributes::{AttributeDecision, BuiltinAttributeCatalog},
            hygiene::HygieneFacts,
            item_attributes::CustomAttributeDecision,
            macro_names::PathLookup,
            source_macro_routes::SourceMacroRoutes,
        },
        service::{
            expansion::DemandContext,
            item_macro_inputs::{self, ResolvedDeriveAnnotations},
            macro_session::MacroSession,
        },
        source::snapshot::{CfgSet, SourceBlob},
        syntax::{outline::ScopeKey, parse::ParsedFile, spans::SourceRange, token_wire as runtime},
    };
    let prepared = prepared();
    let root = prepared.work.join("builtin-attribute-aliases");
    let manifest = service_fixture::prepare(&root, &prepared.config.directory);
    let consumer = *manifest.graph.roots.first().unwrap();
    let (producer, artifact) = manifest.artifacts.iter().next().unwrap();
    let context = DemandContext {
        request: Digest([91; 32]),
        session: Digest([92; 32]),
        unit: consumer,
        revision: 0,
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&consumer].fingerprint(),
    };
    let service = MacroSession::open(
        context.clone(),
        &manifest,
        true,
        PathBuf::from(&manifest.compile_env[&consumer].cargo["CARGO_MANIFEST_DIR"]),
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .unwrap();
    let procedural = service.export_catalog(&context).unwrap();
    let derive = lrr::expand::builtin_attributes::descriptor("derive")
        .unwrap()
        .unwrap();
    let test = lrr::expand::builtin_attributes::descriptor("test")
        .unwrap()
        .unwrap();
    assert_ne!(derive.definition_identity(), test.definition_identity());
    assert!(
        lrr::expand::builtin_attributes::descriptor("user_attribute")
            .unwrap()
            .is_none()
    );
    assert!(lrr::expand::builtin_attributes::descriptor("derive_const").is_err());
    let core = Digest::of_bytes(b"controlled compiler-owned core facet");
    let builtin = BuiltinAttributeCatalog::new(core, 2024, [derive.clone(), test.clone()]).unwrap();
    let prefix: Vec<_> = ["prelude", "v1"]
        .iter()
        .map(|n| MacroName::parse(n, 2024).unwrap())
        .collect();
    for definition in builtin.definitions() {
        let mut forged = definition.clone();
        forged.defining_unit = consumer;
        assert!(builtin.select(&forged).is_err());
    }
    let cfg = CfgSet::default();
    let mut rows = vec![];
    for (case, source) in [
        (
            "derive_alias",
            "use core::prelude::v1::derive as generate; #[generate(Debug)] struct Original; #[test]fn retained(){let _=Original;}",
        ),
        (
            "test_alias",
            "use core::prelude::v1::test as check; #[check]fn retained(){}",
        ),
        (
            "qualified",
            "#[core::prelude::v1::derive(Debug)] struct Original; #[core::prelude::v1::test]fn retained(){let _=Original;}",
        ),
        (
            "user_derive",
            "use lrr_fixture_macros::make_tests as derive; #[derive(Debug)] struct Original;",
        ),
        (
            "user_test",
            "use lrr_fixture_macros::make_tests as test; #[test]fn original(){}",
        ),
        (
            "replacement",
            "use lrr_fixture_macros::make_tests as replace; #[replace] #[unresolved_later_attribute] struct Original;",
        ),
    ] {
        let extended =
            format!("{source} #[cfg(any())] #[unrecognized_attribute] fn disabled(){{missing();}}");
        let source = extended.as_str();
        let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed.outline(consumer, vec![], &cfg).unwrap();
        let mut hygiene = lrr_expand_adapter::hygiene::HygieneStore::default();
        let ctx = hygiene.root(Key(consumer.0), Edition::E2024).unwrap();
        let resolutions = hygiene.resolution_contexts(&[ctx]).unwrap();
        let names = hygiene.name_contexts(&[ctx]).unwrap();
        let facts = || {
            HygieneFacts::new(
                runtime::TokenForest::decode(
                    &TokenForest::new(vec![], hygiene.records(), vec![])
                        .unwrap()
                        .encode()
                        .unwrap(),
                )
                .unwrap()
                .contexts()
                .to_vec(),
                serde_json::from_slice(&serde_json::to_vec(&resolutions).unwrap()).unwrap(),
            )
            .unwrap()
            .with_name_contexts(
                serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap(),
            )
            .unwrap()
        };
        let source_index = lrr::frontend::source_macros::SourceMacroIndex::from_outline(
            &outline,
            runtime::Key(ctx.0),
            facts(),
        )
        .unwrap();
        let mut routes = builtin.source_routes(&prefix).unwrap();
        routes.extend(SourceMacroRoutes::from_procedural_catalog(&procedural).unwrap());
        routes.push(
            SourceMacroRoutes::from_outline(
                &outline,
                runtime::Key(ctx.0),
                facts(),
                &BTreeMap::from([
                    (MacroName::parse("core", 2024).unwrap(), core),
                    (
                        MacroName::parse("lrr_fixture_macros", 2024).unwrap(),
                        *producer,
                    ),
                ]),
            )
            .unwrap(),
        );
        let graph = SourceMacroRoutes::candidate_graph(&routes).unwrap();
        let selected = outline
            .items
            .iter()
            .find(|item| !item.header.attributes.is_empty())
            .unwrap()
            .declaration_span;
        let attributes = parsed.attribute_inputs_at(selected, &cfg).unwrap();
        let attribute_path = parsed
            .attribute_path_at(attributes.attributes[0].range)
            .unwrap();
        assert!(!attribute_path.requires_body_entry());
        let caller = ScopeKey {
            crate_key: consumer,
            module_path: vec![],
            owner: None,
        };
        let namespace = source_index
            .lookup_attribute_candidate(&attribute_path, &graph, &caller)
            .unwrap();
        let mut wrong = caller.clone();
        wrong.crate_key = core;
        assert!(
            source_index
                .lookup_attribute_candidate(&attribute_path, &graph, &wrong)
                .is_err()
        );
        let mut wrong = caller.clone();
        wrong.module_path.push("unrelated".into());
        assert!(
            source_index
                .lookup_attribute_candidate(&attribute_path, &graph, &wrong)
                .is_err()
        );
        let other = ParsedFile::parse(
            &SourceBlob::new(b"#[check] fn other(){}".to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let other_path = other
            .attribute_path_at(SourceRange { start: 2, end: 7 })
            .unwrap();
        assert!(
            source_index
                .lookup_attribute_candidate(&other_path, &graph, &caller)
                .is_err()
        );
        assert!(matches!(namespace, PathLookup::Defined(_)));
        let owner = Digest::of_bytes(case.as_bytes());
        let helpers = ItemHelperScope::new(owner, cfg.fingerprint(), vec![]).unwrap();
        let resolver = lrr::frontend::original_attributes::OriginalAttributeResolver {
            source: &source_index,
            namespaces: &graph,
            procedural: &procedural,
            builtin: &builtin,
            helpers: &helpers,
            owner,
            cfg: cfg.fingerprint(),
            active_derives: &[],
        };
        let lrr::frontend::original_attributes::OriginalAttributeResolution::Candidate(decision) =
            resolver.resolve(&attribute_path, &caller).unwrap()
        else {
            panic!("active indexed attribute became inactive")
        };
        use lrr::frontend::original_attributes::{
            OriginalAttributePoll, OriginalItemAttributeCursor,
        };
        let mut cursor =
            OriginalItemAttributeCursor::new(&resolver, &parsed, caller.clone(), selected)
                .unwrap()
                .unwrap();
        assert!(matches!(
            cursor.poll(&resolver, &parsed).unwrap(),
            OriginalAttributePoll::Candidate { index: 0, .. }
        ));
        assert!(
            cursor
                .advance_nontransforming_candidate(&resolver, &parsed, 1)
                .is_err()
        );
        assert!(cursor.poll(&resolver, &other).is_err());
        if matches!(&decision, AttributeDecision::Builtin(d) if d.action() == lrr::expand::builtin_attributes::BuiltinAttributeAction::Derive)
        {
            cursor
                .advance_nontransforming_candidate(&resolver, &parsed, 0)
                .unwrap();
            assert!(matches!(
                cursor.poll(&resolver, &parsed).unwrap(),
                OriginalAttributePoll::Complete
            ));
            assert!(
                cursor
                    .advance_nontransforming_candidate(&resolver, &parsed, 0)
                    .is_err()
            );
        } else {
            assert!(
                cursor
                    .advance_nontransforming_candidate(&resolver, &parsed, 0)
                    .is_err()
            );
            assert!(matches!(
                cursor.poll(&resolver, &parsed).unwrap(),
                OriginalAttributePoll::Candidate { index: 0, .. }
            ));
        }
        let start = source.find("unrecognized_attribute").unwrap();
        let inactive = parsed
            .attribute_path_at(SourceRange {
                start: u32::try_from(start).unwrap(),
                end: u32::try_from(start + "unrecognized_attribute".len()).unwrap(),
            })
            .unwrap();
        assert!(matches!(
            resolver.resolve(&inactive, &caller).unwrap(),
            lrr::frontend::original_attributes::OriginalAttributeResolution::Inactive
        ));
        let mismatch = lrr::frontend::original_attributes::OriginalAttributeResolver {
            cfg: Digest([255; 32]),
            ..resolver
        };
        assert!(mismatch.resolve(&attribute_path, &caller).is_err());
        let mut prepared_paths = vec![];
        let classification = match decision {
            AttributeDecision::Builtin(descriptor) => {
                if case == "test_alias" {
                    assert_eq!(
                        descriptor.action(),
                        lrr::expand::builtin_attributes::BuiltinAttributeAction::Test
                    );
                    "test"
                } else {
                    assert_eq!(
                        descriptor.action(),
                        lrr::expand::builtin_attributes::BuiltinAttributeAction::Derive
                    );
                    let start = if case == "derive_alias" {
                        source.find("#[generate").unwrap()
                    } else {
                        source.find("#[core::prelude::v1::derive").unwrap()
                    };
                    let end = source.find("struct Original;").unwrap() + "struct Original;".len();
                    let inputs = parsed
                        .derive_item_inputs_at(
                            SourceRange {
                                start: u32::try_from(start).unwrap(),
                                end: u32::try_from(end).unwrap(),
                            },
                            &cfg,
                        )
                        .unwrap();
                    let original = token_wire::parse_source(
                        source,
                        SourceIdentity {
                            unit: Key(consumer.0),
                            content: Key(blob.content().0),
                            path: format!("{case}.rs"),
                        },
                        ContextRecord {
                            parent: None,
                            expansion: None,
                            transparency: Transparency::Opaque,
                            defining_unit: Key(consumer.0),
                            edition: Edition::E2024,
                        },
                        Key([93; 32]),
                    )
                    .unwrap();
                    let forest = runtime::TokenForest::decode(&original.encode().unwrap()).unwrap();
                    let identity = forest.spans()[0].source.clone();
                    let annotations =
                        ResolvedDeriveAnnotations::new(owner, &inputs, [(0, descriptor.clone())])
                            .unwrap();
                    assert!(
                        ResolvedDeriveAnnotations::new(owner, &inputs, [(0, test.clone())])
                            .is_err()
                    );
                    assert!(
                        ResolvedDeriveAnnotations::new(
                            owner,
                            &inputs,
                            [(0, descriptor.clone()), (0, descriptor)]
                        )
                        .is_err()
                    );
                    assert!(
                        item_macro_inputs::derive_resolved(
                            Digest([255; 32]),
                            &inputs,
                            0,
                            &annotations,
                            &identity,
                            &forest
                        )
                        .is_err()
                    );
                    assert!(
                        item_macro_inputs::derive_resolved(
                            owner,
                            &inputs,
                            1,
                            &annotations,
                            &identity,
                            &forest
                        )
                        .is_err()
                    );
                    let invocation = item_macro_inputs::derive_resolved(
                        owner,
                        &inputs,
                        0,
                        &annotations,
                        &identity,
                        &forest,
                    )
                    .unwrap();
                    prepared_paths = invocation
                        .selections
                        .iter()
                        .map(|s| s.segments.clone())
                        .collect();
                    assert_eq!(prepared_paths, [vec!["Debug".to_owned()]]);
                    let item = TokenForest::decode(&invocation.item.encode().unwrap()).unwrap();
                    assert!(identifiers(&item).contains(&"Original".into()));
                    assert!(!identifiers(&item).contains(&"generate".into()));
                    assert!(!identifiers(&item).contains(&"derive".into()));
                    "derive"
                }
            }
            AttributeDecision::Custom(CustomAttributeDecision::Procedural(identity)) => {
                assert!(
                    matches!(identity,lrr::expand::identity::MacroIdentity::Procedural{ref export,..} if export=="make_tests")
                );
                assert!(case.starts_with("user_") || case == "replacement");
                "procedural"
            }
            _ => panic!("attribute identity did not classify"),
        };
        assert_eq!(service.invocations(), 0);
        let path = root.join(format!("{case}.rs"));
        fs::write(&path, source).unwrap();
        let exe = root.join(format!("{case}-native"));
        let compiled = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--test", "--extern"])
            .arg(format!("lrr_fixture_macros={}", artifact.path))
            .arg(&path)
            .arg("-o")
            .arg(&exe)
            .output()
            .unwrap();
        fs::write(root.join(format!("{case}.stderr")), &compiled.stderr).unwrap();
        let mut listing = String::new();
        if compiled.status.success() {
            let list = Command::new(exe).arg("--list").output().unwrap();
            assert!(list.status.success());
            listing = String::from_utf8(list.stdout).unwrap();
        }
        if case == "user_test" {
            assert!(!compiled.status.success());
            assert!(String::from_utf8_lossy(&compiled.stderr).contains("recursion limit reached"));
        } else {
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            assert_eq!(
                listing.contains("retained: test"),
                classification != "procedural"
            );
            if case == "replacement" {
                assert!(listing.contains("generated_one: test"));
                assert!(listing.contains("generated_two: test"));
                assert!(!listing.contains("Original"));
            }
        }
        assert_eq!(service.invocations(), 0);
        let row = serde_json::json!({"case":case,"classification":classification,"prepared_paths":prepared_paths,"success":compiled.status.success(),"stderr":String::from_utf8_lossy(&compiled.stderr),"listing":listing,"session_invocations":service.invocations()});
        rows.push(row);
    }
    fs::write(
        root.join("evidence.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
}

#[test]
fn actual_service_epochs_reexpand_external_inputs_before_pure_output_reuse() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        db::{key::QueryKey, memo::Database, poll::InputRequest},
        expand::cache::ExpansionOutputCache,
        service::{
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
            macro_session::{MacroSession, macro_artifact_identity},
        },
        source::manifest::BodyKey,
    };
    let prepared = prepared();
    let evidence = prepared.work.join("epoch-revalidation");
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let input_plan = recorded_macro_plan(&evidence, &prepared.config.directory);
    let recorded = lrr::service::run::RecordedMacroInputs::begin(&manifest, &input_plan).unwrap();
    let unit = *manifest.graph.roots.first().unwrap();
    let (tool, artifact) = manifest.artifacts.iter().next().unwrap();
    let artifact_bytes = fs::read(&artifact.path).unwrap();
    let external = evidence.join("external.txt");
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    assert!(ExpansionOutputCache::new(0).is_err());
    assert!(ExpansionOutputCache::new(256 * 1024 * 1024 + 1).is_err());
    let mut previous = None;
    let mut observations = vec![];
    for (epoch, value, expected_hit) in [(1, 11, false), (2, 29, false), (3, 29, true)] {
        fs::write(&external, format!("{value}_u64")).unwrap();
        let mut config = prepared.config.clone();
        config.unit = Key(unit.0);
        config.epoch = epoch;
        config.artifacts[0].identity = Key(macro_artifact_identity(*tool, artifact).0);
        let request = invocation(
            &config,
            201,
            "read_external",
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            &format!("{:?}", external.to_str().unwrap()),
        );
        let mut db = Database::default();
        db.publish_inputs(BTreeMap::from([(
            lrr::db::dependency::InputKey::ExpansionEpoch,
            lrr::db::dependency::InputValue::ExpansionEpoch(epoch),
        )]))
        .unwrap();
        let context = DemandContext {
            request: Digest::of_bytes(&epoch.to_le_bytes()),
            session: Digest::of_bytes(&[epoch as u8]),
            unit,
            revision: db.source_revision(),
            epoch,
            source_generation: manifest.inputs.fingerprint(),
            native_generation: None,
            compile_environment: manifest.compile_env[&unit].fingerprint(),
        };
        let mut service = MacroSession::open(
            context.clone(),
            &manifest,
            true,
            PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]),
            Duration::from_secs(10),
            4 * 1024 * 1024,
        )
        .unwrap();
        let key = service.request_key(&context, &request).unwrap();
        let mut coordinator = ExpansionCoordinator::new(context.clone());
        let token = coordinator
            .issue(
                QueryKey::CurrentExpansionInput(Box::new(key.clone())),
                PhaseIdentity {
                    frame: Digest([202; 32]),
                    body: BodyKey {
                        definition: Digest([203; 32]),
                        exact_body_bytes: Digest([204; 32]),
                    },
                    index: 0,
                },
                InputRequest::Expand(Box::new(lrr::db::poll::ExpansionRequest { key })),
            )
            .unwrap();
        let (record, output) = service
            .fulfil_verified(
                &context,
                &mut coordinator,
                &token,
                &request,
                &mut cache,
                &recorded,
            )
            .unwrap();
        coordinator
            .adopt(&context, &token, &mut db, record.clone())
            .unwrap();
        assert!(matches!(
            db.fetch(
                &lrr::db::provider::SyntaxProvider,
                QueryKey::CurrentExpansionInput(Box::new(record.key().clone()))
            ),
            lrr::db::poll::QueryPoll::Ready(Ok(_))
        ));
        let owned = token_wire::TokenForest::decode(&output.tokens().encode().unwrap()).unwrap();
        assert_eq!(literals(&owned), [format!("{value}_u64")]);
        assert_eq!(output.reused(), expected_hit);
        assert!(output.is_current(&context));
        if let Some((old_context, old_semantic)) = previous.take() {
            assert!(!output.is_current(&old_context));
            assert_eq!(output.semantic() == old_semantic, expected_hit);
        }
        let (_, repeated) = service
            .fulfil_verified(
                &context,
                &mut coordinator,
                &token,
                &request,
                &mut cache,
                &recorded,
            )
            .unwrap();
        assert!(Arc::ptr_eq(output.tokens(), repeated.tokens()));
        assert_eq!(service.invocations(), 1);
        let mut stale = context.clone();
        stale.epoch += 1;
        assert!(
            service
                .fulfil_verified(
                    &stale,
                    &mut coordinator,
                    &token,
                    &request,
                    &mut cache,
                    &recorded
                )
                .is_err()
        );
        let mut too_small = ExpansionOutputCache::new(1).unwrap();
        assert!(
            service
                .fulfil_verified(
                    &context,
                    &mut coordinator,
                    &token,
                    &request,
                    &mut too_small,
                    &recorded,
                )
                .is_err()
        );
        assert_eq!(too_small.retained_bytes(), 0);
        assert_eq!(service.invocations(), 1);
        assert_eq!(
            record.output(),
            lrr::core::digest::Digest::of_bytes(&output.tokens().encode().unwrap())
        );
        assert_eq!(fs::read(&artifact.path).unwrap(), artifact_bytes);
        observations.push(serde_json::json!({"epoch":epoch,"value":value,"pure_output_reused":output.reused(),"invocations":service.invocations(),"semantic":output.semantic().hex(),"producer":output.producer().hex()}));
        previous = Some((context, output.semantic()));
    }
    fs::write(evidence.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","macro_inputs":"non_hermetic","observations":observations,"artifact_unchanged":true,"scope":"actual service epoch re-expansion and pure-output revalidation; no guest/generated seal acceptance"})).unwrap()).unwrap();
}

fn recorded_macro_plan(
    evidence: &std::path::Path,
    macros: &std::path::Path,
) -> lrr::project::tool_manifest::InputPlan {
    use lrr::project::tool_manifest::{InputKind, InputPlan, InputRoot};
    InputPlan::new(
        [evidence.join("app"), macros.to_owned()]
            .into_iter()
            .map(|root| {
                InputRoot::open(
                    InputKind::Source,
                    &root,
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
    .unwrap()
}

#[test]
fn actual_macro_recorded_input_mutation_closes_session_and_prevents_output_adoption() {
    recorded_input_mutation_fixture(false);
}
#[test]
fn actual_recorded_input_change_before_macro_execution_denies_native_invocation() {
    recorded_input_mutation_fixture(true);
}
fn recorded_input_mutation_fixture(before: bool) {
    use lrr::{
        core::digest::{Digest, StableEncode},
        db::{
            key::QueryKey,
            memo::Database,
            poll::{ExpansionRequest, InputRequest},
        },
        expand::cache::ExpansionOutputCache,
        service::{
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
            macro_session::{MacroSession, macro_artifact_identity},
            run::RecordedMacroInputs,
        },
        source::manifest::BodyKey,
    };
    let prepared = prepared();
    let evidence = prepared.work.join(if before {
        "recorded-input-before"
    } else {
        "recorded-input-mutation"
    });
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let plan = recorded_macro_plan(&evidence, &prepared.config.directory);
    let witness = RecordedMacroInputs::begin(&manifest, &plan).unwrap();
    let unit = *manifest.graph.roots.first().unwrap();
    let (tool, artifact) = manifest.artifacts.iter().next().unwrap();
    let mut config = prepared.config.clone();
    config.unit = Key(unit.0);
    config.artifacts[0].identity = Key(macro_artifact_identity(*tool, artifact).0);
    let changed = evidence.join("app/src/lib.rs");
    let request = invocation(
        &config,
        211,
        "mutate_recorded_input",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        &format!("{:?}", changed.to_str().unwrap()),
    );
    let mut db = Database::default();
    db.publish_inputs(BTreeMap::from([(
        lrr::db::dependency::InputKey::ExpansionEpoch,
        lrr::db::dependency::InputValue::ExpansionEpoch(config.epoch),
    )]))
    .unwrap();
    let context = DemandContext {
        request: Digest([212; 32]),
        session: Digest([213; 32]),
        unit,
        revision: db.source_revision(),
        epoch: config.epoch,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut service = MacroSession::open(
        context.clone(),
        &manifest,
        true,
        PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]),
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .unwrap();
    let key = service.request_key(&context, &request).unwrap();
    let query = QueryKey::CurrentExpansionInput(Box::new(key.clone()));
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let token = coordinator
        .issue(
            query.clone(),
            PhaseIdentity {
                frame: Digest([214; 32]),
                body: BodyKey {
                    definition: Digest([215; 32]),
                    exact_body_bytes: Digest([216; 32]),
                },
                index: 0,
            },
            InputRequest::Expand(Box::new(ExpansionRequest { key })),
        )
        .unwrap();
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    if before {
        fs::write(&changed, "changed_by_macro").unwrap();
    }
    let error = service
        .fulfil_verified(
            &context,
            &mut coordinator,
            &token,
            &request,
            &mut cache,
            &witness,
        )
        .err()
        .unwrap();
    assert_eq!(error.code, "LRR-E-TOOL-PREPARE-STALE");
    assert_eq!(fs::read_to_string(&changed).unwrap(), "changed_by_macro");
    assert_eq!(service.invocations(), if before { 0 } else { 1 });
    assert_eq!(cache.retained_bytes(), 0);
    assert!(coordinator.ready(&context, &token).is_err());
    assert!(
        service
            .fulfil_verified(
                &context,
                &mut coordinator,
                &token,
                &request,
                &mut cache,
                &witness
            )
            .is_err()
    );
    assert_eq!(service.invocations(), if before { 0 } else { 1 });
    assert!(matches!(
        db.fetch(&lrr::db::provider::SyntaxProvider, query),
        lrr::db::poll::QueryPoll::NeedInput(_)
    ));
    assert!(RecordedMacroInputs::begin(&manifest, &plan).is_err());
    fs::write(evidence.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","error":error.code,"invocations":service.invocations(),"cache_bytes":cache.retained_bytes(),"publication_prevented":true,"mutation_before_invocation":before,"scope":"actual recorded source mutation during external macro execution; session/coordinator close with no adoption/retry"})).unwrap()).unwrap();
}

#[test]
fn actual_macro_run_owner_shares_counter_and_closes_at_seal_with_fresh_next_epoch() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        db::{
            dependency::{InputKey, InputValue},
            memo::Database,
        },
        expand::cache::ExpansionOutputCache,
        service::{
            expansion::{DemandContext, PhaseIdentity},
            macro_session::macro_artifact_identity,
            run::MacroRun,
        },
        source::manifest::BodyKey,
    };
    let prepared = prepared();
    let evidence = prepared.work.join("run-owner");
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let plan = recorded_macro_plan(&evidence, &prepared.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let (tool, artifact) = manifest.artifacts.iter().next().unwrap();
    let phase = |index| PhaseIdentity {
        frame: Digest([221; 32]),
        body: BodyKey {
            definition: Digest([222; 32]),
            exact_body_bytes: Digest([223; 32]),
        },
        index,
    };
    let mut db = Database::default();
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut observations = vec![];
    for epoch in [1, 2] {
        db.publish_inputs(BTreeMap::from([(
            InputKey::ExpansionEpoch,
            InputValue::ExpansionEpoch(epoch),
        )]))
        .unwrap();
        let context = DemandContext {
            request: Digest::of_bytes(&epoch.to_le_bytes()),
            session: Digest([224; 32]),
            unit,
            revision: db.source_revision(),
            epoch,
            source_generation: manifest.inputs.fingerprint(),
            native_generation: None,
            compile_environment: manifest.compile_env[&unit].fingerprint(),
        };
        let mut config = prepared.config.clone();
        config.unit = Key(unit.0);
        config.epoch = epoch;
        config.artifacts[0].identity = Key(macro_artifact_identity(*tool, artifact).0);
        let request = invocation(
            &config,
            225,
            "session_counter",
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            "",
        );
        let mut run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
        let one = run
            .fulfil(&mut db, unit, phase(0), &request, &mut cache)
            .unwrap();
        let repeated = run
            .fulfil(&mut db, unit, phase(1), &request, &mut cache)
            .unwrap();
        assert!(Arc::ptr_eq(one.tokens(), repeated.tokens()));
        assert_eq!(run.unit_invocations(unit), 1);
        assert_eq!(
            literals(&token_wire::TokenForest::decode(&one.tokens().encode().unwrap()).unwrap()),
            ["1_u64"]
        );
        assert_eq!(one.reused(), epoch == 2);
        let other = invocation(
            &config,
            226,
            "session_counter",
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            "",
        );
        let two = run
            .fulfil(&mut db, unit, phase(2), &other, &mut cache)
            .unwrap();
        assert_eq!(
            literals(&token_wire::TokenForest::decode(&two.tokens().encode().unwrap()).unwrap()),
            ["2_u64"]
        );
        assert_eq!(run.unit_invocations(unit), 2);
        let certificate = run.verify_for_seal(&mut db).unwrap();
        assert_eq!(certificate.unit_count(), 1);
        certificate
            .validate(&mut db, &context, manifest.fingerprint())
            .unwrap();
        let mut other_context = context.clone();
        other_context.epoch += 1;
        assert!(
            certificate
                .validate(&mut db, &other_context, manifest.fingerprint())
                .is_err()
        );
        assert!(
            certificate
                .validate(&mut db, &context, Digest([250; 32]))
                .is_err()
        );

        assert!(
            run.fulfil(&mut db, unit, phase(3), &request, &mut cache)
                .is_err()
        );
        assert_eq!(run.unit_invocations(unit), 2);
        observations.push(serde_json::json!({"epoch":epoch,"invocations":2,"first_output_reused":one.reused(),"verification":certificate.identity().hex()}));
        let mut denied =
            MacroRun::begin(&mut db, context.clone(), &manifest, &plan, false).unwrap();
        let error = denied
            .fulfil(&mut db, unit, phase(0), &request, &mut cache)
            .err()
            .unwrap();
        assert_eq!(error.code, "LRR-E-MACRO-PERMISSION");
        assert_eq!(denied.unit_invocations(unit), 0);
        db.publish_inputs(BTreeMap::from([(
            InputKey::ExpansionEpoch,
            InputValue::ExpansionEpoch(epoch + 1),
        )]))
        .unwrap();
        assert!(
            certificate
                .validate(&mut db, &context, manifest.fingerprint())
                .is_err()
        );
    }
    fs::write(evidence.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","observations":observations,"scope":"actual single-consumer run ownership, shared invocation, fresh epoch and seal/authority closure; multi-unit and catalog seals pending"})).unwrap()).unwrap();
}
#[test]
fn actual_macro_run_shared_budget_refuses_output_before_adoption_and_closes_session() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        db::{
            dependency::{InputKey, InputValue},
            memo::Database,
        },
        expand::{budget::SharedExpansionBudget, cache::ExpansionOutputCache},
        service::{
            expansion::{DemandContext, PhaseIdentity},
            macro_session::macro_artifact_identity,
            run::MacroRun,
        },
        source::manifest::BodyKey,
    };
    let prepared = prepared();
    let evidence = prepared.work.join("shared-expansion-budget");
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let plan = recorded_macro_plan(&evidence, &prepared.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let (tool, artifact) = manifest.artifacts.iter().next().unwrap();
    let mut db = Database::default();
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut limit = 0;
    let phase = |index| PhaseIdentity {
        frame: Digest([229; 32]),
        body: BodyKey {
            definition: Digest([230; 32]),
            exact_body_bytes: Digest([231; 32]),
        },
        index,
    };
    for epoch in [1, 2] {
        db.publish_inputs(BTreeMap::from([(
            InputKey::ExpansionEpoch,
            InputValue::ExpansionEpoch(epoch),
        )]))
        .unwrap();
        let context = DemandContext {
            request: Digest::of_bytes(&epoch.to_le_bytes()),
            session: Digest([232; 32]),
            unit,
            revision: db.source_revision(),
            epoch,
            source_generation: manifest.inputs.fingerprint(),
            native_generation: None,
            compile_environment: manifest.compile_env[&unit].fingerprint(),
        };
        let mut config = prepared.config.clone();
        config.unit = Key(unit.0);
        config.epoch = epoch;
        config.artifacts[0].identity = Key(macro_artifact_identity(*tool, artifact).0);
        let request = invocation(
            &config,
            233,
            "session_counter",
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            "",
        );
        let run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
        if epoch == 1 {
            let mut run = run;
            let output = run
                .fulfil(&mut db, unit, phase(0), &request, &mut cache)
                .unwrap();
            let InputValue::ExpansionRecord(record) = db
                .input(&InputKey::ExpansionRecord(output.producer()))
                .unwrap()
                .as_ref()
                .clone()
            else {
                panic!("actual native record absent")
            };
            assert!(record.token_count() > 0);
            limit = record.token_count() - 1;
            let foreign_epoch = SharedExpansionBudget::new(
                context.request,
                manifest.graph.fingerprint(),
                epoch + 1,
                2_000_000,
            )
            .unwrap();
            assert_eq!(
                foreign_epoch.record_output(&record).unwrap_err().code,
                "LRR-E-EXPANSION-INPUT"
            );
            assert_eq!(foreign_epoch.totals().unwrap().outputs, 0);
            assert_eq!(
                run.expansion_budget().totals().unwrap().tokens,
                record.token_count()
            );
            run.verify_for_seal(&mut db).unwrap();
        } else {
            let budget = SharedExpansionBudget::new(
                context.request,
                manifest.graph.fingerprint(),
                epoch,
                limit,
            )
            .unwrap();
            let mut run = run.with_expansion_budget(budget.clone()).unwrap();
            let revision = db.revision();
            let error = run
                .fulfil(&mut db, unit, phase(0), &request, &mut cache)
                .err()
                .unwrap();
            assert_eq!(error.code, "LRR-E-DISCOVERY-LIMIT");
            assert_eq!(run.unit_invocations(unit), 1);
            assert_eq!(db.revision(), revision);
            assert_eq!(budget.totals().unwrap().outputs, 0);
            assert!(
                run.fulfil(&mut db, unit, phase(1), &request, &mut cache)
                    .is_err()
            );
            assert_eq!(run.unit_invocations(unit), 1);
            assert!(run.verify_for_seal(&mut db).is_err());
            fs::write(evidence.join("accounting.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M013","platform":"aarch64-apple-darwin","rust":"1.99.0","quota_refused":true,"output_adopted":false,"native_invocations":1,"retry_invoked":false,"release_complete":false})).unwrap()).unwrap();
        }
    }
}
#[test]
fn actual_macro_run_cumulative_deadline_terminates_helper_without_retry_or_adoption() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        db::{
            dependency::{InputKey, InputValue},
            memo::Database,
        },
        expand::{budget::SharedExpansionBudget, cache::ExpansionOutputCache},
        service::{
            expansion::{DemandContext, PhaseIdentity},
            macro_session::macro_artifact_identity,
            run::MacroRun,
        },
        source::manifest::BodyKey,
    };
    let prepared = prepared();
    let evidence = prepared.work.join("cumulative-deadline");
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let plan = recorded_macro_plan(&evidence, &prepared.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let (tool, artifact) = manifest.artifacts.iter().next().unwrap();
    let mut db = Database::default();
    db.publish_inputs(BTreeMap::from([(
        InputKey::ExpansionEpoch,
        InputValue::ExpansionEpoch(1),
    )]))
    .unwrap();
    let context = DemandContext {
        request: Digest([234; 32]),
        session: Digest([235; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut config = prepared.config.clone();
    config.unit = Key(unit.0);
    config.epoch = 1;
    config.artifacts[0].identity = Key(macro_artifact_identity(*tool, artifact).0);
    let request = invocation(
        &config,
        236,
        "deliberately_hangs",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        "",
    );
    let budget =
        SharedExpansionBudget::new(context.request, manifest.graph.fingerprint(), 1, 2_000_000)
            .unwrap()
            .with_deadlines(Duration::from_secs(10), Duration::from_secs(8))
            .unwrap();
    let mut run = MacroRun::begin(&mut db, context, &manifest, &plan, true)
        .unwrap()
        .with_expansion_budget(budget.clone())
        .unwrap();
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let phase = PhaseIdentity {
        frame: Digest([237; 32]),
        body: BodyKey {
            definition: Digest([238; 32]),
            exact_body_bytes: Digest([239; 32]),
        },
        index: 0,
    };
    let revision = db.revision();
    let start = std::time::Instant::now();
    let error = run
        .fulfil(&mut db, unit, phase.clone(), &request, &mut cache)
        .err()
        .unwrap();
    assert_eq!(error.code, "LRR-E-MACRO-TIMEOUT", "{error:?}");
    assert_eq!(run.unit_invocations(unit), 1);
    assert!(start.elapsed() < Duration::from_secs(10));
    assert_eq!(db.revision(), revision);
    assert_eq!(budget.totals().unwrap().outputs, 0);
    assert!(
        run.fulfil(&mut db, unit, phase, &request, &mut cache)
            .is_err()
    );
    assert_eq!(run.unit_invocations(unit), 1);
    assert!(run.verify_for_seal(&mut db).is_err());
    fs::write(evidence.join("deadline.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M013","platform":"aarch64-apple-darwin","rust":"1.99.0",
        "cumulative_timeout_ms":8000,"invocation_timeout_ms":10000,"diagnostic":error.code,"native_invocations":1,"adopted":false,"retry_invoked":false,"release_complete":false})).unwrap()).unwrap();
}

#[test]
fn actual_cargo_planned_units_have_independent_macro_run_sessions() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        db::{
            dependency::{InputKey, InputValue},
            memo::Database,
        },
        expand::cache::ExpansionOutputCache,
        project::tool_manifest::{InputKind, InputPlan, InputRoot},
        service::{
            expansion::{DemandContext, PhaseIdentity},
            macro_session::macro_artifact_identity,
            run::MacroRun,
        },
        source::manifest::BodyKey,
    };
    let prepared = prepared();
    let evidence = prepared.work.join("multi-unit-owner");
    fs::create_dir(&evidence).unwrap();
    let manifest = service_fixture::prepare_with_live_dependency(
        &evidence.join("prepared"),
        &prepared.config.directory,
    );
    assert_eq!(manifest.graph.roots.len(), 1);
    let units: Vec<_> = manifest
        .graph
        .units
        .iter()
        .filter(|(_, u)| u.key.role == lrr::project::cargo_units::UnitRole::Live)
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(units.len(), 2);
    let roots = [
        evidence.join("prepared/app"),
        evidence.join("prepared/secondary"),
        prepared.config.directory.clone(),
    ];
    let plan = InputPlan::new(
        roots
            .into_iter()
            .map(|root| {
                InputRoot::open(
                    InputKind::Source,
                    &root,
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
    let mut db = Database::default();
    db.publish_inputs(BTreeMap::from([(
        InputKey::ExpansionEpoch,
        InputValue::ExpansionEpoch(1),
    )]))
    .unwrap();
    let root = *manifest.graph.roots.first().unwrap();
    let context = DemandContext {
        request: Digest([231; 32]),
        session: Digest([232; 32]),
        unit: root,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&root].fingerprint(),
    };
    let mut run = MacroRun::begin(&mut db, context, &manifest, &plan, true).unwrap();
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let (tool, artifact) = manifest.artifacts.iter().next().unwrap();
    let mut observations = vec![];
    for unit in &units {
        let mut config = prepared.config.clone();
        config.unit = Key(unit.0);
        config.artifacts[0].identity = Key(macro_artifact_identity(*tool, artifact).0);
        let request = invocation(
            &config,
            233,
            "session_counter",
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            "",
        );
        let phase = |index| PhaseIdentity {
            frame: Digest([234; 32]),
            body: BodyKey {
                definition: Digest([235; 32]),
                exact_body_bytes: Digest([236; 32]),
            },
            index,
        };
        let first = run
            .fulfil(&mut db, *unit, phase(0), &request, &mut cache)
            .unwrap();
        let repeated = run
            .fulfil(&mut db, *unit, phase(1), &request, &mut cache)
            .unwrap();
        assert!(Arc::ptr_eq(first.tokens(), repeated.tokens()));
        assert_eq!(run.unit_invocations(*unit), 1);
        assert_eq!(
            literals(&token_wire::TokenForest::decode(&first.tokens().encode().unwrap()).unwrap()),
            ["1_u64"]
        );
        let other = invocation(
            &config,
            237,
            "session_counter",
            ProcForm::FunctionLike,
            ProcGrammar::Expression,
            "",
        );
        let next = run
            .fulfil(&mut db, *unit, phase(2), &other, &mut cache)
            .unwrap();
        assert_eq!(
            literals(&token_wire::TokenForest::decode(&next.tokens().encode().unwrap()).unwrap()),
            ["2_u64"]
        );
        assert_eq!(run.unit_invocations(*unit), 2);
        observations.push(serde_json::json!({"unit":unit.hex(),"target":manifest.graph.units[unit].key.target_name,"invocations":run.unit_invocations(*unit),"semantic":first.semantic().hex()}));
    }
    assert_ne!(observations[0]["semantic"], observations[1]["semantic"]);
    let verified = run.verify_for_seal(&mut db).unwrap();
    assert_eq!(verified.unit_count(), 2);
    fs::write(evidence.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","observations":observations,"verification":verified.identity().hex(),"scope":"one exact Cargo root with a live dependency, macro-only preparation and schema4 fixture and actual independent unit sessions; production graph integration remains later M038"})).unwrap()).unwrap();
}

#[test]
fn actual_final_macro_input_verification_refuses_edit_and_closes_run() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        db::{
            dependency::{InputKey, InputValue},
            memo::Database,
        },
        expand::cache::ExpansionOutputCache,
        service::{
            expansion::{DemandContext, PhaseIdentity},
            macro_session::macro_artifact_identity,
            run::MacroRun,
        },
        source::manifest::BodyKey,
    };
    let prepared = prepared();
    let evidence = prepared.work.join("final-input-edit");
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let plan = recorded_macro_plan(&evidence, &prepared.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let (tool, artifact) = manifest.artifacts.iter().next().unwrap();
    let mut db = Database::default();
    db.publish_inputs(BTreeMap::from([(
        InputKey::ExpansionEpoch,
        InputValue::ExpansionEpoch(1),
    )]))
    .unwrap();
    let context = DemandContext {
        request: Digest([241; 32]),
        session: Digest([242; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut config = prepared.config.clone();
    config.unit = Key(unit.0);
    config.artifacts[0].identity = Key(macro_artifact_identity(*tool, artifact).0);
    let request = invocation(
        &config,
        243,
        "session_counter",
        ProcForm::FunctionLike,
        ProcGrammar::Expression,
        "",
    );
    let phase = PhaseIdentity {
        frame: Digest([244; 32]),
        body: BodyKey {
            definition: Digest([245; 32]),
            exact_body_bytes: Digest([246; 32]),
        },
        index: 0,
    };
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
    run.fulfil(&mut db, unit, phase.clone(), &request, &mut cache)
        .unwrap();
    assert_eq!(run.unit_invocations(unit), 1);
    let retained = cache.retained_bytes();
    assert!(retained > 0);
    fs::write(
        evidence.join("app/src/lib.rs"),
        "fn cold(){ another_undefined(); }\n",
    )
    .unwrap();
    let error = run.verify_for_seal(&mut db).unwrap_err();
    assert_eq!(error.code, "LRR-E-TOOL-PREPARE-STALE");
    assert!(
        run.fulfil(&mut db, unit, phase, &request, &mut cache)
            .is_err()
    );
    assert!(run.verify_for_seal(&mut db).is_err());
    assert_eq!(run.unit_invocations(unit), 1);
    assert_eq!(cache.retained_bytes(), retained);
    assert!(MacroRun::begin(&mut db, context, &manifest, &plan, true).is_err());
    fs::write(evidence.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","error":error.code,"invocations":1,"certificate_issued":false,"pure_facts_retained":true,"scope":"actual source edit after expansion before final verification closes run; no catalog seal/guest acceptance"})).unwrap()).unwrap();
}

#[test]
fn actual_source_function_procedural_request_preserves_selection_and_executes_once() {
    use lrr::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        db::{
            key::{OutlineKey, QueryKey},
            poll::{ExpansionRequest, InputRequest},
        },
        expand::scopes::MacroName,
        frontend::{hygiene::HygieneFacts, source_macro_routes::SourceMacroRoutes},
        service::{
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
            function_macro_requests::FunctionInvocationInput,
            item_macro_requests::ResolvedItemTarget,
            macro_session::{MacroSession, macro_artifact_identity},
        },
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            manifest::BodyKey,
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::{parse::ParseKey, token_wire as runtime},
    };
    let prepared = prepared();
    let evidence = prepared.work.join("source-function-request");
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let (producer, artifact) = manifest.artifacts.iter().next().unwrap();
    let context = DemandContext {
        request: Digest([91; 32]),
        session: Digest([92; 32]),
        unit,
        revision: 0,
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut session = MacroSession::open(
        context.clone(),
        &manifest,
        true,
        PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]),
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .unwrap();
    let text = "use lrr_fixture_macros::identity as selected; selected!(pub fn made()->i64{1729_i64}); selected!(pub fn other()->i64{1733_i64}); #[cfg(any())] selected!(pub fn inactive(){});";
    let cfg = CfgSet::guest_default();
    let profile = ProfileIdentity::current();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
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
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    assert_eq!(view.outline().item_invocations.len(), 2);
    let mut store = lrr_expand_adapter::hygiene::HygieneStore::default();
    let root = store.root(Key(unit.0), Edition::E2024).unwrap();
    let rows = store.resolution_contexts(&[root]).unwrap();
    let names = store.name_contexts(&[root]).unwrap();
    let contexts = runtime::TokenForest::decode(
        &TokenForest::new(vec![], store.records(), vec![])
            .unwrap()
            .encode()
            .unwrap(),
    )
    .unwrap();
    let facts = HygieneFacts::new(
        contexts.contexts().to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
    let catalog = session.export_catalog(&context).unwrap();
    let mut routes = SourceMacroRoutes::from_procedural_catalog(&catalog).unwrap();
    routes.push(
        SourceMacroRoutes::from_outline(
            view.outline(),
            runtime::Key(root.0),
            facts,
            &BTreeMap::from([(
                MacroName::parse("lrr_fixture_macros", 2024).unwrap(),
                *producer,
            )]),
        )
        .unwrap(),
    );
    let namespaces = SourceMacroRoutes::candidate_graph(&routes).unwrap();
    let first =
        FunctionInvocationInput::from_source(&view, view.outline().item_invocations[0].item_input)
            .unwrap();
    let registry = TokenForest::decode(&first.arguments().encode().unwrap()).unwrap();
    let target = || ResolvedItemTarget {
        artifact: Key(macro_artifact_identity(*producer, artifact).0),
        export: "identity".into(),
        scope: Key(first.scope().fingerprint().0),
        parent_origin: first.parent_origin(),
        registry: &registry,
        definition_site: first.call_site(),
        call_site: first.call_site(),
        mixed_site: first.call_site(),
        sources: vec![],
    };
    let selected = first
        .resolve(&session, &context, &namespaces, target())
        .unwrap();
    assert_eq!(selected.source(), &selection);
    assert_eq!(
        selected.key().invocation,
        view.outline().item_invocations[0].item_input
    );
    assert_eq!(
        selected.key(),
        &session.request_key(&context, selected.request()).unwrap()
    );
    assert_eq!(
        session.invocations(),
        0,
        "source resolution cannot execute the helper"
    );
    assert!(FunctionInvocationInput::from_source(&view, Digest([99; 32])).is_err());
    let mut wrong = target();
    wrong.export = "emit_items".into();
    assert!(
        first
            .resolve(&session, &context, &namespaces, wrong)
            .is_err()
    );
    let mut wrong = target();
    wrong.scope = Key([98; 32]);
    assert!(
        first
            .resolve(&session, &context, &namespaces, wrong)
            .is_err()
    );
    let mut stale = context.clone();
    stale.epoch += 1;
    assert!(
        first
            .resolve(&session, &stale, &namespaces, target())
            .is_err()
    );
    assert!(
        first
            .resolve(&session, &context, &Default::default(), target())
            .is_err()
    );
    let later =
        FunctionInvocationInput::from_source(&view, view.outline().item_invocations[1].item_input)
            .unwrap();
    assert_ne!(later.call_site(), first.call_site());
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let phase = PhaseIdentity {
        frame: Digest([93; 32]),
        body: BodyKey {
            definition: Digest([94; 32]),
            exact_body_bytes: Digest([95; 32]),
        },
        index: 0,
    };
    let token = coordinator
        .issue(
            QueryKey::CurrentExpansionInput(Box::new(selected.key().clone())),
            phase,
            InputRequest::Expand(Box::new(ExpansionRequest {
                key: selected.key().clone(),
            })),
        )
        .unwrap();
    let record = session
        .fulfil(&context, &mut coordinator, &token, selected.request())
        .unwrap();
    assert_eq!(record.key(), selected.key());
    let output_wire = record.tokens().unwrap().encode().unwrap();
    let output = TokenForest::decode(&output_wire).unwrap();
    assert!(identifiers(&output).contains(&"made".into()));
    assert!(!identifiers(&output).contains(&"other".into()));
    assert_eq!(literals(&output), ["1729_i64"]);
    let replay = session
        .fulfil(&context, &mut coordinator, &token, selected.request())
        .unwrap();
    assert_eq!(replay.output(), record.output());
    assert_eq!(session.invocations(), 1);
    fs::write(evidence.join("source.rs"), text).unwrap();
    fs::write(evidence.join("output.tokens"), &output_wire).unwrap();
    fs::write(evidence.join("request.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M013","platform":"aarch64-apple-darwin","rust":"1.99.0","source_request":true,"alias_resolved":true,"native_invocations":session.invocations(),"replayed_without_execution":true,"scope":"source request materialization and authorized session; discovery integration pending","release_complete":false})).unwrap()).unwrap();
}

#[test]
fn actual_procedural_source_worklist_integrates_macro_and_import_before_sealing() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::identity as selected; selected!(macro_rules! leaf{()=>{pub fn made()->i64{1729_i64}}} use crate::made as alias; leaf!();); pub fn caller()->i64{alias()}",
        true,
        "procedural-source-worklist",
    );
}
#[test]
fn actual_procedural_source_worklist_refuses_late_macro_import_ambiguity() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::identity as selected; selected!(use lrr_fixture_macros::session_counter as selected;);",
        false,
        "procedural-source-worklist-ambiguity",
    );
}
#[test]
fn actual_procedural_source_publication_refuses_final_input_edit() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::identity as selected; selected! { macro_rules! leaf { () => {pub fn made()->i64{7}} } use crate::made as alias; leaf!(); } pub fn caller()->i64{alias()}",
        true,
        "procedural-source-final-edit",
    );
}

#[test]
fn actual_procedural_source_module_reuses_run_and_seals_nested_physical_source() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::identity as selected; selected! {pub mod child;} pub fn caller()->i64{child::grandchild::alias()} fn cold(){deliberately_undefined();}",
        true,
        "procedural-source-modules",
    );
}

#[test]
fn actual_procedural_source_module_refuses_late_alias_ambiguity_before_sealing() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::identity as selected; selected! {pub mod child;} pub fn caller()->i64{child::grandchild::alias()} fn cold(){deliberately_undefined();}",
        true,
        "procedural-source-modules-ambiguity",
    );
}

#[test]
fn actual_generated_procedural_call_reuses_run_and_seals_generated_imports() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::identity as selected; selected! { selected! { macro_rules! leaf { () => {pub fn made()->i64{7}} } use crate::made as alias; leaf!(); } } pub fn caller()->i64{alias()}",
        true,
        "procedural-source-nested",
    );
}

#[test]
fn actual_generated_procedural_grandchild_preserves_session_and_output_accounting() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::identity as selected; selected! { selected! { selected! { macro_rules! leaf { () => {pub fn made()->i64{7}} } use crate::made as alias; leaf!(); } } } pub fn caller()->i64{alias()}",
        true,
        "procedural-source-nested-grandchild",
    );
}

#[test]
fn actual_generated_procedural_call_refuses_late_import_ambiguity_before_sealing() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::identity as selected; selected! { selected! { use lrr_fixture_macros::session_counter as selected; } }",
        true,
        "procedural-source-nested-ambiguity",
    );
}

#[test]
fn replacing_generated_attribute_uses_final_cold_impl_inventory() {
    procedural_source_worklist_case_with_impls(
        "struct Global;use lrr_fixture_macros::identity as selected;selected!{#[lrr_fixture_macros::attribute_tokens(macro_rules! leaf{()=>{pub fn made()->i64{7} fn cold(){impl Global{fn generated(){missing();}}missing();}}} leaf!(); use crate::made as alias;)] fn discarded(){impl Global{fn obsolete(){missing();}}missing();}} pub fn caller()->i64{alias()}",
        true,
        "procedural-source-generated-attribute-cold-inventory",
        Some(1),
    );
}

#[test]
fn replacing_generated_attribute_retires_old_unknown_macro_producer() {
    procedural_source_worklist_case_with_impls(
        "struct Global;use lrr_fixture_macros::identity as selected;selected!{#[lrr_fixture_macros::attribute_tokens(macro_rules! leaf{()=>{pub fn made()->i64{7} fn cold(){impl Global{fn generated(){missing();}}missing();}}} leaf!(); use crate::made as alias;)] fn discarded(){unknown!({impl Global{fn obsolete(){missing();}}});missing();}} pub fn caller()->i64{alias()}",
        true,
        "procedural-source-generated-attribute-retired-cold-producer",
        Some(1),
    );
}

#[test]
fn replacing_source_attribute_retires_old_unknown_macro_producer() {
    procedural_source_worklist_case_with_impls(
        "struct Global;#[lrr_fixture_macros::attribute_tokens(macro_rules! leaf{()=>{pub fn made()->i64{7} fn cold(){impl Global{fn generated(){missing();}}missing();}}} leaf!(); use crate::made as alias;)] fn discarded(){unknown!({impl Global{fn obsolete(){missing();}}});missing();} pub fn caller()->i64{alias()}",
        true,
        "procedural-source-attribute-retired-cold-producer",
        Some(1),
    );
}

#[test]
fn replacing_source_attribute_uses_final_cold_impl_inventory() {
    procedural_source_worklist_case_with_impls(
        "struct Global;#[lrr_fixture_macros::attribute_tokens(macro_rules! leaf{()=>{pub fn made()->i64{7} fn cold(){impl Global{fn generated(){missing();}}missing();}}} leaf!(); use crate::made as alias;)] fn discarded(){impl Global{fn obsolete(){missing();}}missing();} pub fn caller()->i64{alias()}",
        true,
        "procedural-source-attribute-cold-inventory",
        Some(1),
    );
}

#[test]
fn actual_source_replacing_attribute_integrates_generated_calls_and_removes_original() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::attribute_tokens as selected_attribute; use lrr_fixture_macros::identity as selected; #[selected_attribute(selected! { macro_rules! leaf { () => {pub fn made()->i64{7}} } use crate::made as alias; leaf!(); })] fn discarded(){deliberately_undefined();} pub fn caller()->i64{alias()}",
        true,
        "procedural-source-attribute",
    );
}

#[test]
fn actual_source_replacing_attribute_removes_later_unresolved_annotation() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::attribute_tokens(macro_rules! leaf { () => {pub fn made()->i64{7}} } use crate::made as alias; leaf!();)] #[unresolved_later_attribute] fn discarded(){deliberately_undefined();} pub fn caller()->i64{alias()}",
        true,
        "procedural-source-attribute-removes-later",
    );
}

#[test]
fn actual_source_replacing_attribute_refuses_generated_alias_ambiguity() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::attribute_tokens as selected_attribute; #[selected_attribute(use lrr_fixture_macros::make_tests as selected_attribute;)] fn discarded(){deliberately_undefined();}",
        false,
        "procedural-source-attribute-ambiguity",
    );
}

#[test]
fn actual_source_replacing_attribute_materializes_cfg_attr_without_inactive_execution() {
    procedural_source_worklist_case(
        "#[cfg_attr(any(), lrr_fixture_macros::panic_attribute)] #[cfg_attr(all(), lrr_fixture_macros::attribute_tokens(macro_rules! leaf { () => {pub fn made()->i64{7}} } use crate::made as alias; leaf!();))] #[unresolved_later_attribute] fn discarded(){deliberately_undefined();} pub fn caller()->i64{alias()}",
        true,
        "procedural-source-attribute-cfg",
    );
}

#[test]
fn actual_generated_replacing_attribute_chain_reanalyses_output_before_sealing() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::attribute_tokens(#[lrr_fixture_macros::attribute_tokens(macro_rules! leaf { () => {pub fn made()->i64{7}} } use crate::made as alias; leaf!();)] fn discarded(){deliberately_undefined();})] fn discarded_outer(){deliberately_undefined();} pub fn caller()->i64{alias()}",
        true,
        "procedural-source-attribute-chain",
    );
}

#[test]
fn actual_generated_replacing_attribute_after_function_macro_preserves_source_origins() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::identity as selected; selected! {#[lrr_fixture_macros::attribute_tokens(macro_rules! leaf { () => {pub fn made()->i64{7}} } use crate::made as alias; leaf!();)] fn discarded(){deliberately_undefined();}} pub fn caller()->i64{alias()}",
        true,
        "procedural-source-generated-attribute",
    );
}

#[test]
fn actual_generated_replacing_attribute_refuses_late_conflicting_alias() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::attribute_tokens as chosen; #[lrr_fixture_macros::attribute_tokens(#[chosen(use lrr_fixture_macros::make_tests as chosen;)] fn discarded(){deliberately_undefined();})] fn discarded_outer(){deliberately_undefined();}",
        true,
        "procedural-source-attribute-chain-ambiguity",
    );
}

#[test]
fn actual_generated_replacing_attribute_keeps_derive_output_pending() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-pending",
    );
}

#[test]
fn actual_generated_derive_helper_requests_capture_exact_source_without_execution() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-helper-declarations",
    );
}

#[test]
fn actual_generated_derive_appends_preserve_original_item_and_publish_only_after_integration() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-append",
    );
}

#[test]
fn actual_generated_derives_execute_in_order_and_wait_for_append_without_replacing_item() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-execution",
    );
}

#[test]
fn actual_generated_derive_helpers_are_item_local_and_preserved_without_execution() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-helper-scope",
    );
}

#[test]
fn actual_generated_derive_discovery_dispatches_and_adopts_helper_receipts_without_sealing() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-helper-adoption",
    );
}

#[test]
fn actual_generated_replacing_attribute_materializes_cfg_attr_before_execution() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::identity as selected; selected! {#[cfg_attr(any(), lrr_fixture_macros::panic_attribute)] #[cfg_attr(all(), lrr_fixture_macros::attribute_tokens(macro_rules! leaf { () => {pub fn made()->i64{7}} } use crate::made as alias; leaf!();))] fn discarded(){deliberately_undefined();}} pub fn caller()->i64{alias()}",
        true,
        "procedural-source-generated-attribute-cfg",
    );
}

#[test]
fn rustc_generated_macro_rules_derive_executes_original_auxiliary_and_original_invocation() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        expand::{
            builtin_attributes::descriptor,
            generated_derives::{GeneratedDeriveAnnotations, GeneratedDerivePlan},
            generated_items::GeneratedItems,
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
            record::ExpansionRecord,
        },
        source::snapshot::CfgSet,
    };
    let prepared = prepared();
    let root = prepared.work.join("official-gen-macro-rules");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname=\"gen_macro_rules\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    let auxiliary = include_bytes!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/gen-macro-rules.rs"
    );
    fs::write(root.join("src/lib.rs"), auxiliary).unwrap();
    // Explicit preparation compiles only the unchanged upstream macro-host source.
    let lock = Command::new("cargo")
        .args([
            "+1.99.0",
            "generate-lockfile",
            "--offline",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .output()
        .unwrap();
    fs::write(root.join("lock.stdout"), &lock.stdout).unwrap();
    fs::write(root.join("lock.stderr"), &lock.stderr).unwrap();
    assert!(lock.status.success());
    let build = Command::new("cargo")
        .args([
            "+1.99.0",
            "build",
            "--locked",
            "--offline",
            "--message-format=json",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(root.join("target"))
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .unwrap();
    fs::write(root.join("build.stdout"), &build.stdout).unwrap();
    fs::write(root.join("build.stderr"), &build.stderr).unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let messages: Vec<Value> = String::from_utf8(build.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let artifacts: Vec<_> = messages
        .iter()
        .filter(|m| m["reason"] == "compiler-artifact")
        .collect();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(
        artifacts[0]["target"]["kind"],
        serde_json::json!(["proc-macro"])
    );
    let filenames = artifacts[0]["filenames"].as_array().unwrap();
    assert_eq!(filenames.len(), 1);
    let artifact = PathBuf::from(filenames[0].as_str().unwrap());
    let mut config = prepared.config.clone();
    config.directory = root.clone();
    config.artifacts = vec![MacroArtifact {
        identity: Key([98; 32]),
        raw_blake3: blake3::hash(&fs::read(&artifact).unwrap())
            .to_hex()
            .to_string(),
        path: artifact,
    }];
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/gen-macro-rules.rs"
    );
    let source_identity = SourceIdentity {
        unit: config.unit,
        content: Key(*blake3::hash(source.as_bytes()).as_bytes()),
        path: "tests/ui/proc-macro/gen-macro-rules.rs".into(),
    };
    let forest = token_wire::parse_source(
        source,
        source_identity.clone(),
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: config.unit,
            edition: Edition::E2015,
        },
        Key([17; 32]),
    )
    .unwrap();
    // An owned source envelope supplies the generated-item payload component.
    // No compiler is invoked for this runtime source.
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest([99; 32]),
        },
        invocation: Digest([100; 32]),
        input_tokens: Digest(source_identity.content.0),
        consumer_compile_env: Digest([101; 32]),
        parent_context: Digest([102; 32]),
        definition_edition: 2015,
        call_edition: 2015,
        grammar: GrammarSlot::Items,
        engine_version: Digest([103; 32]),
        procedural_epoch: None,
    };
    let record =
        Arc::new(ExpansionRecord::from_wire(key.clone(), forest.encode().unwrap()).unwrap());
    let generated = GeneratedItems::admit(&key, record, config.epoch).unwrap();
    let index = generated
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "S"))
        .unwrap();
    let cfg = CfgSet::guest_default();
    let annotations = GeneratedDeriveAnnotations::new(
        &generated,
        index,
        &cfg,
        [(0, descriptor("derive").unwrap().unwrap())],
    )
    .unwrap();
    let plan = GeneratedDerivePlan::prepare(&generated, index, &cfg, &annotations).unwrap();
    assert_eq!(
        plan.annotations()[0].paths()[0].segments,
        ["repro", "repro"]
    );
    let input = TokenForest::decode(
        &plan
            .materialize_input(&generated, index, &cfg)
            .unwrap()
            .encode()
            .unwrap(),
    )
    .unwrap();
    let site = input.spans()[0].key();
    let request = ProcInvocation {
        identity: Key(key.fingerprint().0),
        unit: config.unit,
        epoch: config.epoch,
        artifact: config.artifacts[0].identity,
        export: "repro".into(),
        form: ProcForm::Derive,
        grammar: ProcGrammar::Items,
        scope: Key([104; 32]),
        parent_origin: Key([17; 32]),
        input,
        attributes: None,
        definition_site: site,
        call_site: site,
        mixed_site: site,
        sources: vec![
            lrr_expand_adapter::proc_client::CapturedSpanSource::new(
                source_identity,
                source.into(),
                lrr_expand_adapter::proc_client::SourceDigestEncoding::RawBlake3,
                None,
            )
            .unwrap(),
        ],
    };
    let mut session = ProcSession::new(config).unwrap();
    let output = session.expand(&request).unwrap();
    fs::write(root.join("derive-output.lrrtok"), output.encode().unwrap()).unwrap();
    let [
        TokenTree::Group {
            children: emitted, ..
        },
    ] = output.roots()
    else {
        panic!("macro output root absent")
    };
    let [
        TokenTree::Ident { text, .. },
        TokenTree::Punct { scalar: '!', .. },
        TokenTree::Ident { text: name, .. },
        body @ TokenTree::Group {
            delimiter: token_wire::Delimiter::Brace,
            ..
        },
    ] = emitted.as_slice()
    else {
        panic!("unexpected upstream derive output")
    };
    assert_eq!(text, "macro_rules");
    assert_eq!(name, "m");
    let definition = TokenForest::new(
        vec![body.clone()],
        output.contexts().to_vec(),
        output.spans().to_vec(),
    )
    .unwrap();
    let [TokenTree::Group { children, .. }] = forest.roots() else {
        panic!("source root absent")
    };
    let call = children
        .windows(3)
        .find_map(|tokens| match tokens {
            [
                TokenTree::Ident { text, .. },
                TokenTree::Punct { scalar: '!', .. },
                group @ TokenTree::Group {
                    delimiter: token_wire::Delimiter::Parenthesis,
                    ..
                },
            ] if text == "m" => Some(group.clone()),
            _ => None,
        })
        .unwrap();
    let TokenTree::Group { open, .. } = &call else {
        unreachable!()
    };
    let call_site = *open;
    let input = TokenForest::new(
        vec![call],
        forest.contexts().to_vec(),
        forest.spans().to_vec(),
    )
    .unwrap();
    let result = lrr_expand_adapter::declarative::DeclarativeMacro::new(definition)
        .unwrap()
        .expand(&input, call_site)
        .unwrap();
    let result = result.unwrap();
    let [TokenTree::Group { children, .. }] = result.tokens.roots() else {
        panic!("call output root absent")
    };
    assert!(children.is_empty());
    assert_eq!(session.invocations(), 1);
    assert_eq!(fs::read(root.join("src/lib.rs")).unwrap(), auxiliary);
    fs::write(root.join("official-component.json"), serde_json::to_vec_pretty(&serde_json::json!({"upstream_case":"tests/ui/proc-macro/gen-macro-rules.rs","auxiliary_unchanged":true,"macro_host_artifacts":1,"native_invocations":1,"original_m_invocation_expanded":true,"component_passed":true,"full_fixture_acceptance":false})).unwrap()).unwrap();
}

#[test]
fn rustc_original_derive_stdout_corruption_is_terminal_and_not_a_fixture_pass() {
    use lrr::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        service::item_macro_inputs::{self, ResolvedDeriveAnnotations},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::parse::ParsedFile,
    };
    let prepared = prepared();
    let root = prepared.work.join("official-original-derive-order");
    fs::create_dir_all(root.join("src")).unwrap();
    let auxiliary = include_bytes!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/multiple-derives.rs"
    );
    fs::write(root.join("src/lib.rs"), auxiliary).unwrap();
    fs::write(root.join("Cargo.toml"),"[package]\nname=\"multiple_derives\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    for (label, args) in [
        ("lock", vec!["generate-lockfile", "--offline"]),
        (
            "build",
            vec!["build", "--locked", "--offline", "--message-format=json"],
        ),
    ] {
        let mut command = Command::new("cargo");
        command
            .arg("+1.99.0")
            .args(args)
            .arg("--manifest-path")
            .arg(root.join("Cargo.toml"));
        if label == "build" {
            command.arg("--target-dir").arg(root.join("target"));
        }
        let result = command
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_INCREMENTAL", "0")
            .output()
            .unwrap();
        fs::write(root.join(format!("{label}.stdout")), &result.stdout).unwrap();
        fs::write(root.join(format!("{label}.stderr")), &result.stderr).unwrap();
        assert!(
            result.status.success(),
            "unchanged upstream tool preparation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let messages: Vec<Value> = fs::read_to_string(root.join("build.stdout"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let artifacts: Vec<_> = messages
        .iter()
        .filter(|m| m["reason"] == "compiler-artifact")
        .collect();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(
        artifacts[0]["target"]["kind"],
        serde_json::json!(["proc-macro"])
    );
    let filenames = artifacts[0]["filenames"].as_array().unwrap();
    assert_eq!(filenames.len(), 1);
    let artifact = PathBuf::from(filenames[0].as_str().unwrap());
    let artifact_hash = blake3::hash(&fs::read(&artifact).unwrap())
        .to_hex()
        .to_string();
    let mut config = prepared.config.clone();
    config.directory = root.clone();
    config.artifacts = vec![MacroArtifact {
        identity: Key([171; 32]),
        path: artifact,
        raw_blake3: artifact_hash,
    }];
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.rs"
    );
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let cfg = CfgSet::guest_default();
    let outline = parsed.outline(Digest(config.unit.0), vec![], &cfg).unwrap();
    let item = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "Foo")
        .unwrap();
    let facts = parsed
        .derive_item_inputs_at(item.declaration_span, &cfg)
        .unwrap();
    assert_eq!(facts.item.attributes.len(), 4);
    let identity = SourceIdentity {
        unit: config.unit,
        content: Key(blob.content().0),
        path: "tests/ui/proc-macro/derive-expand-order.rs".into(),
    };
    let original = token_wire::parse_source(
        source,
        identity.clone(),
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: config.unit,
            edition: Edition::E2024,
        },
        Key([172; 32]),
    )
    .unwrap();
    let forest = lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let owner = item.def_key.fingerprint();
    let descriptor = lrr::expand::builtin_attributes::descriptor("derive")
        .unwrap()
        .unwrap();
    let proof =
        ResolvedDeriveAnnotations::new(owner, &facts, (0..4).map(|at| (at, descriptor.clone())))
            .unwrap();
    let mut session = ProcSession::new(config.clone()).unwrap();
    let exports: std::collections::BTreeSet<_> = session
        .exports()
        .map(|e| (e.name.to_owned(), e.form))
        .collect();
    assert_eq!(
        exports,
        ["First", "Second", "Third", "Fourth", "Fifth"]
            .into_iter()
            .map(|n| (n.to_owned(), ProcForm::Derive))
            .collect()
    );
    let mut rows = Vec::new();
    let mut requests = Vec::new();
    for (annotation, ordinal) in [(0, 0), (1, 0), (2, 0), (2, 1), (3, 0)] {
        let input = item_macro_inputs::derive_occurrence_resolved(
            owner,
            &facts,
            (annotation, ordinal),
            &proof,
            &forest.spans()[0].source,
            &forest,
        )
        .unwrap();
        let tokens = TokenForest::decode(&input.item().encode().unwrap()).unwrap();
        let site = tokens.spans()[0].key();
        let request = ProcInvocation {
            identity: Key(input.identity().0),
            unit: config.unit,
            epoch: config.epoch,
            artifact: config.artifacts[0].identity,
            export: input.selected().segments[0].clone(),
            form: ProcForm::Derive,
            grammar: ProcGrammar::Items,
            scope: Key(item.scope.fingerprint().0),
            parent_origin: Key(outline.fingerprint().0),
            input: tokens,
            attributes: None,
            definition_site: site,
            call_site: site,
            mixed_site: site,
            sources: vec![
                lrr_expand_adapter::proc_client::CapturedSpanSource::new(
                    identity.clone(),
                    source.into(),
                    lrr_expand_adapter::proc_client::SourceDigestEncoding::LrrSourceBytes,
                    None,
                )
                .unwrap(),
            ],
        };
        fs::write(
            root.join(format!("input-{annotation}-{ordinal}.lrrtok")),
            input.item().encode().unwrap(),
        )
        .unwrap();
        rows.push(serde_json::json!({"annotation":annotation,"ordinal":ordinal,"export":request.export,"occurrence":input.identity().hex(),"input_blake3":blake3::hash(&input.item().encode().unwrap()).to_hex().to_string(),"executed":false,"completed":false}));
        requests.push(request);
    }
    // This is a terminal-refusal regression, not acceptance of the five-derive case.
    assert_eq!(requests.len(), 5);
    let error = session.expand(&requests[0]).unwrap_err();
    assert_eq!(error.code, "LRR-E-MACRO-CRASH");
    assert!(error.message.starts_with("failed to decode message:"));
    assert_eq!(session.invocations(), 1);
    assert!(session.helper_terminated().unwrap());
    rows[0]["executed"] = serde_json::json!(true);
    let first_invocations = session.invocations();
    let retry = session.expand(&requests[0]).unwrap_err();
    assert_eq!(retry.code, error.code);
    assert_eq!(retry.message, error.message);
    assert_eq!(
        session.expand(&requests[1]).unwrap_err().code,
        "LRR-E-MACRO-CRASH"
    );
    assert_eq!(session.invocations(), 1);
    let (stdout, stderr) = session.diagnostics().unwrap();
    fs::write(root.join("server.stdout"), &stdout).unwrap();
    fs::write(root.join("server.stderr"), &stderr).unwrap();
    assert!(
        stdout
            .windows(b"Derive First:".len())
            .any(|part| part == b"Derive First:")
    );
    let snapshot = session.diagnostic_snapshot().unwrap();
    assert!(!snapshot.protocol_truncated && !snapshot.stderr_truncated);
    fs::write(root.join("source.rs"), source).unwrap();
    fs::write(root.join("original-derive-input.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "upstream_case":"tests/ui/proc-macro/derive-expand-order.rs","upstream_auxiliary":"tests/ui/proc-macro/auxiliary/multiple-derives.rs",
        "native_invocations":session.invocations(),"rows":rows,"native_case_status":"failed_protocol","completed_occurrences":0,"unexecuted_occurrences":4,"diagnostic":error.code,"message":error.message,"helper_terminated":session.helper_terminated().unwrap(),"retry_native_invocations":session.invocations()-first_invocations,"source_adaptations":[],"macro_host_source_unchanged":fs::read(root.join("src/lib.rs")).unwrap()==auxiliary,
        "source_profile_edition":2024,"macro_host_edition":2015,"server_compiler_commit":config.server.compiler_commit,"artifact_blake3":config.artifacts[0].raw_blake3,
        "helper_metadata_prepared":false,"discovery_sealed":false,"guest_execution":false,"full_fixture_acceptance":false
    })).unwrap()).unwrap();
}

#[test]
fn actual_derive_outputs_resolve_prior_prefix_and_replay_before_later_shadow() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish(prefix)] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-prefix",
    );
}

#[test]
fn actual_later_attribute_runs_after_configured_derives_and_replaces_original_item() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish(later_attribute)] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-later-attribute",
    );
}

#[test]
fn actual_separate_derive_groups_append_in_order_before_later_attribute() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish(later_groups)] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-later-groups",
    );
}

#[test]
fn actual_empty_derive_advances_to_following_attribute_without_native_derive_call() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish(empty_attribute)] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-empty-derive",
    );
}

#[test]
fn actual_empty_groups_before_and_after_native_derives_retire_without_extra_calls() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish(surround_empty)] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-surround-empty",
    );
}

#[test]
fn actual_following_identity_attribute_inherits_helpers_without_deriving_again() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish(helper_identity)] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-helper-identity",
    );
}

#[test]
fn actual_inherited_helpers_do_not_cross_a_generated_bang_expansion() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish(helper_bang)] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-helper-bang",
    );
}

#[test]
fn actual_attribute_module_preserves_helper_scope_and_reexports_constructor() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish(helper_module)] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-helper-module",
    );
}

#[test]
fn actual_chained_attributes_preserve_introduced_helpers_and_exact_payloads() {
    procedural_source_worklist_case(
        "#[lrr_fixture_macros::chain_begin] #[lrr_fixture_macros::chain_finish(helper_chain)] fn discarded(){undefined_cold();}",
        true,
        "procedural-source-attribute-chain-derive-helper-chain",
    );
}

#[test]
fn actual_original_source_derive_suspends_for_exact_helper_metadata_without_execution() {
    original_derive_metadata_case(false, "procedural-source-original-derive-declarations");
}
#[test]
fn actual_original_repeated_derive_metadata_receipts_are_occurrence_bound() {
    original_derive_metadata_case(true, "procedural-source-original-derive-repeated");
}
#[test]
fn actual_original_source_derive_executes_once_and_waits_for_append() {
    original_derive_metadata_case(false, "procedural-source-original-derive-execution");
}

#[test]
fn actual_original_source_derive_appends_and_advances_occurrences() {
    original_derive_metadata_case(false, "procedural-source-original-derive-append-order");
}

#[test]
fn actual_original_source_derive_append_discovers_nested_macros() {
    original_derive_metadata_case(false, "procedural-source-original-derive-append-nested");
}

#[test]
fn actual_original_source_derive_stale_append_refuses_integration() {
    original_derive_metadata_case(false, "procedural-source-original-derive-append-stale");
}

#[test]
fn actual_original_source_derive_completes_original_item_and_publishes() {
    original_derive_metadata_case(false, "procedural-source-original-derive-append-complete");
}

#[test]
fn actual_original_source_member_helper_uses_later_group_metadata() {
    original_derive_metadata_case(
        false,
        "procedural-source-original-derive-append-future-members",
    );
}

#[test]
fn actual_original_source_future_helper_does_not_grant_early_outer_scope() {
    original_derive_metadata_case(
        false,
        "procedural-source-original-derive-append-future-outer",
    );
}

#[test]
fn actual_original_source_qualified_future_member_helper_stays_pending() {
    original_derive_metadata_case(
        false,
        "procedural-source-original-derive-append-future-qualified",
    );
}

#[test]
fn rustc_original_source_same_named_helpers_keep_prior_introduction_scope() {
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/helper-attr-compat-collision.rs"
    )
    .replace(
        "#[macro_use(Empty)]\nextern crate test_macros;",
        "use lrr_fixture_macros::CheckHelperCollision as Empty;",
    )
    .replace(
        "#[macro_use(Empty2)]\nextern crate extra_empty_derive;",
        "use lrr_fixture_macros::SameHelper as Empty2;",
    )
    .replace("empty_helper", "lrr_derive_helper");
    procedural_source_worklist_case(
        &source,
        false,
        "procedural-source-original-derive-append-helper-collision",
    );
}

#[test]
fn actual_original_source_derive_unknown_outer_attribute_blocks_publication() {
    original_derive_metadata_case(
        false,
        "procedural-source-original-derive-append-unknown-outer",
    );
}

#[test]
fn actual_original_source_derive_stale_publication_refuses_completed_seal() {
    original_derive_metadata_case(
        false,
        "procedural-source-original-derive-append-publication-stale",
    );
}
#[test]
fn actual_original_source_derive_stale_source_refuses_native_execution() {
    original_derive_metadata_case(false, "procedural-source-original-derive-execution-stale");
}
#[test]
fn actual_original_source_derive_unknown_member_helper_stays_pending() {
    original_derive_metadata_case(false, "procedural-source-original-derive-unknown-member");
}
#[test]
fn actual_original_source_derive_qualified_member_helper_stays_pending() {
    original_derive_metadata_case(false, "procedural-source-original-derive-qualified-member");
}
#[test]
fn actual_original_derive_ambiguity_is_diagnostic_before_metadata_or_execution() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::CheckDeriveInput as Selected; use lrr_fixture_macros::Empty as Selected; #[derive(Selected)] struct Value; fn main(){}",
        false,
        "procedural-source-original-derive-ambiguous",
    );
}
#[test]
fn actual_original_derive_private_path_is_diagnostic_before_metadata_or_execution() {
    procedural_source_worklist_case(
        "mod hidden { use lrr_fixture_macros::CheckDeriveInput as Selected; } #[derive(hidden::Selected)] struct Value; fn main(){}",
        false,
        "procedural-source-original-derive-private",
    );
}
#[test]
fn rustc_original_derive_then_attribute_keeps_append_output_and_publishes_replacement() {
    original_following_attribute_case("replacement");
}
#[test]
fn original_derive_then_attribute_preserves_prior_macro_bindings() {
    original_following_attribute_case("prefix");
}
#[test]
fn original_derive_then_attribute_refuses_source_edit_before_native_dispatch() {
    original_following_attribute_case("stale");
}
#[test]
fn rustc_original_derive_helpers_follow_identity_attribute_without_reexecution() {
    original_following_attribute_case("forward");
}
#[test]
fn original_derive_helper_inheritance_stops_at_function_like_child() {
    original_following_attribute_case("bang");
}
#[test]
fn original_derive_helper_inheritance_does_not_resolve_sibling_attribute() {
    original_following_attribute_case("sibling");
}
#[test]
fn original_derive_helpers_follow_chained_forwarding_attributes() {
    original_following_attribute_case("chain");
}
fn original_following_attribute_case(mode: &str) {
    let original = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/attribute-after-derive.rs"
    );
    let start = original.find("#[derive(Print)]\n#[print_attr]").unwrap();
    let end = original[start..].find("\nfn main()").unwrap() + start;
    // Keep the upstream derive/attribute/member ordering; SDK exports check both
    // the configured derive input and the raw member tokens of the later attribute.
    let adapted = original[start..end]
        .replace(
            "derive(Print)",
            "derive(lrr_fixture_macros::CheckDeriveInput)",
        )
        .replace(
            "#[print_attr]",
            "#[lrr_fixture_macros::after_derive] #[repr(C)] #[lrr_derive_helper]",
        )
        .replace("cfg(false)", "cfg(any())")
        .replace("field: u8", "cold: u8");
    let adapted = if mode == "prefix" {
        format!(
            "{}\nprefix!();",
            adapted.replace("DeriveAttribute", "MarkerPrefix")
        )
    } else if matches!(mode, "forward" | "bang" | "sibling" | "chain") {
        let adapted = adapted
            .replace(
                "lrr_fixture_macros::after_derive]",
                if mode == "chain" {
                    "lrr_fixture_macros::after_derive(identity)] #[lrr_fixture_macros::after_helper]"
                } else if mode == "bang" {
                    "lrr_fixture_macros::after_derive(bang)]"
                } else {
                    "lrr_fixture_macros::after_derive(identity)]"
                },
            )
            .replace(
                "struct DeriveAttribute {\n    #[cfg(any())]\n    cold: u8,\n}",
                "struct DeriveAttribute(#[lrr_derive_helper] u8, #[cfg(any())] u16);",
            );
        assert!(adapted.contains("struct DeriveAttribute("));
        if mode == "sibling" {
            format!("{adapted}\n#[lrr_derive_helper] struct Outside;")
        } else {
            adapted
        }
    } else {
        adapted
    };
    procedural_source_worklist_case(
        &adapted,
        true,
        &format!("procedural-source-original-derive-append-following-attribute-{mode}"),
    );
}

fn original_derive_metadata_case(repeated: bool, label: &str) {
    let original = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-helper-configured.rs"
    );
    let adapted = original
        .replace(
            "#[macro_use]\nextern crate test_macros;",
            "use lrr_fixture_macros::CheckDeriveInput as Empty;",
        )
        .replace("cfg_attr(true,", "cfg_attr(all(),")
        .replace("empty_helper", "lrr_derive_helper");
    let adapted = if repeated {
        adapted.replace("#[derive(Empty)]", "#[derive(Empty, Empty)]")
    } else {
        adapted
    };
    let adapted = if label.starts_with("procedural-source-original-derive-execution")
        || label.starts_with("procedural-source-original-derive-append")
    {
        adapted.replace("#[derive(Empty)]", "#[derive(Empty)] #[repr(C)]")
    } else {
        adapted
    };
    let adapted =
        if label.ends_with("append-order") {
            adapted.replace(
            "#[derive(Empty)]",
            "#[derive(Empty, lrr_fixture_macros::Empty)] #[derive(lrr_fixture_macros::Empty)]",
        ).replace("struct S", "struct MarkerSourceAppend")
        } else if label.ends_with("append-nested") {
            adapted.replace("struct S", "struct MarkerSourceAppend")
        } else if label.ends_with("append-unknown-outer") {
            adapted.replace(
                "struct S",
                "#[not_a_helper] struct MarkerSourcePendingOuter",
            )
        } else {
            adapted
        };
    let adapted = if label.contains("append-future-") {
        let adapted = adapted.replace("use lrr_fixture_macros::CheckDeriveInput as Empty;", "use lrr_fixture_macros::CheckFutureHelpers as First; use lrr_fixture_macros::CheckDeriveInput as Empty;")
            .replace("#[derive(Empty)]", "#[derive(First)] #[derive(Empty)]");
        if label.ends_with("future-outer") {
            adapted
                .replace(
                    "#[derive(First)] #[derive(Empty)]",
                    "#[derive(First)] #[lrr_derive_helper] #[derive(Empty)]",
                )
                .replace("#[cfg_attr(all(), lrr_derive_helper)]\nstruct", "struct")
        } else if label.ends_with("future-qualified") {
            adapted.replace(
                "#[cfg_attr(all(), lrr_derive_helper)]\n    field",
                "#[cfg_attr(all(), qualified::lrr_derive_helper)]\n    field",
            )
        } else {
            adapted
        }
    } else {
        adapted
    };
    let adapted = if label == "procedural-source-original-derive-unknown-member" {
        adapted.replace("field: u8,", "#[unknown_helper] field: u8,")
    } else if label == "procedural-source-original-derive-qualified-member" {
        adapted.replace(
            "#[cfg_attr(all(), lrr_derive_helper)]\n    field",
            "#[cfg_attr(all(), qualified::lrr_derive_helper)]\n    field",
        )
    } else {
        adapted
    };
    if label.ends_with("qualified-member") {
        assert!(adapted.contains("qualified::lrr_derive_helper"));
    }
    procedural_source_worklist_case(&adapted, false, label);
}

fn procedural_source_worklist_case(text: &str, expect_seal: bool, label: &str) {
    procedural_source_worklist_case_with_impls(text, expect_seal, label, None)
}
fn procedural_source_worklist_case_with_impls(
    text: &str,
    expect_seal: bool,
    label: &str,
    expected_impls: Option<usize>,
) {
    use lrr::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        db::key::{OutlineKey, QueryKey},
        service::{
            expansion::{DemandContext, PhaseIdentity},
            item_macro_requests::ResolvedItemTarget,
            macro_session::macro_artifact_identity,
            run::MacroRun,
        },
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            manifest::BodyKey,
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    let prepared = prepared();
    let original_derives = label.starts_with("procedural-source-original-derive-");
    let repeated_original = label == "procedural-source-original-derive-repeated";
    let evidence = prepared.work.join(label);
    let attribute = label.starts_with("procedural-source-attribute");
    let nested = label.starts_with("procedural-source-nested")
        || label.starts_with("procedural-source-generated-attribute");
    let nested_ambiguity = label == "procedural-source-nested-ambiguity"
        || label == "procedural-source-attribute-chain-ambiguity";
    let derive_prefix = label == "procedural-source-attribute-chain-derive-prefix";
    let empty_attribute = label == "procedural-source-attribute-chain-empty-derive";
    let helper_bang = label == "procedural-source-attribute-chain-derive-helper-bang";
    let helper_module = label == "procedural-source-attribute-chain-derive-helper-module";
    let helper_chain = label == "procedural-source-attribute-chain-derive-helper-chain";
    let helper_identity = label == "procedural-source-attribute-chain-derive-helper-identity"
        || helper_bang
        || helper_module
        || helper_chain;
    let surround_empty = label == "procedural-source-attribute-chain-derive-surround-empty";
    let separate_groups = label == "procedural-source-attribute-chain-derive-later-groups";
    let later_attribute = label == "procedural-source-attribute-chain-derive-later-attribute"
        || separate_groups
        || helper_identity;
    let derive_append = label == "procedural-source-attribute-chain-derive-append"
        || derive_prefix
        || later_attribute
        || surround_empty;
    let derive_execution =
        label == "procedural-source-attribute-chain-derive-execution" || derive_append;
    let derive_scope =
        label == "procedural-source-attribute-chain-derive-helper-scope" || derive_execution;
    let derive_adoption =
        label == "procedural-source-attribute-chain-derive-helper-adoption" || derive_scope;
    let derive_helpers =
        label == "procedural-source-attribute-chain-derive-helper-declarations" || derive_adoption;
    let derive_pending = label == "procedural-source-attribute-chain-derive-pending"
        || derive_helpers
        || empty_attribute
        || original_derives;
    let modules = label.starts_with("procedural-source-modules");
    let modules_ambiguity = label == "procedural-source-modules-ambiguity";
    let mut manifest = if modules {
        service_fixture::prepare_sources(
            &evidence,
            &prepared.config.directory,
            &[
                ("lib.rs", text),
                ("child.rs", "pub mod grandchild;"),
                (
                    "child/grandchild.rs",
                    if modules_ambiguity {
                        "use lrr_fixture_macros::identity as selected; selected! { macro_rules! leaf { () => {pub fn made()->i64{7}} } pub use self::made as alias; leaf!(); use lrr_fixture_macros::session_counter as selected; }"
                    } else {
                        "use lrr_fixture_macros::identity as selected; selected! { macro_rules! leaf { () => {pub fn made()->i64{7}} } pub use self::made as alias; leaf!(); }"
                    },
                ),
            ],
        )
    } else {
        service_fixture::prepare(&evidence, &prepared.config.directory)
    };
    let plan = recorded_macro_plan(&evidence, &prepared.config.directory);
    if label == "procedural-source-prepared-server" {
        adopt_fixture_prepared_server(&mut manifest, &evidence);
        let store_path = evidence.join("repaired-store");
        fs::create_dir(&store_path).unwrap();
        let store = lrr::project::tool_manifest::ToolStore::open(&store_path).unwrap();
        let publication = lrr::project::tool_transaction::ToolTransaction::begin(&plan)
            .unwrap()
            .commit(&store, &manifest)
            .unwrap();
        assert_eq!(
            store.load(publication).unwrap().fingerprint(),
            manifest.fingerprint()
        );
        fs::write(
            evidence.join("repaired-manifest.json"),
            manifest.encode().unwrap(),
        )
        .unwrap();
    }
    let unit = *manifest.graph.roots.first().unwrap();
    let (producer, artifact) = manifest.artifacts.iter().next().unwrap();
    let mut context = DemandContext {
        request: Digest([91; 32]),
        session: Digest([92; 32]),
        unit,
        revision: 0,
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };

    let cfg = CfgSet::guest_default();
    let profile = ProfileIdentity::current();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
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
    .unwrap();
    let environment =
        lrr::expand::builtins::FrozenCompileEnvironment::new(unit, &manifest.compile_env[&unit])
            .unwrap();
    let inputs = inputs
        .with_compile_environment(Arc::new(environment.clone()))
        .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };

    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    let source_revision = db.source_revision();
    db.begin_expansion_epoch(1).unwrap();
    assert_eq!(db.source_revision(), source_revision);
    assert!(db.begin_expansion_epoch(0).is_err());
    assert!(db.begin_expansion_epoch(1).is_err());
    context.revision = db.source_revision();
    // A pure provider must refuse before attempting an invalid input capture.
    struct PureRunAttempt<'a> {
        context: DemandContext,
        manifest: &'a lrr::project::tool_manifest::ToolManifest,
        plan: &'a lrr::project::tool_manifest::InputPlan,
    }
    impl lrr::db::provider::Provider for PureRunAttempt<'_> {
        fn execute(
            &self,
            db: &mut lrr::db::memo::Database,
            _: &QueryKey,
        ) -> lrr::db::poll::QueryPoll<lrr::db::provider::Provided> {
            let error =
                match MacroRun::begin(db, self.context.clone(), self.manifest, self.plan, true) {
                    Err(error) => error,
                    Ok(_) => panic!("pure provider started macro coordinator"),
                };
            lrr::db::poll::QueryPoll::Ready(Err(error))
        }
    }
    let invalid_plan =
        lrr::project::tool_manifest::InputPlan::new(vec![], Default::default()).unwrap();
    let pure = PureRunAttempt {
        context: context.clone(),
        manifest: &manifest,
        plan: &invalid_plan,
    };
    let pure_key = QueryKey::Constant(fixture_key(
        Digest([201; 32]),
        vec![],
        Digest([202; 32]),
        ProfileIdentity::current(),
    ));
    let lrr::db::poll::QueryPoll::Ready(Err(error)) = db.fetch(&pure, pure_key) else {
        panic!("pure coordinator guard absent")
    };
    assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
    assert_eq!(db.source_revision(), source_revision);
    let mut denied = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, false).unwrap();
    assert_eq!(
        denied.catalog(&mut db, unit).unwrap_err().code,
        "LRR-E-MACRO-PERMISSION"
    );
    assert_eq!(denied.unit_invocations(unit), 0);
    assert!(denied.catalog(&mut db, unit).is_err());
    let mut run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
    use lrr::expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep};
    let catalog = run.catalog(&mut db, unit).unwrap();
    assert!(
        SourceDiscovery::new(
            vec![view.share()],
            view.outline().root_scope.clone(),
            lrr::source::manifest::Namespace::Value,
            Digest([97; 32]),
            1
        )
        .unwrap()
        .with_declarative(context.compile_environment, Digest([96; 32]))
        .unwrap()
        .with_procedural_catalog(catalog.clone())
        .is_err()
    );
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        if label.ends_with("append-complete") {
            lrr::source::manifest::Namespace::Type
        } else {
            lrr::source::manifest::Namespace::Value
        },
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_declarative(context.compile_environment, Digest([96; 32]))
    .unwrap()
    .with_procedural_catalog(catalog.clone())
    .unwrap()
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap();
    if derive_pending {
        discovery = discovery
            .with_compiler(Arc::new(
                lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                    &ProfileIdentity::current(),
                )
                .unwrap(),
            ))
            .unwrap();
    }
    if matches!(
        label,
        "procedural-source-original-derive-ambiguous" | "procedural-source-original-derive-private"
    ) {
        let error = discovery
            .poll()
            .err()
            .expect("settled invalid derive path stayed pending");
        assert_eq!(
            error.code,
            if label.ends_with("ambiguous") {
                "LRR-E-MACRO-AMBIGUOUS"
            } else {
                "LRR-E-MACRO-INACCESSIBLE"
            }
        );
        assert_eq!(run.unit_invocations(unit), 0);
        assert_eq!(discovery.accounted_expansion_count().unwrap(), 0);
        assert!(discovery.sealed_expansions().is_err());
        assert!(discovery.publish(&mut db).is_err());
        fs::write(evidence.join("source.rs"), text).unwrap();
        fs::write(
            evidence.join("original-derive-lookup.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "diagnostic":error.code,"native_invocations":run.unit_invocations(unit),
                "accounted_expansions":discovery.accounted_expansion_count().unwrap(),
                "published":false,"full_fixture_acceptance":false
            }))
            .unwrap(),
        )
        .unwrap();
        return;
    }
    if label.starts_with("procedural-source-original-derive-append") {
        let following_attribute = label.contains("following-attribute");
        let following_prefix = following_attribute && label.ends_with("prefix");
        let following_bang = following_attribute && label.ends_with("bang");
        let following_chain = following_attribute && label.ends_with("chain");
        let following_barrier =
            following_bang || (following_attribute && label.ends_with("sibling"));
        let ordered = label.ends_with("order");
        let future = label.contains("append-future-");
        let collision = label.ends_with("helper-collision");
        let expected_nested =
            usize::from(ordered || label.ends_with("nested") || following_prefix || following_bang);
        let expected_exports = if ordered {
            vec!["CheckDeriveInput", "Empty", "Empty"]
        } else if future {
            vec!["CheckFutureHelpers", "CheckDeriveInput"]
        } else if collision {
            vec!["CheckHelperCollision", "SameHelper"]
        } else {
            vec!["CheckDeriveInput"]
        };
        let mut executed = Vec::new();
        let mut appended = Vec::new();
        let mut declarations = Vec::new();
        let mut nested_outputs = Vec::new();
        let mut occurrence_inputs = Vec::new();
        let mut attributes = Vec::new();
        let mut pending_attribute = None;
        let mut cache = lrr::expand::cache::ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
        for _ in 0..32 {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::NeedAttributeSelection { selection } => {
                    assert!(following_attribute);
                    assert_eq!(executed.len(), expected_exports.len());
                    assert_eq!(appended.len(), executed.len());
                    assert!(attributes.is_empty());
                    let request = run
                        .prepare_attribute_selection(&mut db, &selection)
                        .unwrap();
                    assert_eq!(request.request().export, "after_derive");
                    discovery.bind_attribute_request(request.clone()).unwrap();
                    pending_attribute = Some(request);
                }
                SourceDiscoveryStep::NeedExpansion { request } => {
                    assert!(following_attribute);
                    if request.procedural_request().is_none() {
                        assert!(following_prefix);
                        let record = Arc::new(request.expand().unwrap());
                        nested_outputs.push(record.descriptor().fingerprint().hex());
                        discovery.admit_expansion(record).unwrap();
                        continue;
                    }
                    let selected = pending_attribute.take().unwrap();
                    assert_eq!(request.key(), selected.key());
                    let phase = PhaseIdentity {
                        frame: Digest([93; 32]),
                        body: BodyKey {
                            definition: Digest([94; 32]),
                            exact_body_bytes: Digest([95; 32]),
                        },
                        index: u32::try_from(executed.len() + attributes.len()).unwrap(),
                    };
                    if label.ends_with("attribute-stale") {
                        let updated = PreparedSourceInputs::refresh(
                            Some(&inputs),
                            vec![SourceUnitInput {
                                path: SourcePath::new("main.rs").unwrap(),
                                crate_key: unit,
                                module_path: vec![],
                                profile: ProfileIdentity::current(),
                                cfg: CfgSet::guest_default(),
                                blob: SourceBlob::new(text.replace("u8", "u16").into_bytes())
                                    .unwrap(),
                            }],
                            Default::default(),
                        )
                        .unwrap()
                        .with_compile_environment(Arc::new(environment.clone()))
                        .unwrap();
                        updated.publish(&mut db).unwrap();
                        let error = run
                            .fulfil_discovery(&mut db, &mut discovery, phase, &selected, &mut cache)
                            .err()
                            .unwrap();
                        assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
                        assert_eq!(run.unit_invocations(unit), 1);
                        assert_eq!(discovery.accounted_expansion_count().unwrap(), 1);
                        assert!(discovery.sealed_expansions().is_err());
                        assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
                        fs::write(evidence.join("original-following-attribute-stale.json"), serde_json::to_vec_pretty(&serde_json::json!({
                            "diagnostic":error.code, "native_invocations":1, "attribute_native_invocations":0,
                            "sealed":false,"published":false,"full_fixture_acceptance":false
                        })).unwrap()).unwrap();
                        return;
                    }
                    run.fulfil_discovery(&mut db, &mut discovery, phase, &selected, &mut cache)
                        .unwrap();
                    attributes.push(selected.key().fingerprint().hex());
                }
                SourceDiscoveryStep::NeedDeriveDeclaration { request } => {
                    let lrr::service::derive_declarations::DeriveDeclarationPoll::Ready(receipt) =
                        run.resolve_derive_declaration(&mut db, &request).unwrap()
                    else {
                        panic!("append helper metadata unavailable")
                    };
                    declarations.push(request.demand().hex());
                    discovery
                        .adopt_derive_declaration(&mut db, &request, Arc::new(receipt))
                        .unwrap();
                }
                SourceDiscoveryStep::NeedSourceDeriveSelection { selection } => {
                    if future && executed.is_empty() {
                        assert_eq!(
                            declarations.len(),
                            2,
                            "first derive executed before later member-helper declarations"
                        );
                    }
                    assert_eq!(
                        executed.len(),
                        appended.len(),
                        "later occurrence overtook append discovery"
                    );
                    if ordered && !executed.is_empty() {
                        assert_eq!(
                            nested_outputs.len(),
                            1,
                            "later derive overtook nested output discovery"
                        );
                    }
                    if !ordered && !future && !collision && !executed.is_empty() {
                        panic!("completed original derive was selected again");
                    }
                    let request = run
                        .prepare_source_derive_selection(&mut db, &selection)
                        .unwrap();
                    assert_eq!(request.request().export, expected_exports[executed.len()]);
                    occurrence_inputs.push((
                        selection.input().identity(),
                        selection.input().item().encode().unwrap(),
                    ));
                    discovery.bind_source_derive_request(request).unwrap();
                }
                SourceDiscoveryStep::NeedSourceDeriveExpansion { request } => {
                    let phase = PhaseIdentity {
                        frame: Digest([93; 32]),
                        body: BodyKey {
                            definition: Digest([94; 32]),
                            exact_body_bytes: Digest([95; 32]),
                        },
                        index: u32::try_from(executed.len()).unwrap(),
                    };
                    let output = run
                        .fulfil_source_derive(&mut db, &mut discovery, phase, &request, &mut cache)
                        .unwrap();
                    executed.push(request.key().fingerprint().hex());
                    fs::write(
                        evidence.join(format!("derive-{}.lrrtok", executed.len())),
                        output.tokens().encode().unwrap(),
                    )
                    .unwrap();
                }
                SourceDiscoveryStep::NeedSourceDeriveAppend { request, record } => {
                    let before = run.unit_invocations(unit);
                    if label.ends_with("append-stale") {
                        let updated = PreparedSourceInputs::refresh(
                            Some(&inputs),
                            vec![SourceUnitInput {
                                path: SourcePath::new("main.rs").unwrap(),
                                crate_key: unit,
                                module_path: vec![],
                                profile: ProfileIdentity::current(),
                                cfg: CfgSet::guest_default(),
                                blob: SourceBlob::new(
                                    text.replace("field: u8", "field: u16").into_bytes(),
                                )
                                .unwrap(),
                            }],
                            Default::default(),
                        )
                        .unwrap()
                        .with_compile_environment(Arc::new(environment.clone()))
                        .unwrap();
                        updated.publish(&mut db).unwrap();
                        let error = run
                            .integrate_source_derive_append(
                                &mut db,
                                &mut discovery,
                                &request,
                                record,
                            )
                            .unwrap_err();
                        assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
                        assert_eq!(run.unit_invocations(unit), before);
                        assert_eq!(discovery.accounted_expansion_count().unwrap(), 1);
                        assert!(discovery.sealed_expansions().is_err());
                        assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
                        fs::write(evidence.join("original-derive-append-stale.json"), serde_json::to_vec_pretty(&serde_json::json!({
                            "diagnostic":error.code,"native_invocations":before,"accounted_expansions":1,
                            "integrated":false,"published":false,"full_fixture_acceptance":false
                        })).unwrap()).unwrap();
                        return;
                    }
                    let forged = Arc::new(
                        lrr::expand::record::ExpansionRecord::from_wire(
                            request.key().clone(),
                            request.request().input.encode().unwrap(),
                        )
                        .unwrap(),
                    );
                    assert!(
                        run.integrate_source_derive_append(
                            &mut db,
                            &mut discovery,
                            &request,
                            forged
                        )
                        .is_err()
                    );
                    let mut foreign =
                        MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true)
                            .unwrap()
                            .with_expansion_budget(run.expansion_budget())
                            .unwrap();
                    assert!(
                        foreign
                            .integrate_source_derive_append(
                                &mut db,
                                &mut discovery,
                                &request,
                                record.clone()
                            )
                            .is_err()
                    );
                    assert_eq!(foreign.unit_invocations(unit), 0);
                    run.integrate_source_derive_append(
                        &mut db,
                        &mut discovery,
                        &request,
                        record.clone(),
                    )
                    .unwrap();
                    assert!(
                        run.integrate_source_derive_append(
                            &mut db,
                            &mut discovery,
                            &request,
                            record.clone()
                        )
                        .is_err()
                    );
                    assert_eq!(run.unit_invocations(unit), before);
                    appended.push(record.descriptor().fingerprint().hex());
                }
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                    if let Some(invocation) = request.procedural_request() {
                        assert!(following_chain);
                        assert_eq!(invocation.export, "after_helper");
                        assert_eq!(attributes.len(), 1);
                        let phase = PhaseIdentity {
                            frame: Digest([93; 32]),
                            body: BodyKey {
                                definition: Digest([94; 32]),
                                exact_body_bytes: Digest([95; 32]),
                            },
                            index: u32::try_from(executed.len() + attributes.len()).unwrap(),
                        };
                        run.fulfil_generated_discovery(
                            &mut db,
                            &mut discovery,
                            phase,
                            &request,
                            &mut cache,
                        )
                        .unwrap();
                        attributes.push(request.key().fingerprint().hex());
                        continue;
                    }
                    assert_eq!(executed.len(), 1);
                    assert_eq!(run.unit_invocations(unit), 1 + u64::from(following_bang));
                    let record = Arc::new(request.expand().unwrap());
                    let items = lrr::expand::generated_items::GeneratedItems::admit(
                        request.key(),
                        record.clone(),
                        1,
                    )
                    .unwrap();
                    assert!(items.items().iter().any(|h| {
                        h.name.as_ref().is_some_and(|name| {
                            name.text
                                == if following_bang {
                                    "Leak"
                                } else {
                                    "source_append_child"
                                }
                        })
                    }));
                    nested_outputs.push(record.descriptor().fingerprint().hex());
                    discovery.admit_expansion(record).unwrap();
                }
                SourceDiscoveryStep::Incomplete {
                    diagnostic,
                    blocked,
                } => {
                    assert!(
                        label.ends_with("unknown-outer")
                            || label.ends_with("future-outer")
                            || label.ends_with("future-qualified")
                            || following_barrier,
                        "completed original derives did not seal: {diagnostic:?}; {blocked:?}"
                    );
                    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
                    assert_eq!(
                        executed.len(),
                        if label.ends_with("future-qualified") {
                            0
                        } else if label.ends_with("future-outer") {
                            1
                        } else {
                            expected_exports.len()
                        }
                    );
                    assert_eq!(appended.len(), executed.len());
                    assert_eq!(
                        run.unit_invocations(unit),
                        u64::try_from(executed.len() + attributes.len()).unwrap()
                    );
                    assert_eq!(nested_outputs.len(), expected_nested);
                    assert_eq!(
                        discovery.accounted_expansion_count().unwrap(),
                        executed.len() + nested_outputs.len() + attributes.len()
                    );
                    if ordered {
                        assert_ne!(occurrence_inputs[0].0, occurrence_inputs[1].0);
                        assert_eq!(occurrence_inputs[0].1, occurrence_inputs[1].1);
                        assert_ne!(occurrence_inputs[1].1, occurrence_inputs[2].1);
                    }
                    assert!(discovery.sealed_expansions().is_err());
                    assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
                    fs::write(evidence.join("source.rs"), text).unwrap();
                    fs::write(evidence.join("original-derive-append.json"), serde_json::to_vec_pretty(&serde_json::json!({
                        "executed":executed,"appended":appended,"declarations":declarations,"nested_outputs":nested_outputs,"following_attributes":attributes,
                        "native_invocations":run.unit_invocations(unit),"accounted_expansions":discovery.accounted_expansion_count().unwrap(),
                        "original_producer_pending":true,"sealed":false,"published":false,"full_fixture_acceptance":false,
                        "mode":if ordered {"ordered_occurrences_and_groups"} else {"nested_generated_macro"}
                    })).unwrap()).unwrap();
                    return;
                }
                SourceDiscoveryStep::Sealed(seal) => {
                    assert!(
                        !following_barrier,
                        "helper escaped its inheritance boundary"
                    );
                    assert!(
                        !label.ends_with("unknown-outer")
                            && !label.ends_with("future-outer")
                            && !label.ends_with("future-qualified"),
                        "unresolved outer annotation sealed"
                    );
                    assert_eq!(executed.len(), expected_exports.len());
                    assert_eq!(appended.len(), executed.len());
                    assert_eq!(nested_outputs.len(), expected_nested);
                    assert_eq!(
                        attributes.len(),
                        usize::from(following_attribute) + usize::from(following_chain)
                    );
                    assert_eq!(
                        run.unit_invocations(unit),
                        u64::try_from(executed.len() + attributes.len()).unwrap()
                    );
                    assert_eq!(
                        discovery.accounted_expansion_count().unwrap(),
                        executed.len() + expected_nested + attributes.len()
                    );
                    assert_eq!(
                        discovery.sealed_expansions().unwrap().len(),
                        executed.len() + expected_nested + attributes.len()
                    );
                    if ordered {
                        assert_ne!(occurrence_inputs[0].0, occurrence_inputs[1].0);
                        assert_eq!(occurrence_inputs[0].1, occurrence_inputs[1].1);
                        assert_ne!(occurrence_inputs[1].1, occurrence_inputs[2].1);
                    }
                    if label.ends_with("append-complete") {
                        let expected = view
                            .outline()
                            .items
                            .iter()
                            .find(|item| item.def_key.declared_name == "S")
                            .unwrap()
                            .def_key
                            .fingerprint();
                        assert!(
                            matches!(discovery.lookup("S").unwrap(), lrr::frontend::names::NameLookup::Defined(actual) if actual.identity == expected)
                        );
                    } else {
                        assert!(matches!(
                            discovery
                                .lookup(if collision {
                                    "COLLIDING_HELPER_SEEN"
                                } else {
                                    "DERIVE_INPUT_ATTRIBUTES"
                                })
                                .unwrap(),
                            lrr::frontend::names::NameLookup::Defined(_)
                        ));
                        if expected_nested != 0 && !following_prefix {
                            assert!(matches!(
                                discovery.lookup("source_append_child").unwrap(),
                                lrr::frontend::names::NameLookup::Defined(_)
                            ));
                        }
                    }
                    if future {
                        assert!(matches!(
                            discovery.lookup("FUTURE_HELPERS_SEEN").unwrap(),
                            lrr::frontend::names::NameLookup::Defined(_)
                        ));
                    }
                    if following_attribute {
                        if label.ends_with("attribute-forward") || following_chain {
                            assert!(matches!(
                                discovery.lookup("DeriveAttribute").unwrap(),
                                lrr::frontend::names::NameLookup::Defined(_)
                            ));
                        } else {
                            assert!(matches!(
                                discovery.lookup("AfterDerived").unwrap(),
                                lrr::frontend::names::NameLookup::Defined(_)
                            ));
                            assert!(matches!(
                                discovery.lookup("DeriveAttribute").unwrap(),
                                lrr::frontend::names::NameLookup::Absent
                            ));
                        }
                    }
                    if following_prefix {
                        assert!(matches!(
                            discovery.lookup("FIRST_PREFIX").unwrap(),
                            lrr::frontend::names::NameLookup::Defined(_)
                        ));
                        assert!(matches!(
                            discovery.lookup("MarkerPrefix").unwrap(),
                            lrr::frontend::names::NameLookup::Absent
                        ));
                    }
                    if label.ends_with("publication-stale") {
                        let updated = PreparedSourceInputs::refresh(
                            Some(&inputs),
                            vec![SourceUnitInput {
                                path: SourcePath::new("main.rs").unwrap(),
                                crate_key: unit,
                                module_path: vec![],
                                profile: ProfileIdentity::current(),
                                cfg: CfgSet::guest_default(),
                                blob: SourceBlob::new(
                                    text.replace("field: u8", "field: u16").into_bytes(),
                                )
                                .unwrap(),
                            }],
                            Default::default(),
                        )
                        .unwrap()
                        .with_compile_environment(Arc::new(environment.clone()))
                        .unwrap();
                        updated.publish(&mut db).unwrap();
                        let error = run
                            .publish_discovery(&mut db, &mut discovery)
                            .err()
                            .unwrap();
                        assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
                        assert_eq!(run.unit_invocations(unit), 1);
                        fs::write(evidence.join("original-derive-publication-stale.json"), serde_json::to_vec_pretty(&serde_json::json!({
                            "diagnostic":error.code,"native_invocations":1,"sealed_before_edit":true,
                            "published":false,"full_fixture_acceptance":false
                        })).unwrap()).unwrap();
                        return;
                    }
                    assert!(
                        discovery.publish(&mut db).is_err(),
                        "native output published without owning run"
                    );
                    run.publish_discovery(&mut db, &mut discovery).unwrap();
                    if label.ends_with("append-complete") {
                        let key = QueryKey::SealedName(Box::new(lrr::db::key::SealedNameKey {
                            seal: discovery.key().clone(),
                            bucket: lrr::db::key::BucketKey {
                                scope: view.outline().root_scope.clone(),
                                namespace: lrr::source::manifest::Namespace::Type,
                                name: "S".into(),
                            },
                        }));
                        let lrr::db::poll::QueryPoll::Ready(Ok(outcome)) =
                            db.fetch(&lrr::db::provider::SyntaxProvider, key.clone())
                        else {
                            panic!("published original type query did not become ready")
                        };
                        assert!(matches!(
                            outcome.value.as_ref(),
                            lrr::db::value::QueryValue::SealedName {
                                lookup: lrr::frontend::names::NameLookup::Defined(_),
                                ..
                            }
                        ));
                        let updated = PreparedSourceInputs::refresh(
                            Some(&inputs),
                            vec![SourceUnitInput {
                                path: SourcePath::new("main.rs").unwrap(),
                                crate_key: unit,
                                module_path: vec![],
                                profile: ProfileIdentity::current(),
                                cfg: CfgSet::guest_default(),
                                blob: SourceBlob::new(
                                    text.replace("field: u8", "field: u16").into_bytes(),
                                )
                                .unwrap(),
                            }],
                            Default::default(),
                        )
                        .unwrap()
                        .with_compile_environment(Arc::new(environment.clone()))
                        .unwrap();
                        updated.publish(&mut db).unwrap();
                        assert!(
                            matches!(
                                db.fetch(&lrr::db::provider::SyntaxProvider, key),
                                lrr::db::poll::QueryPoll::NeedInput(_)
                            ),
                            "stale type certificate was reused after source edit"
                        );
                    }
                    fs::write(evidence.join("source.rs"), text).unwrap();
                    fs::write(evidence.join("original-derive-completion.json"), serde_json::to_vec_pretty(&serde_json::json!({
                        "executed":executed,"appended":appended,"declarations":declarations,"nested_outputs":nested_outputs,"following_attributes":attributes,
                        "native_invocations":run.unit_invocations(unit),"accounted_expansions":discovery.accounted_expansion_count().unwrap(),
                        "original_producer_pending":false,"sealed":true,"published":true,"full_fixture_acceptance":false,
                        "seal":seal.fingerprint().hex(),"mode":label,"namespace":if label.ends_with("append-complete") {"type"} else {"value"},
                        "published_type_query_and_edit_invalidation":label.ends_with("append-complete")
                    })).unwrap()).unwrap();
                    return;
                }
                _ => panic!("unexpected discovery phase in original append regression"),
            }
            assert!(discovery.sealed_expansions().is_err());
        }
        panic!("original append discovery did not reach its pending final transformation");
    }
    let first = discovery.poll().unwrap();
    assert_eq!(run.unit_invocations(unit), 0);
    assert!(discovery.publish(&mut db).is_err());
    if original_derives {
        let SourceDiscoveryStep::NeedDeriveDeclaration { request } = first else {
            panic!("original source derive did not suspend for exact metadata")
        };
        let SourceDiscoveryStep::NeedDeriveDeclaration { request: repeated } =
            discovery.poll().unwrap()
        else {
            panic!("original source metadata demand was not retained")
        };
        assert_eq!(request.fingerprint(), repeated.fingerprint());
        let mut metadata_denied =
            MacroRun::begin(&mut db, context.clone(), &manifest, &plan, false).unwrap();
        assert_eq!(
            metadata_denied
                .resolve_derive_declaration(&mut db, &request)
                .err()
                .unwrap()
                .code,
            "LRR-E-MACRO-PERMISSION"
        );
        assert_eq!(metadata_denied.unit_invocations(unit), 0);
        let lrr::service::derive_declarations::DeriveDeclarationPoll::Ready(receipt) =
            run.resolve_derive_declaration(&mut db, &request).unwrap()
        else {
            panic!("original source metadata receipt unavailable")
        };
        assert_eq!(receipt.helpers(), ["lrr_derive_helper"]);
        let observed_helpers = receipt.helpers().to_vec();
        let receipt = Arc::new(receipt);
        assert_eq!(run.unit_invocations(unit), 0);
        discovery
            .adopt_derive_declaration(&mut db, &request, receipt.clone())
            .unwrap();
        let mut demands = vec![request.demand().hex()];
        if repeated_original {
            let SourceDiscoveryStep::NeedDeriveDeclaration { request: second } =
                discovery.poll().unwrap()
            else {
                panic!("second original occurrence did not suspend")
            };
            assert_ne!(second.demand(), request.demand());
            assert_ne!(second.occurrence(), request.occurrence());
            assert!(
                discovery
                    .adopt_derive_declaration(&mut db, &second, receipt.clone())
                    .is_err()
            );
            let lrr::service::derive_declarations::DeriveDeclarationPoll::Ready(other) =
                run.resolve_derive_declaration(&mut db, &second).unwrap()
            else {
                panic!("second original metadata unavailable")
            };
            assert_eq!(other.helpers(), observed_helpers);
            demands.push(second.demand().hex());
            discovery
                .adopt_derive_declaration(&mut db, &second, Arc::new(other))
                .unwrap();
        }
        if matches!(
            label,
            "procedural-source-original-derive-unknown-member"
                | "procedural-source-original-derive-qualified-member"
        ) {
            let SourceDiscoveryStep::Incomplete { diagnostic, .. } = discovery.poll().unwrap()
            else {
                panic!("unresolved member attribute received an execution selection")
            };
            assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
            assert_eq!(run.unit_invocations(unit), 0);
            assert_eq!(discovery.accounted_expansion_count().unwrap(), 0);
            assert!(discovery.sealed_expansions().is_err());
            assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
            fs::write(evidence.join("source.rs"), text).unwrap();
            fs::write(evidence.join("original-derive-member-refusal.json"),serde_json::to_vec_pretty(&serde_json::json!({
                "diagnostic":diagnostic.code,"native_invocations":run.unit_invocations(unit),"accounted_expansions":discovery.accounted_expansion_count().unwrap(),
                "metadata_demands":demands,"helpers":observed_helpers,"published":false,"full_fixture_acceptance":false,"member_case":label
            })).unwrap()).unwrap();
            return;
        }
        let SourceDiscoveryStep::NeedSourceDeriveSelection { selection } =
            discovery.poll().unwrap()
        else {
            panic!("original derive metadata did not produce a typed execution selection");
        };
        let SourceDiscoveryStep::NeedSourceDeriveSelection { selection: again } =
            discovery.poll().unwrap()
        else {
            panic!("source derive selection was not retained")
        };
        assert_eq!(selection.fingerprint(), again.fingerprint());
        let mut execution_denied =
            MacroRun::begin(&mut db, context.clone(), &manifest, &plan, false).unwrap();
        assert_eq!(
            execution_denied
                .prepare_source_derive_selection(&mut db, &selection)
                .err()
                .unwrap()
                .code,
            "LRR-E-MACRO-PERMISSION"
        );
        assert_eq!(execution_denied.unit_invocations(unit), 0);
        let selected = run
            .prepare_source_derive_selection(&mut db, &selection)
            .unwrap();
        assert_eq!(selected.request().form, ProcForm::Derive);
        assert_eq!(selected.key().invocation, selection.input().identity());
        assert!(discovery.bind_attribute_request(selected.clone()).is_err());
        discovery
            .bind_source_derive_request(selected.clone())
            .unwrap();
        assert!(
            discovery
                .bind_source_derive_request(selected.clone())
                .is_err()
        );
        let SourceDiscoveryStep::NeedSourceDeriveExpansion { request: bound } =
            discovery.poll().unwrap()
        else {
            panic!("bound source derive execution demand absent")
        };
        assert_eq!(bound.key(), selected.key());
        if label == "procedural-source-original-derive-execution-stale" {
            let updated = PreparedSourceInputs::refresh(
                Some(&inputs),
                vec![SourceUnitInput {
                    path: SourcePath::new("main.rs").unwrap(),
                    crate_key: unit,
                    module_path: vec![],
                    profile: ProfileIdentity::current(),
                    cfg: CfgSet::guest_default(),
                    blob: SourceBlob::new(text.replace("field: u8", "field: u16").into_bytes())
                        .unwrap(),
                }],
                Default::default(),
            )
            .unwrap()
            .with_compile_environment(Arc::new(environment.clone()))
            .unwrap();
            updated.publish(&mut db).unwrap();
            let phase = PhaseIdentity {
                frame: Digest([93; 32]),
                body: BodyKey {
                    definition: Digest([94; 32]),
                    exact_body_bytes: Digest([95; 32]),
                },
                index: 0,
            };
            let mut cache =
                lrr::expand::cache::ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
            let error = run
                .fulfil_source_derive(&mut db, &mut discovery, phase, &selected, &mut cache)
                .err()
                .unwrap();
            assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
            assert_eq!(
                error.message,
                "macro run is closed or its frozen revision/epoch changed"
            );
            assert_eq!(run.unit_invocations(unit), 0);
            assert_eq!(discovery.accounted_expansion_count().unwrap(), 0);
            assert!(discovery.sealed_expansions().is_err());
            fs::write(evidence.join("source.rs"), text).unwrap();
            fs::write(evidence.join("original-derive-stale.json"),serde_json::to_vec_pretty(&serde_json::json!({
                "diagnostic":error.code,"native_invocations":run.unit_invocations(unit),"accounted_expansions":discovery.accounted_expansion_count().unwrap(),
                "published":false,"full_fixture_acceptance":false,"request":selected.key().fingerprint().hex(),"source_revision":db.source_revision()
            })).unwrap()).unwrap();
            return;
        }
        if label == "procedural-source-original-derive-execution" {
            let phase = PhaseIdentity {
                frame: Digest([93; 32]),
                body: BodyKey {
                    definition: Digest([94; 32]),
                    exact_body_bytes: Digest([95; 32]),
                },
                index: 0,
            };
            let mut cache =
                lrr::expand::cache::ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
            let mut foreign =
                MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
            assert_eq!(
                foreign
                    .fulfil_source_derive(
                        &mut db,
                        &mut discovery,
                        phase.clone(),
                        &selected,
                        &mut cache
                    )
                    .err()
                    .unwrap()
                    .code,
                "LRR-E-CATALOG-INCOMPLETE"
            );
            assert_eq!(foreign.unit_invocations(unit), 0);
            let output = run
                .fulfil_source_derive(&mut db, &mut discovery, phase, &selected, &mut cache)
                .unwrap();
            let tokens = TokenForest::decode(&output.tokens().encode().unwrap()).unwrap();
            assert!(identifiers(&tokens).contains(&"DERIVE_INPUT_ATTRIBUTES".into()));
            assert_eq!(literals(&tokens), ["2_usize"]);
            assert_eq!(run.unit_invocations(unit), 1);
            assert_eq!(discovery.accounted_expansion_count().unwrap(), 1);
            let SourceDiscoveryStep::NeedSourceDeriveAppend {
                request: append,
                record,
            } = discovery.poll().unwrap()
            else {
                panic!("source derive output did not suspend for append")
            };
            assert_eq!(append.key(), selected.key());
            assert_eq!(record.key(), selected.key());
            let SourceDiscoveryStep::NeedSourceDeriveAppend { record: again, .. } =
                discovery.poll().unwrap()
            else {
                panic!("source append demand was not retained")
            };
            assert_eq!(
                record.descriptor().fingerprint(),
                again.descriptor().fingerprint()
            );
            assert_eq!(run.unit_invocations(unit), 1);
            assert!(discovery.sealed_expansions().is_err());
            assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
            fs::write(evidence.join("source.rs"), text).unwrap();
            fs::write(
                evidence.join("source-derive-input.lrrtok"),
                selection.input().item().encode().unwrap(),
            )
            .unwrap();
            fs::write(
                evidence.join("source-derive-output.lrrtok"),
                output.tokens().encode().unwrap(),
            )
            .unwrap();
            fs::write(evidence.join("original-derive-execution.json"),serde_json::to_vec_pretty(&serde_json::json!({
                "native_invocations":run.unit_invocations(unit),"accounted_expansions":discovery.accounted_expansion_count().unwrap(),
                "selection":selection.fingerprint().hex(),"occurrence":selection.input().identity().hex(),"request":selected.key().fingerprint().hex(),
                "descriptor":record.descriptor().fingerprint().hex(),"helpers":observed_helpers,"demands":demands,
                "output_literals":literals(&tokens),"waiting_for_append":true,"published":false,"full_fixture_acceptance":false,
                "upstream_case":"tests/ui/proc-macro/derive-helper-configured.rs","adaptations":["fixture CheckDeriveInput export aliased as Empty","cfg(true) to cfg(all())","fixture helper name","repr(C) for controlled input assertion"]
            })).unwrap()).unwrap();
            return;
        }
        assert_eq!(run.unit_invocations(unit), 0);
        assert_eq!(discovery.accounted_expansion_count().unwrap(), 0);
        assert!(discovery.sealed_expansions().is_err());
        assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
        fs::write(evidence.join("source.rs"), text).unwrap();
        fs::write(evidence.join("original-derive-metadata.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "native_invocations":run.unit_invocations(unit), "accounted_expansions":discovery.accounted_expansion_count().unwrap(),
            "request":request.fingerprint().hex(), "demands":demands,"helpers":observed_helpers, "incomplete":true,
            "published":false,"full_fixture_acceptance":false,
            "upstream_case":"tests/ui/proc-macro/derive-helper-configured.rs",
            "adaptations":["fixture CheckDeriveInput export aliased as Empty", "cfg(true) to cfg(all())", "fixture helper name"],"repeated_occurrences":repeated_original
        })).unwrap()).unwrap();
        return;
    }
    let selected = match first {
        SourceDiscoveryStep::NeedAttributeSelection { selection: pending } => {
            assert!(attribute);
            let selected = run.prepare_attribute_selection(&mut db, &pending).unwrap();
            assert_eq!(selected.request().form, ProcForm::Attribute);
            discovery.bind_attribute_request(selected.clone()).unwrap();
            selected
        }
        SourceDiscoveryStep::NeedProceduralSelection { selection: pending } => {
            let registry =
                TokenForest::decode(&pending.input().arguments().encode().unwrap()).unwrap();
            let target = ResolvedItemTarget {
                artifact: Key(macro_artifact_identity(*producer, artifact).0),
                export: "identity".into(),
                scope: Key(pending.input().scope().fingerprint().0),
                parent_origin: pending.input().parent_origin(),
                registry: &registry,
                definition_site: pending.input().call_site(),
                call_site: pending.input().call_site(),
                mixed_site: pending.input().call_site(),
                sources: vec![],
            };
            let selected = run
                .prepare_source_selection(&mut db, &pending, target)
                .unwrap();
            discovery.bind_procedural_request(selected.clone()).unwrap();
            selected
        }
        _ => panic!("procedural producer must suspend discovery"),
    };
    let SourceDiscoveryStep::NeedExpansion { request } = discovery.poll().unwrap() else {
        panic!("exact bound request absent")
    };
    assert!(request.procedural_request().is_some());
    assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-PERMISSION");
    let phase = PhaseIdentity {
        frame: Digest([93; 32]),
        body: BodyKey {
            definition: Digest([94; 32]),
            exact_body_bytes: Digest([95; 32]),
        },
        index: 0,
    };
    let mut cache = lrr::expand::cache::ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut wrong_run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
    assert_eq!(
        wrong_run
            .fulfil_discovery(
                &mut db,
                &mut discovery,
                phase.clone(),
                &selected,
                &mut cache
            )
            .err()
            .unwrap()
            .code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    assert_eq!(wrong_run.unit_invocations(unit), 0);
    assert!(wrong_run.catalog(&mut db, unit).is_err());
    assert_eq!(run.unit_invocations(unit), 0);
    let mut forged_config = prepared.config.clone();
    forged_config.unit = Key(unit.0);
    let forged_tokens = invocation(
        &forged_config,
        99,
        "identity",
        ProcForm::FunctionLike,
        ProcGrammar::Items,
        "pub fn substituted(){}",
    )
    .input;
    let forged = Arc::new(
        lrr::expand::record::ExpansionRecord::from_wire(
            selected.key().clone(),
            forged_tokens.encode().unwrap(),
        )
        .unwrap(),
    );
    assert!(discovery.admit_expansion(forged.clone()).is_err());
    assert!(
        discovery
            .admit_procedural_expansion(&mut db, forged.clone())
            .is_err()
    );
    let admission = run.fulfil_discovery(
        &mut db,
        &mut discovery,
        phase.clone(),
        &selected,
        &mut cache,
    );
    let adopted = db
        .input(&lrr::db::dependency::InputKey::ExpansionRecord(
            selected.key().fingerprint(),
        ))
        .unwrap();
    let lrr::db::dependency::InputValue::ExpansionRecord(record) = adopted.as_ref() else {
        panic!("adopted native output absent")
    };
    let record = Arc::new(record.clone());
    assert!(
        discovery
            .admit_procedural_expansion(&mut db, forged)
            .is_err()
    );
    if !expect_seal {
        let diagnostic = match admission {
            Err(error) => error,
            Ok(_) => match discovery.poll() {
                Err(error) => error,
                Ok(SourceDiscoveryStep::Incomplete { diagnostic, .. }) => diagnostic,
                _ => panic!(
                    "changed procedural selection must not seal or request another execution"
                ),
            },
        };
        assert!(diagnostic.code == "LRR-E-MACRO-AMBIGUOUS", "{diagnostic:?}");
        assert!(discovery.publish(&mut db).is_err());
        assert_eq!(run.unit_invocations(unit), 1);
        fs::write(evidence.join("source.rs"), text).unwrap();
        fs::write(evidence.join("refusal.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M013","platform":"aarch64-apple-darwin","rust":"1.99.0","diagnostic":diagnostic.code,"native_invocations":run.unit_invocations(unit),"sealed":false,"published":false,"release_complete":false})).unwrap()).unwrap();
        return;
    }
    admission.unwrap();

    let mut expected_derive_demand = None;
    let mut expected_derive_occurrence = None;
    let mut declaration_requests: Vec<
        lrr::frontend::generated_derive_exports::DeriveDeclarationRequest,
    > = Vec::new();
    let mut adopted_declarations = 0usize;
    let mut executed_derives = Vec::new();
    let mut derive_events = Vec::new();
    let tool_input_plan = &plan;
    let mut generated = 0;
    let mut captures = BTreeMap::new();
    let authorities = lrr::source::include_paths::IncludeAuthorities::new(
        unit,
        manifest.inputs.fingerprint(),
        [lrr::source::include_paths::IncludeRoot {
            identity: Digest([203; 32]),
            path: evidence.join("app/src"),
            kind: lrr::source::include_paths::IncludeRootKind::Source,
        }],
    )
    .unwrap();
    loop {
        let step = match discovery.poll() {
            Ok(step) => step,
            Err(error) if nested_ambiguity => {
                assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
                record_procedural_module_refusal(
                    &evidence,
                    &mut db,
                    &discovery,
                    run.unit_invocations(unit),
                    &error,
                );
                return;
            }
            Err(error) => panic!("procedural discovery poll failed: {error:?}"),
        };
        match step {
            SourceDiscoveryStep::NeedDeriveAppend { request, record } => {
                assert!(derive_execution);
                if !derive_append {
                    discovery.defer_derive_append(&request).unwrap();
                    continue;
                }
                let before = run.unit_invocations(unit);
                assert!(discovery.publish(&mut db).is_err());
                let forged = Arc::new(
                    lrr::expand::record::ExpansionRecord::from_wire(
                        request.key().clone(),
                        request
                            .procedural_request()
                            .unwrap()
                            .input
                            .encode()
                            .unwrap(),
                    )
                    .unwrap(),
                );
                assert!(
                    run.integrate_derive_append(&mut db, &mut discovery, &request, forged)
                        .is_err()
                );
                let mut foreign = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true)
                    .unwrap()
                    .with_expansion_budget(run.expansion_budget())
                    .unwrap();
                assert!(
                    foreign
                        .integrate_derive_append(&mut db, &mut discovery, &request, record.clone())
                        .is_err()
                );
                assert_eq!(foreign.unit_invocations(unit), 0);
                run.integrate_derive_append(&mut db, &mut discovery, &request, record)
                    .unwrap();
                derive_events.push(format!(
                    "append:{}",
                    request.procedural_request().unwrap().export
                ));
                assert_eq!(run.unit_invocations(unit), before);
            }
            SourceDiscoveryStep::Sealed(_) if empty_attribute => {
                assert_eq!(run.unit_invocations(unit), 3);
                assert_eq!(discovery.accounted_expansion_count().unwrap(), 3);
                assert_eq!(discovery.retained_derive_output_count().unwrap(), 0);
                assert_eq!(discovery.derive_declaration_count().unwrap(), 0);
                assert!(matches!(
                    discovery.lookup("Marker").unwrap(),
                    lrr::frontend::names::NameLookup::Absent
                ));
                assert!(matches!(
                    discovery.lookup("AfterEmpty").unwrap(),
                    lrr::frontend::names::NameLookup::Defined(_)
                ));
                assert!(discovery.publish(&mut db).is_err());
                run.publish_discovery(&mut db, &mut discovery).unwrap();
                fs::write(evidence.join("empty-derive.json"),serde_json::to_vec_pretty(&serde_json::json!({"native_invocations":3,"native_derives":0,"helper_declarations":0,"accounted_expansions":3,"replacement_present":true,"original_absent":true,"attribute_input_retains_member_cfg":true,"sealed":true,"published":true,"full_fixture_acceptance":false})).unwrap()).unwrap();
                return;
            }
            SourceDiscoveryStep::Sealed(_) if derive_append => {
                assert_eq!(
                    run.unit_invocations(unit),
                    if helper_chain {
                        6
                    } else if later_attribute {
                        5
                    } else {
                        4
                    }
                );
                assert_eq!(
                    discovery.accounted_expansion_count().unwrap(),
                    if derive_prefix || helper_chain {
                        6
                    } else if later_attribute {
                        5
                    } else {
                        4
                    }
                );
                assert_eq!(discovery.retained_derive_output_count().unwrap(), 2);
                assert!(matches!(
                    discovery
                        .lookup(if helper_identity {
                            "Marker"
                        } else if later_attribute {
                            "AfterDerived"
                        } else if derive_prefix {
                            "MarkerPrefix"
                        } else {
                            "Marker"
                        })
                        .unwrap(),
                    lrr::frontend::names::NameLookup::Defined(_)
                ));
                assert!(matches!(
                    discovery.lookup("DERIVE_INPUT_ATTRIBUTES").unwrap(),
                    lrr::frontend::names::NameLookup::Defined(_)
                ));
                assert!(matches!(
                    discovery.lookup("discarded").unwrap(),
                    lrr::frontend::names::NameLookup::Absent
                ));
                if derive_prefix {
                    for name in ["FIRST_PREFIX", "SECOND_PREFIX"] {
                        assert!(
                            matches!(
                                discovery.lookup(name).unwrap(),
                                lrr::frontend::names::NameLookup::Defined(_)
                            ),
                            "missing {name}"
                        );
                    }
                }
                if later_attribute && !helper_identity {
                    assert!(matches!(
                        discovery.lookup("Marker").unwrap(),
                        lrr::frontend::names::NameLookup::Absent
                    ));
                }
                assert!(discovery.publish(&mut db).is_err());
                run.publish_discovery(&mut db, &mut discovery).unwrap();
                assert!(matches!(
                    db.fetch(
                        &lrr::db::provider::SyntaxProvider,
                        QueryKey::DiscoverySeal(discovery.key().clone())
                    ),
                    lrr::db::poll::QueryPoll::Ready(Ok(_))
                ));
                if later_attribute {
                    let expected = if separate_groups {
                        [
                            "derive:CheckDeriveInput",
                            "append:CheckDeriveInput",
                            "derive:Empty",
                            "append:Empty",
                            "attribute:after_derive",
                        ]
                    } else {
                        [
                            "derive:CheckDeriveInput",
                            "derive:Empty",
                            "append:CheckDeriveInput",
                            "append:Empty",
                            "attribute:after_derive",
                        ]
                    };
                    if helper_chain {
                        let mut chained = expected.to_vec();
                        chained.push("attribute:after_helper");
                        assert_eq!(derive_events, chained);
                    } else {
                        assert_eq!(derive_events, expected);
                    }
                    if helper_identity {
                        assert!(!helper_bang, "bang helper barrier must remain incomplete");
                        fs::write(evidence.join("helper-inheritance.json"),serde_json::to_vec_pretty(&serde_json::json!({"native_invocations":if helper_chain {6} else {5},"retained_outputs":2,"marker_present":true,"constant_present":true,"helpers_preserved":true,"module_reexport":helper_module,"chained_attributes":helper_chain,"events":derive_events,"sealed":true,"published":true,"full_fixture_acceptance":false})).unwrap()).unwrap();
                    } else {
                        fs::write(evidence.join("derive-later-attribute.json"),serde_json::to_vec_pretty(&serde_json::json!({"native_invocations":5,"retained_outputs":2,"annotation_groups":if separate_groups {2} else {1},"events":derive_events,"helper_tokens_preserved":true,"original_item_absent":true,"replacement_present":true,"appended_const_present":true,"derive_input_configured":true,"attribute_input_retains_member_cfg":true,"sealed":true,"published":true,"full_fixture_acceptance":false,"derives":executed_derives})).unwrap()).unwrap();
                    }
                } else {
                    fs::write(evidence.join("derive-append.json"),serde_json::to_vec_pretty(&serde_json::json!({"native_invocations":4,"retained_outputs":2,"original_item_present":true,"appended_const_present":true,"discarded_item_absent":true,"sealed":true,"published":true,"full_fixture_acceptance":false,"derives":executed_derives})).unwrap()).unwrap();
                }
                if derive_prefix {
                    fs::write(evidence.join("derive-prefix.json"),serde_json::to_vec_pretty(&serde_json::json!({"native_invocations":4,"accounted_expansions":6,"retained_outputs":2,"earlier_prefix_found":true,"later_shadow_found":true,"sealed":true,"published":true,"full_fixture_acceptance":false})).unwrap()).unwrap();
                }
                if surround_empty {
                    fs::write(evidence.join("surround-empty.json"),serde_json::to_vec_pretty(&serde_json::json!({"native_invocations":4,"retained_outputs":2,"empty_groups":2,"original_present":true,"constant_present":true,"sealed":true,"published":true,"full_fixture_acceptance":false})).unwrap()).unwrap();
                }
                return;
            }
            SourceDiscoveryStep::NeedDeriveDeclaration { request } => {
                assert!(derive_pending);
                assert_eq!(
                    request.fingerprint(),
                    declaration_requests[adopted_declarations].fingerprint()
                );
                let SourceDiscoveryStep::NeedDeriveDeclaration { request: repeated } =
                    discovery.poll().unwrap()
                else {
                    panic!("pending declaration was not retained")
                };
                assert_eq!(repeated.fingerprint(), request.fingerprint());
                assert_eq!(run.unit_invocations(unit), 2);
                if !derive_adoption {
                    discovery.defer_derive_declaration(&request).unwrap();
                    continue;
                }
                let lrr::service::derive_declarations::DeriveDeclarationPoll::Ready(receipt) =
                    run.resolve_derive_declaration(&mut db, &request).unwrap()
                else {
                    panic!("source receipt unavailable")
                };
                let receipt = Arc::new(receipt);
                if adopted_declarations == 0 {
                    let wrong = &declaration_requests[1];
                    let lrr::service::derive_declarations::DeriveDeclarationPoll::Ready(other) =
                        run.resolve_derive_declaration(&mut db, wrong).unwrap()
                    else {
                        panic!("second receipt unavailable")
                    };
                    assert!(
                        discovery
                            .adopt_derive_declaration(&mut db, &request, Arc::new(other))
                            .is_err()
                    );
                    assert_eq!(discovery.derive_declaration_count().unwrap(), 0);
                }
                discovery
                    .adopt_derive_declaration(&mut db, &request, receipt.clone())
                    .unwrap();
                adopted_declarations += 1;
                assert_eq!(
                    discovery.derive_declaration_count().unwrap(),
                    adopted_declarations
                );
                assert!(
                    discovery
                        .adopt_derive_declaration(&mut db, &request, receipt)
                        .is_err()
                );
                assert!(discovery.sealed_expansions().is_err());
                assert_eq!(run.unit_invocations(unit), 2);
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                if helper_chain
                    && request
                        .procedural_request()
                        .is_some_and(|r| r.export == "after_helper")
                {
                    assert_eq!(run.unit_invocations(unit), 5);
                    assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-PERMISSION");
                    let mut next_phase = phase.clone();
                    next_phase.index = 5;
                    run.fulfil_generated_discovery(
                        &mut db,
                        &mut discovery,
                        next_phase,
                        &request,
                        &mut cache,
                    )
                    .unwrap();
                    assert_eq!(run.unit_invocations(unit), 6);
                    derive_events.push("attribute:after_helper".into());
                    continue;
                }
                if empty_attribute
                    && request
                        .procedural_request()
                        .is_some_and(|r| r.export == "after_empty")
                {
                    assert_eq!(run.unit_invocations(unit), 2);
                    let mut next_phase = phase.clone();
                    next_phase.index = 2;
                    assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-PERMISSION");
                    run.fulfil_generated_discovery(
                        &mut db,
                        &mut discovery,
                        next_phase,
                        &request,
                        &mut cache,
                    )
                    .unwrap();
                    assert_eq!(run.unit_invocations(unit), 3);
                    continue;
                }
                if later_attribute
                    && request
                        .procedural_request()
                        .is_some_and(|r| r.export == "after_derive")
                {
                    assert_eq!(run.unit_invocations(unit), 4);
                    assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-PERMISSION");
                    let mut next_phase = phase.clone();
                    next_phase.index = 4;
                    run.fulfil_generated_discovery(
                        &mut db,
                        &mut discovery,
                        next_phase,
                        &request,
                        &mut cache,
                    )
                    .unwrap();
                    assert_eq!(run.unit_invocations(unit), 5);
                    if helper_identity {
                        let args = request
                            .procedural_request()
                            .unwrap()
                            .attributes
                            .as_ref()
                            .map(identifiers);
                        let value = db
                            .input(&lrr::db::dependency::InputKey::ExpansionRecord(
                                request.key().fingerprint(),
                            ))
                            .unwrap();
                        let lrr::db::dependency::InputValue::ExpansionRecord(output) =
                            value.as_ref()
                        else {
                            panic!("attribute output absent")
                        };
                        let output_tokens =
                            TokenForest::decode(&output.tokens().unwrap().encode().unwrap())
                                .unwrap();
                        fs::write(evidence.join("helper-attribute-observation.json"),serde_json::to_vec_pretty(&serde_json::json!({"arguments":args,"output_identifiers":identifiers(&output_tokens)})).unwrap()).unwrap();
                    }
                    derive_events.push("attribute:after_derive".into());
                    continue;
                }
                if request.is_derive() {
                    assert!(derive_adoption);
                    if !derive_execution {
                        discovery.defer_generated_derive(&request).unwrap();
                        continue;
                    }
                    let at = executed_derives.len();
                    assert!(at < 2);
                    assert_eq!(
                        request.procedural_request().unwrap().export,
                        ["CheckDeriveInput", "Empty"][at]
                    );
                    assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-PERMISSION");
                    let SourceDiscoveryStep::NeedGeneratedExpansion { request: repeated } =
                        discovery.poll().unwrap()
                    else {
                        panic!("derive request lost before execution")
                    };
                    assert_eq!(repeated.key(), request.key());
                    assert_eq!(run.unit_invocations(unit), u64::try_from(2 + at).unwrap());
                    let parent = request.parent_record().key().fingerprint();
                    let parent_fingerprint = request.parent_record().descriptor().fingerprint();
                    let forged = Arc::new(
                        lrr::expand::record::ExpansionRecord::from_wire(
                            request.key().clone(),
                            request
                                .procedural_request()
                                .unwrap()
                                .input
                                .encode()
                                .unwrap(),
                        )
                        .unwrap(),
                    );
                    assert!(discovery.admit_expansion(forged.clone()).is_err());
                    assert!(
                        discovery
                            .admit_procedural_expansion(&mut db, forged)
                            .is_err()
                    );
                    let mut derive_phase = phase.clone();
                    derive_phase.index = u32::try_from(2 + at).unwrap();
                    run.fulfil_generated_discovery(
                        &mut db,
                        &mut discovery,
                        derive_phase,
                        &request,
                        &mut cache,
                    )
                    .unwrap();
                    assert_eq!(run.unit_invocations(unit), u64::try_from(3 + at).unwrap());
                    assert_eq!(discovery.retained_derive_output_count().unwrap(), at + 1);
                    let value = db
                        .input(&lrr::db::dependency::InputKey::ExpansionRecord(parent))
                        .unwrap();
                    let lrr::db::dependency::InputValue::ExpansionRecord(original) = value.as_ref()
                    else {
                        panic!("derive parent record absent")
                    };
                    assert_eq!(original.descriptor().fingerprint(), parent_fingerprint);
                    let value = db
                        .input(&lrr::db::dependency::InputKey::ExpansionRecord(
                            request.key().fingerprint(),
                        ))
                        .unwrap();
                    let lrr::db::dependency::InputValue::ExpansionRecord(record) = value.as_ref()
                    else {
                        panic!("derive output record absent")
                    };
                    let output = lrr::expand::generated_items::GeneratedItems::admit(
                        request.key(),
                        Arc::new(record.clone()),
                        context.epoch,
                    )
                    .unwrap();
                    if at == 0 {
                        assert!(output.items().iter().any(|h| {
                            h.name
                                .as_ref()
                                .is_some_and(|n| n.text == "DERIVE_INPUT_ATTRIBUTES")
                        }));
                    } else if derive_prefix {
                        assert!(
                            output
                                .items()
                                .iter()
                                .any(|h| h.name.as_ref().is_some_and(|n| n.text == "prefix"))
                        );
                    } else {
                        assert!(output.items().is_empty());
                    }
                    assert!(
                        discovery
                            .admit_procedural_expansion(&mut db, Arc::new(record.clone()))
                            .is_err()
                    );
                    fs::write(
                        evidence.join(format!("derive-{at}-output.lrrtok")),
                        record.tokens().unwrap().encode().unwrap(),
                    )
                    .unwrap();
                    fs::write(
                        evidence.join(format!("derive-{at}-key.json")),
                        serde_json::to_vec_pretty(request.key()).unwrap(),
                    )
                    .unwrap();
                    executed_derives.push(serde_json::json!({"export":request.procedural_request().unwrap().export,"occurrence":request.key().invocation.hex(),"key":request.key().fingerprint().hex(),"parent":parent.hex(),"parent_descriptor":parent_fingerprint.hex(),"output":record.output().hex(),"native_ordinal":run.unit_invocations(unit)}));
                    derive_events.push(format!(
                        "derive:{}",
                        request.procedural_request().unwrap().export
                    ));
                    continue;
                }
                if request.procedural_request().is_some() {
                    assert!(
                        nested || attribute,
                        "unexpected generated procedural request"
                    );
                    let before = run.unit_invocations(unit);
                    assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-PERMISSION");
                    let forged = Arc::new(
                        lrr::expand::record::ExpansionRecord::from_wire(
                            request.key().clone(),
                            request
                                .procedural_request()
                                .unwrap()
                                .input
                                .encode()
                                .unwrap(),
                        )
                        .unwrap(),
                    );
                    assert_eq!(
                        discovery.admit_expansion(forged.clone()).unwrap_err().code,
                        "LRR-E-EXPANSION-INPUT"
                    );
                    assert!(
                        discovery
                            .admit_procedural_expansion(&mut db, forged)
                            .is_err()
                    );
                    assert_eq!(run.unit_invocations(unit), before);
                    let mut generated_phase = phase.clone();
                    generated_phase.index = u32::try_from(before).unwrap();
                    let admission = run.fulfil_generated_discovery(
                        &mut db,
                        &mut discovery,
                        generated_phase,
                        &request,
                        &mut cache,
                    );
                    if nested_ambiguity && let Err(error) = &admission {
                        assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
                        record_procedural_module_refusal(
                            &evidence,
                            &mut db,
                            &discovery,
                            run.unit_invocations(unit),
                            error,
                        );
                        return;
                    }
                    admission.unwrap();
                    if derive_pending && !empty_attribute {
                        let value = db
                            .input(&lrr::db::dependency::InputKey::ExpansionRecord(
                                request.key().fingerprint(),
                            ))
                            .unwrap();
                        let lrr::db::dependency::InputValue::ExpansionRecord(record) =
                            value.as_ref()
                        else {
                            panic!("actual second attribute output missing")
                        };
                        let items = lrr::expand::generated_items::GeneratedItems::admit(
                            request.key(),
                            Arc::new(record.clone()),
                            context.epoch,
                        )
                        .unwrap();
                        let index = items
                            .items()
                            .iter()
                            .position(|h| {
                                h.name.as_ref().is_some_and(|n| {
                                    n.text
                                        == if derive_prefix {
                                            "MarkerPrefix"
                                        } else {
                                            "Marker"
                                        }
                                })
                            })
                            .unwrap();
                        let proof =
                            lrr::expand::generated_derives::GeneratedDeriveAnnotations::new(
                                &items,
                                index,
                                &CfgSet::guest_default(),
                                (0..if surround_empty {
                                    3
                                } else if separate_groups {
                                    2
                                } else {
                                    1
                                })
                                    .map(|at| {
                                        (
                                            at,
                                            lrr::expand::builtin_attributes::descriptor("derive")
                                                .unwrap()
                                                .unwrap(),
                                        )
                                    }),
                            )
                            .unwrap();
                        let plan = lrr::expand::generated_derives::GeneratedDerivePlan::prepare(
                            &items,
                            index,
                            &CfgSet::guest_default(),
                            &proof,
                        )
                        .unwrap();
                        let wire = record.tokens().unwrap().encode().unwrap();
                        assert_eq!(Digest::of_bytes(&wire), record.output());
                        fs::write(evidence.join("generated-derive-output.lrrtok"), wire).unwrap();
                        fs::write(
                            evidence.join("generated-derive-key.json"),
                            serde_json::to_vec_pretty(request.key()).unwrap(),
                        )
                        .unwrap();
                        let compiler =
                            lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                                &ProfileIdentity::current(),
                            )
                            .unwrap();
                        let builtin = compiler.attribute_catalog().unwrap();
                        let mut names = lrr::frontend::macro_names::MacroNamespaces::default();
                        for export in catalog.exports() {
                            names
                                .define(
                                    lrr::frontend::macro_names::MacroPath::new(
                                        export.definition().defining_unit,
                                        vec![export.name().clone()],
                                    )
                                    .unwrap(),
                                    export.definition().clone(),
                                )
                                .unwrap();
                        }
                        names
                            .dependency(
                                unit,
                                lrr::expand::scopes::MacroName::parse("lrr_fixture_macros", 2024)
                                    .unwrap(),
                                *producer,
                            )
                            .unwrap();
                        let definition = builtin
                            .definitions()
                            .find(|d| builtin.select(d).unwrap().unwrap().name() == "derive")
                            .unwrap();
                        names
                            .define(
                                lrr::frontend::macro_names::MacroPath::new(
                                    unit,
                                    vec![
                                        lrr::expand::scopes::MacroName::parse("derive", 2024)
                                            .unwrap(),
                                    ],
                                )
                                .unwrap(),
                                definition.clone(),
                            )
                            .unwrap();
                        let scope = view.outline().root_scope.clone();
                        let annotations =
                            lrr::frontend::generated_derives::GeneratedDeriveResolution::resolve(
                                &items,
                                index,
                                &CfgSet::guest_default(),
                                scope,
                                &names,
                                builtin,
                            )
                            .unwrap();
                        let exports = annotations
                            .exports(
                                &items,
                                index,
                                &CfgSet::guest_default(),
                                &names,
                                builtin,
                                &catalog,
                            )
                            .unwrap();
                        assert_eq!(exports.rows().len(), 2);
                        assert!(exports.rows().iter().all(|row| row.selected().is_some()));
                        assert_ne!(
                            exports.rows()[0].helper_demand(),
                            exports.rows()[1].helper_demand()
                        );
                        exports
                            .validate_replay(
                                &annotations
                                    .exports(
                                        &items,
                                        index,
                                        &CfgSet::guest_default(),
                                        &names,
                                        builtin,
                                        &catalog,
                                    )
                                    .unwrap(),
                            )
                            .unwrap();
                        expected_derive_occurrence = Some(exports.rows()[0].occurrence());
                        declaration_requests = (0..exports.rows().len())
                            .map(|i| exports.declaration_request(i, &catalog).unwrap())
                            .collect();
                        if derive_helpers {
                            let first = exports.declaration_request(0, &catalog).unwrap();
                            let second = exports.declaration_request(1, &catalog).unwrap();
                            assert_ne!(first.fingerprint(), second.fingerprint());
                            let mut denied = MacroRun::begin(
                                &mut db,
                                context.clone(),
                                &manifest,
                                tool_input_plan,
                                false,
                            )
                            .unwrap();
                            assert!(
                                matches!(denied.resolve_derive_declaration(&mut db,&first),Err(e) if e.code=="LRR-E-MACRO-PERMISSION")
                            );
                            assert_eq!(denied.unit_invocations(unit), 0);
                            let mut foreign = MacroRun::begin(
                                &mut db,
                                context.clone(),
                                &manifest,
                                tool_input_plan,
                                true,
                            )
                            .unwrap();
                            assert!(
                                matches!(foreign.resolve_derive_declaration(&mut db,&first),Err(e) if e.code=="LRR-E-CATALOG-INCOMPLETE")
                            );
                            assert_eq!(foreign.unit_invocations(unit), 0);
                            let mut probe = MacroRun::begin(
                                &mut db,
                                context.clone(),
                                &manifest,
                                tool_input_plan,
                                true,
                            )
                            .unwrap();
                            struct PureDeclaration<'a,'b> {
                                run: std::cell::RefCell<&'a mut MacroRun<'b>>,
                                request: &'a lrr::frontend::generated_derive_exports::DeriveDeclarationRequest,
                            }
                            impl lrr::db::provider::Provider for PureDeclaration<'_, '_> {
                                fn execute(
                                    &self,
                                    db: &mut lrr::db::memo::Database,
                                    _: &QueryKey,
                                ) -> lrr::db::poll::QueryPoll<lrr::db::provider::Provided>
                                {
                                    let error = match self
                                        .run
                                        .borrow_mut()
                                        .resolve_derive_declaration(db, self.request)
                                    {
                                        Err(error) => error,
                                        Ok(_) => {
                                            panic!("pure provider admitted source helper metadata")
                                        }
                                    };
                                    lrr::db::poll::QueryPoll::Ready(Err(error))
                                }
                            }
                            {
                                let pure = PureDeclaration {
                                    run: std::cell::RefCell::new(&mut probe),
                                    request: &first,
                                };
                                let key = QueryKey::Constant(fixture_key(
                                    Digest([231; 32]),
                                    vec![],
                                    Digest([232; 32]),
                                    ProfileIdentity::current(),
                                ));
                                assert!(
                                    matches!(db.fetch(&pure,key),lrr::db::poll::QueryPoll::Ready(Err(error)) if error.code=="LRR-E-INTERNAL-SNAPSHOT")
                                );
                            }
                            assert_eq!(probe.unit_invocations(unit), 0);
                            let revision = db.source_revision();
                            let lrr::service::derive_declarations::DeriveDeclarationPoll::Ready(
                                receipt,
                            ) = run.resolve_derive_declaration(&mut db, &first).unwrap()
                            else {
                                panic!("exact original source declaration missing")
                            };
                            assert_eq!(receipt.helpers(), &["lrr_derive_helper".to_string()]);
                            receipt.validate_request(&first).unwrap();
                            assert!(receipt.validate_request(&second).is_err());
                            let lrr::service::derive_declarations::DeriveDeclarationPoll::Ready(
                                empty,
                            ) = run.resolve_derive_declaration(&mut db, &second).unwrap()
                            else {
                                panic!("exact empty declaration missing")
                            };
                            let lrr::service::derive_declarations::DeriveDeclarationPoll::Ready(
                                replayed,
                            ) = run.resolve_derive_declaration(&mut db, &first).unwrap()
                            else {
                                panic!("metadata replay lost exact declaration")
                            };
                            assert_eq!(replayed.source_declaration(), receipt.source_declaration());
                            assert_eq!(replayed.helpers(), receipt.helpers());
                            assert!(empty.helpers().is_empty());
                            empty.validate_request(&second).unwrap();
                            assert_eq!(run.unit_invocations(unit), 2);
                            assert_eq!(db.source_revision(), revision);
                            assert!(discovery.sealed_expansions().is_err());
                            fs::write(evidence.join("helper-declarations.json"),serde_json::to_vec_pretty(&serde_json::json!({"first_request":first.fingerprint().hex(),"second_request":second.fingerprint().hex(),"first_source_declaration":receipt.source_declaration().hex(),"second_source_declaration":empty.source_declaration().hex(),"first_helpers":receipt.helpers(),"second_helpers":empty.helpers(),"macro_invocations":run.unit_invocations(unit),"source_revision_unchanged":db.source_revision()==revision,"sealed":false,"published":false,"derive_execution":false,"pure_query_refused":true,"macro_authority_denied":true,"foreign_run_refused":true,"receipt_mismatch_refused":true,"metadata_replay_same":true})).unwrap()).unwrap();
                        }
                        expected_derive_demand = exports.first_demand().unwrap();
                        assert_ne!(
                            expected_derive_demand,
                            Some(
                                plan.annotations()
                                    .iter()
                                    .flat_map(|a| a.paths())
                                    .next()
                                    .unwrap()
                                    .identity
                            )
                        );
                        fs::write(evidence.join("derive-export-choices.json"),serde_json::to_vec_pretty(&exports.rows().iter().map(|row|serde_json::json!({"occurrence":row.occurrence().hex(),"helper_demand":row.helper_demand().unwrap().hex(),"definition":{"identity":row.selected().unwrap().definition().identity.hex(),"unit":row.selected().unwrap().definition().defining_unit.hex(),"edition":row.selected().unwrap().definition().definition_edition},"identity":row.selected().unwrap().identity()})).collect::<Vec<_>>()).unwrap()).unwrap();
                    }
                    assert_eq!(run.unit_invocations(unit), before + 1);
                } else {
                    generated += 1;
                    discovery
                        .admit_expansion(Arc::new(request.expand().unwrap()))
                        .unwrap();
                }
            }
            SourceDiscoveryStep::NeedProceduralSelection { selection: pending } if modules => {
                assert_eq!(pending.input().scope().module_path, ["child", "grandchild"]);
                assert_eq!(run.unit_invocations(unit), 1);
                let registry =
                    TokenForest::decode(&pending.input().arguments().encode().unwrap()).unwrap();
                let target = ResolvedItemTarget {
                    artifact: Key(macro_artifact_identity(*producer, artifact).0),
                    export: "identity".into(),
                    scope: Key(pending.input().scope().fingerprint().0),
                    parent_origin: pending.input().parent_origin(),
                    registry: &registry,
                    definition_site: pending.input().call_site(),
                    call_site: pending.input().call_site(),
                    mixed_site: pending.input().call_site(),
                    sources: vec![],
                };
                let selected = run
                    .prepare_source_selection(&mut db, &pending, target)
                    .unwrap();
                assert_ne!(selected.source(), &selection);
                discovery.bind_procedural_request(selected.clone()).unwrap();
                let SourceDiscoveryStep::NeedExpansion { request } = discovery.poll().unwrap()
                else {
                    panic!("module procedural request not retained")
                };
                assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-PERMISSION");
                assert_eq!(request.key(), selected.key());
                let forged = Arc::new(
                    lrr::expand::record::ExpansionRecord::from_wire(
                        selected.key().clone(),
                        selected.request().input.encode().unwrap(),
                    )
                    .unwrap(),
                );
                assert_eq!(
                    discovery.admit_expansion(forged.clone()).unwrap_err().code,
                    "LRR-E-EXPANSION-INPUT"
                );
                assert!(
                    discovery
                        .admit_procedural_expansion(&mut db, forged)
                        .is_err()
                );
                assert_eq!(run.unit_invocations(unit), 1);
                let mut phase = phase.clone();
                phase.index = 1;
                let admission =
                    run.fulfil_discovery(&mut db, &mut discovery, phase, &selected, &mut cache);
                if modules_ambiguity && let Err(error) = &admission {
                    assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
                    record_procedural_module_refusal(
                        &evidence,
                        &mut db,
                        &discovery,
                        run.unit_invocations(unit),
                        error,
                    );
                    return;
                }
                admission.unwrap();
                assert_eq!(run.unit_invocations(unit), 2);
            }
            SourceDiscoveryStep::NeedProceduralSelection { .. } => {
                panic!("unexpected procedural selection in this fixture")
            }
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::Incomplete { diagnostic, .. } if helper_bang => {
                assert_eq!(run.unit_invocations(unit), 5);
                assert_eq!(discovery.accounted_expansion_count().unwrap(), 6);
                assert!(discovery.sealed_expansions().is_err());
                assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
                fs::write(evidence.join("helper-bang-barrier.json"),serde_json::to_vec_pretty(&serde_json::json!({"native_invocations":5,"accounted_expansions":6,"incomplete":true,"published":false,"diagnostic":diagnostic.code,"full_fixture_acceptance":false})).unwrap()).unwrap();
                return;
            }
            SourceDiscoveryStep::Incomplete { diagnostic, .. } if modules => {
                let generated_module = discovery
                    .unresolved_module_sources()
                    .find(|module| !captures.contains_key(&module.producer()))
                    .map(|module| module.producer());
                let ordinary_module = discovery
                    .unresolved_ordinary_module_sources()
                    .find(|module| !captures.contains_key(&module.producer()))
                    .map(|module| module.producer());
                if let Some((producer, ordinary)) = generated_module
                    .map(|id| (id, false))
                    .or_else(|| ordinary_module.map(|id| (id, true)))
                {
                    let input = if ordinary {
                        discovery
                            .prepare_ordinary_module_source(producer, &authorities)
                            .unwrap()
                    } else {
                        discovery
                            .prepare_module_source(producer, &authorities, std::path::Path::new(""))
                            .unwrap()
                    };
                    adopt_procedural_worklist_module(
                        &mut db,
                        &context,
                        lrr::source::include_paths::IncludeAuthorities::new(
                            unit,
                            manifest.inputs.fingerprint(),
                            [lrr::source::include_paths::IncludeRoot {
                                identity: Digest([203; 32]),
                                path: evidence.join("app/src"),
                                kind: lrr::source::include_paths::IncludeRootKind::Source,
                            }],
                        )
                        .unwrap(),
                        &input,
                    );
                    if ordinary {
                        assert!(matches!(
                            discovery.stage_ordinary_module_source(&mut db, &input),
                            lrr::db::poll::QueryPoll::Ready(Ok(_))
                        ));
                    } else {
                        assert!(matches!(
                            discovery.stage_module_source(&mut db, &input),
                            lrr::db::poll::QueryPoll::Ready(Ok(_))
                        ));
                    }
                    captures.insert(producer, (input, ordinary, false));
                    continue;
                }
                let mut advanced = false;
                // Finish physical descendants before the generated parent.
                for ordinary in [true, false] {
                    for (input, is_ordinary, complete) in captures
                        .values_mut()
                        .filter(|(_, is_ordinary, complete)| *is_ordinary == ordinary && !*complete)
                    {
                        if *is_ordinary {
                            match discovery.complete_ordinary_module_namespace(&mut db, input) {
                                lrr::db::poll::QueryPoll::Ready(Ok(lrr::expand::source_discovery::OrdinaryModuleReadiness::Ready(_))) => { *complete = true; advanced = true; }
                                lrr::db::poll::QueryPoll::Ready(Ok(lrr::expand::source_discovery::OrdinaryModuleReadiness::Pending { .. })) => {}
                                lrr::db::poll::QueryPoll::Ready(Err(error)) if modules_ambiguity => {
                                    assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
                                    record_procedural_module_refusal(&evidence, &mut db, &discovery, run.unit_invocations(unit), &error);
                                    return;
                                }
                                other => panic!("ordinary procedural module completion: {}", match other { lrr::db::poll::QueryPoll::Ready(Err(e)) => format!("{e:?}"), _ => "unexpected capture demand".into() }),
                            }
                        } else {
                            match discovery.complete_module_namespace(&mut db, input) {
                                lrr::db::poll::QueryPoll::Ready(Ok(
                                    lrr::expand::source_discovery::ModuleSourceReadiness::Ready(_),
                                )) => {
                                    *complete = true;
                                    advanced = true;
                                }
                                lrr::db::poll::QueryPoll::Ready(Ok(
                                    lrr::expand::source_discovery::ModuleSourceReadiness::Pending {
                                        ..
                                    },
                                )) => {}
                                other => panic!(
                                    "generated procedural module completion: {}",
                                    match other {
                                        lrr::db::poll::QueryPoll::Ready(Err(e)) => format!("{e:?}"),
                                        _ => "unexpected capture demand".into(),
                                    }
                                ),
                            }
                        }
                    }
                }
                assert!(
                    advanced,
                    "physical procedural source fixed point blocked: {diagnostic:?}; native={}; generated={generated}",
                    run.unit_invocations(unit)
                );
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } if derive_pending => {
                assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
                fs::write(evidence.join("derive-pending-debug.json"),serde_json::to_vec_pretty(&serde_json::json!({"blocked":blocked,"native_invocations":run.unit_invocations(unit),"adopted_declarations":adopted_declarations,"accounted_outputs":discovery.accounted_expansion_count().unwrap(),"retained_derive_outputs":discovery.retained_derive_output_count().unwrap(),"label":label})).unwrap()).unwrap();
                let demand = if derive_adoption {
                    assert_eq!(
                        adopted_declarations,
                        2,
                        "derive receipt suspension: blocked={blocked:?}, native={}",
                        run.unit_invocations(unit)
                    );
                    assert_eq!(discovery.derive_declaration_count().unwrap(), 2);
                    if derive_scope {
                        assert_eq!(discovery.derive_helper_attribute_count().unwrap(), 1);
                        fs::write(evidence.join("helper-scope.json"),serde_json::to_vec_pretty(&serde_json::json!({"inert_attributes":discovery.derive_helper_attribute_count().unwrap(),"exact_declarations":2,"native_invocations":run.unit_invocations(unit),"sealed":false,"published":false,"derive_execution":derive_execution})).unwrap()).unwrap();
                    }

                    fs::write(evidence.join("helper-adoption.json"),serde_json::to_vec_pretty(&serde_json::json!({"adopted_declarations":2,"lexical_order_preserved":true,"wrong_receipt_refused":true,"duplicate_adoption_refused":true,"native_invocations":run.unit_invocations(unit),"sealed":false,"published":false,"derive_execution":derive_execution})).unwrap()).unwrap();
                    expected_derive_occurrence.expect("prepared derive occurrence absent")
                } else {
                    expected_derive_demand.expect("generated derive occurrence absent")
                };
                assert!(
                    blocked.iter().any(|(_, inputs)| inputs.contains(&demand)),
                    "production discovery did not suspend on the exact derive occurrence: {blocked:?}"
                );
                assert_eq!(
                    run.unit_invocations(unit),
                    if derive_execution { 4 } else { 2 }
                );
                assert_eq!(
                    discovery.accounted_expansion_count().unwrap(),
                    if derive_execution { 4 } else { 2 }
                );
                assert!(discovery.sealed_expansions().is_err());
                assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
                assert!(discovery.publish(&mut db).is_err());
                assert!(!matches!(
                    db.fetch(
                        &lrr::db::provider::SyntaxProvider,
                        QueryKey::DiscoverySeal(discovery.key().clone())
                    ),
                    lrr::db::poll::QueryPoll::Ready(Ok(_))
                ));
                if derive_execution {
                    assert_eq!(executed_derives.len(), 2);
                    assert_eq!(discovery.retained_derive_output_count().unwrap(), 2);
                    assert_eq!(executed_derives[0]["parent"], executed_derives[1]["parent"]);
                    fs::write(evidence.join("derive-execution.json"),serde_json::to_vec_pretty(&serde_json::json!({"derives":executed_derives,"native_invocations":run.unit_invocations(unit),"retained_outputs":discovery.retained_derive_output_count().unwrap(),"original_parent_unchanged":true,"unauthorized_output_refused":true,"duplicate_output_refused":true,"append_pending":true,"sealed":false,"published":false,"full_fixture_acceptance":false})).unwrap()).unwrap();
                }
                fs::write(evidence.join("source.rs"), text).unwrap();
                fs::write(evidence.join("refusal.json"), serde_json::to_vec_pretty(&serde_json::json!({"task":"M013","platform":"aarch64-apple-darwin","diagnostic":diagnostic.code,"native_invocations":run.unit_invocations(unit),"accounted_outputs":discovery.accounted_expansion_count().unwrap(),"sealed":false,"published":false,"release_complete":false,"compiler_annotation_selected":true,"prepared_derive_exports_selected":2,"derive_execution":derive_execution,"derive_occurrence_demand":demand.hex(),"scope":"two ordered replacements settle; required derive/helper discovery remains pending"})).unwrap()).unwrap();
                return;
            }
            SourceDiscoveryStep::Incomplete { diagnostic, .. } => {
                panic!("fixed point blocked: {diagnostic:?}")
            }
            _ => panic!("unexpected discovery demand"),
        }
    }
    assert_eq!(generated, 1);
    let native_count = if label == "procedural-source-nested-grandchild" {
        3
    } else if modules
        || nested
        || label == "procedural-source-attribute"
        || label == "procedural-source-attribute-chain"
    {
        2
    } else {
        1
    };
    assert_eq!(run.unit_invocations(unit), native_count);
    if modules {
        assert_eq!(captures.len(), 2);
        assert!(captures.values().all(|(_, _, complete)| *complete));
        assert!(matches!(
            discovery.lookup("caller").unwrap(),
            lrr::frontend::names::NameLookup::Defined(_)
        ));
        assert_eq!(discovery.sealed_expansions().unwrap().len(), 3);
    } else {
        assert!(matches!(
            discovery.lookup("made").unwrap(),
            lrr::frontend::names::NameLookup::Defined(_)
        ));
        assert!(matches!(
            discovery.lookup("alias").unwrap(),
            lrr::frontend::names::NameLookup::Defined(_)
        ));
    }
    if attribute || label.starts_with("procedural-source-generated-attribute") {
        assert!(matches!(
            discovery.lookup("discarded").unwrap(),
            lrr::frontend::names::NameLookup::Absent
        ));
    }
    if label == "procedural-source-attribute-chain" {
        assert!(matches!(
            discovery.lookup("discarded_outer").unwrap(),
            lrr::frontend::names::NameLookup::Absent
        ));
    }
    assert!(discovery.publish(&mut db).is_err());
    let mut empty_run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true)
        .unwrap()
        .with_expansion_budget(run.expansion_budget())
        .unwrap();
    assert_eq!(
        empty_run
            .publish_discovery(&mut db, &mut discovery)
            .unwrap_err()
            .code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
    assert_eq!(empty_run.unit_invocations(unit), 0);
    assert!(discovery.publish(&mut db).is_err());
    assert!(!matches!(
        db.fetch(
            &lrr::db::provider::SyntaxProvider,
            QueryKey::DiscoverySeal(discovery.key().clone())
        ),
        lrr::db::poll::QueryPoll::Ready(Ok(_))
    ));
    if label == "procedural-source-final-edit" {
        fs::write(
            evidence.join("app/src/lib.rs"),
            "fn cold(){ changed_input(); }\n",
        )
        .unwrap();
        let error = run.publish_discovery(&mut db, &mut discovery).unwrap_err();
        assert_eq!(error.code, "LRR-E-TOOL-PREPARE-STALE");
        assert!(
            run.fulfil(&mut db, unit, phase, selected.request(), &mut cache)
                .is_err()
        );
        assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
        assert!(discovery.publish(&mut db).is_err());
        assert!(!matches!(
            db.fetch(
                &lrr::db::provider::SyntaxProvider,
                QueryKey::DiscoverySeal(discovery.key().clone())
            ),
            lrr::db::poll::QueryPoll::Ready(Ok(_))
        ));
        assert_eq!(run.unit_invocations(unit), 1);
        fs::write(evidence.join("refusal.json"), serde_json::to_vec_pretty(&serde_json::json!({"task":"M013","diagnostic":error.code,"native_invocations":1,"sealed_locally":true,"published":false,"release_complete":false})).unwrap()).unwrap();
        return;
    }
    if let Some(expected) = expected_impls {
        assert_eq!(
            discovery.impl_header_inventory().unwrap().headers().len(),
            expected,
            "replaced item declarations leaked into final inventory"
        );
    }
    let certificate = run.publish_discovery(&mut db, &mut discovery).unwrap();
    assert_eq!(certificate.unit_count(), 1);
    certificate
        .validate(&mut db, &context, manifest.fingerprint())
        .unwrap();
    assert!(run.catalog(&mut db, unit).is_err());
    assert!(discovery.publish(&mut db).is_err());
    let query = QueryKey::DiscoverySeal(discovery.key().clone());
    assert!(matches!(
        db.fetch(&lrr::db::provider::SyntaxProvider, query.clone()),
        lrr::db::poll::QueryPoll::Ready(Ok(_))
    ));
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(db.source_revision(), source_revision);
    assert!(matches!(
        db.fetch(&lrr::db::provider::SyntaxProvider, query),
        lrr::db::poll::QueryPoll::Ready(Err(_))
    ));
    assert_eq!(
        discovery.accounted_expansion_count().unwrap(),
        native_count as usize + generated
    );
    let output_wire = record.tokens().unwrap().encode().unwrap();
    fs::write(evidence.join("source.rs"), text).unwrap();
    fs::write(evidence.join("output.tokens"), output_wire).unwrap();
    fs::write(evidence.join("worklist.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M013","platform":"aarch64-apple-darwin","rust":"1.99.0","native_invocations":run.unit_invocations(unit),"generated_invocations":generated,"sealed":true,"published":true,"unadopted_output_refused":true,"physical_modules":captures.len(),"scope":"original and physical-module function-like procedural worklist; full forms/nested generated/CLI integration pending","release_complete":false})).unwrap()).unwrap();
}

fn adopt_procedural_worklist_module(
    db: &mut lrr::db::memo::Database,
    base: &lrr::service::expansion::DemandContext,
    authorities: lrr::source::include_paths::IncludeAuthorities,
    input: &lrr::db::poll::InputRequest,
) {
    use lrr::{
        core::digest::{Digest, StableEncode},
        core::profile::ProfileIdentity,
        db::{key::QueryKey, poll::InputRequest},
        service::{
            expansion::{ExpansionCoordinator, PhaseIdentity},
            source_capture::SourceCapture,
        },
        source::manifest::BodyKey,
    };
    let InputRequest::CaptureModule { request, .. } = input else {
        panic!("expected physical module capture")
    };
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let record = capture.capture_module(request).unwrap();
    let mut context = base.clone();
    context.request = input.fingerprint();
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let token = coordinator
        .issue(
            QueryKey::Constant(fixture_key(
                Digest([204; 32]),
                vec![],
                Digest::default(),
                ProfileIdentity::current(),
            )),
            PhaseIdentity {
                frame: Digest([205; 32]),
                body: BodyKey {
                    definition: Digest([206; 32]),
                    exact_body_bytes: Digest([207; 32]),
                },
                index: 0,
            },
            input.clone(),
        )
        .unwrap();
    coordinator.start(&context, &token).unwrap();
    coordinator
        .accept_module(&context, &token, &record)
        .unwrap();
    coordinator
        .adopt_module(&context, &token, db, record)
        .unwrap();
}

fn record_procedural_module_refusal(
    evidence: &std::path::Path,
    db: &mut lrr::db::memo::Database,
    discovery: &lrr::expand::source_discovery::SourceDiscovery,
    invocations: u64,
    error: &lrr::core::diagnostic::Diagnostic,
) {
    assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
    assert_eq!(invocations, 2);
    assert!(discovery.sealed_expansions().is_err());
    assert!(discovery.publish(db).is_err());
    assert!(!matches!(
        db.fetch(
            &lrr::db::provider::SyntaxProvider,
            lrr::db::key::QueryKey::DiscoverySeal(discovery.key().clone())
        ),
        lrr::db::poll::QueryPoll::Ready(Ok(_))
    ));
    fs::write(evidence.join("refusal.json"), serde_json::to_vec_pretty(&serde_json::json!({"task":"M013","platform":"aarch64-apple-darwin","rust":"1.99.0","diagnostic":error.code,"native_invocations":invocations,"sealed":false,"published":false,"release_complete":false,"scope":"actual late procedural alias ambiguity"})).unwrap()).unwrap();
}

#[test]
fn rustc_printing_derives_use_prepared_matched_server_without_protocol_corruption() {
    use lrr::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        service::item_macro_inputs::{self, ResolvedDeriveAnnotations},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::parse::ParsedFile,
    };
    let prepared = prepared();
    let root = prepared
        .work
        .join("official-original-derive-order-prepared");
    fs::create_dir_all(root.join("src")).unwrap();
    let auxiliary = include_bytes!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/multiple-derives.rs"
    );
    fs::write(root.join("src/lib.rs"), auxiliary).unwrap();
    fs::write(root.join("Cargo.toml"),"[package]\nname=\"multiple_derives\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    for (label, args) in [
        ("lock", vec!["generate-lockfile", "--offline"]),
        (
            "build",
            vec!["build", "--locked", "--offline", "--message-format=json"],
        ),
    ] {
        let mut command = Command::new("cargo");
        command
            .arg("+1.99.0")
            .args(args)
            .arg("--manifest-path")
            .arg(root.join("Cargo.toml"));
        if label == "build" {
            command.arg("--target-dir").arg(root.join("target"));
        }
        let result = command
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_INCREMENTAL", "0")
            .output()
            .unwrap();
        fs::write(root.join(format!("{label}.stdout")), &result.stdout).unwrap();
        fs::write(root.join(format!("{label}.stderr")), &result.stderr).unwrap();
        assert!(
            result.status.success(),
            "unchanged upstream tool preparation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let messages: Vec<Value> = fs::read_to_string(root.join("build.stdout"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let artifacts: Vec<_> = messages
        .iter()
        .filter(|m| m["reason"] == "compiler-artifact")
        .collect();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(
        artifacts[0]["target"]["kind"],
        serde_json::json!(["proc-macro"])
    );
    let filenames = artifacts[0]["filenames"].as_array().unwrap();
    assert_eq!(filenames.len(), 1);
    let artifact = PathBuf::from(filenames[0].as_str().unwrap());
    let artifact_hash = blake3::hash(&fs::read(&artifact).unwrap())
        .to_hex()
        .to_string();
    let mut config = prepared.config.clone();
    config.directory = root.clone();
    config.server = prepared_matched_server().clone();
    config.artifacts = vec![MacroArtifact {
        identity: Key([171; 32]),
        path: artifact,
        raw_blake3: artifact_hash,
    }];
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.rs"
    );
    let expected = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.stdout"
    );
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let cfg = CfgSet::guest_default();
    let outline = parsed.outline(Digest(config.unit.0), vec![], &cfg).unwrap();
    let item = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "Foo")
        .unwrap();
    let facts = parsed
        .derive_item_inputs_at(item.declaration_span, &cfg)
        .unwrap();
    assert_eq!(facts.item.attributes.len(), 4);
    let identity = SourceIdentity {
        unit: config.unit,
        content: Key(blob.content().0),
        path: "tests/ui/proc-macro/derive-expand-order.rs".into(),
    };
    let original = token_wire::parse_source(
        source,
        identity.clone(),
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: config.unit,
            edition: Edition::E2024,
        },
        Key([172; 32]),
    )
    .unwrap();
    let forest = lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let owner = item.def_key.fingerprint();
    let descriptor = lrr::expand::builtin_attributes::descriptor("derive")
        .unwrap()
        .unwrap();
    let proof =
        ResolvedDeriveAnnotations::new(owner, &facts, (0..4).map(|at| (at, descriptor.clone())))
            .unwrap();
    let mut session = ProcSession::new(config.clone()).unwrap();
    let exports: std::collections::BTreeSet<_> = session
        .exports()
        .map(|e| (e.name.to_owned(), e.form))
        .collect();
    assert_eq!(
        exports,
        ["First", "Second", "Third", "Fourth", "Fifth"]
            .into_iter()
            .map(|n| (n.to_owned(), ProcForm::Derive))
            .collect()
    );
    let mut rows = Vec::new();
    let mut payloads = Vec::new();
    for (annotation, ordinal) in [(0, 0), (1, 0), (2, 0), (2, 1), (3, 0)] {
        let input = item_macro_inputs::derive_occurrence_resolved(
            owner,
            &facts,
            (annotation, ordinal),
            &proof,
            &forest.spans()[0].source,
            &forest,
        )
        .unwrap();
        let tokens = TokenForest::decode(&input.item().encode().unwrap()).unwrap();
        let site = tokens.spans()[0].key();
        payloads.push(tokens.clone());
        let request = ProcInvocation {
            identity: Key(input.identity().0),
            unit: config.unit,
            epoch: config.epoch,
            artifact: config.artifacts[0].identity,
            export: input.selected().segments[0].clone(),
            form: ProcForm::Derive,
            grammar: ProcGrammar::Items,
            scope: Key(item.scope.fingerprint().0),
            parent_origin: Key(outline.fingerprint().0),
            input: tokens,
            attributes: None,
            definition_site: site,
            call_site: site,
            mixed_site: site,
            sources: vec![
                lrr_expand_adapter::proc_client::CapturedSpanSource::new(
                    identity.clone(),
                    source.into(),
                    lrr_expand_adapter::proc_client::SourceDigestEncoding::LrrSourceBytes,
                    None,
                )
                .unwrap(),
            ],
        };
        let result = session.expand(&request);
        if let Err(error) = &result {
            let (stdout, stderr) = session.diagnostics().unwrap();
            fs::write(root.join("server.stdout"), &stdout).unwrap();
            fs::write(root.join("server.stderr"), &stderr).unwrap();
            fs::write(
                root.join("protocol-failure.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "diagnostic":error.code,"message":error.message,"export":request.export,
                    "native_invocations":session.invocations(),"full_fixture_acceptance":false
                }))
                .unwrap(),
            )
            .unwrap();
        }
        let output = result.unwrap();
        assert!(identifiers(&output).is_empty());
        let [TokenTree::Group { children, .. }] = output.roots() else {
            panic!("upstream derive output root absent")
        };
        assert!(children.is_empty());
        fs::write(
            root.join(format!("input-{annotation}-{ordinal}.lrrtok")),
            input.item().encode().unwrap(),
        )
        .unwrap();
        rows.push(serde_json::json!({"annotation":annotation,"ordinal":ordinal,"export":request.export,"occurrence":input.identity().hex(),"input_blake3":blake3::hash(&input.item().encode().unwrap()).to_hex().to_string(),"empty_output":true}));
    }
    assert_eq!(session.invocations(), 5);
    session.cancel();
    let (stdout, stderr) = session.diagnostics().unwrap();
    fs::write(root.join("server.stdout"), &stdout).unwrap();
    fs::write(root.join("server.stderr"), &stderr).unwrap();
    let snapshot = session.diagnostic_snapshot().unwrap();
    assert!(!snapshot.protocol_truncated && !snapshot.stderr_truncated);
    let printed: Vec<_> = String::from_utf8_lossy(&stderr)
        .lines()
        .filter(|line| line.starts_with("Derive "))
        .map(str::to_owned)
        .collect();
    let expected: Vec<_> = expected.lines().collect();
    assert_eq!(printed.len(), 5);
    for ((actual, expected), input) in printed.iter().zip(expected).zip(&payloads) {
        let (actual_label, actual_payload) = actual.split_once(": ").unwrap();
        let (expected_label, expected_payload) = expected.split_once(": ").unwrap();
        assert_eq!(actual_label, expected_label);
        let parse = |text: &str| {
            token_wire::parse_source(
                text,
                SourceIdentity {
                    unit: config.unit,
                    content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
                    path: "printed.rs".into(),
                },
                ContextRecord {
                    parent: None,
                    expansion: None,
                    transparency: Transparency::Opaque,
                    defining_unit: config.unit,
                    edition: Edition::E2024,
                },
                Key([174; 32]),
            )
            .unwrap()
        };
        let actual = parse(actual_payload);
        let expected = parse(expected_payload);
        assert_eq!(
            derive_print_shape(actual.roots()),
            derive_print_shape(expected.roots())
        );
        assert_eq!(
            derive_print_shape(input.roots()),
            derive_print_shape(expected.roots())
        );
    }
    fs::write(root.join("source.rs"), source).unwrap();
    fs::write(root.join("original-derive-input.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "upstream_case":"tests/ui/proc-macro/derive-expand-order.rs","upstream_auxiliary":"tests/ui/proc-macro/auxiliary/multiple-derives.rs",
        "native_invocations":session.invocations(),"rows":rows,"printed":printed,"source_adaptations":[],"macro_host_source_unchanged":fs::read(root.join("src/lib.rs")).unwrap()==auxiliary,
        "source_profile_edition":2024,"macro_host_edition":2015,"server_compiler_commit":config.server.compiler_commit,"artifact_blake3":config.artifacts[0].raw_blake3,
        "helper_metadata_prepared":false,"diagnostics_channel":"stderr","protocol":6,"server":config.server.path,"server_raw_blake3":config.server.raw_blake3,"completed_occurrences":5,"discovery_sealed":false,"guest_execution":false,"full_fixture_acceptance":false
    })).unwrap()).unwrap();
}

fn derive_print_shape(tokens: &[token_wire::TokenTree]) -> serde_json::Value {
    serde_json::Value::Array(
        tokens
            .iter()
            .map(|t| match t {
                token_wire::TokenTree::Group {
                    delimiter,
                    children,
                    ..
                } => serde_json::json!([format!("{delimiter:?}"), derive_print_shape(children)]),
                token_wire::TokenTree::Ident { text, raw, .. } => {
                    serde_json::json!(["ident", text, raw])
                }
                token_wire::TokenTree::Punct {
                    scalar, spacing, ..
                } => serde_json::json!(["punct", scalar, format!("{spacing:?}")]),
                token_wire::TokenTree::Literal { token_text, .. } => {
                    serde_json::json!(["literal", token_text])
                }
            })
            .collect(),
    )
}

fn adopt_fixture_prepared_server(
    manifest: &mut lrr::project::tool_manifest::ToolManifest,
    evidence: &std::path::Path,
) {
    use lrr::core::digest::Digest;
    use lrr::project::prepared_macro_server::{PreparedServerRequest, capture};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let server = prepared_matched_server();
    let echo_source = root.join("fixtures/macros/tool-workspace/fixture-macros/Cargo.toml");
    let result = Command::new("cargo")
        .args([
            "+1.99.0",
            "build",
            "--locked",
            "--offline",
            "--message-format=json",
            "--manifest-path",
        ])
        .arg(&echo_source)
        .arg("--target-dir")
        .arg(evidence.join("echo-target"))
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .unwrap();
    fs::write(evidence.join("echo-prepare.stdout"), &result.stdout).unwrap();
    fs::write(evidence.join("echo-prepare.stderr"), &result.stderr).unwrap();
    assert!(
        result.status.success(),
        "explicit compatibility host preparation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let messages: Vec<Value> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let artifacts: Vec<_> = messages
        .iter()
        .filter(|m| m["reason"] == "compiler-artifact")
        .collect();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(
        artifacts[0]["target"]["kind"],
        serde_json::json!(["proc-macro"])
    );
    let files = artifacts[0]["filenames"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    let artifact = PathBuf::from(files[0].as_str().unwrap());
    let tool_source_root = root
        .join("tools")
        .canonicalize()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let request = PreparedServerRequest {
        allow_tool_preparation: true,
        allow_macro_execution: true,
        source_root: &tool_source_root,
        server: &server.path,
        compatibility_artifact: &artifact,
        compatibility_content: Digest::of_bytes(&fs::read(&artifact).unwrap()),
        directory: evidence,
    };
    let mut denied = request.clone();
    denied.allow_tool_preparation = false;
    denied.source_root = std::path::Path::new("/absent-source-root");
    assert_eq!(
        capture(&manifest.toolchain, &denied).unwrap_err().code,
        "LRR-E-TOOLCHAIN-CAPABILITY"
    );
    denied.allow_tool_preparation = true;
    denied.allow_macro_execution = false;
    assert_eq!(
        capture(&manifest.toolchain, &denied).unwrap_err().code,
        "LRR-E-MACRO-PERMISSION"
    );
    let mut wrong_compiler = manifest.toolchain.clone();
    wrong_compiler.compiler_commit = "0".repeat(40);
    assert!(
        capture(&wrong_compiler, &request)
            .unwrap_err()
            .message
            .contains("tuple changed before preparation")
    );
    assert!(!evidence.join("server-preparation.json").exists());
    let captured = capture(&manifest.toolchain, &request).unwrap();
    fs::write(
        evidence.join("server-capability.json"),
        serde_json::to_vec_pretty(&captured.capability).unwrap(),
    )
    .unwrap();
    manifest.toolchain.macro_server = captured.server;
    manifest.toolchain.prepared_macro_server = Some(captured.provenance);
    manifest.validate().unwrap();
    let mut forged = manifest.clone();
    forged.toolchain.prepared_macro_server = None;
    assert!(
        forged.validate().is_err(),
        "outside-sysroot path alone authorized server"
    );
    let mut wrong_pin = manifest.clone();
    wrong_pin
        .toolchain
        .prepared_macro_server
        .as_mut()
        .unwrap()
        .source_commit = "0".repeat(40);
    assert!(wrong_pin.validate().is_err());
    let mut missing_source = manifest.clone();
    missing_source
        .toolchain
        .prepared_macro_server
        .as_mut()
        .unwrap()
        .sources
        .pop_first();
    assert!(missing_source.validate().is_err());
}

#[test]
fn actual_prepared_server_macro_run_publishes_generated_source_with_checked_provenance() {
    procedural_source_worklist_case(
        "use lrr_fixture_macros::identity as selected; selected! { macro_rules! leaf { () => {pub fn made()->i64{7}} } pub use self::made as alias; leaf!(); }",
        true,
        "procedural-source-prepared-server",
    );
}

#[test]
fn rustc_original_printing_derives_complete_macro_run_source_worklist_and_publish_foo() {
    rustc_original_derive_worklist_component(false);
}

#[test]
fn rustc_original_add_impl_appends_actual_declarations_and_preserves_original_type() {
    rustc_original_derive_worklist_component(true);
}

fn rustc_original_derive_worklist_component(add_impl: bool) {
    use lrr::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        db::{
            key::{BucketKey, OutlineKey, QueryKey, SealedNameKey},
            memo::Database,
            poll::QueryPoll,
            provider::SyntaxProvider,
            value::QueryValue,
        },
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        service::{
            expansion::{DemandContext, PhaseIdentity},
            run::MacroRun,
        },
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            manifest::{BodyKey, Namespace},
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
    };
    let prepared = prepared();
    let base = prepared.work.join(if add_impl {
        "official-add-impl-worklist"
    } else {
        "official-printing-derive-worklist"
    });
    fs::create_dir(&base).unwrap();
    let macro_source = base.join("multiple-derives");
    fs::create_dir_all(macro_source.join("src")).unwrap();
    let auxiliary: &[u8] = if add_impl {
        include_bytes!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/add-impl.rs"
        )
    } else {
        include_bytes!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/multiple-derives.rs"
        )
    };
    fs::write(macro_source.join("src/lib.rs"), auxiliary).unwrap();
    fs::write(macro_source.join("Cargo.toml"), "[package]\nname=\"multiple_derives\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    let original = if add_impl {
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/add-impl.rs")
            .replace("extern crate add_impl;", "extern crate multiple_derives;")
    } else {
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.rs"
        )
        .to_owned()
    };
    let type_name = if add_impl { "B" } else { "Foo" };
    let expected_exports: &[&str] = if add_impl {
        &["AddImpl"]
    } else {
        &["First", "Second", "Third", "Fourth", "Fifth"]
    };
    let expected_count = expected_exports.len();
    let evidence = base.join("preparation");
    let mut manifest =
        service_fixture::prepare_original_derives(&evidence, &macro_source, &original);
    adopt_fixture_prepared_server(&mut manifest, &evidence);
    let plan = recorded_macro_plan(&evidence, &macro_source);
    let unit = *manifest.graph.roots.first().unwrap();
    let cfg = CfgSet::guest_default();
    let profile = ProfileIdentity::current();
    let blob = SourceBlob::new(original.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("lib.rs").unwrap();
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: lrr::syntax::parse::ParseKey {
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
        panic!("original source absent")
    };
    let expected_foo = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == type_name)
        .unwrap()
        .def_key
        .fingerprint();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let context = DemandContext {
        request: Digest([185; 32]),
        session: Digest([186; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
    let catalog = run.catalog(&mut db, unit).unwrap();
    assert!(
        SourceDiscovery::new(
            vec![view.share()],
            view.outline().root_scope.clone(),
            Namespace::Type,
            Digest([192; 32]),
            1
        )
        .unwrap()
        .with_declarative_procedural(
            context.compile_environment,
            Digest([187; 32]),
            catalog.clone()
        )
        .is_err()
    );
    assert_eq!(run.unit_invocations(unit), 0);
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Type,
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_declarative_procedural(context.compile_environment, Digest([187; 32]), catalog)
    .unwrap()
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap()
    .with_compiler(Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &ProfileIdentity::current(),
        )
        .unwrap(),
    ))
    .unwrap();
    let mut cache = lrr::expand::cache::ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut exports = Vec::new();
    let mut appended = Vec::new();
    let mut occurrences = Vec::new();
    let mut payloads = Vec::new();
    let module_authorities = lrr::source::include_paths::IncludeAuthorities::new(
        unit,
        manifest.inputs.fingerprint(),
        [lrr::source::include_paths::IncludeRoot {
            identity: Digest([203; 32]),
            path: evidence.join("app/src"),
            kind: lrr::source::include_paths::IncludeRootKind::Source,
        }],
    )
    .unwrap();
    let mut module_captures = BTreeMap::new();
    let mut sealed = false;
    for _ in 0..64 {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedDeriveDeclaration { request } => {
                let lrr::service::derive_declarations::DeriveDeclarationPoll::Ready(receipt) =
                    run.resolve_derive_declaration(&mut db, &request).unwrap()
                else {
                    panic!("original derive declaration unavailable")
                };
                discovery
                    .adopt_derive_declaration(&mut db, &request, Arc::new(receipt))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedSourceDeriveSelection { selection } => {
                assert_eq!(exports.len(), appended.len());
                let request = run
                    .prepare_source_derive_selection(&mut db, &selection)
                    .unwrap();
                fs::write(
                    base.join(format!("input-{}.lrrtok", exports.len())),
                    selection.input().item().encode().unwrap(),
                )
                .unwrap();
                occurrences.push(selection.input().identity().hex());
                payloads.push(
                    TokenForest::decode(&selection.input().item().encode().unwrap()).unwrap(),
                );
                assert_eq!(request.key().definition_edition, 2015);
                assert_eq!(request.key().call_edition, 2024);
                discovery.bind_source_derive_request(request).unwrap();
            }
            SourceDiscoveryStep::NeedSourceDeriveExpansion { request } => {
                let phase = PhaseIdentity {
                    frame: Digest([188; 32]),
                    body: BodyKey {
                        definition: Digest([189; 32]),
                        exact_body_bytes: Digest([190; 32]),
                    },
                    index: u32::try_from(exports.len()).unwrap(),
                };
                let output = run
                    .fulfil_source_derive(&mut db, &mut discovery, phase, &request, &mut cache)
                    .unwrap();
                let [lrr::syntax::token_wire::TokenTree::Group { children, .. }] =
                    output.tokens().roots()
                else {
                    panic!("empty derive output root absent")
                };
                assert_eq!(children.is_empty(), !add_impl);
                exports.push(request.request().export.clone());
            }
            SourceDiscoveryStep::NeedSourceDeriveAppend { request, record } => {
                let items = lrr::expand::generated_items::GeneratedItems::admit(
                    record.key(),
                    record.clone(),
                    1,
                )
                .unwrap();
                fs::write(base.join("append-items.json"),serde_json::to_vec_pretty(&items.items().iter().map(|i|serde_json::json!({"identity":i.key.fingerprint(),"kind":format!("{:?}",i.kind),"range":[i.start,i.end]})).collect::<Vec<_>>()).unwrap()).unwrap();
                let before = run.unit_invocations(unit);
                run.integrate_source_derive_append(
                    &mut db,
                    &mut discovery,
                    &request,
                    record.clone(),
                )
                .unwrap();
                assert_eq!(before, run.unit_invocations(unit));
                appended.push(record.descriptor().fingerprint().hex());
            }
            SourceDiscoveryStep::Sealed(_) => {
                sealed = true;
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                if add_impl {
                    let pending_module = discovery
                        .unresolved_module_sources()
                        .find(|module| !module_captures.contains_key(&module.producer()))
                        .map(|module| module.producer());
                    if let Some(producer) = pending_module {
                        let input = discovery
                            .prepare_module_source(
                                producer,
                                &module_authorities,
                                std::path::Path::new(""),
                            )
                            .unwrap();
                        adopt_procedural_worklist_module(
                            &mut db,
                            &context,
                            lrr::source::include_paths::IncludeAuthorities::new(
                                unit,
                                manifest.inputs.fingerprint(),
                                [lrr::source::include_paths::IncludeRoot {
                                    identity: Digest([203; 32]),
                                    path: evidence.join("app/src"),
                                    kind: lrr::source::include_paths::IncludeRootKind::Source,
                                }],
                            )
                            .unwrap(),
                            &input,
                        );
                        assert!(matches!(
                            discovery.stage_module_source(&mut db, &input),
                            QueryPoll::Ready(Ok(_))
                        ));
                        module_captures.insert(producer, (input, false));
                        continue;
                    }
                    let mut advanced = false;
                    for (input, completed) in module_captures
                        .values_mut()
                        .filter(|(_, completed)| !*completed)
                    {
                        match discovery.complete_module_namespace(&mut db, input) {
                            QueryPoll::Ready(Ok(
                                lrr::expand::source_discovery::ModuleSourceReadiness::Ready {
                                    ..
                                },
                            )) => {
                                *completed = true;
                                advanced = true;
                            }
                            QueryPoll::Ready(Ok(
                                lrr::expand::source_discovery::ModuleSourceReadiness::Pending {
                                    ..
                                },
                            )) => {}
                            other => panic!(
                                "derive append module did not complete: {}",
                                match other {
                                    QueryPoll::Ready(Err(error)) => format!("{error:?}"),
                                    _ => "unfulfilled module input".into(),
                                }
                            ),
                        }
                    }
                    if advanced {
                        continue;
                    }
                }
                let scan = lrr::syntax::parse::ParsedFile::parse_declarations(
                    &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
                    ProfileIdentity::current(),
                )
                .unwrap()
                .scan_declarations(
                    SourcePath::new("lib.rs").unwrap(),
                    view.outline().root_scope.clone(),
                    &CfgSet::guest_default(),
                    Default::default(),
                )
                .unwrap();
                let producers: Vec<_> = scan.producers().iter().map(|p| serde_json::json!({"identity":p.identity,"kind":format!("{:?}",p.kind),"range":[p.range.start,p.range.end],"path":p.path.iter().map(|t|t.text.as_str()).collect::<Vec<_>>() })).collect();
                fs::write(base.join("incomplete.json"), serde_json::to_vec_pretty(&serde_json::json!({"native_invocations":run.unit_invocations(unit),"appended":appended,"exports":exports,"source_producers":producers,"source_items":view.outline().items.iter().map(|i|serde_json::json!({"identity":i.def_key.fingerprint(),"kind":format!("{:?}",i.def_key.item_kind),"name":i.def_key.declared_name})).collect::<Vec<_>>(),"captured_modules":module_captures.len(),"impl_headers":discovery.impl_header_inventory().unwrap().headers().iter().map(|h|serde_json::json!({"identity":h.identity,"header_identity":h.header_identity})).collect::<Vec<_>>(),"blocked":blocked,"diagnostic":diagnostic.code,"guest_execution":false,"full_fixture_acceptance":false})).unwrap()).unwrap();
                panic!(
                    "unchanged derive fixture did not settle: {diagnostic:?}; {blocked:?}; native={}; appended={}",
                    run.unit_invocations(unit),
                    appended.len()
                )
            }
            _ => panic!("unexpected original derive demand"),
        }
    }
    assert!(
        sealed,
        "original derive worklist exhausted its bounded step allowance"
    );
    assert_eq!(exports, expected_exports);
    assert_eq!(appended.len(), expected_count);
    assert_eq!(
        occurrences
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        expected_count
    );
    assert_eq!(
        usize::try_from(run.unit_invocations(unit)).unwrap(),
        expected_count
    );
    assert_eq!(
        run.unit_declaration_expansions(unit),
        if add_impl { 0 } else { 1 }
    );
    assert_eq!(
        discovery.accounted_expansion_count().unwrap(),
        expected_count
    );
    assert_eq!(discovery.sealed_expansions().unwrap().len(), expected_count);
    assert!(
        matches!(discovery.lookup(type_name).unwrap(),lrr::frontend::names::NameLookup::Defined(actual) if actual.identity==expected_foo)
    );
    if add_impl {
        assert_eq!(
            discovery.impl_header_inventory().unwrap().headers().len(),
            1,
            "actual derive append impl absent from declaration inventory"
        );
    }
    run.publish_discovery(&mut db, &mut discovery).unwrap();
    let key = QueryKey::SealedName(Box::new(SealedNameKey {
        seal: discovery.key().clone(),
        bucket: BucketKey {
            scope: view.outline().root_scope.clone(),
            namespace: Namespace::Type,
            name: type_name.into(),
        },
    }));
    let QueryPoll::Ready(Ok(value)) = db.fetch(&SyntaxProvider, key) else {
        panic!("published Foo query not ready")
    };
    assert!(
        matches!(value.value.as_ref(),QueryValue::SealedName {lookup:lrr::frontend::names::NameLookup::Defined(actual),..} if actual.identity==expected_foo)
    );
    if add_impl {
        assert_eq!(
            fs::read(macro_source.join("src/lib.rs")).unwrap(),
            auxiliary
        );
        fs::write(base.join("worklist.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "upstream_case":"tests/ui/proc-macro/add-impl.rs", "native_invocations":run.unit_invocations(unit), "appended":appended, "exports":exports,
            "original_type_preserved":true,"captured_modules":module_captures.len(), "published_type_ready":true, "impl_headers":discovery.impl_header_inventory().unwrap().headers().len(),
            "macro_host_source_unchanged":true,"source_adaptations":["extern crate add_impl -> extern crate multiple_derives dependency alias"],
            "guest_execution":false,"full_fixture_acceptance":false,"scope":"actual original-source derive append declaration inventory and authority publication"
        })).unwrap()).unwrap();
        return;
    }
    let diagnostics = run.unit_diagnostics(&mut db, unit).unwrap().unwrap();
    struct DiagnosticQuery<'r, 'm> {
        run: &'r MacroRun<'m>,
        unit: Digest,
    }
    impl lrr::db::provider::Provider for DiagnosticQuery<'_, '_> {
        fn execute(
            &self,
            db: &mut Database,
            _: &QueryKey,
        ) -> QueryPoll<lrr::db::provider::Provided> {
            QueryPoll::Ready(Err(self
                .run
                .unit_diagnostics(db, self.unit)
                .expect_err("query provider read mutable helper diagnostics")))
        }
    }
    let denied_key = QueryKey::Constant(fixture_key(
        Digest([193; 32]),
        vec![],
        Digest([194; 32]),
        ProfileIdentity::current(),
    ));
    let QueryPoll::Ready(Err(error)) = db.fetch(&DiagnosticQuery { run: &run, unit }, denied_key)
    else {
        panic!("query diagnostic access was not refused")
    };
    assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
    assert_eq!(
        usize::try_from(run.unit_invocations(unit)).unwrap(),
        expected_count
    );
    assert!(!diagnostics.protocol_truncated && !diagnostics.stderr_truncated);
    fs::write(base.join("server.stderr"), &diagnostics.stderr_preview).unwrap();
    fs::write(base.join("server.protocol"), &diagnostics.protocol_preview).unwrap();
    let printed: Vec<_> = String::from_utf8_lossy(&diagnostics.stderr_preview)
        .lines()
        .filter(|l| l.starts_with("Derive "))
        .map(str::to_owned)
        .collect();
    let expected: Vec<_> = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.stdout"
    )
    .lines()
    .collect();
    assert_eq!(printed.len(), 5);
    for ((actual, expected), input) in printed.iter().zip(expected).zip(payloads) {
        let (actual_name, actual_tokens) = actual.split_once(": ").unwrap();
        let (expected_name, expected_tokens) = expected.split_once(": ").unwrap();
        assert_eq!(actual_name, expected_name);
        let parse = |text: &str| {
            token_wire::parse_source(
                text,
                SourceIdentity {
                    unit: Key(unit.0),
                    content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
                    path: "printed.rs".into(),
                },
                ContextRecord {
                    parent: None,
                    expansion: None,
                    transparency: Transparency::Opaque,
                    defining_unit: Key(unit.0),
                    edition: Edition::E2024,
                },
                Key([191; 32]),
            )
            .unwrap()
        };
        assert_eq!(
            derive_print_shape(parse(actual_tokens).roots()),
            derive_print_shape(parse(expected_tokens).roots())
        );
        assert_eq!(
            derive_print_shape(input.roots()),
            derive_print_shape(parse(expected_tokens).roots())
        );
    }
    assert_eq!(
        fs::read(evidence.join("app/src/lib.rs")).unwrap(),
        original.as_bytes()
    );
    assert_eq!(
        fs::read(macro_source.join("src/lib.rs")).unwrap(),
        auxiliary
    );
    fs::write(base.join("worklist.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "upstream_case":"tests/ui/proc-macro/derive-expand-order.rs","native_invocations":run.unit_invocations(unit),"exports":exports,"occurrences":occurrences,"appended":appended,
        "original_source_unchanged":true,"macro_host_source_unchanged":true,"source_adaptations":[],"foo_canonical_identity":expected_foo,
        "sealed":true,"published":true,"published_foo_ready":true,"guest_execution":false,"full_fixture_acceptance":false,"diagnostics_channel":"stderr"
        ,"printed":printed,"macro_host_declaration_expansions":run.unit_declaration_expansions(unit),"definition_edition":2015,"call_edition":2024
    })).unwrap()).unwrap();
}

#[test]
fn prepared_server_source_edit_refuses_macro_run_before_guest_macro_execution() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        project::tool_manifest::PreparedFiles,
        service::{expansion::DemandContext, run::MacroRun},
    };
    let prepared = prepared();
    let evidence = prepared.work.join("prepared-server-source-edit");
    let mut manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    adopt_fixture_prepared_server(&mut manifest, &evidence);
    let copied_root = evidence.join("copied-provenance");
    fs::create_dir(&copied_root).unwrap();
    let record = manifest.toolchain.prepared_macro_server.as_mut().unwrap();
    let original_root = PathBuf::from(&record.source_root);
    let copied_sources: BTreeMap<_, _> = record
        .sources
        .iter()
        .map(|(path, digest)| {
            let destination = copied_root.join(
                std::path::Path::new(path)
                    .strip_prefix(&original_root)
                    .unwrap(),
            );
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(path, &destination).unwrap();
            (destination.to_str().unwrap().to_owned(), *digest)
        })
        .collect();
    record.source_root = copied_root.to_str().unwrap().into();
    record.sources = copied_sources;
    let binary =
        copied_root.join("tools/prepared_macro_server/target/debug/rust-analyzer-proc-macro-srv");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::copy(&manifest.toolchain.macro_server.path, &binary).unwrap();
    manifest.toolchain.macro_server.path = binary.to_str().unwrap().into();
    manifest.validate().unwrap();
    PreparedFiles::capture(&manifest).unwrap();
    fs::write(
        copied_root.join("tools/prepared_macro_server/src/main.rs"),
        "changed after explicit preparation",
    )
    .unwrap();
    assert_eq!(
        PreparedFiles::capture(&manifest).unwrap_err().code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
    let unit = *manifest.graph.roots.first().unwrap();
    let mut db = lrr::db::memo::Database::default();
    db.begin_expansion_epoch(1).unwrap();
    let context = DemandContext {
        request: Digest([181; 32]),
        session: Digest([182; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let plan = recorded_macro_plan(&evidence, &prepared.config.directory);
    let error = match MacroRun::begin(&mut db, context, &manifest, &plan, true) {
        Err(error) => error,
        Ok(_) => panic!("edited prepared server provenance was admitted"),
    };
    assert_eq!(error.code, "LRR-E-TOOL-PREPARE-STALE");
    fs::write(evidence.join("source-edit-refusal.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "diagnostic":error.code,"guest_macro_invocations":0,"compatibility_probe_executed_during_explicit_preparation":true,
        "macro_run_created":false,"published":false,"full_fixture_acceptance":false
    })).unwrap()).unwrap();
}

#[test]
fn matched_server_preparation_requires_explicit_tool_authority() {
    // Synchronize the separately authorized fixture preparation before measuring refusal.
    let _ = prepared_matched_server();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let receipt = root.join("artifacts/macos-first/m013-prepared-server-preparation.json");
    let before = fs::read(&receipt).ok();
    let result = Command::new("python3")
        .arg(root.join("tools/prepare_macro_server.py"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("explicit tool-preparation authority is required")
    );
    assert!(result.stdout.is_empty());
    assert_eq!(before, fs::read(&receipt).ok());
}

#[test]
fn rustc_tracked_environment_macro_uses_recorded_values_and_missing_fallback() {
    let prepared = prepared();
    let root = prepared.work.join("official-tracked-environment-prepared");
    fs::create_dir_all(root.join("src")).unwrap();
    let auxiliary = include_bytes!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/env.rs"
    );
    fs::write(root.join("src/lib.rs"), auxiliary).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname=\"env\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    for (label, args) in [
        ("lock", vec!["generate-lockfile", "--offline"]),
        (
            "build",
            vec!["build", "--locked", "--offline", "--message-format=json"],
        ),
    ] {
        let result = Command::new("cargo")
            .arg("+1.99.0")
            .args(args)
            .arg("--manifest-path")
            .arg(root.join("Cargo.toml"))
            .env("RUSTC_BOOTSTRAP", "1")
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_INCREMENTAL", "0")
            .output()
            .unwrap();
        fs::write(root.join(format!("{label}.stdout")), &result.stdout).unwrap();
        fs::write(root.join(format!("{label}.stderr")), &result.stderr).unwrap();
        assert!(
            result.status.success(),
            "unchanged macro-host preparation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let messages: Vec<Value> = fs::read_to_string(root.join("build.stdout"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let artifacts: Vec<_> = messages
        .iter()
        .filter(|m| m["reason"] == "compiler-artifact")
        .collect();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(
        artifacts[0]["target"]["kind"],
        serde_json::json!(["proc-macro"])
    );
    let filenames = artifacts[0]["filenames"].as_array().unwrap();
    assert_eq!(filenames.len(), 1);
    let path = PathBuf::from(filenames[0].as_str().unwrap());
    let artifact = MacroArtifact {
        identity: Key([175; 32]),
        raw_blake3: blake3::hash(&fs::read(&path).unwrap()).to_hex().to_string(),
        path,
    };
    let mut rows = Vec::new();
    for (case, environment, expected) in [
        (
            "original",
            BTreeMap::from([("THE_CONST".into(), "1".into())]),
            ["1", "2"],
        ),
        ("missing", BTreeMap::new(), ["0", "2"]),
        (
            "both",
            BTreeMap::from([
                ("THE_CONST".into(), "12".into()),
                ("ANOTHER".into(), "present".into()),
            ]),
            ["12", "1"],
        ),
    ] {
        let mut config = prepared.config.clone();
        config.server = prepared_matched_server().clone();
        config.directory = root.clone();
        config.artifacts = vec![artifact.clone()];
        config.environment = environment;
        let mut session = ProcSession::new(config.clone()).unwrap();
        let request = invocation(
            &config,
            176,
            "generate_const",
            ProcForm::FunctionLike,
            ProcGrammar::Items,
            "",
        );
        let output = session.expand(&request).unwrap();
        assert_eq!(session.invocations(), 1);
        let [TokenTree::Group { children, .. }] = output.roots() else {
            panic!("output root absent")
        };
        let constants: Vec<_> = children
            .windows(6)
            .filter_map(|tokens| match tokens {
                [
                    TokenTree::Ident { text: keyword, .. },
                    TokenTree::Ident { text: name, .. },
                    TokenTree::Punct { scalar: ':', .. },
                    TokenTree::Ident { text: ty, .. },
                    TokenTree::Punct { scalar: '=', .. },
                    TokenTree::Literal { token_text, .. },
                ] if keyword == "const" && ty == "u32" => {
                    Some((name.as_str(), token_text.as_str()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            constants,
            vec![("THE_CONST", expected[0]), ("ANOTHER", expected[1])]
        );
        fs::write(
            root.join(format!("{case}.lrrtok")),
            output.encode().unwrap(),
        )
        .unwrap();
        session.cancel();
        let snapshot = session.diagnostic_snapshot().unwrap();
        assert!(!snapshot.protocol_truncated && !snapshot.stderr_truncated);
        rows.push(serde_json::json!({"case":case,"environment":config.environment,"constants":constants,"native_invocations":session.invocations()}));
    }
    assert_eq!(fs::read(root.join("src/lib.rs")).unwrap(), auxiliary);
    fs::write(root.join("tracked-environment.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "upstream_case":"tests/ui/proc-macro/env.rs", "upstream_auxiliary":"tests/ui/proc-macro/auxiliary/env.rs",
        "rows":rows,"native_invocations":3,"macro_host_source_unchanged":true,
        "preparation_role":"proc-macro-host-only","rustc_bootstrap_scope":"unstable upstream macro host feature only",
        "server":prepared_matched_server().path,"server_raw_blake3":prepared_matched_server().raw_blake3,
        "protocol":6,"production_adoption":false,"guest_execution":false,"full_fixture_acceptance":false
    })).unwrap()).unwrap();
}

#[test]
fn cold_procedural_request_assembly_preserves_all_exact_grammar_slots_without_execution() {
    use lrr::{
        core::digest::{Digest, StableEncode},
        expand::identity::GrammarSlot,
        service::{
            expansion::DemandContext,
            item_macro_requests::{PreparedItemRequest, ResolvedItemTarget},
            macro_session::{MacroSession, macro_artifact_identity},
        },
    };
    let prepared = prepared();
    let manifest = service_fixture::prepare(
        &prepared.work.join("cold-procedural-slots"),
        &prepared.config.directory,
    );
    let unit = *manifest.graph.roots.first().unwrap();
    let (tool, artifact) = manifest.artifacts.iter().next().unwrap();
    let mut config = prepared.config.clone();
    config.unit = Key(unit.0);
    config.artifacts[0].identity = Key(macro_artifact_identity(*tool, artifact).0);
    let context = DemandContext {
        request: Digest([121; 32]),
        session: Digest([122; 32]),
        unit,
        revision: 0,
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let service = MacroSession::open(
        context.clone(),
        &manifest,
        true,
        PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]),
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .unwrap();
    for (ordinal, grammar, slot, text) in [
        (
            1,
            ProcGrammar::Expression,
            GrammarSlot::Expression,
            "{impl Local{}true}",
        ),
        (2, ProcGrammar::Items, GrammarSlot::Items, "impl Local{}"),
        (3, ProcGrammar::Type, GrammarSlot::Type, "[i32; 3]"),
        (4, ProcGrammar::Pattern, GrammarSlot::Pattern, "Some(value)"),
        (
            5,
            ProcGrammar::Statements,
            GrammarSlot::Statements,
            "impl Local{}",
        ),
        (
            6,
            ProcGrammar::TraitItems,
            GrammarSlot::TraitItems,
            "fn value(&self);",
        ),
        (
            7,
            ProcGrammar::ImplItems,
            GrammarSlot::ImplItems,
            "fn value(&self){}",
        ),
    ] {
        let request = invocation(
            &config,
            121 + ordinal,
            "identity",
            ProcForm::FunctionLike,
            grammar,
            text,
        );
        let runtime =
            lrr::syntax::token_wire::TokenForest::decode(&request.input.encode().unwrap()).unwrap();
        let target = ResolvedItemTarget {
            artifact: request.artifact,
            export: request.export.clone(),
            scope: request.scope,
            parent_origin: request.parent_origin,
            registry: &request.input,
            definition_site: request.definition_site,
            call_site: request.call_site,
            mixed_site: request.mixed_site,
            sources: request.sources.clone(),
        };
        let assembled = PreparedItemRequest::function_like(
            &service,
            &context,
            Digest(request.identity.0),
            &runtime,
            slot,
            target,
        )
        .unwrap();
        assert_eq!(assembled.key().grammar, slot);
        assert_eq!(
            assembled.key(),
            &service.request_key(&context, &request).unwrap()
        );
        assert_eq!(assembled.request().input, request.input);
        assert_eq!(service.invocations(), 0);
    }
}

#[test]
fn cold_prepared_procedural_source_selection_is_external_and_preserves_grammar() {
    use lrr::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        db::key::OutlineKey,
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        service::{expansion::DemandContext, macro_session::MacroSession},
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            manifest::Namespace,
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    let prepared = prepared();
    let manifest = service_fixture::prepare(
        &prepared.work.join("cold-source-selection"),
        &prepared.config.directory,
    );
    let unit = *manifest.graph.roots.first().unwrap();
    let mut context = DemandContext {
        request: Digest([131; 32]),
        session: Digest([132; 32]),
        unit,
        revision: 0,
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let text =
        "struct Global;fn cold(){lrr_fixture_macros::identity!({impl Global{}true});missing();}";
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
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
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    context.revision = db.source_revision();
    let mut service = MacroSession::open(
        context.clone(),
        &manifest,
        true,
        PathBuf::from(&manifest.compile_env[&unit].cargo["CARGO_MANIFEST_DIR"]),
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )
    .unwrap();
    let mut work = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Type,
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_declarative_procedural(
        context.compile_environment,
        Digest([133; 32]),
        Arc::new(service.export_catalog(&context).unwrap()),
    )
    .unwrap();
    let SourceDiscoveryStep::NeedProceduralSelection { selection } = work.poll().unwrap() else {
        panic!("cold procedural selection absent")
    };
    assert_eq!(
        selection.input().grammar(),
        lrr::expand::identity::GrammarSlot::Statements
    );
    assert_eq!(service.invocations(), 0);
    assert!(work.sealed_expansions().is_err());
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::NeedProceduralSelection { .. }
    ));
    let registry = TokenForest::decode(&selection.input().arguments().encode().unwrap()).unwrap();
    let (producer, artifact) = manifest.artifacts.iter().next().unwrap();
    let target = lrr::service::item_macro_requests::ResolvedItemTarget {
        artifact: Key(lrr::service::macro_session::macro_artifact_identity(*producer, artifact).0),
        export: "identity".into(),
        scope: Key(selection.input().scope().fingerprint().0),
        parent_origin: selection.input().parent_origin(),
        registry: &registry,
        definition_site: selection.input().call_site(),
        call_site: selection.input().call_site(),
        mixed_site: selection.input().call_site(),
        sources: vec![],
    };
    let request = Arc::new(selection.prepare(&service, &context, target).unwrap());
    work.bind_procedural_request(request.clone()).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request: pending } = work.poll().unwrap() else {
        panic!("bound cold request absent")
    };
    assert_eq!(pending.key(), request.key());
    let mut coordinator = lrr::service::expansion::ExpansionCoordinator::new(context.clone());
    let token = coordinator
        .issue(
            lrr::db::key::QueryKey::CurrentExpansionInput(Box::new(request.key().clone())),
            lrr::service::expansion::PhaseIdentity {
                frame: Digest([134; 32]),
                body: lrr::source::manifest::BodyKey {
                    definition: Digest([135; 32]),
                    exact_body_bytes: Digest([136; 32]),
                },
                index: 0,
            },
            lrr::db::poll::InputRequest::Expand(Box::new(lrr::db::poll::ExpansionRequest {
                key: request.key().clone(),
            })),
        )
        .unwrap();
    let record = service
        .fulfil(&context, &mut coordinator, &token, request.request())
        .unwrap();
    assert!(work.admit_expansion(Arc::new(record.clone())).is_err());
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);

    coordinator
        .adopt(&context, &token, &mut db, record.clone())
        .unwrap();
    work.admit_procedural_expansion(&mut db, Arc::new(record))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
    assert_eq!(service.invocations(), 1);
    assert!(
        work.publish(&mut db)
            .unwrap_err()
            .message
            .contains("final macro run input verification")
    );
}

#[test]
fn cold_procedural_macro_run_certifies_nested_record_before_publication() {
    cold_procedural_run_case(false, false, false, false, false, false);
}
#[test]
fn cold_procedural_source_alias_retains_exact_export_and_revalidation() {
    cold_procedural_run_case(true, false, false, false, false, false);
}
#[test]
fn cold_procedural_source_edit_refuses_final_verified_publication() {
    cold_procedural_run_case(false, true, false, false, false, false);
}
#[test]
fn cold_source_user_macro_shadows_prepared_procedural_alias_without_native_execution() {
    cold_procedural_run_case(false, false, true, false, false, false);
}
#[test]
fn cold_generated_procedural_call_retains_actual_emission_and_publishes_verified_impl() {
    cold_procedural_run_case(false, false, false, true, false, false);
}
#[test]
fn cold_generated_procedural_source_edit_refuses_final_publication() {
    cold_procedural_run_case(false, true, false, true, false, false);
}
#[test]
fn cold_generated_user_macro_shadows_prepared_alias_without_native_execution() {
    cold_procedural_run_case(false, false, true, true, false, false);
}
#[test]
fn cold_original_intrinsic_argument_procedural_call_discovers_verified_impl() {
    cold_procedural_argument_case();
}
#[test]
fn cold_original_intrinsic_argument_procedural_source_edit_refuses_publication() {
    cold_procedural_run_case(false, true, false, false, true, false);
}
#[test]
fn cold_original_intrinsic_argument_user_macro_shadows_prepared_alias() {
    cold_procedural_run_case(false, false, true, false, true, false);
}
#[test]
fn cold_generated_intrinsic_argument_procedural_call_preserves_emitting_record() {
    cold_procedural_run_case(false, false, false, true, true, false);
}
#[test]
fn cold_original_intrinsic_argument_procedural_alias_keeps_exact_export() {
    cold_procedural_run_case(true, false, false, false, true, false);
}
#[test]
fn item_generated_cold_procedural_impl_requires_explicit_macro_run_before_publication() {
    cold_procedural_run_case(false, false, false, true, false, true);
}
fn cold_procedural_argument_case() {
    cold_procedural_run_case(false, false, false, false, true, false);
}
fn cold_procedural_run_case(
    alias: bool,
    stale: bool,
    shadow: bool,
    generated: bool,
    argument: bool,
    item_output: bool,
) {
    use lrr::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        db::key::OutlineKey,
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        service::expansion::DemandContext,
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            manifest::Namespace,
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    let prepared = prepared();
    let evidence = prepared.work.join(if item_output {
        "cold-item-generated-procedural"
    } else if argument && generated {
        "cold-generated-procedural-argument"
    } else if argument && alias {
        "cold-procedural-argument-alias"
    } else if argument && shadow {
        "cold-procedural-argument-shadow"
    } else if argument && stale {
        "cold-procedural-argument-stale"
    } else if argument {
        "cold-procedural-argument"
    } else if generated && shadow {
        "cold-generated-shadow"
    } else if generated && stale {
        "cold-generated-stale"
    } else if generated {
        "cold-generated-run"
    } else if shadow {
        "cold-source-run-shadow"
    } else if stale {
        "cold-source-run-stale"
    } else if alias {
        "cold-source-run-alias"
    } else {
        "cold-source-run"
    });
    let manifest = service_fixture::prepare(&evidence, &prepared.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let mut context = DemandContext {
        request: Digest([131; 32]),
        session: Digest([132; 32]),
        unit,
        revision: 0,
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let text = if item_output {
        "struct Global;macro_rules! make{()=>{fn cold(){lrr_fixture_macros::identity!({impl Global{}true});missing();}}}make!();"
    } else if argument && generated {
        "struct Global;macro_rules! make{()=>{{assert!({lrr_fixture_macros::identity!({impl Global{}true})});}}}fn cold(){make!();missing();}"
    } else if argument && alias {
        "use lrr_fixture_macros::identity as chosen;struct Global;fn cold(){assert!({chosen!({impl Global{}true})});missing();}"
    } else if argument && shadow {
        "use lrr_fixture_macros::identity as chosen;struct Global;fn cold(){assert!({macro_rules! chosen{($($t:tt)*)=>{{impl Global{}true}}}chosen!({ignored})});missing();}"
    } else if argument {
        "struct Global;fn cold(){assert!({lrr_fixture_macros::identity!({impl Global{}true})});missing();}"
    } else if generated && shadow {
        "use lrr_fixture_macros::identity as chosen;struct Global;macro_rules! make{()=>{{macro_rules! chosen{()=>{{impl Global{}true}}}chosen!();}}}fn cold(){make!();missing();}"
    } else if generated {
        "struct Global;macro_rules! make{()=>{{lrr_fixture_macros::identity!({impl Global{}true});}}}fn cold(){make!();missing();}"
    } else if shadow {
        "use lrr_fixture_macros::identity as chosen;struct Global;macro_rules! chosen{($($t:tt)*)=>{{impl Global{}true}}}fn cold(){chosen!({ignored});missing();}"
    } else if alias {
        "use lrr_fixture_macros::identity as chosen;struct Global;fn cold(){chosen!({impl Global{}true});missing();}"
    } else {
        "struct Global;fn cold(){lrr_fixture_macros::identity!({impl Global{}true});missing();}"
    };
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
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
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    context.revision = db.source_revision();
    let plan = recorded_macro_plan(&evidence, &prepared.config.directory);
    let mut run =
        lrr::service::run::MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true)
            .unwrap();
    let catalog = run.catalog(&mut db, unit).unwrap();
    let mut work = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Type,
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_declarative_procedural(context.compile_environment, Digest([133; 32]), catalog)
    .unwrap()
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap()
    .with_compiler(Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &ProfileIdentity::current(),
        )
        .unwrap(),
    ))
    .unwrap();
    if generated {
        let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
            panic!("outer macro absent")
        };
        work.admit_expansion(Arc::new(request.expand().unwrap()))
            .unwrap();
    }
    if shadow {
        let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
            panic!("user macro demand absent")
        };
        assert!(request.procedural_request().is_none());
        work.admit_expansion(Arc::new(request.expand().unwrap()))
            .unwrap();
        assert!(matches!(
            work.poll().unwrap(),
            SourceDiscoveryStep::Sealed(_)
        ));
        assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
        assert_eq!(run.unit_invocations(unit), 0);
        run.publish_discovery(&mut db, &mut work).unwrap();
        return;
    }
    let SourceDiscoveryStep::NeedProceduralSelection { selection } = work.poll().unwrap() else {
        panic!("cold procedural selection absent")
    };
    assert_eq!(
        selection.input().grammar(),
        if argument {
            lrr::expand::identity::GrammarSlot::Expression
        } else {
            lrr::expand::identity::GrammarSlot::Statements
        }
    );
    if argument && !generated {
        assert_eq!(
            selection.input().parent_origin().0,
            lrr::db::key::QueryKey::Outline(view.selection().outline.clone())
                .fingerprint()
                .0
        );
        assert!(
            selection
                .input()
                .arguments()
                .spans()
                .iter()
                .all(|span| span.source.content.0 == view.selection().outline.parse.content.0)
        );
    }
    assert_eq!(run.unit_invocations(unit), 0);
    assert!(work.sealed_expansions().is_err());
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::NeedProceduralSelection { .. }
    ));
    let registry = TokenForest::decode(&selection.input().arguments().encode().unwrap()).unwrap();
    let (producer, artifact) = manifest.artifacts.iter().next().unwrap();
    let target = lrr::service::item_macro_requests::ResolvedItemTarget {
        artifact: Key(lrr::service::macro_session::macro_artifact_identity(*producer, artifact).0),
        export: "identity".into(),
        scope: Key(selection.input().scope().fingerprint().0),
        parent_origin: selection.input().parent_origin(),
        registry: &registry,
        definition_site: selection.input().call_site(),
        call_site: selection.input().call_site(),
        mixed_site: selection.input().call_site(),
        sources: vec![],
    };
    let request = run
        .prepare_source_selection(&mut db, &selection, target)
        .unwrap();
    work.bind_procedural_request(request.clone()).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request: pending } = work.poll().unwrap() else {
        panic!("bound cold request absent")
    };
    assert_eq!(pending.key(), request.key());
    let phase = lrr::service::expansion::PhaseIdentity {
        frame: Digest([134; 32]),
        body: lrr::source::manifest::BodyKey {
            definition: Digest([135; 32]),
            exact_body_bytes: Digest([136; 32]),
        },
        index: 0,
    };
    let mut cache = lrr::expand::cache::ExpansionOutputCache::new(64 * 1024 * 1024).unwrap();
    run.fulfil_discovery(&mut db, &mut work, phase, &request, &mut cache)
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(
        work.accounted_expansion_count().unwrap(),
        1 + usize::from(generated)
    );
    assert_eq!(run.unit_invocations(unit), 1);
    assert!(work.publish(&mut db).is_err());
    if stale {
        let input = evidence.join("app/src/lib.rs");
        let original = fs::read_to_string(&input).unwrap();
        fs::write(
            &input,
            format!("{original}\n// changed after cold expansion\n"),
        )
        .unwrap();
        assert!(run.publish_discovery(&mut db, &mut work).is_err());
        assert_eq!(run.unit_invocations(unit), 1);
        assert!(
            db.input(&lrr::db::dependency::InputKey::DiscoverySeal(
                work.key().clone()
            ))
            .is_none()
        );
    } else {
        let certificate = run.publish_discovery(&mut db, &mut work).unwrap();
        assert_eq!(certificate.unit_count(), 1);
    }
}

#[test]
fn procedural_test_catalog_uses_final_attributes_and_never_reexecutes_on_listing() {
    procedural_test_catalog_case(false);
}
#[test]
fn procedural_test_catalog_required_error_prevents_filtered_listing() {
    procedural_test_catalog_case(true);
}
#[test]
fn procedural_test_catalog_attribute_named_test_requires_kind_resolution() {
    procedural_test_catalog_variant(TestCatalogVariant::RootAttribute);
}
fn procedural_test_catalog_case(failing: bool) {
    procedural_test_catalog_variant(if failing {
        TestCatalogVariant::RequiredError
    } else {
        TestCatalogVariant::FinalAttributes
    });
}
#[test]
fn procedural_test_catalog_generated_attribute_named_test_requires_kind_resolution() {
    procedural_test_catalog_variant(TestCatalogVariant::GeneratedAttribute);
}
#[test]
fn procedural_test_catalog_generated_test_keeps_function_like_macro_name_shadow() {
    procedural_test_catalog_variant(TestCatalogVariant::GeneratedBangShadow);
}
#[test]
fn procedural_test_catalog_included_attribute_named_test_requires_kind_resolution() {
    procedural_test_catalog_variant(TestCatalogVariant::IncludedAttribute);
}
#[test]
fn procedural_test_catalog_generated_consumed_test_rejects_removed_registration() {
    procedural_test_catalog_variant(TestCatalogVariant::LeadingMarker);
}
#[test]
fn procedural_test_catalog_generated_consumed_test_rejects_renamed_registration() {
    procedural_test_catalog_variant(TestCatalogVariant::GeneratedLeadingRenamed);
}
#[test]
fn procedural_test_catalog_consumed_root_test_rejects_removed_registration() {
    procedural_test_catalog_variant(TestCatalogVariant::RootLeadingRemoved);
}
#[test]
fn procedural_test_catalog_consumed_root_test_rejects_renamed_registration() {
    procedural_test_catalog_variant(TestCatalogVariant::RootLeadingRenamed);
}
#[test]
fn procedural_test_catalog_staged_physical_attribute_named_test_requires_kind_resolution() {
    procedural_test_catalog_variant(TestCatalogVariant::PhysicalAttribute);
}
#[test]
fn procedural_test_catalog_staged_physical_keeps_function_like_macro_name_shadow() {
    procedural_test_catalog_variant(TestCatalogVariant::PhysicalBangShadow);
}
#[test]
fn procedural_test_catalog_staged_physical_required_error_prevents_listing() {
    procedural_test_catalog_variant(TestCatalogVariant::PhysicalError);
}
#[test]
fn procedural_test_catalog_staged_physical_replacing_attribute_retires_test_item() {
    procedural_test_catalog_variant(TestCatalogVariant::PhysicalReplacement);
}
#[test]
fn procedural_test_catalog_cold_nested_attribute_named_test_requires_kind_resolution() {
    procedural_test_catalog_variant(TestCatalogVariant::ColdAttribute);
}
#[test]
fn procedural_test_catalog_cold_nested_keeps_function_like_macro_name_shadow() {
    procedural_test_catalog_variant(TestCatalogVariant::ColdBangShadow);
}
#[test]
fn procedural_test_catalog_cold_nested_keeps_plain_compiler_test_marker() {
    procedural_test_catalog_variant(TestCatalogVariant::ColdPlainMarker);
}
#[test]
fn procedural_test_catalog_cold_nested_required_error_prevents_listing() {
    procedural_test_catalog_variant(TestCatalogVariant::ColdError);
}
#[test]
fn procedural_test_catalog_cold_nested_local_import_selects_actual_attribute() {
    procedural_test_catalog_variant(TestCatalogVariant::ColdLocalImport);
}
#[test]
fn procedural_test_catalog_cold_nested_unknown_import_remains_pending() {
    procedural_test_catalog_variant(TestCatalogVariant::ColdUnresolvedLocalImport);
}
#[test]
fn procedural_test_catalog_cold_nested_conflicting_imports_refuse_before_invocation() {
    procedural_test_catalog_variant(TestCatalogVariant::ColdAmbiguousLocalImport);
}

#[test]
fn procedural_test_catalog_generated_cold_attribute_named_test_requires_kind_resolution() {
    procedural_test_catalog_variant(TestCatalogVariant::GeneratedColdAttribute);
}
#[test]
fn procedural_test_catalog_generated_cold_required_error_prevents_listing() {
    procedural_test_catalog_variant(TestCatalogVariant::GeneratedColdError);
}
#[test]
fn procedural_test_catalog_generated_cold_keeps_function_like_shadow() {
    procedural_test_catalog_variant(TestCatalogVariant::GeneratedColdBangShadow);
}
#[test]
fn procedural_test_catalog_generated_cold_keeps_plain_compiler_marker() {
    procedural_test_catalog_variant(TestCatalogVariant::GeneratedColdPlainMarker);
}
#[test]
fn procedural_test_catalog_generated_cold_replacement_retires_inner_impl() {
    procedural_test_catalog_variant(TestCatalogVariant::GeneratedColdImpl);
}
#[derive(Clone, Copy)]
enum TestCatalogVariant {
    FinalAttributes,
    RequiredError,
    RootAttribute,
    GeneratedAttribute,
    GeneratedColdAttribute,
    GeneratedColdError,
    GeneratedColdBangShadow,
    GeneratedColdPlainMarker,
    GeneratedColdImpl,
    GeneratedBangShadow,
    IncludedAttribute,
    LeadingMarker,
    GeneratedLeadingRenamed,
    RootLeadingRemoved,
    RootLeadingRenamed,
    PhysicalAttribute,
    PhysicalBangShadow,
    PhysicalError,
    PhysicalReplacement,
    ColdAttribute,
    ColdBangShadow,
    ColdPlainMarker,
    ColdError,
    ColdLocalImport,
    ColdUnresolvedLocalImport,
    ColdAmbiguousLocalImport,
}
fn procedural_test_catalog_variant(variant: TestCatalogVariant) {
    procedural_test_catalog_variant_with_selected_async(variant, false);
}
#[test]
fn procedural_test_catalog_selected_generated_async_fails_before_body_without_macro_reexecution() {
    procedural_test_catalog_variant_with_selected_async(TestCatalogVariant::FinalAttributes, true);
}
fn procedural_test_catalog_variant_with_selected_async(
    variant: TestCatalogVariant,
    selected_async: bool,
) {
    use TestCatalogVariant::*;
    let failing = matches!(variant, RequiredError | PhysicalError | ColdError);
    let shadow_test = matches!(
        variant,
        RootAttribute
            | GeneratedColdAttribute
            | GeneratedColdError
            | GeneratedColdImpl
            | GeneratedAttribute
            | IncludedAttribute
            | LeadingMarker
            | GeneratedLeadingRenamed
            | PhysicalAttribute
            | PhysicalError
            | PhysicalReplacement
            | ColdAttribute
            | ColdError
            | ColdLocalImport
    );
    let generated_test = matches!(
        variant,
        GeneratedAttribute
            | GeneratedColdAttribute
            | GeneratedColdError
            | GeneratedColdBangShadow
            | GeneratedColdPlainMarker
            | GeneratedColdImpl
            | GeneratedBangShadow
            | IncludedAttribute
            | LeadingMarker
            | GeneratedLeadingRenamed
            | PhysicalReplacement
    );
    let bang_shadow = matches!(variant, GeneratedBangShadow | PhysicalBangShadow);
    let included_test = matches!(variant, IncludedAttribute);
    let leading_marker = matches!(variant, LeadingMarker);
    let physical_module = matches!(
        variant,
        PhysicalAttribute | PhysicalBangShadow | PhysicalError | PhysicalReplacement
    );
    let cold_attribute = matches!(
        variant,
        ColdAttribute
            | ColdBangShadow
            | ColdPlainMarker
            | ColdError
            | ColdLocalImport
            | ColdUnresolvedLocalImport
            | ColdAmbiguousLocalImport
    );
    use lrr::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        db::{
            key::{OutlineKey, QueryKey},
            memo::Database,
        },
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        service::{
            expansion::{DemandContext, PhaseIdentity},
            run::MacroRun,
        },
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            manifest::BodyKey,
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    let fixture = prepared();
    let evidence = fixture.work.join(if selected_async {
        "m015-test-catalog-selected-async"
    } else if matches!(
        variant,
        GeneratedColdAttribute
            | GeneratedColdError
            | GeneratedColdBangShadow
            | GeneratedColdPlainMarker
            | GeneratedColdImpl
    ) {
        match variant {
            GeneratedColdError => "m015-generated-cold-error",
            GeneratedColdBangShadow => "m015-generated-cold-bang",
            GeneratedColdPlainMarker => "m015-generated-cold-plain",
            GeneratedColdImpl => "m015-generated-cold-impl",
            _ => "m015-generated-cold-attribute",
        }
    } else if cold_attribute {
        match variant {
            ColdBangShadow => "m015-test-catalog-cold-bang",
            ColdPlainMarker => "m015-test-catalog-cold-plain",
            ColdError => "m015-test-catalog-cold-error",
            ColdLocalImport => "m015-test-catalog-cold-local",
            ColdUnresolvedLocalImport => "m015-test-catalog-cold-unknown-local",
            ColdAmbiguousLocalImport => "m015-test-catalog-cold-ambiguous-local",
            _ => "m015-test-catalog-cold-attribute",
        }
    } else if physical_module && failing {
        "m015-test-catalog-physical-error"
    } else if physical_module && bang_shadow {
        "m015-test-catalog-physical-bang-shadow"
    } else if physical_module && generated_test {
        "m015-test-catalog-physical-replacement"
    } else if physical_module {
        "m015-test-catalog-physical-module"
    } else if leading_marker {
        "m015-test-catalog-leading-marker"
    } else if matches!(variant, GeneratedLeadingRenamed) {
        "m015-test-catalog-generated-consumed-renamed"
    } else if matches!(variant, RootLeadingRemoved) {
        "m015-test-catalog-root-consumed-removed"
    } else if matches!(variant, RootLeadingRenamed) {
        "m015-test-catalog-root-consumed-renamed"
    } else if included_test {
        "m015-test-catalog-included-shadow-test"
    } else if bang_shadow {
        "m015-test-catalog-generated-bang-shadow"
    } else if generated_test {
        "m015-test-catalog-generated-shadow-test"
    } else if shadow_test {
        "m015-test-catalog-shadow-test"
    } else if failing {
        "m015-test-catalog-error"
    } else {
        "m015-test-catalog-final"
    });
    let manifest = if physical_module {
        service_fixture::prepare_sources(
            &evidence,
            &fixture.config.directory,
            &[(
                "child.rs",
                if failing {
                    "use lrr_fixture_macros::panic_attribute as test; #[test] fn removed(){}"
                } else if bang_shadow {
                    "use lrr_fixture_macros::identity as test; #[test] fn normal(){missing();}"
                } else if generated_test {
                    "use lrr_fixture_macros::attribute_tokens; #[attribute_tokens()] #[test] fn removed(){}"
                } else {
                    "use lrr_fixture_macros::attribute_tokens as test; #[test] fn removed(){}"
                },
            )],
        )
    } else if included_test {
        service_fixture::prepare_sources(
            &evidence,
            &fixture.config.directory,
            &[("piece.rs", "#[test] fn removed(){}")],
        )
    } else {
        service_fixture::prepare(&evidence, &fixture.config.directory)
    };
    let plan = recorded_macro_plan(&evidence, &fixture.config.directory);
    let unit = *manifest.graph.roots.first().unwrap();
    let text = if cold_attribute {
        match variant {
            ColdBangShadow => {
                "use lrr_fixture_macros::identity as test; fn cold(){#[test] fn inner(){deliberately_undefined();}}"
            }
            ColdPlainMarker => "fn cold(){#[test] fn inner(){deliberately_undefined();}}",
            ColdError => {
                "use lrr_fixture_macros::panic_attribute as test; fn cold(){#[test] fn removed(){deliberately_undefined();}}"
            }
            ColdLocalImport => {
                "fn cold(){use lrr_fixture_macros::attribute_tokens as test; #[test] fn inner(){deliberately_undefined();}}"
            }
            ColdUnresolvedLocalImport => {
                "fn cold(){use absent::attribute as test; #[test] fn inner(){deliberately_undefined();}}"
            }
            ColdAmbiguousLocalImport => {
                "fn cold(){use lrr_fixture_macros::attribute_tokens as test; use lrr_fixture_macros::panic_attribute as test; #[test] fn inner(){deliberately_undefined();}}"
            }
            _ => {
                "use lrr_fixture_macros::attribute_tokens as test; fn cold(){#[test] fn removed(){deliberately_undefined();}}"
            }
        }
    } else if matches!(variant, GeneratedColdError) {
        "use lrr_fixture_macros::attribute_tokens; use lrr_fixture_macros::panic_attribute as test; #[attribute_tokens(fn cold(){#[test] fn removed(){deliberately_undefined();}})] fn placeholder(){}"
    } else if matches!(variant, GeneratedColdBangShadow) {
        "use lrr_fixture_macros::attribute_tokens; use lrr_fixture_macros::identity as test; #[attribute_tokens(fn cold(){#[test] fn inner(){deliberately_undefined();}})] fn placeholder(){}"
    } else if matches!(variant, GeneratedColdPlainMarker) {
        "use lrr_fixture_macros::attribute_tokens; #[attribute_tokens(fn cold(){#[test] fn inner(){deliberately_undefined();}})] fn placeholder(){}"
    } else if matches!(variant, GeneratedColdImpl) {
        "struct Global;use lrr_fixture_macros::attribute_tokens; use lrr_fixture_macros::attribute_tokens as test; #[attribute_tokens(fn cold(){#[test] fn removed(){impl Global {}}})] fn placeholder(){}"
    } else if matches!(variant, GeneratedColdAttribute) {
        "use lrr_fixture_macros::attribute_tokens; use lrr_fixture_macros::attribute_tokens as test; #[attribute_tokens(fn cold(){#[test] fn removed(){deliberately_undefined();}})] fn placeholder(){}"
    } else if physical_module {
        "use lrr_fixture_macros::attribute_tokens; #[attribute_tokens(pub mod child;)] fn placeholder(){}"
    } else if matches!(variant, RootLeadingRemoved) {
        "use lrr_fixture_macros::attribute_tokens; #[test] #[attribute_tokens()] fn registered(){}"
    } else if matches!(variant, RootLeadingRenamed) {
        "use lrr_fixture_macros::attribute_tokens; #[test] #[attribute_tokens(fn renamed(){})] fn registered(){}"
    } else if matches!(variant, GeneratedLeadingRenamed) {
        "use lrr_fixture_macros::attribute_tokens; #[attribute_tokens(#[test] #[attribute_tokens(fn renamed(){})] fn registered(){})] fn placeholder(){}"
    } else if leading_marker {
        "use lrr_fixture_macros::attribute_tokens; #[attribute_tokens(#[test] #[attribute_tokens()] fn removed(){})] fn placeholder(){}"
    } else if included_test {
        "use lrr_fixture_macros::attribute_tokens; use lrr_fixture_macros::attribute_tokens as test; #[attribute_tokens(include!(\"piece.rs\");)] fn placeholder(){}"
    } else if bang_shadow {
        "use lrr_fixture_macros::attribute_tokens; use lrr_fixture_macros::identity as test; #[attribute_tokens(#[test] fn normal(){missing();})] fn placeholder(){}"
    } else if generated_test {
        "use lrr_fixture_macros::attribute_tokens; use lrr_fixture_macros::attribute_tokens as test; #[attribute_tokens(#[test] fn removed(){})] fn placeholder(){}"
    } else if shadow_test {
        "use lrr_fixture_macros::attribute_tokens as test; #[test] fn removed(){}"
    } else if failing {
        "use lrr_fixture_macros::panic_attribute; #[panic_attribute] #[test] fn removed(){} #[test] fn known(){}"
    } else {
        r#"use lrr_fixture_macros::attribute_tokens;
        #[attribute_tokens(
            #[test] fn normal(){}
            #[test] #[ignore="later"] fn ignored(){missing();}
            #[test] #[should_panic(expected="needle")] fn expected(){panic!("needle");}
            #[test] async fn asynchronous(){missing();}
        )]
        #[test] fn removed(){}
        #[test] fn plain(){}"#
    };
    let profile = ProfileIdentity::current();
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
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
    .with_compile_environment(Arc::new(environment.clone()))
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("test source absent")
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let context = DemandContext {
        request: Digest([171; 32]),
        session: Digest([172; 32]),
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
    let proc_catalog = run.catalog(&mut db, unit).unwrap();
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
    .with_declarative_compiler(context.compile_environment, Digest([173; 32]), compiler)
    .unwrap()
    .with_procedural_catalog(proc_catalog)
    .unwrap()
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap();
    assert!(discovery.test_catalog(&mut db).is_err());

    if matches!(
        variant,
        ColdUnresolvedLocalImport | ColdAmbiguousLocalImport
    ) {
        if matches!(variant, ColdUnresolvedLocalImport) {
            assert!(matches!(
                discovery.poll().unwrap(),
                SourceDiscoveryStep::Incomplete { .. }
            ));
        } else {
            assert_eq!(
                discovery
                    .poll()
                    .err()
                    .expect("ambiguous local imports were accepted")
                    .code,
                "LRR-E-MACRO-AMBIGUOUS"
            );
        }
        assert_eq!(run.unit_invocations(unit), 0);
        assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
        assert!(discovery.test_catalog(&mut db).is_err());
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
        return;
    }
    if matches!(variant, ColdBangShadow | ColdPlainMarker) {
        assert!(matches!(
            discovery.poll().unwrap(),
            SourceDiscoveryStep::Sealed(_)
        ));
        run.publish_discovery(&mut db, &mut discovery).unwrap();
        let catalog = discovery.test_catalog(&mut db).unwrap();
        assert!(catalog.cases().is_empty());
        for _ in 0..3 {
            catalog.require_current(&mut db).unwrap();
            assert!(discovery.test_catalog(&mut db).unwrap().cases().is_empty());
        }
        assert_eq!(run.unit_invocations(unit), 0);
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
        return;
    }
    let selection = match discovery.poll().unwrap() {
        SourceDiscoveryStep::NeedAttributeSelection { selection } => selection,
        SourceDiscoveryStep::Sealed(_) => panic!(
            "attribute selection absent: discovery sealed with {} native invocations",
            run.unit_invocations(unit)
        ),
        SourceDiscoveryStep::Incomplete {
            diagnostic,
            blocked,
        } => panic!("attribute selection absent: {diagnostic:?}, blocked={blocked:?}"),
        _ => panic!("attribute selection absent: different required producer"),
    };
    let request = run
        .prepare_attribute_selection(&mut db, &selection)
        .unwrap();
    assert_eq!(
        request.request().export,
        if failing && !physical_module {
            "panic_attribute"
        } else {
            "attribute_tokens"
        }
    );
    discovery.bind_attribute_request(request.clone()).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request: bound } = discovery.poll().unwrap() else {
        panic!("bound attribute request absent")
    };
    assert_eq!(bound.key(), request.key());
    assert_eq!(run.unit_invocations(unit), 0);
    assert!(discovery.test_catalog(&mut db).is_err());
    let phase = PhaseIdentity {
        frame: Digest([174; 32]),
        body: BodyKey {
            definition: Digest([175; 32]),
            exact_body_bytes: Digest([176; 32]),
        },
        index: 0,
    };
    let mut cache = lrr::expand::cache::ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let result = run.fulfil_discovery(&mut db, &mut discovery, phase, &request, &mut cache);
    if failing && !physical_module {
        let error = result.err().expect("required macro panic was suppressed");
        assert_eq!(error.code, "LRR-E-MACRO-PANIC");
        assert_eq!(run.unit_invocations(unit), 1);
        assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
        assert!(discovery.test_catalog(&mut db).is_err());
        fs::write(evidence.join("m015-test-catalog.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M015","diagnostic":error.code,"published":false,"native_invocations":1,"full_fixture_acceptance":false})).unwrap()).unwrap();
        return;
    }
    result.unwrap();
    let mut physical_input = None;
    if physical_module {
        use lrr::source::include_paths::{IncludeAuthorities, IncludeRoot, IncludeRootKind};
        assert!(matches!(
            discovery.poll().unwrap(),
            SourceDiscoveryStep::Incomplete { .. }
        ));
        let producer = discovery
            .unresolved_module_sources()
            .next()
            .unwrap()
            .producer();
        let root = fs::canonicalize(evidence.join("app/src")).unwrap();
        let make_authorities = || {
            IncludeAuthorities::new(
                unit,
                manifest.inputs.fingerprint(),
                [IncludeRoot {
                    identity: Digest([179; 32]),
                    path: root.clone(),
                    kind: IncludeRootKind::Source,
                }],
            )
            .unwrap()
        };
        let input = discovery
            .prepare_module_source(producer, &make_authorities(), std::path::Path::new(""))
            .unwrap();
        adopt_procedural_worklist_module(&mut db, &context, make_authorities(), &input);
        assert!(matches!(
            discovery.stage_module_source(&mut db, &input),
            lrr::db::poll::QueryPoll::Ready(Ok(_))
        ));
        if bang_shadow {
            assert!(matches!(
                discovery.poll().unwrap(),
                SourceDiscoveryStep::Incomplete { .. }
            ));
            assert_eq!(run.unit_invocations(unit), 1);
        } else {
            let SourceDiscoveryStep::NeedAttributeSelection { selection } =
                discovery.poll().unwrap()
            else {
                panic!("staged physical test attribute was retained without kind resolution");
            };
            let selected = run
                .prepare_attribute_selection(&mut db, &selection)
                .unwrap();
            assert_eq!(
                selected.request().export,
                if failing {
                    "panic_attribute"
                } else {
                    "attribute_tokens"
                }
            );
            discovery.bind_attribute_request(selected.clone()).unwrap();
            let SourceDiscoveryStep::NeedExpansion { request } = discovery.poll().unwrap() else {
                panic!("staged physical attribute request absent");
            };
            assert_eq!(request.key(), selected.key());
            let phase = PhaseIdentity {
                frame: Digest([180; 32]),
                body: BodyKey {
                    definition: Digest([181; 32]),
                    exact_body_bytes: Digest([182; 32]),
                },
                index: 1,
            };
            let result =
                run.fulfil_discovery(&mut db, &mut discovery, phase, &selected, &mut cache);
            if failing {
                assert_eq!(
                    result
                        .err()
                        .expect("required physical attribute error was suppressed")
                        .code,
                    "LRR-E-MACRO-PANIC"
                );
                assert_eq!(run.unit_invocations(unit), 2);
                assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
                assert!(discovery.test_catalog(&mut db).is_err());
                return;
            }
            result.unwrap();
        }
        physical_input = Some(input);
    }
    if included_test {
        use lrr::{
            db::poll::{ExpansionRequest, InputRequest},
            service::{
                eager_compile_inputs::EagerCompileInputPoll,
                expansion::{ExpansionCoordinator, Reply},
                source_capture::SourceCapture,
            },
            source::include_paths::{IncludeAuthorities, IncludeRoot, IncludeRootKind},
        };
        let root = fs::canonicalize(evidence.join("app/src")).unwrap();
        let authorities = IncludeAuthorities::new(
            unit,
            manifest.inputs.fingerprint(),
            [IncludeRoot {
                identity: Digest([177; 32]),
                path: root,
                kind: IncludeRootKind::Source,
            }],
        )
        .unwrap();
        let mut coordinator = ExpansionCoordinator::new(context.clone());
        let mut captured = 0;
        let mut completed = false;
        for index in 0..8 {
            let step = discovery.poll().unwrap();
            let key = match &step {
                SourceDiscoveryStep::NeedExpansion { request } if request.is_compiler() => {
                    request.key()
                }
                SourceDiscoveryStep::NeedGeneratedExpansion { request }
                    if request.is_compiler() =>
                {
                    request.key()
                }
                _ => {
                    completed = true;
                    break;
                }
            };
            let query = QueryKey::CurrentExpansionInput(Box::new(key.clone()));
            let phase = PhaseIdentity {
                frame: Digest([178; 32]),
                body: BodyKey {
                    definition: key.invocation,
                    exact_body_bytes: key.input_tokens,
                },
                index,
            };
            match discovery
                .poll_compiler(&mut db, &environment, &authorities)
                .unwrap()
            {
                EagerCompileInputPoll::NeedInput(input @ InputRequest::CaptureInclude { .. }) => {
                    let InputRequest::CaptureInclude { request, .. } = &input else {
                        panic!("include request changed");
                    };
                    let capture_authorities = IncludeAuthorities::new(
                        unit,
                        manifest.inputs.fingerprint(),
                        [authorities.root(Digest([177; 32])).unwrap().clone()],
                    )
                    .unwrap();
                    let record = SourceCapture::open(capture_authorities, BTreeMap::new())
                        .unwrap()
                        .capture(request)
                        .unwrap();
                    let token = coordinator.issue(query, phase, input).unwrap();
                    coordinator.start(&context, &token).unwrap();
                    coordinator
                        .accept_include(&context, &token, &record)
                        .unwrap();
                    coordinator
                        .adopt_include(&context, &token, &mut db, record)
                        .unwrap();
                    captured += 1;
                }
                EagerCompileInputPoll::Produced(record) => {
                    let input = InputRequest::Expand(Box::new(ExpansionRequest {
                        key: record.key().clone(),
                    }));
                    let token = coordinator.issue(query, phase, input).unwrap();
                    coordinator.start(&context, &token).unwrap();
                    coordinator
                        .accept(&context, &token, Reply::success(&token, record.output()))
                        .unwrap();
                    coordinator
                        .adopt(&context, &token, &mut db, record)
                        .unwrap();
                }
                EagerCompileInputPoll::Complete(record) => discovery
                    .admit_compiler_expansion(&mut db, &environment, &authorities, Arc::new(record))
                    .unwrap(),
                _ => panic!("unexpected included test demand"),
            }
        }
        assert!(
            completed,
            "include capture did not complete within bounded work"
        );
        assert_eq!(captured, 1);
        assert_eq!(run.unit_invocations(unit), 1);
        assert!(discovery.test_catalog(&mut db).is_err());
    }
    if matches!(variant, GeneratedColdBangShadow | GeneratedColdPlainMarker) {
        assert!(matches!(
            discovery.poll().unwrap(),
            SourceDiscoveryStep::Sealed(_)
        ));
        run.publish_discovery(&mut db, &mut discovery).unwrap();
        assert!(discovery.test_catalog(&mut db).unwrap().cases().is_empty());
        assert_eq!(run.unit_invocations(unit), 1);
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
        return;
    }
    if matches!(
        variant,
        GeneratedColdAttribute | GeneratedColdError | GeneratedColdImpl
    ) {
        let SourceDiscoveryStep::NeedAttributeSelection { selection } = discovery.poll().unwrap()
        else {
            panic!("generated cold attribute kind was not selected");
        };
        let request = run
            .prepare_attribute_selection(&mut db, &selection)
            .unwrap();
        assert_eq!(
            request.request().export,
            if matches!(variant, GeneratedColdError) {
                "panic_attribute"
            } else {
                "attribute_tokens"
            }
        );
        discovery.bind_attribute_request(request.clone()).unwrap();
        let SourceDiscoveryStep::NeedExpansion { request: bound } = discovery.poll().unwrap()
        else {
            panic!("generated cold attribute request absent");
        };
        assert_eq!(bound.key(), request.key());
        let phase = PhaseIdentity {
            frame: Digest([174; 32]),
            body: BodyKey {
                definition: Digest([175; 32]),
                exact_body_bytes: Digest([176; 32]),
            },
            index: 1,
        };
        let result = run.fulfil_discovery(&mut db, &mut discovery, phase, &request, &mut cache);
        if matches!(variant, GeneratedColdError) {
            assert_eq!(
                result
                    .err()
                    .expect("generated cold native error suppressed")
                    .code,
                "LRR-E-MACRO-PANIC"
            );
            assert_eq!(run.unit_invocations(unit), 2);
            assert!(run.publish_discovery(&mut db, &mut discovery).is_err());
            assert!(discovery.test_catalog(&mut db).is_err());
            return;
        }
        result.unwrap();
    } else if generated_test && !bang_shadow && !physical_module {
        let SourceDiscoveryStep::NeedGeneratedExpansion { request } = discovery.poll().unwrap()
        else {
            panic!("generated test attribute was treated as compiler metadata");
        };
        assert_eq!(
            request.procedural_request().unwrap().export,
            "attribute_tokens"
        );
        assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-PERMISSION");
        let next_phase = PhaseIdentity {
            frame: Digest([174; 32]),
            body: BodyKey {
                definition: Digest([175; 32]),
                exact_body_bytes: Digest([176; 32]),
            },
            index: 1,
        };
        run.fulfil_generated_discovery(&mut db, &mut discovery, next_phase, &request, &mut cache)
            .unwrap();
    }
    if let Some(input) = &physical_input {
        assert!(matches!(
            discovery.poll().unwrap(),
            SourceDiscoveryStep::Incomplete { .. }
        ));
        match discovery.complete_module_namespace(&mut db, input) {
            lrr::db::poll::QueryPoll::Ready(Ok(
                lrr::expand::source_discovery::ModuleSourceReadiness::Ready(_),
            )) => {}
            lrr::db::poll::QueryPoll::Ready(Ok(
                lrr::expand::source_discovery::ModuleSourceReadiness::Pending { waiting },
            )) => panic!("physical module remains pending: {waiting:?}"),
            lrr::db::poll::QueryPoll::Ready(Err(error)) => {
                panic!("physical module readiness failed: {error:?}")
            }
            lrr::db::poll::QueryPoll::NeedInput(input) => panic!(
                "physical module readiness requires missing input: {:?}",
                input.fingerprint()
            ),
        }
    }
    match discovery.poll().unwrap() {
        SourceDiscoveryStep::Sealed(_) => {}
        SourceDiscoveryStep::Incomplete {
            diagnostic,
            blocked,
        } => panic!("test catalog unresolved: {} {:?}", diagnostic, blocked),
        _ => panic!("test catalog retained a required producer"),
    }
    assert_eq!(
        run.unit_invocations(unit),
        if (generated_test || physical_module) && !bang_shadow {
            2
        } else {
            1
        }
    );
    run.publish_discovery(&mut db, &mut discovery).unwrap();
    if matches!(
        variant,
        RootLeadingRemoved | RootLeadingRenamed | LeadingMarker | GeneratedLeadingRenamed
    ) {
        let error = match discovery.test_catalog(&mut db) {
            Ok(_) => panic!("removed or renamed compiler Test registration was silently listed"),
            Err(error) => error,
        };
        assert_eq!(error.code, "LRR-E-TEST-DISCOVERY");
        assert!(error.message.contains("removed or renamed"));
        assert_eq!(
            run.unit_invocations(unit),
            if generated_test { 2 } else { 1 }
        );
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
        return;
    }
    let catalog = discovery.test_catalog(&mut db).unwrap();
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
    let before_listing = run.unit_invocations(unit);
    for _ in 0..3 {
        catalog.require_current(&mut db).unwrap();
        assert_eq!(
            discovery.test_catalog(&mut db).unwrap().cases().len(),
            catalog.cases().len()
        );
    }
    assert_eq!(run.unit_invocations(unit), before_listing);
    if bang_shadow {
        assert_eq!(
            catalog
                .cases()
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            if physical_module {
                vec!["child::normal"]
            } else {
                vec!["normal"]
            }
        );
        assert_eq!(run.unit_invocations(unit), 1);
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
        return;
    }
    if matches!(variant, GeneratedColdImpl) {
        assert!(
            discovery
                .impl_header_inventory()
                .unwrap()
                .headers()
                .is_empty(),
            "removed generated cold body retained its impl"
        );
    }
    if shadow_test {
        assert!(
            catalog.cases().is_empty(),
            "procedural test attribute was treated as compiler metadata"
        );
        assert_eq!(
            run.unit_invocations(unit),
            if (generated_test || physical_module) && !bang_shadow {
                2
            } else {
                1
            }
        );
        if matches!(variant, ColdLocalImport) {
            for _ in 0..3 {
                catalog.require_current(&mut db).unwrap();
                assert!(discovery.test_catalog(&mut db).unwrap().cases().is_empty());
                assert_eq!(run.unit_invocations(unit), 1);
            }
            assert!(db.trace.events.iter().all(|event| !matches!(
                event.key,
                QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
            )));
        }
        return;
    }
    assert_eq!(
        catalog
            .cases()
            .iter()
            .map(|case| case.name.as_str())
            .collect::<Vec<_>>(),
        vec!["asynchronous", "expected", "ignored", "normal", "plain"]
    );
    assert!(catalog.select(Some("removed"), true).is_empty());
    assert!(catalog.select(Some("asynchronous"), true)[0].unsupported_signature);
    assert!(catalog.select(Some("ignored"), true)[0].ignored);
    let expected = catalog.select(Some("expected"), true)[0];
    assert!(expected.should_panic);
    assert_eq!(expected.panic_contains.as_deref(), Some("needle"));
    use lrr::engine::test_catalog::IgnorePolicy;
    for (policy, names) in [
        (
            IgnorePolicy::Exclude,
            vec!["asynchronous", "expected", "normal", "plain"],
        ),
        (IgnorePolicy::Only, vec!["ignored"]),
        (
            IgnorePolicy::Include,
            vec!["asynchronous", "expected", "ignored", "normal", "plain"],
        ),
    ] {
        let selected = catalog.select_with_ignore_policy(None, false, policy);
        assert_eq!(
            selected
                .iter()
                .map(|case| case.name.as_str())
                .collect::<Vec<_>>(),
            names
        );
        for case in selected {
            let original = catalog
                .cases()
                .iter()
                .find(|row| row.name == case.name)
                .unwrap();
            assert!(std::ptr::eq(case, original));
        }
        for exact in [false, true] {
            assert!(
                catalog
                    .select_with_ignore_policy(Some("removed"), exact, policy)
                    .is_empty()
            );
            assert!(
                catalog
                    .select_with_ignore_policy(Some("absent"), exact, policy)
                    .is_empty()
            );
            for (name, ignored, panic) in [
                ("ignored", true, false),
                ("expected", false, true),
                ("normal", false, false),
            ] {
                let filter = if exact { name } else { &name[..name.len() - 1] };
                let filtered = catalog.select_with_ignore_policy(Some(filter), exact, policy);
                let included = match policy {
                    IgnorePolicy::Exclude => !ignored,
                    IgnorePolicy::Only => ignored,
                    IgnorePolicy::Include => true,
                };
                assert_eq!(filtered.len(), usize::from(included));
                if included {
                    let original = catalog.select(Some(name), true)[0];
                    assert!(std::ptr::eq(filtered[0], original));
                    assert_eq!(filtered[0].ignored, ignored);
                    assert_eq!(filtered[0].should_panic, panic);
                }
            }
        }
    }
    assert!(catalog.cases().iter().all(|case| {
        case.body
            .as_ref()
            .is_some_and(|body| body.definition == case.definition)
    }));
    for _ in 0..3 {
        catalog.require_current(&mut db).unwrap();
        assert_eq!(discovery.test_catalog(&mut db).unwrap().cases().len(), 5);
    }
    if selected_async {
        // Exercise the common unsupported-selection guard on an actual generated
        // case after its catalog certificate is checked. Guest-body execution
        // and the public CLI/procedural broker integration remain separate gates.
        catalog.require_current(&mut db).unwrap();
        let selected = catalog.select(Some("asynchronous"), true);
        assert_eq!(selected.len(), 1);
        let captured_sources = [view.share()];
        let result = lrr::engine::test_harness::run_original(
            &mut db,
            &captured_sources,
            selected[0],
            true,
            1000,
        )
        .unwrap();
        assert!(matches!(
            result.status,
            lrr::engine::test_harness::TestStatus::Failed
        ));
        assert_eq!(
            result.diagnostic.as_ref().unwrap().code,
            "LRR-E-UNSUPPORTED"
        );
        assert_eq!(run.unit_invocations(unit), 1);
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }
    assert_eq!(run.unit_invocations(unit), 1);
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
    fs::write(evidence.join("m015-test-catalog.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M015","names":catalog.cases().iter().map(|case|&case.name).collect::<Vec<_>>(),"native_invocations":1,"removed_original":true,"final_attributes":true,"repeated_listing_reexecutes":false,"signature_or_body_queries":0,"full_fixture_acceptance":false})).unwrap()).unwrap();
    db.begin_expansion_epoch(2).unwrap();
    assert!(catalog.require_current(&mut db).is_err());
}

include!("support/constant_key.rs");
