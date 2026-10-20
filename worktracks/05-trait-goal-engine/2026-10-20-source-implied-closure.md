# LOCAL-049 / WT05-02 / M020 authentic implied-clause closure

Before-code architecture record during V516 source freeze. HEAD at inspection:
f4a74d3bf0f8ae1b0dc276c5f87cbdd7e708fda4 plus frozen candidate. Read
2026-10-20-query-domain-fact-factory.md, current Q86/Q88, pinned elaborate.rs,
outlives.rs and collect/item_bounds.rs. This task edits this document only until
parent explicitly returns source ownership. No source/tool/fixture edits,
tests/builds/compiler/interpreter invocation or subagents. Parent exclusively
owns prepared.rs table split/admission and overall root integration.

Invariant/witness: unchanged enum-discriminant/repr128.rs reaches the genuine
DiscriminantKind normalization/PartialEq<U> inference goal. Relevant candidate
impl_super_outlives must derive its answer from the authenticated symbolic impl
trait head using pinned rustc_type_ir::elaborate, then retain only TypeOutlives /
RegionOutlives exactly as collect/item_bounds.rs:530–542. It is neither
inferred_outlives_of(Impl)==[] nor Q86's complete null clauses_of branch. No dummy
WF/static goal, fixture conditional, primitive/operator shortcut or side solver.
Retain host panic=abort: callback data prepares before algorithm entry, with no
missing-callback catch/replay or host-unwind protocol.

Exact upstream callback contract (pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18):
- elaborate.rs:86–151 uses Filter::All, ElaborateSized::No and pinned visited /
  anonymize_bound_vars handling. Each trait clause first asks authentic
  is_trait_lang_item(trait_id,Sized); when true it stops that elaboration edge.
  This is not Q85 is_sizedness_trait (which also includes MetaSized).
- Positive Trait at150–184 asks explicit_implied_clauses_of(actual trait ID),
  preserves each source span/index, and uses pinned instantiate_supertrait on
  the actual incoming trait arguments. Negative trait rows have no expansion.
  explicit_super_clauses_of is required only for filter_only_self/supertraits;
  this impl_super_outlives entry does not select that mode.
- HostEffect at187–200 asks explicit_implied_const_bounds(actual trait ID),
  preserves actual constness and performs pinned supertrait substitution.
  Current Q88 QualifiedBound holds do not supply this separate family. Any
  genuine unsupported HostEffect source path remains Pending before entry.
- TypeOutlives at201–236 skips a bound RHS region, then calls existing
  push_outlives_components and elaborate_component_to_clause. outlives.rs:59–240
  only calls opt_alias_variances when visiting an alias with escaping bound vars
  through compute_alias_components_recursive. Ordinary nonescaping aliases
  become alias components without that callback. Nonalias type components need
  no nominal variance/field/signature/body query for this algorithm.
- opt_alias_variances must retain its real alias-kind domain: explicit ordinary
  associated projection uses actual Q83 RPITIT=false to return None; opaque/
  synthetic RPITIT variance domains require genuine producers, never []/false
  guesses. Use Q87 actual alias ID/layout/parent joins and Q79 values as demanded.
- declared_bounds_from_definition at outlives.rs:267–286 asks item_self_bounds,
  but is NOT invoked by this push_outlives_components path. It is not an
  unconditional prerequisite of this one computation. Other root branches may
  separately require it. RegionOutlives/Projection/WellFormed/ConstEvaluatable /
  ConstArgHasType/UnstableFeature have no further elaboration in this match.

Common actual source inputs and readiness boundary:
Q86 key is ImplHeaderSyntaxKey, preserving inventory/raw syntax/canonical owner.
Its super_outlives_demand supplies real impl/trait IDs, not a closure proof.
Its encode_head retains PreparedImplSourceTypes and constructs actual checked
TraitRef args with DeclarationParameters/SourceGenericLayouts. Prepare Q88 keys
{inventory,definition,filter:Implied} for the actual demanded trait and every
positive dependency row. Retain actual Arc<QueryValue> roots, pending source
rows, Q76 classification for every trait whose Sized query can be reached,
owning configuration, generic/projection bindings and relevant source guards.
A visited DATA-key set may avoid duplicate pure preparation, retaining all graph
edges. It must not decide candidate applicability, cycles, normalization or
coinduction. Compiler query-forcing/cycle semantics are not proven just because
a graph node repeats; unsupported required query semantics stay pending.

For a Q88-ready simple source trait, its encoded ordered immediate rows genuinely
represent the exact Super/Implied source-selected answer in its supported domain.
Readiness of that single plan does not supply recursive compiler forcing or all
algorithm callbacks. Example: pin-aligned facade PartialEq has actual immediate
Self:PointeeSized; real PointeeSized has no colon/where/default clauses, so its
complete immediate source answer is genuinely empty. Both require authenticated
parameter/lang/config/source evidence, not their names. The retained source
closure can therefore supply real immediate rows for this graph; it cannot
label Q88 dependency descriptors themselves as certificates. Full Eq/Ord/
PartialOrd const bounds, associated constraints, HRTB and alias parameter defaults
remain actual pending producer domains. No source language registry absence is
inferred from sparse maps: per-ID Sized equality comes from Q76's authenticated
compiler role or proven ordinary/other role, with missing classification pending.

Concrete proposed module/API after authorization (names for parent agreement):
new source_impl_outlives.rs; SourceImplOutlivesKey=ImplHeaderSyntaxKey;
PreparedSourceImplOutlives::prepare(db,&key)->QueryPoll<Self>, key/pending,
actual impl/trait head access and immutable immediate query-input iteration.
It retains Q86 + concrete Q88/Q76/config/alias query roots and real dependency
keys, and reports typed missing key/domain/source when any input is unavailable.
No public constructor accepts arbitrary clauses/defaults/boolean completeness.
A read-only Q88 bindings() accessor is needed to combine authentic bindings with
Q86 source_types().bindings() via SourceGenericLayouts::from_bindings into ONE
source/Cx namespace. Existing separate generic_layouts() plans cannot safely be
encoded into unrelated temporary contexts and then combined.

Materialization proposal: encode_inputs(cx,configuration) supplies the actual
checked symbolic impl TraitRef, exact per-ID EarlyBinder<Vec<(Clause,Span)>>
immediate Implied rows, authentic per-ID Sized-role booleans and any genuine
required const/alias query answers, retaining source proof handles. These are
independent inputs for the parent's PreparedProvider query tables, not complete
DefinitionFacts or arbitrary ReadyProvider/root permission. Parent must supply
an opaque admitted elaboration boundary tied to this EXACT head/Cx/source-world
and fully validated callback domain set. Only then call the retained pinned
elaborate iterator and original outlives-kind filter, returning an actual
EarlyBinder<Clauses> for this authenticated impl. Missing generic/layout/source
metadata stays typed pending; no repeated source work occurs in callbacks.
Source preparation itself cannot issue a permission token for a different head,
root/ParamEnv/inference owner or complete solver query set.

Type/const branch closure must be proven from actual data before entry. This can
conservatively retain all actual aliases appearing in the demanded head/implied
source data with their Q83/Q87/Q79 domains; it must not manually substitute a
second elaboration algorithm to discover keys. Current unsupported binder/
const/source forms remain explicit holds. The initial implemented component may
support a narrower genuinely complete elaboration input domain while reporting
other domains Pending; it cannot turn missing inputs into successful absence.

Authority/removal/cost: existing query framework prepares each demanded immutable
metadata key once, with source/header/edition/expansion identity and dependency
closure guards. No whole-world discovery, method leaf/bodies or signatures are
needed just to compute this query. Graph bookkeeping is O(V+E) in demanded actual
source keys plus existing selected plan/TypeIR costs; pinned elaboration owns
semantic visited/substitution behavior and its own cost. No speed claim or cap.
No legacy applicability/solver route is removed or promoted by this component;
parent's real runtime root/table admission and later legacy retirement remain
separate acceptance work.

Eight recurring modes, not MA audit IDs: (1) no assertion evaluator; (2) no
macro-output scalar executor; (3) no numeric/member or primitive candidate rule;
(4) same original/generated source-key/hygiene route; (5) no patterns/cold body or
static checking; (6) once-per-query Arc plans, no callback/installed source
planning; (7) relevant dependency graph only, no hot global census; (8) no
BodyIndex copying or whole-file node search. Required actual observations after
source authorization/validation: genuine source impl head and per-ID callback
rows; pinned iterator actually runs; actual outlives values (including zero only
after true computation), typed missing input and source/epoch withdrawal,
original/user/wrapper/all-edition route, cold bodies. Component evidence cannot
close M020, repr128/runtime, full compiler query families or interpreter/JIT gates.

Frozen inspected SHA256 inputs:
- Q88 source_trait_clauses.rs: 6a77da6aa2a0e856ae42e3ad4717b991908d70db122a2a6a2053c99a01f8d2c7
- Q86 source_impl_semantics.rs: c0c260c79ccb0ad0ae4d80b3f5aaaed0bb4eb454d83f0f04a6ee24bd59bac9e5
- pinned elaborate.rs: c7f6f7cedb01a8aa21942e6121920ac0153defee800d8024176e4fe0a4a0a41d
- pinned outlives.rs: d6dfd70b229b97e0fb835874448cba78ee306002040895116a6f72d1ca0bf563
- query-domain-fact-factory.md: 272bb12d6f264ab0f7d6d6ce852f91e5723cb32764a4e18d61c7a3edc19b72d6
No source implementation or executable acceptance has occurred in this task.
