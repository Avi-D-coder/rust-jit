# M019/M020 / LOCAL-049: actual reached-call relation adapter

Preimplementation bounded architecture record. This slice owns only new
`traits/upstream/root_relations.rs` and this record. The lead owns shared encoder
accessors, source-fact materialization, caller/root activation, module registration,
reached-call capture changes and validation. No Cargo or Git mutation belongs to
this slice.

The invariant is that reached source equality/coercion constraints use exactly
the caller's actual pinned `SolverContext`, its `RootVariables`, genuine declared
generic ordering, certified caller assumptions and original source spans. The
minimal witness is a generic call whose formal associated output and actual
unsuffixed numeric variable constrain one another; separately encoding rigid
callee parameters or defaulting the numeric value before real relation/fulfillment
loses that contract. The unchanged runtime acceptance target is
`tests/ui/enum-discriminant/repr128.rs`, not a source-name or fixture switch.

Existing ordinary route: `RunMachine::capture_reached_call_inputs` retains actual
formal/argument/result terms and source sites in `SourceRootCallInput`; its runtime
slot/call/currentness ownership stays in the engine. `SourceGenericLayouts`
retains source declarations and `DeclarationParameters` maps actual parent-first
indices. The existing `source_terms::Encoder::ty` handles shared typed source and
root inference keys; no new walker/parser is authorized. The lead is exposing
`encode_root_type` through that same encoder. Source formals must first become
shared IR and then instantiate with pinned `EarlyBinder` over actual call
arguments; no independent source substitution/unifier is added.

`rustc_type_ir::relate::solver_relating::RelateExt::relate` is the genuine shared
equality entry. Pinned compiler `infer/at.rs::eq_trace` uses `Invariant`, preserving
all returned child goals. The shared `PredicateKind::Coerce(CoercePredicate)` is
handled by pinned `rustc_next_trait_solver::EvalCtxt::compute_coerce_goal`, which
uses subtype constraints. Preserve it as a real obligation, actual source ->
formal target for arguments. The result expectation has the opposite direction
(callee output -> caller expectation); the existing DTO currently fails to mark
that distinction, which is reported to the lead for the shared capture repair.
The shared Coerce predicate is not rustc HIR's complete adjustment algorithm:
unsize/reborrow/function-pointer adjustments and their runtime representation are
not certified by this adapter.

Caller assumptions must come from `PreparedSourceCallerClauses` and, when present,
the exact `VerifiedCallerApplication` via `encode_applied`; the genuinely bare
caller path uses `encode`, which still requires actual source-proven emptiness.
No raw arbitrary ParamEnv input is admitted and no callee bound enters caller
assumptions. The runtime root owner separately revalidates active handles,
configuration, currentness and committed effects. Missing source/type/generic or
caller evidence remains typed pending before relation mutation.

Prepare all relation terms once from immutable source input, then use the actual
owner's existing pinned snapshot commit/rollback API for the relation batch. Keep
actual child goals, spans, original relation identity and TypeErrors. Do not run a
second fulfillment scheduler. Successful inference constraints remain in that
owner and still need the same admitted multi-root fulfillment/root response/VM
commit. There is no source query, source parsing, global namespace scan, body
preparation or guest effect in the adapter. Cold bodies stay cold; execution
continuations remain with the engine. Costs are linear over reached relation
inputs and actual generic parameters plus the work done by pinned encoding,
substitution and inference. No performance claim follows.

Before implementation, all eight architecture modes were reviewed: (1) no
assertion evaluator; (2) no generated-expression executor; (3) no macro-specific
numeric/member inference; (4) no generated-source exclusion; (5) no pattern engine
or cold-arm executable check; (6) no parsing/planning at installed execution;
(7) no full-world namespace/cache invalidation; (8) no immutable BodyIndex copy or
whole-file search. Existing MA001-MA008 paths and legacy custom trait/unification
routes remain separately held; this new direct adapter does not claim their
removal. Record final source fingerprints and a separate source-review pass
before integration. Unchanged runtime/origin/edition controls, root activation,
child-goal fulfillment, cleanup/suspension and every milestone/interpreter/JIT
gate remain unexecuted here and are lead-owned acceptance obligations.

## Source review and frozen handoff

Implemented only `crates/lrr/src/traits/upstream/root_relations.rs`, SHA256
`97f67443ba30b28b29688a1994df0f1033723d4c2ef9ad56cd56cd4cfc316c1b`.
`rustfmt +1.99.0 --edition 2024
crates/lrr/src/traits/upstream/root_relations.rs` exits0. No Rust type check,
Cargo operation, language control or Git mutation was performed by this slice.
The lead supplied the common `encode_root_type` and added the actual
`CoerceResult` source-input distinction before this implementation. The file is
unregistered until the lead's first combined compile.

`PreparedRootRelations::prepare` takes the actual `SolverContext`, its borrowed
`RootVariables`, the actual `SourceGenericLayouts`, configuration, reached input,
retained `PreparedSourceCallerClauses` and optional actual
`VerifiedCallerApplication`. There is no raw ParamEnv entry. Both caller branches
run the existing source-proof encoder; absence of the application object cannot
bypass `encode`'s actual caller descriptor/source-completeness checks. Variable
specifications must exactly match the actual owner's map. Layouts share the
caller's retained catalog; configuration and definition worlds are checked.
Actual callee `DeclarationParameters::from_upstream` supplies every argument in
the pinned parent-first order. The exact input parameter map must be complete;
non-Type arguments and missing metadata remain typed pending. The common encoder
encodes formals and actuals, then pinned
`EarlyBinder::bind(cx, formal).instantiate(cx, actual_args).skip_norm_wip()`
instantiates formals. The unwrap retains projection normalization for the real
relation/solver and makes no normalization-success claim.

`PreparedRootRelations::apply(self)` consumes the prepared batch and uses only the
existing pinned `owner.commit_if_ok` and shared `RelateExt::relate(&**owner,
param_env, formal, Invariant, actual, site)`. Every child Goal becomes an actual
`PredicateObligation` with unchanged predicate/ParamEnv, original relation cause
and source span, and root recursion depth0 as in pinned `At::goals_to_obligations`.
Coercions produce the exact `PredicateKind::Coerce`: actual argument -> formal
parameter, and formal result -> actual caller expectation. They are returned as
unfulfilled obligations, never reported as successful adjustment conversions.
Any equality TypeError is returned with its cause and both actual shared Types;
the existing snapshot rolls back the entire failed batch, so no failed partial
inference mutation survives. Successful constraints remain in this exact owner.
`AppliedRootRelations::into_obligations` checks owner pointer identity and the
retained snapshot count before transferring every goal. It cannot accept a
replacement goal or ParamEnv. All actual child/coercion obligations still require
admitted same-owner multi-root fulfillment, and no selected witness/VM commit is
issued.

Separate source-review pass against the frozen hash:

1. `prepare`/`apply` have no expression/assertion evaluator; MA001-MA003 are not
   extended.
2. Both source-origin paths use the same `encode_root_type`, typed IR and pinned
   relation APIs; no literal/break-only macro executor or MA004/MA005 route grows.
3. There is no macro name, method whitelist or numeric default. Numeric variables
   preserve their actual `RootVariables` families; ordinary shared inference
   owns the result. MA006/MA007 remain held elsewhere.
4. No generated-source refusal is added. Missing caller/source metadata is typed
   pending with the exact relation/side/parameter identity.
5. No patterns, cold branch/body preparation, borrow checker or whole-body
   inference is introduced. Only captured reached equations are consumed.
6. Source terms are encoded once into retained shared handles; `apply` never
   parses, re-encodes source, queries the DB or constructs a second plan.
7. No namespace census, source publication scan, dependency guard or cache is
   added. Relevant preparation/currentness and complete callback admission remain
   with the actual source factory/root owner.
8. No `BodyIndex` clone or full-file node search exists. The prepared plan retains
   typed Type handles and compact original-relation identifiers, not copied
   source bodies. Work scales with actual generic arguments/relation terms plus
   pinned encoding/substitution/relation work; no speed claim is made.

Mechanical lifetime pass: the plan and applied result borrow the same shared
`SolverContext`; root variables are checked before encoding. Every IR handle has
the existing context lifetime. `apply` takes no replacement owner; result transfer
rejects a foreign owner and asserts the original snapshot. Caller/source receipts
are consumed by their existing source encoders, while the lead must retain and
revalidate actual runtime packet/activation/currentness before entry and commit.
Complete fixture, source-origin/edition, rollback, child-normalization, coercion,
negative-source, suspension/cleanup and VM commit execution are unexecuted here.
All remaining earlier architecture/runtime/JIT holds remain attached to this
source-only component. The authorized validation/review agent was sent this exact
fingerprint for an independent read-only pass before combined validation.

## V524 read-only root-admission findings

This follow-up changes this excluded documentation file only during the lead's
whole-source freeze. It runs no Cargo, tests, source generator or Git mutation.
Inspected source fingerprints: `prepared.rs`
`8dc69f71a841663d589465ecd5aa57e5db1fc2504c947778f39e8edabec053f6`,
`source_factory.rs`
`34bcb8af724fc25da55f71135c12715a07275c11bdc6114409d2fa90525eb693`,
`context.rs`
`e1c3c7b3b522d2e848314a77b12e28fa1adcc0651af4471c0e9066f7b02832d6`,
`infer/delegate.rs`
`0cf91947b833fbb373ab70ee541aefaf6264678930d2666403230d5617c51a76`,
`root_response.rs`
`d37d63dc68ddecc90a28267f3345c8755b265900ae47841498003e599c11c9b4`,
and `frontend/binding_constraint_generics.rs`
`410279d968022da849d2b87db4626dd18e310f4bed470bcb07e83936d1d7a5e4`.
The relation file remains at its previously frozen fingerprint.

### Exact normal/proof route boundary and correction

`PreparedProvider::publish` validates source ownership and creates the pinned
candidate indexes, then fills its OnceCell. `ready` requires every field on every
published definition/trait/impl and all associated leaves, and returns only a
`PreparedIndexSnapshot`. That coarse snapshot neither certifies a particular
input packet nor activates a root. `with_provider` initializes `root_active` to
false; no writer exists in the inspected source. The only positive assertion on
that cell is `PreparedProvider::evaluate_root_goal_for_proof_tree_raw`.

The earlier message that normal canonical evaluation necessarily reaches this
assertion was too broad and is superseded here. Pinned normal
`SolverDelegateEvalExt::evaluate_root_goal` calls `EvalCtxt::enter_root`, which
constructs its own `SearchGraph` and a no-op inspector. Its
`evaluate_goal_no_fast_paths`/`evaluate_goal_raw`/`evaluate_in_search_graph` path
calls the shared graph directly. Proof evaluation instead calls the interner's
`evaluate_root_goal_for_proof_tree_raw`, which forwards through `Context` to the
guarded provider callback. Thus `root_active == false` alone does not mechanically
gate direct normal evaluation today. `Context::with_global_cache` also directly
borrows the context's cache without checking source-root admission.

A concrete adapter-only repair can use the pinned overridable
`SolverDelegate::cx` in the local `SolverContext` implementation: normal root
entry obtains `self.cx().recursion_limit()` before `EvalCtxt::enter_root`, so a
source-session assertion there precedes normal graph entry. The local
`SolverContext::compute_goal_fast_path` and the packet-bound fulfillment entry
must enforce the same session as well: registration can succeed or emit region
constraints through a fast path without a graph/cache call. A provider/session
check before `Context::with_global_cache` borrows the cache is useful additional
protection against answer reuse. Cache checking alone is insufficient: the
pinned graph tests depth/provisional results before the global cache, and fast
paths can avoid the graph entirely. The proof callback must consult the identical
private session. This changes LRR ownership transport, not upstream solving.

The session must bind the actual original inference owner and reached input set,
provider/context, source world/configuration and admitted closure. Canonical child
delegates created by pinned `SolverContext::build_with_canonical` belong to that
same private session lineage; their distinct InferCtxt addresses are not foreign
runtime root owners. Require lineage when constructing those children rather than
mistakenly comparing each child to the original root's address. A session boolean
without the private packet/owner lineage is not the complete contract. Keep the
new entry unavailable until the packet and query closure are certified; do not
expose a `set_root_active`/arbitrary-goal adapter.

### Numeric demand and the existing common source service

Section52.2's numeric exception explicitly permits collecting the connected
numeric source constraints before fallback, including later uses, without guest
execution/thunk preparation. Together with Section53.0 argument/result inference,
this permits a genuine input-inference demand from the projected
`PartialEq<U>` constraint connected to an observed unresolved numeric argument.
My earlier suggestion that this necessarily waits for the body dictionary use
was too restrictive. Section53.3 still forbids forcing all ordinary callee
clauses: disconnected ground/unused bounds must remain unforced. The inference
predicate is an obligation proved using the actual caller ParamEnv, never an
assumption injected into that ParamEnv.

There is no literal `source_numeric_inference` symbol in this inspected tree.
The actual source service is `frontend::binding_constraints::prepare` and its
`Component`, fed by the immutable `SourceConstraintPlan` through
`db::source_constraint_provider::prepare`/`QueryKey::SourceConstraints`.
`Component::constrain_declared` and
`binding_constraint_generics::{relate_parameter,relate_signature_type}` connect
the occurrences of a declared generic parameter across actual arguments/results.
That source/dataflow ownership should also supply the connected obligation
demand, including parameters occurring in another predicate's generic arguments
or projection, rather than making call entry force every declared predicate.
For this reached shape the real equations connect callee U to the original
Integer-family root variable; the source clause
`<T as DiscriminantKind>::Discriminant: PartialEq<U>` connects that same U to the
projection. Submit that genuine clause and its projection dependencies through
the pinned root owner, with all original substitutions and spans.

Do not reuse `Component::parameter_bounds`/`trait_bound` as a replacement solver.
They are historical migration debt: `trait_bound` calls
`MemberIndex::primitive_candidates`, chooses a primitive from candidate
cardinality, recursively combines blanket answers and uses
`NATIVE_PRIMITIVE_TRAITS` plus a hardcoded128 depth check. That is not the new
upstream selection/recursion route. Retain the useful source connectivity and
query/memo/currentness mechanism, route semantic inference through actual
upstream goals, and hold/remove the superseded bound-answer path as consumers
migrate. No new numeric default, primitive candidate enumerator or static checker
is justified by this review.

### Required source-query closure for the concrete reached shape

The exact demanded shape is a real caller-certified PartialEq predicate whose
Self is the DiscriminantKind associated projection of the actual enum and whose
argument is the original numeric inference variable. The same mechanism must
derive this shape from source types/relations, never a fixture/callee name. Read
all requirements before root execution; callbacks are immutable typed readers
and cannot query the DB, parse source, execute macros/guest code, or turn a missing
QueryFact panic into a successful solver answer/ordinary NoSolution.

| Necessary domain | Exact existing producer / solver consumer |
| --- | --- |
| Actual caller/callee/header identities, source spans, generic kinds/order and root Type bindings | `DefinitionHeaderPlan`, `DeclarationGenericTypes`, `SourceBindings`, `SourceGenericLayouts`, `PreparedSourceCallerClauses::{encode,encode_applied}`, `PreparedRootRelations::prepare`; consumed by `generics_of`, `projection_parent`, `check_alias_term_args_compatible` and real EarlyBinder instantiation. |
| Connected source predicate, its original owning binder and actual substitutions | `DeclarationSourcePredicates`, `SourceDeclarationClauses` and its hierarchy, shared `encode_root_predicate` and compiler EarlyBinder; must be demand-filtered for the observed numeric component rather than all callee bounds. |
| Complete positive/negative candidate membership for every demanded trait, even a genuinely empty builtin-trait bucket | `ImplTraitCatalog`, `SourceFactRequest.traits`, `SourceFactBundle::candidates`, `TraitFacts.implementations` and `CandidateIndex::prepare`; pinned `assemble_impl_candidates` still invokes `for_each_relevant_impl` for a builtin DiscriminantKind candidate, so builtin success does not license an unprepared source bucket. |
| Every potentially relevant retained source impl head and its independent scalar/header constraints | `ImplSourceTypes`, `SourceImplSemantics`, `ImplSourcePredicates`, `SourceImplOutlives`; consumed by pinned `trait_goals::consider_impl_candidate`: `impl_trait_ref`, `impl_is_default`, `impl_polarity`, `generics_of`, complete `clauses_of` and `impl_super_outlives`. Do not drop members because one of these is unprepared or predict which impl will succeed. |
| Real trait classification and compiler language identities | `SourceTraitSemantics`, `SourceLangIdentity`, filtered `SourceTraitClauses`; consumed by trait auto/alias/marker/coinductive/sizedness callbacks, per-definition lang tests and builtin assembly. Missing role lookup is unprepared, distinct from a proven non-lang-item or absent role. |
| Associated type identity/parent/argument namespaces and conditional alias-bound domains | `TraitAssociatedTypes`, `SourceAliasLayouts`, original generic/header metadata; consumed by projection `trait_ref`/`own_args`, builtin dispatch, and alias-bound candidate assembly. `own_clauses_of`/`item_self_bounds`/`item_non_self_bounds` are additionally required when actual source/env/rigid-alias branches reach them; source shape must certify their omission, not a guessed normalized answer. |
| Actual enum kind and explicit repr-derived discriminant Type | `SourceAdtFacts { fields:false }`, `AdtInfo.kind` and `.discriminant`; pinned `normalizes_to::consider_builtin_discriminant_kind_candidate` calls `Ty::discriminant_ty`, LRR's complete pinned `pinned_discriminant_ty`, then `AdtDefinition::enum_discriminant_type`. This requires no field bodies, discriminant value evaluation, or handwritten mapping keyed on enum/fixture name. |

Pinned `assemble_and_evaluate_candidates` first structurally normalizes Self,
then uses its builtin/source/param/alias candidates and merges them. The primitive
comparison candidate can consequently constrain the original numeric variable to
the actual discriminant Type. Let upstream do that; the table above is input
closure, not applicability/selection logic. If source candidates can require
specialization, associated leaves or nested projections/predicates, their exact
upstream metadata domains join the closure. A retained source absence certificate
may answer a genuinely empty domain; do not manufacture `specializes=[]` from a
desired unique result. Complete closure includes the predicates/headers of failed
candidates, not just the final selected one.

The current factory does not yet certify this closure. In particular:
`DefinitionFacts.predicates.{item,item_self,item_non_self,explicit_item}` and
variance queries remain unprepared; full `all`/`own` clauses are installed only for
Q86's genuinely complete empty-impl branch, not arbitrary nonempty impl predicates.
Trait full `associated_items`/`supertraits`, impl specialization ancestry, and
associated leaves remain unmaterialized. `MaterializedSourceFacts::unprepared`
records encoding failures, not an exhaustive required-query list; empty holds
cannot admit a root. Conversely coarse `ready`'s every-field/every-leaf census
requires unrelated constant/coroutine/closure domains for this shape and is not
the minimal demand proof. A header-only dependency closure rooted in actual
demands may conservatively include every possible candidate/header branch while
leaving unrelated bodies cold. This is LRR source scheduling, not a replacement
search graph. These necessary families have source evidence, but sufficiency and
the actual callback-read subset remain unexecuted until an admitted root trace
validates them; do not silently call this table a passing closure certificate.

### Actual response and commit evidence

Pinned `canonical::instantiate_and_apply_query_response` instantiates canonical
variables, unifies `var_values` back into `original_values`, registers old/new
region constraints and newly defined opaque types into the same inference owner,
and returns normalization nested goals. Normal `evaluate_goal_no_fast_paths`
requires its top-level normalization-nested-goal storage to be empty. This work
already belongs to upstream; do not replay canonical responses with a local
unifier. `GoalEvaluation.goal` is explicitly the pre-response input with earlier
variables resolved, not the resulting substitution. The actual final values
must be read through the original `RootVariables` after shared multi-root
fulfillment, as `export_root_response` already does.

Required completion evidence is no TrueError/fatal overflow, no residual
ambiguity/overflow MaybeInfo for the forced root set, no pending obligation, and
actual original-variable resolutions. The exporter alone proves none of these:
it can return a portable response while roots remain unfulfilled and its
`unresolved` map is intentionally explicit. The scoped reader must also retain
or classify external region/opaque constraints. Existing readers are
`InferenceContext::{make_deduplicated_region_constraints,get_solver_region_constraint}`
and `clone_opaque_types_lookup_table`/`clone_duplicate_opaque_types`; the source
exporter presently exports neither region constraints nor opaque state. For the
primitive/fieldless-enum demand, verify actual storage is empty rather than
assuming that from the desired answer. Nonempty general constraints cannot be
silently discarded or turned into static borrow checking; preserve the specified
residual/IndeterminateRegion distinction and any interpreter-relevant obligations.

An owned root result must bind world/configuration, the original attempt and
packet/callee/signature identity, root keys/families, actual exported values and
residuals, and relevant source/negative candidate dependencies. It is not a
portable context pointer or an arbitrary externally replaceable answer map.
The engine must revalidate the retained reached packet, actual caller handles
and source guards after preparation and before mutating literals/GenericEnv or
entering the callee. Only the resolved original numeric key may bind the same
validated GuestStore literal handle; no fallback may have fixed it beforehand.
Check exact-once effects and continuation identity across NeedInput. A successful
constraint answer also does not identify an executable impl method: source-bound
selection/witness/default-member evidence remains separately required where the
consumer invokes a trait member. The actual discriminant initializer/intrinsic
continues through its ordinary VM route. No admission, runtime/full-fixture,
cleanup/suspension, performance, interpreter or JIT gate is closed by this note.

## V526-terminal successor: retain the actual source application

After the lead reported V526 terminal and source guard0, this bounded slice was
released for `root_relations.rs` and this record only. The new fingerprint is
`1b5e8861f529b5f915af92ac841c0120e9948a8eeebac462d4a159fb0d455c0b`.
All prior execution/review evidence retains its original fingerprint; no Rust
type check or language test is claimed for this successor. `rustfmt +1.99.0
--edition 2024 crates/lrr/src/traits/upstream/root_relations.rs` exits0. No Cargo,
test operation or Git mutation was performed by this contributor.

The specific invariant was prepare once: the actual Args used to instantiate
formal Types and actual caller ParamEnv used for equality must remain available
for the same source-root owner's numeric-connected clause demands. Previously
`prepare` discarded the constructed Args and `apply` discarded the caller
environment; `into_obligations` exported only goals, inviting a second argument
construction/environment join. The repair is immutable transport, not another
relation or clause solver. The exact actual callee `DefinitionId`, signature
digest, Args, source-proven caller ParamEnv, original owner reference and snapshot
are now stored in privately constructed `SourceRootApplication`. Both
`PreparedRootRelations` and `AppliedRootRelations` retain that same value.

Concrete consumer API, coordinated with `/root/source_fact_bundle`:

- `AppliedRootRelations::source_application(&self, owner)` returns a borrowed
  `SourceRootApplication` only after checking original owner pointer and snapshot.
- `SourceRootApplication::{callee,signature,arguments,param_env}` read the exact
  retained values and recheck its original snapshot. `require_owner(owner)`
  rejects an application from another owner even if both share an IR lifetime.
- `AppliedRootRelations::into_parts(self, owner)` preserves the actual application
  and every original obligation in private `RootRelationParts`.
- `RootRelationParts::source_application(owner)` exposes the same checked view;
  `into_application_and_obligations(self, owner)` consumes both original values
  together. The transferred application retains the original owner borrow.
- The obsolete obligation-only `into_obligations` entry had no external source
  consumer and was removed. Earlier references to it describe the old hash.

No API takes caller-supplied replacement Args, ParamEnv, callee ID or signature.
There is no public application/parts constructor or mutable field. The original
source packet, configuration/currentness/activation proof and admitted root
requirements remain external; this view is explicitly not a root certificate.
The genuine upstream equality, transaction, Coerce/CoerceResult and child-goal
algorithms are unchanged. Cold cost adds fixed-size retained handles/identities;
the read/transfer checks use pointer identity and snapshot depth. No source
walking, encoding, generic argument rebuilding or query is added on those reads.

Separate source-review pass against the new hash rechecked all eight modes:
(1) no assertion/expression evaluator; (2) no generated syntax executor;
(3) no macro/member/numeric semantic switch or default; (4) no generated-source
exclusion; (5) no pattern/body/cold-arm work or whole-body checker; (6) retained
Args/ParamEnv remove the need for repeated source encoding rather than adding
planning; (7) no namespace/global dependency scan or invalidation; (8) immutable
Type/Args handles are moved, with no BodyIndex/source-body clone or search.
The prior original owner/snapshot/typed-error and all-child-goal guarantees were
reviewed through the new application field; source provenance and all acceptance
holds remain unchanged. The authorized reviewer received the exact new hash for
independent read-only review. Compilation and actual source-clause use belong to
the next lead-owned frozen validation terminal.

## Pre-V527 correction: pending relation goals are not settled arguments

The lead identified a required cross-API precondition before freezing V527.
`apply` executes explicit Equal constraints, but it only emits argument/result
Coerce goals and equality child goals. Thus a callee U may still resolve to a
Type variable until actual fulfillment relates it to the original Integer/Float
variable. Running numeric-incidence observation at that point could incorrectly
classify an empty observation as a closed disconnected constraint.

The corrective source fingerprint is
`1b17566242f395b334fc164b0e0d449cb550454c41f50a7e9ee23cd34e6f3eb0`.
`SourceRootApplication` now privately retains `outstanding_relation_goals`, set
from the actual successful `apply` output vector's `len()` before any application
view is exposed. Prepared values are private and have no view accessor; a failed
apply returns no application. Every borrowed/consuming transfer carries the same
immutable requirement. `outstanding_relation_goals()` checks the original
snapshot and returns that actual count. There is no setter, decrement, externally
supplied fulfilled flag, fabricated child goal or method to mark completion.

Coordinated contract: the source numeric-demand helper first checks
`application.require_owner(owner)` and this actual count. A nonzero result
returns its typed `Pending::RelationGoals(count)` before numeric incidence
classification; it does not solve/clear obligations or treat no observations as
closed. Zero certifies only that actual apply emitted no deferred relation goal,
not that any solver root/caller activation/VM result is accepted. A future
`AdmittedRootSession` must consume the real original relation goals through
pinned fulfillment before issuing its separate settled-application proof. This
component cannot mint that proof. The count is provenance of unmet integration,
not a resource-accounting limit or an inference/semantic shortcut.

The new field/read/check adds no source query, executable preparation, source
walk, copying of bodies, namespace scan, numeric fallback, static checking or
independent relation/solver. All eight prior source-review conclusions remain
applicable except the old application's pending-relation precondition, which is
explicitly superseded by this requirement. Actual equality/coercion algorithms,
original obligation/span/ParamEnv retention and rollback stay unchanged.
`rustfmt +1.99.0 --edition 2024
crates/lrr/src/traits/upstream/root_relations.rs` exits0. No Cargo, tests or Git
mutation ran here; actual combined helper/compile acceptance remains pending.

## V527 read-only: finite metadata closure and private session lineage

Only this excluded record changes during the V527 freeze. No source/tool edits,
Cargo, tests or Git mutation are performed. The relation source remains
`1b17566242f395b334fc164b0e0d449cb550454c41f50a7e9ee23cd34e6f3eb0`.
Additional inspected hashes: pinned-candidate traversal adapter `candidates.rs`
`982bbad588cd129ab6a5acec73d88748ddcad4f8f8853f714e4e1b60acc0fa58`,
`infer/region_response.rs`
`ee94ee89d9b459ada6bf4c9c79391070b1a898b3f2da16b854577d7b2add3814`,
`infer/solver_region_constraints.rs`
`a02bb227640932639a059dfb58abcf4e042ffab6eaf7b1f9ce7325bfee2972f8`.

The finite preparation key space should be actual `(DefinitionId, query family)`
plus actual alias-kind/lang-role/configuration keys. Prepared values are genuine
EarlyBinder/header templates; do not instantiate an unbounded recursive trait
goal tree during preparation. For a finite completed source catalog, revisiting
a header/query key adds no new source metadata. A deduplicated source-dependency
agenda is consequently finite without an LRR fuel/count/depth budget. This agenda
does not solve goals, declare semantic cycles successful, schedule solver
fixpoints, select candidates or canonicalize; those remain entirely upstream.

Concrete preparation strategy:

1. Seed from the exact reached packet's relation Types, actual caller/callee
   declarations and source constraints connected to original numeric arguments,
   not every unused callee clause. Retain actual query values and guards.
2. Acquire complete `ImplTraitCatalog` membership for demanded traits. Prepare
   typed heads for every member needed by existing `CandidateIndex::prepare`
   and use that pinned classifier. An unprepared head never permits omission
   from membership or publication of a smaller bucket.
3. For a fixed non-alias self constructor, existing pinned
   `PreparedProvider::for_each_relevant_source_impl` can enumerate constructor
   group plus blankets into a metadata-demand recording callback. It must not
   perform local matching/applicability selection. Original Integer/Float
   variables use the same traversal's actual pinned primitive-family groups;
   do not duplicate its list or pick a numeric width.
4. A projection/unknown self requires a conservative union of constructor
   groups upstream normalization/inference may expose. Calling the traversal
   on an unnormalized alias and admitting only its blankets would be unsound:
   its own comment says normalized aliases are handled by a later compiler
   phase. Without a narrower source-backed bound, prepare every member's
   relevant header metadata in this demanded trait bucket. This is conservative
   header preparation, not a whole-world/body scan or predicted answer.
5. For every possibly visited candidate prepare the actual callback families
   in pinned `consider_impl_candidate`: generics/head, default/polarity flags,
   complete `clauses_of` and `impl_super_outlives`. Add declaration dependencies
   named in those templates. Actual alias/auto-trait/dyn/const/fn/coroutine or
   specialization shapes add their respective families; no executable body
   checking/compilation/preparation follows.
6. Stop before root entry when a required family is unavailable. Known empty
   needs a real completeness producer. `NotApplicable` is a domain fact, not an
   answer if a callback can actually run. Validate the query-family certificate
   against its required slots rather than an empty encoding-holds list or
   `DefinitionFacts::prepared()`'s unrelated every-field census. Once admitted,
   record actual reads and check they are within the certificate. Callbacks
   never discover missing source through DB work or panic-to-NeedInput recovery.

For this concrete enum-projection comparison shape, the relevant frontier is
actual enum kind/explicit-repr discriminant metadata, DiscriminantKind projection
identity/layout/parent, complete comparison/builtin-trait candidate buckets,
trait flags/lang classification and possible candidate headers/clauses.
`SourceAdtFacts { fields:false }` suffices for the enum discriminant builtin itself;
it does not require fields or discriminant initializer values. A genuine
zero-generic primitive comparison impl with Q86's complete empty-clause proof
does not need a clause solver. Possible blanket/nested candidates with nonempty
clauses require the full compiler clause producer; written-only Q80 rows are not
`clauses_of`. Auto-trait fields, fn signatures, const evaluation and associated
leaves join only when a reachable actual shape permits those callbacks. This is
necessary source reasoning, not an executed sufficiency certificate.

The root owner must preserve one inference family across exact relation goals,
numeric goals and original-variable export. Current immutable
`outstanding_relation_goals` correctly prevents pre-Coerce empty observations
being treated as settled. A real admitted session consumes actual original goals
through pinned fulfillment before issuing a privately constructed settled
application. General mutually constraining demands may need a joint actual
upstream root set: retain the real source relation provenance rather than create
an LRR solver retry engine. No caller completion flag/decrement is appropriate.
The full certificate producer and such general scheduling remain unimplemented.

### Normal/proof lineage and the exact adapter boundary

The source-owned entry should be a private packet-bound operation returning an
owned result through the existing nonescaping context scope. It must not accept
`(snapshot, arbitrary Goal, arbitrary ParamEnv)`. Before relation application,
fast-path registration or graph entry, certify actual attempt/input/caller
currentness, provider/world/configuration, query closure and original variables.
Active state needs the private session identity/lineage, not only a true bool.

Local `SolverContext` can override pinned `SolverDelegate::cx`; normal evaluation
uses it before `EvalCtxt::enter_root`. The local `compute_goal_fast_path` and
packet-bound fulfillment entry need identical lineage checks because fast paths
can hold/stall or emit constraints without graph/cache work. Before
`Context::with_global_cache` reads/inserts, a provider-specific session hook can
close its current unguarded direct borrow. That generic Context path needs an
explicit source-owned `SourceFacts` hook, not a downcast/default fake-ready
provider. The existing proof raw callback must use the same lineage and actual
immutable fact/index ownership. Cache-only guarding misses earlier fast paths,
provisional/depth results; source wrapper plus adapter guards are complementary.
Shared search/canonicalization/candidate algorithms need no changes.

Canonical children have fresh InferCtxt addresses. Pinned
`SolverContext::build_with_canonical` should carry derived private lineage only
under the admitted provider session, validating provider/context/session rather
than comparing every child to the original root pointer. Original
SourceRootApplication/RootVariables still require exact original pointer
identity. No arbitrary caller gets a constructor for a lineaged child/replacement
root. Proof inspection may use upstream's separate proof search/cache path, but
cannot authorize a different root. Outer active-attempt/signature/currentness
checks still precede commit. Higher-ranked scopes/private constructors keep
borrowed handles from escaping as portable authority; caches never survive
changed source-fact providers without the actual source dependency checks.

### Lossless external capture on the original owner

Pinned `EvalCtxt::make_canonical_response` returns old/new external region
constraints only for `Certainty::Yes`; ambiguous responses intentionally use an
empty old vector or true nextgen constraint. Empty constraints on a Maybe answer
therefore prove no completed fulfillment. Canonical responses return only newly
defined opaque types; canonical apply already merges them into the original
owner. Do not replay those responses with a local unifier or apply constraints
twice. Read original variable resolutions, not `GoalEvaluation.goal`.

`make_deduplicated_region_constraints` is a solver response helper, not a lossless
exporter: it merges duplicates, omits origin spans and asserts Verify is empty.
For actual capture use non-destructive `with_region_constraints` to clone
`RegionConstraintData { constraints: Vec<(Constraint,Span)>, verifys }`, together
with original `InferCtxtInner::{region_obligations,region_assumptions}` as needed.
Nextgen `get_solver_region_constraint` retains the spanned tree. Do not invoke
`take_and_reset_data`/`take_registered_region_assumptions` during a snapshot or
clear state to make it appear empty.

Opaque capture should prefer `clone_opaque_types()`: it retains
`ProvisionalHiddenType { span,ty }` and includes lookup plus duplicate entries.
The actual storage comments say lookup-only iteration is for canonicalization;
`clone_opaque_types_lookup_table` omits duplicates and spans, while
`clone_duplicate_opaque_types` also loses spans. Portable export must preserve
keys, actual Types, source identities and origins or return the specified typed
unexportable/residual condition. Never leak arena pointers or add static
region/borrow checking.

The primitive/enum instance may establish actual old constraints, Verify records,
type-outlives obligations, assumptions, nextgen tree and every opaque entry empty
after fulfilled Yes roots. That is a measured terminal condition, not a type-name
assumption. Such a receipt plus actual original numeric resolutions can support
a bounded primitive result while general owned constraint export stays held.
Nonempty state needs the specified owned residual/IndeterminateRegion policy,
not automatic rejection of borrowed programs. Actual witness/member/default
evidence, literal handles, active attempt/signature/currentness and VM commit
remain separate. No actual root admission or runtime gate passes in this note.

## V527 source function-pointer header prerequisite (before implementation)

M019/M020 / LOCAL-049: the demanded complete PartialEq source bucket reaches
the ordinary facade `ptr.rs` function-pointer impl head. The current
`source_terms::Encoder::ty` rejects every Function term because historical
E-R5-008 runtime terms erased ABI/safety. The invariant is to build pinned FnPtr
Types only from real retained source signatures, preserving binder variables,
parameter/result Types, ABI, safety and C variadic status. The unchanged repr128
runtime target only exposes this shared prerequisite; no fixture or function-name
selection belongs in the encoder.

The common frontend FnPtr/symbolic-header walker, owned by the source-fact worker,
retains `FunctionSignature::source_header: Option<SourceFunctionHeader>`; ordinary
substitution carries it. This slice extends only the existing Function arm of
`source_terms::Encoder::ty`, using that same recursive encoder, actual binder
stack and Context list allocators. Pinned `rustc_type_ir::FnSigTys`, FnHeader,
FnSigKind and `rustc_abi::ExternAbi::from_str` supply the shared representation
and ABI vocabulary. None/unknown ABI stays FunctionMetadata. Legacy runtime
erasure is unchanged and cannot be promoted to source evidence. The existing
unsafe extern alias control must preserve that hold or verify actual qualifiers,
never silently become safe Rust.

This is immutable demanded header preparation, linear in the actual signature
size and retained binder count. No body lowering, cold-branch executable work,
runtime ABI call, native symbol, namespace census, solver algorithm or runtime
entry work is added. Parent owns module integration and Cargo validation; this
slice must report its source hash and all-eight source review, while runtime,
full-fixture and root admission acceptance remain held.

### Encoder handoff / source review

Implemented `source_terms::Encoder::ty` Function arm at source SHA-256
`a1d393455352887bb677370e8448fe16483b7454a54840c5490a20d1122c081b`.
`ExternAbi::from_str` uses every pinned ABI spelling, including unwind variants;
unsupported spellings retain FunctionMetadata. The arm pushes the genuine
source binder even when empty, encodes every input and result with the existing
Encoder and checks bound-variable kind/index/depth through the existing `bound`
method. Actual Context bound-kind and Type lists feed pinned Binder::bind_with_vars,
FnSigTys, FnSigKind::new and FnHeader. Source FnPtr syntax has no splatted input;
None records that fact in the pinned constructor. No local arity cap is added.
ABI/safety/variadic qualifiers are encoded facts, not runtime refusals.

The source_terms-local `check_erased_function_pending` control now requires
FunctionMetadata for missing metadata, and actual Unsafe+C/nonvariadic FnPtr for
the unchanged unsafe extern alias whenever its header is retained. Removing the
actual source_header from a cloned control term must still yield FunctionMetadata.
That assertion is unexecuted pending the parent-owned build; imported source is
unchanged. The worker retains None for direct-walk omitted reference lifetimes
and unrepresented binders, while genuine symbolic-header binder capture can
retain Some. Encoder trust remains bounded by that actual producer contract.

Separate all-eight source pass: (1) only the ordinary `Encoder::ty` changed; no
assertion evaluator. (2) no generated-only scalar/break route or macro identity.
(3) no method/numeric semantic selection; qualifiers are actual source fields.
(4) the same Function term encodes for admitted handwritten/generated source;
missing evidence keeps a typed hold rather than an origin exclusion. (5) no
pattern dispatch or cold-body traversal; only demanded signature structure is
visited. (6) existing immutable query retention remains; the arm recursively
encodes nested types once during the demanded preparation, with no parser or
installed-operation planning. (7) no namespace scan/cache invalidation added;
existing SourceBindings supplies definitions. (8) only local binder-vector/list
construction, no BodyIndex clone or file-node search. No superseded runtime
function erasure route is removed: it serves its documented ordinary runtime
contract and now remains distinguishable from source evidence. Actual root
activation, full bucket publication and unchanged repr128 execution remain held.

Only source read/patch/hash terminals were run for this slice (hash command exit
0); no Cargo, test, Git, runtime or compiler-diagnostic acceptance is claimed.

V528 pre-freeze control correction supersedes the permissive positive branch
above: source_terms SHA-256
`3dfa449283cfeecec6241fdd26ebae2855c21078867eead083711611e96a5569`
requires the unchanged actual unsafe extern alias to retain Some source_header
and successfully encode Unsafe+C/nonvariadic FnPtr. None/FunctionMetadata cannot
pass the positive check. The explicit metadata-erased clone must still hold.
Only that cfg control changed; production encoder is unchanged. No Cargo ran.

## Post-V528 concrete relation fulfillment component (before implementation)

Parent released root_relations.rs and source_runtime_demands.rs only. This slice
will consume genuine RootRelationParts and retain its exact original
RootVariables identity, owner, snapshot, application, caller ParamEnv and every
relation obligation. Its actual registration/evaluation/drain body is module-
private and has no caller until the real packet-bound source/query entry exists;
no public always-pending solve API, ready bool, arbitrary callback, raw Goal or
replacement environment is added. The missing prerequisite is the ordinary
actual reached-packet/currentness consumer plus the exact demanded QueryFact
closure described in the normal-root record. This slice cannot activate it.

Existing FulfillmentContext owns all fast paths, progress/stalling, recursion and
overflow. The consumer registers original obligations once, evaluates once using
that upstream loop, then retains every TrueError and drained Ambiguity with its
stalled state. Fatal overflow retains its real context/pending state and original
work. Only an error-free, empty genuine completion constructs the private
SettledRootApplication; original outstanding_relation_goals is never cleared.
The numeric source-demand helper gains a settled entry joining its existing
TypeIR incidence walker; the zero-emitted-goal entry remains separately guarded.

No semantic solver, metadata producer, engine call path, fallback or adjustment
algorithm changes. Work is the actual obligation batch plus retained metadata;
only handles/causes are copied for original provenance during registration.
Validation and source admission remain held; all-eight review and source hashes
will be recorded before handing these files back for the parent's Cargo owner.

### Private fulfillment component handoff

Source fingerprints after `rustfmt +1.99.0 --edition 2024` (terminal 0):

- root_relations.rs: `5e7ca4995e8742ff8a00a680cd2348c915e1d877987549124bb776c4d532cfa3`.
- source_runtime_demands.rs: `b721073cae6416b0c5ec3722f6710f4e8a06892317b0b019d21cc0996e5490fd`.

`RootRelationParts::into_fulfillment_work(owner, variables)` consumes actual
parts, requiring both exact original owner and exact original RootVariables
pointer. SourceRootApplication now retains that original map; no reconstructed
map on the same owner can substitute. A preparation mismatch returns the entire
original parts with the typed pending reason. Read-only work accessors preserve
application/original obligations and any retained terminal context.

`RootRelationFulfillmentWork::fulfill` is a plain module-private method with no
call sites. It uses only the original stored owner, environment and obligations;
it accepts no replacement roots, callback, environment or completion flag. A
future actual runner must live in this module and first join the real reached
packet and demanded query closure. This source component is intentionally
inactive; no ready/receipt escape was added. Existing broader ungated solver APIs
remain named debt in the normal-root record, not newly authorized by this work.

The actual body constructs FulfillmentContext, registers each original
obligation, invokes its pinned evaluation loop and drains residual errors.
Unresolved outcomes retain original work plus distinct actual TrueError and
Ambiguity records (including stall/MaybeInfo). Fatal overflow retains its exact
FulfillmentOverflow and the live terminal fulfillment context with pending/stall
state, rather than draining it as ordinary ambiguity. The private success branch
alone constructs SettledRootApplication after error-free empty completion; it
retains original obligations and does not change outstanding_relation_goals.
The empty original batch passes through actual registration/evaluation/drain;
no synthetic goal is fabricated and no unrelated trait claim follows.

`SourceRuntimeDemands::after_fulfillment` requires the genuine settled value,
original owner/map and existing retained clause plan. It and guarded zero-emitted-
goal `after_relations` join the same private `select` TypeIR occurrence walker.
No source/numeric semantics were duplicated. Source/epoch/signature guards,
external constraints, later numeric source roots, runtime coercion adjustments
and once-consumed VM commit remain outside this relation-only proof.

Separate all-eight source pass: (1,2) existing ordinary source relation/TypeIR
routes only, no assertion or generated-output evaluator; (3) pinned fulfillment
and one unchanged numeric-incidence walker, no new selection/unification; (4)
no generated-origin rule; (5) no pattern or cold-body traversal; (6) retained
immutable goals and one upstream progress loop, no parsing/planning/retry loop;
(7) no namespace scan or global invalidation; (8) cloned goal handles/causes for
original provenance only, no BodyIndex copies or file-node searches. Additional
work is linear original obligation registration/storage and O(1) pointer checks,
plus actual existing upstream solving cost. No measured performance claim.

Source read/hash/review terminals were 0. No Cargo or tests ran in this slice;
compilation, runtime/full-fixture and actual root-admission acceptance are held.
