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

## V18: source constraints and pattern namespace classification

Exact source `88b73c9d329eb4b4fc3c4cc9729f224c0f5eebda133ee5b70fd903487ea65797`
compiles (lib `225eb8/0`, CLI `6e89d0/0`) with both-build freeze. Ten exact
compiled Rust leaves were staged before release. Primary six unchanged originals
all complete; additional named struct completes, named enum fails at Root pattern
compound field binding (17 calls / 204 dispatches). Newly passing numbered fields
complete both opposite substitutions (14 calls / 268 dispatches), the record
literal completes (0 / 72), and named fields completes (12 / 270).

Eight/nine controls pass. The actual cold alternative/range/false-guard retry
control completes all four editions. PatternTree now retains an optional canonical
binding for a plain name until actual namespace classification; constants never
need a local binding identity, real fresh bindings retain canonical/hygiene checks.
The source walker queries the actual occurrence and publishes only Some canonical
binding. Tree domains advance, without a second matcher. Generated origin control
continues failing: owner/cfg/record/epoch match; complete seal keys differ. Root
followup aligns cold boundary creation with existing GeneratedSyntaxInput::same_unit
and boundary currentness graph+epoch source correspondence, retaining its own full
completion seal validation. Foreign-world denial and standard macro context are
added as source-only controls, unexecuted until next candidate.

Worker1 next implements genuine selected-callee declaration scope for cold Type
equations without preparing its body. Worker2 next transports a reached nominal
field demand from the same non-consuming matcher to the shared member query for
generic enum patterns. Root owns generated boundary publication. These successors
are source-only; no integrated architecture/feature/milestone/JIT closure.

Thirteen distinct targeted cases have newly passed at least once across this
resumption's exact binaries against historical saved v16. This is not a current
whole-suite count; V17 numeric mains were not gratuitously rerun in V18.
[Exact V18](../../worktracks/evidence/2026-10-18-interpreter-v18-validation.json).
Source checkpoint `5761075884f60ea25053072be8db916108ffe57f` (exact V17 plus
scoped evidence/plans) was pushed to the implementation branch.

## V19: failed compilation preserved, narrow repairs pending

Exact source `99482a19550ba703be628080bb4557c97a0f2217ce34c2394924cd997fd858d9`
(3,327 leaves, HEAD `3feaeb81822666664570f54c47892c597e42dede`) failed the
library build, terminal `a8fba7/101`, 16.857 seconds. Seven errors are mechanical:
test helper module paths/private memo inspection, a missing manifest ItemKind
import, and a test cloning an ExpansionRecord reference instead of its actual Arc
handle. No CLI build or runtime commands ran. The failed snapshot remains attached
to its original source; test API/import/record_input repairs are a separate successor.
[Exact failed V19](../../worktracks/evidence/2026-10-18-interpreter-v19-validation.json).

The exact compiled V18 source/evidence checkpoint `3feaeb81822666664570f54c47892c597e42dede`
was pushed. Its index matches all 3,326 V18 compiled source leaves, not just the ten
changed Rust files. No unvalidated successor Rust was included.

## V20: reached nominal pattern specialization and cold declaration facts

Exact source `18f193dc916e43f77d5088666145052a511f9ab141f231daecbdf9308793f15e`
(3,327 leaves, HEAD `3feaeb81822666664570f54c47892c597e42dede`) passes both frozen
builds: lib `ebf17b/0` (29.246s), CLI `8c007f/0` (23.596s). All seventeen changed
Rust leaves, including the new route control, were staged against the manifest
before writer release; successor source remains unstaged.

All seven primary unchanged originals complete. The named enum newly completes
19 calls / 303 dispatches / 429 classified operations, including recursive Redirect
and Root patterns and the final assertion. Previous six primary passes remain.
Two additional unchanged originals fail: match-const-fn-structs stops at actual
VM-produced structural constant pattern comparison; const-struct stops in w's
record update after earlier operations. Neither is counted as a pass.

Six of nine controls pass. Pure cold Eq facts return U32 in both literal/call
branches across four editions, with actual SelectedCall equation evidence and no
callee body preparation. Nested heterogeneous nominal patterns pass their owning
2021 route/exact-once control. The all-origin initializer control crosses the
earlier seal refusal, then fails missing canonical item-kind recognition for N;
its exact unlabeled source variant is unknown. Cold runtime user-macro literal
case fails an actual token-context attachment; the ancestry control fails during
its existing discovery helper before boundary assertions. Those remain blockers.

Root's next code unifies existing original/generated declaration-kind metadata
for ordinary values and patterns and improves failure-only source facts; no
unsupported shape is treated as success. Worker1 extends genuine retained local
header Type scope, and worker2 implements ordinary record rest transfer through
the same constructor/copy/cleanup mechanisms. These successors remain unvalidated.
[Exact V20](../../worktracks/evidence/2026-10-18-interpreter-v20-validation.json).
Fourteen distinct targeted cases have newly passed at least once in this
resumption's exact historical binaries; no refreshed full-inventory count follows.

The exact compiled V20 source and scoped evidence were pushed in checkpoint
`0dc40e40125d9536b1da4115f427dd0c1078f2e8`. Current successors remain unstaged.

## V21: source-reviewed successor failed compilation

Frozen source `648c0af067ec62f738f5d3680da600a1002e2c444783c1d25e07cda07b6e67a3`
(3,330 leaves, HEAD `0dc40e40125d9536b1da4115f427dd0c1078f2e8`) failed the
library build, actual `e34c33/101`, 16.705 seconds. Five worker1 module/import
references and one worker2 exhaustive operation-capture classification join
need mechanical corrections. CLI and all planned runtime commands were unexecuted;
V20 binaries remain intact. [Exact failed V21](../../worktracks/evidence/2026-10-18-interpreter-v21-validation.json).
The failed source and prior all-eight review snapshots remain preserved; narrow
four-leaf owner repairs will be captured as a distinct successor.

## V22: common kind metadata, record transfer and actual lifecycle evidence

Exact source `85d8844fcca9ebd6ef3f34eab3fbb505e419dad75bed33558bbfe774afb3c2ac`
(3,330 leaves, HEAD `0dc40e40125d9536b1da4115f427dd0c1078f2e8`) passes both frozen
builds: lib `0d5b5a/0` 19.929s; CLI `786f91/0` 16.347s. All 32 changed Rust leaves
were staged against that manifest before release. CLI SHA `8b927161038a419602effdbd3de0750619983eadeb345d3b50f40b2df8cfe2bb`;
test SHA `eb032113259045b22b16e06952252107b85886a5f8fa507e72fea8ca75ae62a0`.

Twelve/thirteen unchanged upstream runtime contracts complete: primary10/11 plus
two additional FRU edges. New saved-failed witnesses are const-struct,
struct-update-syntax-2463, fsu-moves-and-copies and drop/issue-21486. These include
nested Drop wrappers, actual early-return event-log assertions, omitted-only
transfer and a cold base closure. expr-fn is retained historical pass evidence,
not a new gain. functional-struct-upd crosses its update, then fails formatting
catalogue source attachment; it is not a runtime-contract pass.

Ten/fifteen actual controls pass, including generated constant patterns all four
contexts/editions (64.193s). Main wrapper `ef7d50/1` ran13 bodies before a missing
wire selector; that error is retained. Only the two unexecuted selectors were
corrected from --list and run (`20c6c6/0`), no duplicate13 bodies. Main stock
`a99a69/0`; edge stock `cfcf16/0`. All binary hashes remain unchanged.

Actual failure facts identify a macro-definition diagnostic label315..331
(`macro_rules!emit`) passed as an identifier range; token containment correctly
refuses it. Another control fails the existing ValuePlace operand0 assumption
for a reached rvalue at a later operand. Ancestry setup is genuinely incomplete
(import/macro producer fixedpoint), before the wrong-graph assertions. The local
pure control's expectation of absent headers was false: ordinary coordinator
preparation legitimately publishes genuine local headers without executable
bodies. The unexercised unpublished-only header adapter is removed in the next
source candidate, keeping V20's existing SelectedCall Eq service; the control
will check actual metadata, stable inputs, no child body preparation and U32.

Next source-only repairs are actual MacroDefinitionShape AST name-range capture,
the shared exact indexed-operand verifier proof, and completed catalogue context
projection retaining the genuine pre-append provider/before rather than assuming
every catalogue has an original invocation storage key. No macro-specific
execution/inference rules or relaxed hygiene checks. [Exact V22](../../worktracks/evidence/2026-10-18-interpreter-v22-validation.json).
Eighteen distinct saved-failed targeted witnesses have newly passed at least once
across this resumption; this is not the current whole-inventory count. Aggregate
constant run-once templates/generic cache identity remain E007 holds: const w
being initialized twice here is not a duplicate FRU transfer within one activation.

### V23: actual macro names and record operand verification

V22 exact32 source leaves and evidence pushed as917fc1329be4b4b8210c7c154aa27e0382526fd2, preserving failed controls/unneeded header helper as historical tested WIP. Successor V23 removes the unused header helper/five production changes, corrects the actual coordinator lifecycle control (headers legitimately publish, executable bodies stay cold), uses real AST macro Name range, retains completed-catalogue actual prefix and validates the executed Operand ordinal. Exact source01d505d4c705f7178e28c8122a61e08c37376ed5a029362a53b3f5dbcfc8f7a8,3329leaves, buildHEAD917fc1329b. Lib93609b/0(12.675s,84warnings), CLI6150dd/0(8.953s,172warnings); test19ed1bdd9410946d83fd401233abb36eb2b699a841629e8b2b8c26238119539c, CLI1829bde400f0a0dfddc791e427caaef135cc69aa0a987baa403ec174c251eefc. All11modified+1deleted Rust leaves exact-index-staged before writer release; binary hashes unchanged through runtime.

Stock85f9fe/0 is4/5 actual runtime contracts (const-struct,fsu-moves-and-copies,drop/issue-21486,expr-fn retained), zero new stock gains. Functional-struct-upd still fails, now generated declaration macro module absent in Debug::fmt facade/core/fmt.rs38448..38473, after crossing prior missing producer/prefix refusal. Controls f7f69e/0 actual6/9PASS: corrected local pure SelectedCall Eq/header/currentness/coldbody proof and all4editions record-update ownership/300elementCopy/source+macro+wrapper+assert/exactonce routes newly pass. Generated origin matrix, existing pureEq, prefix lower-level and sourceDeref retained. Old and local cold runtime macro controls now fail missing structural occurrence proof (token-range fault crossed); ancestry still Incomplete before wronggraph negative assertions. No new upstream fixture/gate claims. Raw reportSHA49f4db6490cfa2dc2f7ae4fbf7233773099dee161686fcbde9c5b378bdbf475b,35file index8c8e52f4c7168e206eae9ce88dadf4358d1bc33e50b0b77b72ff524d931f1ed1. Shareable worktracks/evidence/2026-10-18-interpreter-v23-validation.json.

Next independently owned repairs: root structural constant comparison in the existing Search over actual evaluated cell/ticket; agent1 actual declaration import-target hygienic selection with retained lookup index; agent2 actual retained syntax occurrence/ancestry reference proof. All source-only until exact build+runtime. E007 aggregate templates/once-per-item+instantiation/generic keys remain held; the const-struct two uses still initialize twice. No JIT work begins.

### V24 compile failure retained, one-line V25 correction

Frozen20leaf candidate V24 source2784f7554f0ecd1a0bd2dcc27b2938767af9988c0fbc0fc1d6213238abaa94b5,3330leaves,HEAD5fe70d59298508da56ae4062cdcd427138ed4bd5, failed lib9d4ac9/101(16.421s,11warnings) on E0624 only: SourceDeclarative3211 calls GeneratedDeclarationMacroIndex::producer private at impl_index1425. No CLI/runtime/test body ran; source/generator/HEAD remained unchanged. V23 binaries stayed intact. Raw reportSHA5318ced933bcaa8624d35a9401a9130cc10716073d5138b730a2edf9b4ad9642, rawindexdffe7964afcd7b47f5b138f3b0f86c164a7480fb33d21029b469ccaa28459040. Exact shareable worktracks/evidence/2026-10-18-interpreter-v24-validation.json.

One-leaf successor only makes the intended checked retained-scan producer accessor pub(crate); no external authority/API or lookup changes. impl_index2ecd6eb657a860bc6c59bad7ebc2d49781a3c0e9786a0cac451414dce6f504f8, receipt worktracks/evidence/2026-10-18-declaration-macro-import-alias-v25-mechanical.json. Independent review and fresh frozen builds required; no pass inferred from repair. All other candidate source held at exact V24. V25 runtime queue remains five unchanged stock roots/nine controls, not a whole-suite refresh.

### V25: six actual new upstream runtime contracts, held architecture controls

Source4d7efa672dcc84f9ce330990ed8ae254c3320e285e628412cefcd6ff65e5b0e8,3330leaves,HEAD5fe70d59298508da56ae4062cdcd427138ed4bd5; only impl_index checked accessor visibility differs from failedV24. Lib21e67f/0(30.115s,83warnings), CLI8801b9/0(24.601s,171warnings), both source/generator/HEAD unchanged. Test05f9bb8126712d744d6c6e6bb81b67b8c747470482947e8aab21b504b4c893c4, CLI97c7a9c7095ccc3cacece5179e3517f2219d3881274e1099557a48e6fb6f9ced; binaries unchanged through all queues. Exact20Rust leaves index-staged before writer release; successor edits stay outside that source.

Primary007bbc/0 is4/5 upstream runtime contracts: new match-const-fn-structs, cyclic-const-dag-pass and functional-struct-upd, plus retained expr-fn. Functional capture stdout is Foo { x: 4, y: 2 } newline. Custom-eq-branch-pass fails observed structural constructor cannot replace its source result context at main slice [CustomEq] construction(1call35dispatches); source-selected nominal members in structural/slice contexts remain a common prerequisite. Additional3 originals867112/0 allPASS: struct-partial-move-1,-2,struct-order-of-eval-3 with real move/effect/order assertions. Combined7/8, six new saved-failed runtime witnesses; ever-new targeted history24 across differing binaries is NOT a current suite total or full-fixture count. Do not blanket-enable restSome inference: these three originals already pass, while type-changing rest source relations remain held.

Controlsde639a/0 actual5/9 bodiesPASS. Existing scalar cold/retry, initializer origins, corrected localpure, all-editionFRU and retainedgenerated sourceentry pass. New structural control completes2015origin0 source equality/inequality/owner assertions but opcode19 is4 expected3; the exact-once assertion fails and remaining origins/editions do not run. There is no retained per-call trace (helper uses capture disabled, deletes its temporary), so attributing fourthcall to initializer replay is unproven; per-use aggregate cache key remains independently source-confirmed E007 debt. Keep expected3 unchanged. Global/local generated coldcall controls cross prior occurrence proof then panic value>0 for2015route1 coldfoo branch; literal-only branch advanced. Associated ancestry control now stalls its earlier new impl+localmacro case on producer setup selfcycle before alias and negative checks; no Someassociated/alias authority acceptance.

Raw reportSHA79944183e2bc21c008809f1c787bd26adc71136207a8e3611b4dbc9f2b979343,48file indexe08f08ab524f15b07a136cd81370d724222cfaa6a5be8b79669a92d7f0f8bb7b. Shareable worktracks/evidence/2026-10-18-interpreter-v25-validation.json. No diagnostic/tooling/full-fixture/architecture/interpreter/JIT gate closes. New source-only successors: worker1 actual Const/Static owner binders oneleaf reviewed; worker2 generated puregoal diagnostic/control extension; root common source-selected structural context repair under investigation, no production patch yet. Remaining actual generic initializer/template prerequisites in worktracks/04-calls-source-views/2026-10-18-constant-applications-and-templates.md.

## V26 focused prerequisite checkpoint

Exact source `dfcb89188427a17e67e82f63adac9774840ade7bae881cc3ee4d60032339b9cc`
compiled both lib (`6842bf/0`) and CLI (`6a7f94/0`) unchanged.
**2/3 stock runtime contracts and 4/7 controls pass; zero new stock pass.**
The actual source structural context fix crosses the singleton nominal array,
then custom-eq-branch-pass fails the EMPTY `&[]` initializer. The common source
return-goal controls still report Pending for the generated caller binding;
associated ancestry crosses impl setup and fails trait setup. The all-edition
owner-binder metadata control passes; that is not associated-const main execution
or full GenericArg transport. Three V25 controls without a source fix were
not rerun: the unexplained fourth call and two runtime cold-call numeric cases.
Their failures remain held, not skips converted to passes.

Four compiled Rust leaves are checkpointed separately from successor work.
Root's retained actual WrittenType result-hint transport, local macro event
identity bridge, and actual trait producer lifecycle repair are source-only
successors. Preserve their exact review/validation boundaries. No aggregate
template/runonce/full generic cache, source authority, architecture, runtime
acceptance or JIT gate is closed. [Portable V26 receipt](../../worktracks/evidence/2026-10-18-interpreter-v26-validation.json).

## V27 actual runtime and source-goal progress

Exact source `cd6f8e7eea60a6c1c040f9aa192bedb6ab60903da5347a64e9ddc391157caf68`,
3330 leaves, compiled unchanged at HEAD68195da3483d755478002f67c9eb8304c32787d6
(lib60acce/0, CLI96663b/0). Exact twelve compiled Rust leaves were staged before
writers resumed; sealed binaries stayed unchanged throughout all queues.

Primary unchanged roots **3/3 runtime-pass**, with one new witness:
consts/const_in_pattern/custom-eq-branch-pass.rs now crosses main singleton and
EMPTY initializer failures and completes its actual branch behavior. Struct
update and match-const-fn-structs retain passes. This is 25 ever-new distinct
witnesses across separately validated binaries in the resumption, not a current
suite total. **10/11 controls pass**. Both global/local pure goal matrices and
both ordinary/user/wrapper/stdmacro runtime matrices pass every tested edition
and cold-branch route. Associated owning ancestry now reaches/passes direct
impl/trait, alias and negative scope proofs. Nested-child trait completion is
still unimplemented and was not exercised by that matrix.

The failing lifecycle control asserts an actual macro invocation remains a
producer before its declaration job completes; actual producer was retired.
Its later completion assertions are unexecuted; source review must establish
whether the assertion confuses expanded macro retirement with the separate
typed declaration frontier. Do not weaken that oracle without actual proof.
The unexplained fourth-call V25 control remains held and was not rerun.

Two separately approved trait originals yield1/2: item-inside-macro passes,
default-method/macro fails INTERNAL-SNAPSHOT at actual String::new Vec::new()
(alloc/string.rs522..532). Both passed historical raw-v16, so this is a current
regression relative to that old binary; its introducing source is unproven.
A separate associated-const-type-parameters original fails Self::X in the real
Foo::get_x, before later generic Proxy applications. No partial program pass
is counted. Failure-only key/stamp diagnostic candidate is source-only next.
Root common Copy guard and worker source application carrier are also separate
uncompiled successors, with explicit full-template/application/cache holds.
No full-fixture/architecture/interpreter/JIT gate closes. [Exact V27 receipt](../../worktracks/evidence/2026-10-18-interpreter-v27-validation.json).

## V28 common Copy guard and actual stale-input evidence

Exact source9b9bdb78711adc79375259d4a0c3de41ac052eda1cae889b5d1dc25bbc374493
(3331leaves) compiled unchanged at4ad527499a6dd5e23752476669c2c7a9eb4b7eef:
lib177c3a/0,CLIa28f47/0. Exact10sourceleaves staged before writer release.
All12controls pass: eight existing Copy ownership/resume/source-epoch controls,
three prepared-input guards and the cached numeric pure-failure control.
The physical Copy engine now uses real common ConstructionId guards and common
field transfer/commit, removing its separate ownerless-object publication path.
Public Copy admission/source stamps remain; no non-Copy template purpose is
enabled, and full E007 cleanup/application/runonce integration remains held.
The new associated-constant occurrence/provider carrier is source-only and
keeps semantic application Pending until genuine Type/substitution evidence.

Four of five affected originals retain complete runtime captures; zero new
stock passes. Default-method/macro still fails, now with an exact changed
InputKey: Vec::new BodyEnvironment92f6561ef805853a1b50cb026ad1efac8cccfb8b34fbae3c84225a90ea28de3d
for bodyfeb51f1b9a1fea793ae4c5813055829c2cc92c7c984687197632fa8d7600e72c,
expected missing stamp1cc671930104d41f2e86a825a1705770517814e6f8af0e195fa095bf0c3d2d12
to publishedf2546ba072a00618a88f93f5d553f54516b1b0543e8ea54be6d67e6b89f16092.
Actual declaration-only empty-bound preparation is a separate successor; no
blanket renewal or weakening of currentness guards is authorized.

The V27 lifecycle failure is now source-confirmed as production double
completion: no-catalog pending output retired its macro producer early then
attempted another completion after declaration work. The one-branch successor
delays no-catalog completion until the real pending receipt completes, retaining
early known-catalog publication. Existing assertions stay unchanged; source
review is clear, runtime validation pending. Self::X requires actual implicit
Self qualifier binding through ordinary source/type/member services, with its
new memo/consumer work separately reserved. No full-suite/architecture/JIT gate
closes from these components. [Exact V28 receipt](../../worktracks/evidence/2026-10-18-interpreter-v28-validation.json).

## Successor source review before V29 (unbuilt)

Actual interpreter goal remains active, no JIT started. No-catalog single completion and declaration-empty-equation canonical/opaque refinements have independent source reviews. Self qualifier source trait/bound identity is preserved; an anchored Type regression is being repaired through the existing indexed actual-Type selector, not a legacy-mode bypass. A single common initializer-entry packet hold prevents generic/default scalar cache erasure across both value and pattern demands; actual full Type/Const/Region application remains unimplemented.

Root private template code now admits only owned intrinsic structural graphs, with actual StoreSession/epoch/root/field ownership and same AggregateCopy/ConstructionId/ValuePath continuation. It declines owned nominal nodes. Two independent reviews found first stale published Drop rows (fixed via shared live member-world handle), then incomplete initial Drop absence from cold-function non-local impls. The latter is still a correctness prerequisite: no new absence proof is retained, no nominal once-cache claim. Prior reviewed proof candidate is retained as held evidence. Existing nominal per-use behavior and expected3 structural CallSeal oracle unchanged; no terminal yet for this source. The validator owns Cargo and prepared seven unchanged upstream originals plus focused LRR authority/lifecycle/promotion/Copy controls, not launched. Keep root and worker1 ready leaves stationary until worker2 final proof review and exact-source freeze.

## Actual V29/V30 terminals and held source checkpoint

V29 source8231fd90 lib no-run terminal0b3989/101 failed in17.058s on three compile errors (pinned TypeBound API, missing HasGenericArgs trait import, moved template Option). CLI/runtime were unexecuted; failure preserved. Mechanical successor V30 source7a15c2b92ea4b245d9f9bb2c9ac50dbbf5417148ac98992e78d5b3ae03201ffc built from HEADb1fe6ef6f8fa43617d18c7b7f50e346b6768c111: libf1c196/0 30.722s and CLI31fc7f/0 25.201s. Test636c3b74/CLI759af305 remained unchanged for actual runs. Root staged all29 exact compiled Rust leaves before releasing writers; raw index audite24bad4e.

Stock queue21104 terminalb968c5/0 is wrapper completion, **0/8 actual runtime passes**, all fault with invalid macro identifier `$crate` edition2024, no successful complete capture. Controls95636 terminal49b000/0 is wrapper completion, **10/12 actual control bodies pass**. Real producer lifecycle now passes all four editions; new published Drop and cross-store template controls and selected Copy/currentness controls pass. Existing P structural expected3 still observes4; promotion fails same `$crate`. No new stock pass or refreshed fullsuite count; historical ever-new25 stays attached to older distinct binaries.

Worker2 common repair now owns typed actual DollarCrate occurrence/hygienic defining-unit root through existing qualified Type/module lookup; no global MacroName relaxation, Pending fallback or macro evaluator. Worker1 is factoring a single declaration-only lexical scope collector from the entered route, still source-only. Root will own common initializer application/cache transport, still held pending real source packet. New successor Rust must remain unstaged until its own exact review/build; V30 index is the held compiled checkpoint. These are work divisions, not acceptance. See compact V29/V30 evidence in worktracks/evidence.
