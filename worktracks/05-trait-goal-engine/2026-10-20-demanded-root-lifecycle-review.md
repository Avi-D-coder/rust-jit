# M020 independent demanded-root lifecycle review

Reviewer `/root/wf_validation_review`; before-code baseline
`1084e893d05a7020e14aa5583a030984ce365aa9`, LOCAL-049 / WT05-02.
Read-only source/design review; no source edits, builds or tests by the reviewer.
Final implementation review, exact source fingerprint and terminals remain pending.
The actual target is repr128.rs's projected PartialEq inference before integer
fallback, not a main-entry WF goal or another independent solver.

## Existing actual lifecycle and blocking boundary

`PreparedSolverConfiguration::with_source_context` checks the actual owning
source/configuration, expansion epoch and exact catalog publication/root.
`prepared::with_provider` then owns a new facts OnceCell, diagnostics vector,
generic cache and `context::State`, with its nondropping StateRef and scoped
canonical/generic cache guards. The higher-ranked callback result cannot retain
borrowed upstream IR. State owns the global canonical/search-result cache,
canonical parameter-environment and instantiation caches, interners and arenas.

`InferenceContext::new` owns unification/undo/region/opaque state, error taint,
universe, placeholder assumptions and overflow state. `SolverContext` owns that
context directly. `evaluate_prepared` checks its provider/context against the
ready token, then the existing `upstream::evaluate` calls pinned
SolverDelegateEvalExt::evaluate_root_goal. The pinned eval_ctxt implementation
creates SearchGraph::new at enter_root; recursive/canonicalized child goals stay
inside the upstream solver. This root API must remain the semantic entry.

Current PreparedProvider::publish/ready protect important real invariants, but
the storage requires eager complete DefinitionFacts/PredicateFacts and a global
associated-leaf Cartesian closure. A function used only as an environment owner
therefore carries unrelated signature, variance, opaque, const and other demands.
Changing missing fields to false/None/empty would change the solver's answers;
typed per-query availability is the required representation repair.

## Host unwind prerequisite and exact suspension boundary

SourceFacts methods match upstream's mostly infallible interfaces: bool, Option,
lists, binders and callback visitors do not have a QueryPoll/NeedInput result.
An expected missing-data abort can use a private owned typed payload, but it
must unwind past and destroy the complete attempt. The catch belongs outside
the call that creates configuration/with_provider/State, not around just
evaluate_prepared, a probe, a candidate callback or an existing InferenceContext.
Every failed attempt must lose all infer state, root/search graph, caches,
opaque/region/placeholder state, diagnostics and arena handles before source
preparation or another attempt begins. No partially unwound state is reusable.

Critical baseline prerequisite: Cargo.toml release and dev profiles both set
`panic = "abort"` (lines 45/48 at baseline). A catch_unwind-only change can work
in tests while the actual CLI aborts. Supported host profiles must explicitly
use unwinding for this protocol, or the upstream callback interfaces need a
different genuinely fallible adaptation. The latter is a much larger port.
The pinned std panic.rs catch/resume documentation explicitly excludes aborting
panics. Profile changes and actual CLI evidence must be recorded separately.

For expected internal suspension, std::panic::resume_unwind(Box::new(private
MissingFact)) avoids invoking the global panic hook. panic_any would print the
normal demand unless the process-global hook were changed; changing that hook
is unnecessary and would affect unrelated threads. Catch only the exact private
payload; resume_unwind every other payload. Neither ordinary assertions, OOM,
programmer bugs nor arbitrary panics become missing facts or NoSolution.
AssertUnwindSafe is justified only by destroying the isolated state; it is not
proof that borrowed external mutable state was rolled back.

The existing ScopedCache/CacheGuard releases a live value before arena owner
destruction on normal return and unwind. Preserve declaration/drop order in
with_provider. Callbacks must not retain RefCell borrows across cleanup. The
existing storage ownership regression is component evidence, not a substitute
for an actual source-backed missing-demand/restart control.

## Minimal owned request, attempt and response interfaces

Keep the complete retry protocol outside the solver:

1. Prepare an immutable owned root recipe from the actual source requirement,
   ParamEnv, source identities, argument bindings, span and stable input-variable
   keys/kinds. Do no guest mutation in the attempt closure.
2. Validate current source/configuration/catalog and build a fresh provider,
   State, interner and inference delegate from the owned admitted source plans.
   Recreate the same owned input variable map using upstream fresh variables.
3. Translate the actual root and invoke the existing evaluate_prepared root.
   SourceFacts callbacks read only frozen tables or abort with MissingFact.
4. Before dropping a successful context, resolve retained original input-variable
   handles and translate supported outputs/constraints/diagnostics to owned data.
   Do not return Context, Ty, Clause, Args, canonical IR, TyVid or stalled handles.
5. After the context and all guards are destroyed, catch a missing request,
   prepare that actual source query outside callbacks, and add its validated
   immutable domain. Retry the unchanged owned root in a fresh complete attempt.
6. Verify source dependencies before committing an owned answer to caller/frame
   facts. Source/epoch changes invalidate the selection/root instead of mixing
   worlds or resuming stale IR. Only successful export may change runtime hints.

Suggested ownership shape: OwnedRoot -> AttemptOutcome<OwnedResponse>, with
separate NeedFact(RequiredFact), actual NoSolution, actual Maybe/ambiguity and
typed unrepresentable/pending outcomes. Missing data is not semantic failure.
Prefer requests such as TypeOf(definition), Predicates(definition, family),
TraitCandidates(trait), AssociatedLeaf(implementation, trait_item), AdtFields(id)
or LanguageRole(role). DefinitionId itself owns world/source/local data. Payloads
must be Send + 'static, containing only portable owned identities/arguments.
Most source queries can request a declaration-level domain without serializing
solver-created internal inference variables. If arguments are truly needed,
export representable owned data or hold explicitly; do not add a canonicalizer.

Each retry must publish a newly available actual domain or return typed pending/
NeedInput. Repeated identical demands without new preparation are not progress.
Use that invariant rather than an invented retry/fuel/time/count cap.

## Inference and canonical answer mapping

Pinned GoalEvaluation (solve/mod.rs:470) documents that `.goal` is the input
resolved before applying the answer's inference constraints. It is not the
post-solution substitution map. The root evaluator applies canonical responses
internally to the caller's SolverContext. Preserve the original input-variable
handles and query their resolved values through the existing pinned inference
operations after evaluation. Upstream owns canonicalization, response matching,
transactions, candidate solving and cycles throughout.

Existing SourceTypeEncoder rejects TyTerm::Infer. Extend the common transport
with an explicit root-local InferVarKey -> fresh upstream variable map, rather
than disguising variables as source parameters or cloning the type translator.
Preserve Type/Integer/Float/Const distinctions and genuine const Types. repr128's
unsuffixed argument must remain an integer inference variable until the actual
projection/PartialEq obligation constrains U. Reading a value tag or coercing
already-concrete numeric operands is not this inference.

Export only representable results. Maybe is not Yes or NoSolution; unresolved
variables and unsupported external region/opaque/normalization constraints
remain explicit. Do not silently discard constraints to return a concrete
dictionary or claim a root solved. An initial bridge may require a complete
Yes response and export just the genuinely resolved required input bindings,
holding other response shapes until their owned mapping exists.

The reached call coordinator must perform this root demand before its
CallSeal bind_literal_fallback loop. Build/export privately, validate dependency
guards, then install inferred U and allow the ordinary primitive operation.
Do not mutate caller cells, Frame hints or GuestStore semantic facts inside an
attempt that may unwind: dropping the infer context cannot undo those effects.

## Keep validation, partition only actual published domains

| Published domain | Validation retained |
| --- | --- |
| Basic definition/generic metadata | exact world/source receipt, real header/context/parse, source parameter identity/order/parent count and typed generic compatibility; APIT incompleteness never becomes zero generics |
| Trait/candidate bucket | actual complete source membership for that trait, unique ordered canonical impl IDs, authentic impl heads and pinned simplification/blanket index comparison; missing bucket never means no candidates |
| Impl head/scalar metadata | actual source classification, canonical owner, trait reference/polarity and per-query source semantic proof; unrelated body/signature/CTFE domains need not be published |
| Predicate family | distinct all/own/item/explicit item/self/nonself/super/implied/const families with actual producer evidence; Q78/Q80 written source lists cannot fill every family |
| Associated leaf | exact requested (impl, trait member), actual container/name, specialization ancestry/finalization and real item identity; remove global Cartesian preflight only after every missing leaf aborts soundly |
| ADT fields/repr/variance | real schema/variant/discriminant receipt and actual declared generic domain; require fields or variance only when that corresponding callback is demanded |
| Language roles/defaults | actual role/source/catalog/configuration receipts; Known(None)/false/empty only for established semantic absence, never because a role map or option input was not prepared |
| Const/transmute/opaque/coroutine clients | their existing typed admission checks and complete exact requested input; unrelated missing client data cannot become guessed callbacks |

Represent Unavailable separately from Known(value), including Known(None),
Known(false) and Known(empty). Source NotApplicable means the compiler query is
invalid for that role, not a convenient empty value. Invalid published data and
wrong worlds are validation errors; do not hide contradictions as retryable
MissingFact. Frozen optional domains need not imply every referenced definition
has every semantic field, but every published reference must retain authentic
identity/header/layout evidence required by its actual query.

## Source scalar worker pass

Reviewed new source_definition_semantics.rs against pinned is_const_fn,
is_conditionally_const, constness, defaultness.has_value, alias/closure domains,
codegen attributes, opaque and const-condition providers. Trait-impl
HasItemDefinition=true matches the pinned lowering's explicit has_value=true;
trait members use genuine default body/type/initializer presence. Nonconditional
const_conditions is NotApplicable, matching the compiler's invalid-query branch.
Opaque/default-clause/RequiresSizedSelf gaps stay pending rather than empty.

Two source-review corrections were sent and the worker reports them fixed:
has_target_features has a valid Static domain (rustc_hir/def.rs:423–430), and
rustc_comptime+const still produces Always data after the compiler emits its
separate static diagnostic. No interpreter static rejection is required.
Final Q83 code/registration/currentness/control review awaits its frozen hash.

## New root input transport source pass

Inspected `root_variables::RootVariables`, `RootVariableKind`,
`source_terms::encode_root_predicate`, the shared `Encoder::ty`, and the new
source-backed observation in `source_predicates::check_actual_projected_source_predicates`.
The map borrows its actual SolverContext, rejects duplicate owned keys before
allocating, and uses the existing pinned inference client's next_ty_var,
next_int_var and next_float_var. These preserve the upstream TyVar/IntVar/FloatVar
families (pinned rustc_infer/infer/mod.rs:911–965). A repeated source key returns
the same handle. The source encoder now accepts Infer only through this explicit
map; declaration encoding remains rigid. The actual owner pointer check rejects
a different simultaneously live inference owner even when its Cx is identical.
It is an ownership check, not the invalid fresh-context pointer-inequality test
described above. No borrowed infer handle is exported as an owned answer.

`resolved` uses the original variable and existing pinned
deeply_resolve_ignoring_regions. Its return is still scoped IR; it neither solves
the root nor proves all constraints discharged. Const and region root variables,
owned answer export, real root evaluation and retry/lifecycle evidence remain
unimplemented here. Encoder/root-map cost is a BTreeMap allocation with
O(v log v) input construction and O(log v) per inference-key occurrence; the
ordinary recursive source walk and interner are reused, with no second parser,
solver, candidate transaction or source/DB work in the map.

The cfg observation substitutes the genuine repr128 declaration's U argument in
its retained projected PartialEq recipe and checks actual projection identity,
IntVar identity, repeated encoding, rigid rejection, wrong-owner rejection and
unknown-key rejection. It does not evaluate a root. Review found a cross-slice
control mismatch: its second-clause selection required the old
ResolvedSizedDefault variant after Q78 switched to Q84-produced Explicit rows.
The owning worker repaired this before validation. Rereview confirmed selection
of the genuine Explicit row by the actual Q84 SourceLangIdentity-backed Sized
ID, U ParamKey and exact retained parameter span, with source preparation before
the no-query encoding observation. The upstream/source_predicates.rs repair
hash is `95d18b2c159586a9134d27fa58e1d38c6f57e592039349d4346047e7471c1aaf`.
A terminal is still pending. No inference result or runtime gain follows from
constructing an unevaluated integer variable.

## Pinned default-bound admission and Q78 consumer pass

Inspected `PreparedSourceDefaultBounds::{prepare,apply}`, the mechanically
retained collect/add/should_add/default_traits policy, configuration receipt,
owning-root selection, facade metadata and `tools/upstream_default_bounds.py`.
The generator verifies exact retained source hashes for bounds.rs, context.rs,
options.rs and marker.rs, extracts the complete selected policy bodies, and
audits the actual three marker declarations' hierarchy and semantic attributes.
SourceBound replaces HIR enumeration only after real source subject/polarity/span
admission. Negative written clauses stay typed pending until their transport
exists; static diagnostics are not implemented as interpreter rejections.

The configuration comes from the actual declaration's owning crate root with
matching profile and actual CrateCfg, including effective cfg_attr metadata.
The private NoCompilerDefaultOverrides receipt is limited to the current
validated semantic profile with no compiler-option override input. A future
override profile needs a producer and cannot reuse that absence claim.

Three initial review issues were corrected in the adapter: authenticated
no-implicit configuration returns implicit-only empty without reading pending
written rows; missing Sized no longer blocks the pinned trait-auto/PointeeSized
early exits that need only MetaSized/PointeeSized; and associated-type subjects
now require the real Q77 item ID, parent header/context/layout, child Self
namespace, rebased parent parameters and actual own identity arguments. Missing
applicable language roles remain typed LanguageIdentity, not guessed traits.
The returned empty default list never certifies a complete written clause list.

Remaining availability limits: outside those compiler early exits, an unrelated
Pending written row can conservatively prevent implicit-only application before
exact-subject filtering. Associated subjects currently admit identity arguments;
other instantiations are held. These limits cannot be advertised as complete
compiler query availability. `pending()` reports preparation-level conditions;
successful per-context application still requires its own role and source
checks. Source preparation queries/retains all three marker lookups cold even
when a context needs fewer; hot policy selection uses retained rows only.

Q78 `PreparedDeclarationSourcePredicates::prepare` now calls common
prepare_own for written recipes, applies Q84 only for actual own Type parameters
and genuine trait Self, then retains the written recipes. prepare_own clears the
legacy simple Sized defaults before their old lowering, preventing duplicate
implicit routes. General `source_predicates::prepare` retains the named legacy
simple-Sized path and remains migration debt. Parameter sites are retained
source ranges; trait Self uses the actual trait syntax and authenticated child
slot. Parent written recipes are not demanded for associated own requirements.
This preserves the existing Q80-own/Q81-hierarchy demand distinction.

The subsequent Q82 integration was reviewed separately: SourceItemBounds retains
the actual Q84 memo and applies AssociatedType only to its Q77/Q78-derived source
projection. Output defaults remain separate from written bounds and parent
candidates. Configuration no longer stays unconditionally pending after a
completed producer. The cfg-only check_actual_item_default_encoding captures the
real Q82 plan, uses common require_recipe/SourceBindings/SourceGenericLayouts and
encode_recipe, and checks genuine Sized, parent/item projection identity and
source span. The existing unchanged associated-types-bound control now expects
one actual output default and checks its shared TypeIR encoding while complete
facts readiness remains closed. Source review found no additional blocker; its
twelve-world terminal and independent source/epoch evidence remain pending.

For all eight modes, this delta extends the same ordinary/generated source
query/encoder route: no assertion evaluator, literal macro executor, contextual
method/numeric selection, generated exclusion, pattern engine or executable
cold-arm preparation. Q84 scans immutable written rows per actually requested
subject (O(parameters × written rows) for a generic declaration), retains query
Arcs and actual negative lookup dependencies, and performs no global dispatch
census or BodyIndex copy. Actual source-currentness/invalidation and full matrix
execution remain unmeasured until the final frozen controls; no speed claim is
established. Exact final fingerprint and source/binary guards remain pending.

## Eight-mode design review and required evidence

1. The entry is the actual pinned solver root, not an assertion/expression
   evaluator or parallel trait/projection engine.
2. Actual ordinary/generated source requirements must use the same owned root;
   no scalar-only macro executor or dummy main-entry WF demonstration.
3. Resolve numeric constraints through the actual source predicate before
   fallback, never assertion/method/fixture names or concrete value coercion.
4. Keep actual source/member/hygiene/currentness receipts across all origins;
   missing metadata aborts preparation rather than excluding generated syntax.
5. Query availability does not authorize callee-body/cold-arm/static checking.
   Infer/solver transactions stay upstream; no new pattern machinery.
6. Immutable source plans are shared across retries; only disposable IR/inference
   state is rebuilt. Memoize installed owned answers with their true arguments/
   dependencies; ordinary execution must not replay planning per operation.
7. Completeness/index comparison belongs to cold publication of demanded
   candidate domains, not per-dispatch full-world namespace scans. Negative and
   relevant/unrelated dependency tests remain required.
8. Borrow existing immutable source/query/body structures; do not deep-copy
   BodyIndex or rescan complete files during each phase/retry. Owned term mapping
   has explicit root/fact-size cost; no speed or scaling claim is established.

The minimal source-backed control should withhold one genuinely prepared query
domain, observe the exact typed demand with no successful semantic answer,
prove the entire attempt/diagnostics/cache is discarded, prepare outside the
callback, then solve the same actual root using a fresh context. Use real source
receipts, not a fake Provider. Preserve unknown panic behavior and actual CLI
unwind behavior; tests alone cannot establish dev/release profile support.
Allocator addresses may be reused after drop, so pointer inequality is not a
fresh-context proof. Retain root/input identity and measured attempt lifecycle.

Then replay unchanged repr128 and affected prior witnesses with full capture,
and relevant actual-source scalar/default/query-domain controls across editions
and origins. No builds/tests until explicit global freeze; no acceptance before
all terminals and source/HEAD/test/CLI/separate-digester closing guards. Full
factory/root/dictionary/runtime/architecture matrices and interpreter/JIT gates
remain held until actual evidence, not this design record, establishes them.

## V509 frozen source and failed compilation receipt

The independent source review above is bound to frozen fingerprint
`83cf39afff5fb521a9564bafb722aa567f7fd44adfae22bbd216f90d39d9f2be`,
HEAD `1084e893d05a7020e14aa5583a030984ce365aa9`, 8,709 guarded input files.
The asynchronous direct-checkout Rust1.99 offline/locked library no-run build
terminated 101: source_item_bounds.rs:352 and :379 still passed `core` after
its old local binding was removed. This mechanical mismatch was missed during
the bounded source pass; compilation establishes no component acceptance.
No CLI/digester build, selected control or runtime case ran for V509.

The independent new default-policy audit terminated 0; bootstrap with explicit
`--verify-upstream` terminated 0 with 230 files, 3,937,600 bytes, zero retained
original Rust changes and upstream Git objects verified. Before/after-build/
after-runtime source and HEAD guards matched; no source changed. The guarded
CLI/digester were explicitly the old V508 binaries, respectively SHA256
`4eba828cdab81985c0e701e2a165db180a870ebf9923c6468ceb2bc9f7ae0949`
and `2304e69f3b3e5e5d2e858d97df5b8d76248f1b32d59c6ba81f0f281351abaa1d`.
The separate digester closing comparison matched. All processes were terminal
and source ownership was returned before repair. Raw disposable receipts are
under artifacts/validation-current-v509; exact commands use the maintained
validation workflow and the tracked reached-root-source-controls selection.

V510 is separately bound to fingerprint
`152353d13a9876be8005956a039a991eedfd179e152d2689f4077de008a49657`
at the same HEAD and 8,709 files. The captured manifests differ in exactly one
source: source_item_bounds.rs, now SHA256
`5cf1d6f7205ba1ddf053b7f0f5235751076eab3917189627cdc4dbb1e5e38fcf`.
The repair restores `core = environment.language_item_core(frame.profile())`
for the existing output/parent collectors. This uses actual owning source
metadata and does not introduce name-based language identities or change the
eight-mode review above. Builds/control/runtime and closing terminals remain
pending for this fingerprint; V509 failure is not replaced by a pass claim.

V510 final terminals: lib no-run 0 (40.62 s), CLI build 0 (41.50 s),
digester build 0 (0.58 s). The six exact tracked selectors terminated
0/0/0/101/0/0, respectively 29.657/29.089/30.570/2.676/47.435/23.666 s.
Observed completed source-world rows were 12/12/12/0/12/4. Repr128's component
observed 36 actual own clauses and 12 projected subjects; output bounds observed
12 written and 12 default recipes; the GAT parent control observed 24 parent,
12 own clauses and 24 actual Q83 associated declaration observations. The two
constructor controls observed 264/264 and 1,624/1,624 operations/completions.
Each selected body ran exactly once with source inputs unchanged.

The failing HRTB control stopped at upstream_source_clauses.rs:209, where it
still expected the old Recipe(Bound(HigherRanked)) classification. The log did
not print the actual parent pending variant. Earlier own-bundle/no-parent-demand
and hierarchy-Parent assertions completed before that failure, but zero world
completion lines were emitted and later cold/currentness/revocation checks in
that body were not executed. Source inspection suggests Q84's newly demanded
trait-Self default preparation now produces a Source(DefaultBounds(WrittenRecipe))
hold; this is a source-path diagnosis, not a measured enum value. No source
repair or additional test ran before all processes and guards closed.

The unchanged owning-edition runtime batch terminated 1: associated constructor,
generic-derived-type and empty-generic-brackets each passed all 13 runtime checks;
repr128 retained executor LRR-E-OP-TYPE, `operator requires supported concrete
primitive operand descriptors`, without passing runtime checks. There is no new
runtime gain, native comparison or full-fixture/diagnostic/JIT acceptance. Its
complete disposable report is
/private/tmp/lrr-v510-reached-root-source-runtime/report.json. Source, HEAD and
test/CLI after-runtime guards matched with zero source changes, and the separate
fresh digester matched its after-build hash:
`b6ea1f5bfb3df82c145d2884539a7cfb29f626334d00a4cb259c562aaf74d69e`.
The actual Cargo JSON test binary SHA256 is
`2e7cea65e6a2267b01bf8f6e96c4ffad85c5ede72a43346e7b0a4b9259934c40`;
fresh CLI is
`45457ed83759a1a26b747e8a2b9e0000d49cfde08b74910c7874cbf243b26315`.
All terminals and parsed observations were preserved before explicitly returning
source/Cargo ownership. V509's new-policy/bootstrap audits apply to unchanged
policy/raw/config/facade inputs; V510 changed only the restored source-item
collector binding. Root evaluation, response export, missing-fact suspension,
actual repr128 acceptance and the host panic policy remain held.
