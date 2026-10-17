# LRR interpreter implementation handoff — October 16, 2026

## Handoff boundary

The user requested finishing the work already in flight, writing this handoff, and starting no new implementation work. The implementation goal is **not complete**. Resume only under the incoming agent's assignment. Do not interpret source checkpoints, imported tests, or component checks as interpreter compatibility acceptance.

Repository: `/Users/avi/Code/rust-jit`
Implementation branch: `codex/lrr-implementation`
Remote: <https://github.com/Avi-D-coder/rust-jit.git>

Read `AGENTS.md`, `lrr_rust2024_macros_traits_implementation_spec.md` revision 5, `docs/audits/macro-specific-semantics.json`, and `docs/errata/revision5-common-macro-engine.md` before editing. Historical app goal text mentioned revision 3, the deleted old spec, and Linux acceptance; **those portions are stale**. The goal tool returned no active goal during final closeout; no replacement goal was started. Current user instructions, revision 5, and the runtime-semantics gate govern.

Final closeout validation and checkpoint details are recorded below.

## Required direction

- One compositional interpreter: ordinary source, arbitrary user expansions, wrappers, and standard macro expansions must join the same source identity, parsing/indexing, demand-driven resolution, lowering/verifier, VM, ownership and cleanup mechanisms.
- Let unchanged official rustc tests choose the next repair. No assertion-specific evaluator, synthetic operand function, method whitelist, macro-specific inference, copied macro definition, expected-output shortcut, or fixture exclusion.
- Keep lazy parsing/type preparation and eager execution of reached Rust operations. Do not implement a borrow checker or full static checker. `cargo check` is separate. Do not prepare unselected executable arms or callee bodies merely to obtain signature facts.
- macOS aarch64 first, Rust **1.99.0**. Linux is a later port; Docker is supplementary. Support guest editions 2015, 2018, 2021 and 2024 with owning editions preserved.
- Guest JIT remains gated by **all applicable upstream runtime contracts plus interpreter correctness gates**. Diagnostic/static-checking, compiler-output/optimization, rustdoc/tooling and full-fixture compatibility have separate reports. Unsupported runtime features remain blockers.
- Read the eight architecture failure modes in `AGENTS.md`. Record bounded precode, actual entry points, removed/held alternatives, authority, costs and exact-source review. Existing violations are repair debt, not precedent.

## Source checkpoints already pushed

| Commit | Scope | Evidence limit |
| --- | --- | --- |
| `01b9f8a9eb66fa854a5028a851d37a3835da8ac2` | Common indexed Type/call retention and actual reached local body/readiness repair | 20 focused bodies passed on exact frozen source; one additional distinct unchanged nested-statement runtime witness, not a full-fixture pass |
| `91d560c10736bf4d9fca55105a8a1533ac56dcbb` | Genuine original-source lease control and unchanged `shift.rs` baseline | Lease control passed; shift main failed |
| `dd177a1e7a` | Shared pinned syntax seeds, completed per-source assembly transport, opaque target-body publication transition | 15 focused bodies passed; six earlier seed/transport bodies separately passed on identical production; no new distinct stock pass |
| `c675dcd8cf` | Pure body memo, actual standalone entry consumer, body-locator dependency, atomic publisher/observer call renewal | 22 scoped bodies passed; upstream CLI hygiene main still failed; no new distinct stock pass |
| `7337f1cd47` | Unchanged official nested-call test import and actual failing baseline | Compile passed; main preparation failed before guest execution |

These are source-only implementation-branch checkpoints, not merges or release/milestone acceptance. No PR code was fetched or applied. Inspect the final closeout checkpoint below for the last source changes.

## What exists and what is reused

The parser is **reused**, not written from scratch: `ra_ap_syntax = 0.0.356`. Macro machinery reuses the coherently pinned rust-analyzer `mbe`, `hir-expand`, parser, token trees, span/hygiene, syntax bridge and matched proc-macro API through `lrr-expand-adapter` and `vendor/rust-analyzer-macros`. The adapter uses pinned Salsa internally. Do not replace that frontend or mix incompatible syntax/interner stacks across its owned boundary.

LRR implements the source/discovery authority joins, demand/query services, symbolic source/type views, ordinary body indexing and LRIR lowering/verifying, tagged-value VM, continuations, cleanup and capability/native boundaries. Relevant paths:

- `crates/lrr/src/expand/source_discovery.rs` and its children: producer closure, source roles, seals and completed environments.
- `crates/lrr/src/expand/source_declarative/assembly*.rs`: attach actual issued outputs into retained ordinary syntax.
- `crates/lrr/src/db/`: pure memo/input/dependency and retention framework.
- `crates/lrr/src/frontend/`: ordinary source/member/Type/call constraints.
- `crates/lrr/src/syntax/`, `ir/`, `vm/`, `engine/`: ordinary body/index/recipe/execution path.

Cranelift dependencies and historical JIT code are present. They do not authorize starting guest JIT work ahead of the interpreter gate. Revision 5's later ABI/tiering requirements remain in force.

## Verified evidence: do not add these counts together as stock coverage

| Frozen source / local artifact | Actual outcome | Meaning |
| --- | --- | --- |
| `b69405c8…`, `root-stock-type-argument-validation-v7` | 20/20 focused bodies pass | Entire unchanged `macro-nested_stmt_macros.rs` main runs in all four editions with three ordinary generic calls and real generated nominal Type identities; stock local-shadowing regression repaired |
| `04cb078e…`, `root-stock-module-cast-baseline-v2` | Original-source lease control passes; `shift.rs` owning 2015 fails (`e94ed7/1`) | Legacy assertion route cannot resolve a nonlocal binding; other editions unexecuted |
| `fdec9bb3…`, `root-common-reached-body-components-v4` | 15/15 focused bodies pass | Target publication authority plus 14 regressions; six prior seed/transport passes remain separate |
| `05bbe91f…`, `root-common-reached-body-components-v7` | **22 scoped bodies pass, one CLI whole-main failure** | Eight new memo/renewal/entry controls and 14 regressions pass; unchanged `issue-40847.rs` fails owning 2015 during unresolved `assert_eq` discovery |
| `454cbbf5…`, `root-stock-call-observer-baseline-v1` | Compile `6de9a4/0`; owning 2015 fails `416d6c/1` before guest execution | Unchanged `issue-94923.rs` selected-body assembly lacks an actual issued expansion reply; later editions unexecuted |

Tracked receipts:

- `worktracks/evidence/2026-10-16-common-reached-body-components-validation.json`
- `worktracks/evidence/2026-10-16-common-body-memo-renewal-validation.json`
- `worktracks/evidence/2026-10-16-stock-shift-whole-main-baseline.json`
- `worktracks/evidence/2026-10-16-stock-call-observer-baseline.json`

The latest complete scoped interpreter regression result is **not a cumulative upstream suite count**. This closeout has not established an additional distinct upstream runtime-contract or full-fixture pass. Imported inventory counts are not passes. No current unfiltered milestone or full rustc runtime gate is claimed.

Keep reports separate: `upstream_runtime_acceptance`, `diagnostic_compatibility`, `compiler_tooling_compatibility`, `full_suite_compatibility`.

## Shared mechanisms just implemented

### Preparation and immutable ownership

`SyntaxAttachmentSeed` shares the actual pinned syntax tree, origins and safely synchronized SpanStore; fork avoids reparsing, walking and hashing the whole source. `CompletedSourceSyntax` retains the real per-source assembly seed and actual issued reply/receipt chain. Missing required output is a refusal, never fabricated.

Query **48**, `PreparedBodySyntax`, consumes opaque input **34**, `CompletedSourceSyntax`. `Database::publish_completed_source_syntax` is coordinator-only. The query performs structural selected-body preparation without executing producers or publishing. `PreparedBodySyntax::admit` enters the existing `RetainedSyntaxInput` path. Wrapper/plan/result physical ownership is handled by the existing weak retention framework.

`cli::retained_entry::prepare` now uses that memo instead of directly reconstructing selected source assembly. Repeated actual entry preparation is checked over unchanged upstream source.

**Held accounting:** opaque pinned Salsa intern-arena allocation is not fully measurable with the existing API; external/unpublished ownership and transient peaks have explicit remaining limits. Do not claim complete Section 32.2 or performance acceptance.

### Target publication and call evidence

`db/retained_call_transition.rs::RetainedCallBodyTransition` is minted by the actual checked publisher. It certifies exactly four actual target rows before/after publication: source, header, body locator and declaration context. The original call proof and guard remain stale.

`engine/install_call_renewal.rs` fetches fresh ordinary Q47/Thunk evidence, checks the same selected declaration/full signature/source Type rows and **exact recipe operations, origin and exit**, then atomically replaces only already installed phases of that call node with the same RecipeIds. It does not prepare future phases or replay arguments.

Observer renewal handles another genuinely selected call to the same definition using that observer's own prior dependency stamps and source contract. It requires **all four real prior target stamps**, not just equal current stamps. A source review found that cold module admission checked `DefinitionBody` using non-recording `published_input`; it now also observes the same actual row through `db.input`. The canonical locator/stamp check is preserved. `FunctionSignature` remains body-independent.

**Not integrated:** `RunMachine` still needs charged persistent publication receipts, fresh pending-call ownership, and resume ordering. A local transition variable is insufficient: after `NeedInput`, installed resolution runs before CallSeal preparation. An outer suspended call may already have selected a target prepared by a nested argument call. Preserve evaluated slots/hints/explicit descriptors/argument progress; renew before installed resolution. Do not prepare bodies early at CallPrepare merely to avoid this problem: an argument may never return or may have effects before callee entry.

Read `worktracks/evidence/2026-10-16-runtime-module-publication-continuation-source-review.json`. `run_module_source_preparation.rs` is inherited **unhooked WIP**, not a validated runtime fix.

## Current SDK/macro source frontier

The upstream failures point to actual missing macro-source admission, not a need for a larger assertion evaluator.

The final in-flight source component adds owning `SealedMacroExportProviders`: genuine completed SDK providers, original provider arenas/ordinals, generated/staged catalog roots, parent links, records, routes and seals are moved after all recoverable checks. Consumed discovery methods refuse further use. Borrowed `CapturedMacroContext` APIs can obtain actual foreign definition bodies/origins while retaining the real caller and each SDK arena separately. SDK source-role ownership does not mint ordinary runtime source/body authority.

The final source also retains imported SDK-owner ancestors with a checked depth bound, identity and conservative bytes; contexts borrow one graph owner instead of attaching a huge SDK Arc to every source seed/result. The actual cross-crate source control uses unchanged `cross-crate-pat-span.rs` (owning 2021, check-pass) and its unchanged `foreign-crate-macro-pat.rs` auxiliary (definition edition 2018). That is a **preparation component witness**, not an upstream runtime-contract or full-fixture pass.

**SDK caller adoption is not finished.** The `SourceDiscovery.exports` field defaults to `None`; there is no completed SDK admission builder or certified relevant namespace projection. Existing configured-facet safety refusal remains. Some internal callers still use default-context wrapper APIs; `generated_macros::call_namespaces_with_routes` also needs actual SDK route transport in the eventual common consumer. Generated/staged/recursive SDK ancestry coverage remains incomplete.

### Critical next prerequisite — not implemented

Read:

- `worktracks/evidence/2026-10-16-selected-sdk-macro-dependency-contract.json`
- `worktracks/evidence/2026-10-16-selected-sdk-macro-projection-stop-handoff.json`
- `worktracks/evidence/2026-10-16-owning-sdk-consumer-admission-precode.json`

Do **not** attach the full SDK seal dependency closure to every ordinary recipe. `PreparedInputGuard` flattens query dependencies; existing `SealedName`/`NamespaceReads::certify` pull in the full SDK seal, and `CurrentModuleName` also requires ordinary `CapturedSource`. Those shortcuts would cause broad SDK dependency work/invalidation in sensitive execution paths.

Reuse the actual observed macro resolver and `NamespaceReads` to build a privately certified relevant lookup/source projection under the existing DB publication framework. Positive, negative, ambiguity, module-prefix, reexport and glob facts must come from the genuine completed SDK namespace. Publication lifecycle must revoke or refresh affected facts when another producer/source adds a competitor; watching only the selected source is unsound. Preserve unrelated bucket answers. Full SDK validation stays cold; no parallel resolver or independent namespace registry.

The proposed projection API, lifecycle and possible input tag **35 are investigation only**. No DB/schema/tag implementation was started. The user stopped new work before that phase. Read the proposal's open proof/lifecycle issues rather than treating it as an approved completed design.

## Next-agent implementation order

This is a continuation map, not authorization for the outgoing agents to start more work.

1. Reconcile the checkout with the final checkpoint and preserve unrelated dirty work. Read the architecture audit and exact pending claims.
2. Implement the genuine relevant SDK namespace/source dependency prerequisite, then common SDK caller adoption and definition-site/caller scope transport. Keep the configured refusal until the replacement is equally sound.
3. Join actual selected reached module bodies to shared memoized syntax admission and ordinary source environments. Integrate publication/observer renewal with persistent, accounted continuation state and suspension/cleanup.
4. Run the unchanged upstream witnesses whole: `shift.rs`, `issue-40847.rs`, `issue-94923.rs`; then owning/supplemental editions and the required source/user/wrapper/standard-macro route matrix. Follow each actual first failure through common mechanisms.
5. Remove superseded macro-specific production dispatch/evaluator/index/verifier routes only when the shared replacement and required matrix are evidenced. Keep architecture/task/JIT gates held until actual acceptance.

`issue-94923.rs` is a useful same-target nested-call witness: `f1<T>` calls `f0::<(i8,T)>(f0::<(u8,T)>(x))`; full main expects 348 through f8. It also needs tuple generics and loops. The harness allows 64 million shared fuel before any operation; actual call-count lower bounds do not prove per-argument exact-once effects. Its upstream `-C opt-level=3` directive is preserved; runtime evidence is separate from compiler optimization assertions.

## Checkout and validation operation

The main checkout contains substantial **inherited dirty/untracked work**, including associated/trait scans, generated declarations, field/source changes and unhooked module preparation. Do not `git add -A`, reset, clean, or assume everything visible was tested. Two other imported fixtures are inherited working-tree-only additions: `issue-75299.rs` and `unary-minus-suffix-inference.rs`; their live manifest entries were deliberately excluded from the source checkpoints here.

The pushed/frozen test inventory for this batch is **337 cases / 859 original files** at upstream commit `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`. The mutable live manifest may contain **339 / 861** because of those two inherited additions. Keep original MIT and Apache license texts and unchanged rustc bytes. Use the existing importer; do not overwrite the narrow frozen manifest with the larger moving one when validating or staging.

All validation ran on frozen source copies created with **`cp -Rc`**. Tests run asynchronously under one validation owner while implementation uses another source area. One reusable Cargo target, called **T3**, is:

`/Users/avi/Code/rust-jit/artifacts/macos-first/root-constant-next-validation-v1/validation-target`

Do not copy/clean/rebind it while active. After release, obtain a fresh process inspection and use the final owner/release receipt before rebinding. Existing artifact `bind_target.py`, `SOURCE-FROZEN.json`, `command.json`, terminal/child-terminal, binary hash, source guard and release scripts show the protocol. Raw local artifacts and temporary packets are not in Git; the tracked evidence summarizes exact source/terminal identities for remote handoff.

Typical compilation, owned by the asynchronous validator in its frozen source:

```sh
cargo +1.99.0 test --offline --locked -p lrr --lib --no-run
```

The actual command records also pin target ownership, two build jobs, no incremental builds, no debug symbols, single-threaded test execution and upstream checkout path. Use exact listed test selectors; a helper module with zero tests is not a pass. Run only checks justified by changed source or unresolved failures. Do not repeatedly rerun the whole suite to occupy an agent.

Before source checkpoints: `python3 worktracks/check.py`, exact staged source/fixture guard, and `git diff --cached --check`. The source guard reads pinned files relative to its **working directory**. When the staged manifest comes from a frozen source, run the guard with that frozen directory as cwd, `GIT_DIR` pointing to the real repository, and `GIT_WORK_TREE` pointing to the frozen source. Setting only the environment while cwd remains the dirty main tree causes a false manifest/filter mismatch. Never weaken the guard or replace the live inherited manifest to get past it.

Unsigned checkpoints were explicitly authorized after signing-agent refusal. Periodic source-only pushes to the implementation branch are authorized. Never push targets, caches, raw validation runs, full source snapshots or large files. Do not fetch/apply/merge PR code without user approval.

## Worktracks and incoming ownership

There are **12 worktracks / 60 planned items**. See `worktracks/README.md`, `registry.json`, `claims.json`, `history.jsonl`, and `docs/tasks.json`; the specification's Section 57 order still governs.

Outgoing root work was coordinated under `LOCAL-024`; the old root primitive slice was `LOCAL-035`. Final closeout records release those outgoing reservations for handoff without marking features accepted. Historical active reservations `LOCAL-026` and `LOCAL-030` are not live agents in the current team; they remain visible for maintainer/owner triage rather than being silently deleted. Read their scopes before overlap.

The outgoing three agents finish current packets/validation and then stop. Do not assume an idle historical agent name owns a fresh task. A future resumed implementation team requires disjoint slices: implementing lead, two implementation agents and one asynchronous validation/review agent under the current authorization. No new team work is launched as part of this handoff.

## Final closeout result

The current bounded source work is checkpointed with explicit holds. The final frozen source is **`c9ad087752af9738e7a9d139383220f1df429cdd370a8c3d41b871c666308e09`**, after correcting actual compiler-reported integration errors. Compile **`a6d276/0` passes**. The four-body closeout queue **`c241e0/1` fails overall: two pass, two fail**.

| Focused check | Actual result |
| --- | --- |
| Borrowed facet requires real completion/publication | Pass |
| Genuine AUX owning transport, currentness/consumption/refusal controls | Pass |
| Unchanged full `core` source preparation | Fail: `LRR-E-SOURCE-PATH`, portable-SIMD include outside granted source authority; graph transfer unentered |
| Unchanged cross-crate/AUX borrowed-definition source control | Fail at setup: `LRR-E-CATALOG-INCOMPLETE`, dependency consumer not selected; request, receipt, expansion and origin/edition assertions unexecuted |

The cross-crate control constructs discovery with only the auxiliary while the captured dependency graph contains caller → auxiliary. The existing constructor correctly requires the consumer to be selected. Establish a coherent configured SDK/caller preparation boundary before rerunning; do not drop genuine dependencies merely to make the control green. The full-core failure requires legitimate include capture/preparation authority, not relaxed path checks or altered upstream source. These are next-agent prerequisites; the outgoing batch starts no new implementation to address them.

Earlier failed compile candidates are preserved, including initial missing import/mutable DB API errors and the final-join 11 mechanical errors on `965b9407…`. The corrected source was unchanged during compilation and all four checks. No additional distinct stock runtime-contract or full-fixture pass was established. The current full milestone was not run; this is a source-only checkpoint with failed component acceptance, not an integrated task.

[Tracked final validation receipt](../../worktracks/evidence/2026-10-16-interpreter-handoff-validation.json) includes exact failed attempts, final source/binary hashes, all four outcomes and target release. [Final all-eight source review](../../worktracks/evidence/2026-10-16-interpreter-handoff-source-review.json) is pinned to the same final source. The final local release receipt is `artifacts/macos-first/root-handoff-final-source-v2/RELEASE.json` (SHA-256 `1124d0c39819aabe5472c1e3b3a58d3dab4703be30b7b85ee4cdbbaf86c530e1`); T3 is inactive and released with owner `5700475c444995602252670a73cb4586e8b740251d2b96afaf2100e70bd8a46f`.

Outgoing `LOCAL-024` and `LOCAL-035` reservations are released for handoff; unresolved features remain incomplete. The two implementation agents stopped after their current packets; the validation agent stopped after sealing the actual terminal/release. No replacement tasks were assigned. The source checkpoint containing this document is on `codex/lrr-implementation`; obtain its exact ID with `git log -1 -- docs/handoffs/2026-10-16-interpreter-handoff.md`. Raw artifacts and inherited WIP stay local.
