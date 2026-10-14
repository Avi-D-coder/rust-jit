# WT07 — Standard library facade and runtime support

Implement the source-resolved library operations, output and iteration needed to execute the upstream runtime suite.

## Current boundary

Builtin macro/prelude hooks exist; required collections, general formatting, conversion and iterator behavior remain incomplete.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `T060`, `T061`, `T062`, `M031`, `M033`.
- Suggested code areas: `crates/lrr/src/facade/`, `crates/lrr/src/runtime/object.rs`, `crates/lrr/src/runtime/reference.rs`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: WT05, WT06.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT07-01 — Option Result and scalar APIs

Implement exact facade identities and standard conversions/methods through ordinary source-aware witnesses, including question-mark dependencies.

### WT07-02 — Vec slices and arrays

Complete construction, indexing, push/pop, get/get_mut, slicing and consuming iteration. Track reallocation and element-specific invalidation; reserve before effects.

### WT07-03 — String and output

Implement UTF-8-aware String operations, formatting and stdout/stderr capture through the broker; preserve actual output ordering and attributed failures.

### WT07-04 — Iterators and conversions

Support demanded iterator/adaptor chains and edition-sensitive IntoIterator behavior for arrays/boxed slices, using correct source trait/member identities.

### WT07-05 — Remaining runtime library backlog

Use stock failures to map required standard operations beyond current inventory. Keep each missing runtime operation a blocker and seek explicit spec additions where necessary.

## First assignable slice

Build a fixture-to-facade gap map and one narrow Option/Result runtime slice; integration waits for the required trait/storage APIs rather than adding special-case test outputs.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

Original selected programs execute their real operations and output; storage invalidation, remaining-element destruction and callback exact-once behavior are exercised.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Run validation in a separate frozen checkout/target asynchronously; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

WT05 owns standard trait headers/dictionaries, WT06 owns storage and WT11 owns effects/output authorization. No arbitrary std/native symbol loading.
