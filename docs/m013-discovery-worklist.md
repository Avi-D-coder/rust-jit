# M013 discovery transition checkpoint

Discovery owns a deterministic source/module-ordered queue. Job kinds distinguish
parsing, cfg, headers, import/macro resolution, expansion, integration and nested
scan. An invocation can advance through multiple kinds with the same identity.
A secondary deterministic index detects conflicting source order without scanning
all jobs. Re-enqueuing an exact completed job does not replay it.

Only the active epoch/attempt ticket can complete or block a job. Blocked producer
chains remain explicit and sorted. New membership wakes blocked jobs; a stable
pass with blocked producers returns LRR-E-CATALOG-INCOMPLETE, never a partial seal
or missing export. Polling an active job repeats its ticket, not its execution.
External services must retain their existing exact-once invocation protocol.

Completion preflights counts and metadata before cloning candidate state. Inputs
with conflicting fingerprints, stale tickets or limits leave active state intact.
Seal issuance requires no ready, active or blocked jobs. Private seal records bind
profile, domain, unit graph, epoch, sorted membership, all completed scanned jobs,
and sorted input fingerprints. Empty membership does not erase scanned-source
identity. Late enqueue after sealing is LRR-E-EXPANSION-SEAL-VIOLATION.

These eight controls exercise the actual transition engine. They do not represent
complete MX027/MX028 source fixtures. The next M013 work must seed jobs from exact
source publications, drive existing import/macro resolution, validate and integrate
actual expansion records, and publish seals into query dependencies. Generated
import/macro cycles must use that integrated path; a fabricated update is not proof
of expansion. Nested impl/test discovery consumers remain M014/M015. Macro
execution stays external and authorized; this scheduler performs no expansion,
VM execution, type checking, native loading, process creation or ambient IO.

The limit interface reduces the fixed job/header/metadata maxima. Actual token,
output-byte, import-edge, expansion-depth/deadline limits belong to the integrated
producer adapters and must be exercised before M013 completion. The task and full
project remain in progress, with no release, full LRR fixture or Linux claim.
