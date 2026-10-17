# Implementation instructions

Specification: [LRR revision 5](lrr_rust2024_macros_traits_implementation_spec.md).

## Simple interpreter scope (October 16, 2026) — overrides conflicting text below and in the spec

Explicit user direction, recorded as [E-R5-006](docs/errata/revision5-no-resource-accounting.md):

- **Only rustc's limits.** A limit is allowed only if it reproduces one the pinned rustc enforces, cited by compiler source location (for example `recursion_limit`, default 128, honoring `#![recursion_limit]`). No memory accounting, no fuel or time counting, no byte budgets, no count caps. The OS bounds resources. When an existing cap blocks a correct program, delete it instead of raising it.
- **Looser than rustc, never stricter** (user, October 17, 2026). Run the code and produce the same result the compiled program would; do not re-implement rustc's static checks (borrow checking, const-context restrictions, E0106/E0508/E0509-style rejections, privacy or lint enforcement). Users who want those run `rustc`/`cargo check`. Everything rustc compiles must run; being more permissive than rustc is allowed. Delete runtime refusals that only re-enforce a static rule.
- **No sandbox for now.** The permission broker, IO grants and capability/authority checks on source or runtime access are out of scope. Do not extend them or let them block language work; source reads follow what rustc reads.
- **Goal.** A simple, correct interpreter tested against unchanged upstream rustc tests, then the JIT. The one-pipeline rules (E-R5-005) still apply.
- **Constant initializers** (October 17, 2026, [E-R5-007](docs/errata/revision5-const-initializers-vm.md)): reached `const`/`static` initializers with nominal values, references or calls run once through the ordinary interpreter into run-long static storage (also used by byte strings and promoted `'static` data). The pure evaluator stays only for preparation-time values such as array lengths.
- **Workflow.** Edit and build directly in this checkout (`cargo +1.99.0 test --offline --locked -p lrr --lib --no-run`); no worktrees or frozen copies.

## Architecture statement of work and implementation gates (October 15, 2026)

The required deliverable is one compositional Rust interpreter, followed by the specified JIT tiers after their gates pass. A feature must retain its semantics when moved between ordinary source, a user macro expansion, a macro wrapper and a standard-library macro expansion. Passing selected examples through a second, context-specific implementation is architectural failure, even when the values match. This section incorporates the user's architectural review of `fab75fe`; its eight findings below remain repair obligations until current source and actual evidence establish their removal.

Read [the macro architecture audit](docs/audits/macro-specific-semantics.json), [E-R5-005](docs/errata/revision5-common-macro-engine.md) and the relevant specification sections before changing affected paths. Existing violations are repair debt, never precedent or permission to extend them. Do not promote held macro-specific candidates because their historical tests pass.

### Binding statement of work

Implement the specified common mechanisms, rather than accumulating fixture-specific compatibility. The deliverable includes removal of superseded production routes, integration through ordinary source preparation and execution, and evidence for the required contracts. A new helper API, isolated successful execution or larger passing-test count does not by itself deliver that work.

Before starting a repair, write a bounded architecture record in its claim, handoff or PR containing: the violated invariant and minimal witness; the actual shared entry points to extend; the alternate routes to remove; dependency and authority boundaries; cold-path and hot-path costs; and the evidence required for closure. Distinguish the eight failure modes listed here from the separately numbered `MA001`–`MA008` audit findings; reference the applicable findings explicitly rather than treating their numbers as interchangeable.

Repair prerequisites in this order where they apply: admitted generated syntax and source/scope identity, shared expression lowering, shared source constraints/member resolution, then shared transactional patterns and their execution/cleanup. Respect Section 57 dependencies throughout. If a witness exposes missing common infrastructure, implement or explicitly track that prerequisite; do not grow a parallel evaluator to make the witness pass. Keep every affected legacy route named and held until its replacement and removal are evidenced.

### Before changing a language feature

Record the following in the bounded claim, worktrack handoff or PR, using actual files and symbols:

- The T/H/M task, invariant and unchanged upstream witness.
- The existing ordinary-source route: source identity and hygiene, syntax/index node, name/type constraint service, lowering/verifier, runtime operation and continuation/cleanup owner.
- Where generated and handwritten syntax join that route, and which existing mechanism the change extends. Identify any duplicate implementation it replaces and the condition for deleting it.
- The exact preparation demand and cache dependencies, what remains cold, and what execution/currentness work occurs per entry or operation.
- The acceptance scope and known gaps. Intermediate source components are allowed, but must identify the remaining integration/removal work and cannot close the feature or architecture gate.

If the proposed implementation needs a second parser/evaluator, a context-only rule, a generated-source exclusion for already supported syntax, or repeated source preparation in execution, stop that approach and implement the missing common mechanism. Do not silently narrow the feature to the convenient fixture. An actual specification contradiction requires a minimal witness and a clearly explained design question; routine implementation of the specified common engine does not require another permission request.

### One macro and language execution pipeline

Resolve and expand actual macros through the common hygienic macro system. Attach their actual output to the owning source tree, then use ordinary parsing, indexing, name/type resolution, expression/pattern lowering, verification and interpreter operations. Preserve original and generated member identity, owning editions, token contexts, caller bindings, labels, temporaries and continuations.

Do not implement macro-specific runtime expression, pattern, inference, call, method, ownership or cleanup semantics. In particular:

- No assertion-operand compiler or evaluator, synthetic function/block around macro operands, or reparsing display text to obtain a convenient executable body.
- No literal-only generated-expression executor, generated-break/scalar-payload executor or opaque macro-only phase/index/verifier namespace. Extend ordinary syntax/control handling instead.
- No numeric constraints selected by `assert_eq!`, `assert_ne!` or another macro's identity. Infer from the ordinary expanded source constraints and selected member signatures.
- No method-name whitelist available only inside assertions or another expression context. Ordinary member selection must choose the same source body or explicitly specified runtime intrinsic everywhere.
- No new `original source only` refusal for syntax already supported by the common engine. If generated syntax lacks required source/scope evidence, implement that evidence join; keep an explicit unsupported result until then. Never bypass hygiene/currentness or count that gap as compatibility acceptance.
- One transactional pattern engine must serve `match`, `if let`, `while let`, let-chains, `let else` and generated patterns, including non-consuming tests, binding modes, moves/borrows, alternative retry after false guards, commit and cleanup.

A renamed helper, shared opcode, matching final value or source-identity check around an alternate evaluator does not satisfy this requirement. Compiler expansion hooks stay in the pinned adapter; they must not become another runtime Rust interpreter. The separately specified pure constant evaluator is intentional and remains within its specified purity, demand and authority contract. Explicitly specified runtime/library intrinsics remain valid when selected through the common source/member service.

### Lazy preparation, eager reached execution

Preserve lazy parsing/type preparation and ordinary eager execution of reached Rust operations. Declaration discovery and required macro-producer closure may inspect or expand syntax as the specification requires; that is not permission to prepare executable code for every branch or function.

An entered operation must not require all unselected match results or cold branches to have supported executable shapes. Obtain needed facts through the shared demand-driven source/type/member services. A declared callee signature may answer a type goal without lowering or executing its body. Add a witness in which only a never-taken branch changes from a literal to a source call and ensure that this does not cause eager executable preparation or an unrelated unsupported error. Do not implement a borrow checker or full static checker as a workaround; `cargo check` remains separate.

### Prepare once and share immutable results

Use the existing pure query/memo framework for immutable source preparation. Key plans by their real source, scope, edition, expansion epoch and semantic dependencies. Phase selection must select from a retained plan; it must not repeatedly parse/walk the same operands or reconstruct a complete plan for each phase. Installed operation execution must not re-enter source planning. Runtime guards check the relevant runtime facts and prepared dependency versions.

Use relevant sealed namespace buckets and positive/negative lookup dependencies. Do not add full-world namespace-map construction/comparison to dispatch, installed-recipe guards, per-operation checks or ordinary call entry. Unrelated namespace publication must not invalidate a recipe unless it changes a dependency of that recipe. Relevant additions must still invalidate prior absence/ambiguity conclusions. Preserve existing safety refusals until an equally sound dependency mechanism replaces them; a global census is not an acceptable permanent hot-path implementation.

Share immutable parser/body/index structures through retained handles such as `Arc`. Frames own their mutable execution state, slots and continuation; they must not deep-clone immutable `BodyIndex` data during call preparation and again at entry. Resolve indexed nodes through retained direct syntax locations instead of searching the complete parsed file for every node phase. Bound and account new indexes/handles under Section 32.2.

Performance acceptance needs structural evidence that normal dispatch does no source parsing, full-plan reconstruction or global namespace scan, plus representative measurements before making speed/scaling claims. Source inspection alone can establish a suspected cost, not measured release performance.

### Required architecture acceptance and review

Use unchanged official rustc fixtures and their actual directives/auxiliaries as the primary language witnesses. Add LRR-specific route, authority, suspension and exact-once controls outside imported originals. For the changed feature, compare the same operation handwritten, emitted by an arbitrary user macro, emitted through a wrapper and nested in an applicable standard macro. Inspect the actual indexed nodes, verifier route and execution trace/counters; matching outputs alone cannot prove a shared engine.

Cover the affected contracts: blocks with statements, guarded matches and other patterns, ordinary versus macro-nested methods, numeric constraints, generated functions with reference parameters, caller bindings/labels, cold branches, exact-once calls/effects, failure-only formatting, panic/cleanup, suspension/resume and source/epoch invalidation. Reuse upstream cases incrementally; do not create a miniature substitute language suite. Report unsupported and unexecuted matrix entries as gaps. Do not claim the architectural feature complete until the applicable matrix and current milestone pass.

Every language/frontend/runtime contribution must include a source-aware architecture review against all eight recurring failure modes:

1. Separate assertion/expression compiler or evaluator.
2. Literal/break-only macro-output execution instead of ordinary syntax.
3. Context-specific method selection or numeric inference.
4. Generated-source rejection of already supported ordinary semantics.
5. Duplicated pattern machinery or eager checks of unselected executable arms.
6. Repeated parsing/planning during phase preparation or installed execution.
7. Full-world namespace scans or unrelated invalidation on sensitive execution paths.
8. Deep copies of immutable body structures or repeated whole-file node searches.

Name the actual shared entry points and changed call sites, the removed/remaining alternate routes, relevant asymptotic work and reproducible witnesses. Use an already authorized reviewer/test agent when available; this requirement does not authorize extra agents. Otherwise perform and record a separate source-review pass. Documentation checkboxes and green feature tests do not substitute for reviewing the code path.

Record that review against an exact commit or source fingerprint before integration. For each applicable failure mode, cite the inspected symbols, the common route actually used, remaining exceptions and the relevant reproduction/terminal evidence. Mark unreviewed or unexecuted requirements explicitly; do not infer them from a neighboring passing case. If source changes after review or validation, identify which conclusions need rechecking and keep the old evidence attached to its original source.

A repair is complete only when the replacement handles its required contexts, obsolete production dispatch/evaluator/index/verifier paths are removed, source and runtime authority checks remain sound, and the required architecture matrix and milestone have actual passing evidence. A staged migration may be checkpointed with explicit holds; it cannot be reported as the completed repair. Documentation changes establish instructions, not implementation correctness or a guarantee against recurrence.

If review finds architectural drift, hold integration and feature-completion claims, record the minimal counterexample and make the common-engine repair the next implementation priority. Do not expand a shortcut's vocabulary while promising to remove it later. Do not self-authorize an exception by renaming code, writing an erratum, opening another worktrack or labeling the shortcut temporary. Only an explicit user-approved design change can change the pinned architecture. Preserve failed attempts and prior evidence, and keep component/runtime/full-fixture/diagnostic/tooling outcomes distinct.

## Parallel contributor work

Use [worktracks/README.md](worktracks/README.md) and [worktracks/registry.json](worktracks/registry.json) to select one of the 12 substantial work streams. These plans divide ownership; the specification and docs/tasks.json still govern task order and acceptance. Read the chosen track, active claims in worktracks/claims.json, and its upstream witness before changing code. Claim a bounded item through the worktrack issue template, record exact files/symbols and API dependencies, and use a separate fork/branch or isolated checkout. Existing local reservations must be coordinated before overlapping work. Contributors may bring their own agents and compute; each contribution needs a human owner, a reviewable PR and reproducible evidence.

Keep independent agents on disjoint slices and give them the task, invariant, fixture and handoff requirements. The user-authorized default is three assigned subagents alongside the implementing lead: two independent implementation slices and one asynchronous validation/review slice. Reassign a finished agent promptly to concrete prerequisite work or review; do not launch redundant tests merely to keep it busy. The lead must continue implementation and code reasoning, not only coordinate. A later user instruction may change this team size; never exceed the current authorization. A source-only draft is useful while prerequisites or asynchronous validation run; it is not an integrated implementation or passing fixture. Keep tested sources unchanged and report actual command terminals before claiming acceptance.

An assigned test agent owns asynchronous validation while the implementation agent reasons about errors and changes code in a different source area. On macOS, use `cp -Rc` when giving that agent a copy of an existing Cargo target; record the source, destination and actual copy terminal. Keep the validation source frozen and give each target one Cargo owner.

Push source checkpoints periodically to `https://github.com/Avi-D-coder/rust-jit.git` on the implementation branch. Include source, tests, documentation, plans and required licensed inputs; keep build products, validation runs, local caches and full source snapshots local. Run `python3 tools/check_source_checkpoint.py` against the staged checkpoint before committing. Do not fetch, check out, apply or merge pull request code without the user's approval. A pushed work-in-progress checkpoint is not an integration, runtime-acceptance or release claim.

Each PR identifies its worktrack/item, claim issue, T/H/M cards, prerequisite PRs, source baseline, owning platform/editions and exact acceptance scope. Use the PR template. Keep imported rustc originals unchanged and put adaptations outside them. Preserve runtime-contract versus full-fixture outcomes and separately labeled compiler evidence under Section 1.2.1. No worktrack permits early guest JIT or changes the pinned architecture.

At handoff and merge, update the item/claim, linked PR and evidence in worktracks/registry.json, worktracks/claims.json and append-only worktracks/history.jsonl. Update docs/tasks.json and implementation-state only to the acceptance actually established. Maintainers refresh plans after each merge and at active weekly triage, recheck dependency/path conflicts, and collect the integrated milestone in Section 57 order. Run `python3 worktracks/check.py` for plan/claim/link integrity; this is a planning check, not a runtime gate. Do not invent issue/PR/merge IDs or publish externally without authorization.

Implement and test on macOS first with Rust 1.99.0 (current host aarch64-apple-darwin). Linux x86-64 is a later port; Docker is optional supplementary tooling and unavailable Linux validation must not block macOS task progression. Preserve platform-specific evidence and require actual Linux acceptance before claiming Linux support.

Implement revision 5 using the fixed cross-ledger order in Section 57 (T/H/M tasks). Completed revision-3/revision-4 evidence remains historical component evidence until the applicable revision-5 integration requirements pass. Sections 61–62 select direct Rust ABI bindings, Cranelift and LLVM tiers, typed optimization, reconstruction maps and explicit precise/throughput contracts; this authorized design revision does not start guest JIT implementation ahead of interpreter acceptance. Do not choose a different frontend, query framework, native backend, language profile, sandbox backend, or tiering strategy. Preserve explicit interpreter/JIT/capability semantics. Before changing code, identify the task and invariant being implemented. Add its acceptance test before marking the task complete. Run the current milestone gate after each integrated task. Never mark a selected fixture passed by skipping it, catching all errors as success, or hard-coding an expected output. Never load arbitrary native symbols or compile a selected live runtime unit. Only explicit authorized preparation may run build tools. Execution of a previously compiled procedural macro requires separate macro authority and the matched external server; it is never a query-provider side effect. Only an immutable schema-checked native generation may load in an authorized hybrid worker. Use pinned dependency source to repair mechanical API mismatches. Record an actual contradiction as a minimal witness plus regression and erratum, not an undocumented design fork. Finish with reproducible release artifacts and an honest feature/performance/security report.

Run tests asynchronously in the background and continue independent implementation or review while they run. Collect terminal results before claiming a pass or integrating a candidate; keep the tested source unchanged during each validation run.

Follow Section 32.2: deterministic indexes, typed outcomes, checked sizes and explicit stable encodings. Local production unsafe is confined to jit/abi.rs, jit/helpers.rs, jit/module_cache.rs, native/loader.rs, and native/ffi.rs. Do not claim planned or unexercised checks passed. Keep docs/tasks.json and artifacts accurate.

Use the official rustc checkout at rustc/rust as the primary source of language fixtures as features are implemented. Copy relevant tests incrementally, including auxiliary crates, directives, revisions and expected output files; record the upstream commit and hashes. Keep copied tests unchanged in fixtures/upstream/rustc-checkout under the original rustc MIT OR Apache-2.0 license, with both license texts retained. Put LRR adaptations and harness assertions outside that directory. Supplement upstream language tests with LRR-specific authority, suspension, invalidation and seal regressions. Imported, unexecuted or partially exercised cases are not full fixture passes.

# Interpreter-first runtime-semantics priority (October 13, 2026)

Before guest JIT implementation, require complete interpreter execution of every applicable official rustc runtime contract, including currently unsupported runtime language features, plus applicable interpreter correctness gates. Preserve a complete upstream inventory; compile-time diagnostics, static type/borrow checking and compiler/tooling outputs have separate acceptance reports and do not block guest JIT. LRR does not implement a borrow checker or full static type checker; users run cargo check separately. Report runtime-contract passes, full-fixture passes, failures, unsupported and unexecuted cases distinctly; imported files and component checks are not full stock passes. Section 1.2.1 defines this user-authorized runtime-semantics prerequisite.

## All official editions (October 13, 2026)

Support Rust 2015, 2018, 2021 and 2024 for selected roots, dependencies and macro origins. This user-authorized policy supersedes the earlier 2024-only root restriction. Preserve owning editions in parsing, name resolution, preludes, macro spans, patterns, temporary lifetimes and cache/unit identities. Default standalone edition remains 2024 with explicit --edition selection; Cargo targets retain their declared edition. Unknown or unreleased editions must fail explicitly. Drive edition-sensitive implementation with unchanged official rustc fixtures; edition admission is not full compatibility acceptance.
