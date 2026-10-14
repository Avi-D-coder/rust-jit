//! Actual pinned planning/preparation of a macro-only dependency and invalid live consumer.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::GUEST_TARGET,
    },
    project::{
        cargo_units::{ArtifactProfiles, Selection, UnitContext, UnitGraph, UnitRole},
        tool_manifest::{
            CompileEnv, CompileEnvInputs, CompilerArtifactMessage, InputKind, InputPlan, InputRoot,
            PackageEnvironments, PreparedFiles, ToolBinary, ToolManifest, ToolStore,
            ToolchainRecord, capture_compiler_artifacts, verify_adoption,
        },
        tool_transaction::ToolTransaction,
    },
    source::snapshot::CfgSet,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn installed(selection: &str, program: &str) -> PathBuf {
    let output = Command::new("rustup")
        .args(["which", "--toolchain", selection, program])
        .output()
        .unwrap();
    assert!(output.status.success());
    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
}
fn run(mut command: Command, evidence: &Path, label: &str) -> Vec<u8> {
    fs::write(
        evidence.join(format!("{label}.argv")),
        format!("{command:?}\n"),
    )
    .unwrap();
    let output = command.output().unwrap();
    fs::write(evidence.join(format!("{label}.stdout")), &output.stdout).unwrap();
    fs::write(evidence.join(format!("{label}.stderr")), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
pub fn prepare(evidence: &Path, macro_source: &Path) -> ToolManifest {
    prepare_named(evidence, macro_source, "lrr_macro_service_consumer")
}
pub fn prepare_named(evidence: &Path, macro_source: &Path, package: &str) -> ToolManifest {
    prepare_case(
        evidence,
        macro_source,
        package,
        false,
        &[],
        "lrr_fixture_macros",
    )
}
pub fn prepare_with_live_dependency(evidence: &Path, macro_source: &Path) -> ToolManifest {
    prepare_case(
        evidence,
        macro_source,
        "lrr_macro_service_consumer",
        true,
        &[],
        "lrr_fixture_macros",
    )
}
pub fn prepare_sources(
    evidence: &Path,
    macro_source: &Path,
    sources: &[(&str, &str)],
) -> ToolManifest {
    prepare_case(
        evidence,
        macro_source,
        "lrr_macro_service_consumer",
        false,
        sources,
        "lrr_fixture_macros",
    )
}
pub fn prepare_original_derives(
    evidence: &Path,
    macro_source: &Path,
    source: &str,
) -> ToolManifest {
    prepare_case(
        evidence,
        macro_source,
        "rustc_original_derive_consumer",
        false,
        &[("lib.rs", source)],
        "multiple_derives",
    )
}
#[allow(dead_code)]
pub fn prepare_test_count_attributes(
    evidence: &Path,
    macro_source: &Path,
    source: &str,
) -> ToolManifest {
    prepare_case(
        evidence,
        macro_source,
        "rustc_test_count_consumer",
        false,
        &[("lib.rs", source)],
        "test_count_attributes",
    )
}
#[allow(dead_code)]
pub fn prepare_rustc_macro_sources(
    evidence: &Path,
    macro_source: &Path,
    source: &str,
    alias: &str,
) -> ToolManifest {
    prepare_case(
        evidence,
        macro_source,
        "rustc_original_macro_consumer",
        false,
        &[("lib.rs", source)],
        alias,
    )
}
fn prepare_case(
    evidence: &Path,
    macro_source: &Path,
    package: &str,
    extra: bool,
    sources: &[(&str, &str)],
    macro_alias: &str,
) -> ToolManifest {
    fs::create_dir(evidence).unwrap();
    let app = evidence.join("app");
    fs::create_dir(&app).unwrap();
    fs::create_dir(app.join("src")).unwrap();
    fs::write(app.join("Cargo.toml"),format!("[package]\nname=\"{package}\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[dependencies]\n{macro_alias}={{path={:?}}}\n[workspace]\n",macro_source.to_str().unwrap())).unwrap();
    let secondary = evidence.join("secondary");
    if extra {
        fs::create_dir(&secondary).unwrap();
        fs::create_dir(secondary.join("src")).unwrap();
        fs::write(secondary.join("Cargo.toml"),format!("[package]\nname=\"lrr_live_second\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[dependencies]\nlrr_fixture_macros={{path={:?}}}\n[workspace]\n",macro_source.to_str().unwrap())).unwrap();
        fs::write(
            secondary.join("src/lib.rs"),
            "fn cold(){ deliberately_undefined(); }\n",
        )
        .unwrap();
        let cargo = fs::read_to_string(app.join("Cargo.toml")).unwrap().replace(
            "[workspace]",
            &format!(
                "lrr_live_second={{path={:?}}}\n[workspace]",
                secondary.to_str().unwrap()
            ),
        );
        fs::write(app.join("Cargo.toml"), cargo).unwrap();
    }
    let live_source = app.join("src/lib.rs");
    fs::write(&live_source, "fn cold() { deliberately_undefined(); }\n").unwrap();
    for (relative, contents) in sources {
        let path = app.join("src").join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    let tools = lrr::project::toolchain::capture("1.99.0").unwrap();
    assert_eq!(tools.host, GUEST_TARGET);
    let planner = installed("nightly-2026-10-01", "cargo");
    let mut lock = Command::new(&planner);
    lock.current_dir(&app)
        .args(["generate-lockfile", "--offline"])
        .env("RUSTC", &tools.compiler);
    run(lock, evidence, "lock");
    let mut roots = vec![
        InputRoot::open(
            InputKind::Source,
            &app,
            BTreeSet::new(),
            BTreeSet::new(),
            true,
            BTreeSet::new(),
        )
        .unwrap(),
        InputRoot::open(
            InputKind::Source,
            macro_source,
            BTreeSet::new(),
            BTreeSet::new(),
            true,
            BTreeSet::new(),
        )
        .unwrap(),
    ];
    if extra {
        roots.push(
            InputRoot::open(
                InputKind::Source,
                &secondary,
                BTreeSet::new(),
                BTreeSet::new(),
                true,
                BTreeSet::new(),
            )
            .unwrap(),
        );
    }
    let input_plan = InputPlan::new(roots, BTreeSet::new()).unwrap();
    let transaction = ToolTransaction::begin(&input_plan).unwrap();
    let mut python = Command::new("python3");
    python.args(["-c", "import sys;print(sys.executable)"]);
    let python = String::from_utf8(run(python, evidence, "python")).unwrap();
    let wrapper = evidence.join("compiler.py");
    fs::write(&wrapper,format!("#!{}\nimport fcntl,json,os,subprocess,sys\ncompiler,*args=sys.argv[1:]\nresult=subprocess.run([compiler,*args],check=False,close_fds=False)\nwith open(os.environ['LRR_MACRO_SERVICE_COMPILER_LOG'],'a') as log:\n fcntl.flock(log,fcntl.LOCK_EX)\n log.write(json.dumps({{'compiler':compiler,'argv':args,'cwd':os.getcwd(),'exit_code':result.returncode}})+'\\n')\n log.flush()\nsys.exit(result.returncode)\n",python.trim())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let command = || {
        let mut command = Command::new(&planner);
        command
            .current_dir(&app)
            .env("RUSTC", &tools.compiler)
            .env("RUSTC_WRAPPER", &wrapper)
            .env(
                "LRR_MACRO_SERVICE_COMPILER_LOG",
                evidence.join("compiler.jsonl"),
            )
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_PROFILE_TEST_DEBUG", "0")
            .env("CARGO_INCREMENTAL", "0")
            .args([
                "check",
                "-Z",
                "unstable-options",
                "--lib",
                "--profile",
                "test",
                "--target",
                GUEST_TARGET,
                "--locked",
                "--offline",
                "--target-dir",
            ])
            .arg(evidence.join("target"));
        command
    };
    let mut metadata = Command::new(&planner);
    metadata
        .current_dir(&app)
        .args(["metadata", "--format-version", "1", "--locked", "--offline"]);
    let packages = PackageEnvironments::decode(&run(metadata, evidence, "metadata")).unwrap();
    let mut plan = command();
    plan.arg("--unit-graph");
    let raw = run(plan, evidence, "unit-graph");
    let value: Value = serde_json::from_slice(&raw).unwrap();
    let index = value["roots"][0].as_u64().unwrap() as usize;
    let mut cfg_command = Command::new(&tools.compiler);
    cfg_command.args(["--print", "cfg", "--target", GUEST_TARGET]);
    let cfg = run(cfg_command, evidence, "compiler-cfg");
    let context = UnitContext {
        package: value["units"][index]["pkg_id"].as_str().unwrap().into(),
        selection: Selection::Library {
            test: true,
            name: package.into(),
        },
        ordinary_role: UnitRole::Live,
        host: tools.host.clone(),
        target: GUEST_TARGET.into(),
        cfg_by_target: BTreeMap::from([(GUEST_TARGET.into(), Digest::of_bytes(&cfg))]),
        rustc_fingerprint: tools.compiler_content,
    };
    let graph = UnitGraph::parse(&raw, &context).unwrap();
    let profiles = ArtifactProfiles::parse(&raw, &context).unwrap();
    assert_eq!(
        graph
            .units
            .values()
            .filter(|u| u.key.role == UnitRole::Live)
            .count(),
        if extra { 2 } else { 1 }
    );
    assert_eq!(
        graph
            .units
            .values()
            .filter(|u| u.key.role == UnitRole::ProcMacroHost)
            .count(),
        1
    );
    let mut build = command();
    build.args(["--compile-time-deps", "--message-format=json"]);
    let messages = run(build, evidence, "prepare");
    let records = std::str::from_utf8(&messages)
        .unwrap()
        .lines()
        .filter_map(|line| {
            let message: Value = serde_json::from_str(line).unwrap();
            (message["reason"] == "compiler-artifact")
                .then(|| CompilerArtifactMessage::decode(line.as_bytes()).unwrap())
        })
        .collect();
    let artifacts = capture_compiler_artifacts(&graph, &profiles, records).unwrap();
    assert_eq!(artifacts.len(), 1);
    let calls: Vec<Value> = fs::read_to_string(evidence.join("compiler.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for call in &calls {
        for arg in call["argv"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .filter(|s| s.ends_with(".rs"))
        {
            let path = Path::new(call["cwd"].as_str().unwrap())
                .join(arg)
                .canonicalize()
                .unwrap();
            assert_ne!(
                path, live_source,
                "selected live runtime source was compiled/check-analyzed"
            );
            for unit in graph
                .units
                .values()
                .filter(|u| u.key.role == UnitRole::Live)
            {
                assert_ne!(
                    path,
                    PathBuf::from(&unit.source_path),
                    "a live dependency source was compiled"
                );
            }
            assert_eq!(call["exit_code"], 0);
        }
    }
    let mut base_cfg = CfgSet {
        atoms: BTreeSet::new(),
        values: BTreeMap::new(),
    };
    for line in std::str::from_utf8(&cfg).unwrap().lines() {
        lrr::syntax::cfg_input::add(&mut base_cfg, line).unwrap();
    }
    let primary = graph
        .roots
        .iter()
        .map(|id| graph.units[id].key.package.clone())
        .collect();
    let mut compile_env = BTreeMap::new();
    for (id, unit) in &graph.units {
        compile_env.insert(
            *id,
            CompileEnv::assemble(
                unit,
                CompileEnvInputs {
                    host: &tools.host,
                    base_cfg: &base_cfg,
                    profiles: &profiles,
                    cargo: packages
                        .unit_variables(unit, planner.to_str().unwrap(), &primary)
                        .unwrap(),
                    declared: BTreeMap::new(),
                    output: None,
                },
            )
            .unwrap(),
        );
    }
    let mut planner_version = Command::new(&planner);
    planner_version.arg("-Vv");
    let planner_version =
        String::from_utf8(run(planner_version, evidence, "planner-version")).unwrap();
    let mut server_version = Command::new(&tools.server);
    server_version
        .env("RUST_ANALYZER_INTERNALS_DO_NOT_USE", "this is unstable")
        .arg("--version");
    let server_version =
        String::from_utf8(run(server_version, evidence, "server-version")).unwrap();
    let toolchain = ToolchainRecord {
        planner: ToolBinary {
            path: planner.to_str().unwrap().into(),
            content: Digest::of_bytes(&fs::read(&planner).unwrap()),
            selection: "nightly-2026-10-01".into(),
            version_verbose: planner_version,
        },
        compiler: ToolBinary {
            path: tools.compiler.to_str().unwrap().into(),
            content: tools.compiler_content,
            selection: "1.99.0".into(),
            version_verbose: tools.compiler_version,
        },
        compiler_commit: tools.compiler_commit,
        host: tools.host,
        sysroot: tools.sysroot.to_str().unwrap().into(),
        macro_server: ToolBinary {
            path: tools.server.to_str().unwrap().into(),
            content: Digest::of_bytes(&fs::read(&tools.server).unwrap()),
            selection: "1.99.0".into(),
            version_verbose: server_version,
        },
        macro_protocol: 6,
        component_manifest: tools.server_component_manifest.to_str().unwrap().into(),
        component_manifest_content: tools.component_manifest_content,
        prepared_macro_server: None,
    };
    let manifest = ToolManifest {
        schema_version: 4,
        semantics_profile: "lrr-2024-v2".into(),
        toolchain,
        graph,
        inputs: transaction.inputs().clone(),
        artifacts,
        compile_env,
        build_outputs: BTreeMap::new(),
    };
    manifest.validate().unwrap();
    let after = input_plan.capture().unwrap();
    let files = PreparedFiles::capture(&manifest).unwrap();
    verify_adoption(&manifest, &manifest.toolchain, &after, &files).unwrap();
    fs::create_dir(evidence.join("store")).unwrap();
    let store = ToolStore::open(&evidence.join("store")).unwrap();
    let publication = transaction.commit(&store, &manifest).unwrap();
    assert_eq!(
        store.load(publication).unwrap().fingerprint(),
        manifest.fingerprint()
    );
    fs::write(
        evidence.join("schema4-manifest.json"),
        manifest.encode().unwrap(),
    )
    .unwrap();
    fs::write(evidence.join("preparation-report.json"),serde_json::to_vec_pretty(&serde_json::json!({"selected_live_source":live_source,"live_compiler_invocations":0,"artifact_units":manifest.artifacts.keys().collect::<Vec<_>>(),"publication":publication,"inputs_unchanged":true})).unwrap()).unwrap();
    manifest
}

/// Private candidate validation must execute a binary compiled for that exact
/// checkout. Ordinary milestone runs use the main target directory instead.
#[allow(dead_code)]
pub fn require_current_validation_source() {
    let Some(expected) = std::env::var_os("LRR_VALIDATION_SOURCE_ROOT") else {
        return;
    };
    let expected = fs::canonicalize(Path::new(&expected)).unwrap();
    let compiled = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    assert_eq!(
        fs::canonicalize(compiled).unwrap(),
        expected,
        "validation binary belongs to another checkout"
    );
    // Embedded source facts independently detect a stale copied-source binary.
    // This is harness-only I/O before preparation or guest/macro execution.
    let embedded: &[(&str, &[u8])] = &[
        (
            "crates/lrr/src/service/source_attribute_requests.rs",
            include_bytes!("../../../crates/lrr/src/service/source_attribute_requests.rs"),
        ),
        (
            "crates/lrr/src/frontend/generated_macros.rs",
            include_bytes!("../../../crates/lrr/src/frontend/generated_macros.rs"),
        ),
        (
            "crates/lrr/src/expand/generated_call_request.rs",
            include_bytes!("../../../crates/lrr/src/expand/generated_call_request.rs"),
        ),
        (
            "crates/lrr/src/expand/source_discovery.rs",
            include_bytes!("../../../crates/lrr/src/expand/source_discovery.rs"),
        ),
        (
            "crates/lrr/src/service/run.rs",
            include_bytes!("../../../crates/lrr/src/service/run.rs"),
        ),
        (
            "crates/lrr/src/engine/run.rs",
            include_bytes!("../../../crates/lrr/src/engine/run.rs"),
        ),
        (
            "crates/lrr/src/engine/failure.rs",
            include_bytes!("../../../crates/lrr/src/engine/failure.rs"),
        ),
        (
            "crates/lrr/src/syntax/origin.rs",
            include_bytes!("../../../crates/lrr/src/syntax/origin.rs"),
        ),
    ];
    for (relative, bytes) in embedded {
        assert_eq!(
            *bytes,
            fs::read(expected.join(relative)).unwrap(),
            "validation embedded source differs: {relative}"
        );
    }
}
