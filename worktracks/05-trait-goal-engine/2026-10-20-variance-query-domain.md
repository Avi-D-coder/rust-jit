# M020 bounded variance query domain

Before source edits: baseline pushed `3d25d2b`, worker `/root/wf_value_helpers`,
lead `/root`. Own `traits/upstream/prepared.rs`, new
`traits/upstream/source_variance_queries.rs` and new CLI control file
`cli/preentry_loader_tests/upstream_variance_queries.rs`. Parent registers
modules. No Cargo/commit; validator owns frozen builds. No nominal variance
algorithm port, GAT/parser changes, fake complete facts or synthetic IDs.

Invariant: unconditional DefinitionFacts variance/alias-variance slices discard
pinned rustc query applicability. Actual upstream
`rustc_hir_analysis/src/variance/mod.rs::variances_of` first checks total actual
`generics_of(...).is_empty()` (count==0), then handles Fn/AssocFn/Enum/Struct/
Union/Ctor, actual trait RPITIT and actual OpaqueTy; all other nonzero queries
are compiler invariant failures. `rustc_middle/src/ty/util.rs::opt_alias_variances`
returns Some for opaque and actual RPITIT projection types, None for every other
alias kind. Ordinary explicit associated aliases are not RPITIT, even if they
inherit trait Self. No fallback empty/invariant vector is permitted.

Unchanged witnesses: generic-derived-type.rs Pair<T> and associated-types-basic.rs
Foo/trait associated T/impl alias T across owning editions and original/emit/
wrapper source metadata where available. Controls inspect actual declarations
and domain outcomes, never guest expected values or manufactured impls/functions.

Common route: real source DefinitionReceipt/DefinitionHeaderPlan plus actual
DeclarationParameters count -> immutable query domain -> computed upstream
variance result from its separate actual producer when applicable -> published
Known(values) or NotApplicable. Pending stays outside DefinitionFacts. Callback
reads only retained data and matches complete upstream alias-kind dispatch;
invalid variances_of remains an invariant failure. Actual opaque/RPITIT/constructor
source identity producers remain held; no invented IDs or source-kind guesses.

Costs/dependencies: one demanded source/header/generic preparation with existing
query guards; callback constant-time data dispatch, no parsing/source queries,
IO/evaluation or namespace census. No caps or memory accounting. Shared ordinary
source syntax/parser/name services retain generated identities/owning editions.
No alternate evaluator or new solver. Completion requires separate eight-mode
source review at final fingerprint, actual-source controls, frozen macOS builds
and full root/admission/runtime evidence; component controls do not close M020,
interpreter or JIT gates. Parent registers tests/modules and owns integration.

Frozen worker handoff (source review, not execution):

- `source_variance_queries.rs` SHA256
  `fe85bb137bb29bc0cf3f8df05a71732375d8b205927a392195059534f19c5969`.
- `prepared.rs` SHA256
  `90039056bb1ef45203933fe5bdb10bb4a66bc4b163d778f0dbbfe38c0a2ff8d2`.
- `cli/preentry_loader_tests/upstream_variance_queries.rs` SHA256
  `9a923e40751bcce57ae35a4195ce004edbfbe45a2b1d72b4a1ede85dd2c5ebde`.

Exact retained upstream provenance, all pin
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18` (the nominal-algorithm worker
owns source retention/manifest/tool registration):

- `compiler/rustc_hir_analysis/src/variance/mod.rs`, SHA256
  `5a9fe3fb38938d03cb7758dac1c338ac818b2807c20e5310e5fae0e00aa06abb`,
  `variances_of` lines 37–90: exact zero-count precedence, actual source roles,
  invalid-query invariant; unsupported synthetic identity producers stay held.
- `compiler/rustc_middle/src/ty/generics.rs`, SHA256
  `f5231c1921d2691ee02305a0c3987e420487c6b841be256c10eafe2752b935b0`,
  `Generics::is_empty`: count equals zero, including inherited/implicit Self.
- `compiler/rustc_middle/src/ty/util.rs`, SHA256
  `53b880e72a439d2f27ad5435bc6077ecd9a4342a67f4f6c495aa1e42dad336d7`,
  `opt_alias_variances` lines 954–978: complete AliasTermKind match transported
  into one helper shared by the provider and real associated-source control.
  Only spelling/lifetime/callback transport changes; callbacks read facts.

Separate source review against the eight recurring failure modes:

1. No assertion/expression compiler/evaluator: only source header/parameter
   metadata and pinned variance clients changed; source-aware CLI controls do
   not enter guest main or install a complete provider.
2. No generated scalar/break route: original/emit/wrapper declarations demand
   the same Q79 -> DefinitionHeaderPlan -> DeclarationParameters/actual nominal
   preparation. Actual runtime macro behavior is unexecuted.
3. No context method/numeric policy: role/count select the pinned query domain;
   Pair/Covariant expectations exist solely in the external host control.
4. No generated-source exclusion: real DefinitionReceipt and the shared source
   header retain original/generated IDs and owning editions. Missing metadata
   stays typed Pending; no name-hash, hidden opaque or RPITIT ID is invented.
5. No pattern machinery or branch/body validation: variance preparation reads
   demanded declaration/field Types; no FunctionSignature, BodyStructure,
   Thunk or PatternPlan demand is authorized by this slice. Controls assert it.
6. Q79 memo retains header/nominal roots once; SourceFacts callbacks read
   borrowed immutable results and the exact alias-kind branch only. No source
   query, parse or plan reconstruction is present in either callback.
7. No namespace census or new invalidation policy: existing memo input closure
   and PreparedInputGuard carry source dependencies. Publish additionally
   authenticates receipt, SealKey and total actual generic count. Controls
   check pointer sharing/currentness and revocation; execution remains pending.
8. Arc-backed headers and owned immutable result vectors are retained; no
   BodyIndex copy or whole-file node search is introduced. Nominal cold costs
   and full algorithm review belong to the separately recorded producer slice.

Removed routes: arbitrary `DefinitionFacts.variances` and `.alias_variances`
are gone. Published facts require a borrowed, privately constructed completed
query result; Known and NotApplicable are distinct, and Pending cannot bind.
Invalid nonzero query kinds panic like the compiler invariant; they never
become solver NoSolution. Nonzero Function (including foreign placement),
opaque/RPITIT/constructor identities and missing nominal leaves remain held.
The full actual factory/admission, root solver and runtime/JIT gates remain
held. Source-control selector:
`unchanged_source_variance_query_domains_all_editions_and_origins`, intended
24 worlds / 96 declaration checks, including exact actual Pair `[Covariant]`.
No Cargo or source tests run by this worker; `git diff --check` returned zero.
Read-only validator reviewed the three source paths and found no blocking
defect after foreign Function applicability was corrected; final Pair value
assertion supplements the original count-only draft.

Follow-on before edit: parent source review found count completeness is not yet
proved by written GenericParamList metadata. Actual argument-position `impl
Trait` introduces anonymous compiler generics; `fn foo(f: impl Display + Clone)`
in unchanged imported `impl-trait/universal_multiple_bounds.rs` and nested
`&Vec<impl Display>` in `universal_in_adt_in_parameters.rs` have no written
generic parameters. Returning Known(empty) would be incorrect. Extend shared
`frontend/declaration_generics.rs` to retain only actual function parameter-Type
APIT locations and unexpanded Type-macro frontiers, with its existing parser
snapshot/source receipts. `upstream/parameters.rs::DeclarationParameters::prepare`
must return an explicit identity-completeness Pending before exposing a total
upstream count. Do not change ordinary written layout/ordinals, create anonymous
IDs, prepare FunctionSignature/body, or scan executable constant declarations.
Cold work is one parameter-Type AST walk per demanded declaration memo; callbacks
are unchanged and use no source work. No alternate route is removed: this closes
a false-completeness claim in the common source bridge. Complete APIT identity
production remains held. Add external source controls for both unchanged
witnesses in original/emit/wrapper across four editions; parameter metadata,
pending count, memo, cold demands and revocation are component evidence only.
Also require every published Known variance vector length to equal its actual
authenticated total generic count. No Cargo; re-review all eight modes and
refresh fingerprints before the renewed handoff.

Renewed frozen handoff after the follow-on (prior fingerprints/evidence above
remain attached to the earlier draft):

- `frontend/declaration_generics.rs`
  `196e86238f84d72aa87eaffbc32431cf3425ab1a7029ddcf7d9e8ebbf6cf9fde`
- `frontend/declaration_generic_types.rs`
  `a2c7daa4196a8e942634445abb8002af6f2bd36a9b918bf982794a3aac3e747a`
- `traits/upstream/parameters.rs`
  `2e0d8bd89ebdd2f235c63a4672cdbe95a14ab2f2076810779ad4c47ee696355a`
- `traits/upstream/source_variance_queries.rs`
  `1ab035ebe99ee7108b398891915aa53714c353da7f79fd8f934ae46f9e23e644`
- `traits/upstream/prepared.rs` unchanged from prior handoff
  `90039056bb1ef45203933fe5bdb10bb4a66bc4b163d778f0dbbfe38c0a2ff8d2`
- `cli/preentry_loader_tests/upstream_variance_queries.rs`
  `7fb9bb49d0af6dcdafed42984875d1fac07088c3d709216297f07fec4cdcf06e`

Shared typed-generic preparation now consumes the same retained completeness
marker as DeclarationParameters: DeclarationGenericTypes returns its existing
Parameters pending, so SourceBindings retains a typed ParameterLayout demand
instead of failing later inside SourceGenericLayouts. The ordinary written
parameters/layout remain unchanged. DeclarationGenerics encoding is v4 and
includes actual parameter syntax locations. Type macro frontiers remain held
until their admitted expansions join this metadata; this is not a macro-only
count implementation.

Separate eight-mode delta review: (1–3) no evaluator, macro execution route or
context method/numeric policy was added; (4) identical retained parameter AST
scan serves original/generated parser snapshots without a generated refusal;
(5) traversal starts at each function parameter Type and skips every Expr/Item,
leaving return Types, executable const declarations and bodies cold; (6) one
walk per memoized declaration and immutable range vectors, no callback source
work; (7) existing query dependency closure/guards, no census or unrelated
namespace invalidation change; (8) existing Arc parser snapshots/direct ranges,
no BodyIndex clones or repeated whole-file searches. Read-only validator's
separate review found no blocking source issue in this delta. The legacy
SignatureParameters helper (test-only call sites) and actual parameter-Type
macro expansion/APIT identity production remain unexecuted/unimplemented gaps;
no complete function binder claim is made.

Added selector
`unchanged_argument_impl_trait_never_claims_zero_upstream_generics_all_editions_and_origins`:
intended 24 additional actual-source worlds, two unchanged imported witnesses,
zero written generics preserved, direct/nested APIT locations resolved in their
actual retained AST, count/domain unknown plus AnonymousFunctionParameters,
typed-generic Parameters pending, ordinary main zero-count, pointer sharing,
cold executable preparation and currentness/revocation. All remain unexecuted
by this worker. `git diff --check` again returned zero. No Cargo, fake provider,
runtime execution, full-factory or gate-completion claim.

V502 frozen read-only prerequisite report (unimplemented design; no source edits,
Cargo or tests). The parent authorized this documentation-only addition.

Actual entry frontier: `upstream/mod.rs::evaluate` calls the pinned
`SolverDelegateEvalExt::evaluate_root_goal`; `prepared::evaluate_prepared` checks
ReadyProvider/context equality before it. Searches for evaluate_prepared,
PreparedFacts::new and DefinitionFacts construction find only their definitions
and exports, not a runtime caller/factory. Current ordinary overloaded operators
still follow `vm/frame_operator_calls.rs::Frame::overloaded_operator` ->
`engine/run_operator_calls.rs::RunMachine::select_operator_method` ->
`members.trait_method` and ordinary selected source calls. Root answer/member
dictionary integration is therefore genuinely absent, rather than hidden behind
the newly repaired variance query. Pinned GoalEvaluation (solve/mod.rs:470)
contains goal/certainty/has_changed/stalled_on, not a selected implementation ID;
runtime witness extraction must use upstream's supported selection/inspection
semantics, not independently rerun custom matching.

Necessary closure versus overbroad readiness:

- Complete relevant source impl membership and pinned candidate classification
  remain mandatory: `PreparedProvider::publish/ready`, ImplTraitCatalog and
  candidates::for_each_relevant_source_impl must not accept a convenient subset.
- Pinned next-solver `solve/trait_goals.rs::consider_impl_candidate` requires the
  actual impl TraitRef, generic arguments, defaultness/polarity, instantiated
  clauses and impl_super_outlives, plus the trait metadata/lang classification
  used by candidate assembly. ParamEnv must come from the real source owner.
  Auto/alias/coinductive/marker/lang-role answers require genuine source/query
  producers, not default false/empty maps. Missing required-role lookup is a
  preparation dependency, never successful absence.
- `PreparedProvider::publish` currently requires every demanded implementation
  times every trait associated member to have an associated_leaves entry
  (loop near line 775). Ordinary TraitClause applicability does not call
  fetch_eligible_assoc_item; actual projection/member demands do. This eager
  Cartesian-product requirement is broader than upstream applicability. Keep it
  until sound typed query-demand/admission exists; merely deleting the check
  would turn an undetected demand into an invariant panic.
- A source function serving only as the owner of a ParamEnv does not thereby
  require FnDef type_of/fn_sig/variance/coroutine/opaque/CTFE/transmute queries.
  For unchanged repr128.rs `discr<T,U>`, existing source generics and Q78 own
  clauses already represent the two Sized clauses and projected PartialEq<U>
  clause without constructing a function DefinitionFacts row. Requiring that
  row merely to obtain the environment would impose unrelated nonzero function
  variance Pending. Actual FnDef values/builtin callable goals still demand
  their real signatures/variance inputs when reached.

Smallest common next source prerequisite: promote the retained own-declaration
clause transport already exercised by
`source_predicates::check_actual_projected_source_predicates` from cfg(test)
into an immutable demanded production bundle, then build the pinned clause
query distinctions rather than fill all PredicateFacts arrays with one list.
Suggested key `(DefinitionHeaderKey, ClauseQueryKind)` and private result with
actual roots/parent binding plus Known clauses or typed Pending. Q78 and
ImplSourcePredicates feed real own recipes, SourceRequirements/SourceBindings
resolve actual IDs, SourceGenericLayouts materializes real binder order and
source_terms::encode_predicate does shared IR transport. Parent-associated
contexts remain Pending until their exact rebasing producer exists.

Exact pin semantics to retain before implementation:
`rustc_hir_analysis/src/collect/clauses_of.rs::clauses_of` adds inferred outlives
and a trait's identity Self:Trait (lines 28–78); `explicit_clauses_of` filters
own associated projection bounds into item bounds (line 505 onward).
`collect/item_bounds.rs::associated_type_bounds` includes the declaration's own
bounds plus applicable parent-trait where bounds, sizedness/default traits and
filter/remapping rules. `rustc_middle/src/ty/context/impl_interner.rs:373–394`
distinguishes clauses_of via instantiate_identity (parent closure) from
own_clauses_of via instantiate_own_identity. Q78 own recipes are consequently
not already a complete ParamEnv/elaborated/all/item/self/nonself query answer.
Reuse those algorithms with typed source/query leaves, not a new solver or
source-text projection filter. Missing inferred/implicit/parent data stays an
explicit query dependency; this is not permission for a borrow checker.

Demand/admission and callback suspension are a separate necessary step, not a
relaxation of ReadyProvider: introduce a typed source query-demand manifest for
the actual external goal, including demanded `(impl, associated item)` leaves.
An outer preparation query can return QueryPoll/typed Pending before entry;
published callback bundles contain only completed query results. No source
provider, parse, initializer or guest operation runs inside callbacks. Missing
data cannot become None, an empty clause list or NoSolution. CTFE/transmute
leaves are needed only when upstream reaches their actual query families.

The current bare-value SourceFacts/Interner signatures cannot directly yield
NeedInput. Supporting an unexpected dynamic callback demand would require a
dedicated typed nonsemantic cancellation boundary around an entirely disposable
root attempt, external memo preparation and a restart from unchanged canonical
input using upstream machinery. This mechanism is NOT implemented or proven.
Do not catch a panic and reuse the inference context: current
infer/snapshot/mod.rs::probe/commit_if_ok perform rollback only after normal
closure return, and pinned solve/eval_ctxt/mod.rs::enter_canonical resets opaque
storage only after its closure returns. Any future cancellation design must
discard the full attempt's inference/search/cache/diagnostic/opaque state,
preserve caller state and apply response constraints exactly once. Until that
lifecycle proof exists, missing callback data is an admission invariant breach;
ordinary resumable needs belong before entry.

Unchanged runtime witness remains repr128.rs `discr<T,U>` and its projected
PartialEq bound, across ordinary/emit/wrapper and four editions. The smaller
associated-types-basic.rs Foo::T normalization witness exercises genuine
projection/leaf/type_of demand; method-self-arg-trait.rs Bar/Foo exercises
applicability without requiring every method leaf merely to prove Foo:Bar,
then separately demands the actually selected method bodies. These are proposed
source/root/runtime closure witnesses, not executed results. All original
readiness, full-factory, runtime/interpreter/JIT gates remain held.
