# WT03 — Control flow and transactional patterns

Execute Rust control-flow and pattern contracts with lazy arm preparation and transactional ownership.

## Current boundary

Basic blocks/loops and condition scopes have component evidence; general match/pattern flow is incomplete. The current literal-match candidate is unexecuted.

This is an ownership/work-planning track, not a new task order or a feature-completion claim. Follow revision 5 Section 57 and the [contribution workflow](../README.md).

## Task and code ownership

- Ledger cards: `T030`, `T031`, `T050`, `T051`, `H002`, `H003`.
- Suggested code areas: `crates/lrr/src/syntax/body_index.rs`, `crates/lrr/src/syntax/conditions.rs`, `crates/lrr/src/syntax/temporary_extensions.rs`, `crates/lrr/src/vm/pattern.rs`, `crates/lrr/src/vm/control.rs`. Missing modules listed here are planned, not existing implementations.
- Coordination dependencies: none at whole-track level; each slice still has its ledger/API prerequisites.

Broad path lists are coordination hints, not exclusive reservations. Claim exact files/symbols for one slice; shared `syntax/thunks.rs`, `db/memo.rs`, `engine/run.rs`, `service/` and facade/storage glue require interface agreement.

## Substantial deliverables

### WT03-01 — Literal match and alternatives

Integrate the bounded literal/bool match candidate after acceptance, then support permitted alternative patterns while evaluating the scrutinee once and preparing only selected arm semantics.

### WT03-02 — Transactional structural patterns

Implement pending bindings for tuple/struct/enum/slice patterns, binding modes, conflicting moves and rollback. Failed arms preserve the scrutinee; commit moves once.

### WT03-03 — Guards and conditional forms

Implement match guards, if-let/while-let, let-else and let chains with correct scopes, alternative retry rules and cold branches. Source editions govern ergonomics and lifetime behavior.

### WT03-04 — Loop transfers and cleanup

Cover labeled break/continue, loop results, nested transfers and temporary destruction without replaying operations or losing values across suspension.

### WT03-05 — Question mark flow

Connect Option/Result propagation and reached From conversion; success forces no conversion, failure invokes the selected conversion once and checks the enclosing return contract.

## First assignable slice

The literal-match/or-pattern successor is locally reserved; external slices should start with an independently scoped upstream pattern inventory or a different control-flow form.

Before implementation, record a named upstream runtime witness (or an LRR-specific correctness witness), the task/invariant, missing dependency contracts and the exact acceptance command plan. Planned slices are not automatically ready to integrate.

## Acceptance and handoff

Failed arms/guards leave ownership intact, selected bindings commit once, all scope/temporary exits run in owning-edition order, and stock runtime outputs plus effect traces agree.

Keep upstream originals unchanged, including helpers/directives/revisions/expected files and licenses. Put adapters and LRR assertions outside the upstream directory. Run validation in a separate frozen checkout/target asynchronously; retain commands, source hashes, platform/toolchain, terminal statuses, failures and execution scope. A prepared or partially exercised original is not a full fixture pass.

Update the claimed item, issue/PR links and evidence in the registry/history. A merged partial slice does not close its whole track or mark a T/H/M card complete.

## Cross-track coordination

WT06 provides move/lease/cleanup transactions; WT05 provides conversions. Ranges and other excluded forms require a recorded spec decision, not an implicit scope expansion.
