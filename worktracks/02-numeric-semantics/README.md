# WT02 — Numeric inference, primitives and constants

Complete Rust-compatible source constraints and primitive behavior without moving effects or forcing cold semantics.

## Current boundary

Decoded integer recipes and bounded primitive cast contexts have scoped acceptance; general source constraints, primitive coverage and pure constants remain incomplete.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `T022`, `T024`, `T025`, `T054`.
- Suggested code areas: `crates/lrr/src/syntax/local_numeric_constraints.rs`, `crates/lrr/src/syntax/numeric_fallback.rs`, `crates/lrr/src/runtime/cell.rs`, `crates/lrr/src/vm/`, `crates/lrr/src/frontend/constants.rs`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: none at whole-track level; each slice still has its ledger/API prerequisites.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT02-01 — Reached local constraints

Extend source-selected numeric constraints from declaration and later exact member/call signatures, with source identity and lexical shadowing checks; never run later statements for type information.

### WT02-02 — Literal and cast boundaries

Cover every admitted width/base/suffix, negative minima, overflow, float/char conversions and exact written diagnostics using stock originals. Keep independent literal bindings and lexical decode-once behavior.

### WT02-03 — Primitive operator coverage

Complete arithmetic, comparisons, shifts, boolean/bitwise and compound assignment under source-selected operation kinds. Delegate overloaded/reference operators to WT05 rather than matching runtime tags.

### WT02-04 — Pure constant evaluator

Implement budgeted immutable constants, array lengths, bounded const calls, cycle outcomes and reached forbidden-effect refusal; instantiate fresh mutable guest aggregates per use.

### WT02-05 — Cache and cost acceptance

Pair with WT10 for decoded schema cold misses/invalidation and record actual allocations/timings independently from semantic passes.

## First assignable slice

Review the frozen later-call numeric constraints and compound-assignment candidates, then select a stock numeric frontier outside WT04 lexical admission.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

Unchanged numeric originals exercise exact behavior across their required owning editions; overflow/refusal attribution, cold-path laziness, fuel and fresh bindings are checked outside originals.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Build and validate directly in the shared checkout with one Cargo owner; run unchanged official rustc programs asynchronously on frozen tested sources; component or mocked checks cannot establish interpreter acceptance; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

Source call headers come from WT04; trait operators from WT05; constant descriptors/guest storage from WT06. Shared thunks/cell files need a named slice reservation.
