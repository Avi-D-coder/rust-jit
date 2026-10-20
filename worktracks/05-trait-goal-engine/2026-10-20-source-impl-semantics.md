# M020 ahead-of-entry impl semantic inputs

Before edits: baseline `3d481c9d4eaca5326b23b9a30c1c59b7f7afcd0a`, LOCAL-049 /
WT05-02 / M020. Worker owns only new upstream/source_impl_semantics.rs and this
record. Lead reserves Q86 SourceImplSemantics and owns registration, prepared.rs,
complete source fact assembly and preflight. No existing source clause/item-bound
producer, tests, Cargo or additional agent is owned by this slice.

Invariant/witness: repr128's reached projected PartialEq<U> goal must select its
actual source impl through the pinned solver. The actual facade impl PartialEq for
i128 has an existing completed Rhs=Self head; a real conditional impl must retain
its requirements and missing query domains. Raw/catalog identity is not typed
applicability. Source plans must not supply default false/empty compiler queries
to make a complete DefinitionFacts row. Negative parser activation and its two
official controls remain held; this slice does not activate them or repair legacy
boolean applicability with another solver.

Common route: completed original/generated source catalog -> authentic raw impl
header/canonical owner and ImplTraitIdentity -> actual DefinitionHeaderPlan and
ImplSourceTypes/retained generic layouts -> demanded Q86 immutable semantic inputs
-> lead's source fact/query closure preflight -> existing upstream callbacks and
candidate equality/obligations. The actual source header modifiers retain polarity,
written const/default/safety evidence. Any compiler constness answer uses its
actual query semantics, including the existing Q83 conditionality result. Inherent
impls are invalid domains for trait-only compiler queries; they are not negative
answers or absent trait buckets. Required source metadata stays ahead of entry;
no parsing, queries or producer effects occur inside solver callbacks.

Exact pinned distinctions at ae0fe511b15ab760d7e963b13389d1bc2ab42b18:
collect.rs::impl_trait_header (1565 onward) lowers the real trait ref with actual
Self and carries safety/polarity/constness; ty/mod.rs::impl_polarity (2093) reads
that header; impl_interner.rs::impl_is_const (422) delegates to
is_conditionally_const, not merely the written const keyword. defaultness is
rustc_ty_utils/src/ty.rs80 and lower_impl_trait_header/lower_impl_item source
defaultness, rather than guessed candidate status.

inferred_outlives_of is rustc_hir_analysis/src/outlives/mod.rs11–45: actual Impl
kinds take the _ => [] branch. This is a genuine separate compiler query result.
It does NOT prove impl_super_outlives empty. The latter is collect/item_bounds.rs530:
map the actual impl trait reference through rustc_type_ir::elaborate, retain only
TypeOutlives/RegionOutlives, and bind the resulting clauses. Its default Filter::All
elaboration reads explicit_implied_clauses_of and Sized language classification;
HostEffect/outlives alias paths can require implied const bounds and alias variance
inputs. The retained shared elaborate.rs/outlives.rs supplies all
semantic branches. Do not replace it with a primitive/empty/supertrait-name walk.

Q80/Q81 expose explicit own/parent source requirements, and Q82 retains separate
output/default/parent candidates with full remapping/elaboration holds. Those
plans are useful dependencies, not certifications of full explicit-implied queries
or the outlives closure. Q86 exposes a real dependency descriptor and typed pending
until the lead's authentic query closure preflight can issue authorization. Per
lead direction no algorithm method or closure certificate is implemented this
turn; the descriptor alone, an arbitrary callback or a partial map cannot certify
readiness. No dummy
WF/root entry or fake complete provider is added.

Demand/cost: Q86 is keyed by ImplHeaderSyntaxKey (actual inventory + raw impl),
retains immutable Arc source query/header/type roots and canonical identity, and
exposes independent domain/answer/pending values. Header metadata is cold; typed
source head encoding reuses its existing memoized plan and generic layout. Full
elaboration remains a separate demanded computation after closure readiness.
No hot census, per-operation planning, body/signature lowering, BodyIndex copy,
resource accounting or new unsafe code. The source review must name all eight
recurring architecture modes at final fingerprints; source-only readiness does
not establish an actual root, full compiler clause closure or runtime acceptance.

Closure evidence remains lead/validator-owned: genuine existing source candidates,
original/user/wrapper and editions, actual identity/modifier/defaulted-head facts,
separate inferred/super outlives domains and typed missing closure. No builds/tests
are run by this worker, and no new test or fixture is introduced in this slice.

## Bounded null-clause follow-on and final API

Lead requested the genuine degenerate `clauses_of` branch during this slice.
`PreparedSourceImplSemantics::complete_clauses_of` returns
`ImplSemantic<EmptyImplClauses>` only after actual Impl authentication,
DeclarationParameters count zero, no written generics/predicates/default impl,
no unstable_feature_bound or unproduced TransformUnsupported attribute, and
the actual owning crate's SolverConfiguration proves generic_const_exprs off.
The configuration root is selected from the retained completed environment by
actual child crate/profile and unique root identity; it is not the guest root
guessed for a dependency. Other branches retain ClauseParameterLayout,
ClauseParameters, ClauseDefaultImpl, ClauseWrittenPredicates,
ClauseAttributeEffects, ClauseConfiguration or ClauseGenericConstExpressions.

Exact compiler proof: collect/clauses_of.rs28–75 adds inferred outlives and (for
traits only) Self:Trait; actual Impl contributes neither. Lines82–315 have no
RPITIT/delegation/trait/opaque branch for the real Impl node, no default-impl
predicate, no parameter implicit/default/ConstArgHasType clauses, no written
predicates, no GCE clauses and no unstable-feature-bound clauses under the
captured conditions. collect/generics_of.rs's Node::Item(Impl) has no generic
parent. constrained_generic_params.rs::setup_constraining_clauses takes a mutable
slice and only reorders it; it cannot add a clause to the empty result.
EmptyImplClauses has a private constructor and exposes parent None / recipes [].
This is one actual query result, not empty values for every PredicateFacts field.
The ordinary non-null Q80/Q81 written own/parent transport is unchanged and is
not relabeled as full compiler clauses_of.

Registration: Q86 `QueryKey/QueryValue::SourceImplSemantics`, key alias
`SourceImplSemanticsKey = ImplHeaderSyntaxKey`, result
`PreparedSourceImplSemantics`; lead owns db/module/provider integration.
`prepare`, `key`, `pending` and `CurrentSourceImplSemantics::{capture,plan,
require_current}` follow existing immutable query/currentness infrastructure.
Structural pending is not whole-query readiness: modifiers, polarity, default,
unsafe and compiler conditional constness use independent typed ImplSemantic
answers. Inherent impls are NotApplicable for trait-only queries. The separate
inferred_outlives_of is genuinely empty for any authenticated Impl. Full
impl_super_outlives is always ElaborationClosure Pending for trait impls.
`super_outlives_demand` identifies actual canonical impl and implemented trait;
its direct query families are Sized language identity, explicit implied clauses,
explicit implied const bounds and alias variances. Correction to the initial
investigation: item_self_bounds is read by the separate
outlives.rs::declared_bounds_from_definition helper, not directly by this
elaborator's push_outlives_components path. No closure or empty super-outlives
claim follows from its absence in this descriptor.

`source_semantics` retains Q83/header provenance, `source_types` retains the
common ImplSourceTypes source bindings, and `generic_layouts` refuses structural
or typed source pending before constructing the context-owned parameter map.
`encode_head` delegates to that existing pure encoder and verifies the encoded
trait identity against actual ImplTraitIdentity. It does not call TraitRef
construction, elaborate, the solver or missing callbacks. Source planning is
memoized once; the optional null-clause preparation scans only retained source
selections to join the owning root, with no executable preparation or hot path
census. Scalar availability conservatively waits for the retained actual header,
and Q83 can capture parent scalar metadata; this is not an exact minimal-demand
claim for each scalar callback.

## Separate source architecture review

Reviewed module fingerprint
`7ffed5357c46800dcb7362755f8c019ec7fe3e13ed1df4d24e0bbd0ac06e416a`.
This review covers the worker module and inspected shared entry points, not later
lead integration or a runtime/root execution. Applicable E-R5-005 / eight-mode
findings are separate from MA001–MA008 numbering.

1. No assertion/expression compiler or evaluator: prepare consumes genuine
   ImplTraitIdentity, SourceDefinitionSemantics/header and ImplSourceTypes;
   encode_head uses the ordinary shared source encoder only.
2. No literal/break macro route: original/generated/wrapper raw headers join
   through the existing ImplDeclarationContext and canonical owner receipt.
   No new generated-source condition or exclusion is introduced.
3. No method/numeric inference special case: no fixture/name/primitive filter
   exists. Actual trait identity/modifiers apply equally to every admitted impl.
4. No generated rejection or hygiene bypass: matches_metadata verifies child
   ParseKey/HeaderRecipe/DeclarationContext against the retained identity;
   missing canonical/parent/header evidence stays typed pending.
5. No pattern machinery or cold executable checking: no body/signature lowering,
   staticchecker, frame preparation or branch evaluation is requested.
6. No repeated parsing/planning at execution: prepare is a pure memo provider,
   Arc roots retain the plan, encode_head reads no Database, and currentness uses
   the existing PreparedInputGuard. Full elaboration remains held, not repeated.
7. No dispatch census: only cold environment root selection is added; retained
   query dependency fingerprints and positive/missing input guards govern reuse.
   Cost/scaling of repeated cold demands is unmeasured; no performance claim.
8. No BodyIndex or immutable body clone/search: only Arc handles are cloned;
   no executable tree or whole-file search is added by this module.

Independent reviewer found a source generic-layout availability hole in the
draft (unresolved head could leave empty SourceBindings). It was corrected by
checking structural and ImplSourceTypes pending before returning layouts.
No build/test/Cargo/guest or compiler runtime was executed by this worker.
Original/generated/wrapper, four-edition, conditional/nonempty, default impl,
GCE, attributes, inferred/super outlives, memo revocation and actual root controls
remain unexecuted for this new module. Repr128 projection/PartialEq and the
unchanged conditional-impl witnesses remain the real integration witnesses;
the negative-bound parser and controls remain held. Host abort policy, runtime
contract reporting and interpreter/JIT/full-factory gates are unchanged.

## Exact source references

All paths below are relative to compiler/ at pin
ae0fe511b15ab760d7e963b13389d1bc2ab42b18. References inspected in the official
checkout are reconstructible pinned inputs; this slice adds no vendor source or
handwritten replacement for rustc's elaborate algorithm.

| Path | SHA256 |
| --- | --- |
| rustc_hir_analysis/src/collect.rs | 0fdb73851e12d6c135fb1ba33746523966843647a62bb899c1e17aa0d46ac211 |
| rustc_hir_analysis/src/collect/item_bounds.rs | aa745ea1aab56ef3b39d06f28dd77597604acf8c5d0ffcaf78ddf7506733f1c7 |
| rustc_hir_analysis/src/collect/clauses_of.rs | 465f5dd0f397f87716aedc65048a12dce7ec3af51d88e584ea5ad902b2251bf3 |
| rustc_hir_analysis/src/collect/generics_of.rs | 901423a7bc0e566b5464b756862a35d11d7530fd5fbe2ca1ae1dd70d2bdc9452 |
| rustc_hir_analysis/src/constrained_generic_params.rs | 885f639e6bfe767a08cca157d97627aa09b11e015ea81a59990334ca05bbe0dd |
| rustc_hir_analysis/src/outlives/mod.rs | 3b7aadd0577a4c0b6a6939f23b9e5e3ea9e66e015860fff7b5b0eda4f31fe7e9 |
| rustc_ty_utils/src/ty.rs | 0d1edae3ada2b3186448a44c151a6d07916461b781f1803edb9f1e5f6bd5afee |
| rustc_middle/src/ty/context/impl_interner.rs | 9df7d014fea7d44e4d8e09022fc94ccb1356fff57a78ff697250e3c7a6b0e920 |
| rustc_type_ir/src/elaborate.rs | c7f6f7cedb01a8aa21942e6121920ac0153defee800d8024176e4fe0a4a0a41d |
| rustc_type_ir/src/outlives.rs | d6dfd70b229b97e0fb835874448cba78ee306002040895116a6f72d1ca0bf563 |
