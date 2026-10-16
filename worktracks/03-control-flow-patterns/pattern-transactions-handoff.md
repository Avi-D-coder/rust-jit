# Common pattern transaction successor — bounded architecture record

October 15, 2026, T050/T051/H003, WT03-02/03. Coordinator LOCAL-024 owns
integration; `/root/common_pattern_engine` implements the shared pattern/IR/VM
slice. The preceding shared scalar-test packet remains frozen under overlay
SHA-256 `79a130cf8bd1ca51c59cd609df71fe020e2b9a4303d43630b8f1c79b5338f9b2`.
This is a separate successor and never changes that validation source.

Violated invariant and minimal witness: `match 2_i32 { 1..=3 | 2 if reject()
=> (), _ => () }` must retry the second matching alternative after a false
guard without evaluating the scrutinee again. The existing ordinary lowerer
returns both guard outcomes to a single arm continuation, then refuses multiple
alternatives because it has retained no selected-alternative/retry transaction.
The primary stock frontier is unchanged rustc
`tests/ui/or-patterns/search-via-bindings.rs`, including its exact guard-count
matrix and nested/cartesian patterns. Its structural forms remain an explicit
acceptance frontier; an LRR scalar retry control is not its full contract.

Extend existing ordinary `literal_match::prepare_inner`,
`condition_ranges::prepare`, `ParsedFile::initial_let_condition_thunk`, normal
`ir::validate::verify` and VM operation/exit execution. A generic bounded
`vm::pattern` transaction retains the actual owner, source slot, selected
alternative, prepared retry PC and pending binding intents. Test/guard failure
rolls back those intents; success commits once. Guard previews are limited to
existing proved intrinsic Copy sources; general aggregate/reference previews
remain unsupported until the common borrowing/projection prerequisites exist.
Do not copy or move an owning scrutinee to test a pattern.

New ordinary IR uses one `Pattern` opcode and a `PatternRetry` exit, with explicit
stable tags, schema migration, metadata accounting, verifier source/role/body
checks and runtime continuation checks. No parser, macro identity, synthetic
function, new evaluator or per-entry world census is involved. Source planning
retains immutable retry destinations; installed execution consults only current
frame state and prepared PCs. State/intent storage is bounded/accounted at frame
creation, and scope retirement rolls back metadata with no replay of completed
operations. Existing pure query/memo ownership remains unchanged; outer retained
plan memoization is still outstanding architecture debt.

Obsolete routes to remove when replaced: scalar pattern's multi-alternative
guard refusal; direct match BindLocal-before-guard sequence; separate
identifier/wildcard conditional commit lowering in thunks.rs. Shared pending
binding staging/commit must replace those production paths, not coexist behind
new names. Source name/type and generated-scope authority remain their existing
services; no transaction intent grants namespace or capability authority.

Shared interface ownership: this worker owns ir/op.rs, recipe.rs, validate.rs,
vm/frame.rs, step.rs, pattern.rs, syntax pattern/match/condition leaves and new
component tests. Root owns thunks module/dispatch glue, engine/failure.rs and
run_capture exhaustiveness glue, provenance/ledgers. The reference worker owns
runtime/cell.rs and provides preflight_pattern_bindings/reserve_pattern_bindings
for atomic checked destination-cell/ticket batch reservation; no overlapping
edits to those leaves. No Cargo/native tests, extra agents, commits or ledger
completion are authorized for this worker. The sole validator will receive a
separately frozen source manifest and terminal plan.

Closure requires actual normal-PC verifier/dispatch evidence, unchanged stock
contracts, handwritten/user/wrapper/standard macro matrix, false-guard retry and
pending-binding cleanup, source/epoch invalidation, suspension/panic/exact-once
controls and the current milestone. The implementation may checkpoint a closed
ordinary subset with explicit structural/source-authority holds; it cannot
close T050/T051 or E-R5-005 without that evidence.

## Source-only successor handoff and separate architecture review

The coordinator froze combined 44-leaf V2 from baseline
`fa071ee4d381e88645a837b23574534e1aeeaf80` at
`artifacts/macos-first/common-pattern-reference-transactions-v2/overlay`, packet
SHA-256 `90ca357f770b7d03d813ec1c95f7e940cc30c7be80aa50b565cd36e276492b44`.
Combined 43-leaf V1 (`24f51daa9c8ef0704a7b91b189ce07369a7c82ee634c38cb5c9b2d8af7e821ed`)
remains historical and unexecuted. V2 corrects the obsolete BindLocal observer in
`ir::validate::scalar_binding_copy` to require the actual StageBinding source,
owner and binding, and updates the two `xtask/tests/clause_scopes.rs` observers
to the shared operation. Exact corrected hashes are
`bfdea4d8a9ee530fe8f073d3eaaaae7e99407ba7430328a6e4f17c7ab30ef108`
and `183ed5a27dc2604c0c6b23cda08468d29bb836d20f440b376036993d3ee9ad85`,
respectively. This review applies to V2; later retained-plan source is a separate
successor. No Cargo, Rust test or native execution was performed by this worker.

The live ordinary route is `literal_match::prepare_inner` and
`condition_ranges::prepare` -> `PatternOperation` -> `ir::validate::verify` ->
`Frame::{preflight_pattern,execute_pattern}` -> `Transactions`. Match and
condition tests use the same staged binding intents and checked finish/rollback.
`RecipeExit::PatternRetry` uses the prepared same-body forward PC retained by
Begin, rather than re-entering the scrutinee. Preview publishes actual checked
intrinsic-Copy cells through `GuestStore::copy_from`; it does not consume the
pending source. The separately prepared match ReadPlace scratch is explicitly
discarded. Commit transfers once through ordinary `Frame::bind`; selected guard
bindings adopt the provisional copies. Scope cleanup retires transaction
metadata after guest-local cleanup, including resumable cleanup. No slot-marker
restoration is used. Bulk checked cell/ticket reservation comes from the
reference worker's `GuestStore::{preflight_pattern_bindings,reserve_pattern_bindings}`.

Removed production routes: the scalar multi-alternative guard refusal, match's
direct BindLocal-before-guard sequence, and thunks' separate identifier/wildcard
conditional commit. Root-owned thunks delegates all selected conditions to the
ordinary condition lowerer. Structural patterns, ref/@ binding, non-Copy guard
preview, or-pattern binding namespace admission and let-else remain explicit
gaps; this bounded mechanism does not close their contracts.

The reference-result prerequisite shares the same schema migration but does not
add pattern authority: `HintRecipe::DefinitionResult` consumes only the existing
opaque result-source certificate, at root NodeId(0)/Entry/result slot. Its actual
caller region survives materialization. Root thunks selects it only for a real
RefType return; `GenericEnv::reference_result_source` is reference-worker owned.
`Frame::preflight_return` admits kind9 only after live reference-record checking,
preserving the certified return-source and result descriptor checks.

Separate source-review pass against the eight recurring failure modes (these
numbers are not MA001--MA008 audit identifiers):

1. **Separate assertion evaluator:** new transactions enter only ordinary
   `Op::Pattern` dispatch in `step.rs`; no macro spelling or assertion branch
   appears in the new lowerer or VM state. Existing assertion-specific routes in
   unaffected sources remain audit repair debt; this component does not close
   their MA findings.
2. **Literal/break-only generated execution:** generated syntax uses the same
   indexed match/LetExpr lowerers and verifier. No generated opcode/index space
   was added. The previous scalar component removed provenance-only blanket
   refusals, while hygienic name/Type proof refusals remain. Full standard-macro
   matrix execution is pending.
3. **Context-specific inference/member selection:** source hints still use the
   existing local/parameter/primitive admission services. Transaction intents
   contain no name resolver, method selector or numeric-constraint rule. General
   source-constraint sharing remains a prerequisite outside this bounded slice.
4. **Generated-source rejection:** `condition_ranges::prepare` and
   `literal_match::context` do not branch on generated_origin to reject intrinsic
   token patterns. Local/parameter hygienic lookup remains authoritative; absent
   generated Type-name proof still refuses. Negative missing-defsite controls
   remain gap evidence, not valid-Rust contract passes.
5. **Duplicate patterns/eager cold execution:** shared `IrrefutablePattern`,
   `ScalarTestPlan`, `PatternOperation` and `Transactions` serve both owners.
   Match-wide preparation inspects structural source shapes and token families,
   never prepares result bodies or decodes a cold upper endpoint. Alternative
   retry retains actual source/owner/PC identity. General structural matching is
   still unsupported rather than implemented through another evaluator.
6. **Repeated planning:** installed Pattern/PatternRetry consult only frame,
   store and prepared PCs; no source query/parser runs there. Ordinary match
   preparation still rebuilds its full plan per requested phase, and condition
   phases reconstruct their small source plan. This is an explicitly open cost
   and the next bounded prerequisite, not a claimed repair.
7. **Namespace scans/invalidation:** no new namespace map or dispatch census was
   added. Plain identifier match bindings still use the existing reached
   ScalarPatternBindingAdmission and namespace witness. Existing preparation
   census/relevant dependency work is separately held; installed transaction
   authority does not replace that service.
8. **Immutable copies/node searches:** per-frame Transactions allocate bounded
   mutable intent/stack state accounted from indexed node count, never clone
   BodyIndex. Source local-let joins use root's exact binding_let_statement
   attachment. Remaining source-plan reconstruction/index-row sorting is open
   and must be replaced by retained query results before performance acceptance.

The source-derived cost is bounded stack/intent reservation at frame creation;
Begin/Finish/Retry metadata operations are bounded by active/pending state and
binding publication checks scan the staged bindings for conflicts. No measured
speed or scaling claim is made. Schema11/opcode45/PatternRetry tag8 and explicit
PatternOperation tags preserve wire meaning; schema/opcode-count controls were
updated together.

## Witnesses, tests and reproducible validation plan

Unchanged imported primary frontier:
`fixtures/upstream/rustc-checkout/tests/ui/or-patterns/search-via-bindings.rs`,
rustc commit `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, blob
`42174bd7cef737eb4e84049ca6c7f8b67f089a19`, 2823 bytes, SHA-256
`38723ec8521136188a19a4b465362e979d95f59ed65009a358aff640a23a08b6`.
Its run-pass directive is retained; no auxiliaries/expected outputs are needed.
Original MIT/Apache licenses remain retained. Nested/cartesian patterns and the
complete guard-count matrix are unexecuted and unsupported frontier entries.
Existing unchanged range originals remain source witnesses; this packet does
not promote their previous component evidence into full fixture acceptance.

New or adapted external controls: three `vm::frame::pattern_tests` bodies check
stage/commit exact-once ownership, duplicate/overflow atomicity and non-consuming
preview plus cleanup; two `ir::pattern_wire_tests` bodies check wire tags and
source/owner/body/root-result authority. Six
`service::common_pattern_route_tests` bodies include the two new false-guard and
condition-binding controls across handwritten/user/wrapper forms and all four
editions, with actual route/call/effect counters. Existing scalar/condition tests
observe the common op sequence; xtask scalar_match_guards, condition_bindings and
clause_scopes preserve cleanup and source-cell lifetime assertions. Standard
macro, panic/suspension/resume, invalidation and complete stock structural forms
need actual additional evidence; neighboring controls do not establish them.

Only authorized formatting and whitespace checks were executed: rustfmt on owned
changed leaves and `git diff --check`, both exit0. Sole-validator commands on an
unchanged frozen packet should include `cargo test -p lrr pattern`,
`cargo test -p lrr common_pattern_route_tests`, `cargo test -p lrr scalar_match`,
`cargo test -p lrr condition_ranges`, reference-result/wire filters and
`cargo test -p xtask --test scalar_match_guards --test condition_bindings --test clause_scopes`,
followed by the coordinator's current milestone gate. The validator must report
actual terminals and keep runtime-contract, component, full-fixture, diagnostic
and tooling outcomes distinct. Every Rust command in this paragraph is a plan,
not an executed or passing result.
