# Actual projection/numeric-root callback closure: bounded next slice

Read-only ahead-of-entry analysis while V529/V530 validation owns frozen source.
No production edits, Cargo, source-side solver invocation or root admission.
This adds concrete callback distinctions to the earlier normal-root record.

Inspected local SHA-256 values (before the lead's next private entry edits):

| File under crates/lrr/src/traits/upstream | SHA-256 |
| --- | --- |
| source_factory.rs | 4390b8873c595b404da8b320cfc7c6ae83d9384d963afc995fe7783e3ac5c2c4 |
| source_fact_bundle.rs | 194f9654a34d411c33051d73ebd3e4eb1f97a5d72aac744eaba7c7d1a3e5c320 |
| prepared.rs | 8adfe5c1db64be1ad911a47a42d3545bf6d6223b69d71d537fca8be55a3a18f9 |
| candidates.rs | 982bbad588cd129ab6a5acec73d88748ddcad4f8f8853f714e4e1b60acc0fa58 |
| discriminant.rs | 238fc9bd2964834b2ea466f8e58f282045ff54b1f5ca37b8193bfa3251e09004 |
| root_input.rs | 369072f7ad0ce552c15ed70e5904ac047a3b9f270f8b9fe9b9020d9b486b118b |
| infer/delegate.rs | 0cf91947b833fbb373ab70ee541aefaf6264678930d2666403230d5617c51a76 |

## Actual source demand and compiler branches

The unchanged repr128 call's Q80 source requirements include an instantiated
`<T as DiscriminantKind>::Discriminant: PartialEq<U>` connected to an original
unresolved numeric argument. The root still uses its actual callee Args, original
RootVariables and source-proven caller ParamEnv. SourceRuntimeDemands also selects
any genuine numeric-connected implicit Sized clause; do not force unrelated
ground bounds. The normal owner must first fulfill actual relation goals before
using its separate settled application. A pending relation is not completion.

Pinned ae0fe511b15ab760d7e963b13389d1bc2ab42b18:

- assembly/mod.rs::assemble_and_evaluate_candidates (484+) structurally normalizes
  Self before ordinary trait candidate lookup. Do not locally replace the goal.
- normalizes_to.rs::compute_normalizes_to_goal (45+) first probes the projection
  parent's trait goal, then assembles actual normalization candidates.
- assembly/mod.rs549+,587+ calls for_each_relevant_impl even after a builtin
  candidate succeeds. Hence the actual DiscriminantKind source bucket must be
  complete/proven empty too; the current observation request's PartialEq bucket
  alone cannot run this normal root. A selected Sized root needs its real domain
  when the configured fast path does not discharge it.
- normalizes_to.rs::consider_builtin_discriminant_kind_candidate (979+) uses the
  pinned discriminant-Type client. For an actual enum it reads actual AdtKind
  and prepared enum discriminant repr. No fields/body/value execution is needed.
- trait_goals.rs::consider_impl_candidate (75+) reads impl_trait_ref, pinned
  DeepRejectCtxt, default flag, polarity, fresh actual generic args, full
  clauses_of and impl_super_outlives for every applicable queried candidate.
  No local candidate voting/matching is permitted.

## Missing domains versus already available inputs

| Domain | Bounded requirement and present source |
| --- | --- |
| Outer/projection-parent candidate membership | SourceFactBundle request must include actual outer goal trait and all demanded projection parents; materialize only marks requested buckets Known. Complete known-empty DiscriminantKind is meaningful input. No new solver is needed. |
| Primitive/relevant impl clauses | Q86 PreparedSourceImplSemantics::complete_clauses_of already proves the complete zero-parameter/null branch; factory publishes it as full/own Known([]). Actual relevant generic/conditional/blanket impls instead require their real full compiler-clause producer; Q80 explicit rows cannot stand in for it. |
| Relevant impl super outlives | Q89 PreparedSourceImplOutlives and PreparedProvider::prepare_impl_super_outlives use genuine pinned elaboration. Check each actually visited row's QueryFact, not a global encountered-holds count. |
| Trait metadata | Existing SourceTraitSemantics supplies actual auto/alias/coinductive/marker/sizedness and per-definition language classification. Source projection parent/argument layouts and actual generic ordering must be Known before encoding/root entry. |
| Enum repr | SourceAdtFacts(fields=false) plus PreparedEnumDiscriminant supplies actual enum kind/explicit repr; both original input enums were observed among prepared reprs in V528. That is prior source-component evidence, not a V529 or root pass. No new repr producer is needed for that branch. |
| Variance/field/general associated domains | Leave unavailable when the actual bounded path cannot call them. Pinned relate.rs385+ skips ADT argument/variance work for empty ADT args; generalized source inputs with nonempty args need the real variance producer. Field, destructor, specialization, associated leaf/value and item-bound domains cannot be filled with defaults if an actual path needs them. |

`language_items` does not need a whole-world absence registry for this bounded
normal branch. PreparedProvider::is_trait_lang_item already compares its exact
per-definition classification (or separately prepared trait_tests). Thus negative
Unsize/MetaSized/etc comparisons are source-backed without forward role None.
The actual PartialEq compiler role `eq` is outside SolverTraitLangItem and has a
genuine None classification in that enum; DiscriminantKind is positive. A forward
require_*_lang_item callback still needs its actual positive demanded role. A
future branch really calling a missing forward role must prepare that query;
unobserved role absence must never be invented.

For the actual source-proven empty caller environment and a source-proven empty
DiscriminantKind impl bucket, the pinned enum builtin does not visit associated
value/specialization leaves or GAT own clauses. normalizes_to.rs::match_assumption
and consider_impl_candidate use those queries on different actual branches; do
not promote that observation to a general normalization certificate. Likewise
the trait's declared Discriminant item has many bounds, but the normal builtin
branch does not imply a request to statically prove every one of them.

## Conservative preparation, without solving

Keep the complete outer/projection-parent catalog memberships and all typed heads
needed to build the real CandidateIndex. Cold header preparation can be broader
than the callback metadata scope. For a known constructor, use only the actual
pinned for_each_relevant_source_impl traversal with a recording visitor to obtain
the metadata owners it would visit; preserve its blanket list. For an integer
Self variable, use its existing full pinned integer grouping, not a chosen width.
An unnormalized alias's blanket-only visit is not a complete post-normalization
closure: never use it to exclude future constructor groups.

The smallest useful enum branch may use the already prepared pure
pinned_discriminant_ty/enum repr query to bound possible *metadata domains*, when
the actual caller/projection-parent bucket facts justify that bound. It must keep
the original goal unchanged and execute actual normalization only in the solver.
If there are other possible source implementations, nonempty caller projection
assumptions, or an unresolved result domain, conservatively prepare every possible
callback owner or retain a typed unmet source requirement. Do not pick an impl,
assert that a predicate holds, equate original vars or add an LRR retry scheduler
in this preparation service. Source dependency closure is a finite agenda of
actual declaration/query-family keys, not a graph of solved trait obligations.

The next metadata slice is therefore: derive actual demanded trait/projection
parent keys from retained source goals; request their complete existing catalogs;
join actual enum/generic/lang metadata; record the conservative candidate owner
set through pinned grouping; inspect required QueryFact slots for those owners.
For already proven null primitive rows, no new general clauses collector is
needed. If an actually visited generic row is unavailable, keep its precise
clauses_of requirement and implement that real producer next. Do not require all
unrelated generic impl clauses merely because materialize retained their headers.
`MaterializedSourceFacts.unprepared` is encoding diagnostics, and `ready()` is a
coarse whole-row census; neither is the required per-root closure check.

## Private normal entry and canonical descendants

Lead owns the actual entry join in root_input/prepared. Proposed minimal contract:
an opaque SourceRootEntry is constructed only by the actual reached-call consumer
after its attempt/currentness/result contract and retained source plans/closure
are checked. SourceRootCallInput's crate-visible fields are transport, not that
proof. The concrete operation consumes the original RootRelationParts and exact
RootVariables; expose no `(Goal,ParamEnv)` entry, caller ready bool, owner callback
or mutable inspector. Keep the actual fulfillment body private in its owning
module. An externally returned scope guard/active owner would reopen arbitrary
normal roots while the guard is live and is not the desired entry API.

PreparedProvider's initially empty scoped state carries a private session identity
bound to exact Cx/provider/world/configuration/entry. Original inference owners
start with no lineage; only the concrete operation binds the checked original.
SolverContext::build_with_canonical inherits that identity for the actual fresh
canonical contexts created by pinned evaluation. Child owners must not pass the
original RootVariables pointer test; they pass session+Cx/provider lineage while
original application/response transfer still checks the actual original pointer.
Closing scope invalidates root/children/cache access, including after unwind;
later sessions use a fresh identity rather than a persistent ready bit.

Required adapter seams remain: local SolverContext::cx and compute_goal_fast_path,
InferCtxtLike::disable_trait_solver_fast_paths (needed for stalled early return),
fulfillment entry before even an empty batch, and Context::with_global_cache via
a required provider hook. Keep raw proof callback closed during normal entry.
Do not change upstream search, canonicalization, cache semantics, progress or
limits. Because build_with_canonical has no parent argument, the private no-
callback/nonescape concrete operation is essential; ambient lineage alone does
not bind arbitrary goal replacement. See normal-root-authority.md for the direct
vendor-raw-provider limitation and separate private-delegate prerequisite if a
broader reentrant API were ever needed.

All preparation uses actual retained source scopes, editions, expansion/input
identities, catalog and configuration. New query reads flow through normal memo
dependencies and NeedInput before entry. The solver callback may only read frozen
values. There is no additional evaluator, macro/name special case, cold-body
check, installed planning/global scan, BodyIndex copy or static checker. The code
slice is a source/query adapter plus private ownership guards, with unchanged
pinned semantic algorithms. No source-root, callback trace or runtime acceptance
was executed in this analysis; the stated bounded domain must be confirmed by
actual normal-root controls and owned response/external-constraint/VM evidence.
