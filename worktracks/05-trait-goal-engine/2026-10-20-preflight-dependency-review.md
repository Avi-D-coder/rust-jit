# M020 source-free entry dependency review

Reviewer `/root/wf_validation_review`; baseline
`3d481c9d4eaca5326b23b9a30c1c59b7f7afcd0a`, prior V512 source fingerprint
`ebea0de06be9db158f640f7d6708335b1cfa9236a2a55edc481c8a2d4d215ec9`.
Read-only source review; only this new handoff is reviewer-owned. No builds,
tests, compiler invocations or production edits during mutable-source review.
Final V513/V514 fingerprints and execution receipts appear below. No upstream
root evaluation or complete source-free factory acceptance is claimed.

The next genuine factory boundary is a closed source-query closure for the
reached projection/PartialEq obligation, followed by scoped per-query
publication and a root-bound entry certificate. Q85 trait flags, Q86 impl
inputs and Q87 alias layouts supply necessary leaves. They do not alone supply
complete implied-clause/elaboration inputs or authorize solver entry.
The decisive remaining path is complete applicable impl clauses plus
`impl_super_outlives`'s real `explicit_implied_clauses_of` closure. Existing
mandatory whole-definition publication also needs a representation repair;
filling unrelated fields with false/None/empty is not a shortcut to readiness.

## Prove the actual call obligation, then assume it in the callee

The unchanged primary fixture is enum-discriminant/repr128.rs. At the first
reached call, its actual discr requirement becomes
`<Signed as DiscriminantKind>::Discriminant: PartialEq<?IntegerU>`; Unsigned
has the analogous actual source enum. T must be the actual reached enum
application, not the callee's still-symbolic T parameter. Preserve integer
inference kind and the original variable handle until after upstream response
application, before CallSeal's literal fallback. Calls whose value already
has a concrete integer descriptor need a check, not a new unconstrained type.

The caller ParamEnv contains only genuine caller assumptions. In main it can
be authenticated empty. Do not install the callee requirement under proof into
that environment. Pinned assembly/mod.rs:515–550 prefers unconstrained applicable
environment/alias-bound candidates and may skip source impls. An assumption of
the target obligation would thus prove itself without obtaining the required
integer guidance. Callee-body assumed clauses/dictionaries are installed only
after call-site proof and owned result/currentness validation.

Q80 supplies three actual own source requirements: Sized(T), Sized(U), and the
projected PartialEq(U) bound. A scoped inference operation may evaluate the
third real requirement to obtain U; it cannot advertise a complete call contract
unless the other requirements are also discharged by genuine inputs. Sized(U)
has upstream integer fast paths; Sized(T) may additionally demand genuine enum
structural-sizedness metadata. A dummy WF goal at main entry is not this work.

## Exact pinned solver route and its query families

`prepared::evaluate_prepared` delegates to the existing pinned root evaluator.
The delegate's fast path reads actual language classification; the source
projection remains non-rigid in the input. `solve::assembly::
assemble_and_evaluate_candidates` structurally normalizes self before matching
trait candidates. `solve/mod.rs::structurally_normalize_term` creates the real
Projection goal; `normalizes_to::compute_normalizes_to_goal` proves the
corresponding DiscriminantKind trait and assembles normalization candidates.
The builtin DiscriminantKind branch in normalizes_to.rs:985–1038 calls the
existing pinned discriminant-Type client for a concrete enum. Its AdtDef
queries reach actual enum kind and source repr, returning i128/u128 into
upstream equality/inference. The adapter must not replace the input projection
with a manually normalized primitive.

With primitive self, PartialEq assembly visits its actual relevant impl bucket
and every blanket impl. Pinned trait_goals.rs:60–123 obtains impl_trait_ref,
checks defaultness and polarity, creates real fresh impl arguments, relates
the trait refs, adds instantiated clauses_of and impl_super_outlives, then
evaluates those goals. Canonicalization, candidate transactions, equality,
normalization, ambiguity, specialization and search cycles remain upstream.

| Required domain | Actual source/client route and boundary |
| --- | --- |
| Root/config/world | actual Q80 requirement, reached T/U bindings, source span, source SolverConfiguration, exact catalog/epoch/publication and owning editions |
| Generic identity/layout | DefinitionReceipt, DeclarationGenericTypes, DeclarationParameters and SourceGenericLayouts for the actual trait, associated item, enum and admitted impl declarations; source parameter identity/order/parent counts remain checked |
| Projection identity | Q77 actual Discriminant item, actual parent, Q87 argument kinds, SourceBindings projection receipt; Q83 named-item IsImplTraitInTrait=false is needed by opt_alias_variances |
| Lang roles | Q76 actual defining source/facet receipts for DiscriminantKind, Discriminant, PartialEq and sizedness roles; no spelling-only IDs or guessed role absence |
| Trait scalar queries | auto/alias/coinductive/marker and classification as sizedness/default; Q85 handles actual source flags/config. CurrentGoalKind reads coinductive; assembly reads auto/alias/lang; CandidatePreferenceMode always reads sizedness/auto/default; impl merging reads marker |
| Enum representation | actual enum kind + PreparedEnumDiscriminant schema/repr receipt. Discriminant type does not demand execution of Zero/Next or other initializer expressions. Current AdtInfo coupling must use genuine captured shape or be partitioned; do not invent empty fields/destructor/unsizing facts |
| Candidate coverage | actual complete trait source membership, canonical raw-to-source join, pinned simplified-head index and exact relevant-constructor plus blanket coverage; pending membership never means no impls |
| Impl head/scalars | actual ImplHeadTypes/ImplSourceTypes, Q86 polarity/defaultness, complete generics and actual trait-reference arguments. Existing complete_trait_arguments reads genuine Rhs=Self defaults from the retained trait declaration |
| Impl complete clauses | full clauses_of semantics for each demanded candidate, not an alias for Q80 explicit own recipes. Q86's bounded null branch can prove genuine empty for actual parameterless nondefault/no-where/no-attribute-effect/GCE-disabled impls |
| Impl super outlives | exact actual head through existing TypeIR elaborate, then TypeOutlives/RegionOutlives filtering; requires genuine filtered explicit-implied closure, not guessed [] |
| Successful export | retained original root variables resolved after root response, supported owned results and external constraints, dependency guard validation before VM hints/dictionaries change |

Trait metadata needs individual availability checks. A plan-level pending()==None
only proves source capture, not that every Q85 flag or Q86 domain is Known.
Dyn compatibility remains pending without a genuine compiler producer, except
where an actual force-incompatible attribute proves false. This concrete root
does not contain dyn, opaque, function, coroutine or const terms; do not prepare
their bodies/CTFE/transmute clients solely to complete an eager struct.

## Candidate closure: preserve all possible competitors

Actual primitive PartialEq impls already exist in
facade/core/primitive_impls.rs (i128 around line 181, u128 around line 427).
No artificial builtin equality impl is needed. Their omitted Rhs is completed
through `source_types::complete_trait_arguments`, using the real PartialEq
declaration/default Self. This is source transport, not candidate matching.

ImplTraitCatalog currently classifies every raw header's trait identity once
and requires complete membership before proving an empty bucket. Its
require_translation and PreparedProvider publication additionally require
translation of every impl of a published trait, not merely relevant heads.
That stronger current contract may be retained for an initial bounded factory
when all such head translation succeeds; do not silently omit unsupported rows.
A later demand partition may separate complete authenticated membership/head
indexing from richer predicates/scalars for relevant candidates. Every omitted
constructor must be excluded by the pinned classifier, not by an LRR semantic
guess or a convenient fixture subset.

For the closed-enum target, the actual normalized head can only come from its
authenticated repr branch, and the source-free solver must still perform that
normalization. The ahead collector may conservatively include both signed and
unsigned integer constructor buckets (or all integer buckets) and all blankets;
it must never replace or pre-answer the goal. If self can remain an integer
inference variable, candidates.rs intentionally visits all integer constructor
buckets. Generic/rigid aliases introduce item_self/non_self bounds and wider
normalization dependencies: do not admit them under a certificate proved only
for a concrete enum source application.

Specialization/associated-leaf facts are demanded only on actual reachable
branches. Complete bucket coverage remains necessary even when a builtin or
one primitive impl appears sufficient. More than one relevant candidate can
introduce specialization reads and nested obligations; their missing inputs
must block admission rather than be interpreted as an empty ancestry set.

## Genuine producer chain after Q85/Q86/Q87

1. Retain the actual source/root/caller recipes and existing DefinitionReceipt,
   SourceBindings, generic layout and enum-repr plans. Fetch complete trait
   membership and all heads required by the pinned candidate indexing contract.
   Admit Q85/Q86/Q87 individual Known domains, not an eager all-fields factory.
2. Prepare complete clauses_of for the demanded primitive impls. Pinned
   collect/clauses_of.rs:28 starts from explicit_clauses_of, appends
   inferred_outlives_of, and adds Self:Trait only when the owner is a trait.
   outlives/mod.rs:11–43 returns actual empty for DefKind::Impl. In the admitted
   zero-parameter/nondefault/no-where/no-feature-attribute/GCE-disabled branch,
   explicit clause gathering adds none and constraining-clause setup only
   permutes the empty input. Q86 now exposes a private EmptyImplClauses witness
   for precisely that branch. Other impls remain typed pending; no general
   complete-clause claim follows from Q80/Q81 source lists.
3. Implement actual filtered implied-clause preparation. Pinned
   clauses_of.rs:618 uses SelfOnly for explicit_super_clauses_of; :636 uses
   SelfAndAssociatedTypeBounds for regular-trait explicit_implied_clauses_of,
   All for trait aliases. Filtering Q78 by superficial names or copying all
   Rhs parameter bounds into these families is incorrect. Retain actual source
   subjects, implicit/default policy, where and associated-bound ownership,
   substitution namespaces, positive/negative polarity and source spans.
   Unsupported equalities/HRTB/configuration remain explicit source gaps.
4. Close those actual query dependencies before invoking the retained
   rustc_type_ir::elaborate. For the current facade PartialEq declaration, its
   Self-sizedness implications lead through actual MetaSized/PointeeSized
   declarations; the Rhs ?Sized default is not a Self supertrait. The pinned
   Sized short-circuit, identity instantiation and visited-set traversal remain
   upstream. Call the pinned item_bounds.rs:530 map/elaborate/filter body for
   impl_super_outlives only after this closure is admitted. A primitive head
   alone is no proof that the transitive outlives result is empty.
5. Publish scoped query domains and construct the exact-root entry certificate;
   use the existing evaluate_prepared/upstream root and owned export. No
   callback may fetch Database queries, parse syntax, execute macros/guest
   code or guess data. Under unchanged panic=abort profiles, closure errors
   must be caught as typed preparation outcomes before entry; there is no
   expected host-unwind suspension protocol.

The elaborator's directly reachable query families also include Sized lang
classification, explicit implied const bounds for actual host-effect clauses,
and alias variance/outlives-component metadata on applicable outlives branches.
Q86's descriptor is a dependency description, not populated keys/answers or a
closure certificate. An observed successful short trace cannot certify those
unobserved branches for broader inputs.

## Root-bound, source-free entry certificate

Current ReadyProvider binds only provider/Cx, while evaluate_prepared accepts an
arbitrary Goal. A narrowed ahead-preparation certificate must not be reusable
for another goal in the same Cx. Bind the actual owned root recipe/instantiation,
caller assumptions, inference keys/kinds, typing mode, configuration, catalog,
epoch and source dependency guards. Keep scoped goal/certificate construction
private and use the existing root/source encoder. Root recipe identity is not
a new semantic canonicalizer; upstream still canonicalizes the actual goal.

Use immutable owned source-query values/currentness guards to prepare the
closure, then materialize typed IR in the existing scoped Context. A dependency
worklist here enumerates actual query keys/families from source plans; it must
not select candidates, solve obligations, manufacture new inference terms or
become a second cycle scheduler. Use existing memo/cycle machinery and pinned
limits, not a retry/count/time/resource cap. A producer that cannot prove its
dependency closure returns typed Pending/NeedInput before solver entry.

The certificate should prove both value authenticity and callback coverage for
its admitted input shape/pinned algorithm revision. Every semantic callback
reads frozen typed maps only; inaccessible/unpublished is distinct from a
genuine Known(false/None/empty). If unexpected callback access remains possible,
the contract is not yet proved and cannot be advertised as source-free complete
entry. Do not use a sentinel value or catch an arbitrary panic as NoSolution.

Retain publisher checks for real world/definition/generic identity, exact source
membership and pinned index comparison. Partition the current all-associated
member/impl Cartesian leaf requirement only where the entry closure proves
those callbacks are unreachable. The ordinary trait proof does not select
eq/ne bodies and builtin Discriminant normalization does not require an
invented associated implementation leaf. Unrelated signature, opaque, CTFE,
variance and predicate families stay Unavailable, not fabricated absences.

On success, inspect the original SolverContext variables after response
application: GoalEvaluation.goal is explicitly pre-response input. Export only
supported owned values/constraints and actual Yes/Maybe/NoSolution outcomes.
Validate currentness before installing numeric U or dictionaries; no guest cell,
Frame hint or visible diagnostic mutation precedes successful export. PartialEq
proof/inference is separate from any later source method/dictionary execution.

## Independent eight-mode pass and present source scope

1. The semantic entry remains pinned root evaluation; Q85/Q86/Q87 are source
   inputs, not an assertion evaluator, canonicalizer or parallel trait solver.
2. Ordinary/generated requirements use common retained source/query/encoder
   routes, not a literal/break-only macro executor or a dummy WF root.
3. Numeric U is constrained by the actual instantiated PartialEq requirement
   before ordinary fallback; no assertion/method-name inference rule is added.
4. Source identities/parents/lang roles/layouts require actual receipts for each
   origin and owning edition. Missing metadata remains preparation pending;
   no original-only bypass or generated exclusion is introduced.
5. Source-only signatures/requirements do not authorize callee-body checking,
   cold-arm preparation or static borrow/privacy/const checks. Inference and
   candidate transactions remain upstream; no pattern machinery changes.
6. Q85/Q86/Q87 retain query Arcs and existing parsed nodes; materialization and
   closure preparation occur once cold. Installed execution must retain an
   owned answer plus relevant guards rather than reconstruct these plans.
7. Complete source membership/index construction is cold publication work;
   root/call guards must not rebuild a world namespace census. Relevant positive
   and negative bucket dependencies, including new competitors, remain required.
8. Borrow immutable source/body data and indexed declaration nodes; no BodyIndex
   deep copy or per-operation whole-file scan belongs in the new closure. Q85
   attr checks, Q86 header/config checks and Q87 namespace construction have
   source-size cold costs; no measured speed/scaling claim is established.

Q85's effective attribute/source/token and header joins were reviewed against
pinned collect.rs::trait_def, including alias exclusions and auto coinduction.
Sizedness uses actual sized/meta_sized roles, not PointeeSized or spelling;
default classification false requires the actual admitted no-override receipt.
Dyn-compatible absence is pending, not true. The facade now retains actual
DiscriminantKind/Discriminant language attributes and nine output bounds, and
actual auto Send/Sync/Unpin metadata. Q86's actual scalar/null-clause domains,
separate super-outlives hold and generic-layout pending gate were reviewed.
Q87 authenticates actual Q77 projection identity and DeclarationParameters
argument order; projection constants and invalid impl-associated alias domains
remain distinct pending/NotApplicable outcomes.

New Q85 seven-core-trait observations and Q87 actual GAT [Self, region]/impl-alias
NotApplicable observations were source-reviewed in existing unchanged-source
controls. They are not root entry or negative grammar support. Negative parser
activation and its two controls remain held. Final frozen hashes, real callback
coverage/currentness, root response, affected component/runtime terminals and
source/HEAD/test/CLI/separate-digester guards are required before new evidence
claims. None is established by this handoff alone.

## V513 frozen source review and failed compiler terminal

This review binds the attempted candidate at HEAD
`3d481c9d4eaca5326b23b9a30c1c59b7f7afcd0a`, 8,720 guarded inputs,
source fingerprint
`7bb5efdf1980e4762d4fe7e0c66b5c5d250a702e71661746f4954413627de994`.
The final control additions also inspect two authentic primitive PartialEq impl
null-clause receipts, generic impl ClauseParameters pending, actual ordinary
generated trait flags, and nine written Discriminant output bounds. These are
source-reviewed control intentions; no control body executed in V513.

The direct-checkout Rust1.99 offline locked library no-run build terminated 101:
E0433 at source_alias_layouts.rs:169 (`traits::member_index` module path), and
E0277 at source_impl_semantics.rs:506 (PreparedScopeImplHeaders does not implement
StableEncode). CLI and digester builds, all six active controls and all four
runtime cases remained unexecuted. Both negative controls remained held and
unexecuted. Compiler failure establishes no new component or runtime acceptance.

The bootstrap `--verify-upstream` audit terminated 0: 234 retained original
files, 4,085,867 source bytes, zero original Rust changes, verified pinned Git
objects. The default-bound source audit terminated 0. Actual terminals and
before/after-build/after-runtime guards are retained in
`artifacts/validation-current-v513`. Closing guards terminated 0 with identical
source fingerprint and HEAD, zero source changes, and unchanged existing V512
CLI/digester binaries. They are explicitly old, not V513 builds. Separate
digester guards also match. No validation process remains active; source/target
ownership was returned before any repair.

The V514 renewed source freeze retains the same HEAD and 8,720 inputs, with
fingerprint `1ff6d8be1242d77b67a888276b41836cf5319ea62f043cebecdaaf208cb85b70`.
The captured manifest delta contains exactly source_alias_layouts.rs and
source_impl_semantics.rs. Separate source inspection confirms only the module
path was corrected to frontend::member_index and the retained environment uses
its existing stable fingerprint API. The per-query/identity/closure review
above therefore applies to this candidate as well; compiler and execution
terminals remain separate evidence. V513's two source audits are retained at
their original attempt: neither audited policy nor original/provenance input
changed in the V514 delta.

## V514 actual terminals and bounded acceptance

The library no-run, CLI and source-digester builds terminated 0 (Cargo-reported
45.40s, 47.89s and 0.70s respectively). The actual Cargo JSON selected test
binary has SHA256
`8a9844a87737b538dba44804bc84e884dead0db4314218c551d5eb8922a230f7`;
CLI `57a7153a5225f34e65243eba2846ab0a2d4c43bd9c49785c40c5edb75d358268`;
fresh digester
`de0973f48eae8998ab5998fa15f00b653f05dd6bb2906ca2d03f7afa58f86b2f`.

All six exact active selectors in
`2026-10-20-ahead-preparation-controls.json` terminated 0, one selected body
each, with per-control source/HEAD/test-binary guards unchanged. Actual parsed
observations in `artifacts/validation-current-v514/observed-control-counts.json`:

| Control | Seconds | Completed observations |
| --- | ---: | --- |
| Projected discr source requirement/root input | 32.166 | 12 worlds; 36 own clauses, 12 projected subjects |
| Associated output/default bounds | 30.196 | 12 worlds; 0 own, 12 output, 12 default recipes |
| GAT parent instantiation/layouts | 34.760 | 12 worlds; 24 parent, 12 own clauses; 24 semantic declarations |
| Own/hierarchy HRTB demand | 29.746 | 12 worlds; own ready-empty, parent explicitly pending in all 12 |
| Unchanged associated constructor | 47.807 | 12 worlds; 264 operations, all completed |
| Constructor type arguments/context | 24.550 | 4 edition runs containing the origin controls; 1,624 operations, all completed |

The extended source assertions for Q85/Q86/Q87, authentic primitive candidate
metadata and nine Discriminant output bounds passed within these controls.
This exercises metadata/encoding and currentness, not solver root execution.
The two negative-grammar controls remained explicitly held and unexecuted.

The separate unchanged owning-2015 runtime batch terminated 1, selected batch
false, with unchanged imported originals/manifest/binary. Its report is
`/private/tmp/lrr-v514-ahead-preparation-runtime/report.json`:

- assoc-type-in-struct-constructor.rs, generic-derived-type.rs and
  empty-generic-brackets-equiv.rs each passed all 13 runtime-contract checks.
  Generic-derived guest stdout is `10\n10\n`; the other two guest streams are
  empty. Diagnostic/compiler-tooling contracts remain unassessed.
- repr128.rs remains an interpreter failure, exit 2, executor
  `LRR-E-OP-TYPE`: `operator requires supported concrete primitive operand
  descriptors`. It has no runtime-contract check acceptance. This is the same
  previously observed failure, not a new root-inference result.

There are zero new stock runtime gains and no complete fixture, full suite,
JIT, factory or milestone closure. Reproducible commands and actual terminals
are retained in the normal V514 artifact directory and the durable combined
V513/V514 evidence capsule. No raw-policy audit was rerun merely for these two
mechanical source fixes; the retained V513 audit source inputs are unchanged.

After-build and after-runtime guards terminated 0 against the exact V514
fingerprint/HEAD above, 8,720 inputs and zero source changes. The test/CLI hashes
match the fresh build; the separate fresh-digester hash also matches. Every
validation process is terminal, and source/target ownership was explicitly
returned before any subsequent source repair. The decisive next common-engine
work remains the authenticated explicit-implied/elaboration closure and scoped
entry certificate described above.
