# WT06 — Aggregates, places, ownership and cleanup

Complete guest storage and checked ownership semantics for structural values, references, user destruction and managed boxes/dyn views.

## Current boundary

Aggregate/place/cleanup components exist, with many historical cards component-complete; that is not complete upstream ownership or revision-5 integration acceptance.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `T040`, `T041`, `T042`, `T043`, `T044`, `T045`, `M029`, `M032`.
- Suggested code areas: `crates/lrr/src/runtime/`, `crates/lrr/src/engine/run_places.rs`, `crates/lrr/src/engine/run_fields.rs`, `crates/lrr/src/engine/run_panic.rs`, `crates/lrr/src/vm/cleanup.rs`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: none at whole-track level; each slice still has its ledger/API prerequisites.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT06-01 — Place and lease coverage

Finish projections, disjoint borrows, mutable/shared conflicts, reborrows, epochs and use-after-move outcomes under checked guest semantics.

### WT06-02 — Structural transfer and construction

Complete partial initialization/moves, aggregate copy policy, assignment replacement and quota-reserved transactions without duplicate ownership or leaked obligations.

### WT06-03 — User Copy Clone Drop Default

Replace intrinsic-only gaps with demanded trait witnesses, exact manual/derived behavior and user Drop execution; cleanup ordering follows recorded language/edition policy.

### WT06-04 — Panic and cancellation cleanup

Exercise nested unwinding, destructor faults, cancellation and interrupted cleanup with durable continuation state and exact-once destruction.

### WT06-05 — Managed Box and dyn storage

Implement managed ownership and trait views with source type identity, object-safety constraints and checked dictionary dispatch; do not reuse native vtable layouts.

## First assignable slice

Choose a stock ownership/drop runtime failure beyond existing component coverage and reserve its exact storage or cleanup files; preserve inherited behavior and receipts.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

Stock programs plus LRR-specific transaction/lease/fuel/cancellation regressions prove no duplicate move/drop and no ownership change before failed reservation/admission.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Run validation in a separate frozen checkout/target asynchronously; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

WT03 patterns and WT07 containers depend on these transactions; trait-based policies/dyn views require WT05. Native resident storage is later H-task work.
