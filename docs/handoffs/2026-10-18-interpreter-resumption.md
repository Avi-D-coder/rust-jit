# Interpreter resumption — October 18, 2026

The active goal is to finish the revision-5 interpreter and then the specified
JIT tiers. Guest JIT remains blocked by complete applicable upstream runtime
acceptance and interpreter correctness. Diagnostic/tooling evidence stays
separate. No whole interpreter or JIT milestone is closed here.

Continue directly in this checkout on `codex/lrr-implementation`. Baseline HEAD
was `b97c954c86712c3b9c005af39902459f2bcfe11f`; preserve the large inherited dirty
source set (nested bodies, parameter patterns, dyn views, layout/discriminants,
macro attachment, collections and more). It compiled at resumption. Earlier
handoffs' clean-tree, missing-readiness and source-copy statements are historical.
Follow the current AGENTS E-R5-006/E-R5-007 overrides: no sandbox/resource caps,
no static compiler checks, VM-backed reached initializers, and direct builds.

## Ownership and validation

Root implements generated body environments and local const/static admission.
`generic_projection_resume` owns bound projections, generated inline Type scope
and atomic facade/library operations. `closure_callable_resume` owns ordinary
callable source-Deref continuation, result descriptors and retained numeric facts.
`interpreter_validation_resume` alone owns Cargo and `target/`; tests run in the
background. Writers pause for exact-source builds, then continue independent work
while the resulting binary is tested. No worktree, target copy or cargo clean was
used in this resumption. There are 12 contributor worktracks; bounded ownership
and evidence are in claims LOCAL-036 through LOCAL-041.

## Verified evidence and failures

The pinned upstream checkout is `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`.
The current import manifest contains 2,807 cases and 3,477 files, including the
unchanged coretests atomic support file newly imported for selected library test
execution. Imports are not passes. Both upstream license texts remain retained.

The older local inventory `artifacts/runs/inventory/raw-v16-facade.json` reports
1,429 runtime-contract passes on its own binary (SHA-256
`732aec123f8f5de8cf465c9c5ccd757970908ae62bba3dfd68f61a260b27650b`).
That is historical evidence, not a current-source/full-fixture count. No newly
passing whole upstream fixture has been verified in this resumption yet.

All resumption raw terminals, source manifests and reports are local under
`artifacts/validation-resume-2026-10-18/`; do not commit build products or these
large raw runs. Each candidate report retains failed attempts and binary/source
identity. This document records bounded evidence, not a replacement full survey.

- Initial source `37e6b1a1522473d549129c62d703f8f1ef5fa14cde9bc337635beae3e57dbfdf`:
  lib compiled; six selected test bodies passed (readiness, projection and existing
  closure controls). Corrected zero-match invocations are not counted.
- V1 generated boundary/source failures were preserved. V2 failed to compile from
  an incorrect test-helper path; corrected in V3, not hidden.
- V3 source `256c2681c1bd44aca4b81cc5d9c71b745c68a587e070e17c90d9c5e612327f09`:
  nine bodies, six pass/three fail. Bound-aware projection matrix and earlier
  generated controls pass. Four-edition rustc metadata checks are compiler-only
  evidence, not interpreter execution.
- V4 source `a841b6b91668cb3d3d581bc85d4e1a307b23a34701148a5d4d19f9c6f634bbd3`:
  generated ordinary/user/wrapper/inline-module matrix passes in all four editions;
  module epoch refresh passes. Existing Type-scope ambiguity assertion fails;
  attribution remains unresolved. Source-Deref control advances from structural
  result descriptor refusal to missing numeric context. Vec constructor remains
  unsupported. Unchanged `const-fn-nested` runtime completes; already passed in
  the historical inventory, so zero newly passing stock cases from it.
- V5 source `75a85e3ac6503093388dadb5c1ad68159b7e72c18a2419f8f286fd0fc7506e8e`:
  lib build terminal `85e8be/0`, CLI `74d204/0`. Six lib bodies: five prior
  initializer/readiness controls pass; new local nominal initializer matrix fails
  at function-only declared result Type admission. Actual whole const arithmetic
  fails there too. Atomic selected library tests own edition **2024**, not 2015;
  earlier 2015 attempts failed setup and executed zero test bodies. Correct 2024
  attempts selected one body each (42 catalogued/41 filtered); all six fail before
  verified atomic operation behavior (local initializer result Type or primitive
  namespace context). The associated-types via-where UI witness fails before any
  proved Atomic execution, in optional explicit-Type argument inference. Do not
  describe it as having executed fetch_add. V5 report SHA-256:
  `faf408fce5828955110c490aa686993e60fd5e21e2a8835b20895c45da8ce686`.
- V6 source `3d5608832ad2a0610c3d3ff5be25d9657825fe0e058993e1cd1cd8eaacb8736e`:
  lib terminal `5c67c9/0`, CLI `1e63a9/0`, exact source unchanged for both builds.
  Three focused bodies all pass: source-Deref exact-once/unsuffixed control
  (18.06 s), local nominal const/static matrix (53.41 s; ordinary/user/wrapper,
  all four editions, repeated calls), and bound projection matrix (21.81 s).
  Whole unchanged const arithmetic now executes 60 source-call boundaries and
  8,231 dispatches before the explicit missing 128-bit integer profile support.
  Whole via-where witness reaches 27 source calls/375 dispatches before missing
  generic value-path context. Neither is a whole runtime-contract pass. Captures
  contain body-relative ranges but no physical source location mapping, so the
  likely PhantomData expression link remains an inference. Three numeric/wire
  regression checks also pass: all six selected bodies pass. CLI SHA-256:
  `b045d2cc8a4bd8c9afedeb942151ff6d4390d1c35433c4cf6fdf121b778b12de`.
  A follow-on typed optional-goal query-domain guard is unvalidated working-tree
  source, separate from the staged V6 checkpoint.

## Mechanisms and remaining obligations

Generated source retains its actual `GeneratedBodyBoundary` in the completed
source environment. `capture_module_source` attaches that shared source view;
`require_current` checks actual source/unit/epoch/scope rather than comparing
unrelated namespace-seal domains. Actual generated inline Type selection joins
the current scope certificate and source membership. These are scoped repairs,
not proof of every generated-source/currentness contract.

Shorthand projections use their actual declaring bound and trait arguments;
existing shared substitution handles projection arguments. Callable adjustment
uses ordinary CalleeDeref IR and the existing source operator continuation;
result descriptors use the actual bound generic environment, without selecting
an arbitrary ambiguous binding. Arc construction and generic Vec constructor
inference remain outstanding. The retained numeric hint uses existing source
component equalities and already-live same-frame values; no assertion identity,
cold call execution or private nominal Deref layout inference is allowed.

Local const/static declarations publish their genuine initializer BodyHandle
and use the same ordinary declaration-to-callable admission/frame/result/storage
route as module initializers. The declared result Type service now admits their
actual annotations and retains headers with the initializer expression excluded;
it does not execute or inspect bodies for field Types. Its V6 origin/edition matrix passes; whole-suite runtime validation
is incomplete. Full E-R5-007 remains open: reached scalar pure-evaluator fast paths
and repeated aggregate const initialization require separate common-engine work.
No full cleanup, suspension/exact-once or run-once-template claim is made here.

The Atomic facade uses one explicit library RMW intrinsic in serialized GuestStore
memory, not methods special-cased in language evaluation. No guest-thread/weak
memory/Atomic128 claim is made. Its runtime behavior remains unvalidated because
the unchanged primary witnesses stop earlier. Optional inference must retain typed
unanswered call-signature goals rather than converting them into strict actual
call failures; associated/method proof coverage is still a prerequisite gap.

Read the bounded precode and all-eight-mode reviews in `worktracks/evidence/`
and `worktracks/04-calls-source-views/` before extending these areas. Keep removed
and remaining production routes explicit. Source/compile/component evidence does
not close architectural repair, tasks or the runtime acceptance gate.

## Source checkpoint boundary

The staged V6 checkpoint includes inherited and current compiled source, unchanged
licensed inputs and documentation. It is a work-in-progress snapshot, not runtime
acceptance or integration. The only byte change from the tested V6 Rust leaves
is removal of a trailing blank line from `lexical_numeric_callees_tests.rs`; no
Rust tokens changed. Ongoing semantic successors stay in the working tree. The
small shareable report is [V6 evidence](../../worktracks/evidence/2026-10-18-interpreter-v6-validation.json);
raw build/test logs stay local. All six focused/regression bodies passed; neither
whole selected upstream UI fixture passed.
