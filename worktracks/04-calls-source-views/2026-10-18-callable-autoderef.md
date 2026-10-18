# Callable autoderef — bounded WT04-04 / T032, T033, M018, M025, M030 repair

Owner: `/root/closure_callable_resume`, October 18, 2026. Root approved disjoint
IR/VM callable paths. Existing dirty source is preserved; work is in this checkout.

Invariant: the once-evaluated callee place follows ordinary builtin reference/Box
and source `Deref` calls before its reached callable identity is selected. No Rc,
Arc or assertion-specific storage projection or evaluator is permitted.

Primary unchanged witness: `rustc/rust/tests/ui/autoref-autoderef/autoderef-arc-boxed-closure-call.rs`
(run-pass). Its Arc constructor is independently unsupported; this repair does
not claim that whole witness passes. Also inspect unchanged
`tests/ui/autoref-autoderef/autoderef-vec-box-fn-36786.rs` for ordinary wrappers.

Existing route: common retained BodyIndex Call expression -> value_call_thunk ->
callee operand place -> select_value_call -> existing closure/source selection ->
ordinary argument preparation/frame entry/return. Generated and handwritten
expressions share that route. New CalleeDeref operation belongs to callee operand
completion, before Commit/call preparation. It reuses ordinary operator activation,
source-selected Deref and parked-reference cleanup (`resume_place_deref`); no
second parser/evaluator or runtime namespace service is introduced. Existing
builtin autoderef loops in select_value_call/select_closure_call will be removed
once CalleeDeref owns the adjusted place.

Only reached callee preparation occurs. Its expression executes once; the
operation resumes across NeedInput/source calls with the existing activation.
Returned references remain parked to the ordinary operand scope; panic uses the
existing operator unwind/cleanup owner. Cost is proportional to actual autoderef
steps, with no new cap, body clone, parser call or world scan. No sandbox checks
or new resource accounting is added.

Acceptance needed: unchanged stock wrapper calls plus existing Box/function
pointer/closure controls, reached Deref effects exactly once before argument
side effects, nested wrappers and suspension/cleanup. Source review must inspect
all eight recurring failure modes. Until actual validator terminals, all runtime
claims and the full macro-origin matrix remain unexecuted; no task/JIT gate closes.

## Separate source review (before validation)

1. Assertion/expression evaluator: `ParsedFile::value_call_thunk` emits ordinary
   `CalleeDeref` independent of macro name. `select_value_call` still selects the
   actual callable/source identity. No assertion evaluator was introduced.
2. Literal/break-only generated route: no generated-source route or whitelist
   changed; ordinary call nodes use the same operation. General legacy audit debt
   is not closed by this slice.
3. Context-specific member/numeric rules: only reached callable-place adjustment
   changes; source Deref selection is `RunMachine::enter_operator_call` and
   `select_operator_method`, shared with ordinary operators. No Rc/Arc name/layout
   check and no numeric rule was added.
4. Generated-source exclusion: no source-origin conditional. Existing common
   retained syntax/index admission remains the join. Full generated matrix is not
   yet executed.
5. Pattern/eager-arm machinery: no pattern lowerer or cold-body inspection added.
   The current callee's Deref body executes eagerly once reached; arguments run
   after completion of its operand recipe. Fn/FnMut overload ranking beyond
   shared-Deref callable adjustment is not established by this source review.
6. Repeated parsing/planning: VM uses retained operand slots and actual operator
   activation; no parsing or complete-plan rebuilding in `step`. Real source call
   preparation is the existing shared service; its costs are inherited rather
   than replaced. Returned activation reruns only CalleeDeref, not its evaluated
   callee expression or prior ValuePlace prefix.
7. Namespace scans: no new namespace census, guard or invalidation dependency.
   Deref uses existing actual member/operator service and source-call cache.
8. Immutable bodies/node lookup: no body clone/new file walk; BodyIndex operand
   lookup remains direct. Builtin adjustment reuses `Frame::peel_builtin_derefs`.
   Removed the two separate builtin loops from callable and closure selection;
   existing method/borrow builtin adjustment is shared.

Stable wire: new opcode68, recipe schema14/domainv14 invalidate prior recipe
cache world; counters now include the new ordinary operation. Verifier restricts
it to the call's actual first operand place at Operand(0). The shared activation
holds its returned reference in dispatcher storage and `resume_place_deref` parks
that reference to its ordinary operand scope before retry. Existing operator
unwind is retained; panic/suspension/exact-once behavior still needs terminal
validation. No unsafe, new accounting/cap, source authority or sandbox is added.

Exact reviewed source leaves (SHA-256, includes inherited changes present at
review; hashes do not claim that all inherited code was newly written here):

- `crates/lrr/src/engine/run_value_calls.rs`: `57a536307ad3e55fc38f4e041d3993db26c93a3ca14ea493b0ffc6c370758534`
- `crates/lrr/src/engine/run_capture.rs`: `fe688cf00b72229c7bc8857a983595f748254ef5b85c7472c2889869a87beec0`
- `crates/lrr/src/ir/op.rs`: `708b74c51a50d4ffc7562c187960444534dee0709166a9953546ba932e8e00bb`
- `crates/lrr/src/ir/recipe.rs`: `4d71b4c936843aa773719402042708c99c828884d8f0f932686322dbb4ccc4ef`
- `crates/lrr/src/ir/validate.rs`: `4b302ec6b1a9cdf6998f83c06243dcd0bdd475acec6ac4c002cb7a0d70ef94b4`
- `crates/lrr/src/ir/parameter_place_wire_tests.rs`: `80be2c54e42d1d193911ee7f2287c781ba9053980cc03cea624631cece977b4a`
- `crates/lrr/src/ir/definition_numeric_wire_tests.rs`: `1ee07b4e7b7b5ac5713e32267d0353b4767bd1e132caf5fcb688d3d198f81fc4`
- `crates/lrr/src/syntax/value_call_thunk.rs`: `e5ca86676016f9a937832960de84e421f23fa4c2ea64fb0fe2c67b3c5e473797`
- `crates/lrr/src/vm/frame_operator_calls.rs`: `e55e6820ce813f1b0085f33f9ba807a0bfaa9de39a60eaefb39f83ce399f28ca`
- `crates/lrr/src/vm/step.rs`: `feec3a9b3070f08d5402d9910b2150a69b0b8ec7ee3c442ed8f1eb041b6e8732`
- `crates/lrr/src/cli/callable_value_tests.rs`: `7ebdaebe6eca68349f609400c8d427084d801d77d216a21613698fab4cbd18c5`

Validation: pending sole validation agent; no build/test command run by this worker.

## Actual V1 frontier and bounded V2 correction

Sole validator candidate-v1 compile passed. The three new selectors first failed
without the embedded facade. Same binary rerun with the facade reached these
actual distinct frontiers (`artifacts/validation-resume-2026-10-18/candidate-v1/facade-control-{0,1,2}.log`):
source Deref `structural result descriptor is not closed`; Arc original declared
`alloc::sync::Arc::new` not executable; Vec original generic unit variant lacks
its type context at alloc/vec.rs656..659. No new stock pass was established.

Root explicitly granted the narrow generic result descriptor join in frame.rs.
`definition_result_descriptor` and `materialize_definition_result_descriptor`
previously tried written result materialization with an empty parameter table
before the existing actual bound result descriptor service. A written `&T` where
T is a reached closure/nominal instance therefore refused before considering the
real runtime generic binding. V2 reverses those two existing alternatives: the
bound descriptor first, written source schema fallback second. There is no new
inference service or callable/RC-specific descriptor rule. Source constraints,
Deref selection, result ownership and frame continuation remain unchanged.

All8 rereview of V2: modes1–5 unchanged shared syntax/operator/result route;
mode6 existing bound descriptor lookup/intern is used once at existing demand,
without source parsing; mode7 no namespace logic changes; mode8 no body/index
copy or whole-file search. The new test adapters now explicitly reuse existing
`format_trait_facade_tests::run` with the standard facade. Imported originals are
read without edits and passed as their complete bytes; the LRR route control is
outside originals. Arc and Vec independent prerequisites are retained; no
passing expectation is weakened. New runtime evidence remains pending.

V2 revised exact leaves (other V1 hashes above are unchanged):

- `crates/lrr/src/vm/frame.rs`: `6b47aa6b9e93aaa4a90ad6e85f5116e31f41d96ff4f80d926751af943a03e0f1`
- `crates/lrr/src/cli/callable_value_tests.rs`: `a8d0904eb7874f44c90225f76e95e4d1c040488ed991f27befae7a06cb362a59`

### Mechanical V2 compile correction

V2 no-run failed E0433: the existing facade test helper is nested under
`cli::crate_graph`, not directly under `cli`. Corrected test-only path and
made that existing test module visible to its parent, its `run` function visible
within cli. No duplicate harness, production behavior or acceptance adjustment.
All8 runtime source review unchanged. Revised test-only exact leaves:

- `crates/lrr/src/cli/callable_value_tests.rs`: `5a1d27140675629fbf304141bc34ae91a15b762fa5eef6b2fa931c1c9f73112a`
- `crates/lrr/src/cli/crate_graph.rs`: `fcc091e38bf63cbf3a71c6af8633593890f2b9e6d8f892a79fc3e4e5e9744ca5`
- `crates/lrr/src/cli/format_trait_facade_tests.rs`: `46889792f748300b30877db29a707e7b2224ee2ba6bac8574d2c445b88623d96`

Traceability refinement: M030 directly owns the reused operator/Index/Deref continuation; M018 owns the shared declared result SourceView/descriptor join. These are component prerequisite repairs, not completion of any whole card or architecture/JIT gate.

## V3 observed failure and shared bound-result repair

V3 compiled; source-Deref and unchanged Vec controls still failed at their prior
runtime frontiers. Reordering alone was insufficient. Source inspection found
`GenericEnv::bound_result_descriptor_*` only inspected the original signature's
`result_symbolic`, while source/member-resolved generic results can have no such
original template. The already-admitted `structural_result` retains this same
call's resolved shape. Its closed nominal/closure leaves have actual descriptors
paired with the retained `symbolic` and `runtime` substitution tables, rather
than source schemas from which the VM could reconstruct them.

Root granted narrow vm/context.rs extension. The existing bound-result service
now tries the original template and retained structural result, and recursively
uses the actual paired source/runtime substitution for equal closed source
leaves. No name resolution, descriptor fabrication, private layout projection or
source parsing is added. Lookup cost is relevant generic-binding-table size per
result leaf (not world size); Param lookup still uses the immutable sorted table.

All8 rereview: 1–5 same common call/result service for all origins, no assertions,
new evaluator, patterns, origin refusal or cold body walk; 6 no parsing/replanning,
only actual entered result descriptor closure; 7 only this call's paired generic
bindings are inspected, no namespace census/invalidation change; 8 no immutable
body clone/file search. Actual ownership/call identity checks are unchanged.
Acceptance remains pending; the Arc facade and Vec nominal result-context gaps
are not declared repaired by source inspection. Previous terminals remain attached
to their own source versions.

Revised source SHA-256:

- `crates/lrr/src/vm/context.rs`: `cef187e7a72b661d731d2d68acb051d3522677cae59b862e304498ae5aeb0c9c`
- `crates/lrr/src/vm/frame.rs`: `6b47aa6b9e93aaa4a90ad6e85f5116e31f41d96ff4f80d926751af943a03e0f1`
- `crates/lrr/src/cli/callable_value_tests.rs`: `5a1d27140675629fbf304141bc34ae91a15b762fa5eef6b2fa931c1c9f73112a`

Pre-validation source-review correction: equal source views may erase distinct
closure capture/runtime representations. The closed-source fallback now requires
all matching retained bindings to agree on one descriptor; it never chooses an
arbitrary first representation. Formal Param lookup retains its exact keyed
mechanism. A missing precise template with ambiguous erased source views remains
an explicit gap, not permission to select the wrong value. Updated context hash:

`a49c087593fb79e29f5532e07fe6cc618837225f97b190cf2e3477f380a0eaaf`

Stationary final context leaf: `0bad51ee9c54efdbab203156823ffd64eaf5e09b841bf9e611040deb92b06e95`.

## V4 actual terminal and remaining common prerequisites

Sole validator `candidate-v4` compiled successfully (compile.log, 9.50s). The
new shared bound-result repair advances the source-Deref control beyond the
prior structural-result descriptor refusal. Its actual terminal is now
`LRR-E-TYPE-CONTEXT: numeric literal needs an explicit suffix or reached type hint`
in the caller main, `candidate-v4/control-1.log`. The unchanged Vec original
still fails generic unit-constructor result context (`control-2.log`). Arc's
previous actual unsupported constructor terminal remains an independent hold.
No new stock-runtime or full-fixture pass is claimed by this slice. The full
source-Deref/macro/exact-once control has NOT passed; partial advance is not its
acceptance. Existing generic calls, function pointers and wire regressions need
actual terminals at the final candidate before integration claims.

Root's unrelated generated-module fixture passes in the same V4 queue; that is
not callable acceptance. V4 source/binary identifiers and command terminals are
owned by the validator's `artifacts/validation-resume-2026-10-18/candidate-v4`
receipts. The source has remained stationary since the final context hash above.

Do not fix the remaining numeric frontier by defaulting every borrowed numeric
literal or by adding assertion operand rules. `let r = &1; let _: &u8 = r` must
obtain the ordinary future annotation constraint, and a comparison involving a
known-u8 callee must preserve that width. Required common source/type/member
constraints must precede reference materialization when no reached hint is yet
available. Current source inspection identifies `GuestStore::place_type` /
`bound_literal` as one refusal path and `syntax::numeric_fallback::literal_only`
as deliberately excluding a bare reference result; it does not prove the exact
failed instruction because V4's facade helper did not retain a full failure
execution record. No workaround or fixture suffix edit was made.

Further unchanged witnesses considered, not newly imported/executed here:
`tests/ui/typeck/issue-72225-call-fnmut-through-derefmut.rs` (check-pass with a real
main using RefCell<S{f:Box<FnMut>}>; useful execution component, not a run-pass
contract); `tests/ui/closures/issue-42463.rs` (run-pass source Deref implementation
calling a closure field, then Vec operations). The former exercises callable
fields through overloaded places; neither is falsely presented as a newly passed
direct Rc/Arc callable runtime witness.
