# Item values enter the ordinary value-call pipeline

Root, WT04/T032, LOCAL-048, E-R5-005/E007. Actual unchanged owning-2015
issue-17718 with its real auxiliary now crosses import preparation on V13, then
fails at C3() (original bytes653..657) with missing callee classification. C3 is
a const fn-pointer value. The existing ordinary value_call_thunk already evaluates
a callee once, adjusts its place, and selects TypeKey::Callable/Function through
run_value_calls before ordinary call contracts/continuation. A path to a const or
static currently falls into named source-function selection instead.

Use actual EnteredValue declaration evidence at the call's indexed callee path.
Classify its exact retained AST item as Const/Static, retaining the existing
selection guard plus actual ParseFile dependency guard; a missing or different
source occurrence is an error, never permission to call by spelling. Add a typed
Value selection to EnteredCallGoal; ordinary bindings use the same value lowering.
The pure call goal executes no initializer and admits no Fn body for a non-Fn.
At lowering, reuse value_call_thunk on the original CallExpr and callee operand;
no invented wrapper, parser, call expression or value interpreter. That operand
then enters the ordinary E007 initializer/body/storage route before value-call
selection. Qualified item paths use their actual entered Value selection too.

Files: frontend/entered_call_goals.rs, db/entered_call_thunk.rs,
syntax/thunks.rs. No new opcode, evaluator, guest/native compiler, macro identity,
permission gate, static check or resource budget. Preserve actual source/edition,
scope/hygiene, currentness, callee exact-once and existing continuation/cleanup.
The typed source-function selection and constructor routes remain separate domains
of the same call engine, never fallbacks that turn non-Fn items into fake signatures.

Cold: one actual declaration occurrence and guarded ParseFile query, retained by
the existing call memo. Phase lowering reads retained proof and actual indexed
syntax. Installed execution does not parse/plan; the callee expression executes
once before argument entry, as existing value calls do. No full namespace census,
BodyIndex deep copy or unrelated namespace invalidation mechanism is added.

Acceptance: actual unchanged root+aux capture plus existing value-call/source
currentness controls on next exact binary. All-origin/all-edition/cold-branch,
suspension and exact-once matrix entries remain unexecuted unless separately
measured. All eight recurring modes reviewed against the named shared routes;
none permits a const-name-specific dispatch or assertion evaluator. Generic
associated-const argument/template cache gaps remain explicit; no full E007,
callable-feature, stock fixture or interpreter/JIT gate closure from source alone.

## Source review snapshot before validation

- `crates/lrr/src/frontend/entered_call_goals.rs`: `76ee16d9b769798c4736d857245b3b03b5aa68f46acd80195a42de4cf22db71b`
- `crates/lrr/src/db/entered_call_thunk.rs`: `46fefabbc6855fc2087e5925c7bac108844c81b49825d0d0679be82e2d29b5ba`
- `crates/lrr/src/syntax/thunks.rs`: `3516a91ab0f6e0ec412bea50ae8127973c49dc756424473288701e50178a672e`

Selection::Value stores the actual selected identity and a ParseFile guard alongside the common Q39 guard. EnteredCallGoal domain advanced v4 to v5. Value and Binding selections never expose a FunctionSignature/retained source-function target. require_value_call_site validates exact owning body/index/source/call range and phase; provider currentness checks both selection and actual syntax before lowering. Ordinary local/module Fn and constructor paths remain. Actual Const/Static AST classification performs no initializer execution. Its cold lookup of EnteredValue for ordinary other kinds is a memo hit when reused by existing function admission; direct item kind selection visits only the retained declaration path, not a full file walk. No claimed execution result yet.
