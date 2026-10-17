//! Pinned external protocol probe. Libraries never load in this process.
mod capabilities;
mod command;
mod interruptible;
mod process_guard;
mod session;
mod span_sources;
mod transport;
use crate::AdapterError;
pub use capabilities::ProcSpanOperation;
use paths::AbsPathBuf;
use proc_macro_api::{
    MacroDylib, ProcMacroClient, ProcMacroKind,
    legacy_protocol::msg::{Message, Request, Response},
    version,
};
use serde::Serialize;
pub use session::{
    DeriveHelperMetadata, MacroArtifact, ProcCancellation, ProcDiagnosticSnapshot, ProcExportRef,
    ProcForm, ProcGrammar, ProcInvocation, ProcSession, ServerIdentity, SessionConfig,
};
use span::{
    Edition, EditionedFileId, FileId, ROOT_ERASED_FILE_AST_ID, Span, SpanAnchor, SyntaxContext,
    TextRange,
};
pub use span_sources::{CapturedSpanSource, SourceDigestEncoding};
use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct EchoProbeRequest {
    pub allow_macro_execution: bool,
    pub server: PathBuf,
    pub expected_server_raw_blake3: String,
    pub compiler_commit: String,
    pub artifact: PathBuf,
    pub expected_artifact_raw_blake3: String,
    pub expected_protocol: u32,
    pub directory: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> EchoProbeRequest {
        EchoProbeRequest {
            allow_macro_execution: true,
            server: PathBuf::from("relative-missing-server"),
            expected_server_raw_blake3: "0".repeat(64),
            compiler_commit: "0".repeat(40),
            artifact: PathBuf::from("relative-missing-artifact"),
            expected_artifact_raw_blake3: "0".repeat(64),
            expected_protocol: version::CURRENT_API_VERSION,
            directory: PathBuf::from("."),
        }
    }

    #[test]
    fn attributes_preserve_declaration_slots_without_widening_derive_or_expression_forms() {
        for grammar in [
            ProcGrammar::Items,
            ProcGrammar::ImplItems,
            ProcGrammar::TraitItems,
            ProcGrammar::Statements,
        ] {
            assert!(ProcForm::Attribute.accepts_grammar(grammar));
        }
        for grammar in [
            ProcGrammar::Expression,
            ProcGrammar::Pattern,
            ProcGrammar::Type,
        ] {
            assert!(!ProcForm::Attribute.accepts_grammar(grammar));
            assert!(!ProcForm::Derive.accepts_grammar(grammar));
            assert!(ProcForm::FunctionLike.accepts_grammar(grammar));
        }
        for grammar in [
            ProcGrammar::ImplItems,
            ProcGrammar::TraitItems,
            ProcGrammar::Statements,
        ] {
            assert!(!ProcForm::Derive.accepts_grammar(grammar));
        }
        assert!(ProcForm::Derive.accepts_grammar(ProcGrammar::Items));
    }
    #[test]
    fn missing_macro_authority_precedes_filesystem_and_process_access() {
        let mut request = request();
        request.allow_macro_execution = false;
        let error = echo_probe(&request).unwrap_err();
        assert_eq!(error.code, "LRR-E-MACRO-PERMISSION");
        assert!(error.message.contains("separate authority"));
    }

    #[test]
    fn changed_server_content_is_rejected_before_execution() {
        let mut request = request();
        request.server = std::env::current_exe().unwrap();
        let actual = blake3::hash(&fs::read(&request.server).unwrap())
            .to_hex()
            .to_string();
        assert_ne!(actual, request.expected_server_raw_blake3);
        let error = echo_probe(&request).unwrap_err();
        assert_eq!(error.code, "LRR-E-MACRO-SERVER-MISMATCH");
        assert_eq!(error.message, "recorded tool/artifact hash mismatch");
    }
}
#[derive(Debug, Serialize)]
pub struct EchoProbeReport {
    pub server_version: String,
    pub protocol: u32,
    pub macro_names: Vec<String>,
    pub macro_kind: String,
    pub token_and_span_roundtrip: bool,
    pub identity_expression_roundtrip: bool,
    pub raw_version_request: String,
    pub raw_version_response: String,
    pub version_stderr: String,
    pub handshake_stderr: String,
    pub server_raw_blake3: String,
    pub artifact_raw_blake3: String,
    pub expansion_protocol_stdout: String,
    pub expansion_protocol_stdout_truncated: bool,
    pub helper_pids: Vec<u32>,
}
#[derive(Default)]
struct Capture {
    bytes: Vec<u8>,
    truncated: bool,
}
struct CapturedReader<R> {
    reader: transport::BoundedReader<R>,
    capture: Arc<Mutex<Capture>>,
}
impl<R: std::io::BufRead> std::io::Read for CapturedReader<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let count = std::io::Read::read(&mut self.reader, out)?;
        self.record();
        Ok(count)
    }
}
impl<R: std::io::BufRead> CapturedReader<R> {
    fn record(&self) {
        if let Ok(mut capture) = self.capture.lock() {
            let (bytes, truncated) = self.reader.preview();
            let retained = capture.bytes.len();
            capture.bytes.extend_from_slice(&bytes[retained..]);
            capture.truncated = truncated;
        }
    }
}
impl<R: std::io::BufRead> std::io::BufRead for CapturedReader<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        std::io::BufRead::fill_buf(&mut self.reader)
    }
    fn consume(&mut self, amount: usize) {
        std::io::BufRead::consume(&mut self.reader, amount);
        self.record();
    }
}
struct OwnedProcess {
    guard: process_guard::ProcessGuard,
}
impl proc_macro_api::process::ProcessExit for OwnedProcess {
    fn exit_err(&mut self) -> Option<proc_macro_api::ServerError> {
        match self.guard.status() {
            Ok(Some((reason, status))) => Some(proc_macro_api::ServerError {
                message: format!("macro helper terminated ({reason:?}): {status}"),
                io: None,
            }),
            Ok(None) => None,
            Err(error) => Some(proc_macro_api::ServerError {
                message: error.to_string(),
                io: Some(Arc::new(error)),
            }),
        }
    }
}

fn mismatch(message: impl Into<String>) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-SERVER-MISMATCH",
        message: message.into(),
    }
}
fn verify(path: &PathBuf, expected: &str) -> Result<(), AdapterError> {
    if !path.is_absolute()
        || expected.len() != 64
        || !expected
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(mismatch("invalid tool/artifact identity"));
    }
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|e| mismatch(e.to_string()))?;
    let mut hash = blake3::Hasher::new();
    let mut bytes = [0u8; 8192];
    loop {
        let count = file.read(&mut bytes).map_err(|e| mismatch(e.to_string()))?;
        if count == 0 {
            break;
        }
        hash.update(&bytes[..count]);
    }
    if hash.finalize().to_hex().as_str() != expected {
        return Err(mismatch("recorded tool/artifact hash mismatch"));
    }
    Ok(())
}
struct Handshake {
    server_version: String,
    protocol: u32,
    raw: Vec<u8>,
    response: command::Output,
    version_stderr: Vec<u8>,
}
fn handshake(
    server_path: &PathBuf,
    compiler_commit: &str,
    expected_protocol: u32,
) -> Result<Handshake, AdapterError> {
    let deadline = Instant::now()
        .checked_add(crate::NO_TIME_LIMIT)
        .ok_or_else(|| mismatch("startup deadline overflow"))?;
    handshake_with_deadline(server_path, compiler_commit, expected_protocol, deadline)
}
fn startup_remaining(deadline: Instant) -> Result<Duration, AdapterError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(AdapterError {
            code: "LRR-E-MACRO-TIMEOUT",
            message: "macro startup deadline expired".into(),
        });
    }
    Ok(remaining)
}
fn startup_io(error: std::io::Error) -> AdapterError {
    if error.kind() == std::io::ErrorKind::TimedOut {
        AdapterError {
            code: "LRR-E-MACRO-TIMEOUT",
            message: error.to_string(),
        }
    } else {
        mismatch(error.to_string())
    }
}
fn handshake_with_deadline(
    server_path: &PathBuf,
    compiler_commit: &str,
    expected_protocol: u32,
    deadline: Instant,
) -> Result<Handshake, AdapterError> {
    if compiler_commit.len() != 40
        || !compiler_commit
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(mismatch("invalid recorded compiler commit"));
    }
    let commit = compiler_commit
        .get(..8)
        .filter(|s| s.len() == 8 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| mismatch("compiler commit unavailable"))?;
    let version_output = command::run(
        Command::new(server_path)
            .env_clear()
            .env("RUST_ANALYZER_INTERNALS_DO_NOT_USE", "this is unstable")
            .arg("--version"),
        &[],
        usize::MAX,
        startup_remaining(deadline)?,
    )
    .map_err(startup_io)?;
    let server_version = String::from_utf8(version_output.stdout)
        .map_err(|_| mismatch("server version is not UTF-8"))?;
    if !version_output.status.success()
        || !server_version.starts_with("rust-analyzer-proc-macro-srv 1.99.0 ")
        || !server_version.contains(commit)
    {
        return Err(mismatch(
            "server/compiler tuple mismatch before macro loading",
        ));
    }
    let mut raw = Vec::new();
    Request::ApiVersionCheck {}
        .write(&mut raw)
        .map_err(|e| mismatch(e.to_string()))?;
    let response = command::run(
        Command::new(server_path)
            .env_clear()
            .env("RUST_ANALYZER_INTERNALS_DO_NOT_USE", "this is unstable"),
        &raw,
        usize::MAX,
        startup_remaining(deadline)?,
    )
    .map_err(startup_io)?;
    if !response.status.success() {
        return Err(mismatch("macro protocol handshake failed"));
    }
    let decoded = Response::read(&mut Cursor::new(&response.stdout), &mut String::new())
        .map_err(|e| mismatch(e.to_string()))?;
    let Some(Response::ApiVersionCheck(protocol)) = decoded else {
        return Err(mismatch("unexpected protocol response"));
    };
    if protocol != expected_protocol || protocol != version::CURRENT_API_VERSION {
        return Err(mismatch(format!("unsupported server protocol {protocol}")));
    }
    Ok(Handshake {
        server_version,
        protocol,
        raw,
        response,
        version_stderr: version_output.stderr,
    })
}

pub fn echo_probe(request: &EchoProbeRequest) -> Result<EchoProbeReport, AdapterError> {
    if !request.allow_macro_execution {
        return Err(AdapterError {
            code: "LRR-E-MACRO-PERMISSION",
            message: "native macro execution requires separate authority".into(),
        });
    }
    verify(&request.server, &request.expected_server_raw_blake3)?;
    verify(&request.artifact, &request.expected_artifact_raw_blake3)?;
    let Handshake {
        server_version,
        protocol,
        raw,
        response,
        version_stderr,
    } = handshake(
        &request.server,
        &request.compiler_commit,
        request.expected_protocol,
    )?;
    let server = AbsPathBuf::try_from(
        request
            .server
            .to_str()
            .ok_or_else(|| mismatch("server path is not UTF-8"))?,
    )
    .map_err(|_| mismatch("server path is not absolute"))?;
    let artifact = AbsPathBuf::try_from(
        request
            .artifact
            .to_str()
            .ok_or_else(|| mismatch("artifact path is not UTF-8"))?,
    )
    .map_err(|_| mismatch("artifact path is not absolute"))?;
    let capture = Arc::new(Mutex::new(Capture::default()));
    let pids = Arc::new(Mutex::new(Vec::new()));
    let captured = capture.clone();
    let spawned = pids.clone();
    let process_path = request.server.clone();
    // Retain the pinned client/codec and supply owned, captured child channels.
    let client = ProcMacroClient::with_io_channels(
        &server,
        move |format| {
            let mut command = Command::new(&process_path);
            command
                .env_clear()
                .env("RUST_ANALYZER_INTERNALS_DO_NOT_USE", "this is unstable")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit());
            if let Some(format) = format {
                command.arg("--format").arg(format.to_string());
            }
            let mut process = OwnedProcess {
                guard: process_guard::ProcessGuard::spawn(
                    &mut command,
                    crate::NO_TIME_LIMIT,
                )?,
            };
            let pid = process.guard.id();
            spawned
                .lock()
                .map_err(|_| std::io::Error::other("PID recorder poisoned"))?
                .push(pid);
            let stdin = interruptible::Interruptible::new(
                process.guard.take_stdin()?,
                process.guard.control(),
            )?;
            let stdout = interruptible::Interruptible::new(
                process.guard.take_stdout()?,
                process.guard.control(),
            )?;
            Ok((
                Box::new(process),
                Box::new(stdin),
                Box::new(CapturedReader {
                    reader: transport::BoundedReader::new(
                        std::io::BufReader::new(stdout),
                        transport::MAX_REPLY_BYTES,
                        transport::MAX_PREVIEW_BYTES,
                    )?,
                    capture: captured.clone(),
                }),
            ))
        },
        None,
        1,
    )
    .map_err(|e| mismatch(e.to_string()))?;
    let macros = client
        .load_dylib(MacroDylib::new(artifact))
        .map_err(|e| mismatch(e.to_string()))?;
    let echo = macros
        .iter()
        .find(|m| m.name() == "echo" && m.kind() == ProcMacroKind::Attr)
        .ok_or_else(|| mismatch("prepared echo macro missing"))?;
    let edition = Edition::Edition2024;
    let span = Span {
        range: TextRange::empty(0.into()),
        anchor: SpanAnchor {
            file_id: EditionedFileId::new(FileId::from_raw(0), edition),
            ast_id: ROOT_ERASED_FILE_AST_ID,
        },
        ctx: SyntaxContext::root(edition),
    };
    let source = "fn r#gen<'a>(value: &'a i64) -> i64 { let text = r#\"λ\"#; -42_i64 + *value }";
    let input = syntax_bridge::parse_to_token_tree(edition, span.anchor, span.ctx, source)
        .ok_or_else(|| mismatch("bootstrap token fixture did not lex"))?;
    let attributes = syntax_bridge::parse_to_token_tree_static_span(edition, span, "")
        .ok_or_else(|| mismatch("empty attribute fixture did not lex"))?;
    let directory = request
        .directory
        .to_str()
        .ok_or_else(|| mismatch("fixture directory is not UTF-8"))?
        .to_owned();
    let expanded = echo
        .expand(
            input.view(),
            Some(attributes.view()),
            vec![],
            span,
            span,
            span,
            directory,
            None,
        )
        .map_err(|e| AdapterError {
            code: "LRR-E-MACRO-CRASH",
            message: e.to_string(),
        })?
        .map_err(|e| AdapterError {
            code: "LRR-E-MACRO-PANIC",
            message: e,
        })?;
    let roundtrip = expanded == input;
    if !roundtrip {
        return Err(AdapterError {
            code: "LRR-E-MACRO-OUTPUT",
            message: "echo token/span forest did not roundtrip".into(),
        });
    }
    let identity = macros
        .iter()
        .find(|m| m.name() == "identity" && m.kind() == ProcMacroKind::Bang)
        .ok_or_else(|| mismatch("prepared identity macro missing"))?;
    let expression = syntax_bridge::parse_to_token_tree(edition, span.anchor, span.ctx, "3_i64")
        .ok_or_else(|| mismatch("identity expression fixture did not lex"))?;
    let expression_result = identity
        .expand(
            expression.view(),
            None,
            vec![],
            span,
            span,
            span,
            request
                .directory
                .to_str()
                .ok_or_else(|| mismatch("fixture directory encoding"))?
                .into(),
            None,
        )
        .map_err(|e| AdapterError {
            code: "LRR-E-MACRO-CRASH",
            message: e.to_string(),
        })?
        .map_err(|e| AdapterError {
            code: "LRR-E-MACRO-PANIC",
            message: e,
        })?;
    if expression_result != expression {
        return Err(AdapterError {
            code: "LRR-E-MACRO-OUTPUT",
            message: "identity!(3_i64) tokens/spans changed".into(),
        });
    }
    verify(&request.server, &request.expected_server_raw_blake3)?;
    verify(&request.artifact, &request.expected_artifact_raw_blake3)?;
    let capture = capture
        .lock()
        .map_err(|_| mismatch("protocol recorder poisoned"))?;
    Ok(EchoProbeReport {
        server_version: server_version.trim().into(),
        protocol,
        macro_names: macros.iter().map(|m| m.name().into()).collect(),
        macro_kind: "attribute".into(),
        token_and_span_roundtrip: roundtrip,
        identity_expression_roundtrip: true,
        raw_version_request: String::from_utf8(raw).map_err(|_| mismatch("request encoding"))?,
        raw_version_response: String::from_utf8(response.stdout)
            .map_err(|_| mismatch("response encoding"))?,
        version_stderr: String::from_utf8_lossy(&version_stderr).into_owned(),
        handshake_stderr: String::from_utf8_lossy(&response.stderr).into_owned(),
        server_raw_blake3: request.expected_server_raw_blake3.clone(),
        artifact_raw_blake3: request.expected_artifact_raw_blake3.clone(),
        expansion_protocol_stdout: String::from_utf8(capture.bytes.clone())
            .map_err(|_| mismatch("captured protocol output is not UTF-8"))?,
        expansion_protocol_stdout_truncated: capture.truncated,
        helper_pids: pids
            .lock()
            .map_err(|_| mismatch("PID recorder poisoned"))?
            .clone(),
    })
}
