# M020 authentic transient predicate emission

Before-code record, LOCAL-049 / WT05-02. During V521 source freeze this is a
read-only design; no producer source, fixture, tool, registration or test edits.
After explicit ownership return the assigned slice is only new
`traits/upstream/predicate_emission.rs` and this record. Lead owns all consumer
integration and registration; `SourceBindings` remains unchanged.
Ownership returned after V521 closing guards at checkpoint `7ea94b8f6b`.
Implementation below is authorized for those two paths only; no tests/builds.

Invariant: actual positive `PointeeSized` bounds affect source default-bound
policy but do not become compiler IR clauses. Exact pin
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`,
`rustc_hir_analysis/src/hir_ty_lowering/mod.rs:983–1002` selects a transient
scratch vector by authenticated lang identity, after which the same lowering
continues and the vector is discarded. No validation is bypassed by returning
early. This is source-to-IR policy, not a trait proof or independent solver.

Unchanged primary witness `enum-discriminant/repr128.rs` requires actual
PartialEq whose source header declares Self and Rhs PointeeSized bounds. The
already imported run-pass `dyn-compatibility/pointeesized.rs` explicitly
witnesses removal before the solver and has a real TypeId equality runtime
contract. SHA256 `d17c1542edd487ecb0a9e84d35045f2c4ee838c298677454701046bc67e9d617`.
It exercises dyn lowering too; this clause component cannot claim that whole
fixture. Existing imported `traits/core-marker-name-shadowing-issue-2284.rs`
declares ordinary user `Send` (line12), used by `f<T:Send>` (line16), and must
retain an emitted ordinary bound with Q76 Known(None). SHA256
`947a164cfa2d31cac735e0bae09d5dd4fb5830e0b0917bbaf906463b61936f62`.
Its empty main is only a source-role control opportunity, not runtime evidence
for this producer. Neither fixture was changed or executed here.

## Exact consumer joins

| Plan / call site | Retained addition and build order |
| --- | --- |
| Q78 `frontend/declaration_source_predicates.rs::PreparedDeclarationSourcePredicates` | Preserve written/default recipes, ActualBoundSite and parameter/parent provenance. No emission filtering or Q76 lookup here is necessary. |
| Q80 `source_declaration_clauses.rs::PreparedSourceDeclarationClauses::prepare/encode` | After `require_recipe` and actual `SourceBindings::prepare`, prepare emission sidecar over `source().recipes()`. Retain it beside `bindings`; pass it to `encode_recipe` at current line185. Q81 hierarchy inherits encoded results without a second policy. |
| Ordinary impl `source_predicates.rs::PreparedImplSourcePredicates::prepare/encode` | After actual bindings, prepare sidecar over real ImplPredicates requirements, retaining it beside `requirements`/`bindings`; current line208 calls the same `encode_recipe`. No synthetic declaration. |
| Q88 `source_trait_clauses.rs::PreparedSourceTraitClauses::prepare/encode` | After source defaults/order and binding, retain sidecar over recipes. Current line445 calls common encoder. Build `positive_dependencies` only from positive rows whose checked emission decision is Emit. Retain raw transient recipes and source Pending rows separately. |
| Q89 `source_impl_outlives.rs::prepare` | Current line256 follows Q88 `positive_dependencies`; it must see emitted semantic dependencies, not discarded PointeeSized rows. Full encoded ClauseKind inspection and typed requirements remain unchanged. |
| Q82 `frontend/source_item_bounds.rs` | Keep written/default recipes and parent candidates separate. No production item-bounds encoder currently exists; that gap stays held. Existing Q84 cfg observer (`source_default_bounds.rs:926`) prepares a sidecar before its common `encode_recipe` call. Future production item-bounds encoding uses the identical sidecar/encoder boundary. |

The only production `encode_recipe` callers are the three above; the fourth
call is the Q84 cfg observer. `source_terms::encode_predicate` stays an actual
predicate constructor; transient *output publication* belongs to the optional
clause boundary, not inside Type/alias binding or numeric inference.

## Concrete prepared interface and availability

`PreparedClauseEmission::prepare(db, catalog: Arc<InputValue>,
recipes: impl IntoIterator<Item=&SourcePredicateRecipe>) -> QueryPoll<Self>`
validates the actual catalog, gathers distinct positive actual trait IDs and
fetches Q76 `SourceLangIdentity(DefinitionHeaderKey{inventory,definition})` for
each. Retain catalog and every Q76 Arc, plus checked per-ID
Emit / TransientPositivePointeeSized / Pending decisions. A successful actual
classification of another role or None means Emit; classification failure is
typed pending. No missing map entry means Emit. Ordinary shadow names thus do
not select compiler policy. Classify resolved defaults as genuine positive
trait references too; unresolved legacy defaults and written Pending stay held.

`require/decision(actual_trait_id)` returns a checked decision or typed missing
identity/capture reason. `apply_encoded(actual_trait_id,(Clause,Span))` checks
the encoded positive trait identity/world and returns Some for Emit or None
for transient; it performs no queries. Lead's `encode_recipe` first performs
the existing full real predicate/argument encoding, propagating unsupported
Types/aliases/regions/pending, and only then discards the transient output.
It must not return None before encoding or erase pending written arguments.
Relaxed-source handling and unsupported negative/qualified bounds retain their
existing explicit availability contracts; they cannot become positive clauses.

Stable encoding includes inventory/catalog fingerprint, actual per-ID Q76
fingerprints and precise decision/pending tags. Existing enclosing memo guards
retain all nested Q76/receipt inputs. No complete-world lang registry or global
absence proof is needed. Important acyclic order:
owner source plan -> SourceBindings complete -> emission sidecar ->
Q76 -> Q75 -> SourceBindings. Since SourceBindings never reads this sidecar or
Q76, there is no recursive language classification installed in generic/type
construction. Q84 may return before language capture under no-implicit-bounds;
this separate positive-clause policy still needs authentic Q76 and must not
interpret that early return as an empty or ordinary-language classification.

Cost: one memo lookup per distinct positive trait ID on cold plan preparation,
retained Arc roots, and logarithmic sidecar lookup per encoded clause. At solver
entry all clauses/dependencies are frozen; no parsing, source walking, namespace
census or re-preparation occurs in callbacks. No performance measured.

Eight recurring modes source scope: no assertion evaluator; no scalar macro
executor; no contextual methods/numeric rules; original/generated Q76/Q75 joins
remain common; no patterns/cold executable arms; one retained policy per memo,
no hot planning; relevant per-ID dependencies only; no BodyIndex clone or file
search. Historical MA001–MA008 audit debts are not closed by this component.
The real emitted-clause/default/provenance/origin/currentness and root matrix
must be validated against frozen source after implementation. Missing factory
domains, actual pinned elaboration/normalization and repr128 runtime acceptance
remain separate gates; abort policy is unchanged.

## Implemented component handoff

New `predicate_emission.rs` implements the interface above. Catalog capture
checks actual `catalog_world` publication and the exact current database input
Arc. Positive recipe IDs are deduplicated in a BTreeSet before Q76 fetches;
each cacheable matching Q76 root is retained in the immutable per-ID map.
Known(None) is an actual ordinary-role classification, never missing data.
`pending()` includes original written Pending/legacy unresolved Sized rows and
Q76 failures. `decision(id)` and `require(id)` are per-ID observations; missing
IDs return Unprepared. They do not certify neighboring rows or whole queries.
Enclosing consumers must check complete-plan availability independently.

`apply_encoded(id,(Clause,Span))` requires a real positive Trait clause,
matching source ID and catalog world before applying the retained decision.
Negative polarity, non-trait clauses and foreign identities return distinct
typed holds. Emit returns the exact original tuple; transient returns None.
The method accepts only completed output and does not perform or replace the
common encoder. Consumer integration must encode every argument first and
propagate its errors. Stable encoding includes actual catalog/Q76 fingerprints,
all map decisions and exact pending source offsets/parameter keys.

Separate source review against module SHA256
`a767253ac0748e7d17de75296d187316d2f14c4ab88a529974b22a3c23a54f88`:
all new fetches occur only in `prepare`; `pending`/`require`/`apply_encoded`
perform no DB reads. The source callback path remains the shared encoder;
there are no parser, normalizer, solver, body, pattern, operator or macro
branches. Catalog/Q76 Arc handles are retained without immutable BodyIndex
copies or file searches. Checked per-ID map lookups replace no source namespace
mechanism and do not add a global scan. No unsafe or resource limits were added.
These establish the eight source observations above only; historical audit
findings and all runtime/matrix gates stay open. Scoped Rust1.99.0 rustfmt and
whitespace checks completed. No build/tests/compiler/fixture copies or commits
were run by this worker. Lead owns common caller and dependency integration;
the validation worker owns actual unchanged-source observations/execution.
Independent validation/review worker completed a read-only pass of that exact
module fingerprint and reported no semantic blocker. Consumer integration
enum/helper mismatches were reported to their owning lead separately; they
are not evidence of compilation or runtime acceptance for this component.

## V523 read-only reached-constraint relevance

Section53.0/53.3 requires argument/result inference goals and reached dictionary
goals; an ordinary function's unused bound stays unforced. Thus Q80 readiness
does not authorize registering every callee clause as a root. For unchanged
`repr128::discr<T,U>`, actual `T` becomes Signed/Unsigned from its reached first
argument. Ground `T:Sized` is not an inference demand. `U` is related to the
actual unsuffixed literal's live Integer variable, and the real projected
PartialEq<U> clause can determine it before fallback.

Concrete common entry is `engine/run.rs:3658`, before
`GuestStore::bind_literal_fallback`, using existing
`RunMachine::capture_reached_call_inputs`. Its immutable packet supplies actual
callee/signature identity, source parameter mappings, Type/Integer/Float
families, equality/coercion relations, caller source context and guardable
argument snapshots. `root_variables::RootVariables` allocates the real pinned
owner variables. Apply actual relations in that owner first; only then inspect
instantiated/deeply-resolved Q80/Q81 clause terms for relevance. Inspecting raw
ParamKeys or all freshly allocated callee variables before those relations
would incorrectly select T:Sized or unused generic parameters.

Seed observations are unresolved variables actually occurring in reached
argument/result relations whose Types/descriptors are needed for this call,
not every variable allocated by `parameters`. An ordinary TypeIR visitor can
record variable incidence in instantiated source clauses. Follow that
data-dependency incidence through intermediate open variables so indirect
constraints are retained; this is demand metadata, never matching/proving a
trait or a semantic cycle scheduler. Fully ground declaration clauses have no
such edge and remain dormant dictionaries. Preserve source order/spans and
parent-before-own identity during this selection. Unsupported subject/binder
transport stays pending; do not equate a missing term with no incidence.
An open U:Sized row can belong to this inference component, but do not turn it
into an unconditional check of already-ground T's ADT sizedness. This is source
planning only, not an implemented or accepted relevance classifier.

Pinned `rustc_hir_typeck/src/fn_ctxt/_impl.rs:114–138`
`deeply_resolve_ignoring_regions_with_obligations` tries selection when an
observed Type still has inference variables. `fallback.rs:27–43` selects before
numeric fallback and again afterward. Rustc's compiler registers more checking
obligations than LRR permits; reuse its inference/selection/fallback sequencing,
not its whole-body/full-callee checking scope. The current adapter's
`upstream::evaluate` exposes one `evaluate_root_goal`, not a complete shared
root-obligation engine. Multi-root stall/progress/retry must reuse pinned
`rustc_trait_selection/src/solve/fulfill.rs::FulfillmentCtxt`, especially
`try_evaluate_obligations` and GoalStalledOn/HasChanged handling, or another
actual upstream-provided equivalent. Do not write a substitute retry/fixpoint
loop, treat one ambiguous pass as fallback permission, or replay constraints
in a new owner. Any needed mechanical port is a separate real prerequisite.

Crucial asymmetry: this relevance selection applies to external ordinary-call
declaration demands only. Pinned `trait_goals.rs::consider_impl_candidate`
always adds complete `clauses_of(impl)` and `impl_super_outlives(impl)` as child
goals. Filtering those by open-variable occurrence would accept a false
conditional impl and is forbidden even when its conditions are ground.
Likewise callee requirements must never become the caller ParamEnv proving
themselves; only independently authenticated caller evidence may discharge
them. Actual selected member/dictionary needs remain roots even when ground.

Existing `call_contracts.rs:1811` consumes demanded associated equations via
`associated_equations::constrain`; that legacy syntactic equation loop is not
the new projection/trait inference engine. Replace its overlapping reached-call
constraint ownership only once the common upstream consumer/readback/commit
route handles it, retaining named holds in the meantime. No numeric comparison,
literal retyping, projection-specific program case, body scan or dummy WF root
belongs in this repair. Snapshot/currentness guards must survive NeedInput and
response commit; canonical response is applied/read in the same owner through
RootVariables/root_response before authentic runtime literal materialization.
All source/runtime/edition/origin/cleanup acceptance remains unexecuted here.
