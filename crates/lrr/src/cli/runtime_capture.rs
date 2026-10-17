//! CLI protocol rendering of live, retained executor observations.
//! Transport, host preparation diagnostics and guest streams are separate.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    engine::run::{RunMachine, RunPoll, TerminalExecution},
    source::snapshot::Snapshot,
};
pub(super) struct Request {
    pub enabled: bool,
    pub context: Option<Digest>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-RUNTIME-CAPTURE", message.into())
}
#[derive(serde::Serialize)]
struct CapturedFile {
    path: String,
    content: Digest,
    bytes: u64,
}
#[derive(serde::Serialize)]
struct SnapshotObservation {
    identity: Digest,
    owning_edition: u16,
    root_source: String,
    root_crate: Digest,
    root_cfg: Digest,
    files: Vec<CapturedFile>,
}
impl SnapshotObservation {
    fn observe(snapshot: &Snapshot, owning_edition: u16) -> Result<Self, Diagnostic> {
        let mut files = Vec::new();
        files
            .try_reserve_exact(snapshot.files().len())
            .map_err(|_| fault("capture source allocation"))?;
        let mut bytes = files
            .capacity()
            .checked_mul(std::mem::size_of::<CapturedFile>())
            .ok_or_else(|| fault("capture source metadata limit"))?;
        for (path, blob) in snapshot.files() {
            bytes = bytes
                .checked_add(path.as_str().len())
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| fault("capture source metadata limit"))?;
            let mut owned = String::new();
            owned
                .try_reserve_exact(path.as_str().len())
                .map_err(|_| fault("capture path allocation"))?;
            owned.push_str(path.as_str());
            files.push(CapturedFile {
                path: owned,
                content: blob.content(),
                bytes: u64::try_from(blob.bytes().len())
                    .map_err(|_| fault("capture byte width"))?,
            });
        }
        let project = snapshot.project();
        let cfg = snapshot
            .cfg_by_crate()
            .get(&project.selected_crate)
            .ok_or_else(|| fault("capture root cfg missing"))?;
        Ok(Self {
            identity: snapshot.id(),
            owning_edition,
            root_source: project.root_source.as_str().to_owned(),
            root_crate: project.selected_crate,
            root_cfg: cfg.fingerprint(),
            files,
        })
    }
}
#[derive(serde::Serialize)]
struct GuestStreams {
    status: &'static str,
    /// Binary bytes; JSON transport never belongs in these arrays. Empty is
    /// possible only with a live closed no-external-effect producer witness.
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
}
#[derive(serde::Serialize)]
struct CaptureObservation {
    complete: bool,
    scope: &'static str,
    facts: Option<serde_json::Value>,
    guest: GuestStreams,
    blocker: Option<&'static str>,
}
#[derive(serde::Serialize)]
pub(super) struct Packet {
    schema: &'static str,
    transport_stdout_role: &'static str,
    host_preparation_streams: &'static str,
    request_context: Option<Digest>,
    process_id: u32,
    snapshot: Option<SnapshotObservation>,
    entry_body: Option<crate::source::manifest::BodyKey>,
    terminal: serde_json::Value,
    capture: CaptureObservation,
    #[serde(skip)]
    exit_code: i32,
}
impl Packet {
    fn incomplete(
        context: Option<Digest>,
        terminal: serde_json::Value,
        blocker: &'static str,
    ) -> Self {
        Self {
            schema: "lrr/v5/standalone-runtime-capture/v1",
            transport_stdout_role: "protocol_json_not_guest_stdout",
            host_preparation_streams: "outside_guest_capture_no_macro_host_or_build_output_promotion",
            request_context: context,
            process_id: std::process::id(),
            snapshot: None,
            entry_body: None,
            terminal,
            capture: CaptureObservation {
                complete: false,
                scope: "dispatcher_external_effects_only_not_memory_trace",
                facts: None,
                guest: GuestStreams {
                    status: "unobserved",
                    stdout: None,
                    stderr: None,
                },
                blocker: Some(blocker),
            },
            exit_code: 2,
        }
    }
    pub(super) fn preparation_fault(
        error: Diagnostic,
        context: Option<Digest>,
    ) -> Result<Self, Diagnostic> {
        Ok(Self::incomplete(
            context,
            serde_json::json!({"status":"fault", "phase":"before_observation_or_finish_fault",
            "code":error.code,"message":error.message,"diagnostic":error}),
            "executor_not_observed",
        ))
    }
    pub(super) fn observe(
        run: &RunMachine,
        result: RunPoll,
        snapshot: &Snapshot,
        owning_edition: u16,
        context: Option<Digest>,
    ) -> Result<Self, Diagnostic> {
        let retained = run.terminal_facts();
        // Guest `_print`/`_eprint` bytes written through the managed facade.
        let mut output = crate::engine::run::GuestOutput::default();
        let (terminal, code) = match result {
            RunPoll::Completed(report) => {
                let Some(facts) = retained else {
                    return Err(fault("completed report lacks retained terminal"));
                };
                let TerminalExecution::Completed { unit } = &facts.outcome else {
                    return Err(fault("retained completion differs"));
                };
                if *unit != matches!(report.value, crate::engine::run::ValueSummary::Unit)
                    || report.live_edition != owning_edition
                {
                    return Err(fault("completed report identity differs"));
                }
                output = report.output.clone();
                (
                    serde_json::to_value(report)
                        .map_err(|_| fault("capture completed serialization"))?,
                    0,
                )
            }
            RunPoll::Panicked(report) => {
                let Some(facts) = retained else {
                    return Err(fault("panic report lacks retained terminal"));
                };
                let TerminalExecution::GuestPanic { message } = &facts.outcome else {
                    return Err(fault("retained panic differs"));
                };
                if message != report.panic.description() || report.live_edition != owning_edition {
                    return Err(fault("panic report identity differs"));
                }
                output = report.output.clone();
                (
                    serde_json::to_value(report)
                        .map_err(|_| fault("capture panic serialization"))?,
                    1,
                )
            }
            RunPoll::Fault(error) => (
                serde_json::json!({"status":"fault","phase":"executor","code":error.code,"message":error.message,"diagnostic":error}),
                2,
            ),
            RunPoll::NeedInput(_) => (
                serde_json::json!({"status":"suspended","phase":"executor",
                "code":"LRR-E-UNSUPPORTED","message":"runtime capture requires later input/macro authority"}),
                2,
            ),
        };
        let complete = run
            .runtime_capture_facts()
            .is_some_and(|facts| facts.complete());
        let mut packet = Self::incomplete(
            context,
            terminal,
            "fault_suspension_unhandled_boundary_or_missing_closed_producer",
        );
        packet.exit_code = code;
        packet.snapshot = Some(SnapshotObservation::observe(snapshot, owning_edition)?);
        packet.entry_body = Some(run.root_body().clone());
        packet.capture.facts = run
            .runtime_capture_facts()
            .map(|facts| {
                serde_json::to_value(facts).map_err(|_| fault("capture facts serialization"))
            })
            .transpose()?;
        if complete {
            packet.capture.complete = true;
            packet.capture.blocker = None;
            packet.capture.guest = GuestStreams {
                status: if output.stdout.is_empty() && output.stderr.is_empty() {
                    "closed_no_external_effects_observed"
                } else {
                    "closed_managed_output_observed"
                },
                stdout: Some(output.stdout.into_bytes()),
                stderr: Some(output.stderr.into_bytes()),
            };
        }
        // Bound the serialized packet through a bounded writer. This does not
        // serialize directly onto host/guest streams or accept partial JSON.
        packet.render()?;
        Ok(packet)
    }
    pub(super) fn exit_code(&self) -> i32 {
        self.exit_code
    }
    #[cfg(test)]
    pub(super) fn guest_complete(&self) -> bool {
        self.capture.complete
    }
    #[cfg(test)]
    pub(super) fn guest_stdout(&self) -> Option<&[u8]> {
        self.capture.guest.stdout.as_deref()
    }
    #[cfg(test)]
    pub(super) fn guest_stderr(&self) -> Option<&[u8]> {
        self.capture.guest.stderr.as_deref()
    }
    pub(super) fn render(&self) -> Result<Vec<u8>, Diagnostic> {
        struct Bounded(Vec<u8>);
        impl std::io::Write for Bounded {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                let next = self
                    .0
                    .len()
                    .checked_add(bytes.len())
                    .ok_or_else(|| std::io::Error::other("runtime capture packet limit"))?;
                self.0
                    .try_reserve(next - self.0.len())
                    .map_err(std::io::Error::other)?;
                self.0.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = Bounded(Vec::new());
        serde_json::to_writer(&mut writer, self)
            .map_err(|_| fault("capture serialization/output bound"))?;
        Ok(writer.0)
    }
}
