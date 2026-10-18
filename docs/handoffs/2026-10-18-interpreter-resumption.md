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

## Current V9 checkpoint

Exact compiled source `462e11a293d49c1f11592251cfcde6d7c4227f187a7d73e880176019f6940c5a`
passes both builds (lib `b83b99/0`, CLI `4d5f5b/0`) with source unchanged.
Six selected controls pass and four of seven unchanged whole-main attempts
complete. New since V7: const-int-arithmetic (134 calls / 18,352 dispatches)
and associated-types-project-from-type-param-via-bound-in-where (49 / 722).
MIN and Vec autoderef remain passing. All four were faults in saved v16; this is
a targeted delta, not a refreshed global total. Whole i128 fails VALUE-REPR;
u128 fails structural generic binding. Proper issue-17718 aux preparation stops
on unresolved Ordering import, before runtime. [Exact source/evidence](../../worktracks/evidence/2026-10-18-interpreter-v9-validation.json).

V8 lib returned exit 0 but a source changed during compilation; its freeze is
invalid. No CLI or runtime acceptance is attributed to V8. V9 explicitly froze
all writers through both builds. Source-only successors (overflowing literals,
actual mismatch diagnostics, retained-origin diagnostics, generic field reads)
are separate from the staged V9 source. E007 still lacks nonprimitive run-once
templates and generic cache audit; pure constant patterns remain held debt.
No full milestone, interpreter runtime gate, JIT or full-fixture closure.

Current implementation branch checkpoints b6aa9706bc72d4a5c8f69e30fe7124992f23d8fd
and its predecessor 9d4c588ea279ca3c25ff19d99b4e4cdd5b21b112 were pushed.
Only source, fixtures, plans and compact evidence were included.

## V10 numeric and diagnostic followup

Source `0a21e2a8fdb73527f37dc3de2fd26964729790a005ee570371d5699092177ff2`
compiles unchanged (lib `e5a6ad/0`, CLI `53e33b/0`). Six selected literal,
arithmetic and retained-origin controls pass. No new whole-main pass: i128 now
reaches 81 source calls / 31,833 dispatches, u128 reaches 70 / 15,995; both fail
the same reference/nominal type contract. Shared Unifier diagnostics retain actual
terms; retained token origins now map the reached equality to its real standard
macro definition. They do not prove the original caller expression location.
[Exact V10 evidence](../../worktracks/evidence/2026-10-18-interpreter-v10-validation.json).

Successor source adds ordinary reverse string comparison impls (the missing
library direction, without loosening unification), 128-bit primitive/fold trait
impls, guarded nominal member specialization, and failure-only ordinary import
resolution evidence. These remain unvalidated until a subsequent exact build.
Current upstream gains remain four targeted whole-main runtime passes, not a
fresh global result or full-feature/interpreter/JIT gate.

## V11/V12 retained failure and correction frontier

V11 lib exited 101 (`e46fff`) from the NominalMember provider arm return type;
no CLI or tests ran. V12 changed only that provider control-flow join and compiled
exact source `a02f33176cf1a277a32bb6b27ca90c959ce5a99607de2e95e80b7b5e88c7bd32`
(lib `2b504c/0`, CLI `883381/0`). Five/seven controls pass; zero/six upstream
whole-main attempts complete. The source-Deref and via-where regressions hold
integration. i128/u128 pass the old string comparison frontier but fail a new
constructor-context result-slot requirement in a valid synthetic pattern call.
The initial review covered ordinary Expr calls and missed this other call family;
its correction is preserved. Numbered fields reach a bogus Struct-as-Fn signature
demand; named open record construction remains unsupported. Properaux imports
still stall despite a currently defined Type target.

The next unstaged successors share optional context across synthetic sites, keep
nonprojection Params for anonymous runtime arguments, classify actual Fn AST
occurrences in both source-callee paths, retain full GenericArg member identities,
and refresh current import results before declaring discovery incomplete. These
are source-reviewed candidates, not passes. Exact staged V12 is preserved as a
held source checkpoint separately from those dirty successors. See [V12 evidence](../../worktracks/evidence/2026-10-18-interpreter-v12-validation.json).

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
That is historical evidence, not a current-source/full-fixture count. At the historical V6 checkpoint, no new whole-main runtime gain had been
verified; V7 and V9 gains below supersede that dated observation.

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

## V7 — first verified upstream gains in this resumption

V6 WIP checkpoint `9d4c588ea279ca3c25ff19d99b4e4cdd5b21b112` was pushed to
`origin/codex/lrr-implementation`. Its source-only checker, plan check and staged
whitespace check passed. No PR code was fetched/applied.

V7 source `e7f91cd60edcdc9d80648bbd657dec6ef93ea9bb8772d350ce832d2d4907a449`
(3,321 Rust/Cargo/vendor leaves; generator separately hashed) built without source
changes: lib terminal `46c359/0`, CLI `7706c2/0`. Test binary SHA
`7b4566c5342548b9da9bca9f4443a5d0e55ac1a68fb04ec11b44af0b34a9f280`;
CLI `1ca2bb6d0739555f1c6181054926785cc4512f9cf20dc221e73b002c5609299b`.

All six selected controls pass, including the corrected genuine derived-scope
dependency agreement and duplicate-primary refusal. Two unchanged upstream whole
mains now have complete runtime captures and were failures in saved v16:

- `numbers-arithmetic/i128-min-literal-parses.rs`: seven operations/five dispatches.
- `autoref-autoderef/autoderef-vec-box-fn-36786.rs`: eleven source calls, 82 dispatches,
  116 classified operations, 43 cleanup elements.

These are **two targeted new distinct runtime-contract passes versus historical
v16**, not a refreshed global count or full-suite acceptance. Three selected
unchanged owning-2024 atomic library bodies pass: bool NAND and the two invalid
load/store ordering panics. Each selects one of 42 catalogued bodies; the other
41 are filtered, not passes. Their expected-panic path requires an actual guest
panic containing the upstream expected string; faults/missing inputs do not pass.

Four other upstream whole-main captures still fail: i128/u128 programs need the
actual `test` crate preparation (`test::black_box`); const arithmetic reaches 63
calls then the existing syntax primitive-hint map lacks u128; via-where reaches
45 calls then generic record constructor binding is incomplete. Current source
repairs for those gaps are separate unvalidated successors. No 128-bit full
feature, constant-template, full-fixture, architecture, milestone or JIT closure.

[V7 shareable evidence](../../worktracks/evidence/2026-10-18-interpreter-v7-validation.json),
raw report SHA `31401a2fcebf99d2355a16fc674c0e678a0434db2b3ffc3c1a3e8ad50f0e02bc`.
Raw logs remain local. Existing global 1,429 result is still only its older binary's
historical survey; report it separately from this targeted delta.

## V13 frontier and V14 candidate

V13 exact source `5110ee4d0fd8acf2c4eba42e7eac7bb8ca72c7f187c7af4ab44d6d16ff5a3a53`
compiles (lib `1e1528/0`, CLI `9f815c/0`) and passes seven selected controls
(`2b8781/0`). Zero of six unchanged originals complete (`5ddb19/0` is the
reporting runner terminal, not six guest passes). Source-Deref and alias/reference
controls are restored; via-where remains regressed. The i128 main reaches 835
source calls and lacks the ordinary receiver result context for `None`; u128
reaches 721 and lacks the generic result context for `Sum::sum(self)`. Numbered
fields now reach foo(&node), where its associated-equality result parameter is
unbound. Named/reference and via-where records need ordinary pending field
specialization. Proper issue-17718 and its actual auxiliary cross preparation
and stop at C3(), a const fn-pointer callee. Exact failures and hashes are in
[the V13 report](../../worktracks/evidence/2026-10-18-interpreter-v13-validation.json).

V14 freezes the reviewed shared primitive binary inference, concrete/written
field callback join and pending record activation, ordinary item-value callee
lowering, and failure-only trait result diagnostics. Source review found no
blocker; runtime results remain pending. Root owns the item-value route, worker1
primitive/result constraints, worker2 record binding, and the validation agent
solely owns Cargo. No guest JIT work or current global pass total is claimed.

Source checkpoint `85d7d64eafbbbcd8c87ee783b641e0a076bbaa04` (exact V12, held
with regressions) was pushed. V13 twelve Rust leaves were staged separately
from corrective successor edits, retaining the exact tested snapshot.

## V15 actual runtime progress

V14 failed compilation (private diagnostic field); no CLI or tests ran. V15
fixes only that diagnostic accessor and compiles exact source
`53e16d94dc8bd8fb6eab7a87dfb8f92eef091cfbca75c08a3d4e1e4514bfa476`
(lib `65e968/0`, CLI `32878f/0`). Six/six selected controls pass; main six
originals produce two whole-main passes and the separate two field originals
produce one. The new passes are associated-types-ref-from-struct (8 calls /
108 dispatches) and associated-types-normalize-unifield-struct (2 / 19); via-where
is restored (49 / 722). All three complete captures and cleanup. This adds two
distinct targeted gains against saved v16, without refreshing the global suite.
V9 four gains remain attached to V9; no unrun current control is assumed.

i128 now crosses None inference and reaches 860 source calls before Sum's
expected result is lost. Actual GenericEnv binds i128/u128, but shared
Frame::numeric_hint_value forwards only IDs 2..15, dropping 17/18; root fixes
that ordinary forwarding guard next. issue-17718 crosses C3() and reaches 23
calls, then its actual C1 match pattern enters the old pure constant route and
fails source admission. That pattern route must demand the reached initializer
through the ordinary VM, without executing constants in cold alternatives.
Worker1 handles actual associated-equality call inference for numbered fields;
worker2 handles source constraint inference for an unannotated Thing record.
Root also corrects intrinsic Copy constant storage using the existing descriptor,
including fn/raw pointers. These successor edits remain unvalidated.

See [V15 evidence](../../worktracks/evidence/2026-10-18-interpreter-v15-validation.json).
Exact 24 compiled Rust leaves were staged before writer release; no successor
source is attributed to those results. No interpreter, architecture or JIT gate
is closed.

## V17: full numeric mains and ordinary VM constant patterns

Exact source `7a2253f1994777663f2236bdabddee99b6c656738a6c7145ae98691200df6fe0`
compiles (lib `50986d/0`, CLI `67f527/0`) with both-build freeze. Thirty-two
exact compiled Rust leaves were staged before writer release. V16's six mechanical
compile errors remain recorded; no V16 CLI/tests ran. V17's unchanged originals
produce 7/9 runtime-contract passes: four new versus saved v16/V15 (i128 892 calls,
u128 753, proper issue-17718 + auxiliary 50, fresh const-meth-pattern 1) and three
retained (ref-from-struct, via-where, normalize-unifield). Complete captures and
closed cleanup are recorded. The two failing mains are numbered fields (U32/I32
constraint mismatch after equation entry succeeds) and an unannotated record.

Selected controls pass 7/9. The failed all-origin and cold/range/retry controls
are acceptance gaps, not ignored failures. Actual potential constants in or
alternatives were rejected as unresolved syntactic bindings before namespace
classification; root successor retains an optional canonical candidate until the
actual Value selection establishes constant versus local. Generated body boundary
failure needs exact owner/seal/epoch/cfg/record facts; root adds failure-only
diagnostics without loosening checks. Worker1 shares equation inference with
ordinary source result constraints; worker2 joins the existing prepared Record
recipe to the common open-expression fact/hint adapter. All successors are
unstaged relative to V17 and unexecuted pending V18.

The obsolete runtime constant_item_value/constant_item_data helpers are removed.
One non-consuming pattern search now reports a typed demand only when it reaches
a missing constant, ordinary VM initializer frames satisfy it, and the same
operation resumes before transactional publication. Pure preparation queries and
historical SourceConstant operation models remain separate held debt. NonCopy
run-once templates, generic initializer cache keys, structural constant equality,
full origin/suspension matrix and interpreter/JIT gates remain open.

[Exact V17 report](../../worktracks/evidence/2026-10-18-interpreter-v17-validation.json).
No refreshed suite total or full-fixture acceptance follows from these nine cases.
Ten distinct targeted cases have newly passed at least once versus saved v16
across this resumption's historical binaries; that is not ten confirmed passes
on every successor or a full inventory delta.
