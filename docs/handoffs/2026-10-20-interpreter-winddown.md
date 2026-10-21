# Interpreter handoff — October 20, 2026 winddown

> **Superseded for current state** by [the October 20/21 round-2 handoff](2026-10-21-interpreter-round2.md): repr128.rs and union-generic-rpass.rs listed below as failures now pass (V540). This document is kept as the historical V537 record.

The user requested winddown after the current round. All implementation and validation workers have stopped; no new repair was begun after this request. Continue from this checkout, preserving inherited work. The project goal is incomplete and implementation is paused for handoff, not completed.

Repository: `/Users/avi/Code/rust-jit`, branch `codex/lrr-implementation`, remote `https://github.com/Avi-D-coder/rust-jit.git`. Read current `AGENTS.md` and revision 5 specification first. Earlier handoffs and checkpoint banners are historical. This document describes the outgoing shared source tree. The source checkpoint containing it can be identified with `git log -1`; the validated baseline HEAD before checkpointing was `08c177d83dda20a283e06709c92cfffc19eee6be` with the changes now checkpointed.

## Actual acceptance, not component counts

V537 built the library (`--no-run`), production CLI and source-digest helper successfully on macOS ARM64 with Rust1.99.0. It executed eight unchanged licensed rustc originals through the production CLI/VM: **six runtime-contract passes, two failures**, one newly passing original compared with V534/V536. All source/HEAD/binary closing guards passed. No isolated tests were selected or executed. This is not a refreshed comprehensive suite count, full-fixture acceptance, a completed language feature or an interpreter/JIT milestone.

Exact tested source fingerprint: `4aad77b936cadd40051d71242d82681e9604a428e72360686551d4d081e6cec9`, 8751 source inputs. CLI SHA256 `43834cd150cf627e45857eda06556f65d37abaa61694c3791ad15d95939aef50`. Digest helper SHA256 `355c98a8de6671224cf264f780cd516aad7e85407ea267609a982851eb0e2e4d`. All implementation/tool/fixture inputs stayed unchanged through the run. Later winddown edits affect documentation/planning only.

| Unchanged upstream original | V537 interpreter result |
| --- | --- |
| tests/ui/type-alias/static-method-type-alias-11047.rs | **New pass**, full main including `Ham::new()` and `StringVec::new()`, 22 dispatches/22 outcomes, 3 source calls, 13/13 capture checks, cleanup true |
| tests/ui/enum-discriminant/discriminant_size.rs | Retained pass, 4684 dispatches, 108 source calls |
| tests/ui/enum-discriminant/discriminant_value.rs | Retained pass, 4537 dispatches, 46 source calls |
| tests/ui/associated-types/assoc-type-in-struct-constructor.rs | Retained pass, 15 dispatches, 2 source calls |
| tests/ui/generics/generic-derived-type.rs | Retained pass, 199 dispatches, 5 source calls; stdout `10\n10\n` |
| tests/ui/structs-enums/struct-aliases.rs | Retained pass, 961 dispatches, 4 source calls |
| tests/ui/enum-discriminant/repr128.rs | Failure, 7 dispatches/7 outcomes/0 source calls, before solver execution: callback preflight forces unrelated Copy requirements |
| tests/ui/union/union-generic-rpass.rs | Failure, 265 dispatches/265 outcomes/16 source calls: nested explicit `Iter<_>` inference hole |

Durable complete commands, checks, streams, source fingerprints, diagnostics and cleanup: [V537](../../worktracks/evidence/2026-10-20-v537-interpreter-runtime.json). Keep [V535 build failure](../../worktracks/evidence/2026-10-20-v535-interpreter-build.json) and [V536 five-pass/regressed-alias frontier](../../worktracks/evidence/2026-10-20-v536-interpreter-runtime.json); do not erase failed attempts. Imported inventory is 2820 cases/3500 files, not passing tests. Full-suite, diagnostic and tooling compatibility remain separately unassessed; interpreter and guest-JIT gates remain incomplete.

## Connected changes in this checkpoint

- `engine/run.rs` / `run_call_inputs.rs`: ordinary CallSeal obtains the existing actual result expectation before numeric fallback, retains evaluated argument/source/caller/callee/attempt inputs across genuine NeedInput, calls the pinned source-root entry, then applies owned original numeric answers through the ordinary store/contract/frame path. Successful answers retain actual query guards and postbinding cell snapshots through retries, prepared calls, entry and return. Unresolved numeric answers hold because full future-use constraint closure is unfinished. Missing coordinator integration is explicit, not a silent alternative numeric evaluator.
- `traits/upstream/reached_call.rs`: production six-argument source-root entry over actual SourceCallable/GenericEnv and source facts. It uses original upstream variables, relations and fulfillment; private owner lineage guards fast/stalled paths, canonical descendants and cache access. Responses preserve real source/query guards; overflow retains pending obligations. **repr128 still stops at preflight, so this run does not establish a successful upstream root invocation.**
- `cli/mod.rs` / `proc_macros.rs`: retain a dormant authentic ImplCatalog demand, service only the matching reached Discover request through existing compiler/module preparation and publication, then repoll the same interpreter without replaying arguments. No new source snapshot or second interpreter.
- `configured_catalog.rs`, `db/local_declaration_inputs.rs`, `cold_declaration_name_inputs.rs`: private actual catalog receipt allows staged immutable metadata additions or exact retained rows during a run. Conflicting active name publications remain refused; old cold guards are preserved. No body/source/runtime-environment replacement.
- `frontend/entered_call_goals.rs`, `explicit_type_arguments.rs`, `db/entered_call_thunk.rs`, `engine/run_owner_arguments.rs`: actual Q51/Q73 associated qualifier and source-selected callable feed the same Q44/Q47 call lowering. Closed full alias owner arguments retain source schemas, including String. Uses existing hygienic written-callee spelling. Explicit trait qualification cannot be overridden by inherent selection. Existing owner matcher is unchanged migration debt, not a new solver.
- `frontend/source_predicates.rs`: authentic `[const] Trait`/`~const Trait` Maybe forms emit the ordinary Trait clause through the shared lowerer, as pinned rustc does; separate conditional-effect queries are not fabricated. Always const/async/use/negative/HRTB unsupported domains retain their holds.
- `runtime/cell.rs::bind_numeric_literals`: preflight entire original-cell/descriptor batch and alias consistency, then commit literal bindings with no allocation/recoverable failure after the first write. No new inference algorithm.

Architecture/source reviews and exact hashes are in:

- `worktracks/04-calls-source-views/2026-10-20-static-alias-call-production.md`
- `worktracks/05-trait-goal-engine/2026-10-20-reached-response-consumer.md`
- `worktracks/05-trait-goal-engine/2026-10-20-reached-root-executor.md`
- `worktracks/05-trait-goal-engine/2026-10-20-reached-catalog-publication.md`
- `worktracks/05-trait-goal-engine/2026-10-20-union-runtime-inference.md`

These reviews do not close the full architecture matrix or remove all old alternate semantic routes.

## Next actual failures to fix

1. **repr128 source callback closure.** `reached_call.rs` currently promotes child traits from all 66 PartialEq implementation clauses before upstream relevance selection. The actual failure is Copy `d1cbccf973e385bbcbe0f6699b377439493c7a649e78ad2b788591fd9c2fe39e`, source span 1110..1134, unsupported builtin callback plus actual `ParentIdentity` producer hold. Do not fix this by extending a fixture-specific builtin whitelist or treating unknown facts as empty/NoSolution. Existing `prepared.rs::CandidateIndex::prepare` and `candidates.rs::for_each_relevant_source_impl` already implement the pinned simplified-group/blanket traversal. The selected PartialEq Self is initially a projection, so authentic pinned normalization must precede relevant-group dependency closure. The worker identified `rustc_next_trait_solver::normalize::NormalizationFolder::new` and pinned `rustc_trait_selection/src/solve/normalize.rs` as the existing client pattern. This is a proposal, not implemented or validated. Preserve the same owner/original variables, real ProjectionClause/caller ParamEnv, errors/Maybe/residual/external constraints and immutable fact/layout/currentness boundaries. Never run nested roots inside SearchGraph or create an LRR projection/candidate solver.
2. **Generic union constructor.** The unchanged program's `MaybeItem::<std::slice::Iter<_>>` is rejected by `frontend/explicit_type_arguments.rs::select_argument` before its field executes. A whole-argument None or guessed type is wrong. Retain nested source-owned hole identity/full application, use actual ADT field types from PreparedAdtSourceFields and the ordinary constructor continuation/ownership path, and relate actual `ManuallyDrop<&u8>` with `ManuallyDrop<<Iter<?T> as Iterator>::Item>` through pinned inference. Current root accepts genuine function calls only; do not fake a constructor as a function call. Real local-region inputs and full source-use constraint joins are missing prerequisites. Existing pending_nominal_args/binding_constraint_nominal projection heuristics must not grow.

Also open: complete future-use constraints before fallback; generic caller application beyond the bounded actual producer; other CLI/test entry services requiring the same coordinator join; owner-segment generic spelling; conditional effects; full origin/edition/suspension/currentness matrices and obsolete-route removal. The existing delegation explicit-paths stock regression is recorded, but its delegation parser prerequisites are unsupported and it has not run here. Do not present it as exercised coverage.

## Resume workflow

Follow current AGENTS/worktrack ownership before editing. LOCAL-049 is released for this human handoff; M019/M020 and WT05-02 remain in progress. Other inherited reservations must be inspected, not silently erased. All agents work in this one checkout/source tree with disjoint files. No isolated mock implementation, worktree, frozen source copy or private target. macOS first, all official editions; Docker/Linux later.

Use unchanged official tests as the driver. Do not replace whole-main execution with helper evaluations. Use the next-generation rustc solver/type IR from exact upstream `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`; no independent solver, static/borrow checker, macro-specific language logic, resource budgets or sandbox work. Ordinary reached execution is eager; parsing/type preparation is demand-driven. Guest JIT waits for complete applicable runtime/interpreter correctness gates.

Reproduction inputs/scripts are in Git. `artifacts/` holds disposable outputs only; V537 raw 19 files/625241 bytes are optional at `artifacts/validation-current-v537/runtime-observations`. No unique source or private Cargo target is there. The ordinary shared `target/` is a rebuildable cache. Do not run cargo clean as a routine first step. Existing official rustc checkout can be reprovisioned from the tracked pin; preserve it.

Run all builds/tests asynchronously under one Cargo owner. Keep the tested source/tools/fixtures/HEAD unchanged until all terminal and closing guards finish, while other agents reason/review or edit documentation. See `tools/validation/README.md`; Python3.11+ is required (this host uses `/opt/homebrew/bin/python3.12`). Use a fresh output/report directory. The exact V537 argv are in its capsule. The tracked selection is `worktracks/05-trait-goal-engine/2026-10-20-interpreter-runtime-selection.json` (eight cases, controls empty).

```sh
cargo +1.99.0 test --offline --locked -p lrr --lib --no-run --message-format=json
cargo +1.99.0 build --offline --locked -p lrr --bin lrr
cargo +1.99.0 build --offline --locked -p xtask --example lrr_source_digests
```

After sequential build terminals and source/binary guards, run the maintained `tools/rustc_checkout_runtime.py` with `LRR_EMBEDDED_FACADE=1`, explicit CLI/digester paths, a fresh report directory and repeated `--case` for all eight original paths above. Use `tools/validation/source_manifest.py` before/after-build/after-runtime as the capsule records. Harness exit1 means failing cases exist, not that six passes disappeared. After recording durable compact evidence, remove reproducible temporary projects/private targets. Source-only periodic unsigned checkpoints/pushes to the implementation branch are authorized; run `python3 worktracks/check.py`, `git diff --check` and staged `python3 tools/check_source_checkpoint.py`. Do not fetch/apply/merge external PR code without approval. A pushed WIP is not integration or release acceptance.
