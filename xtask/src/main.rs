mod audit;
mod compile_time_probe;
mod gates;
mod macro_audit;
mod macro_server_probe;
mod query_gate;
mod tasks;
mod validation_source;
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, ExitCode},
    time::Instant,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}

fn direct_versions() -> Result<Value, String> {
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(root().join("Cargo.toml")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let dependencies = manifest["workspace"]["dependencies"]
        .as_table()
        .ok_or("missing workspace dependencies")?;
    let spec = fs::read_to_string(root().join("lrr_rust2024_macros_traits_implementation_spec.md"))
        .map_err(|e| e.to_string())?;
    let text = spec
        .split("### 3.2 Exact direct dependencies")
        .nth(1)
        .ok_or("missing dependency specification")?
        .split("```toml\n")
        .nth(1)
        .ok_or("missing TOML")?
        .split("```")
        .next()
        .unwrap();
    let expected: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    if manifest["workspace"]["dependencies"] != expected["workspace"]["dependencies"] {
        return Err("direct dependency pins differ from specification".into());
    }
    let lock: toml::Value =
        toml::from_str(&fs::read_to_string(root().join("Cargo.lock")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let packages = lock["package"].as_array().ok_or("missing lock packages")?;
    let mut versions = serde_json::Map::new();
    for (name, dep) in dependencies {
        let version = dep
            .as_str()
            .or_else(|| dep.get("version").and_then(|v| v.as_str()))
            .ok_or("non-version dependency")?
            .trim_start_matches('=');
        if !packages.iter().any(|p| {
            p["name"].as_str() == Some(name)
                && p["version"]
                    .as_str()
                    .is_some_and(|v| v.split('+').next() == Some(version))
        }) {
            return Err(format!("lock does not contain exact {name} {version}"));
        }
        versions.insert(name.clone(), json!(version));
    }
    Ok(Value::Object(versions))
}

fn license_inventory() -> Result<Value, String> {
    let report = command_record(
        "cargo",
        &[
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--offline",
            "--filter-platform",
            lrr::core::profile::GUEST_TARGET,
        ],
    )?;
    if report["success"] != true {
        return Err(report.to_string());
    }
    let metadata: Value =
        serde_json::from_str(report["stdout"].as_str().ok_or("missing metadata stdout")?)
            .map_err(|e| e.to_string())?;
    let mut inventory = Vec::new();
    for package in metadata["packages"].as_array().ok_or("missing packages")? {
        if package["license"].is_null() && package["license_file"].is_null() {
            return Err(format!("missing license declaration for {}", package["id"]));
        }
        inventory.push(json!({"id":package["id"], "name":package["name"], "version":package["version"], "license":package["license"], "license_file":package["license_file"], "source":package["source"]}));
    }
    inventory.sort_by_key(|p| p["id"].as_str().unwrap_or("").to_owned());
    let record = json!({"schema":1,"target":lrr::core::profile::GUEST_TARGET,"packages":inventory});
    fs::create_dir_all(root().join("artifacts")).map_err(|e| e.to_string())?;
    fs::write(
        root().join("artifacts/dependency-licenses.json"),
        serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(json!({"packages":inventory.len(), "artifact":"artifacts/dependency-licenses.json"}))
}

fn check(name: &str, result: Result<Value, String>) -> Value {
    match result {
        Ok(detail) => json!({"name":name,"status":"passed","detail":detail}),
        Err(error) => json!({"name":name,"status":"failed","error":error}),
    }
}
fn success(checks: &[Value]) -> bool {
    checks.iter().all(|c| c["status"] == "passed")
}
fn output(command: &mut Command) -> Result<std::process::Output, String> {
    command.output().map_err(|e| e.to_string())
}
fn command_record(program: &str, args: &[&str]) -> Result<Value, String> {
    let start = Instant::now();
    let result = output(Command::new(program).args(args).current_dir(root()))?;
    Ok(
        json!({"command":program,"arguments":args,"exit_code":result.status.code(),"success":result.status.success(),"elapsed_ms":start.elapsed().as_millis(),"stdout":String::from_utf8_lossy(&result.stdout),"stderr":String::from_utf8_lossy(&result.stderr)}),
    )
}
fn record(path: &str, checks: Vec<Value>) -> Result<bool, String> {
    let toolchain = command_record("rustc", &["-Vv"])?;
    let git = command_record("git", &["rev-parse", "HEAD"])?;
    let working_tree = command_record("git", &["status", "--porcelain"])?;
    let passed = success(&checks);
    let report = json!({"schema":1,"runtime_version":lrr::core::profile::RUNTIME_VERSION,"semantics_profile":lrr::core::profile::SEMANTICS_PROFILE,"spec_revision":"5.0","evidence_scope":"provisional infrastructure; revision-5 release integrations pending","build_id":blake3::hash(&fs::read(std::env::current_exe().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?).to_hex().to_string(),"passed":passed,"host":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"pointer_width":usize::BITS},"git":git,"working_tree":working_tree,"toolchain":toolchain,"checks":checks});
    let destination = root().join(path);
    fs::create_dir_all(destination.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(
        &destination,
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(passed)
}
fn ambient_check() -> Result<Value, String> {
    let start = Instant::now();
    let result = output(
        Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .arg("--ambient-import-probe")
            .current_dir(std::env::temp_dir()),
    )?;
    let stderr = String::from_utf8_lossy(&result.stderr);
    if result.status.success() || !stderr.contains("can't resolve symbol getpid") {
        return Err(format!(
            "ambient probe did not fail for the intended closed import: {stderr}"
        ));
    }
    Ok(
        json!({"exit_code":result.status.code(),"elapsed_ms":start.elapsed().as_millis(),"stderr":stderr}),
    )
}
fn compile_time_capability() -> Result<Value, String> {
    if !macro_adapter_audit()? {
        return Err("M001 component audit failed: artifacts/gates/m001-components.json".into());
    }
    if !compile_time_probe::run(&root())? {
        return Err("M002 capability failed: artifacts/gates/m002-components.json".into());
    }
    if !macro_server_probe::run(&root())? {
        return Err("M003 compatibility failed: artifacts/gates/m003-components.json".into());
    }
    Ok(
        json!({"fresh_component_reports":["artifacts/gates/m001-components.json",
        "artifacts/gates/m002-components.json","artifacts/gates/m003-components.json"],
        "scope":"authorized controlled tool preparation and matched external echo"}),
    )
}
fn compile_time_tools_gate() -> Result<bool, String> {
    let checks = vec![
        check(
            "supported_host",
            lrr::core::profile::require_supported_host()
                .map(|_| json!(lrr::core::profile::GUEST_TARGET))
                .map_err(|e| e.to_string()),
        ),
        check("compile_time_capability", compile_time_capability()),
        check(
            "acceptance_tests",
            (|| {
                let result =
                    command_record("cargo", &["test", "--workspace", "--locked", "--offline"])?;
                if result["success"] != true {
                    return Err(result.to_string());
                }
                let stdout = result["stdout"].as_str().ok_or("missing test output")?;
                let required: Vec<String> = [
                    "changed_server_content_is_rejected_before_execution",
                    "missing_macro_authority_precedes_filesystem_and_process_access",
                    "cargo_array_reordering_preserves_unit_and_edge_identities",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect();
                let unpassed = gates::unpassed(&required, stdout);
                if !unpassed.is_empty() {
                    return Err(format!(
                        "required acceptance tests did not pass: {unpassed:?}"
                    ));
                }
                Ok(result)
            })(),
        ),
    ];
    record("artifacts/gates/compile-time-tools.json", checks)
}
fn bootstrap() -> Result<bool, String> {
    let checks = vec![
        check(
            "supported_host",
            lrr::core::profile::require_supported_host()
                .map(|_| json!(lrr::core::profile::GUEST_TARGET))
                .map_err(|e| e.to_string()),
        ),
        check(
            "pinned_toolchain",
            (|| {
                let result = command_record("rustc", &["-Vv"])?;
                if !result["stdout"]
                    .as_str()
                    .unwrap_or("")
                    .contains("release: 1.99.0\n")
                {
                    return Err("host compiler differs from 1.99.0".into());
                }
                Ok(result)
            })(),
        ),
        check("direct_versions", direct_versions()),
        check(
            "macro_adapter_dependency_boundary",
            macro_audit::audit(&root()),
        ),
        check("macro_adapter_probe", macro_adapter_probe()),
        check("macro_source_provenance", macro_source_provenance()),
        check("compile_time_capability", compile_time_capability()),
        check(
            "parser_counts_cold_function_without_resolving",
            lrr::syntax::parse::parser_smoke().map(|_| json!(true)),
        ),
        check(
            "jit_adds_seven",
            lrr::jit::abi::native_smoke(false).and_then(|v| {
                if v == 42 {
                    Ok(json!(v))
                } else {
                    Err(format!("wrong result {v}"))
                }
            }),
        ),
        check(
            "jit_calls_registered_helper",
            lrr::jit::abi::native_smoke(true).and_then(|v| {
                if v == 42 {
                    Ok(json!(v))
                } else {
                    Err(format!("wrong result {v}"))
                }
            }),
        ),
        check("jit_does_not_resolve_ambient_symbol", ambient_check()),
    ];
    record("artifacts/bootstrap.json", checks)
}
fn macro_source_provenance() -> Result<Value, String> {
    let result = command_record("python3", &["tools/bootstrap_vendor.py"])?;
    if result["success"] != true {
        return Err(result.to_string());
    }
    Ok(result)
}
fn macro_adapter_probe() -> Result<Value, String> {
    let result = lrr_expand_adapter::bootstrap_probe().map_err(|e| e.to_string())?;
    if result.editions.len() != 4 || result.editions.iter().any(|e| e.parsed_rules != 2) {
        return Err("upstream matcher bootstrap output mismatch".into());
    }
    serde_json::to_value(result).map_err(|e| e.to_string())
}
fn macro_adapter_audit() -> Result<bool, String> {
    let checks = vec![
        check("source_provenance", macro_source_provenance()),
        check(
            "dependency_and_source_boundary",
            macro_audit::audit(&root()),
        ),
        check("pinned_component_probe", macro_adapter_probe()),
    ];
    record("artifacts/gates/m001-components.json", checks)
}
fn gate(number: &str) -> Result<bool, String> {
    if matches!(number, "02" | "03") {
        let prerequisite_number = if number == "03" { "02" } else { "01" };
        let prerequisite = gate(prerequisite_number)?;
        let execution = if prerequisite {
            (|| {
                let fixtures: toml::Value = toml::from_str(
                    &fs::read_to_string(root().join("compat/fixtures.toml"))
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let report: serde_json::Value = serde_json::from_str(
                    &fs::read_to_string(root().join("artifacts/gates/00.json"))
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let stdout = report["checks"]
                    .as_array()
                    .ok_or("missing gate checks")?
                    .iter()
                    .find(|c| c["name"] == "host_tests")
                    .and_then(|c| c["detail"]["stdout"].as_str())
                    .ok_or("missing fresh host test evidence")?;
                if number == "03" {
                    gates::control_flow_calls(&fixtures, stdout)
                } else {
                    gates::scalar_execution(&fixtures, stdout)
                }
            })()
        } else {
            Err(format!("gate {prerequisite_number} prerequisite failed"))
        };
        return record(
            &format!("artifacts/gates/{number}.json"),
            vec![check(
                if number == "03" {
                    "control_call_test_component_acceptance"
                } else {
                    "scalar_execution_acceptance"
                },
                execution,
            )],
        );
    }
    if number == "01" {
        let substrate = gate("00")?;
        return record(
            "artifacts/gates/01.json",
            vec![
                check(
                    "source_query_acceptance",
                    if substrate {
                        Ok(json!(true))
                    } else {
                        Err("gate 00 source/query/cache fixtures failed".into())
                    },
                ),
                check("initial_edit_matrix", query_gate::edit_matrix()),
                check(
                    "cache_ownership_completion",
                    query_gate::retired_payload_accounting(),
                ),
            ],
        );
    }
    if number != "00" {
        return record(
            &format!("artifacts/gates/{number}.json"),
            vec![check(
                "gate_implementation",
                Err(format!(
                    "milestone {number} is not implemented; no checks skipped as passed"
                )),
            )],
        );
    }
    let mut checks = Vec::new();
    let listing = command_record(
        "cargo",
        &[
            "test",
            "--workspace",
            "--locked",
            "--offline",
            "--",
            "--list",
        ],
    )?;
    let mut names: Vec<String> = [
        "workspace_has_exact_members",
        "toolchain_is_pinned",
        "task_ledger_validates",
        "direct_versions_match_spec",
        "vendor_patch_is_exact",
        "parser_counts_cold_function_without_resolving",
        "jit_adds_seven",
        "jit_calls_registered_helper",
        "jit_does_not_resolve_ambient_symbol",
        "forbidden_worker_import_is_rejected",
        "unsafe_outside_boundary_is_rejected",
        "pinned_macro_components_parse_rules_in_all_supported_editions",
        "intentional_infer_body_import_is_rejected",
        "resolved_adapter_closure_has_no_inference_engine",
        "pinned_macro_vendor_hashes_validate",
        "dependency_license_inventory_is_complete",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let fixtures: toml::Value = toml::from_str(
        &fs::read_to_string(root().join("compat/fixtures.toml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    for fixture in fixtures["fixture"]
        .as_array()
        .ok_or("missing fixture inventory")?
    {
        names.push(
            fixture["test"]
                .as_str()
                .ok_or("fixture without test name")?
                .to_owned(),
        );
    }
    names.sort();
    names.dedup();
    let missing: Vec<_> = names
        .iter()
        .filter(|name| {
            !listing["stdout"]
                .as_str()
                .unwrap_or("")
                .contains(&format!("{name}: test"))
        })
        .collect();
    checks.push(check(
        "test_inventory",
        if listing["success"] == true && missing.is_empty() {
            Ok(listing)
        } else {
            Err(format!(
                "missing required tests: {missing:?}; list command: {listing}"
            ))
        },
    ));
    let tests = command_record("cargo", &["test", "--workspace", "--locked", "--offline"])?;
    let unpassed = gates::unpassed(&names, tests["stdout"].as_str().unwrap_or(""));
    checks.push(check(
        "host_tests",
        if tests["success"] == true && unpassed.is_empty() {
            Ok(tests)
        } else {
            Err(format!(
                "required fixtures did not pass: {unpassed:?}; test command: {tests}"
            ))
        },
    ));
    checks.push(check(
        "architecture",
        audit::architecture(&root()).map(|_| json!(true)),
    ));
    checks.push(check(
        "fixture_references",
        audit::validate_references(&root()).map(|_| json!(true)),
    ));
    checks.push(check("dependency_licenses", license_inventory()));
    checks.push(check(
        "ledger",
        tasks::validate(&root()).map(|_| json!(true)),
    ));
    checks.push(check(
        "bootstrap",
        bootstrap().and_then(|ok| {
            if ok {
                Ok(json!(true))
            } else {
                Err("bootstrap checks failed: artifacts/bootstrap.json".into())
            }
        }),
    ));
    record("artifacts/gates/00.json", checks)
}
fn run() -> Result<bool, String> {
    let expected = std::env::var_os("LRR_VALIDATION_SOURCE_ROOT");
    validation_source::checked_root(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        expected.as_deref().map(std::path::Path::new),
    )?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--ambient-import-probe") => {
            lrr::jit::abi::ambient_import_probe()?;
            Ok(true)
        }
        Some("bootstrap-audit") => bootstrap(),
        Some("architecture-audit") => {
            audit::architecture(&root())?;
            macro_audit::audit(&root())?;
            Ok(true)
        }
        Some("macro-adapter-audit") => macro_adapter_audit(),
        Some("compile-time-probe") => compile_time_probe::run(&root()),
        Some("macro-server-probe") => macro_server_probe::run(&root()),
        Some("compile-time-tools-gate") => compile_time_tools_gate(),
        Some("license-audit") => {
            license_inventory()?;
            Ok(true)
        }
        Some("fixture-reference-audit") => {
            audit::validate_references(&root())?;
            Ok(true)
        }
        Some("manifest-audit") => {
            direct_versions()?;
            tasks::validate(&root())?;
            Ok(true)
        }
        Some("vendor-audit") => {
            let report = command_record("python3", &["tools/vendor_audit.py"])?;
            println!("{report}");
            Ok(report["success"] == true)
        }
        Some("gate") => gate(args.get(1).ok_or("usage: xtask gate NN")?),
        Some(
            "verify-all"
            | "edition2024-gate"
            | "native-prepare-gate"
            | "native-abi-gate"
            | "hybrid-gate"
            | "hybrid-bench"
            | "release-audit"
            | "expansion-gate"
            | "discovery-gate"
            | "trait-goals-gate"
            | "trait-runtime-gate"
            | "frontend-cache-gate"
            | "native-macro-gate",
        ) => Err(format!(
            "{} is not implemented; the runtime is not release ready",
            args[0]
        )),
        _ => Err(
            "usage: xtask bootstrap-audit|architecture-audit|vendor-audit|manifest-audit|fixture-reference-audit|gate NN"
                .into(),
        ),
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
