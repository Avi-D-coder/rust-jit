# WT10 — Queries, derived facts and portable caches

Preserve deterministic dependency-tracked reuse, pure queries, bounded publication and portable schema-safe caches.

## Current boundary

Typed fetch/suspension and memo components exist; complete revision-5 persistent decoded-record restore and goal/macro portable caches remain incomplete.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `T010`, `T011`, `T013`, `T014`, `T015`, `M008`, `M012`, `M027`, `M036`, `H022`, `H023`.
- Suggested code areas: `crates/lrr/src/db/`, `crates/lrr/src/source/changes.rs`, `crates/lrr/src/source/checkpoint.rs`, `crates/lrr/src/expand/cache.rs`, `crates/lrr/src/traits/cache.rs`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: none at whole-track level; each slice still has its ledger/API prerequisites.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT10-01 — Atomic derived publication

Expose narrow coordinator-owned transactions with checked metadata, source/epoch/seal certificates and all-or-nothing facts; reject publication from providers or stale callers.

### WT10-02 — Positive and negative dependencies

Invalidate selected/absent/ambiguous answers under source, namespace, generated impl and macro epoch edits; retain semantic reuse only with exact dependency verification.

### WT10-03 — Portable stable encoding

Implement versioned records with explicit tags/lengths/IDs and fresh interning; reject stale literal/recipe schemas as cache misses and never persist pointers or guest/session state.

### WT10-04 — Cache quotas and reclamation

Bound memo/input/continuation bytes and generations; keep active immutable facts usable while retiring old metadata deterministically.

### WT10-05 — Restart and edit acceptance

Exercise cold/warm/body/header/macro/generated edits and fresh workers, with required procedural revalidation and actual allocation/cost evidence.

## First assignable slice

Root owns the lexical-call admission transaction. A separate contributor can prepare the CA027 persistent-reader/schema-miss slice once the exact checkpoint interface is agreed.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

Stale/negative/schema inputs cannot be adopted; pure provider NeedInput causes no effects; restart remaps identities and all quota failures preserve prior state.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Run validation in a separate frozen checkout/target asynchronously; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

Publication callers WT04/WT08/WT05 share narrow interfaces. Full M036/H022 integration has later ledger dependencies; isolated preparatory work is not task completion.
