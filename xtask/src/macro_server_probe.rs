use lrr::project::{
    cargo_units::{TargetKind, UnitGraph, UnitRole},
    toolchain,
};
use lrr_expand_adapter::proc_client::{EchoProbeRequest, echo_probe};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};

pub fn run(root: &Path) -> Result<bool, String> {
    let toolchain = toolchain::capture("1.99.0").map_err(|e| e.to_string())?;
    let previous: Value = serde_json::from_slice(
        &fs::read(root.join("artifacts/gates/m002-components.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if previous["passed"] != true {
        return Err("M003 requires passing M002 component evidence".into());
    }
    let graph_path = previous["checks"]
        .as_array()
        .ok_or("missing M002 checks")?
        .iter()
        .find(|c| c["name"] == "lib-tests:stable_unit_publication")
        .ok_or("missing normalized unit publication")?["detail"]["artifact"]
        .as_str()
        .ok_or("missing graph path")?;
    let graph_path = Path::new(graph_path);
    let evidence = graph_path.parent().ok_or("missing probe directory")?;
    let graph: UnitGraph =
        serde_json::from_slice(&fs::read(graph_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let lines =
        fs::read_to_string(evidence.join("lib-tests-first.stdout")).map_err(|e| e.to_string())?;
    let messages: Vec<Value> = lines
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let artifacts: Vec<_> = messages
        .iter()
        .filter(|m| {
            m["reason"] == "compiler-artifact" && m["target"]["kind"] == json!(["proc-macro"])
        })
        .collect();
    if artifacts.len() != 1 {
        return Err("macro artifact association is not unique".into());
    }
    let artifact = artifacts[0];
    let features: BTreeSet<_> = artifact["features"]
        .as_array()
        .ok_or("macro features absent")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or("feature is not a string")
        })
        .collect::<Result<_, _>>()?;
    let units: Vec<_> = graph
        .units
        .iter()
        .filter(|(_, u)| {
            u.key.role == UnitRole::ProcMacroHost
                && u.key.target_kind == TargetKind::ProcMacro
                && u.key.package == artifact["package_id"].as_str().unwrap_or("")
                && u.key.features == features
        })
        .collect();
    if units.len() != 1
        || units[0].1.key.rustc_fingerprint != toolchain.compiler_content
        || units[0].1.key.compile_target != toolchain.host
    {
        return Err("macro compiler/unit tuple mismatch".into());
    }
    let filenames = artifact["filenames"]
        .as_array()
        .ok_or("macro filenames absent")?;
    if filenames.len() != 1 {
        return Err("macro dynamic artifact path is ambiguous".into());
    }
    let path = std::path::PathBuf::from(filenames[0].as_str().ok_or("macro path type")?);
    let recorded = previous["checks"]
        .as_array()
        .ok_or("missing preparation checks")?
        .iter()
        .find(|c| c["name"] == "prepared_macro_artifact_identity")
        .ok_or("missing preparation-time macro identity")?;
    let identities = recorded["detail"]["artifacts"]
        .as_array()
        .ok_or("missing prepared artifacts")?;
    if identities.len() != 1
        || identities[0]["path"].as_str() != path.to_str()
        || identities[0]["package_id"] != artifact["package_id"]
        || identities[0]["features"] != artifact["features"]
        || identities[0]["compiler_content"] != json!(toolchain.compiler_content)
    {
        return Err("prepared macro artifact identity does not match its unit".into());
    }
    let artifact_hash = identities[0]["raw_blake3"]
        .as_str()
        .ok_or("missing preparation-time artifact hash")?
        .to_owned();
    let request = EchoProbeRequest {
        allow_macro_execution: true,
        server: toolchain.server.clone(),
        expected_server_raw_blake3: toolchain.server_raw_blake3.clone(),
        compiler_commit: toolchain.compiler_commit.clone(),
        artifact: path,
        expected_artifact_raw_blake3: artifact_hash,
        expected_protocol: 6,
        directory: root.join("fixtures/macros/tool-workspace"),
    };
    let probe = echo_probe(&request).map_err(|e| e.to_string());
    let mut checks = vec![super::check(
        "matched_external_echo",
        probe.map(|r| json!(r)),
    )];
    let mut unauthorized = request.clone();
    unauthorized.allow_macro_execution = false;
    checks.push(super::check(
        "separate_macro_authority_required",
        match echo_probe(&unauthorized) {
            Err(e) if e.code == "LRR-E-MACRO-PERMISSION" => Ok(json!({"diagnostic":e})),
            Err(e) => Err(e.to_string()),
            Ok(_) => Err("macro execution without authority was accepted".into()),
        },
    ));
    let mut changed = request.clone();
    changed.expected_artifact_raw_blake3 = "0".repeat(64);
    checks.push(super::check(
        "changed_artifact_refused_before_server_execution",
        match echo_probe(&changed) {
            Err(e)
                if e.code == "LRR-E-MACRO-SERVER-MISMATCH"
                    && e.message == "recorded tool/artifact hash mismatch" =>
            {
                Ok(json!({"diagnostic":e}))
            }
            Err(e) => Err(e.to_string()),
            Ok(_) => Err("changed artifact was accepted".into()),
        },
    ));
    let mut denied = request;
    let mut wrong_server = denied.clone();
    wrong_server.expected_server_raw_blake3 = "0".repeat(64);
    checks.push(super::check(
        "changed_server_refused_before_execution",
        match echo_probe(&wrong_server) {
            Err(e)
                if e.code == "LRR-E-MACRO-SERVER-MISMATCH"
                    && e.message == "recorded tool/artifact hash mismatch" =>
            {
                Ok(json!({"diagnostic":e}))
            }
            Err(e) => Err(e.to_string()),
            Ok(_) => Err("changed server was accepted".into()),
        },
    ));
    let mut wrong_compiler = denied.clone();
    wrong_compiler.compiler_commit = "0".repeat(40);
    checks.push(super::check(
        "compiler_tuple_mismatch_refused_before_load",
        match echo_probe(&wrong_compiler) {
            Err(e)
                if e.code == "LRR-E-MACRO-SERVER-MISMATCH"
                    && e.message == "server/compiler tuple mismatch before macro loading" =>
            {
                Ok(json!({"diagnostic":e}))
            }
            Err(e) => Err(e.to_string()),
            Ok(_) => Err("wrong compiler tuple was accepted".into()),
        },
    ));
    denied.expected_protocol = u32::MAX;
    let rejected = match echo_probe(&denied) {
        Err(e)
            if e.code == "LRR-E-MACRO-SERVER-MISMATCH"
                && e.message.starts_with("unsupported server protocol") =>
        {
            Ok(json!({"diagnostic":e,"phase":"protocol preflight before load_dylib"}))
        }
        Err(e) => Err(e.to_string()),
        Ok(_) => Err("mismatched protocol was accepted".into()),
    };
    checks.push(super::check(
        "protocol_mismatch_refused_before_load",
        rejected,
    ));
    let manifest = json!({"schema":1,"evidence_scope":"controlled M003 selected-platform component; Linux port validation deferred","toolchain":toolchain,"adapter_source_commit":lrr_expand_adapter::UPSTREAM_COMMIT,"macro_unit":units[0].0,"macro_execution_authorized":true,"checks":checks});
    fs::write(
        root.join("artifacts/m003-server-compatibility.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    super::record("artifacts/gates/m003-components.json", checks)
}
