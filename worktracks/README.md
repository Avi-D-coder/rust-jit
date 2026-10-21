# Parallel worktracks

Contributors can bring their own time, model budget and agents to LRR. Pick a bounded slice of a worktrack, coordinate ownership, and deliver a reviewable PR with reproducible evidence. A track contains several substantial deliverables; one contributor need not finish the entire track. Human owners remain responsible for their agents' changes and claims.

The governing documents are the [revision-5 spec](../lrr_rust2024_macros_traits_implementation_spec.md), [AGENTS.md](../AGENTS.md), [task ledger](../docs/tasks.json) and [implementation state](../docs/implementation-state.md). Worktracks divide ownership and planning; they do not replace Section 57's fixed integration order or authorize architecture changes. Suggested paths include planned modules and shared integration hotspots; they are not blanket exclusive locks.

## Choose work

| Track | Substantial work stream |
|---|---|
| [WT01](01-upstream-contracts/README.md) | Complete upstream inventory, contract classification and execution drivers |
| [WT02](02-numeric-semantics/README.md) | Numeric source constraints, primitive operations and pure constants |
| [WT03](03-control-flow-patterns/README.md) | Control flow, pattern transactions and edition-sensitive scopes |
| [WT04](04-calls-source-views/README.md) | Calls, generics, source views and required callable forms |
| [WT05](05-trait-goal-engine/README.md) | Trait catalogs, tabled goals, projections and dispatch |
| [WT06](06-storage-ownership/README.md) | Aggregates, places, ownership, user destruction and managed storage |
| [WT07](07-library-runtime/README.md) | Collections, strings, formatting, conversions and iterators |
| [WT08](08-macros-editions/README.md) | Hygienic macros, generated declarations and every official edition |
| [WT09](09-cargo-preparation/README.md) | Exact Cargo units, tool preparation and native-world provenance |
| [WT10](10-query-cache-invalidation/README.md) | Pure queries, atomic derived facts, invalidation and portable caches |
| [WT11](11-authority-workers-service/README.md) | Capabilities, workers, service protocols and exact-once effects |
| [WT12](12-integration-release/README.md) | Contribution coordination, acceptance, reproduction and release evidence |

[registry.json](registry.json) records stable track/item IDs, coordination dependencies and item states. [claims.json](claims.json) records narrow active reservations, including current local work. [history.jsonl](history.jsonl) is an append-only event log. A track marked `open` can contain reserved items: check both the item and claims before allocating agents. Initial `planned` items require prerequisite review; they are not all implementation-ready.

## Claim and implement a slice

1. Read the spec sections and chosen track. Inspect current source, task state and active claims. Select one item or a bounded part of it, a T/H/M card and an unchanged upstream witness. Record the invariant and exact acceptance scope before editing.
2. Open the **Worktrack claim** issue using [the template](../.github/ISSUE_TEMPLATE/worktrack.yml). State exact files/symbols, deliverables, prerequisites, owning platform/editions, agent allocation and a review date. Reference existing related work. Until GitHub is configured, record a local claim with null issue/PR fields; never invent links.
3. A maintainer acknowledges the scope and records the reservation. Other contributors can prepare nonoverlapping inventories/reviews meanwhile. If two claims overlap, agree on the interface or split the files/symbols before concurrent implementation; an issue comment alone is not an exclusive lock. A review date triggers coordination, not silent reassignment of active work.
4. Work in your own fork/branch or isolated checkout, normally named `codex/wtNN-short-slice`. Preserve inherited dirty work. Give each agent a different code slice, the invariant, upstream fixtures and handoff requirements; identify a human owner. Reserve shared interfaces before editing them.
5. Implement a coherent checkpoint. Use official rustc tests incrementally with all helpers/directives/revisions/expected files, original MIT OR Apache-2.0 license texts, commit and hashes. Keep originals unchanged in `fixtures/upstream/rustc-checkout`; adapters and LRR-specific assertions belong outside it. Missing closure/async/trait/library support remains required runtime backlog.
6. Validate asynchronously in a frozen source checkout and an owned target. Keep that tested source unchanged while doing independent work elsewhere. Record exact commands, source/build identities, macOS/toolchain/edition/configuration, stdout/stderr and terminal status. Collect results before claiming a pass. Failed attempts and unsupported/unexecuted contracts stay visible.
7. Open a draft or ready PR using [the PR template](../.github/pull_request_template.md). Include dependency PRs, evidence and remaining gaps. If a prerequisite is not merged, use a documented stacked PR with a stated base or leave the candidate blocked; do not mark it accepted. Avoid unrelated formatting/refactors and fixture weakening.

A useful agent handoff includes the baseline commit plus dirty-source fingerprint, changed files/symbols, exact task/invariant, upstream provenance, patch or commits, acceptance commands/actual results, unresolved blockers and next slice. Source-only candidates explicitly say `unexecuted`. Tokens spent and files imported measure effort, not compatibility.

## Runtime and compatibility gates

Classify each **asserted contract**, not its directory. The four reports are `upstream_runtime_acceptance`, `diagnostic_compatibility`, `compiler_tooling_compatibility` and `full_suite_compatibility`.

Guest JIT requires every applicable upstream runtime contract through LRR, including run-pass, expected runtime failures and runtime parts of mixed/run-make cases, plus applicable ownership, cleanup, fuel/resource, cancellation, capability and exact-once gates. Required auxiliaries, macro preparation, owning editions/configurations, effects/output and full execution are part of the contract. Unsupported runtime features, missing drivers/dependencies, unexplained skips or unknown classification that could hide runtime work keep this gate closed.

Compile-time acceptance/rejection, exact diagnostics/static borrow checking, MIR/IR/assembly/optimization assertions, rustdoc and other tooling have separate gates. In a mixed fixture, a complete runtime-contract pass can coexist with outstanding diagnostic/tooling assertions; it is not a full-fixture pass. Correct execution of an empty main cannot satisfy an upstream E0308 expectation. External rustc evidence is separately labeled and never counts as interpreter execution. Keep the complete pinned inventory and reconcile all outcomes.

macOS ARM64 with Rust 1.99.0 is primary. Rust 2015/2018/2021/2024 ownership is retained; configurations come from upstream directives and the pinned harness matrix. Linux is a later port and Docker is supplementary. Actual upstream platform deferrals allow macOS progress without implying Linux support. An LRR implementation gap is never a platform exemption.

## Interpreter acceptance: actual upstream programs

User direction, October20: every language contribution must be connected to the same production parser, source tree, preparation services and interpreter, and its acceptance must execute unchanged official rustc programs through that interpreter. Separate file ownership helps contributors edit safely; it does not authorize separate engines, mocked execution, alternate fixture routes or isolated feature acceptance. Source-adapter, metadata and unit checks may diagnose a failure or check an LRR-specific invariant, but do not establish language support, interpreter progress or a stock pass. An uncalled production API remains an unfinished draft.

Start from a real upstream failure and record its actual interpreter terminal. Implement the common mechanism on its execution path, then rerun that original and relevant real upstream regressions asynchronously in the shared checkout with one Cargo owner and unchanged tested sources. Preserve original directives/auxiliaries and observe main, assertions, effects and cleanup. Report actual runtime passes, failures and unexecuted cases; never replace them with component counts. Keep source-only work explicitly held until its executing consumer and actual upstream acceptance exist.

## Review, merge and refresh

Maintainers review source identity, lazy semantics, capability boundaries, deterministic/checked encodings, upstream preservation and actual evidence. Require the current scoped acceptance and the integrated milestone appropriate to Section 57. A merge and a component test pass are separate from acceptance of a full card or track. Guest tier implementation stays parked until the runtime/correctness prerequisites actually pass.

For every handoff/PR/merge, update the relevant item and claim, actual issue/PR link, remaining blockers and evidence. After integration, record the merge commit and integrated gate in history, update `docs/tasks.json` and current implementation state only to the established scope, and then allocate the next disjoint slice. Do not erase failed or superseded history. Avoid committing enormous build targets or machine-specific caches; retain reproducible commands and checked evidence manifests, with large raw evidence placed in a documented retrievable artifact location.

Refresh plans **after each merge** and **weekly while contributions are active**. Reconcile claims against live issues/PRs, review stale reservations with their owners, identify the next ready slices, check task/API dependencies, and update the baseline when the spec, ledger or upstream pin changes. These are maintainer procedures; creating this folder installs no scheduler and posts nothing to GitHub.

Run the structural check before submitting plan changes:

```sh
python3 worktracks/check.py
```

It validates registry/claim/history integrity and local Markdown links. It runs no compiler, guest or runtime tests and cannot open any acceptance gate.

## Recording states

Item/track states: `planned`, `open`, `claimed`, `in_progress`, `blocked`, `in_review`, `merged`, `accepted`, `superseded`. Use `blocked` with a concrete dependency/decision; use `merged` with an actual PR/merge commit; use `accepted` only with actual scoped evidence. Whole-track acceptance requires every deliverable and its ledger dependencies, not one merged slice.

Claim states: `active`, `blocked`, `in_review`, `released`, `merged`. Claim IDs remain stable; keep released/merged claims for provenance. Use `kind=local_agent` for unpublished current work, or `kind=contributor` for public human-owned work. For public contributions, record real issue/PR URLs as they become available. The same item can be split across owners only with disjoint documented scopes and API handoff.

History rows identify their event/time, affected tracks, real issue/PR links when available, evidence and explanatory scope. Do not invent a merge or acceptance event. The baseline is a dated snapshot, not automatically current evidence. [GitHub announcement draft](announcement.md) is ready for later publication; the implementation branch is pushed to [Avi-D-coder/rust-jit](https://github.com/Avi-D-coder/rust-jit). Publishing the announcement and accepting external pull requests remain separate actions.

Latest held M019/M020 query-fact/elaboration checkpoint: [source review](05-trait-goal-engine/2026-10-20-query-facts-source-review.md) and [V520/V521 evidence](evidence/2026-10-20-v520-v521-impl-elaboration.json). One real compiler elaboration control passes; repr128 runtime and root activation remain held. No new runtime gain or gate completion.

Latest held M020 common clause-emission checkpoint: [review](05-trait-goal-engine/2026-10-20-predicate-emission-review.md), [durable V522/V523 evidence](evidence/2026-10-20-v522-v523-predicate-emission.json). Authentic PointeeSized clauses are transient while ordinary user Send remains emitted; 60 source/control worlds passed. Raw outputs were removed after validation; preserved observations remain distinct from currently available raw logs. repr128/root/runtime acceptance remain held; zero new stock gain.
