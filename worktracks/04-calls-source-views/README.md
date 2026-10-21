# WT04 — Calls, generics and source views

Preserve exact source identity and generic context through reached call, argument, receiver and return boundaries.

## Current boundary

Basic reached generic calls and symbolic binder terms have scoped evidence; lexical nested admission, complete SourceView propagation and general callable forms remain incomplete.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `T032`, `T033`, `T052`, `T053`, `M017`, `M018`, `M025`, `M026`.
- Suggested code areas: `crates/lrr/src/frontend/callables.rs`, `crates/lrr/src/frontend/source_view.rs`, `crates/lrr/src/syntax/signatures.rs`, `crates/lrr/src/engine/run_argument_hints.rs`, `crates/lrr/src/engine/run_return.rs`, `crates/lrr/src/vm/frame.rs`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: none at whole-track level; each slice still has its ledger/API prerequisites.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT04-01 — Lexical source callable admission

Admit exact reached lexical declarations through hygienic body namespaces and bounded atomic derived facts. Retain currentness certificates; fetch no child body/thunk until entry.

### WT04-02 — Expression source views

Propagate symbolic and runtime views separately through aliases, reborrows, field projections, named returns and default Self. Same runtime descriptor must not erase rigid member identity.

### WT04-03 — Generic call packets

Complete source-selected argument/return contracts, const/lifetime substitutions, retained bound context and lazy dictionary requests; preserve ordered exact-once evaluation across NeedInput.

### WT04-04 — Callable values and closures

Implement named function values and the runtime closure requirements exposed by stock cases, including captures/ownership and call traits within the pinned architecture. Document missing spec contracts before adding new representation.

### WT04-05 — Async execution dependencies

Inventory and then implement required async/coroutine runtime call behavior in dependency order. Unsupported async remains a runtime blocker; do not invent an executor/representation without resolving missing spec contracts.

## First assignable slice

Lexical call syntax/engine and root frontend/database admission are separately reserved. Next independent slice is a bounded SourceView propagation fixture and implementation.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

The reached callee/member is selected by source identity, arguments/effects execute once, stale namespace/revision facts refuse, and cold bodies stay unprepared until entered.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Build and validate directly in the shared checkout with one Cargo owner; run unchanged official rustc programs asynchronously on frozen tested sources; component or mocked checks cannot establish interpreter acceptance; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

WT05 owns solver/dictionary selection; WT10 owns publication mechanics. Closures and async stay required runtime backlog even where the existing spec needs clarification.
