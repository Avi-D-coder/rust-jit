use lrr::{
    core::{
        digest::{Digest, StableEncoder},
        profile::GUEST_TARGET,
    },
    project::{
        cargo_units::{ArtifactProfiles, Selection, UnitContext, UnitGraph, UnitRole},
        tool_manifest::{ArtifactKind, CompilerArtifactMessage, capture_compiler_artifacts},
    },
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn installed(toolchain: &str, tool: &str) -> Result<PathBuf, String> {
    let result = super::command_record("rustup", &["which", "--toolchain", toolchain, tool])?;
    if result["success"] != true {
        return Err(result.to_string());
    }
    let path = PathBuf::from(result["stdout"].as_str().ok_or("missing tool path")?.trim());
    if !path.is_absolute() || !path.is_file() {
        return Err("installed tool is not an absolute file".into());
    }
    Ok(path)
}
fn info(program: &Path, args: &[&str]) -> Result<Value, String> {
    super::command_record(program.to_str().ok_or("tool path is not UTF-8")?, args)
}
fn cfg_fingerprint(text: &str) -> Digest {
    let atoms: BTreeSet<_> = text.lines().collect();
    let mut e = StableEncoder::new("lrr/v2/captured-compiler-cfg");
    e.sequence(&atoms.into_iter().collect::<Vec<_>>(), |e, s| e.string(s));
    e.finish()
}

pub fn run(root: &Path) -> Result<bool, String> {
    let pin: toml::Value = toml::from_str(
        &fs::read_to_string(root.join("tools/cargo-planner.toml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if pin["channel"].as_str() != Some("nightly-2026-10-01")
        || pin["project_toolchain"].as_str() != Some("1.99.0")
    {
        return Err("toolchain pins differ from M002 contract".into());
    }
    let planner = installed("nightly-2026-10-01", "cargo")?;
    let compiler = installed("1.99.0", "rustc")?;
    let compiler_hash = Digest::of_bytes(&fs::read(&compiler).map_err(|e| e.to_string())?);
    let planner_hash = Digest::of_bytes(&fs::read(&planner).map_err(|e| e.to_string())?);
    let version = info(&compiler, &["-vV"])?;
    if version["success"] != true {
        return Err(version.to_string());
    }
    let version_text = version["stdout"]
        .as_str()
        .ok_or("missing compiler version")?;
    if !version_text.lines().any(|s| s == "release: 1.99.0") {
        return Err("compiler pin mismatch".into());
    }
    let host = version_text
        .lines()
        .find_map(|s| s.strip_prefix("host: "))
        .ok_or("missing compiler host")?
        .to_owned();
    let host_cfg = info(&compiler, &["--print", "cfg"])?;
    let target_cfg = info(&compiler, &["--print", "cfg", "--target", GUEST_TARGET])?;
    if host_cfg["success"] != true || target_cfg["success"] != true {
        return Err("compiler cfg capture failed".into());
    }
    let mut key = StableEncoder::new("lrr/v2/planner-project-compiler-cache");
    key.digest(planner_hash);
    key.digest(compiler_hash);
    key.string(version_text);
    key.string(GUEST_TARGET);
    let key = key.finish();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let relative = format!(
        "artifacts/compile-time-probe/driver-{}-{nonce}",
        std::process::id()
    );
    let evidence = root.join(&relative);
    let target = root.join(".lrr/tool-target").join(key.hex());
    let command = super::command_record(
        "python3",
        &[
            "tools/probe_compile_time_deps.py",
            "--allow-build-tools",
            "--planner",
            planner.to_str().ok_or("planner path encoding")?,
            "--rustc",
            compiler.to_str().ok_or("compiler path encoding")?,
            "--evidence-dir",
            evidence.to_str().ok_or("evidence path encoding")?,
            "--target",
            GUEST_TARGET,
            "--target-dir",
            target.to_str().ok_or("target path encoding")?,
        ],
    )?;
    let mut checks = vec![super::check(
        "controlled_cargo_capability",
        if command["success"] == true {
            Ok(command)
        } else {
            Err(command.to_string())
        },
    )];
    let prepared = (|| {
        let messages = fs::read_to_string(evidence.join("lib-tests-first.stdout"))
            .map_err(|e| e.to_string())?;
        let mut artifacts = Vec::new();
        for line in messages.lines() {
            let message: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
            if message["reason"] != "compiler-artifact"
                || message["target"]["kind"] != json!(["proc-macro"])
            {
                continue;
            }
            let names = message["filenames"]
                .as_array()
                .ok_or("macro filenames absent")?;
            if names.len() != 1 {
                return Err("ambiguous prepared macro artifact".into());
            }
            let path = PathBuf::from(names[0].as_str().ok_or("macro path encoding")?);
            if !path.is_absolute() {
                return Err("macro path is not absolute".into());
            }
            let hash = blake3::hash(&fs::read(&path).map_err(|e| e.to_string())?)
                .to_hex()
                .to_string();
            artifacts.push(json!({"path":path,"raw_blake3":hash,
                "package_id":message["package_id"],"features":message["features"],
                "compiler_content":compiler_hash}));
        }
        if artifacts.len() != 1 {
            return Err("prepared macro artifact association is not unique".into());
        }
        Ok(json!({"artifacts":artifacts,"capture_phase":"after authorized tool preparation"}))
    })();
    checks.push(super::check("prepared_macro_artifact_identity", prepared));
    let report: Value =
        serde_json::from_slice(&fs::read(evidence.join("report.json")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if report["passed"] != true {
        return super::record("artifacts/gates/m002-components.json", checks);
    }
    let mut cfgs = BTreeMap::new();
    cfgs.insert(
        host.clone(),
        cfg_fingerprint(host_cfg["stdout"].as_str().ok_or("missing host cfg")?),
    );
    cfgs.insert(
        GUEST_TARGET.into(),
        cfg_fingerprint(target_cfg["stdout"].as_str().ok_or("missing target cfg")?),
    );
    for (variant, selection) in [
        (
            "lib-tests",
            Selection::Library {
                test: true,
                name: "lrr_tool_probe_app".into(),
            },
        ),
        (
            "integration",
            Selection::IntegrationTest("integration".into()),
        ),
    ] {
        let raw = fs::read(evidence.join(format!("{variant}-unit-graph.stdout")))
            .map_err(|e| e.to_string())?;
        let raw_value: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        let index = raw_value["roots"][0].as_u64().ok_or("missing unit root")? as usize;
        let package = raw_value["units"][index]["pkg_id"]
            .as_str()
            .ok_or("missing selected unit package")?
            .to_owned();
        let context = UnitContext {
            package,
            selection,
            ordinary_role: UnitRole::Live,
            host: host.clone(),
            target: GUEST_TARGET.into(),
            cfg_by_target: cfgs.clone(),
            rustc_fingerprint: compiler_hash,
        };
        let parsed = UnitGraph::parse(&raw, &context);
        let normalized=parsed.map_err(|e|e.to_string()).and_then(|graph|{
            let profiles = ArtifactProfiles::parse(&raw, &context).map_err(|e| e.to_string())?;
            let mut artifact_reports = BTreeMap::new();
            for attempt in ["first", "warm"] {
                let lines = fs::read_to_string(evidence.join(format!("{variant}-{attempt}.stdout"))).map_err(|e| e.to_string())?;
                let mut messages = vec![];
                let mut freshness = vec![];
                for line in lines.lines() {
                    let record: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
                    if record["reason"] == "compiler-artifact" {
                        let message = CompilerArtifactMessage::decode(line.as_bytes()).map_err(|e| e.to_string())?;
                        freshness.push(message.fresh()); messages.push(message);
                    }
                }
                let artifacts = capture_compiler_artifacts(&graph, &profiles, messages).map_err(|e| e.to_string())?;
                if !artifacts.values().any(|a| a.kind == ArtifactKind::HostLibrary)
                    || !artifacts.values().any(|a| a.kind == ArtifactKind::ProcMacroLibrary)
                    || !artifacts.values().any(|a| a.kind == ArtifactKind::BuildProgram)
                { return Err("controlled preparation did not report all host tool roles".into()); }
                let artifact_path = evidence.join(format!("{variant}-{attempt}-captured-artifacts.json"));
                fs::write(&artifact_path, serde_json::to_vec_pretty(&artifacts).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                artifact_reports.insert(attempt, json!({"path":artifact_path,"unit_count":artifacts.len(),"cargo_freshness":freshness}));
            }
            let path=evidence.join(format!("{variant}-normalized-units.json"));
            fs::write(&path,serde_json::to_vec_pretty(&graph).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            Ok(json!({"units":graph.units.len(),"roots":graph.roots,"artifact":path,"compiler_cfg":{"host":host_cfg,"target":target_cfg},"compiled_tool_artifacts":artifact_reports}))
        });
        checks.push(super::check(
            &format!("{variant}:stable_unit_publication"),
            normalized,
        ));
    }
    super::record("artifacts/gates/m002-components.json", checks)
}
