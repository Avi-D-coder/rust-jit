# Real static-alias runtime failure and ordinary call admission join

V533/V534 read-only preparation, 2026-10-20. LOCAL049 / M018/M025/T053;
production remains frozen pending validator release and fresh runtime baseline.
The user requires connected interpreter changes driven by unchanged stock
execution, not another disconnected helper/source-only acceptance result.

## Actual failing original and historical terminal

`fixtures/upstream/rustc-checkout/tests/ui/type-alias/static-method-type-alias-11047.rs`
is the selected unchanged owning-2015 `//@ run-pass` witness, pinned upstream
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, SHA256
`bbd1a7b205e796bf377489b0c151ee14988c8beaac045d32c77f12403d9ff68b`.
Its complete main calls the actual nested Qux::new directly, then Ham::new via
the actual block alias `type Ham = foo::bar::baz::Qux`, then StringVec::new via
the actual applied alias `type StringVec = Vec<String>`.

Durable evidence
`worktracks/evidence/2026-10-19-v407-v414-associated-qualifiers.json`,
results.runtime, records V414 source
`7099e83cc09f72b98155d0dcec845adb4ab0d9e7cf95e8ae546c2e67e7dad305`:
owning2015 exits 2 after 9 dispatches/8 outcomes/one source-call boundary,
`LRR-E-UNSUPPORTED: lexical item callee requires body namespace integration`.
Capture is incomplete, faulted and has no observed cleanup. All four edition
rows have the same failure. Recorded owning2015 argv:

```
LRR_EMBEDDED_FACADE=1 target/debug/lrr --edition 2015 run fixtures/upstream/rustc-checkout/tests/ui/type-alias/static-method-type-alias-11047.rs --runtime-capture --capture-context a6d17fcbdf314e818d0c2ce46afade8c97dc3f62646b0d7843d338e1beea362d
```

That context belongs to the historical evidence, not a current-source result.
Root asked the validator to execute the same unchanged original in frozen V534
with fresh source/HEAD/binary/input guards; that fresh baseline is pending here.
No runtime, Cargo or Git command was run by this worker. The older V421
fn-item-type-coerce failure is not a valid new repair target: V423/V424 durable
evidence already records its actual complete all-four-edition runtime pass.

## Current ordinary path and first missing connection

`db::entered_call_thunk::thunk` selects the real indexed CallExpr only for an
actually admitted BodyEnvironment, asks Q44 EnteredCall, and installs the normal
source-call recipe only for admitted Local or Module declarations. Its normal
Q47 PreparedExplicitTypeArguments holds the same actual target and guard.
`engine::RunMachine::select_common_argument_call` consumes that installed
target, calls `finish_selected_argument_call`, and enters ordinary
CallPrepare/CallSeal/contracts/body entry and cleanup.

`ordinary_local_callables::admit_entered_local` explicitly returns
Pending(PathAdjustment) for qualified and rooted Value goals. Q44 has no actual
associated source target variant. For Ham::new, the fallback normal lowerer
therefore has no source_goal; `syntax/thunks.rs::call` detects the actual local
TypeAlias in its containing statement lists and returns the exact historical
unsupported message before the associated call's ordinary VM operation.
The original's direct Qux call and then Ham alias order is consistent with the
recorded one earlier source-call boundary; a fresh trace must confirm the exact
active source range before implementation.

Q51 `PreparedAssociatedItemQualifier` already shares actual entered source and
Q73 PreparedBodyTypeApplication. It retains the normalized source TyTerm,
actual qualifier/member syntax, actual parent SourceLocalPlan and source query
dependencies. Prior Q51/Q73 source controls establish component facts only;
they did not remove the above runtime admission boundary. The legacy
`select_lexical_associated_call` also passes the selected root declaration ID
directly to associated_of; a TypeAlias ID is not its normalized nominal owner.
Using that fallback would lose the full StringVec target application too.

## Smallest connected production repair after baseline/release

Extend the existing Q44/EnteredCallTarget admission with a private actual
associated source target. Prepare it from the exact callee Path occurrence's
Q51/Q73 qualifier Type plus ordinary member service and canonical selected
function/header/signature facts. Retain its actual source owner application,
selected member identity, caller index/phase/range, relevant query guards,
edition/context and generated source receipts. A source callable is only a
checked declaration/body locator here: no callee BodyStructure, thunk, guest
execution, effects or live compilation is permitted during pure preparation.
Actual member lookup and callable construction must reuse the existing common
CallResolver/source admission machinery; do not invent a synthetic Value goal
or mislabel an impl-owned member as an ownerless module function.

Q44 `signature`, `retained_target`, `retained_plan`, `indexed_source_name` and
currentness projections then cover that same genuine associated admission;
Q47 and `db::entered_call_thunk` consume it through the existing source-call
path. Preserve normal indexed written-name comparison in
`initial_thunk_with_source_call`. Because an actual proof now reaches the
lowerer, it naturally bypasses the legacy no-proof alias refusal. Do not delete
that guard or enable module-by-spelling fallback. Installed execution consumes
the retained target through `select_common_argument_call` and the same
`finish_selected_argument_call`, call contracts and VM packet owner.

Preserve the complete normalized qualifier: Ham resolves to Qux, and StringVec
resolves to applied Vec<String>. Do not retain only a nominal/alias ID or lose
String. Its actual applied owner must reach ordinary generic call preparation.
Any required impl-owner relation/inference belongs to the shared pinned
upstream relation owner. Existing `run_owner_arguments::{match_owner,
owner_bindings}` is legacy migration debt, not a new independent pattern
matcher to copy/extend. Trait-dependent/ambiguous selection requires authentic
ordinary member/upstream preparation; neither candidate cardinality nor a
method-name rule may mint this target. Missing required source/member facts
remain typed preparation demands through the ordinary continuation owner.

Likely implementation boundary is existing entered_call_goals/target,
ordinary callable admission, Q47 and db entered_call_thunk, with the smallest
actual owner-application consumption hook in ordinary engine call preparation.
Exact file ownership must be assigned after fresh baseline because root is
currently changing run.rs/run_call_inputs for repr128. No implementation has
begun under this record, and no unrelated Q73 helper changes are requested.

## Runtime acceptance and architecture boundaries

First reproduce the original main failure with actual terminal and source
range. After the connected repair, rerun complete unchanged main; preserve every
direct/alias/applied-alias call. Require complete unit termination, empty guest
streams, observed cleanup and unchanged source/input/binary guards. A later
failure at StringVec or generic owner transport is the next real production
prerequisite, not a pass or permission to slice out that call. Existing
struct-aliases and fn-item-type-coerce originals are maintained regressions.
Actual original/user-emitter/wrapper/nested-standard contexts, relevant-source
withdrawal/epoch and exact-once suspension controls remain required; new
source-only helper assertions cannot substitute for execution.

Eight-mode design constraints: (1) same ordinary CallExpr lowerer/VM, no second
evaluator; (2) no macro-only literal route; (3) source Type/member selection with
upstream semantics, no alias/method/primitive whitelist; (4) generated and
handwritten occurrences share Q44/Q51/Q73 and source receipts; (5) no pattern
duplication or unselected executable preparation; (6) memoized cold selection
and immutable retained target, no repeated planning at call phases; (7) relevant
source/member/query guards, no full-world dispatch census; (8) shared Arc plans
and indexed syntax, no BodyIndex clone or whole-file lookup. This is a design
assessment; final production source and actual runtime must be reviewed again.

Inspected current hashes: thunks.rs
`d5dd60d3b27920a81eb9798f7b79f993742892f0a6d03ec5628cf7346e9e1bbe`;
db/entered_call_thunk.rs
`46fefabbc6855fc2087e5925c7bac108844c81b49825d0d0679be82e2d29b5ba`;
entered_call_goals.rs
`c6df31b92f24557500789fcbdd98d83bc3283a3afe271a0b56267b425457514e`;
associated_constant_applications.rs
`942a961d07e46ab492f8dff894aa69778a05448fa7ada864b96bc9c32d45cbb9`;
body_type_applications.rs
`c6dd5938f8e7f85c925f87c89fe2338f1042c932c6a1e5a28fa97faa091f008d`;
run_nominal.rs
`1e5ee6a73918c1dd2d0b562b8bfd253b50de0842976904f50d37f73853f32747`;
run.rs
`1942c42579b40fa8a8ad277926a68bd5de9d90363712eec95c33f481757c19c0`.

## Fresh baseline and connected production candidate

V534 `worktracks/evidence/2026-10-20-v534-interpreter-runtime.json` confirms
owning2015 unchanged main at source
`3924de4bdde042d68dbccaa8dbccd2502ad380a6a007a200e17da3241d616a19`,
HEAD `a7e64f338d2c23dde07383a2e730b8324cca4019`: exit2, 9 dispatches,
8 outcomes, one earlier source call and the same unsupported message. The actual
fault range is original bytes478..488, exactly `Ham::new()`. Closing source,
HEAD, binary and digester guards pass. Root released edits after terminal
checkpoint `08c177d`; the existing LOCAL049 scope now records this shared
production reservation and full-original acceptance requirement.

Implemented Q44 `Selection::Associated` / retained target admission stores the
actual Q51 query Arc and guard, actual ordinary member-selected SourceCallable
(declaration/signature/body locator only), and genuine FunctionSignature guard.
Its parent is borrowed from the retained Q51 value, without an additional hidden
parent-plan Arc. Only an actual inherent declaration bucket enters this join;
trait-dependent applicability stays on the ordinary trait/upstream path. The
selection uses existing CallResolver::associated_of, not a new candidate matcher.
Q44 checks exact parent environment, signature identity and selected callable
currentness. Q47 retains that same target and real full closure; its reached
projection revalidates the same actual callable before cloning immutable
declaration facts. There is no executable body index or guest work in this
preparation.

`db::entered_call_thunk` now installs this admitted target through the SAME Q47
source-goal lowerer. No syntax/thunks guard or module fallback was deleted.
`EnteredCallGoal` stable domain is v6 and retained-call-target is v2. The normal
indexed written-name/phase/body comparisons still apply before source_call
installation. Exact source key/query guards remain separate from actual VM
once-consumed call packets and root authorization.

`PreparedExplicitTypeArguments::associated_source` exposes only the retained
target's actual complete qualifier. `ExplicitType::nominal_arguments` decomposes
the retained WrittenType::Generic (or FrameTemplate) alongside its source term
with exact nominal ID, full count and Type-kind agreement. Each argument keeps
its genuine written/schema proof; missing/mixed/unrepresented arguments hold,
and no zip truncation or descriptor-to-source inference occurs. Argument-free
nominals produce genuine empty owner input. This preserves String's nominal
schema rather than pretending intern_structural_term can create it.

With root's explicit coordination, `associated_owner_bindings` minimally feeds
these real arguments into the unchanged ordinary `owner_bindings` transport;
`match_owner` was not edited, copied or extended. Root connected the method in
`run.rs::select_common_argument_call`, appending actual owner rows before the
same finish_selected_argument_call. Its legacy implementation remains migration
debt, but this is one existing ordinary transport shared with written owners,
not a new alias evaluator or inference algorithm. CallPrepare/CallSeal,
CheckParameters, executable body entry, values, effects and cleanup remain the
ordinary runtime route. Full original main including StringVec is the immediate
actual validation target; no helper-only control was added.

Separate source review, all eight modes: (1) ordinary source-call lowerer/VM,
no expression evaluator; (2) no macro-only output path; (3) actual qualifier and
inherent source declaration selected without new trait/primitive matching,
existing owner transport reused unchanged; (4) original/admitted generated
source uses the same Q51/Q73/Q44 path; (5) declaration metadata only, no patterns
or cold executable body checks; (6) immutable memo-retained target/guards, normal
call phases consume it without reselecting names; (7) actual completed source
environment's shared cached member index is cold, and installed target guards
do not build a new world census; existing coarse source-revision/member debt
is not claimed removed; (8) Arc parent/query handles and declaration clones,
no BodyIndex copy, whole-file source search or alternate parser. Cold costs are
existing Type/name/member/signature demands and positional written-argument
transport. No measured performance claim follows.

Owned hashes handed to independent source reviewer:
- entered_call_goals.rs: `1a0e8a5c5b03b2fed25a8a6077fc54d1e3cbf3b36f28d0164878987db0110c35`.
- explicit_type_arguments.rs: `90543d1ec7bda5bc72f2eb8e32cf536547a1e08c4ee9c1eac08857d41c5e7004`.
- db/entered_call_thunk.rs: `66d52d043f78e863246ad70add12a77cca339c3bedb0f00b362f7cedb010715e`.
- run_owner_arguments.rs: `5cbdc48e53dab37d9c0da7aff6e5256624776c529ba30cf9ddc560c7e0bc9255`.

Root owns the separate run.rs hook and its exact final fingerprint. Formatter
stdout parse of these four owned files exited0 without rewriting existing
files. No Cargo/test/runtime/Git command was run by this worker. The candidate
is held for combined asynchronous full-original execution and real regression
programs. No runtime gain, fixture completion, task/milestone/interpreter/JIT
acceptance is claimed before those actual terminals.

Independent source reviewer found a concrete qualification hazard in the first
hash: `<X as Tr>::f()` can have Q51 source Nominal(X) plus an actual retained
trait member context; choosing X's inherent f would override the explicit as
anchor. Corrected before freeze: any actual Q51 member_traits context or
trait-self qualification stays on the ordinary trait/upstream route before
inherent lookup. No trait syntax is reparsed or guessed. This supersedes only
entered_call_goals.rs with SHA256
`c8641135356b5479005bc6511fe4dccfdcfe2c2378914668db6a6bbfb2362cc7`;
the other three hashes remain unchanged. Formatter stdout parse exits0.
Independent successor recheck and actual runtime remain pending.

## Independent source review of associated admission and owner transport

October 20, before combined source freeze/build. Reviewed exact SHA-256: `frontend/entered_call_goals.rs` `c8641135356b5479005bc6511fe4dccfdcfe2c2378914668db6a6bbfb2362cc7`; `db/entered_call_thunk.rs` `66d52d043f78e863246ad70add12a77cca339c3bedb0f00b362f7cedb010715e`; `frontend/explicit_type_arguments.rs` `90543d1ec7bda5bc72f2eb8e32cf536547a1e08c4ee9c1eac08857d41c5e7004`; `engine/run_owner_arguments.rs` `5cbdc48e53dab37d9c0da7aff6e5256624776c529ba30cf9ddc560c7e0bc9255`. No Cargo, test body or production edits by reviewer in these files. The root's separate engine consumption hook was not yet present/reviewed by this stamp.

One actionable review finding was corrected before handoff: Q51 may retain a nominal Self together with a genuine explicit trait anchor/member context. The first draft prepare_associated could select same-named inherent f for `<X as Tr>::f()`. Final c8641135 checks actual member_traits and permits_trait_self_inference before nominal/inherent admission, preserving the existing ordinary trait/upstream route. No further actionable issue found in the bounded four-file source pass. No new isolated control was requested or added; the unchanged actual main is the next witness.

The private admission retains the actual Q51 query value and qualifier guard, ordinary source Callable, and actual FunctionSignature guard. It validates same retained environment, source nominal identity, genuine existing inherent bucket, callable source currentness and matching signature fingerprint. It does not prepare a callee BodyStructure or thunk. Retained target/source-name checks flow through the same Q44/Q47 and ordinary initial_thunk_with_source_call path. Reached target callable extraction revalidates the source Callable and both guards; no alias spelling is treated as a nominal owner ID and the legacy no-proof syntax guard stays intact.

ExplicitType::nominal_arguments requires actual nominal identity/arity and matching WrittenType::Generic structure, preserves each source Type and written child proof, and preserves the exact FrameTemplate definition where applicable. Unsupported/non-Type or unavailable proof packets hold rather than erasing arguments. The associated owner adapter forwards those genuine arguments to the unchanged ordinary owner_bindings/match_owner implementation, as explicitly allocated by the lead. That existing custom owner relation is named migration debt, with no new matcher or additional semantic cases added here. Full StringVec source argument materialization and owner application still require the actual production run.

Eight-mode assessment for these files: (1)-(2) actual CallExpr/Q44/Q47 lowerer/VM route, no alternate evaluator or macro-only executor; (3) ordinary source/member admission plus unchanged legacy owner service, no new alias/method-name/numeric whitelist; (4) same entered source query path for original/generated owners, actual context guards retained; (5) callee declaration/signature only during preparation, no cold executable body or patterns; (6) immutable query target/qualifier retained, although unchanged owner_bindings still has legacy source-resolution costs and must not be represented as full prepare-once migration closure; (7) actual member/qualifier/signature query dependencies with no new full-world dispatch comparison; (8) shared parent/query handles and actual indexed path, no BodyIndex copy or new per-phase whole-file node search. Source Callable cloning includes existing declaration/signature metadata, not an executable BodyIndex.

V534 supplies the fresh actual baseline: unchanged static-method-type-alias-11047 still fails after 9 dispatches/8 outcomes/one call at lexical item callee integration. Current changes do not yet have a build/runtime receipt. Required acceptance remains complete original main through direct Qux, Ham and applied StringVec calls, plus the retained eight-program runtime regressions and unchanged source/HEAD/binary guards. No component/source result here establishes interpreter, full-fixture, upstream solver or JIT acceptance.

## V536 frozen-source regression selection: explicit trait anchor

Read-only selection, no copying/importing inputs or running Cargo/interpreter.
The smallest exact unchanged run-pass witness found in the actual pinned
upstream search is already committed as
`fixtures/upstream/rustc-checkout/tests/ui/delegation/explicit-paths-pass.rs`.
It is 1056 bytes, SHA256
`600d5f51d5c6bd4faf9515867ac93e1c68e27303811073034f987aac5796487b`,
git blob `fd0b9e83689d80db56c001d4b7b1c6681af1cef7`, from the manifest's
actual pinned upstream `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`.
The official checkout and committed fixture have matching SHA256. Both manifest
file and case entries already exist; no import, selection or source change is
needed to request this case after the current batch releases.

Exact original contract: one `//@ run-pass`, no edition override (owning edition
2015), no auxiliary crates, revisions, target restriction or expected-output
file. Retain `#![feature(fn_delegation)]` unchanged. In `main`, the actual source
asserts `S::static_method(42) == 43` and
`<S as Trait>::static_method(42) == 42`. Both declarations really coexist: the
inherent `impl S` delegates the same name through `to_reuse::foo`, adding one,
while `impl Trait for S` delegates the trait static function without that change.
This detects incorrectly overriding the explicit trait anchor with the inherent
bucket, including when nested inside the ordinary `assert_eq!` expansion.

Important prerequisite: the witness uses real upstream function delegation
`reuse`, plus another Display/format assertion before these two calls. Source
inspection found no ordinary LRR delegation producer in the bounded syntax and
frontend search. Its actual first runtime terminal is therefore unexecuted and
may be an earlier common language prerequisite; the selection is not evidence
that the corrected Q44 admission has executed. Do not strip delegation, remove
the format assertion, rewrite the program or synthesize a smaller Rust case.
Whole original `main` is the candidate, with all earlier failures retained.

Nearby smaller originals were inspected and do not replace this exact witness:
`traits/impl-inherent-prefer-over-trait.rs` (509 bytes,
SHA256 `4ff99641f65f6d03d8f8628d51d084b4e3ee5f36e0986cbed98a9307b7c2c31b`)
tests inherent preference with no explicit trait-qualified same-name call;
`associated-consts/associated-const-resolution-order.rs` (547 bytes,
SHA256 `bc121a788a5420b67af7464cbe30c82dbcf18a23066eeca4a3c27819e855249a`)
has the exact competing qualified constant situation, not a function;
`delegation/method-call-priority.rs` (546 bytes) tests a generated delegated
method against a competing inherent method but has no written `<S as Trait>::f`
associated-function call. The exact candidate above adds the required original
written call and distinct asserted results.

Existing harness support is `tools/rustc_checkout_runtime.py::contract` and
`main`: a single plain run-pass is supported, no-edition means 2015, the already
imported case is selectable by `--case`, actual file/gitblob hashes are checked,
and live CLI runtime capture establishes outcomes. Crate feature attributes are
part of the unchanged program, not special harness directives. A later authorized
validator can add only this exact argument to its existing actual batch:

```
--case tests/ui/delegation/explicit-paths-pass.rs
```

Use the same prebuilt `--lrr`, `--source-digester`, actual macOS source/binary
guards and a fresh report directory documented by tools/validation/README.md;
the harness performs no native reference execution. No new miniature fixture,
mock, evaluator or isolated admission control is proposed. No command was run
and no acceptance result is claimed by this selection record.

## V536 actual alias regression: missing retained qualified target

V536 actual full unchanged main regressed to 3 dispatches/3 outcomes/0 source
calls, with `LRR-E-CALL-CONTEXT: entered local call written target absent`.
This supersedes no earlier baseline; it attaches to this candidate, and supplies
no alias runtime pass. Parent owns the exact durable V536 evidence and final
closing source guard. Production remains frozen during this diagnosis.

The exact bug is in the new integration: `prepare_associated` authenticates the
first `foo::bar::baz::Qux::new` actual Q51 qualifier and callable, but
`entered_call_goals::written_target` is the inherited *single local name*
capture helper. Its explicit qualifier/leading-`::` early return yields None.
The Associated constructor stores that None; Q47's ordinary
`indexed_source_name` then correctly refuses missing written metadata. No guard
should be removed: the actual indexed qualified target must be retained.

The bounded connected correction after source release is to obtain this target's
name from existing `ParsedFile::written_callee_spelling` in
`syntax/value_paths.rs`, instead of treating the final segment name as the full
callee name. That common pure helper supplies qualified path components,
absolute roots, owning `$crate` hygiene and existing anchor policy. Retain the
same exact path NodeId, actual/indexed source ranges and final-segment generic
argument sites already built by `written_target`. Require Some before admitting
Associated, as Local/Module already do, so an unsupported spelling stays an
explicit source preparation hold. The same Q44 -> Q47 ->
`initial_thunk_with_source_call` name/range/argument comparisons and ordinary
runtime CallSeal/owner transport must consume it unchanged. Do not obtain a
name by joining display tokens or weaken that comparison.

One existing shared-helper limitation is explicit owner generics:
`written_callee_spelling` currently refuses `Vec::<T>::new` while the ordinary
CallExpr lowerer has existing owner-argument syntax transport. Broadening it
without authenticated owner metadata could let an unrelated module query erase
the owner arguments. Any broader repair must share the actual lowerer's pure
spelling capture with Q44 while retaining Q51/Q73 owner proof; no matcher, name
fallback, or convenient fixture-only vocabulary is warranted. The actual full
alias witness's direct qualified call, Ham alias and StringVec alias use no
explicit owner-segment generic list, and are covered by that existing common
spelling API. This known gap remains explicit, not a completed qualification
feature claim.

Cold cost is one existing retained parsed-path spelling walk at Q44 preparation;
hot thunk execution reads the retained target and checks existing dependencies.
No new parser, evaluator, semantic solver, body lowering, namespace census or
BodyIndex copy is needed. All eight prior source-review directions apply.
Required closure is the next real entire original main and real regression
batch, including StringVec; no new isolated helper acceptance is proposed.

V536 source release successor implemented: only
`frontend/entered_call_goals.rs` changed, SHA256
`85be7ceea526aeb848362ae8a6c1ccb26b04e82888fd3656e79bee36797470b3`.
`written_target` now obtains the complete actual callee name through existing
`ParsedFile::written_callee_spelling`, after validating a real final NameRef,
and keeps its existing final generic-argument sites and exact path/range checks.
Associated admission rejects missing written metadata before creating a Q44
selection. No Q47, lowerer, runtime hook or source comparison guard changed.
Pinned rustfmt `--edition 2024 --emit stdout` parsed the single file, terminal0;
no Cargo/test/execution was run by this worker.

Separate own all-eight source recheck against this successor: (1) ordinary
Q44/Q47 CallExpr route, no evaluator; (2) same normal path for macro output,
including existing `$crate` spelling; (3) no new member or inference rule,
existing Q51 trait-anchor guard remains before inherent selection; (4) no new
generated-source exclusion; (5) declaration/path metadata only, no patterns or
cold executable body preparation; (6) one pure retained-parser path walk at Q44,
no installed replanning; (7) existing spelling/local-scope policy and actual
query guards, no world map construction or dispatch census; (8) retained index
and direct path identity, no immutable BodyIndex copy/new file search. The
existing helper limitation above remains held. Source review and syntax parsing
do not establish the actual entire-main result. Validator review and the next
real unchanged main/regression terminals remain required.
## Independent V537 frozen-source review

Reviewed entered_call_goals.rs SHA256 `85be7ceea526aeb848362ae8a6c1ccb26b04e82888fd3656e79bee36797470b3` separately against the actual V536 missing-written-target regression. `written_target` now uses the existing `ParsedFile::written_callee_spelling` with the actual retained PathExpr and owning cfg, preserving actual final argument sites, node identity and both source/indexed ranges. The early associated-admission check prevents a target-less Q44 result. Spelling is comparison metadata; Q51/Q73/SourceCallable and existing memo guards still select and authorize the declaration. No actionable defect found in this narrow successor. Existing owner-segment generic spelling refusals remain a documented gap; the unchanged alias main is still required evidence.

Eight-mode pass: (1,2) no expression/assertion or macro evaluator; (3) no alias-name-specific member rule; (4) shared hygienic spelling handles original/generated paths under its existing supported domain; (5) no pattern or cold executable-arm preparation; (6) spelling is prepared through the retained query, with no new installed-execution parser; (7) no namespace census or invalidation change; (8) no immutable body copy or whole-file lookup. The helper walks only the actual path and its existing lexical/hygiene dependencies. This source review does not establish full-main acceptance or close remaining alias/owner argument migration debt. V537 uses the same eight real programs, with no isolated controls.
