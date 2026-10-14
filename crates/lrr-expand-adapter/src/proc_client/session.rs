//! Serial external execution for one immutable consuming unit and expansion epoch.
use super::{
    Capture, CapturedReader, CapturedSpanSource, handshake_with_deadline, mismatch,
    process_guard::{ProcessControl, ProcessGuard, StopReason},
    startup_remaining,
    transport::{BoundedReader, BoundedWriter, MAX_PREVIEW_BYTES, MAX_REPLY_BYTES, ReplyAllowance},
    verify,
};
use crate::{
    AdapterError,
    span_store::SpanStore,
    token_wire::{Key, TokenForest},
};
use paths::AbsPathBuf;
use proc_macro_api::{MacroDylib, ProcMacro, ProcMacroClient, ProcMacroKind, ProtocolFormat};
use std::{
    collections::BTreeMap,
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcForm {
    FunctionLike,
    Attribute,
    Derive,
}
impl ProcForm {
    /// Output is validated in the owned invocation slot, independently of host
    /// execution form. Attributes replace declarations; derives append items.
    pub fn accepts_grammar(self, grammar: ProcGrammar) -> bool {
        match self {
            Self::FunctionLike => true,
            Self::Attribute => matches!(
                grammar,
                ProcGrammar::Items
                    | ProcGrammar::ImplItems
                    | ProcGrammar::TraitItems
                    | ProcGrammar::Statements
            ),
            Self::Derive => grammar == ProcGrammar::Items,
        }
    }
    fn tag(self) -> u8 {
        match self {
            Self::FunctionLike => 0,
            Self::Attribute => 1,
            Self::Derive => 2,
        }
    }
    fn pinned(self) -> ProcMacroKind {
        match self {
            Self::FunctionLike => ProcMacroKind::Bang,
            Self::Attribute => ProcMacroKind::Attr,
            Self::Derive => ProcMacroKind::CustomDerive,
        }
    }
}
pub use crate::grammar::Grammar as ProcGrammar;

#[derive(Clone)]
pub struct ServerIdentity {
    pub path: PathBuf,
    pub raw_blake3: String,
    pub compiler_commit: String,
    pub protocol: u32,
}
#[derive(Clone)]
pub struct MacroArtifact {
    pub identity: Key,
    pub path: PathBuf,
    pub raw_blake3: String,
}
#[derive(Clone)]
pub struct SessionConfig {
    pub allow_macro_execution: bool,
    pub required_span_operations: std::collections::BTreeSet<super::ProcSpanOperation>,
    pub unit: Key,
    pub epoch: u64,
    pub server: ServerIdentity,
    pub artifacts: Vec<MacroArtifact>,
    pub environment: BTreeMap<String, String>,
    pub directory: PathBuf,
    pub timeout: Duration,
    pub wire_limit: usize,
    pub output_limit: usize,
}

/// Protocol metadata availability is distinct from a derive with no helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeriveHelperMetadata {
    NotDerive,
    Unavailable { protocol: u32 },
}
/// Inventory of exact artifacts loaded by the already authorized matched helper.
#[derive(Debug, Clone, Copy)]
pub struct ProcExportRef<'a> {
    pub artifact: Key,
    pub name: &'a str,
    pub form: ProcForm,
    pub helpers: DeriveHelperMetadata,
}
pub struct ProcInvocation {
    pub identity: Key,
    pub unit: Key,
    pub epoch: u64,
    pub artifact: Key,
    pub export: String,
    pub form: ProcForm,
    pub grammar: ProcGrammar,
    pub scope: Key,
    pub parent_origin: Key,
    pub input: TokenForest,
    pub attributes: Option<TokenForest>,
    /// The input forest includes the complete span/context records for all three sites.
    pub definition_site: Key,
    pub call_site: Key,
    pub mixed_site: Key,
    pub sources: Vec<CapturedSpanSource>,
}
impl ProcInvocation {
    fn fingerprint(&self) -> Key {
        let mut hash = blake3::Hasher::new();
        hash.update(b"lrr-proc-invocation-v1\0");
        for key in [
            self.identity,
            self.unit,
            self.artifact,
            self.scope,
            self.parent_origin,
            self.input.fingerprint(),
            self.definition_site,
            self.call_site,
            self.mixed_site,
        ] {
            hash.update(&key.0);
        }
        hash.update(&self.epoch.to_le_bytes());
        hash.update(&[self.form.tag(), self.grammar.tag()]);
        hash.update(&(self.export.len() as u64).to_le_bytes());
        hash.update(self.export.as_bytes());
        match &self.attributes {
            Some(tokens) => {
                hash.update(&[1]);
                hash.update(&tokens.fingerprint().0);
            }
            None => {
                hash.update(&[0]);
            }
        }
        hash.update(&(self.sources.len() as u64).to_le_bytes());
        for source in &self.sources {
            hash.update(&source.source().unit.0);
            hash.update(&source.source().content.0);
            hash.update(&(source.source().path.len() as u64).to_le_bytes());
            hash.update(source.source().path.as_bytes());
            hash.update(&[match source.encoding() {
                super::SourceDigestEncoding::RawBlake3 => 0,
                super::SourceDigestEncoding::LrrSourceBytes => 1,
            }]);
            match source.local_path() {
                None => {
                    hash.update(&[0]);
                }
                Some(path) => {
                    hash.update(&[1]);
                    hash.update(&(path.len() as u64).to_le_bytes());
                    hash.update(path.as_bytes());
                }
            }
        }
        Key(*hash.finalize().as_bytes())
    }
}
type Outcome = Result<Arc<TokenForest>, AdapterError>;
struct Attempt {
    fingerprint: Key,
    outcome: Outcome,
}
struct Channels {
    control: ProcessControl,
    quota: ReplyAllowance,
    request_quota: ReplyAllowance,
    pid: u32,
}

/// May be held by a coordinator while a serial expansion is blocked in IO.
#[derive(Clone)]
pub struct ProcCancellation(ProcessControl);
impl ProcCancellation {
    pub fn cancel(&self) -> Result<(), AdapterError> {
        self.0
            .terminate()
            .map(|_| ())
            .map_err(|e| fault("LRR-E-MACRO-CRASH", e.to_string()))
    }
}
struct SessionProcess {
    guard: ProcessGuard,
    stderr: Option<JoinHandle<io::Result<()>>>,
}
impl proc_macro_api::process::ProcessExit for SessionProcess {
    fn exit_err(&mut self) -> Option<proc_macro_api::ServerError> {
        match self.guard.status() {
            Ok(None) => None,
            Ok(Some((reason, status))) => Some(proc_macro_api::ServerError {
                message: format!("macro helper terminated ({reason:?}): {status}"),
                io: None,
            }),
            Err(error) => Some(proc_macro_api::ServerError {
                message: error.to_string(),
                io: Some(Arc::new(error)),
            }),
        }
    }
}
impl Drop for SessionProcess {
    fn drop(&mut self) {
        let _ = self.guard.cancel();
        if let Some(reader) = self.stderr.take() {
            let _ = reader.join();
        }
    }
}

/// Owned bounded channel previews; truncation is not hidden as a complete log.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcDiagnosticSnapshot {
    pub protocol_preview: Vec<u8>,
    pub protocol_truncated: bool,
    pub stderr_preview: Vec<u8>,
    pub stderr_truncated: bool,
}

/// All upstream handles stay private. `&mut self` serializes invocation and cache admission.
pub struct ProcSession {
    config: SessionConfig,
    client: Option<ProcMacroClient>,
    macros: BTreeMap<(Key, String, ProcForm), ProcMacro>,
    attempts: BTreeMap<Key, Attempt>,
    channels: Arc<Mutex<Option<Channels>>>,
    stdout: Arc<Mutex<Capture>>,
    stderr: Arc<Mutex<Capture>>,
    stderr_limit_hit: Arc<AtomicBool>,
    closed: Option<AdapterError>,
    invocations: u64,
    retained_bytes: usize,
    execution_deadline: Option<Instant>,
}
fn fault(code: &'static str, message: impl Into<String>) -> AdapterError {
    let mut message = message.into();
    if message.len() > MAX_PREVIEW_BYTES {
        let mut end = MAX_PREVIEW_BYTES;
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        message = message[..end].to_owned();
    }
    AdapterError { code, message }
}
fn startup_failure(
    channels: &Mutex<Option<Channels>>,
    deadline: Instant,
    message: String,
) -> AdapterError {
    let timed_out = channels
        .lock()
        .ok()
        .and_then(|channels| {
            channels
                .as_ref()
                .and_then(|channels| channels.control.status().ok())
                .flatten()
        })
        .is_some_and(|(reason, _)| reason == StopReason::Deadline);
    if timed_out || Instant::now() >= deadline {
        fault("LRR-E-MACRO-TIMEOUT", message)
    } else {
        mismatch(message)
    }
}
fn absolute(path: &Path) -> Result<AbsPathBuf, AdapterError> {
    AbsPathBuf::try_from(
        path.to_str()
            .ok_or_else(|| mismatch("tool path is not UTF-8"))?,
    )
    .map_err(|_| mismatch("tool path must be absolute"))
}
impl ProcSession {
    pub fn new(config: SessionConfig) -> Result<Self, AdapterError> {
        Self::new_bounded(config, None)
    }
    pub fn new_with_deadline(
        config: SessionConfig,
        deadline: Instant,
    ) -> Result<Self, AdapterError> {
        Self::new_bounded(config, Some(deadline))
    }
    fn new_bounded(config: SessionConfig, deadline: Option<Instant>) -> Result<Self, AdapterError> {
        // The authority guard precedes even validation/filesystem reads.
        if !config.allow_macro_execution {
            return Err(fault(
                "LRR-E-MACRO-PERMISSION",
                "native macro execution requires separate authority",
            ));
        }
        if let Some(operation) = config
            .required_span_operations
            .iter()
            .find(|op| !op.admitted())
        {
            return Err(mismatch(format!(
                "required span operation {operation:?} is unavailable in the matched Rust1.99/API6 contract"
            )));
        }
        if config.artifacts.is_empty()
            || config.artifacts.len() > 1024
            || !config.directory.is_absolute()
            || config.directory.to_str().is_none()
            || config.timeout.is_zero()
            || config.timeout > Duration::from_secs(60)
            || config.wire_limit == 0
            || config.wire_limit > MAX_REPLY_BYTES
            || config.output_limit == 0
            || config.output_limit > 32 * 1024 * 1024
            || config.environment.len() > 4096
        {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "invalid or excessive session configuration",
            ));
        }
        let env_bytes = config
            .environment
            .iter()
            .try_fold(0usize, |sum, (key, value)| {
                if key.is_empty()
                    || key.contains(['\0', '='])
                    || value.contains('\0')
                    || key == "RUST_ANALYZER_INTERNALS_DO_NOT_USE"
                {
                    return None;
                }
                sum.checked_add(key.len())?.checked_add(value.len())
            })
            .filter(|bytes| *bytes <= 1024 * 1024);
        if env_bytes.is_none() {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "invalid or excessive compile environment",
            ));
        }
        let startup_deadline = Instant::now()
            .checked_add(config.timeout)
            .ok_or_else(|| mismatch("startup deadline overflow"))?;
        let startup_deadline = deadline.map_or(startup_deadline, |cap| cap.min(startup_deadline));
        startup_remaining(startup_deadline)?;
        let mut unique = BTreeMap::new();
        for artifact in &config.artifacts {
            if unique.insert(artifact.identity, ()).is_some() {
                return Err(mismatch("duplicate artifact identity"));
            }
            absolute(&artifact.path)?;
            verify(&artifact.path, &artifact.raw_blake3)?;
        }
        verify(&config.server.path, &config.server.raw_blake3)?;
        handshake_with_deadline(
            &config.server.path,
            &config.server.compiler_commit,
            config.server.protocol,
            startup_deadline,
        )?;
        let path = absolute(&config.server.path)?;
        let channels = Arc::new(Mutex::new(None));
        let stdout = Arc::new(Mutex::new(Capture::default()));
        let stderr = Arc::new(Mutex::new(Capture::default()));
        let stderr_limit_hit = Arc::new(AtomicBool::new(false));
        let spawn_channels = channels.clone();
        let spawn_stdout = stdout.clone();
        let spawn_stderr = stderr.clone();
        let spawn_stderr_limit = stderr_limit_hit.clone();
        let server = config.server.path.clone();
        let environment = config.environment.clone();
        let directory = config.directory.clone();
        startup_remaining(startup_deadline)?;
        let wire_limit = config.wire_limit;
        let client = ProcMacroClient::with_io_channels_protocol(
            &path,
            move |format| {
                if format != Some(ProtocolFormat::BidirectionalPostcardPrototype) {
                    return Err(io::Error::other("unapproved protocol format"));
                }
                let mut command = Command::new(&server);
                command
                    .env_clear()
                    .envs(&environment)
                    .env("RUST_ANALYZER_INTERNALS_DO_NOT_USE", "this is unstable")
                    .current_dir(&directory)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                command
                    .arg("--format")
                    .arg(ProtocolFormat::BidirectionalPostcardPrototype.to_string());
                let remaining = startup_remaining(startup_deadline)
                    .map_err(|error| io::Error::new(io::ErrorKind::TimedOut, error.message))?;
                let mut guard = ProcessGuard::spawn(&mut command, remaining)?;
                let pid = guard.id();
                let stdin =
                    super::interruptible::Interruptible::new(guard.take_stdin()?, guard.control())?;
                let stdin = BoundedWriter::new(stdin, wire_limit)?;
                let request_quota = stdin.allowance();
                let output = BoundedReader::new(
                    BufReader::new(super::interruptible::Interruptible::new(
                        guard.take_stdout()?,
                        guard.control(),
                    )?),
                    wire_limit,
                    MAX_PREVIEW_BYTES,
                )?;
                let quota = output.allowance();
                let control = guard.control();
                let stderr_pipe = super::interruptible::Interruptible::new(
                    guard.take_stderr()?,
                    guard.control(),
                )?;
                let drain_control = control.clone();
                let capture = spawn_stderr.clone();
                let limit_hit = spawn_stderr_limit.clone();
                let reader = thread::Builder::new()
                    .name("lrr-macro-stderr".into())
                    .spawn(move || {
                        let mut input = stderr_pipe;
                        let mut total = 0usize;
                        let mut bytes = [0u8; 4096];
                        loop {
                            let count = input.read(&mut bytes)?;
                            if count == 0 {
                                return Ok(());
                            }
                            total = total
                                .checked_add(count)
                                .ok_or_else(|| io::Error::other("macro stderr length overflow"))?;
                            let mut capture = capture
                                .lock()
                                .map_err(|_| io::Error::other("stderr recorder poisoned"))?;
                            let keep = (MAX_PREVIEW_BYTES - capture.bytes.len()).min(count);
                            capture.bytes.extend_from_slice(&bytes[..keep]);
                            capture.truncated |= keep < count;
                            if total > 4 * 1024 * 1024 {
                                limit_hit.store(true, Ordering::Release);
                                drop(capture);
                                let _ = drain_control.terminate();
                                return Err(io::Error::other("macro stderr allowance exhausted"));
                            }
                        }
                    })?;
                *spawn_channels
                    .lock()
                    .map_err(|_| io::Error::other("macro channels poisoned"))? = Some(Channels {
                    control,
                    quota,
                    request_quota,
                    pid,
                });
                Ok((
                    Box::new(SessionProcess {
                        guard,
                        stderr: Some(reader),
                    }),
                    Box::new(stdin),
                    Box::new(CapturedReader {
                        reader: output,
                        capture: spawn_stdout.clone(),
                    }),
                ))
            },
            ProtocolFormat::BidirectionalPostcardPrototype,
            1,
        )
        .map_err(|e| startup_failure(&channels, startup_deadline, e.to_string()))?;
        let mut session = Self {
            config,
            client: Some(client),
            macros: BTreeMap::new(),
            attempts: BTreeMap::new(),
            channels,
            stdout,
            stderr,
            stderr_limit_hit,
            closed: None,
            invocations: 0,
            // Reserve the first BTree node; per-row allowance covers partial nodes.
            retained_bytes: 4096,
            execution_deadline: deadline,
        };
        for artifact in &session.config.artifacts {
            let macros = session
                .client
                .as_ref()
                .ok_or_else(|| mismatch("helper unavailable"))?
                .load_dylib(MacroDylib::new(absolute(&artifact.path)?))
                .map_err(|e| startup_failure(&session.channels, startup_deadline, e.to_string()))?;
            for mac in macros {
                let form = match mac.kind() {
                    ProcMacroKind::Bang => ProcForm::FunctionLike,
                    ProcMacroKind::Attr => ProcForm::Attribute,
                    ProcMacroKind::CustomDerive => ProcForm::Derive,
                };
                if session
                    .macros
                    .insert((artifact.identity, mac.name().into(), form), mac)
                    .is_some()
                {
                    return Err(mismatch("duplicate macro export"));
                }
            }
        }
        for artifact in &session.config.artifacts {
            verify(&artifact.path, &artifact.raw_blake3)?;
        }
        verify(
            &session.config.server.path,
            &session.config.server.raw_blake3,
        )?;
        let control = {
            session
                .channels
                .lock()
                .map_err(|_| mismatch("macro channels poisoned"))?
                .as_ref()
                .ok_or_else(|| mismatch("missing macro helper"))?
                .control
                .clone()
        };
        control
            .disarm()
            .map_err(|e| startup_failure(&session.channels, startup_deadline, e.to_string()))?;
        startup_remaining(startup_deadline)?;
        Ok(session)
    }

    /// Reads retained inventory only; never loads, executes, or probes another library.
    /// API6 has no helper-attribute field, so unavailable never means an empty set.
    pub fn exports(&self) -> impl Iterator<Item = ProcExportRef<'_>> {
        self.macros
            .keys()
            .map(|(artifact, name, form)| ProcExportRef {
                artifact: *artifact,
                name,
                form: *form,
                helpers: if *form == ProcForm::Derive {
                    DeriveHelperMetadata::Unavailable {
                        protocol: self.config.server.protocol,
                    }
                } else {
                    DeriveHelperMetadata::NotDerive
                },
            })
    }
    pub fn invocations(&self) -> u64 {
        self.invocations
    }
    /// Read an already completed invocation without permitting another native call.
    pub fn cached_result(
        &self,
        request: &ProcInvocation,
    ) -> Result<Option<Arc<TokenForest>>, AdapterError> {
        if request.unit != self.config.unit || request.epoch != self.config.epoch {
            return Err(fault(
                "LRR-E-MACRO-CRASH",
                "wrong consuming unit or expansion epoch",
            ));
        }
        if let Some(error) = &self.closed {
            return Err(error.clone());
        }
        if self.helper_terminated()? {
            return Err(fault(
                "LRR-E-MACRO-CRASH",
                "procedural session helper terminated",
            ));
        }
        match self.attempts.get(&request.identity) {
            None => Ok(None),
            Some(attempt) if attempt.fingerprint != request.fingerprint() => Err(fault(
                "LRR-E-MACRO-CRASH",
                "invocation identity reused with different request",
            )),
            Some(attempt) => attempt.outcome.clone().map(Some),
        }
    }
    pub fn cancellation(&self) -> Result<ProcCancellation, AdapterError> {
        self.channels
            .lock()
            .map_err(|_| mismatch("macro channels poisoned"))?
            .as_ref()
            .map(|channels| ProcCancellation(channels.control.clone()))
            .ok_or_else(|| mismatch("missing helper"))
    }
    pub fn helper_pid(&self) -> Result<u32, AdapterError> {
        self.channels
            .lock()
            .map_err(|_| mismatch("macro channels poisoned"))?
            .as_ref()
            .map(|c| c.pid)
            .ok_or_else(|| mismatch("missing helper"))
    }
    pub fn helper_terminated(&self) -> Result<bool, AdapterError> {
        self.channels
            .lock()
            .map_err(|_| mismatch("macro channels poisoned"))?
            .as_ref()
            .ok_or_else(|| mismatch("missing helper"))?
            .control
            .status()
            .map(|status| status.is_some())
            .map_err(|e| fault("LRR-E-MACRO-CRASH", e.to_string()))
    }
    pub fn diagnostics(&self) -> Result<(Vec<u8>, Vec<u8>), AdapterError> {
        let snapshot = self.diagnostic_snapshot()?;
        Ok((snapshot.protocol_preview, snapshot.stderr_preview))
    }
    pub fn diagnostic_snapshot(&self) -> Result<ProcDiagnosticSnapshot, AdapterError> {
        let stdout = self
            .stdout
            .lock()
            .map_err(|_| mismatch("stdout recorder poisoned"))?;
        let stderr = self
            .stderr
            .lock()
            .map_err(|_| mismatch("stderr recorder poisoned"))?;
        Ok(ProcDiagnosticSnapshot {
            protocol_preview: stdout.bytes.clone(),
            protocol_truncated: stdout.truncated,
            stderr_preview: stderr.bytes.clone(),
            stderr_truncated: stderr.truncated,
        })
    }
    pub fn cancel(&mut self) {
        self.close(fault("LRR-E-MACRO-CRASH", "procedural session cancelled"));
    }
    fn close(&mut self, error: AdapterError) {
        if self.closed.is_none() {
            self.closed = Some(error);
        }
        if let Ok(channels) = self.channels.lock()
            && let Some(channels) = channels.as_ref()
        {
            let _ = channels.control.terminate();
        }
        self.macros.clear();
        self.client = None;
    }

    pub fn cap_execution_deadline(&mut self, deadline: Instant) -> Result<(), AdapterError> {
        if deadline <= Instant::now() {
            self.cancel();
            return Err(fault(
                "LRR-E-MACRO-TIMEOUT",
                "macro execution deadline expired",
            ));
        }
        self.execution_deadline = Some(deadline);
        Ok(())
    }
    pub fn expand(&mut self, request: &ProcInvocation) -> Outcome {
        if request.unit != self.config.unit || request.epoch != self.config.epoch {
            return Err(fault(
                "LRR-E-MACRO-CRASH",
                "wrong consuming unit or expansion epoch",
            ));
        }
        if request.export.is_empty() || request.export.len() > 4096 || request.export.contains('\0')
        {
            return Err(fault("LRR-E-MACRO-NOT-FOUND", "invalid macro export name"));
        }
        request.input.validate()?;
        if request.sources.len() > 4096
            || request
                .sources
                .iter()
                .try_fold(0usize, |sum, source| sum.checked_add(source.text().len()))
                .is_none_or(|bytes| bytes > 32 * 1024 * 1024)
        {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "captured span source allowance exceeded",
            ));
        }
        let mut source_keys = std::collections::BTreeSet::new();
        for source in &request.sources {
            let identity = source.source();
            if !source_keys.insert((identity.unit, identity.content, &identity.path)) {
                return Err(fault(
                    "LRR-E-MACRO-OUTPUT",
                    "duplicate captured source identity",
                ));
            }
        }
        if let Some(attr) = &request.attributes {
            attr.validate()?;
        }
        if (request.form != ProcForm::Attribute && request.attributes.is_some())
            || !request.form.accepts_grammar(request.grammar)
        {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "invalid proc form/grammar/attribute combination",
            ));
        }
        let fingerprint = request.fingerprint();
        if self.helper_terminated()? && self.closed.is_none() {
            self.close(fault(
                "LRR-E-MACRO-CRASH",
                "procedural helper terminated; session is invalid",
            ));
        }
        if let Some(attempt) = self.attempts.get(&request.identity) {
            if attempt.fingerprint != fingerprint {
                return Err(fault(
                    "LRR-E-MACRO-CRASH",
                    "invocation identity reused with different request",
                ));
            }
            if attempt.outcome.is_ok()
                && let Some(error) = &self.closed
            {
                return Err(error.clone());
            }
            return attempt.outcome.clone();
        }
        if let Some(error) = &self.closed {
            return Err(error.clone());
        }
        if self.attempts.len() >= 100_000
            || self
                .retained_bytes
                .checked_add(self.config.output_limit.max(MAX_PREVIEW_BYTES + 512))
                .is_none_or(|size| size > 64 * 1024 * 1024)
        {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "session record allowance exhausted",
            ));
        }
        let mac = self
            .macros
            .get(&(request.artifact, request.export.clone(), request.form))
            .ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-NOT-FOUND",
                    "exact artifact export/kind not found",
                )
            })?
            .clone();
        if mac.kind() != request.form.pinned() {
            return Err(mismatch("macro kind mismatch"));
        }
        let mut store = SpanStore::default();
        let input = store.import_proc(&request.input)?;
        let attributes = request
            .attributes
            .as_ref()
            .map(|attr| store.import_proc(attr))
            .transpose()?;
        let definition = store.span(request.definition_site)?;
        let call = store.span(request.call_site)?;
        let mixed = store.span(request.mixed_site)?;
        let (control, quota, request_quota) = {
            let channels = self
                .channels
                .lock()
                .map_err(|_| mismatch("macro channels poisoned"))?;
            let channels = channels
                .as_ref()
                .ok_or_else(|| mismatch("missing helper"))?;
            (
                channels.control.clone(),
                channels.quota.clone(),
                channels.request_quota.clone(),
            )
        };
        quota
            .reset(self.config.wire_limit)
            .map_err(|e| fault("LRR-E-MACRO-OUTPUT", e.to_string()))?;
        request_quota
            .reset(self.config.wire_limit)
            .map_err(|e| fault("LRR-E-MACRO-OUTPUT", e.to_string()))?;
        let timeout = self
            .execution_deadline
            .map_or(self.config.timeout, |deadline| {
                self.config
                    .timeout
                    .min(deadline.saturating_duration_since(Instant::now()))
            });
        if timeout.is_zero() {
            self.cancel();
            return Err(fault(
                "LRR-E-MACRO-TIMEOUT",
                "macro execution deadline expired",
            ));
        }
        control
            .arm(timeout)
            .map_err(|e| fault("LRR-E-MACRO-CRASH", e.to_string()))?;
        self.invocations = self
            .invocations
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-MACRO-OUTPUT", "macro invocation count overflow"))?;
        let callback_failure = Mutex::new(None);
        let callback = |subrequest| {
            let result = control
                .status()
                .map_err(|error| fault("LRR-E-MACRO-CRASH", error.to_string()))
                .and_then(|status| {
                    if status.is_some() {
                        return Err(fault(
                            "LRR-E-MACRO-CRASH",
                            "span callback arrived after helper termination",
                        ));
                    }
                    store.proc_callback(subrequest, &request.sources)
                });
            result.map_err(|error| {
                if let Ok(mut failure) = callback_failure.lock() {
                    *failure = Some(error.clone());
                }
                proc_macro_api::ServerError {
                    message: error.message,
                    io: None,
                }
            })
        };
        let expanded = mac.expand(
            input.view(),
            attributes.as_ref().map(|a| a.view()),
            self.config
                .environment
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            definition,
            call,
            mixed,
            self.config
                .directory
                .to_str()
                .ok_or_else(|| mismatch("working directory encoding"))?
                .into(),
            Some(&callback),
        );
        let mut outcome = match expanded {
            Ok(Ok(tree)) => store
                .export_proc(&tree, request.identity)
                .and_then(|forest| {
                    if forest.encoded_len()? > self.config.output_limit {
                        return Err(fault(
                            "LRR-E-MACRO-OUTPUT",
                            "procedural token output allowance exceeded",
                        ));
                    }
                    crate::grammar::validate_tree(&store, &tree, request.grammar)?;
                    Ok(Arc::new(forest))
                }),
            Ok(Err(panic)) => Err(fault("LRR-E-MACRO-PANIC", panic)),
            Err(error) => {
                let code = if quota.exhausted()
                    || request_quota.exhausted()
                    || self.stderr_limit_hit.load(Ordering::Acquire)
                {
                    "LRR-E-MACRO-OUTPUT"
                } else if matches!(control.status(), Ok(Some((StopReason::Deadline, _)))) {
                    "LRR-E-MACRO-TIMEOUT"
                } else {
                    "LRR-E-MACRO-CRASH"
                };
                Err(fault(code, error.to_string()))
            }
        };
        if let Some(error) = callback_failure
            .into_inner()
            .map_err(|_| fault("LRR-E-MACRO-OUTPUT", "span callback recorder poisoned"))?
        {
            outcome = Err(error);
        }
        if let Ok(tokens) = &outcome
            && self
                .retained_bytes
                .checked_add(tokens.allocated_bytes()?)
                .and_then(|n| {
                    n.checked_add(std::mem::size_of::<Attempt>() + std::mem::size_of::<Key>() + 256)
                })
                .is_none_or(|n| n > 64 * 1024 * 1024)
        {
            outcome = Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "procedural session retained allocation allowance exceeded",
            ));
        }
        let outcome = if outcome.is_ok() && control.disarm().is_err() {
            let code = if self.stderr_limit_hit.load(Ordering::Acquire) {
                "LRR-E-MACRO-OUTPUT"
            } else if matches!(control.status(), Ok(Some((StopReason::Deadline, _)))) {
                "LRR-E-MACRO-TIMEOUT"
            } else {
                "LRR-E-MACRO-CRASH"
            };
            Err(fault(code, "helper terminated before output admission"))
        } else {
            outcome
        };
        let bytes = match &outcome {
            Ok(tokens) => tokens.allocated_bytes()?,
            Err(error) => error.message.capacity(),
        }
        .checked_add(std::mem::size_of::<Attempt>() + std::mem::size_of::<Key>() + 256)
        .ok_or_else(|| fault("LRR-E-MACRO-OUTPUT", "session record size overflow"))?;
        self.retained_bytes = self
            .retained_bytes
            .checked_add(bytes)
            .ok_or_else(|| fault("LRR-E-MACRO-OUTPUT", "session record size overflow"))?;
        self.attempts.insert(
            request.identity,
            Attempt {
                fingerprint,
                outcome: outcome.clone(),
            },
        );
        if let Err(error) = &outcome {
            self.close(error.clone());
        }
        outcome
    }
}
impl Drop for ProcSession {
    fn drop(&mut self) {
        self.cancel();
    }
}
