# WT09 — Cargo units, preparation and native-world provenance

Make exact Cargo compilation-unit identities and authorized compile-time preparation sufficient for runtime fixture dependencies; stage later native provenance in ledger order.

## Current boundary

Pinned tool/planner/server component probes exist; complete selected-unit and native-world integration remain pending.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `M001`, `M002`, `M003`, `M038`, `M039`, `M040`, `T090`, `T091`, `H005`, `H006`, `H007`, `H008`, `H009`, `H010`, `H011`, `H012`, `H013`.
- Suggested code areas: `crates/lrr/src/cargo/`, `crates/lrr/src/cli/compile_inputs.rs`, `tools/cargo-planner.toml`, `tools/probe_compile_time_deps.py`, `tools/prepare_macro_server.py`, `vendor/UPSTREAM.toml`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: none at whole-track level; each slice still has its ledger/API prerequisites.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT09-01 — Exact unit graph

Preserve package/target/profile/features/test intent/host-target distinctions and edition/configuration identities; reject package-level unions and stale plans.

### WT09-02 — Compile-time dependency preparation

Build only authorized macro/build-host dependencies with the pinned planner, capture argv/messages/env/OUT_DIR and demonstrate no selected live runtime unit compilation.

### WT09-03 — Dependency fixture closure

Prepare required auxiliary crates and macro tools for stock runtime contracts with frozen hashes/receipts and separate grants; classify missing preparation as a required blocker.

### WT09-04 — Freshness and generated captures

Detect changed build inputs, generated files, server/compiler tuples and macro facets; no watch/live execution automatically invokes tools.

### WT09-05 — Later native generations

At the applicable H/M gate, implement native/live/build/macro world partition, immutable generation/ABI description and matched producer metadata; no load/call success before actual acceptance.

## First assignable slice

Start with an exact auxiliary/macro unit preparation gap exposed by WT01. Native ABI export/runtime integration remains dependency-gated and cannot become a bypass for interpreter fixtures.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

Actual argv and content receipts show exact authorized units and matched server/compiler identities; races/stale/mismatched worlds fail before publication or execution.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Run validation in a separate frozen checkout/target asynchronously; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

WT08 consumes prepared macro inputs; WT11 enforces authorities. H-task native runtime/optimization work follows Section 57 and never starts guest JIT early.
