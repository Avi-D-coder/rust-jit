mod audit;
mod tasks;
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
    let spec = fs::read_to_string(root().join("lrr_rust2024_hybrid_cargo_implementation_spec.md"))
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
            "x86_64-unknown-linux-gnu",
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
    let record = json!({"schema":1,"target":"x86_64-unknown-linux-gnu","packages":inventory});
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
    let passed = success(&checks);
    let report = json!({"schema":1,"runtime_version":"0.1.0","semantics_profile":"lrr-2024-v1","build_id":blake3::hash(&fs::read(std::env::current_exe().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?).to_hex().to_string(),"passed":passed,"host":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"pointer_width":usize::BITS},"git":git,"toolchain":toolchain,"checks":checks});
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
fn bootstrap() -> Result<bool, String> {
    let checks = vec![
        check(
            "supported_host",
            lrr::core::profile::require_supported_host()
                .map(|_| json!("x86_64-unknown-linux-gnu"))
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
fn gate(number: &str) -> Result<bool, String> {
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
    let names = [
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
        "dependency_license_inventory_is_complete",
    ];
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
    checks.push(check(
        "host_tests",
        if tests["success"] == true {
            Ok(tests)
        } else {
            Err(tests.to_string())
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
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--ambient-import-probe") => {
            lrr::jit::abi::ambient_import_probe()?;
            Ok(true)
        }
        Some("bootstrap-audit") => bootstrap(),
        Some("architecture-audit") => {
            audit::architecture(&root())?;
            Ok(true)
        }
        Some("license-audit") => {
            license_inventory()?;
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
            | "release-audit",
        ) => Err(format!(
            "{} is not implemented; the runtime is not release ready",
            args[0]
        )),
        _ => Err(
            "usage: xtask bootstrap-audit|architecture-audit|vendor-audit|manifest-audit|gate NN"
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
