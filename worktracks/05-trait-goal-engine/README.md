# WT05 — Trait headers, goals and dispatch

Integrate the pinned next-generation rustc trait solver with LRR's demand-driven source queries, dictionaries and ordinary runtime dispatch. [E-R5-014](../../docs/errata/revision5-upstream-trait-solver.md) supersedes the former custom-solver plan.

## Current boundary

Latest held M020 component: V346 replaces the old region representation with current compiler constraint kinds/mutators and explicit source spans/visibility. Both builds and3source-context controls pass with guards. Graph/leak/inference/delegate/root/runtime migration remain incomplete; zero new stock runtime gains. [Evidence](../evidence/2026-10-19-v345-v346-upstream-region-collector.json).

Prior held M020 component: V344 adds exact compiler region-formula storage/undo to the existing context and reuses compiler forall through the shared binder folder. Both builds and3source-context controls pass with guards. Full inference/delegate/root and runtime migration remain open; zero new stock runtime gains. [Evidence](../evidence/2026-10-19-v344-upstream-solver-regions.json).

Prior held M020 component: V343 removes the stable assumptions relation panic by reusing the pinned compiler helper. Both builds,18unchanged helper tests and5source-context controls pass with exact guards. Actual map/data access is available; full inference/delegate/root and runtime migration remain open. Zero new stock runtime gains. [Evidence](../evidence/2026-10-19-v342-v343-upstream-region-assumptions.json).

The pinned upstream next-generation solver builds and links; its generic Interner/type family, pinned client ports and concrete immutable source-fact reader compile. It is not yet invoked by interpreter goals. The real source-fact producer, compatible inference/delegate and runtime dispatch migration remain incomplete.

Current source subset: the configured catalog/membership and actual Type/generic metadata now support real guarded upstream TraitRef construction. V327 builds and12/12controls pass:140actual heads/136TraitRefs across48original/user-macro/wrapper edition worlds, including distinct trait arguments and user Self/[Self] defaults. A genuine V326 guard failure led to common source-default completion; no constructor check is bypassed. Existing selected runtime regressions pass, with zero new distinct stock gain. Actual complete typed facts, inference/delegate, transitive currentness/closure and ReadyProvider/root evaluation remain unimplemented. [Latest evidence](../evidence/2026-10-19-v324-v327-source-generic-construction.json).

These components do not complete M019/M020. Arbitrary generated/local parent/import/alias/replacing/procedural/associated contexts and the complete origin matrix remain open. Prior structural-occurrence request setup failures and the separately parent-reproduced module import classification failure are not claimed repaired. Next implement the complete actual typed fact/ParamEnv/predicate/layout/lang-item producer and transitive dependency closure, then compatible upstream inference/delegate entry and ordinary runtime selection. Historical custom canonical/table candidates remain held until their consumers migrate and must not be promoted as the new solver.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `M019`, `M020`, `M021`, `M022`, `M023`, `M024`, `M027`, `M028`, `M030`, `M034`.
- Suggested code areas: `crates/lrr/src/traits/`, `crates/lrr/src/frontend/member.rs`, `compat/traits.toml`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: WT04.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT05-01 — Header catalogs

Lower generic/conditional/blanket impls, trait members, supertraits and associated headers without ordinary body inference; compare indexes to complete catalogs and generated additions.

### WT05-02 — Pinned upstream solver bridge and transactions

First build `rustc_next_trait_solver`/`rustc_type_ir` from the exact pinned Rust revision on macOS Rust1.99.0. Bridge actual Types, binders, ParamEnv and source/header facts into upstream interfaces and map canonical responses/selection evidence back to LRR. Reuse upstream inference transactions; failed candidates must not leak bindings. Record dependency/license/build-patch provenance and retire superseded custom semantic consumers after acceptance.

### WT05-03 — Upstream conditional selection and search graph

Feed only demanded goals and their required transitive predicates to the shared solver. Reuse its conditional selection, cycle/fixpoint and ambiguity behavior; do not implement another SCC/worklist solver. Connect NeedInput and relevant cache invalidation without repeating completed guest effects. Only actual pinned-rustc limits are permitted; no LRR table/work/byte budgets.

### WT05-04 — Projection and binder semantics

Bridge upstream normalization, GATs, binders/universes and demanded region facts while preserving source identity. Reached associated constant values use the ordinary VM under E-R5-007. Keep unsupported, ambiguous and incomplete outcomes distinct; do not add static borrow checking or a separate projection solver.

### WT05-05 — Dispatch integration

Connect ordered inherent/trait receiver search, UFCS, defaults, supertraits, operator/Index/Deref paths and invalidation to WT04 calls and WT08 generated headers.

## First assignable slice

Continue required M019 header/catalog prerequisites against M017/M018, then establish M020's pinned upstream build and source/inference bridge. Read [the migration contract](../../docs/errata/revision5-upstream-trait-solver.md) before assigning work. No new custom solver implementation is assignable.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

TR006–TR018-style catalog/proof/rollback cases and unchanged stock trait runtime fixtures distinguish real selection from same-name/runtime-tag lookup; no unrelated cold body is checked.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Build/test directly in this checkout asynchronously with one Cargo owner, freezing tested inputs while their users run; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

Uses WT04 symbolic SourceViews, WT08 sealed generated declarations and WT10 negative dependencies. WT07 consumes standard traits. The chosen solver is the pinned upstream next-generation solver; the LRR custom query framework and ordinary VM remain unchanged. No new runtime compatibility or performance evidence follows from this plan revision.
