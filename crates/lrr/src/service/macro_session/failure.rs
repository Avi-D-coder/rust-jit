//! Bounded external macro faults, separate from guest execution and guest panic.
use super::*;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroFaultPhase {
    MacroExpand,
}
#[derive(Debug, Clone, Serialize)]
pub struct MacroFaultReport {
    pub phase: MacroFaultPhase,
    pub demand_context: Digest,
    pub suspended_phase: Digest,
    pub unit: Digest,
    pub epoch: u64,
    pub invocation: Digest,
    pub artifact: Digest,
    pub call_site: Digest,
    pub call_site_origin: Digest,
    pub parent_origin: Digest,
    pub scope: Digest,
    pub diagnostic: Diagnostic,
    /// Protocol and helper diagnostics remain separate from guest output.
    pub protocol_preview: Vec<u8>,
    pub protocol_truncated: Option<bool>,
    pub stderr_preview: Vec<u8>,
    pub stderr_truncated: Option<bool>,
    pub native_attempts: u64,
    /// None means the status observation failed, with its error recorded below.
    pub helper_reaped: Option<bool>,
    pub capture_errors: Vec<Diagnostic>,
}
impl MacroFaultReport {
    pub(super) fn capture(
        context: &DemandContext,
        token: &DemandToken,
        request: &ProcInvocation,
        call_origin: Key,
        diagnostic: Diagnostic,
        session: &ProcSession,
    ) -> Self {
        let mut capture_errors = Vec::new();
        let (protocol_preview, stderr_preview, protocol_truncated, stderr_truncated) =
            match session.diagnostic_snapshot() {
                Ok(channels) => (
                    channels.protocol_preview,
                    channels.stderr_preview,
                    Some(channels.protocol_truncated),
                    Some(channels.stderr_truncated),
                ),
                Err(error) => {
                    capture_errors.push(adapter(error));
                    (Vec::new(), Vec::new(), None, None)
                }
            };
        let helper_reaped = match session.helper_terminated() {
            Ok(reaped) => Some(reaped),
            Err(error) => {
                capture_errors.push(adapter(error));
                None
            }
        };
        Self {
            phase: MacroFaultPhase::MacroExpand,
            demand_context: context.fingerprint(),
            suspended_phase: token.phase().fingerprint(),
            unit: context.unit,
            epoch: context.epoch,
            invocation: Digest(request.identity.0),
            artifact: Digest(request.artifact.0),
            call_site: Digest(request.call_site.0),
            call_site_origin: Digest(call_origin.0),
            parent_origin: Digest(request.parent_origin.0),
            scope: Digest(request.scope.0),
            diagnostic,
            protocol_preview,
            protocol_truncated,
            stderr_preview,
            stderr_truncated,
            native_attempts: session.invocations(),
            helper_reaped,
            capture_errors,
        }
    }
}
