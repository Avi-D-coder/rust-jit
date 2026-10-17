//! Explicit coordinator artifacts. Guest values, environment and file contents
//! never enter the record; collecting it performs no query or compilation.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
        profile,
    },
    engine::{
        failure::{ExecutionRecord, FailureContext},
        run::RunMachine,
    },
    source::snapshot::Snapshot,
};
use serde::Serialize;
use std::{
    cell::Cell,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(super) enum BundleError {
    Limit,
    Encode(serde_json::Error),
    Io(io::Error),
}
impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limit => f.write_str("diagnostic bundle resource limit"),
            Self::Encode(error) => write!(f, "diagnostic bundle encoding failed: {error}"),
            Self::Io(error) => write!(f, "diagnostic bundle I/O failed: {error}"),
        }
    }
}

/// Fixed-size provenance summary. Never serialize diagnostic source paths,
/// excerpts, producer names, messages or raw token origin records into a bundle.
#[derive(Serialize)]
struct OriginSummary {
    macro_origin_digest: Option<Digest>,
    generated_origin_digest: Option<Digest>,
    guest_stack_digest: Option<Digest>,
    macro_parent_count: usize,
    generated_token_count: usize,
    guest_frame_count: usize,
    omitted_guest_frames: usize,
    attachment_failure: Option<&'static str>,
}
impl OriginSummary {
    fn capture(error: &Diagnostic) -> Self {
        Self {
            macro_origin_digest: error
                .macro_origin
                .as_ref()
                .map(|origin| origin.fingerprint()),
            generated_origin_digest: error
                .generated_origin
                .as_ref()
                .map(|origin| origin.fingerprint()),
            guest_stack_digest: error.guest_stack.as_ref().map(|stack| stack.fingerprint()),
            macro_parent_count: error
                .macro_origin
                .as_ref()
                .map_or(0, |origin| origin.parents.len()),
            generated_token_count: error
                .generated_origin
                .as_ref()
                .map_or(0, |origin| origin.tokens.len()),
            guest_frame_count: error
                .guest_stack
                .as_ref()
                .map_or(0, |stack| stack.frames.len()),
            omitted_guest_frames: error
                .guest_stack
                .as_ref()
                .map_or(0, |stack| stack.omitted_frames),
            attachment_failure: error.origin_attachment_failure,
        }
    }
}

#[derive(Serialize)]
struct Bundle<'a> {
    schema: &'static str,
    local_confidential_artifact: bool,
    first_internal_defect_only: bool,
    environment_values_omitted: bool,
    runtime_file_contents_omitted: bool,
    guest_values_and_messages_omitted: bool,
    code: &'static str,
    origins: OriginSummary,
    runtime_version: &'static str,
    semantics_profile: &'static str,
    configured_target: &'static str,
    dependency_lock_digest: Digest,
    dependency_source_pins_digest: Digest,
    expansion_engine_commit: &'static str,
    snapshot: Option<Digest>,
    source_digests: Vec<Digest>,
    omitted_sources: usize,
    execution: Option<&'a ExecutionRecord>,
    execution_context_availability: &'static str,
}

fn reserve<T>(vector: &mut Vec<T>, count: usize) -> Result<(), BundleError> {
    vector
        .try_reserve_exact(count)
        .map_err(|_| BundleError::Limit)
}
fn collect<'a>(
    error: &Diagnostic,
    snapshot: Option<&Snapshot>,
    execution: Option<&'a ExecutionRecord>,
    execution_context_availability: &'static str,
) -> Result<Bundle<'a>, BundleError> {
    let mut source_digests = Vec::new();
    let count = snapshot.map_or(0, |snapshot| snapshot.files().len());
    reserve(&mut source_digests, count)?;
    if let Some(snapshot) = snapshot {
        // Captured path order is deterministic. Paths/text/env are not copied.
        source_digests.extend(
            snapshot.files().values().map(|blob| blob.content()),
        );
    }
    Ok(Bundle {
        schema: "lrr/v2/local-failure-bundle",
        local_confidential_artifact: true,
        first_internal_defect_only: true,
        environment_values_omitted: true,
        runtime_file_contents_omitted: true,
        guest_values_and_messages_omitted: true,
        code: error.code,
        origins: OriginSummary::capture(error),
        runtime_version: profile::RUNTIME_VERSION,
        semantics_profile: profile::SEMANTICS_PROFILE,
        configured_target: profile::GUEST_TARGET,
        dependency_lock_digest: Digest::of_bytes(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../Cargo.lock"
        ))),
        dependency_source_pins_digest: Digest::of_bytes(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/UPSTREAM.toml"
        ))),
        expansion_engine_commit: lrr_expand_adapter::UPSTREAM_COMMIT,
        snapshot: snapshot.map(Snapshot::id),
        omitted_sources: count - source_digests.len(),
        source_digests,
        execution,
        execution_context_availability,
    })
}

struct LimitedBytes(Vec<u8>, bool);
impl Write for LimitedBytes {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// This object is constructed only by an explicit CLI option or host test. It
/// carries no guest capability and is never retained by a query or worker frame.
pub(super) struct Authority {
    directory: PathBuf,
    attempted: Cell<bool>,
}
impl Authority {
    pub(super) fn explicit(directory: &Path) -> Self {
        Self {
            directory: directory.into(),
            attempted: Cell::new(false),
        }
    }
    pub(super) fn write(
        &self,
        error: &Diagnostic,
        snapshot: Option<&Snapshot>,
        run: Option<&RunMachine>,
    ) -> Result<Option<PathBuf>, BundleError> {
        if !error.is_internal() || self.attempted.replace(true) {
            return Ok(None);
        }
        let live = if error.failure_context.is_none() {
            run.map(ExecutionRecord::capture)
        } else {
            None
        };
        let (execution, availability) = match error.failure_context.as_deref() {
            Some(FailureContext::Available(record)) => {
                (Some(record), "retained terminal dispatcher facts")
            }
            Some(FailureContext::ResourceUnavailable) => (None, "terminal metadata resource limit"),
            None => match live.as_ref() {
                Some(Ok(record)) => (Some(record), "live dispatcher facts"),
                Some(Err(_)) => (None, "live metadata resource limit"),
                None => (None, "execution was not started or retained"),
            },
        };
        let record = collect(error, snapshot, execution, availability)?;
        let mut bytes = LimitedBytes(Vec::new(), false);
        let encoded = serde_json::to_writer(&mut bytes, &record);
        if bytes.1 {
            return Err(BundleError::Limit);
        }
        encoded.map_err(BundleError::Encode)?;
        let serial = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| BundleError::Limit)?;
        std::fs::create_dir_all(&self.directory).map_err(BundleError::Io)?;
        let path = self
            .directory
            .join(format!("lrr-internal-{}-{serial}.json", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(BundleError::Io)?;
        file.write_all(&bytes.0).map_err(BundleError::Io)?;
        file.sync_all().map_err(BundleError::Io)?;
        Ok(Some(path))
    }
    pub(super) fn report(
        &self,
        error: &Diagnostic,
        snapshot: Option<&Snapshot>,
        run: Option<&RunMachine>,
    ) {
        match self.write(error, snapshot, run) {
            Ok(Some(path)) => eprintln!("Local confidential diagnostic bundle: {}", path.display()),
            Ok(None) => {}
            Err(error) => eprintln!("{error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{memo::Database, poll::QueryPoll};
    use crate::engine::run::RunPoll;

    fn stock_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/upstream/rustc-checkout/tests/ui/derives/deriving-meta-empty-trait-list.rs")
    }
    fn directory(label: &str) -> PathBuf {
        let serial = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .unwrap();
        std::env::temp_dir().join(format!(
            "lrr-bundle-{}-{serial}-{label}",
            std::process::id()
        ))
    }

    #[test]
    fn rustc_captured_body_failure_bundle_requires_explicit_authority_and_redacts_values() {
        // The unchanged stock body supplies real installed LRIR and counters.
        // The internal trigger is explicitly host-test synthetic, not a claimed
        // compiler defect or a full pass of the stock derive fixture.
        let (prepared, view, sources, snapshot) =
            super::super::capture_standalone(&stock_path(), false).unwrap();
        let body = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        let mut db = Database::default();
        prepared.publish(&mut db).unwrap();
        let QueryPoll::Ready(Ok(mut run)) =
            RunMachine::start_with_sources(&mut db, body, 100_000, sources)
        else {
            panic!("stock body startup refused");
        };
        let trace_before_origins = db.trace.events.len();
        let origins = crate::engine::failure::guest_stack(&run, &mut db).unwrap();
        assert_eq!(origins.frames.len(), 1);
        assert_eq!(origins.frames[0].locations.len(), 1);
        assert_eq!(
            db.trace.events.len(),
            trace_before_origins,
            "origin summary capture ran queries"
        );
        let origin_digest = origins.fingerprint();
        let private_path = origins.frames[0].locations[0].source.path.clone();
        db.begin_run().unwrap();
        assert!(matches!(run.poll(&mut db), RunPoll::Completed(_)));
        db.finish_run().unwrap();
        assert!(!run.installed_recipes().is_empty());
        let before_trace = db.trace.events.len();
        let before_counters = serde_json::to_value(run.opcode_counters().unwrap()).unwrap();
        let root = directory("explicit");
        assert!(!root.exists());
        let authority = Authority::explicit(&root);
        let source_failure = Diagnostic::new(
            "LRR-E-MACRO-EXPANSION",
            "non-internal source failure".into(),
        );
        assert!(
            authority
                .write(&source_failure, Some(&snapshot), Some(&run))
                .unwrap()
                .is_none()
        );
        assert!(
            !root.exists(),
            "ordinary diagnostics obtained artifact authority"
        );
        let secret = "runtime-file-content-and-environment-value-sentinel";
        let mut defect = Diagnostic::new("LRR-E-INTERNAL-IR", secret.into());
        defect.guest_stack = Some(Box::new(origins));
        let artifact = authority
            .write(&defect, Some(&snapshot), Some(&run))
            .unwrap()
            .unwrap();
        let bytes = std::fs::read(&artifact).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(!text.contains(secret));
        assert!(
            !text.contains(&private_path),
            "bundle exposed a diagnostic source path"
        );
        assert!(
            !text.contains("\"locations\""),
            "bundle serialized raw frame origins"
        );
        assert!(
            !text.contains("\"excerpt\""),
            "bundle serialized source excerpts"
        );
        assert!(!text.contains("pub fn main()"));
        let record: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            record["snapshot"],
            serde_json::to_value(snapshot.id()).unwrap()
        );
        assert_eq!(
            record["source_digests"][0],
            serde_json::to_value(snapshot.files().values().next().unwrap().content()).unwrap()
        );
        assert_eq!(
            record["origins"]["guest_stack_digest"],
            serde_json::to_value(origin_digest).unwrap()
        );
        assert_eq!(record["origins"]["guest_frame_count"], 1);
        assert_eq!(record["origins"]["omitted_guest_frames"], 0);
        assert!(record["origins"]["macro_origin_digest"].is_null());
        assert!(record["origins"]["generated_origin_digest"].is_null());
        assert_eq!(record["local_confidential_artifact"], true);
        assert_eq!(record["runtime_file_contents_omitted"], true);
        assert_eq!(record["environment_values_omitted"], true);
        assert!(!record["execution"]["lrir"].as_array().unwrap().is_empty());
        assert!(
            record["execution"]["frames"][0]["counters"]["operations_started"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(record["execution"]["clif"].is_null());
        assert_eq!(db.trace.events.len(), before_trace, "recording ran a query");
        assert_eq!(
            serde_json::to_value(run.opcode_counters().unwrap()).unwrap(),
            before_counters
        );
        assert!(
            authority
                .write(&defect, Some(&snapshot), Some(&run))
                .unwrap()
                .is_none()
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&artifact).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rustc_body_stale_dispatch_failure_retains_context_after_machine_drop() {
        // The stock source remains unchanged. This is the LRR stale-publication
        // invariant, not rustc language acceptance or a synthetic error code.
        let (prepared, view, _, snapshot) =
            super::super::capture_standalone(&stock_path(), false).unwrap();
        let body = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        let mut db = Database::default();
        prepared.publish(&mut db).unwrap();
        let QueryPoll::Ready(Ok(mut run)) = RunMachine::start(&mut db, body, 100_000) else {
            panic!("stock body startup refused");
        };
        let source_revision = run.installed_recipes().source_revision();
        // Publication is legal between runs, but this machine belongs to the
        // earlier publication and must refuse its next dispatch.
        prepared.publish(&mut db).unwrap();
        assert_ne!(db.source_revision(), source_revision);
        let trace_before = db.trace.events.len();
        db.begin_run().unwrap();
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("stale machine executed instead of refusing dispatch");
        };
        db.finish_run().unwrap();
        assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
        let Some(FailureContext::Available(context)) = error.failure_context.as_deref() else {
            panic!("terminal dispatcher context was not retained");
        };
        assert_eq!(context.source_revision, source_revision);
        assert_eq!(context.frames.len(), 1);
        assert_eq!(context.frames[0].counters.operations_started, 0);
        let public = serde_json::to_value(&error).unwrap();
        assert!(public.get("failure_context").is_none());
        let bare = Diagnostic::new(error.code, error.message.clone());
        assert!(error.heap_bytes().unwrap() > bare.heap_bytes().unwrap());
        let retained = serde_json::to_value(context).unwrap();
        drop(run);
        let root = directory("retained");
        assert!(!root.exists(), "capture performed coordinator I/O");
        let authority = Authority::explicit(&root);
        let path = authority
            .write(&error, Some(&snapshot), None)
            .unwrap()
            .unwrap();
        let record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(record["execution"], retained);
        assert_eq!(
            record["execution_context_availability"],
            "retained terminal dispatcher facts"
        );
        assert_eq!(
            db.trace.events.len(),
            trace_before,
            "fault collection ran queries"
        );
        assert!(
            authority
                .write(&error, Some(&snapshot), None)
                .unwrap()
                .is_none()
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn diagnostic_bundle_missing_execution_is_explicit() {
        let defect = Diagnostic::new("LRR-E-INTERNAL-SNAPSHOT", "secret message".into());
        let record = collect(&defect, None, None, "test: no execution").unwrap();
        let mut output = LimitedBytes(Vec::new(), false);
        serde_json::to_writer(&mut output, &record).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&output.0).unwrap();
        assert!(json["snapshot"].is_null() && json["execution"].is_null());
        assert_eq!(json["omitted_sources"], 0);
        assert!(json["origins"]["guest_stack_digest"].is_null());
        assert_eq!(json["origins"]["guest_frame_count"], 0);
    }
}
