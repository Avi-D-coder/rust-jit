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
