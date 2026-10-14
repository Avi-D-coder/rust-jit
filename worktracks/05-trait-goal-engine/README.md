# WT05 — Trait headers, goals and dispatch

Build the tabled demand-driven trait engine and source-aware dispatch required by ordinary Rust runtime contracts.

## Current boundary

Owned symbolic terms and transaction foundations exist; full header indexing, goal solving, projections and trait dispatch remain incomplete.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `M019`, `M020`, `M021`, `M022`, `M023`, `M024`, `M027`, `M028`, `M030`, `M034`.
- Suggested code areas: `crates/lrr/src/traits/`, `crates/lrr/src/frontend/member.rs`, `compat/traits.toml`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: WT04.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT05-01 — Header catalogs

Lower generic/conditional/blanket impls, trait members, supertraits and associated headers without ordinary body inference; compare indexes to complete catalogs and generated additions.

### WT05-02 — Canonical goals and transactions

Canonicalize exact TraitRef arguments and isolate candidate substitutions. Failed candidates roll back; NoSolution, Ambiguous, cycles and resource limits remain distinct.

### WT05-03 — Tabled conditional proofs

Evaluate only needed predicates, establish ordinary versus approved auto-trait recursion policy, and prohibit assumptions proving themselves after concrete substitution.

### WT05-04 — Projection and binder semantics

Normalize associated types/constants, GATs, higher-ranked bounds, region facts and const-generic/opaque contracts with precise indeterminate and cycle outcomes.

### WT05-05 — Dispatch integration

Connect ordered inherent/trait receiver search, UFCS, defaults, supertraits, operator/Index/Deref paths and invalidation to WT04 calls and WT08 generated headers.

## First assignable slice

Prepare a bounded M019 header-lowering/index slice against the current M017/M018 interface; runtime integration waits for that SourceView dependency.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

TR006–TR018-style catalog/proof/rollback cases and unchanged stock trait runtime fixtures distinguish real selection from same-name/runtime-tag lookup; no unrelated cold body is checked.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Run validation in a separate frozen checkout/target asynchronously; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

Uses WT04 symbolic SourceViews, WT08 sealed generated declarations and WT10 negative dependencies. WT07 consumes standard traits; no alternate solver framework.
