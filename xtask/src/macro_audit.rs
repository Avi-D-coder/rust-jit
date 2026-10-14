//! Audit the active macro dependency closure; never build the upstream workspace.
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

fn forbidden_component(name: &str) -> bool {
    matches!(
        name,
        "hir"
            | "hir-def"
            | "hir-ty"
            | "ide"
            | "ide-db"
            | "rustc_driver"
            | "rustc-driver"
            | "rustc_middle"
            | "rustc-interface"
    ) || name.starts_with("ra_ap_hir_ty")
        || name.starts_with("ra_ap_ide")
}
pub fn source_violations(source: &str) -> Vec<String> {
    let facts = lrr::syntax::audit::source_facts(source);
    let mut failures = vec![];
    if facts.parse_errors != 0 {
        failures.push(format!("adapter syntax errors: {}", facts.parse_errors));
    }
    if facts.unsafe_tokens != 0 {
        failures.push("local adapter unsafe is forbidden".into());
    }
    for path in facts.paths.into_iter().chain(facts.method_names) {
        if path.split("::").any(|part| {
            forbidden_component(&part.replace('_', "-"))
                || matches!(
                    part,
                    "infer_body"
                        | "InferenceContext"
                        | "Semantics"
                        | "mir"
                        | "MirBody"
                        | "infer_query"
                        | "rustc_driver"
                        | "rustc_middle"
                )
        }) {
            failures.push(format!("forbidden analyzer inference/model path: {path}"));
        }
        if path.starts_with("libloading") {
            failures.push("macro loading belongs in the external server".into());
        }
    }
    failures
}

pub fn dependency_closure(metadata: &Value, start_name: &str) -> Result<BTreeSet<String>, String> {
    scoped_dependency_closure(metadata, start_name, None)
}
fn scoped_dependency_closure(
    metadata: &Value,
    start_name: &str,
    private_adapter: Option<&str>,
) -> Result<BTreeSet<String>, String> {
    let packages = metadata["packages"]
        .as_array()
        .ok_or("missing Cargo packages")?;
    let start = packages
        .iter()
        .find(|p| p["name"] == start_name)
        .ok_or("missing closure root")?["id"]
        .as_str()
        .ok_or("missing package ID")?
        .to_owned();
    let nodes: BTreeMap<_, _> = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("missing Cargo resolve nodes")?
        .iter()
        .map(|n| (n["id"].as_str().unwrap_or(""), n))
        .collect();
    let mut seen = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(id) = pending.pop() {
        if private_adapter == Some(id.as_str()) {
            continue;
        }
        if !seen.insert(id.clone()) {
            continue;
        }
        let node = nodes
            .get(id.as_str())
            .ok_or("package missing resolve node")?;
        if node["features"]
            .as_array()
            .ok_or("missing features")?
            .iter()
            .any(|f| f == "in-rust-tree")
        {
            return Err("in-rust-tree feature enables compiler-private paths".into());
        }
        for dependency in node["deps"].as_array().ok_or("missing deps")? {
            pending.push(
                dependency["pkg"]
                    .as_str()
                    .ok_or("missing dependency pkg")?
                    .to_owned(),
            );
        }
    }
    Ok(seen)
}

pub fn audit(root: &Path) -> Result<Value, String> {
    let report = super::command_record(
        "cargo",
        &["metadata", "--format-version", "1", "--locked", "--offline"],
    )?;
    if report["success"] != true {
        return Err(report.to_string());
    }
    let metadata: Value =
        serde_json::from_str(report["stdout"].as_str().ok_or("missing metadata")?)
            .map_err(|e| e.to_string())?;
    let closure = dependency_closure(&metadata, "lrr-expand-adapter")?;
    // D08 permits the separately audited adapter's private span interner.
    // Everything reached through another runtime dependency remains visible.
    let adapter = metadata["packages"]
        .as_array()
        .ok_or("missing Cargo packages")?
        .iter()
        .filter(|p| p["name"] == "lrr-expand-adapter")
        .collect::<Vec<_>>();
    if adapter.len() != 1 {
        return Err("private adapter package identity is not unique".into());
    }
    let runtime = scoped_dependency_closure(
        &metadata,
        "lrr",
        Some(adapter[0]["id"].as_str().ok_or("missing adapter ID")?),
    )?;
    let mut packages = vec![];
    let mut failures = vec![];
    for package in metadata["packages"].as_array().ok_or("missing packages")? {
        let id = package["id"].as_str().ok_or("missing ID")?;
        let name = package["name"].as_str().ok_or("missing name")?;
        if runtime.contains(id) && name.starts_with("salsa") {
            failures.push("Salsa may not enter the runtime query DB closure".into());
        }
        if !closure.contains(id) {
            continue;
        }
        if forbidden_component(name) {
            failures.push(format!("forbidden adapter dependency: {name}"));
        }
        if package["source"]
            .as_str()
            .is_some_and(|s| s.starts_with("git+"))
        {
            failures.push(format!("unpinned git adapter dependency: {name}"));
        }
        packages.push(
            json!({"name":name,"id":id,"license":package["license"],"source":package["source"]}),
        );
    }
    fn visit(at: &Path, failures: &mut Vec<String>) -> Result<(), String> {
        let mut paths = fs::read_dir(at)
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        paths.sort();
        for path in paths {
            if path.is_dir() {
                visit(&path, failures)?;
            } else if path.extension().is_some_and(|e| e == "rs") {
                failures.extend(
                    source_violations(&fs::read_to_string(&path).map_err(|e| e.to_string())?)
                        .into_iter()
                        .map(|e| format!("{}: {e}", path.display())),
                );
            }
        }
        Ok(())
    }
    visit(&root.join("crates/lrr-expand-adapter/src"), &mut failures)?;
    if !failures.is_empty() {
        return Err(failures.join("\n"));
    }
    packages.sort_by_key(|p| p["id"].as_str().unwrap_or("").to_owned());
    Ok(
        json!({"source_commit":lrr_expand_adapter::UPSTREAM_COMMIT,"adapter_dependency_closure":packages,"runtime_uses_salsa":false}),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn cardinality_patch_reconstructs_pinned_source_and_rejects_tampering() {
        let output = std::process::Command::new("python3")
            .arg(crate::root().join("tools/test_macro_patch_audit.py"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    fn intentional_infer_body_import_is_rejected() {
        for text in [
            "use hir_ty::infer::infer_body;",
            "fn probe(){ analyzer::infer_body(); }",
            "fn probe(){ db.infer_body(); }",
            "use hir::{Semantics as S};",
            "use rustc_driver;",
        ] {
            assert!(!super::source_violations(text).is_empty(), "{text}");
        }
        assert!(
            super::source_violations("// infer_body\nconst NAME: &str = \"hir_ty::infer_body\";")
                .is_empty()
        );
    }
    #[test]
    fn resolved_adapter_closure_has_no_inference_engine() {
        super::audit(&crate::root()).unwrap();
    }
    #[test]
    fn runtime_query_dependency_audit_keeps_adapter_interner_private_and_rejects_other_paths() {
        let mut metadata = serde_json::json!({"packages":[{"name":"lrr","id":"runtime"},{"name":"lrr-expand-adapter","id":"adapter"},{"name":"salsa","id":"salsa"},{"name":"other-helper","id":"other"}],"resolve":{"nodes":[{"id":"runtime","features":[],"deps":[{"pkg":"adapter"}]},{"id":"adapter","features":[],"deps":[{"pkg":"salsa"}]},{"id":"salsa","features":[],"deps":[]},{"id":"other","features":[],"deps":[{"pkg":"salsa"}]}]}});
        let runtime = super::scoped_dependency_closure(&metadata, "lrr", Some("adapter")).unwrap();
        assert!(!runtime.contains("salsa"));
        assert!(
            super::dependency_closure(&metadata, "lrr-expand-adapter")
                .unwrap()
                .contains("salsa")
        );
        metadata["resolve"]["nodes"][0]["deps"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"pkg":"other"}));
        assert!(
            super::scoped_dependency_closure(&metadata, "lrr", Some("adapter"))
                .unwrap()
                .contains("salsa")
        );
        metadata["resolve"]["nodes"][0]["deps"] = serde_json::json!([{"pkg":"salsa"}]);
        assert!(
            super::scoped_dependency_closure(&metadata, "lrr", Some("adapter"))
                .unwrap()
                .contains("salsa")
        );
    }
    #[test]
    fn pinned_macro_vendor_hashes_validate() {
        let output = std::process::Command::new("python3")
            .arg(crate::root().join("tools/bootstrap_vendor.py"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }
}
