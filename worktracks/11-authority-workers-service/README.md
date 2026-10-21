# WT11 — Authority, workers and service protocols

Enforce separate effect/build/macro/native authorities and bounded worker/service continuation lifetimes.

## Current boundary

Macro supervision and service components exist; real guest broker/worker isolation and complete revision-5 protocol/service acceptance remain incomplete.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `T064`, `T065`, `T092`, `T093`, `T094`, `M009`, `M037`, `M041`, `M042`, `M045`, `H020`, `H021`, `H024`, `H025`.
- Suggested code areas: `crates/lrr/src/service/`, `crates/lrr/src/cli/`, `crates/lrr/src/bin/lrr-worker.rs`, `crates/lrr/src/broker/`, `crates/lrr/src/engine/worker.rs`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: none at whole-track level; each slice still has its ledger/API prerequisites.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT11-01 — Guest effect broker

Implement real filesystem/env/output grants and revocation with deterministic fake-broker equivalence; authorize paths/operations before effects and report exact limits.

### WT11-02 — Worker lifecycle

Implement fresh managed/hybrid workers, bounded IO/control channels, owned process groups, deadlines/cancellation and reset policies without coordinator native-library loading.

### WT11-03 — Service schemas and raw framing

Implement specified public JSON/internal binary versions, checked transfers/digests/budgets and wrong-role/build/schema refusal. Macro-server traffic retains its separate protocol.

### WT11-04 — Resumable requests and watch

Handle overlays, stale/duplicate/late replies, disconnect, shutdown and preparation freshness without repeated guest/macro/native effects or implicit builds.

### WT11-05 — Robustness and telemetry

Run controlled crash/hang/truncation/quota/cancellation regressions and report actual phase/authority/sandbox/tier counters; native helpers never receive a hardened sandbox claim.

## First assignable slice

Choose one stock runtime effect dependency and its broker continuation boundary, or a bounded protocol contract after checking existing M009 ownership; shared macro session edits belong to a coordinated slice.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

Denied/stale/cancelled operations publish no effect; accepted operations execute once across suspension; child ownership and byte quotas have actual terminal evidence.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Build and validate directly in the shared checkout with one Cargo owner; run unchanged official rustc programs asynchronously on frozen tested sources; component or mocked checks cannot establish interpreter acceptance; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

WT09 tool preparation and WT08 macro execution need distinct authorities. These interpreter correctness requirements can block the runtime prerequisite even when stock language outputs agree.
