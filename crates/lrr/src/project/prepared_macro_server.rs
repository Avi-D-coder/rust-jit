//! E-M013-001 preparation-only capture. Never called by a query provider.
use super::tool_manifest::{ToolBinary, ToolchainRecord, absolute, fault};
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncode, StableEncoder},
};
use lrr_expand_adapter::proc_client::{EchoProbeReport, EchoProbeRequest, echo_probe};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedMacroServer {
    pub schema: u32,
    pub repair: String,
    pub source_root: String,
    pub source_commit: String,
    pub compiler_commit: String,
    pub baseline: ToolBinary,
    pub compiler_dev_manifest: String,
    pub compiler_dev_content: Digest,
    pub preparation_receipt: String,
    pub preparation_receipt_content: Digest,
    pub sources: BTreeMap<String, Digest>,
    pub compatibility_artifact: String,
    pub compatibility_content: Digest,
    pub server_content: Digest,
    pub adapter_build: Digest,
    pub protocol: u32,
    pub token_span_roundtrip: bool,
    pub identity_expression_roundtrip: bool,
}
impl StableEncode for PreparedMacroServer {
    const DOMAIN: &'static str = "lrr/v2/prepared-macro-server/stdio-v1";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(self.schema);
        e.string(&self.repair);
        e.string(&self.source_root);
        e.string(&self.source_commit);
        e.string(&self.compiler_commit);
        e.record(&self.baseline);
        e.string(&self.compiler_dev_manifest);
        e.digest(self.compiler_dev_content);
        e.string(&self.preparation_receipt);
        e.digest(self.preparation_receipt_content);
        e.map(&self.sources, |e, p, d| {
            e.string(p);
            e.digest(*d)
        });
        e.string(&self.compatibility_artifact);
        e.digest(self.compatibility_content);
        e.digest(self.server_content);
        e.digest(self.adapter_build);
        e.u32(self.protocol);
        e.bool(self.token_span_roundtrip);
        e.bool(self.identity_expression_roundtrip);
    }
}
fn adapter_build() -> Digest {
    let mut e = StableEncoder::new("lrr/v2/prepared-server-adapter-build");
    e.string(env!("CARGO_PKG_VERSION"));
    e.string(lrr_expand_adapter::UPSTREAM_COMMIT);
    e.string(std::env::consts::ARCH);
    e.string(std::env::consts::OS);
    e.digest(Digest::of_bytes(include_bytes!("../../../../Cargo.lock")));
    for source in [
        include_bytes!("../../../lrr-expand-adapter/Cargo.toml").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/lib.rs").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/proc_client.rs").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/proc_client/session.rs").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/proc_client/transport.rs").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/proc_client/capabilities.rs").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/proc_client/command.rs").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/proc_client/process_guard.rs").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/proc_client/interruptible.rs").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/proc_client/span_sources.rs").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/span_store.rs").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/span_store/proc_callbacks.rs").as_slice(),
        include_bytes!("../../../lrr-expand-adapter/src/token_wire.rs").as_slice(),
    ] {
        e.digest(Digest::of_bytes(source));
    }
    e.finish()
}
fn expected_sources(root: &Path) -> BTreeMap<String, Digest> {
    // Only hashes are used; these tool sources are not compiled into the runtime's dependency closure.
    let sources: &[(&str, &[u8])] = &[
        ("Cargo.lock", include_bytes!("../../../../Cargo.lock")),
        (
            "tools/prepare_macro_server.py",
            include_bytes!("../../../../tools/prepare_macro_server.py"),
        ),
        (
            "tools/bootstrap_vendor.py",
            include_bytes!("../../../../tools/bootstrap_vendor.py"),
        ),
        (
            "tools/prepared_macro_server/Cargo.toml",
            include_bytes!("../../../../tools/prepared_macro_server/Cargo.toml"),
        ),
        (
            "tools/prepared_macro_server/Cargo.lock",
            include_bytes!("../../../../tools/prepared_macro_server/Cargo.lock"),
        ),
        (
            "tools/prepared_macro_server/build.rs",
            include_bytes!("../../../../tools/prepared_macro_server/build.rs"),
        ),
        (
            "tools/prepared_macro_server/src/main.rs",
            include_bytes!("../../../../tools/prepared_macro_server/src/main.rs"),
        ),
        (
            "tools/prepared_macro_server/stdio-v1.patch",
            include_bytes!("../../../../tools/prepared_macro_server/stdio-v1.patch"),
        ),
        (
            "vendor/UPSTREAM.toml",
            include_bytes!("../../../../vendor/UPSTREAM.toml"),
        ),
        (
            "vendor/rust-analyzer-macros.files.json",
            include_bytes!("../../../../vendor/rust-analyzer-macros.files.json"),
        ),
        (
            "vendor/proc-macro-rust99-injected-env-rust_analyzer_span.patch",
            include_bytes!(
                "../../../../vendor/proc-macro-rust99-injected-env-rust_analyzer_span.patch"
            ),
        ),
        (
            "vendor/proc-macro-rust99-injected-env-token_id.patch",
            include_bytes!("../../../../vendor/proc-macro-rust99-injected-env-token_id.patch"),
        ),
        (
            "vendor/rust-analyzer-macros/crates/proc-macro-srv-cli/src/main.rs",
            include_bytes!(
                "../../../../vendor/rust-analyzer-macros/crates/proc-macro-srv-cli/src/main.rs"
            ),
        ),
        (
            "vendor/rust-analyzer-macros/crates/proc-macro-srv/src/server_impl/rust_analyzer_span.rs",
            include_bytes!(
                "../../../../vendor/rust-analyzer-macros/crates/proc-macro-srv/src/server_impl/rust_analyzer_span.rs"
            ),
        ),
        (
            "vendor/rust-analyzer-macros/crates/proc-macro-srv/src/server_impl/token_id.rs",
            include_bytes!(
                "../../../../vendor/rust-analyzer-macros/crates/proc-macro-srv/src/server_impl/token_id.rs"
            ),
        ),
    ];
    sources
        .iter()
        .map(|(p, b)| {
            (
                root.join(p)
                    .to_str()
                    .expect("validated UTF-8 source root")
                    .to_owned(),
                Digest::of_bytes(b),
            )
        })
        .collect()
}
impl PreparedMacroServer {
    pub(crate) fn validate(&self, tools: &ToolchainRecord) -> Result<(), Diagnostic> {
        absolute(&self.source_root)?;
        absolute(&self.baseline.path)?;
        absolute(&self.compiler_dev_manifest)?;
        absolute(&self.preparation_receipt)?;
        absolute(&self.compatibility_artifact)?;
        for path in self.sources.keys() {
            absolute(path)?
        }
        let root = Path::new(&self.source_root);
        let sysroot = Path::new(&tools.sysroot);
        if self.schema != 1
            || self.repair != "stdio-v1"
            || tools.host != "aarch64-apple-darwin"
            || self.source_commit != lrr_expand_adapter::UPSTREAM_COMMIT
            || self.compiler_commit != tools.compiler_commit
            || self.baseline.path
                != sysroot
                    .join("libexec/rust-analyzer-proc-macro-srv")
                    .to_string_lossy()
            || self.baseline.selection != "1.99.0"
            || self.baseline.version_verbose.is_empty()
            || self.compiler_dev_manifest
                != sysroot
                    .join("lib/rustlib/manifest-rustc-dev")
                    .to_string_lossy()
                && self.compiler_dev_manifest
                    != sysroot
                        .join("lib/rustlib/manifest-rustc-dev-aarch64-apple-darwin")
                        .to_string_lossy()
            || tools.macro_server.path
                != root
                    .join("tools/prepared_macro_server/target/debug/rust-analyzer-proc-macro-srv")
                    .to_string_lossy()
            || self.sources != expected_sources(root)
            || self.server_content != tools.macro_server.content
            || self.adapter_build != adapter_build()
            || self.protocol != tools.macro_protocol
            || !self.token_span_roundtrip
            || !self.identity_expression_roundtrip
        {
            return Err(fault("unregistered prepared macro-server provenance"));
        }
        let banner = format!(
            "rust-analyzer-proc-macro-srv 1.99.0 (rustc {}; source {}; lrr-stdio-v1)",
            self.compiler_commit, self.source_commit
        );
        if tools.macro_server.version_verbose.trim() != banner {
            return Err(fault("prepared server version/provenance mismatch"));
        }
        Ok(())
    }
    pub(crate) fn files(&self) -> impl Iterator<Item = (&String, Digest)> {
        [
            (&self.baseline.path, self.baseline.content),
            (&self.compiler_dev_manifest, self.compiler_dev_content),
            (&self.preparation_receipt, self.preparation_receipt_content),
            (&self.compatibility_artifact, self.compatibility_content),
        ]
        .into_iter()
        .chain(self.sources.iter().map(|(p, d)| (p, *d)))
    }
}
#[derive(Clone)]
pub struct PreparedServerRequest<'a> {
    pub allow_tool_preparation: bool,
    pub allow_macro_execution: bool,
    pub source_root: &'a Path,
    pub server: &'a Path,
    pub compatibility_artifact: &'a Path,
    pub compatibility_content: Digest,
    pub directory: &'a Path,
}
#[derive(Debug)]
pub struct CapturedPreparedServer {
    pub server: ToolBinary,
    pub provenance: PreparedMacroServer,
    pub capability: EchoProbeReport,
}
fn read(path: &Path) -> Result<Vec<u8>, Diagnostic> {
    let mut file = File::open(path).map_err(|e| fault(e.to_string()))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .read_to_end(&mut bytes)
        .map_err(|e| fault(e.to_string()))?;
    Ok(bytes)
}
/// Explicitly build only the registered external repair and capture its provenance.
/// No SDK installation, download or selected live runtime unit compilation occurs.
/// The compatibility probe itself executes a host macro and requires its separate grant.
pub fn capture(
    tools: &ToolchainRecord,
    request: &PreparedServerRequest<'_>,
) -> Result<CapturedPreparedServer, Diagnostic> {
    if !request.allow_tool_preparation {
        return Err(fault("explicit tool-preparation authority is required"));
    }
    if !request.allow_macro_execution {
        return Err(Diagnostic::new(
            "LRR-E-MACRO-PERMISSION",
            "server compatibility execution requires separate macro authority".into(),
        ));
    }
    for path in [
        request.source_root,
        request.server,
        request.compatibility_artifact,
        request.directory,
    ] {
        absolute(
            path.to_str()
                .ok_or_else(|| fault("preparation path is not UTF-8"))?,
        )?;
    }
    if tools.prepared_macro_server.is_some()
        || tools.host != "aarch64-apple-darwin"
        || request.server
            != request
                .source_root
                .join("tools/prepared_macro_server/target/debug/rust-analyzer-proc-macro-srv")
    {
        return Err(fault(
            "capture requires the original matched tuple and registered repair path",
        ));
    }
    let current = super::toolchain::capture("1.99.0")?;
    if tools.compiler.path != current.compiler.to_string_lossy()
        || tools.compiler.content != current.compiler_content
        || tools.compiler.version_verbose != current.compiler_version
        || tools.compiler_commit != current.compiler_commit
        || tools.sysroot != current.sysroot.to_string_lossy()
        || tools.macro_server.path != current.server.to_string_lossy()
        || tools.component_manifest != current.server_component_manifest.to_string_lossy()
        || tools.component_manifest_content != current.component_manifest_content
        || tools.macro_protocol != 6
    {
        return Err(fault(
            "original compiler/server component tuple changed before preparation",
        ));
    }
    let sources = expected_sources(request.source_root);
    let mut bytes = BTreeMap::new();
    let mut retained = 0usize;
    for (path, expected) in &sources {
        let captured = read(Path::new(path))?;
        if Digest::of_bytes(&captured) != *expected {
            return Err(fault(
                "prepared server source differs from registered repair",
            ));
        }
        retained = retained
            .checked_add(captured.len())
            .ok_or_else(|| fault("prepared server source retention exceeds 128MiB"))?;
        bytes.insert(path.clone(), captured);
    }
    let dev = Path::new(&tools.sysroot).join("lib/rustlib/manifest-rustc-dev");
    let dev = if dev.is_file() {
        dev
    } else {
        Path::new(&tools.sysroot).join("lib/rustlib/manifest-rustc-dev-aarch64-apple-darwin")
    };
    let dev_bytes = read(&dev)?;
    let baseline_bytes = read(Path::new(&tools.macro_server.path))?;
    if Digest::of_bytes(&baseline_bytes) != tools.macro_server.content {
        return Err(fault("baseline matched server changed"));
    }
    let compiler_bytes = read(Path::new(&tools.compiler.path))?;
    let component_bytes = read(Path::new(&tools.component_manifest))?;
    if Digest::of_bytes(&compiler_bytes) != tools.compiler.content
        || Digest::of_bytes(&component_bytes) != tools.component_manifest_content
    {
        return Err(fault(
            "compiler/component contents changed before server preparation",
        ));
    }
    let built = std::process::Command::new("python3")
        .arg(request.source_root.join("tools/prepare_macro_server.py"))
        .arg("--allow-tool-preparation")
        .arg("--receipt-directory")
        .arg(request.directory)
        .output()
        .map_err(|e| fault(e.to_string()))?;
    if !built.status.success() {
        return Err(fault(format!(
            "registered external server preparation failed: {}",
            String::from_utf8_lossy(&built.stderr[..built.stderr.len().min(4096)])
        )));
    }
    let preparation_receipt = request.directory.join("server-preparation.json");
    let receipt_bytes = read(&preparation_receipt)?;
    let server_bytes = read(request.server)?;
    let artifact_bytes = read(request.compatibility_artifact)?;
    if Digest::of_bytes(&artifact_bytes) != request.compatibility_content {
        return Err(fault("compatibility artifact changed"));
    }
    let capability = echo_probe(&EchoProbeRequest {
        allow_macro_execution: true,
        server: request.server.to_owned(),
        expected_server_raw_blake3: blake3::hash(&server_bytes).to_hex().to_string(),
        compiler_commit: tools.compiler_commit.clone(),
        artifact: request.compatibility_artifact.to_owned(),
        expected_artifact_raw_blake3: blake3::hash(&artifact_bytes).to_hex().to_string(),
        expected_protocol: tools.macro_protocol,
        directory: request.directory.to_owned(),
    })
    .map_err(|e| Diagnostic::new(e.code, e.message))?;
    for (path, before) in bytes.into_iter().chain([
        (
            request.server.to_str().unwrap().into(),
            server_bytes.clone(),
        ),
        (
            request.compatibility_artifact.to_str().unwrap().into(),
            artifact_bytes,
        ),
        (dev.to_str().unwrap().into(), dev_bytes.clone()),
        (tools.macro_server.path.clone(), baseline_bytes),
        (tools.compiler.path.clone(), compiler_bytes),
        (tools.component_manifest.clone(), component_bytes),
        (
            preparation_receipt.to_str().unwrap().into(),
            receipt_bytes.clone(),
        ),
    ]) {
        if read(Path::new(&path))? != before {
            return Err(fault(
                "prepared server inputs changed during compatibility probe",
            ));
        }
    }
    let server = ToolBinary {
        path: request.server.to_str().unwrap().into(),
        content: Digest::of_bytes(&server_bytes),
        selection: "1.99.0".into(),
        version_verbose: capability.server_version.clone(),
    };
    let provenance = PreparedMacroServer {
        schema: 1,
        repair: "stdio-v1".into(),
        source_root: request.source_root.to_str().unwrap().into(),
        source_commit: lrr_expand_adapter::UPSTREAM_COMMIT.into(),
        compiler_commit: tools.compiler_commit.clone(),
        baseline: tools.macro_server.clone(),
        compiler_dev_manifest: dev.to_str().unwrap().into(),
        compiler_dev_content: Digest::of_bytes(&dev_bytes),
        preparation_receipt: preparation_receipt.to_str().unwrap().into(),
        preparation_receipt_content: Digest::of_bytes(&receipt_bytes),
        sources,
        compatibility_artifact: request.compatibility_artifact.to_str().unwrap().into(),
        compatibility_content: request.compatibility_content,
        server_content: server.content,
        adapter_build: adapter_build(),
        protocol: capability.protocol,
        token_span_roundtrip: capability.token_and_span_roundtrip,
        identity_expression_roundtrip: capability.identity_expression_roundtrip,
    };
    let mut selected = tools.clone();
    selected.macro_server = server.clone();
    selected.prepared_macro_server = Some(provenance.clone());
    provenance.validate(&selected)?;
    Ok(CapturedPreparedServer {
        server,
        provenance,
        capability,
    })
}
