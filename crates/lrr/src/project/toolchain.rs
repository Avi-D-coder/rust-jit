//! Explicit preparation-time tool capture. Never called by query providers.
use crate::core::{diagnostic::Diagnostic, digest::Digest};
use serde::Serialize;
use std::{fs, path::PathBuf, process::Command};

#[derive(Debug, Clone, Serialize)]
pub struct ProjectToolchain {
    pub selection: String,
    pub compiler: PathBuf,
    pub compiler_content: Digest,
    pub compiler_version: String,
    pub compiler_commit: String,
    pub host: String,
    pub sysroot: PathBuf,
    pub server: PathBuf,
    pub server_raw_blake3: String,
    pub server_component_manifest: PathBuf,
    pub component_manifest_content: Digest,
}
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SERVER-MISMATCH", message.into())
}
fn output(command: &mut Command) -> Result<String, Diagnostic> {
    let result = command.output().map_err(|e| fault(e.to_string()))?;
    if !result.status.success() {
        return Err(fault(format!(
            "tool capture failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    String::from_utf8(result.stdout).map_err(|_| fault("tool output is not UTF-8"))
}
pub fn capture(selection: &str) -> Result<ProjectToolchain, Diagnostic> {
    if selection != "1.99.0" {
        return Err(fault(
            "project compiler tuple requires its own passing capability fixture",
        ));
    }
    let compiler = PathBuf::from(
        output(Command::new("rustup").args(["which", "--toolchain", selection, "rustc"]))?.trim(),
    );
    if !compiler.is_absolute() {
        return Err(fault("compiler path is not absolute"));
    }
    let version = output(Command::new(&compiler).arg("-vV"))?;
    if !version.lines().any(|s| s == "release: 1.99.0") {
        return Err(fault("project compiler release mismatch"));
    }
    let commit = version
        .lines()
        .find_map(|s| s.strip_prefix("commit-hash: "))
        .ok_or_else(|| fault("compiler commit absent"))?
        .to_owned();
    if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(fault("invalid compiler commit"));
    }
    let host = version
        .lines()
        .find_map(|s| s.strip_prefix("host: "))
        .ok_or_else(|| fault("compiler host absent"))?
        .to_owned();
    let sysroot =
        PathBuf::from(output(Command::new(&compiler).args(["--print", "sysroot"]))?.trim());
    if !sysroot.is_absolute() {
        return Err(fault("sysroot is not absolute"));
    }
    let server = sysroot.join("libexec/rust-analyzer-proc-macro-srv");
    let mut owner = None;
    for entry in fs::read_dir(sysroot.join("lib/rustlib")).map_err(|e| fault(e.to_string()))? {
        let path = entry.map_err(|e| fault(e.to_string()))?.path();
        if path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("manifest-"))
            && path.is_file()
        {
            let text = fs::read_to_string(&path).map_err(|e| fault(e.to_string()))?;
            if text
                .lines()
                .any(|s| s == "file:libexec/rust-analyzer-proc-macro-srv")
            {
                if owner.is_some() {
                    return Err(fault("macro server has ambiguous component ownership"));
                }
                owner = Some(path);
            }
        }
    }
    let manifest =
        owner.ok_or_else(|| fault("matched sysroot component does not contain macro server"))?;
    let bytes = fs::read(&server).map_err(|e| fault(format!("matched server unavailable: {e}")))?;
    Ok(ProjectToolchain {
        selection: selection.into(),
        compiler_content: Digest::of_bytes(&fs::read(&compiler).map_err(|e| fault(e.to_string()))?),
        compiler,
        compiler_version: version,
        compiler_commit: commit,
        host,
        sysroot,
        server,
        server_raw_blake3: blake3::hash(&bytes).to_hex().to_string(),
        component_manifest_content: Digest::of_bytes(
            &fs::read(&manifest).map_err(|e| fault(e.to_string()))?,
        ),
        server_component_manifest: manifest,
    })
}
