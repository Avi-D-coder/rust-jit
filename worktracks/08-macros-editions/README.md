# WT08 — Macros, generated source and all editions

Complete hygienic expansion and generated-source discovery across every official edition while keeping macro authority separate from live execution.

## Current boundary

Many expansion/discovery cards have historical component evidence; nested producers, generated trait integration and full edition/runtime contracts still need revision-5 acceptance.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `H001`, `H002`, `H003`, `M004`, `M005`, `M006`, `M007`, `M010`, `M011`, `M012`, `M013`, `M014`, `M015`, `M016`, `T063`.
- Suggested code areas: `crates/lrr/src/expand/`, `crates/lrr/src/syntax/entered_macros.rs`, `crates/lrr/src/service/`, `crates/lrr/src/source/generated.rs`, `crates/lrr/src/source/included.rs`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: none at whole-track level; each slice still has its ledger/API prerequisites.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT08-01 — Edition-sensitive expansion

Exercise 2015/2018/2021/2024 definition/invocation/dependency combinations, fragment matching, preludes, $crate, raw identifiers and hygiene through unchanged originals.

### WT08-02 — Nested producer discovery

Finish M014 nested impl producers and fixed-point discovery of generated imports/macros/traits/tests while leaving cold ordinary bodies unprepared.

The [common macro engine integration handoff](common-engine-integration.md) records the current E-R5-005 source/query/VM interfaces and outstanding shortcut removals. Syntax attachment or request/record preparation alone is not runtime integration.

### WT08-03 — All procedural forms

Integrate function-like, attribute and derive outputs through matched external servers, grammar/origin checks, helpers and generated test catalogs.

### WT08-04 — Includes cfg and environments

Bind generated/include paths, cfg/env and compiler-known macro values to the exact source unit/preparation and preserve origin-aware diagnostics.

### WT08-05 — Epoch revalidation and seals

Exercise unchanged/changed macro outputs, negative/ambiguous namespace invalidation, suspension/cancellation and deterministic bounded discovery; producers never execute inside pure queries.

## First assignable slice

Reserve a bounded M014 generated impl/header case after checking existing macro work and the WT05 catalog contract; do not overlap the active WT03 edition-sensitive pattern slice.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

MX source/runtime cases preserve token identity, owning editions, generated names and exact macro invocation counts; new epochs revalidate before reuse and cold guest bodies remain cold.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Run validation in a separate frozen checkout/target asynchronously; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

WT09 owns prepared tool artifacts; WT11 owns helper authority/supervision; WT05 consumes generated impls. Shared service/thunks edits require narrow reservations.
