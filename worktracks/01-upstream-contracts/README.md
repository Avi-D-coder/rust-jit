# WT01 — Upstream inventory and contract execution

Turn the complete pinned rustc inventory into executable, separately accounted runtime, diagnostic and tooling contracts.

## Current boundary

Declared case counts and source/program component adapters exist; the complete configured runtime denominator and full-contract executor remain incomplete.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `T003`, `T034`, `T100`, `T104`, `M043`, `M047`.
- Suggested code areas: `tools/`, `xtask/`, `compat/`, `fixtures/upstream/`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: none at whole-track level; each slice still has its ledger/API prerequisites.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT01-01 — Contract classification

Classify every assertion using pinned driver/directive rules. Cover run-pass, expected runtime failures and runtime portions of mixed/run-make cases; preserve unknown classification as a runtime-gate blocker.

### WT01-02 — Configured case enumeration

Resolve revisions, owning editions, cfg, auxiliaries, selected outputs and actual host/tool matrix. Preserve all original files and record denominator changes instead of inventing an all-edition product.

### WT01-03 — Runtime execution drivers

Drive fresh interpreter processes for complete selected contracts, check exit/output/effects and expected panic semantics, and handle auxiliary preparation explicitly. Missing driver support remains unexecuted or contract_blocked.

### WT01-04 — Separate compatibility reports

Implement upstream_runtime_acceptance, diagnostic_compatibility, compiler_tooling_compatibility and full_suite_compatibility. Runtime-only mixed success is not full_pass; external compiler evidence is labeled separately.

### WT01-05 — Inventory completeness and reduction

Reconcile all categories, support files and configurations; minimize genuine runtime failures while retaining originals and trace the resulting required backlog to owning tracks.

## First assignable slice

The stock run-pass contract runner is locally reserved. A contributor can first take a nonoverlapping category classifier after its schema is agreed.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

One unchanged upstream contract executes fully through LRR in its declared configuration with every required assertion checked; negative metadata controls reject hidden runtime obligations and false full passes.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Build and validate directly in the shared checkout with one Cargo owner; run unchanged official rustc programs asynchronously on frozen tested sources; component or mocked checks cannot establish interpreter acceptance; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

Runtime requirements feed every feature track. Compiler-only work has separate gates and cannot reopen whole-program checking as a live runtime requirement.
