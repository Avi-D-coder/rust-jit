# M020 actual projected predicate root-input observations

Before edits: LOCAL-049 / WT05-02, baseline `1084e893d0`. Lead owns the common
root_variables/source_terms bridge and root lifecycle; worker owns only the
existing cfg(test) check_actual_projected_source_predicates in upstream
source_predicates.rs and the unchanged-fixture control in CLI
upstream_projection_types.rs. Q83 source_definition_semantics remains frozen.
Read the reached-upstream-root-inputs record; the host-unwinding policy question
and actual solver/root/runtime work remain with lead. No Cargo/tests by worker.

Invariant/witness: unchanged repr128.rs's real Q80 declaration predicate
<T as DiscriminantKind>::Discriminant: PartialEq<U> must transport an actual
owned U inference key to the same pinned integer inference variable. Preserve
actual trait/member identity, source T parameter, child projection and spans.
Use existing Substitution on the actual recipe's Type terms, existing source
encoder and actual PostAnalysis InferenceContext/SolverContext. No synthetic
source, dummy root or independent inference/substitution solver is introduced.

The existing source helper retains its Q80/count/projected return contract.
Add observations inside its existing scoped source-context closure: repeated
root encoding shares one real IntVar; rigid declaration encoding rejects Infer;
another inference owner with the same Cx is rejected; an unregistered source key
stays pending. The existing CLI fixture/origin/edition loop and cold/memo/source
withdrawal checks remain, with no changed official source or extra fixture.
Existing real configuration, generic layouts, bindings and projection receipt
provide all authority. No solver root executes and no value escapes the arena.

All work is test-only cold source observation. The production shared walker is
lead-owned; no alternate parser/evaluator, macro/context policy, body preparation,
hot namespace census, repeated installed planning, body clone or new unsafe code
is added. Required evidence is a frozen actual twelve-world rerun plus source
review at exact fingerprints. Unexecuted root evaluation, response application,
real fact admission, suspension and repr128 runtime acceptance remain held.

Implemented observations use the actual Q80 PartialEq<U> projection recipe and
actual separate own Sized(U) default. Existing Substitution applies the source U
parameter replacement to those real source terms. One actual PostAnalysis
SolverContext/InferenceContext and RootVariables integer key serve both clauses;
repeated encoding also preserves the same real variable. Actual projection alias
checks retain the source T parameter, DiscriminantKind and Discriminant IDs. The
rigid encoder rejects inference, another SolverContext with the same Cx is refused
before encoding, and an unregistered source key yields typed Inference pending.
No root evaluation, canonical response, integer fallback or runtime mutation is
performed. The existing 3-clause/1-projection helper counts and twelve-world
CLI matrix are unchanged.

Separate eight-mode source review at the following hashes: (1–3) only cfg(test)
source observations are added, with no evaluator/solver/macro-specific inference
policy; (4) the existing actual original/emit/wrapper source Q80/projection join
is reused unchanged over all four editions; (5) no body/signature/pattern/branch
preparation; (6) existing retained Q80/configuration/layout/parser plans are reused
once, and the scoped encoder cannot query db; (7) no namespace construction,
census or changed invalidation; (8) no immutable body copies or whole-file node
search. Existing source cold/memo/currentness/withdrawal assertions remain. The
only production encoder is the lead-owned source_terms common walker, and the
only variable store is the pinned inference owner. No borrowed IR leaves the
source-context closure. Negative controls are raw input-boundary observations,
never fake complete source facts or successful root outcomes.

Ready hashes:

- traits/upstream/source_predicates.rs
  `6ca30111e404b23325b4642ce9bccf5741adec3081e5b34111373343924c0226`
- cli/preentry_loader_tests/upstream_projection_types.rs
  `2dbb7e0728ecaecd6c93946fbf724d122dd6e73c3feda20829cd05afef8939fb`

The changed cfg helper was formatted independently with rustfmt +1.99.0; git
diff --check passed. No Cargo/tests were run by the worker. This records source
readiness only; actual terminals belong to the validator after global freeze.
Q83 production remains unchanged at its previously frozen fingerprint.

Pre-freeze review caught a real interaction with the concurrent Q84 producer:
Q78 now publishes defaults as Explicit Implemented rows, so searching only for
the removed legacy ResolvedSizedDefault representation was incorrect. The control
now observes the actual already memoized Q84 SourceDefaultBounds plan before the
scoped encoder, obtains its SourceLangIdentity-backed Sized declaration ID and
the actual Q78 U parameter span, and selects the genuine explicit U row by all
three facts (Self parameter, trait identity, source span). The common root encoder
uses that real row; no default clause is manufactured. The no-source-query count
starts after this actual preparation and still encloses all inference encoding.
Earlier ready hash is superseded by upstream/source_predicates.rs SHA256
`95d18b2c159586a9134d27fa58e1d38c6f57e592039349d4346047e7471c1aaf`.
CLI hash remains unchanged. RootVariables' existing duplicate-key precheck was
source-inspected to reject before allocation; no extra duplicate-input test or
claim of execution was added in this slice.

## Read-only omitted generic default audit

Frozen production was not changed and no compilation/control was run. The actual
facade declaration core/cmp.rs5 is PartialEq<Rhs: ?Sized = Self>; the candidate
core/primitive_impls.rs181 is impl PartialEq for i128. There is already a genuine
common Type-default completion route: impl_head_types preparation calls
source_types::projection_trait, which calls complete_trait_arguments (1268 onward).
That function fetches the actual TraitDeclaration, resolves each omitted Type
default from the retained TypeParam.default_type AST in its defining scope, passes
the actual impl Self Type through WrittenTypeDeclaration.impl_self, and supplies
already accumulated positional arguments through real ParamKey type_arguments.
The common Type walk's Self leaf (782) returns that supplied Self; ordinary
parameter leaves use parameter_type (459). Thus this source path should retain
Rhs=i128 in the actual source TraitRef before any shared IR encoding. This is
source inspection, not an executed i128/default acceptance claim.

PreparedImplSourceTypes::prepare then collects the actual completed reference
argument Types through SourceBindings::prepare. Encoder::trait_arguments merely
encodes those existing arguments and prepends Self; the expected candidate IR is
PartialEq<i128,i128>. It neither owns nor loses the default when fed this genuine
Q58/Q70 route. SourceBindings retains DeclarationGenericTypes and the referenced
default result's Type dependencies, while SourceGenericLayouts owns parent-first
argument kinds/counts and source ordinal maps. These layers are not general
generic-default producers: DeclarationGenerics retains written default tokens,
DeclarationGenericTypes currently resolves const parameter Types only, and the
shared Generics/GenericParameter representation carries kinds/indices/const Types,
not has_default/default-value compiler query payloads.

Pinned semantics agree for this declaration-context witness:
rustc_hir_analysis/src/hir_ty_lowering/mod.rs::lower_mono_trait_ref (1187) supplies
Self to lower_generic_args_of_path; generics.rs::lower_generic_args (190 onward)
handles Self before parameters (230), then passes preceding_args to inferred_kind
for omitted arguments (391). mod.rs::GenericArgsCtxt::inferred_kind (810 onward)
uses type_of(param.def_id).instantiate(preceding_args) for Type defaults only when
!infer_args && has_default (828–843); Const defaults use const_param_default with
the same preceding arguments (853–867). Type/inference/value-path modes must not
be conflated. The LRR source helper still explicitly holds Const generic arguments,
omitted lifetime arguments and unsupported symbolic Types; no general default or
value-path inference completion is established here.

Smallest integration recommendation: the actual impl-fact assembler must consume
PreparedImplHeadTypes/PreparedImplSourceTypes's already completed trait reference,
not reconstruct a Self-only reference from ImplTraitIdentity (identity-only
metadata) or raw syntax arguments. Verify actual supplied argument kinds/count
against retained SourceGenericLayouts before the upstream TraitRef constructor;
missing completion should stay typed preparation pending, never empty args or a
primitive-name fallback. An actual i128 candidate observation of Self/Rhs from
this existing route is the next narrow source prerequisite evidence if needed.
Only a bypass or a genuine unsupported default should trigger new production
work. A future general default-query producer would need retained actual default
AST/source spans, real generic ordinal/parent/Self substitution, declaration-vs-
inference mode, Type/Const query payloads and transitive source requirements;
it belongs in common demanded source preparation, not the solver callback or a
PartialEq/i128-specific encoder rule. No new claim or source slice was opened.

## Read-only PartialEq candidate/root preflight audit

During the current validation freeze, no production source or tests were changed.
This follows the tail after the actual projected Self has normalized to i128;
DiscriminantKind/enum normalization metadata is the other worker's separate slice.
Pinned next_trait_solver solve/assembly/mod.rs::assemble_and_evaluate_candidates
(481 onward) structurally normalizes Self before selecting candidates; for the
normalized integer, candidates.rs::for_each_relevant_source_impl uses the pinned
simplified constructor bucket plus every actual blanket impl. It must not be
replaced by an i128/PartialEq whitelist, and the complete bucket cannot be inferred
from a single successful candidate. PreparedImplTraitCatalog supplies authentic
raw/canonical membership and source classification, not typed applicability.

Actual pinned trait_goals.rs::consider_impl_candidate (60–126) reads impl_trait_ref,
deep-rejects incompatible arguments, rejects default impls and wrong polarity,
freshens actual impl generics, equates goal/candidate TraitRefs, adds clauses_of
and impl_super_outlives, then evaluates added goals and produces its canonical
response. Thus the actual facade primitive_impls.rs181 candidate can constrain
the open U to i128 through the compiler's equality machinery using the existing
completed [i128,i128] reference. This path requires no eq/ne function body or
signature, no has_item_definition/associated leaf merely to prove PartialEq,
and no CTFE/transmutability client. Reflection-only metadata is not read in
PostAnalysis. Candidate merging (trait_goals.rs1575/1627/1720) additionally needs
actual marker/sizedness/auto/default flags and may consult specialization when
multiple applicable candidates survive; no empty specialization graph is guessed.

Available source plans: ImplTraitCatalog and ImplTraitIdentity give sealed whole
trait membership; ImplHeaderSyntax::modifiers supplies actual negative/const/
unsafe/default source attachments; Q58 ImplHeadTypes/Q70 ImplSourceTypes give the
real completed head Types and parent-first SourceGenericLayouts; Q80/Q81 retain
own and parent source clauses with actual spans/defaults; Q83 supplies source-proven
per-query scalar domains; per-definition SourceLangIdentity and owning solver
configuration support demanded language/default-role facts. Q80/Q81 are explicitly
ExplicitOwnClauses, not a full clauses_of answer. The entire PartialEq bucket,
including real blanket heads, must be authenticated before the relevant index is
safe; only relevant candidates' full predicates should need semantic preparation.

Remaining genuine query producers: (a) trait flags/negative language classification
must come from actual trait header/attributes and SourceLangIdentity/configuration,
not name guesses or an empty language registry; (b) full clauses_of must account
for pinned collect/clauses_of.rs28–75 inferred additions and its implicit/feature/
const-argument/default-impl branches; (c) impl_super_outlives is pinned
collect/item_bounds.rs530's actual elaborate(impl_trait_ref) filtered to outlives;
an empty Q80 list alone cannot prove it empty; (d) specialization/associated-leaf
and complete typed-provider admission still have no complete source assembler.
There is a useful exact domain fact for a future common full-clause producer:
outlives/mod.rs11–45 returns empty inferred_outlives_of for actual Impl kinds.
The nongeneric non-default i128 impl has no written own requirements, but a full
answer still needs the genuine feature/implicit/attribute branches authenticated;
this audit does not promote its source list to a compiler query result.

Current PreparedProvider::publish still demands complete DefinitionFacts records
and every associated leaf for every published trait impl/member pair, and ready
builds its index from those full rows. Those preflights are broader than this
plain trait-candidate tail (PartialEq's eq/ne leaves are not queried here), but
they remain enforced until a sound typed demand/admission contract replaces them.
Cargo dev/release still use panic=abort: missing bare SourceFacts callbacks cannot
be caught to prepare more facts. Actual root entry must therefore wait for a
proven query closure outside the attempt; a missing domain returns preparation
pending before ReadyProvider/evaluate_prepared. Do not enter speculatively, fill
unrelated fields with defaults, remove Cartesian checks alone, or construct a
dummy WF root. The smallest common next contract is a typed per-query preflight
plus authentic full impl-clauses/super-outlives and trait-flag producers, reusing
existing completed heads/catalogs. Actual root evaluation, response application,
reached-call fallback ordering and repr128 runtime acceptance remain unexecuted.

Root-input precision: the reached call's obligation must instantiate actual T
with Signed (then normalize its real discriminant) and leave U open, under the
caller's actual ParamEnv. Reusing discr's own projected PartialEq<U> requirement
as an assumption for that same entry obligation could let the pinned param-env
candidate shadow impl search and would not demonstrate inference from the i128
impl. The current source control intentionally still preserves T as a source
parameter and does not evaluate a root; it establishes neither this call-site
instantiation nor the correct runtime ParamEnv.

Policy footnote for the V511 follow-on: the earlier reached-upstream-root-inputs
record preserves the historical question about disposable host unwinding. The
user subsequently asked about preparing facts ahead of entry; there is no
approved or implemented host-unwinding/profile-policy change. Lead proceeds under
the existing panic=abort contract: prepare the actual required source-fact/query
closure before entry, return typed pending outside the root when it is missing,
and retain the current guards. Metadata work is not waiting for a profile change.
This footnote records the active path without deleting the earlier design-question
context or claiming that the complete closure/root producer already exists.
