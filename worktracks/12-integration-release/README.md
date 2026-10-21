# WT12 — Integration, contribution coordination and release evidence

Keep contributor work reviewable, merge in dependency order and produce reproducible runtime compatibility and release evidence.

## Current boundary

The contribution system is being established locally. No GitHub issues, PRs, release or complete runtime/full-suite acceptance are claimed by creating these plans.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `T003`, `T101`, `T102`, `T103`, `T104`, `T110`, `T111`, `T112`, `T120`, `T121`, `T122`, `M043`, `M044`, `M045`, `M046`, `M047`, `M048`, `H026`, `H027`, `H028`.
- Suggested code areas: `worktracks/`, `.github/`, `docs/implementation-state.md`, `docs/feature-traceability.md`, `xtask/src/gates.rs`, `artifacts/release/`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: none at whole-track level; each slice still has its ledger/API prerequisites.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT12-01 — Claims and integration queue

Maintain public issue claims, narrow path/API ownership, dependency links, review states and append-only merge/evidence history. Avoid duplicate agents and shared-checkout mutations.

### WT12-02 — Four compatibility gates

Publish complete inventory-backed runtime, diagnostic, tooling and full-suite reports plus interpreter correctness. Unknown runtime obligations and missing evidence keep guest JIT closed.

### WT12-03 — Frozen-source acceptance

Collect real asynchronous command terminals, sources/builds/raw logs and required fixture observations; run the current milestone after integration and preserve failed attempts.

### WT12-04 — Reproduction and reporting

Reproduce macOS from a clean checkout with pins/licenses and accurate feature/security/cost reports. Record later Linux evidence separately; untested platform support is not implied.

### WT12-05 — Later optimization release lane

Only after runtime/correctness acceptance and ledger prerequisites, schedule T070–T089/M035–M037 guest tiers and associated equivalence/performance work. Plans and historical bootstrap probes never count as native guest entries.

## First assignable slice

Review one external bounded slice and its dependency/evidence manifest, then maintain plans after each merge and during an active weekly triage. Root owns this initial contribution documentation.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

Every merged change traces claim→PR→task/invariant→upstream fixtures→actual evidence→integrated gate; four report denominators reconcile without upgrading component checks to fixture passes.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Build and validate directly in the shared checkout with one Cargo owner; run unchanged official rustc programs asynchronously on frozen tested sources; component or mocked checks cannot establish interpreter acceptance; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

All tracks feed this lane. Review-ready candidates may be prepared in parallel; dependency-blocked work cannot be integrated as complete or start guest JIT.
