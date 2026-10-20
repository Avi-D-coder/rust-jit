# LOCAL-049 / WT05-02: repr128 reached inference prerequisite

Bounded before-code diagnosis at HEAD
`0c11e2e3aaa11513eb8860875e97c329dc23bbb9`; M020/M025, unchanged official
`tests/ui/enum-discriminant/repr128.rs`, source SHA256
`66af2eb56be2026a744d2169f8467d11df4c75e8ea21d204f0948db2c8e445bc`.
Owner inspects numeric/operator/value-source paths only. Parent owns nominal
constructor/member argument transport; no edits there or shared registration.
No tests, Cargo, binary invocation or production source edits in this slice.

Invariant: a reached generic call must resolve needed source predicate/type
inference before committing numeric fallback. Actual enum discriminant repr
comes from source metadata; primitive comparison cannot repair a missed
generic constraint by coercing concrete integer operands or recognizing an
assertion, DiscriminantKind spelling, source fixture or runtime value tag.

## Actual evidence and common route

V505 compact evidence is retained in
`2026-10-20-source-declaration-clauses-review.md` and the source-guarded report
`/private/tmp/lrr-v505-source-clauses-runtime/report.json`. The observed failure
is owning2015 executor/fault LRR-E-OP-TYPE, "operator requires supported concrete
primitive operand descriptors". Its actual guest stack reaches the first call
`discr(Signed::Zero, 0)` at bytes667–689, then
`discriminant_value(&v) == value` at bytes614–645. No runtime check passes for
this case. The report does not capture primitive operand descriptor IDs;
the type mismatch diagnosis below is source-derived, not an invented runtime
trace. Generic-derived-type's separate V505 pass is not evidence for this goal.

Ordinary source/macro admission -> retained BodyIndex indexed BinExpr ->
ordinary NumericHint/NumericBinary Eq thunk -> VM frame/operator activation ->
vm::step preflight -> vm::numeric::binary_types/binary_in. The macro assertion
adds no alternate numeric evaluator here. The source-selected
LibraryFunction::DiscriminantValue reaches
engine/run_facade.rs -> GuestStore::referent_discriminant ->
enum_discriminant_value, which reads the actual enum descriptor's repr and
uses run-local full-width integer backing for i128/u128. That reader is shared
with enum casts and needs no change for this observed first Signed::Zero.

The earlier common call route is decisive:

* frontend/call_contracts.rs::argument_hints represents the declared U as an
  actual TyTerm::Param with descriptor None. It does not solve bounds.
* frontend/binding_constraint_bounds.rs::written_bounds explicitly returns an
  unanswered result for projected-subject predicates that mention U;
  inside_bounds_inner defers trait type arguments to the shared solver.
* engine/run.rs's CallSeal argument loop calls bind_literal_fallback before
  collecting concrete ArgumentTypes and prepare_source_call. An unbound integer
  becomes i32 in runtime/cell.rs::bind_literal_fallback at this point.
* prepare_source_call/prepare_contracts consume those now-concrete argument
  views. Existing associated_equations only handles declared associated
  equalities; it does not select the projected PartialEq<U> requirement.
* numeric::binary_types requires equal concrete primitive descriptors for Eq.
  preview_scalar honors an existing literal binding before any common operand
  hint. Therefore source predicts i128 from the correctly represented Signed
  discriminant versus the earlier i32 fallback on value. No observed descriptor
  trace was added or test executed in this investigation.

## Exact missing shared dependency; no numeric shortcut

Q78/Q80 can now retain/encode the authentic source recipe
`<T as DiscriminantKind>::Discriminant: PartialEq<U>` with actual trait/item
IDs and parent generic transport. They are explicit own source components,
not a completed solver provider. Runtime call/numeric constraint preparation
does not yet invoke evaluate_prepared to discharge this reached goal before
fallback, and PreparedProvider::ready still requires genuine completed facts.

The demanded upstream operation is projection normalization and trait input
inference with T fixed from the first reached argument and U still an integer
inference variable. Reuse pinned
normalizes_to.rs::consider_builtin_discriminant_kind_candidate: it calls
self_ty.discriminant_ty(cx), whose existing source adapter can use authentic
enum repr metadata. Then use real PartialEq candidate inference, not a rule
that DiscriminantKind always equals a guessed integer. Actual embedded
core/primitive_impls.rs has `impl PartialEq for i128` (and u128), so library
candidate source exists; core/cmp.rs supplies the actual RHS default Self.
The needed registry/trait/impl/generic/default/predicate source facts and
selected-candidate response must reach the admitted provider, with source
guards, before their inference result can bind the existing literal record.
Missing source/semantic inputs must remain pending; no fake complete provider,
empty predicate environment, second projection solver or primitive whitelist.

This is a real reached inference demand of the unchanged runtime witness;
it is unrelated to adding an unused compiler-WF goal at main entry. Avoid
forcing unrelated function bodies, nominal field layouts, static checking or
irrelevant query domains merely to publish a provider. The actual declared
fact/query applicability and required callback closure must govern preparation.

The minimal runtime integration eventually extends the common pre-fallback
call inference boundary and preserves literal identity while upstream infers
its binding. Removing fallback wholesale would break legitimate unconstrained
generic calls; delaying a literal into a callee and unifying by observed
primitive Eq would infer through executed body shape instead of its declared
trait obligation. Retagging a concrete operand in numeric_store or ignoring
binary_types would silently change concrete Rust operations. None is a valid
bounded repair. No production change is proposed until the upstream fact/root
dependency is available. The old source Component bound/equality helpers remain
named migration debt; this slice does not extend their vocabulary.

Cold cost target: demanded memoized source clause/registry/candidate closure,
upstream inference transaction and dependency guard once per retained call
plan. Hot installed operation remains primitive numeric execution or an
already selected ordinary trait call; no parser/namespace scan/whole-plan
reconstruction belongs in NumericBinary. Preserve once-evaluated argument
effects, cells, continuations and guarded currentness through NeedInput.

## Separate all-eight source review and acceptance boundary

1. Inspected run.rs call sealing, run_facade DiscriminantValue and vm numeric
   entry; no alternate assertion evaluator is proposed (historical MA001 held).
2. BinExpr uses ordinary indexed thunk/operator routing for admitted generated
   sources, with no literal/break-only extension (MA004/MA005 held separately).
3. No numeric rule based on macro/method/trait spelling is added. The missing
   projected requirement is delegated to pinned solver semantics (MA003/MA006).
4. No original-source-only exclusion; actual source clause plans already share
   source/generated IDs. Broader full-fixture origin matrix is unexecuted.
5. No pattern machinery or unselected executable arm/body is prepared. Goal is
   the declared requirement of an actual reached call, not whole-body checking.
6. No parsing/planning is moved into installed numeric execution. Proposed
   integration must remain memo-owned cold preparation with retained arguments.
7. No world namespace scan/invalidation rule is added; guard actual source
   clauses/lang/candidate dependencies and retain existing refusals.
8. No BodyIndex cloning or whole-file node search is introduced. Existing
   source stack ranges and retained plan handles are only inspected.

No unsafe, cap, static rejection, descriptor retag or custom semantic solver
was added. This docs-only component identifies a true upstream inference/fact
dependency and does not repair repr128, change V505's failure or close any
runtime/interpreter/JIT gate. Existing tests remain unchanged; all editions,
original/user/wrapper/standard macro, exact-once/currentness/cold-path controls
must be validated by the sole test owner after real integration.

Reviewed source SHA256 (no edits by this owner):

| Path | SHA256 |
| --- | --- |
| engine/run.rs | `a87de19e1e6098c53d38e8a5ce3acf04b729bee3c04cf17af0e0f8de6f107857` |
| engine/run_facade.rs | `3710486e62610e6afa95bd6378124ce5ddeb5f63bbbe4ba6293021df053309ec` |
| frontend/binding_constraint_bounds.rs | `5779a839e6ddb6dcb2401051f637485257037ec451e0ba5573b5c7924c6fd2c5` |
| frontend/call_contracts.rs | `44f6b4b4c7f957266b2043d90ae3d0f76d50032d23f52e082d722424a806345f` |
| runtime/cell.rs | `cbe6a83aae11eb550dc230cf482c7f792c9d110da9570ad6855f7fec8177cd65` |
| vm/step.rs | `4085e6aaa55aa6025e758d1a1ae89265be6e192df6f1f267cc4f40f886962990` |
| vm/numeric.rs | `1b588950f3713963e746d509f61b2bfc4b46e4c059a7e8ec34d51611f03c7b01` |
| vm/numeric_store.rs | `9c5c33e0f704fcd8e3c75bbf28b224a1136be3d3eaf73bdfaf7d1e047310b79c` |
