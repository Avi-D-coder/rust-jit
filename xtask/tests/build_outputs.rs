use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::GUEST_TARGET,
    },
    project::{
        cargo_units::{ArtifactProfiles, Selection, UnitContext, UnitGraph, UnitRole},
        tool_manifest::{
            BuildScriptMessage, CompileEnv, CompileEnvInputs, CompilerArtifactMessage, InputKind,
            InputPlan, InputRoot, PackageEnvironments, PreparedFiles, ToolBinary, ToolManifest,
            ToolStore, ToolchainRecord, associate_build_outputs, capture_compiler_artifacts,
            verify_adoption,
        },
        tool_transaction::ToolTransaction,
    },
    source::snapshot::{CfgSet, SourcePath},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
fn installed(channel: &str, program: &str) -> PathBuf {
    let out = Command::new("rustup")
        .args(["which", "--toolchain", channel, program])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    PathBuf::from(String::from_utf8(out.stdout).unwrap().trim())
}
fn text_output(program: &Path, args: &[&str]) -> String {
    let out = Command::new(program).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}
#[test]
fn real_cargo_build_outputs_remain_attached_to_exact_graph_units() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let fixture = root.join("fixtures/macros/two-build-units");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let evidence = root.join(format!(
        "artifacts/compile-time-probe/two-output-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&evidence).unwrap();
    let target = evidence.join("target");
    let compiler = installed("1.99.0", "rustc");
    let planner = installed("nightly-2026-10-01", "cargo");
    let version = text_output(&compiler, &["-vV"]);
    assert!(version.lines().any(|l| l == "release: 1.99.0"));
    let planner_version = text_output(&planner, &["-Vv"]);
    assert!(planner_version.contains("commit-date: 2026-09-29"));
    fs::write(evidence.join("compiler-version.txt"), &version).unwrap();
    fs::write(evidence.join("planner-version.txt"), &planner_version).unwrap();
    let host = version
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .unwrap()
        .to_owned();
    let cfg = text_output(&compiler, &["--print", "cfg", "--target", GUEST_TARGET]);
    fs::write(evidence.join("compiler-cfg.txt"), &cfg).unwrap();
    let python = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let wrapper = evidence.join("forward-compiler.py");
    fs::write(&wrapper, format!("#!{}\nimport fcntl,json,os,subprocess,sys\ncompiler,*args=sys.argv[1:]\nresult=subprocess.run([compiler,*args],check=False,close_fds=False)\nwith open(os.environ['LRR_TWO_OUTPUT_COMPILER_LOG'],'a') as log:\n fcntl.flock(log,fcntl.LOCK_EX)\n log.write(json.dumps({{'compiler':compiler,'argv':args,'exit_code':result.returncode,'cargo_environment':{{k:v for k,v in os.environ.items() if k.startswith('CARGO_PKG_') or k in ('CARGO','CARGO_MANIFEST_DIR','CARGO_MANIFEST_PATH','CARGO_CRATE_NAME','CARGO_BIN_NAME','CARGO_PRIMARY_PACKAGE')}}}})+'\\n')\n log.flush()\nsys.exit(result.returncode)\n", String::from_utf8(python.stdout).unwrap().trim())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let input_plan = InputPlan::new(
        vec![
            InputRoot::open(
                InputKind::Source,
                &fixture,
                BTreeSet::new(),
                BTreeSet::new(),
                true,
                BTreeSet::new(),
            )
            .unwrap(),
        ],
        BTreeSet::new(),
    )
    .unwrap();
    let transaction = ToolTransaction::begin(&input_plan).unwrap();
    let before = input_plan.capture().unwrap();
    assert_eq!(transaction.inputs(), before.inventory());
    let invoke = |label: &str, extra: &[&str]| {
        let mut command = Command::new(&planner);
        command.current_dir(&fixture).env_clear();
        for name in [
            "PATH",
            "HOME",
            "TMPDIR",
            "SDKROOT",
            "DEVELOPER_DIR",
            "RUSTUP_HOME",
            "CARGO_HOME",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .env("RUSTC", &compiler)
            .env("RUSTC_WRAPPER", &wrapper)
            .env(
                "LRR_TWO_OUTPUT_COMPILER_LOG",
                evidence.join(format!("{label}-compiler.jsonl")),
            );
        command
            .args(["check", "-Z", "unstable-options", "--manifest-path"])
            .arg(fixture.join("Cargo.toml"));
        command
            .args([
                "--package",
                "lrr_two_output_app",
                "--lib",
                "--profile",
                "test",
                "--target",
                GUEST_TARGET,
                "--target-dir",
            ])
            .arg(&target)
            .args(["--locked", "--offline"])
            .args(extra);
        fs::write(
            evidence.join(format!("{label}-argv.txt")),
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
    };
    let metadata_output = Command::new(&planner)
        .current_dir(&fixture)
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .output()
        .unwrap();
    fs::write(evidence.join("metadata.stdout"), &metadata_output.stdout).unwrap();
    fs::write(evidence.join("metadata.stderr"), &metadata_output.stderr).unwrap();
    assert!(metadata_output.status.success());
    let package_environments = PackageEnvironments::decode(&metadata_output.stdout).unwrap();
    let raw = invoke("graph", &["--unit-graph"]);
    let value: Value = serde_json::from_slice(&raw).unwrap();
    let index = usize::try_from(value["roots"][0].as_u64().unwrap()).unwrap();
    let context = UnitContext {
        package: value["units"][index]["pkg_id"].as_str().unwrap().into(),
        selection: Selection::Library {
            test: true,
            name: "lrr_two_output_app".into(),
        },
        ordinary_role: UnitRole::Live,
        host: host.clone(),
        target: GUEST_TARGET.into(),
        cfg_by_target: BTreeMap::from([
            (host, Digest::of_bytes(cfg.as_bytes())),
            (GUEST_TARGET.into(), Digest::of_bytes(cfg.as_bytes())),
        ]),
        rustc_fingerprint: Digest::of_bytes(&fs::read(&compiler).unwrap()),
    };
    let graph = UnitGraph::parse(&raw, &context).unwrap();
    fs::write(
        evidence.join("normalized-graph.json"),
        serde_json::to_vec_pretty(&graph).unwrap(),
    )
    .unwrap();
    let messages = invoke(
        "prepare",
        &[
            "--compile-time-deps",
            "--message-format=json-render-diagnostics",
        ],
    );
    before
        .verify_unchanged(&input_plan.capture().unwrap())
        .unwrap();
    let mut build = vec![];
    let mut raw_build = vec![];
    let mut finished = false;
    for line in std::str::from_utf8(&messages).unwrap().lines() {
        let message: Value = serde_json::from_str(line).unwrap();
        if message["reason"] == "build-script-executed" {
            build.push(BuildScriptMessage::decode(line.as_bytes()).unwrap());
            raw_build.push(message.clone());
        }
        if message["reason"] == "build-finished" {
            assert_eq!(message["success"], true);
            finished = true;
        }
    }
    assert!(finished);
    assert_eq!(build.len(), 2);
    let mut reversed = build.clone();
    reversed.reverse();
    let reversed_associations = associate_build_outputs(&graph, reversed).unwrap();
    let associations = associate_build_outputs(&graph, build).unwrap();
    for (id, association) in &associations {
        assert_eq!(association.out_dir(), reversed_associations[id].out_dir());
        assert_eq!(
            association.consumers(),
            reversed_associations[id].consumers()
        );
    }
    raw_build[1]["out_dir"] = raw_build[0]["out_dir"].clone();
    let shared_directory = raw_build
        .iter()
        .map(|value| BuildScriptMessage::decode(&serde_json::to_vec(value).unwrap()).unwrap())
        .collect();
    assert!(
        associate_build_outputs(&graph, shared_directory)
            .unwrap_err()
            .message
            .contains("share OUT_DIR")
    );
    assert_eq!(associations.len(), 2);
    let generated_path = SourcePath::new(Path::new("generated.rs")).unwrap();
    let mut outputs = BTreeMap::new();
    for (id, association) in &associations {
        assert_eq!(*id, association.producer());
        let captured = association.capture(&graph).unwrap();
        for consumer in association.consumers() {
            assert!(
                graph.units[consumer]
                    .dependencies
                    .iter()
                    .any(|d| d.target == *id)
            );
            assert_eq!(
                graph.units[consumer].key.package,
                graph.units[id].key.package
            );
        }
        let owner = &captured.rustc_env["OUTPUT_OWNER"];
        let expected = match owner.as_str() {
            "left" => b"31_u32\n".as_slice(),
            "right" => b"47_u32\n".as_slice(),
            _ => panic!("unexpected owner"),
        };
        assert_eq!(captured.tree.files()[&generated_path].bytes(), expected);
        assert_eq!(captured.tree.files()[&generated_path].producer(), *id);
        assert!(captured.cfgs.contains(&format!("lrr_generated_{owner}")));
        outputs.insert(*id, captured);
    }
    let calls: Vec<Value> = fs::read_to_string(evidence.join("prepare-compiler.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let mut compiled_scripts = BTreeSet::new();
    for call in &calls {
        let argv = call["argv"].as_array().unwrap();
        let information = argv
            .iter()
            .filter_map(Value::as_str)
            .any(|a| a.starts_with("--print") || a == "-vV" || a == "--version");
        if information {
            continue;
        }
        assert_eq!(call["exit_code"], 0);
        for arg in argv
            .iter()
            .filter_map(Value::as_str)
            .filter(|a| a.ends_with(".rs"))
        {
            let source = fixture.join(arg).canonicalize().unwrap();
            assert!(
                source == fixture.join("left/build.rs") || source == fixture.join("right/build.rs"),
                "live compilation: {call}"
            );
            compiled_scripts.insert(source);
        }
    }
    assert_eq!(compiled_scripts.len(), 2);
    fs::write(
        evidence.join("captured-build-outputs.json"),
        serde_json::to_vec_pretty(&outputs).unwrap(),
    )
    .unwrap();
    // This fixture assembles schema4 records from actual compiler/artifact evidence.
    // General preparation orchestration remains separate work.
    let selected = lrr::project::toolchain::capture("1.99.0").unwrap();
    assert_eq!(selected.compiler, compiler);
    assert_eq!(selected.compiler_content, context.rustc_fingerprint);
    let path_string = |p: &Path| p.to_str().unwrap().to_owned();
    let toolchain = ToolchainRecord {
        planner: ToolBinary {
            path: path_string(&planner),
            content: Digest::of_bytes(&fs::read(&planner).unwrap()),
            selection: "nightly-2026-10-01".into(),
            version_verbose: planner_version.clone(),
        },
        compiler: ToolBinary {
            path: path_string(&selected.compiler),
            content: selected.compiler_content,
            selection: selected.selection.clone(),
            version_verbose: selected.compiler_version.clone(),
        },
        compiler_commit: selected.compiler_commit.clone(),
        host: selected.host.clone(),
        sysroot: path_string(&selected.sysroot),
        macro_server: ToolBinary {
            path: path_string(&selected.server),
            content: Digest::of_bytes(&fs::read(&selected.server).unwrap()),
            selection: selected.selection.clone(),
            version_verbose: format!(
                "matched sysroot server: compiler {}",
                selected.compiler_commit
            ),
        },
        macro_protocol: 6,
        component_manifest: path_string(&selected.server_component_manifest),
        component_manifest_content: selected.component_manifest_content,
        prepared_macro_server: None,
    };
    let profiles = ArtifactProfiles::parse(&raw, &context).unwrap();
    let records = std::str::from_utf8(&messages)
        .unwrap()
        .lines()
        .filter_map(|line| {
            let record: Value = serde_json::from_str(line).unwrap();
            (record["reason"] == "compiler-artifact")
                .then(|| CompilerArtifactMessage::decode(line.as_bytes()).unwrap())
        })
        .collect();
    let cargo_records: Vec<Value> = std::str::from_utf8(&messages)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|record| record["reason"] == "compiler-artifact")
        .collect();
    let mut mismatched = cargo_records[0].clone();
    mismatched["profile"]["opt_level"] = json!("3");
    assert!(
        capture_compiler_artifacts(
            &graph,
            &profiles,
            vec![
                CompilerArtifactMessage::decode(&serde_json::to_vec(&mismatched).unwrap()).unwrap()
            ]
        )
        .is_err()
    );
    let mut duplicate = cargo_records[0].clone();
    duplicate["filenames"] = json!(["/missing/tool", "/missing/tool"]);
    assert!(
        CompilerArtifactMessage::decode(&serde_json::to_vec(&duplicate).unwrap())
            .unwrap_err()
            .message
            .contains("duplicate")
    );
    let mut unknown = cargo_records[0].clone();
    unknown["profile"]["new_field"] = json!(true);
    assert!(CompilerArtifactMessage::decode(&serde_json::to_vec(&unknown).unwrap()).is_err());
    let mut ordinary = cargo_records[0].clone();
    let selected_raw = &value["units"][index];
    ordinary["manifest_path"] = json!(fixture.join("app/Cargo.toml"));
    ordinary["package_id"] = selected_raw["pkg_id"].clone();
    ordinary["target"] = selected_raw["target"].clone();
    ordinary["features"] = selected_raw["features"].clone();
    ordinary["profile"]["test"] = json!(true);
    ordinary["filenames"] = json!(["/missing/live.rlib"]);
    assert!(
        capture_compiler_artifacts(
            &graph,
            &profiles,
            vec![CompilerArtifactMessage::decode(&serde_json::to_vec(&ordinary).unwrap()).unwrap()]
        )
        .unwrap_err()
        .message
        .contains("live/native runtime")
    );
    let mut changed_target = cargo_records[0].clone();
    changed_target["target"]["edition"] = json!("2021");
    assert!(
        capture_compiler_artifacts(
            &graph,
            &profiles,
            vec![
                CompilerArtifactMessage::decode(&serde_json::to_vec(&changed_target).unwrap())
                    .unwrap()
            ]
        )
        .is_err()
    );
    let repeated = cargo_records
        .iter()
        .take(1)
        .chain(cargo_records.iter().take(1))
        .map(|r| CompilerArtifactMessage::decode(&serde_json::to_vec(r).unwrap()).unwrap())
        .collect();
    assert!(capture_compiler_artifacts(&graph, &profiles, repeated).is_err());
    let artifacts = capture_compiler_artifacts(&graph, &profiles, records).unwrap();
    assert_eq!(artifacts.len(), 2);
    let mut base_cfg = CfgSet {
        atoms: BTreeSet::new(),
        values: BTreeMap::new(),
    };
    for line in cfg.lines() {
        lrr::syntax::cfg_input::add(&mut base_cfg, line).unwrap();
    }
    // The base snapshot must not leak another unit's test/features into this one.
    base_cfg.atoms.insert("test".into());
    base_cfg.values.insert(
        "feature".into(),
        BTreeSet::from(["unrelated-base-feature".into()]),
    );
    let primary_packages = graph
        .roots
        .iter()
        .map(|id| graph.units[id].key.package.clone())
        .collect::<BTreeSet<_>>();
    let mut compile_env = BTreeMap::new();
    for (id, unit) in &graph.units {
        let cargo = package_environments
            .unit_variables(unit, planner.to_str().unwrap(), &primary_packages)
            .unwrap();
        let output = outputs.values().find(|out| out.consumers.contains(id));
        let env = CompileEnv::assemble(
            unit,
            CompileEnvInputs {
                host: &selected.host,
                base_cfg: &base_cfg,
                profiles: &profiles,
                cargo,
                declared: BTreeMap::new(),
                output,
            },
        )
        .unwrap();
        assert_eq!(env.cfg.atoms.contains("test"), unit.key.test_intent);
        assert_eq!(
            env.cfg.values.get("feature").cloned().unwrap_or_default(),
            unit.key.features
        );
        if let Some(output) = output {
            assert_eq!(
                env.lookup("OUTPUT_OWNER"),
                output.rustc_env.get("OUTPUT_OWNER").map(String::as_str)
            );
            let wrong = outputs
                .values()
                .find(|candidate| candidate.producer != output.producer)
                .unwrap();
            assert!(
                CompileEnv::assemble(
                    unit,
                    CompileEnvInputs {
                        host: &selected.host,
                        base_cfg: &base_cfg,
                        profiles: &profiles,
                        cargo: package_environments
                            .unit_variables(unit, planner.to_str().unwrap(), &primary_packages)
                            .unwrap(),
                        declared: BTreeMap::new(),
                        output: Some(wrong)
                    }
                )
                .is_err()
            );
        }
        compile_env.insert(*id, env);
    }
    for call in &calls {
        let Some(source) = call["argv"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .find(|arg| arg.ends_with(".rs"))
        else {
            continue;
        };
        let source = fixture.join(source).canonicalize().unwrap();
        let candidates: Vec<_> = graph
            .units
            .iter()
            .filter(|(_, unit)| {
                unit.mode == lrr::project::cargo_units::UnitMode::Build
                    && Path::new(&unit.source_path) == source
            })
            .collect();
        assert_eq!(candidates.len(), 1);
        let env = &compile_env[candidates[0].0];
        let actual: BTreeMap<String, String> =
            serde_json::from_value(call["cargo_environment"].clone()).unwrap();
        for (key, value) in &actual {
            assert_eq!(env.lookup(key), Some(value.as_str()), "{key}: {call}");
        }
        for (key, value) in &env.cargo {
            if key == "OUT_DIR" {
                continue;
            }
            assert_eq!(actual.get(key), Some(value), "{key}: {call}");
        }
    }
    let manifest = ToolManifest {
        schema_version: 4,
        semantics_profile: "lrr-2024-v2".into(),
        toolchain,
        graph: graph.clone(),
        inputs: before.inventory().clone(),
        artifacts,
        compile_env,
        build_outputs: outputs.clone(),
    };
    manifest.validate().unwrap();
    let generated = lrr::project::generated_sources::GeneratedSources::new(std::sync::Arc::new(
        manifest.clone(),
    ))
    .unwrap();
    let mut includes = BTreeMap::new();
    for output in manifest.build_outputs.values() {
        for consumer in &output.consumers {
            let env = &manifest.compile_env[consumer];
            let path = Path::new(env.lookup("OUT_DIR").unwrap()).join("generated.rs");
            let lookup = generated.lookup_original(*consumer, &path).unwrap();
            assert_eq!(lookup.producer(), output.producer);
            assert_eq!(
                lookup.source().unwrap().unwrap().bytes(),
                output.tree.files()[&generated_path].bytes()
            );
            let other = manifest
                .build_outputs
                .values()
                .find(|other| other.producer != output.producer)
                .unwrap();
            assert!(
                generated
                    .lookup_original(
                        *consumer,
                        &Path::new(other.tree.out_dir()).join("generated.rs")
                    )
                    .is_err()
            );
            includes.insert(*consumer,json!({"producer":lookup.producer(),"origin":lookup.original_path(),"source":lookup.source().unwrap().unwrap().text().unwrap(),"lookup":lookup.fingerprint(),"membership":lookup.membership()}));
        }
    }
    fs::write(
        evidence.join("generated-includes.json"),
        serde_json::to_vec_pretty(&includes).unwrap(),
    )
    .unwrap();
    let prepared_files = PreparedFiles::capture(&manifest).unwrap();
    let after = input_plan.capture().unwrap();
    verify_adoption(&manifest, &manifest.toolchain, &after, &prepared_files).unwrap();
    fs::create_dir(evidence.join("store")).unwrap();
    let store = ToolStore::open(&evidence.join("store")).unwrap();
    let published = transaction.commit(&store, &manifest).unwrap();
    assert_eq!(
        store.load(published).unwrap().fingerprint(),
        manifest.fingerprint()
    );
    fs::write(
        evidence.join("schema4-manifest.json"),
        manifest.encode().unwrap(),
    )
    .unwrap();
    fs::write(evidence.join("schema4-publication.json"), serde_json::to_vec_pretty(&json!({"manifest": published,"prepared_files_verified":true,"transaction_finalization":true,"artifact_units":manifest.artifacts.keys().collect::<Vec<_>>(),"package_environment_matched_compiler_records":true,"macro_protocol_verified_in_m003_gate":true})).unwrap()).unwrap();
    fs::write(evidence.join("report.json"), serde_json::to_vec_pretty(&json!({"schema":1,"task":"M004","platform":GUEST_TARGET,"passed":true,"output_units":outputs.keys().collect::<Vec<_>>(),"compiled_scripts":compiled_scripts,"no_live_compilation":true,"full_interpreter_include_execution":false})).unwrap()).unwrap();
}
