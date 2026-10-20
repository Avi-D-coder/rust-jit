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

## Independent query-domain and reached-input review after V516

Read-only production review baseline:
`da1cdc0c3fa3aa4872ba4e4a7afaabe8fd974d42`. Source is mutable during this
review; this section is a design/source handoff, not a new frozen-source or
execution receipt. No Cargo, compiler, control or runtime command was run.
V516 evidence stays attached to its original fingerprint. Final integration
requires a renewed exact-source review and the separately authorized validation.

The changed common entry points are PreparedProvider::{publish,ready}, its
SourceFacts implementation, query_facts::QueryFact, root_variables::RootVariables,
the new root_input transport, engine/run_call_inputs::ReachedCallInputs, and
source_impl_outlives::PreparedSourceImplOutlives. The new packet is inactive:
run.rs has only module registration and an anchor before existing fallback.
There is no measured upstream root or runtime improvement from these additions.

### Validators that survive independent availability

`QueryFact::Known(None/false/[])` is a semantic answer. Unprepared and
NotApplicable are not answers. Callback require() may signal an adapter
invariant fault, but must never become the mechanism for discovering missing
source data under the unchanged host-abort policy. Admission must reject the
missing demanded domain before entry. The historical coarse ready() and its
same-Cx evaluate_prepared API remain insufficient for sparse root admission;
NotApplicable being classified as prepared does not authorize a query outside
its genuine domain.

| Published domain | Required retained validation |
| --- | --- |
| Definition and generics | Actual DefinitionReceipt source/header/context/parse/generic declaration, world/locality and owning profile; require_generic_value equality with SourceGenericLayouts including source parameter namespace/order, inherited count and const parameter Types. Counts alone do not suffice. |
| Impl identity/head | Actual catalog classification and raw-to-canonical join, source scope/range/header identity, unique translated identities and actual trait membership. |
| Candidate coverage | Authentic complete trait bucket, all relevant constructor and blanket impls, pinned simplify_type classification, duplicate/order checks and exact catalog comparison. A flags-only trait row must not create an empty bucket. |
| Nominal metadata | Actual schema kind/name/variant and field order, flattened field-Type agreement and authentic enum representation. model::AdtDefinition still reads one complete AdtInfo even for is_enum/discriminant queries; a sparse skeleton is not sufficient. |
| Language identity | Independently known role lookup and per-definition role/known absence, same-world actual source kind and parent joins, consistent forward/reverse mapping. A missing map key must not become false/None. |
| Associated lists/leaves | Each published list independently validates its actual members, type roles, world/parent/order; requested leaves validate actual member identity, same-trait containers, defining/finalizing ancestry and source parent/name. Unpublished unrelated leaves may remain unavailable. |
| Specialization | Known ancestry must be complete for the supported query domain, or use a genuine per-pair answer. Missing membership in an unpublished relation is not a known false result. |

The draft addresses candidate seeding and language absence: only known
implementation coverage seeds a bucket, and callbacks require an actual
per-definition classification entry. One further concrete review finding was
sent to the lead: associated_types validation was nested under a known
associated_items list. Independently publishing associated_types must validate
that list without demanding unrelated associated function/constant coverage.
The same independence rule applies to every known list or optional value.

### Actual root and preparation capability

Same Cx is not inference ownership: two SolverContexts in one Cx can both
allocate integer variable zero. RootVariables checks its exact owner while
encoding; the final root capability must retain that owner through evaluation
and export, together with the exact encoded goal, caller ParamEnv, typing mode,
site and registered inference-variable families. It must not accept replacement
goal/environment/delegate arguments after admission. A private fresh attempt
or consuming borrowed session around the existing upstream entry is sufficient;
no new canonicalizer, transaction engine or cycle scheduler is authorized.

The caller environment comes from authentic caller own/parent clause receipts
and actual substitutions. A missing GenericEnv is not proof of an empty
ParamEnv. Callee requirements remain goals: inserting the target projected
PartialEq obligation into assumptions can prove itself and can shadow impl
candidates (pinned solve/assembly/mod.rs ParamEnv candidate handling).

Source preparation must close every callback family actually reachable from
the ground root, caller environment, substitutions and all relevant candidate
heads. It need not complete unrelated function signatures, coroutine data,
CTFE/transmute or dyn domains when actual Types/mode cannot reach them. In
pinned trait_goals.rs::consider_impl_candidate, genuine impl head, polarity,
defaultness, generics, clauses_of and impl_super_outlives are immediate demands;
reflection genericity is conditional on reflection mode. Coinductiveness,
marker and candidate-preference flags have separate real callers. Rigid
projections/opaque Types can additionally demand item-self bounds; do not reuse
a ground DiscriminantKind certificate for arbitrary symbolic alias roots.

Tables/indexes are immutable before the first live solver/cache use. Do not
publish extra source facts into a context after an attempted root. The raw
proof-tree root hook also needs the admitted closure; its current context/index
checks are not a separate permission for arbitrary canonical inputs.

Pinned GoalEvaluation.goal is the pre-response input (solve/mod.rs documents
this). Resolve the original RootVariables after upstream response application;
retain ambiguity/external-constraint semantics. Export owned results and check
actual execution authority before changing VM hints/dictionaries. This review
does not claim that any such root consumer is installed.

### Bounded worker review

ReachedCallInputs captures actual selected call/source/signature/body handles,
explicit argument relations and live literal handles; it does not unify them.
Literal deduplication follows validated live kind/handle identity, not equal
spelling or numeric payload. CallerSourceContext explicitly does not certify
caller clauses. The initially added global source_revision gate was removed
after review because unrelated publication should not invalidate relevant
prepared input guards. Existing SourceCallable::revalidate remains broader
legacy debt. The final reviewed packet SHA256 is
`7f4746815d24b0d822eebe632fd495d63347dc57d83fe32189f7f6aa538cd019`.

The packet's require_current checks retained inputs, not a full attempt/commit
certificate. Ordinary CallIdentity is site-based; repeated calls or another
activation can reproduce pc/body/value observations. Future activation must
bind the actual run/frame/call attempt and current result expectation,
especially across suspension. These are explicit activation holds. Keeping
the inactive packet adds no legacy-path capture allocations.

The source_impl_outlives draft retains authentic Q86 head/catalog, Q88 Implied
roots, per-ID Q76 classification, merged bindings and actual configuration. Its
ClauseKind inspection preserves HostEffect, escaping-alias variance and FnDef
binder requirements before the pinned iterator is entered. A finite DefId
metadata worklist is neither recursive-query completion nor semantic
instantiation/cycle proof; empty requirements is not arbitrary provider/root
permission. The parent must admit the genuine pinned elaborator callback
closure. No separate elaborator or source query inside a callback is introduced.

### Eight recurring architecture modes

1. No assertion evaluator is added: the inactive packet attaches to the common
   reached CallSeal path. Actual root/runtime use remains unexecuted.
2. No scalar/break-only macro executor is added; source identity is retained
   through existing body/source joins. Generated execution evidence is pending.
3. Equal/Coerce relations and inference families are transported without a
   local unifier or method-name/numeric policy. Upstream inference is still held.
4. There is no new generated-source rejection; genuine generated source/header
   receipts remain required through the common catalog. Origin evidence awaits
   the final affected controls.
5. The changed modules neither lower cold bodies nor add pattern machinery.
   Candidate preparation must include relevant semantic candidates without
   eagerly preparing their executable bodies.
6. Source plans remain memoized immutable handles; future callbacks read frozen
   tables. Packet capture is not installed, so no repeated-call performance
   claim is established.
7. Candidate/catalog validation is cold preparation work. No new hot full-world
   scan is authorized. The removed packet revision gate avoids one new source
   of unrelated invalidation; relevant positive/negative buckets still govern.
8. BodyIndex and source plans are retained by Arc, not deep-copied for packet
   capture. The real source-range join remains preparation work; no installed
   repeated whole-file search is evidenced.

The actual root, complete factory, guarded exact-once result commit, all-origin
runtime matrix, obsolete-route removal and interpreter/JIT gates remain open.

### V517 frozen-source review delta

The compile-only candidate is frozen at HEAD
`da1cdc0c3fa3aa4872ba4e4a7afaabe8fd974d42`, fingerprint
`9354775bd2f6240d3886c63d4df84d3129e2ee3f29ef89be44775924e215f154`,
8,729 inputs. The actual before guard matches this authorization. Compared with
the mutable review above, the lead removed ReadyProvider/evaluate_prepared and
retained only metadata-only PreparedIndexSnapshot. The raw proof callback now
requires an inactive root-session guard before its context/index checks. This
is withdrawal of an unbound entry prototype, not installation of root admission.

The independently published associated_types list now validates each retained
declaration's world, uniqueness, source alias role and actual parent even when
associated_items is unavailable. Candidate buckets remain dependent on known
complete implementation coverage, and language classification requires a real
per-definition entry. These inspected deltas address the concrete split findings
above; source producers still own semantic completeness of answers they publish.

The all-eight review above applies to this exact candidate with these deltas.
Packet and Q89 outlives inputs remain inactive. A library no-run build, CLI and
digester builds are the only authorized execution for V517. No test bodies,
runtime fixtures, root inference, feature closure or runtime gain are implied.

V517 library compilation terminated 101. The actual errors are E0432 at
source_impl_outlives.rs:36 (unavailable rustc_type_ir::IntoKind import) and four
E0603 diagnostics at line 38 (private rustc_type_ir::visit imports). The bounded
source review did not establish those imports' compatibility; the actual
compiler result supersedes the earlier absence of a mechanical finding.
CLI/digester builds were not started. No test body, runtime fixture or root ran.

Reproduction is the maintained direct-checkout command:
`cargo +1.99.0 test --offline --locked -p lrr --lib --no-run --message-format=json`.
Actual JSON/log/terminal and guard outputs are in
`artifacts/validation-current-v517`. Both closing standard guards terminated 0:
same authorized HEAD/fingerprint, 8,729 inputs and zero source changes. The CLI
is explicitly the old V516 binary, SHA256
`6eeff255fc80382ec39ef978613e2bb759c6b7475bee8e18418073f5e2ad5d7d`;
the separate old digester guard matches
`aa585ff08d146d45daa76e1bf490d649744566fa514157caea4a19d1c7aae1f4`.
The previous test binary also remains unchanged. Every process is terminal and
source/target ownership was returned before any repair. This failed compile
establishes no new component, runtime, root, full-fixture or milestone acceptance.

V518 retains the same HEAD and 8,729 inputs, fingerprint
`9cbf650493469db1a8d5c9110a91c6f2d81e0b88879fe94cd2bd06c92c5c3eb2`.
Its manifest delta is only source_impl_outlives.rs. Source inspection confirms
the import repair uses public crate-root visitor reexports and the existing
inherent trait import; production semantics and the eight-mode review above
are unaffected by this mechanical change.

The V518 library no-run command terminated 101 with E0271/E0599 at
prepared.rs:1235 (an extra copied() on the inherent by-value slice iterator) and
E0308 at line 1240 (DefinitionId passed where the map expects a reference).
No dependent CLI/digester build or test/runtime execution occurred. Both closing
standard guards terminated 0 against the exact V518 fingerprint/HEAD with zero
changed inputs. Old CLI/digester hashes remain the V516 hashes above, explicitly
not fresh V518 builds. Compact errors, actual terminal, source delta and guards
are in artifacts/validation-current-v518. Every process is terminal and
source/target ownership was returned before repair. No acceptance gain is claimed.

V519 is frozen at the same HEAD and 8,729 inputs, fingerprint
`9c0456da588978bc8b284223e15bbf41fef6284eb002189c80882daec5c8fc6c`.
Its manifest delta from V518 contains only prepared.rs; inspection confirms
explicit standard slice iterators in the known associated-list checks, retaining
the same identity/role/parent validation. This mechanical iterator repair does
not change the all-eight source-review conclusions above. The withdrawn
unbound root prototype remains absent, the proof-root guard remains inactive,
and packet/Q89 consumer/root admission remain held. Compilation is a separate
pending observation until its actual terminal; no test/runtime is authorized
by this compile-only freeze.

V519 actual build terminals are all 0: library no-run 52.63s, CLI 44.59s and
digester 0.75s (Cargo-reported). Actual Cargo JSON selects the test binary with
SHA256 `45ddd6e5132618349000a6490f8a6ab74285cf07c7f491be881653391ab02c46`;
fresh CLI is `771e06f24865c7c137d154c01b7b6c7f68ca12ce9eb21460cb8209e65e732dae`;
fresh digester is `271816cec27a8430df20292f7716b221afe3598f53205d594893f0906f48f8a0`.
Both standard closing guards terminated 0 at the exact V519 fingerprint/HEAD,
8,729 inputs and zero source changes; test/CLI hashes match after-build. The
separate closing digester guard also matches. Actual commands, terminals,
source delta and binary guards are retained in artifacts/validation-current-v519.
All processes are terminal and source/target ownership was returned.

No control/test body or runtime fixture executed in V519. This is successful
host compilation of the held metadata/input slice, not activation or acceptance
of a source factory, root session, packet commit, complete elaboration closure,
runtime feature or milestone. V517/V518 failures remain preserved at their own
fingerprints. A subsequent test selection requires its actual changed-source
justification; no unchanged eleven-control batch was automatically replayed.

## Pinned impl-super-outlives consumer and focused control draft

After V519 ownership return the source is mutable again. This review covers the
lead's PreparedProvider::prepare_impl_super_outlives and the one newly assigned
CLI control; its final fingerprint remains pending. No Cargo or test/runtime
execution occurred during this draft.

The pinned collect/item_bounds.rs:530-542 algorithm maps the actual impl trait
head through rustc_type_ir::elaborate::elaborate and retains only TypeOutlives
and RegionOutlives. The new consumer follows that algorithm in the same
PreparedProvider: genuine Q89 source plans encode before entry, their typed
callback requirements must be empty, and a private preparation scope supplies
immutable interned implied-clause lists and per-definition Sized classifications.
The scope does not publish final facts or enable a root, and callbacks perform
no DB/source query. Bound substitution, deduplication, cycle traversal and
outlives decomposition remain with the retained upstream algorithms.

For the actual nonescaping PartialEq/PointeeSized graph, elaborate.rs:129-224
uses is_trait_lang_item(id,Sized) before polarity filtering, and
explicit_implied_clauses_of(id) for positive non-Sized trait clauses. Context
anonymization, Clause::instantiate_supertrait, EarlyBinder/ArgFolder folds,
interning and component reconstruction are local IR operations. Actual argument
compatibility/layouts are still required before encoding. HostEffect adds
explicit_implied_const_bounds; an escaping alias in a TypeOutlives subject adds
opt_alias_variances. Q89 retains those requirements and its conservative bound
input/FnDef holds before algorithm entry. item_self_bounds belongs to a different
outlives helper and is not called directly by this elaborator path. Candidate
indexes, normalization, ADT fields and full clauses_of are not demands of this
bounded computation alone.

Debug paths were inspected separately: model Ty/Clause/Predicate/List wrappers,
Interned, DefinitionId/SourceParameter and Context formatting are structural;
printing.rs::TraitRef prints retained definition/args. Pinned trace/panic
formatting therefore adds no source query family for these inputs. This is a
source conclusion, not a successful debug-enabled execution receipt.

The owned new control is appended only to
cli/preentry_loader_tests/upstream_projection_types.rs; the preexisting prefix
was preserved and only the new function was formatted. It uses unchanged
official fixtures and their existing configured source-world harness:

- repr128.rs supplies actual i128/u128 PartialEq candidate declarations selected
  from the authentic complete catalog, never a caller-supplied impl list. Their
  expected empty filtered output must come from the real pinned iterator.
- builtin-superkinds-self-type.rs supplies Foo: Send + Sized + 'static and its
  actual generic impl. It must yield precisely the real impl T parameter's
  TypeOutlives-to-Static row, preventing all-empty vacuous evidence.
- streaming_iterator.rs supplies the independently captured new member and its
  real StreamEnumerate inherent parent. Q89 must classify that source domain
  NotApplicable. This is a source-domain hold observation; the elaborator is
  not invoked for the inherent impl.

The single selector is tracked in 2026-10-20-impl-super-outlives-controls.json.
Its intended assertions cover four editions and original/user emit/wrapper
sources, exact canonical IDs/source fingerprints, static ParamKey identity,
Q89 memo reuse/currentness, no signature/body/thunk/pattern preparation, epoch
revocation followed by source withdrawal. Withdrawal after epoch is not an
independent fresh-source-withdrawal control. Intended counts (36 worlds, 36
elaborations, 12 outlives and 12 typed domain holds) are unexecuted expectations,
not measurements. No unchanged original is edited or replaced by a fragment.

All eight review modes remain scoped as follows: (1)-(3) no assertion evaluator,
macro scalar executor, numeric rule or local solver is added; (4) the same actual
source joins serve every origin; (5) only source/header metadata is demanded and
executable bodies stay cold by assertion; (6) memo plans/IR inputs are retained
and the actual iterator runs only on demanded preparation; (7) relevant query
roots/guards govern currentness, with no new installed global scan; (8) source
plans and bodies retain Arcs rather than deep copies. These are source-reviewed
claims with pending control execution, not proof of the entire runtime matrix.
The actual root/caller ParamEnv/inference owner/guarded commit and obsolete
runtime fallback removal remain held. On eventual real activation, unchanged
repr128 is the primary runtime witness; extra constructor replay requires an
actual change to their active shared CallSeal/commit path.

### V520/V521 frozen review and failed attempt

V520 froze HEAD da1cdc0c3fa3aa4872ba4e4a7afaabe8fd974d42, 8,732 inputs,
fingerprint `f4d77ef0b0d4261a6529385ddd994c9a920e6442d9e4eaa40f9f02009447cb02`.
The library no-run build terminated 101: E0308 source_caller_clauses.rs:126
(boxed configuration reference), E0596 line 145 (mutable input lookup through
an immutable DB reference), and E0277 prepared.rs:1045 (EarlyBinder needs a
foldable retained List rather than its borrowed slice). CLI/digester builds,
the new control and both runtime fixtures were unexecuted. Closing source/HEAD
guards terminated 0, zero changed inputs; old V519 CLI/digester hashes stayed
unchanged and are explicitly not fresh builds. Every process was terminal and
ownership returned before repair. Actual logs/terminal/guards are retained in
artifacts/validation-current-v520.

V521 freezes the same HEAD and 8,732 inputs at fingerprint
`2dcc5fd54cd54ff3cf2ac812e3b7cd0e7bf4f883414d9a659c7b9300abef2a0c`.
Its manifest delta contains only prepared.rs and source_caller_clauses.rs;
inspection confirms Box::as_ref, exact published-input fingerprint currentness
alongside retained memo guards, and EarlyBinder over the foldable List mapped
to its arena slice. These mechanical repairs retain the reviewed semantic
boundaries and all-eight conclusions. V521 execution remains separately pending.

The new Q90 source_adt_facts was also independently read: Q75/receipt/schema and
enum-representation joins, optional field-Type demand, actual shape proof and
whole-memo/auxiliary currentness are retained. declaration_info leaves all
unproduced destructor/Copy/sizedness/language-related semantics Unprepared;
neither a declaration nor missing map becomes a false/empty semantic answer.
The source/type/configuration guard is exact and shared Arc payloads preserve
the owned source proof. This is source review only: the focused Q89 control
does not exercise Q90 publication, callback leaves or field materialization.

The caller null branch was checked against pinned rustc_ty_utils::param_env
(ty.rs:150-206), clauses_of.rs and outlives/mod.rs. An authentic standalone
ordinary Fn with complete zero parameter count, no parent/written/feature
clauses, generic_const_exprs disabled and nonconditional constness can establish
empty assumptions. It excludes trait Self, default associated-method RPITIT,
const-effect and inferred-function branches. The retained explicit Q81 hierarchy
is separately unavailable for broader callers, not promoted to complete clauses.
Configuration is bound to the actual provider as well as retained source input.

The root_response module is portable result transport only. Same-owner original
RootVariables and original family-matching keys represent unresolved variables;
fresh unknown variables remain Pending. Reverse definition/generic/Q77 joins
authenticate supported Types, and unsupported rigid aliases/regions/constants
remain typed holds. Solver Certainty/external constraints, runtime activation,
current result expectation and exact-once commit are still the future consumer's
authority; no response DTO certifies them. No caller/export execution acceptance
is inferred from a neighboring elaborator control or successful host compilation.

An additional shared-source prerequisite was found while V521 remained frozen:
pinned hir_ty_lowering/mod.rs:983-1002 treats a genuine positive PointeeSized
bound as transient and discards its middle-IR clause. Current common source
transport still encodes that written bound as Implemented. The new control's
implied_traits >= 2 assertion reflects that current transport and must not become
a future requirement that PointeeSized remain an encoded trait clause. Remove
or revise that cardinality expectation with the common lowering repair, retaining
the real head/source/output checks. No source change was made during validation.
Executing the real upstream iterator over these current inputs is component
evidence; it cannot certify exact compiler-query input completeness. The common
bound-lowering repair remains a prerequisite to root/factory activation.

### V521 actual terminals and bounded evidence

Library no-run, CLI and digester builds all terminated 0 (Cargo 45.51s, 38.01s,
0.67s). Actual Cargo JSON selected test SHA256
`164427793600321745cbcccc5ef7089e36bebcc1e7f3d1130428e6942ecfedad`;
CLI `109864d16d0da4346135e6d6e1c54d1e248b83f8c7e7abafd156e244f9aa029b`;
fresh digester
`5b5ddb2c70ad2cc69868becefcb32fe3c040ecb5376bd388b1e56c2b8972786c`.

Only the exact selector in 2026-10-20-impl-super-outlives-controls.json executed.
It terminated 0 in 97.624s, exactly one test body, with unchanged source/HEAD/test
binary guards. Actual parsed rows (not inferred from final assertions) record
36 source worlds: 36 elaborations, 12 filtered outlives rows and 12 typed
inherent-domain holds. The repr128 worlds execute the real upstream iterator
for both primitive PartialEq impls; Foo worlds return the actual T:'static row;
streaming worlds observe NotApplicable without invoking that iterator. All four
editions and three origins completed, with memo/cold/currentness/epoch assertions.
Withdrawal assertions follow epoch revocation, not an independent withdrawal.
Actual per-world observations are in
artifacts/validation-current-v521/observed-control-counts.json. The source-input
PointeeSized defect above still limits compiler-query completeness; this pass
does not close that prerequisite.

The selected runtime batch terminated 1. Both originals ran in owning Rust 2015
with embedded facade and native_execution=false. The associated constructor
passed all 13 runtime-contract checks; repr128 retained exit 2, executor
LRR-E-OP-TYPE, `operator requires supported concrete primitive operand descriptors`,
with no runtime-check acceptance. Imported originals, manifest and binary stayed
unchanged. The report is
/private/tmp/lrr-v521-impl-outlives-runtime/report.json; compact argv/capture/
diagnostic/check evidence is in artifacts/validation-current-v521/runtime-summary.json.
There are zero new stock gains; diagnostic/tooling/full-suite/JIT remain unassessed
or held. Q90, caller null-environment encoding and response export have only
source review/host compilation here, with no executed production observation.

After-build and closing after-runtime guards terminated 0 at the exact V521
fingerprint/HEAD, 8,732 inputs and zero source changes. Actual test/CLI hashes
match after-build, and the separate fresh digester closing hash also matches.
Every Cargo/control/runtime process is terminal. Source and target ownership
were explicitly returned before further repairs; no automatic broad replay ran.

Reproduce builds using the maintained direct Rust1.99 offline/locked commands
above, then run tools/validation/run_controls.py against the one-selector tracked
selection. The runtime command is tools/rustc_checkout_runtime.py with
LRR_EMBEDDED_FACADE=1, --lrr target/debug/lrr,
--source-digester target/debug/examples/lrr_source_digests, a new --report-dir,
--timeout 240, and only --case tests/ui/enum-discriminant/repr128.rs and
--case tests/ui/associated-types/assoc-type-in-struct-constructor.rs. Raw terminals,
actual command receipts and guards remain in artifacts/validation-current-v521.

## Common transient emission review after checkpoint 7ea94b8

Source baseline `7ea94b8f6b3ff5b9a0793a414e2040aa2828d0ff`. The lead owns
ordinary Q80/impl/Q88 consumer integration; the metadata worker owns the new
predicate_emission.rs policy. This reviewer owns only the existing
upstream_projection_types.rs control edits and these review/selection documents.
No Cargo, control or runtime command ran while this source was mutable. The
final integrated fingerprint remains pending global freeze.

The exact pinned policy is hir_ty_lowering/mod.rs:983-1002: positive authentic
PointeeSized selects a temporary output vector, normal lowering continues, and
the completed vector is discarded. This is separate from source retention and
implicit/default-bound policy. It grants no permission to remove unavailable
arguments, negative/qualified bounds or written Pending rows.

PreparedClauseEmission was independently read. prepare() checks its actual
catalog/current Arc, gathers distinct genuine positive trait IDs and retains
their exact Q76 query values. Actual pointee_sized identity is transient;
another role or authentic Known(None) emits; missing/unsupported identity stays
typed Pending. apply_encoded verifies real positive clause identity/world and
filters only an already encoded output, preserving its span when emitted.
Stable encoding includes catalog/Q76 fingerprints and explicit outcome/detail
tags; the enclosing ordinary memo retains dependency/currentness ownership.
No new complete-world language registry or missing-map false answer is added.

The lead's common encode_recipe performs require/binding and full actual
source_terms::encode_predicate before the emission decision. Q80, ordinary
impl predicates, Q88 and the Q84 observation use that boundary. Q88 retains
raw source/default rows while removing only authenticated transient positive
dependency edges; unknown classification preserves Pending and conservative
dependencies. Q89 follows that semantic graph. The sidecar is prepared after
SourceBindings, which does not recursively read it, avoiding a Q76/Q75 binding
cycle. No source lookup occurs inside the pure encoder or solver callbacks.
The existing Q88 observer and new Q80 observer were read: they call production
encode, validate actual IDs/Self/arguments/spans/order and return owned counts
or IDs. Their expected-row filtering is observation, not another encoder.

The wrong implied_traits >= 2 assertion was removed from the existing single
control. Its appended private helper uses unchanged repr128 worlds to check
Q78's two actual Self/Rhs PointeeSized source rows, the retained Q75 colon span,
Q84 actual sizedness identity and authentic default_inputs. Applying that real
default view still suppresses implicit sizedness. Q80 emits no clauses; both
Q88 filters preserve the written row but emit none and retain no semantic edge.
The unchanged core-marker-name-shadowing-issue-2284.rs supplies the actual user
Send and its free f<T:Send>, independently distinguished from Send::f. Q76 must
report Known(None), and production Q80 must retain/emit the user trait bound.
No fixture source or miniature language substitute is introduced.

The same one exact selector now contains the prior 36 elaboration/domain
worlds plus 24 emission/shadow worlds. These are intended 60 configured worlds,
not measured outcomes. The new helper's expected counters are 24 written rows,
24 filtered transient rows and 12 ordinary shadow rows. Core PartialEq remains
ordinary facade source in every root-origin world; the Send source/emission is
directly original/emit/wrapper. Generated transient-language declarations and
dyn runtime lowering remain unexecuted requirements. Guards cover memo reuse,
cold executable preparation and epoch revocation followed by withdrawal, not
independent fresh-source withdrawal.

The new tracked selection is 2026-10-20-predicate-emission-controls.json;
historical V521 selection remains unchanged. Only unchanged repr128 is planned
for runtime replay. The common CallSeal packet remains inactive and constructor
execution is unchanged. Explicit/implicit Sized-order controls contain no
authentic PointeeSized; the new filter preserves every Emit row's order, so no
automatic eleven-control or unrelated ordering replay is selected.

All eight modes were separately reviewed for these changes: (1) no assertion
compiler, (2) no macro scalar executor, (3) no contextual method/numeric rule,
(4) actual source/header/Q76 joins remain common across origins, (5) no body
lowering or pattern machinery, (6) one immutable policy per ordinary memo and
pure encoding rather than hot re-planning, (7) relevant per-ID Q76 dependencies
rather than namespace census or independent revision gate, and (8) retained
Arcs without body clones/whole-file installed searches. No performance claim or
full origin/runtime acceptance follows from source review.

The new caller_application was also read independently: actual complete zero
parameters, both real private GenericEnv binding tables empty, source/header/
context/layout and full signature equality plus retained guards/Arc handles
support its narrow source-application receipt. Pure descriptor matching is not
currentness or activation. The final consumer still needs actual active handles,
attempt/run identity and current result authority; inherited broad callable
revalidation remains named legacy debt. Caller, root, response-export and
guarded commit execution remain unmeasured by this selected control.

This review is bound to the V522 freeze: HEAD
`7ea94b8f6b3ff5b9a0793a414e2040aa2828d0ff`, fingerprint
`7ebcb20b8dc073e1dc76ce077af5d965022bda3555b8e97bb5a397a58d4b53f9`,
8,734 inputs. The actual before guard matches. The owned test source is
`5c79b7267e24d04b65357d747f059285c895fccf977a38da032b9d9de98b6b56`.
All-eight conclusions above apply to this exact candidate; execution remains
separately pending until its actual terminals. The only selected runtime
witness is unchanged repr128; no constructor or eleven-control replay is planned.

V522 library compilation terminated 101 on this reviewer's new control,
upstream_projection_types.rs:187: E0308, ast_adapter::range expects TextRange
rather than &SyntaxNode. No CLI/digester build, test body or runtime ran. Both
closing guards terminated 0 against its exact fingerprint/HEAD with zero changed
inputs; old V521 test/CLI/digester stayed unchanged, explicitly fresh=false.
All processes were terminal and ownership returned before the authorized repair.
The failed receipt was originally captured in artifacts/validation-current-v522.
That disposable raw output is no longer available; the retained V522/V523
capsule linked below records the observed failure and its limitations.

V523 changes only that test call to syntax().text_range(), with no other source
change. The new before fingerprint is
`da7fc5fbb7d486505d275fd78f9b2da67e91bf1d0c52c081f1ae25690488cbab`,
8,734 inputs, same HEAD. The owned test file SHA256 is
`846f4d99143e3cc13948ae9234d8168bede22d57d1729cde02d8f3054ac0f8cf`.
The all-eight source review is unchanged by this span API correction and binds
to V523 as well. Its builds/control/runtime remain separately pending; the
V522 failure establishes no component or runtime acceptance.

### V523 actual component and runtime terminals

All three builds terminated 0: library no-run 41.50s, CLI 33.74s, digester
0.80s (Cargo). Actual selected test SHA256 is
`635ada623231b7466b4b30b3cf5c62b763492785cb59347bd4bca4d3980132c2`;
CLI `6993314f8e20a59e2c98c7114b321b8f5f2f1c9b2cdfaa342f210ccfd28d152b`;
fresh digester
`b3dbd46725dae700f93f3c24a0ac1a90b014f0389ff266eb92d33f46241e6ff5`.

Only the one exact selector in predicate-emission-controls.json executed. It
terminated 0 in 158.760s, one test body, unchanged input/HEAD/test guards.
Actual parsed observations record 60 configured source worlds, separately:

| Observation | Actual completed evidence |
| --- | --- |
| Retained elaborator/domain cases | 36 worlds; 36 pinned elaborations, 12 outlives rows, 12 typed inherent-domain holds. |
| Ordinary core transient source in repr128 worlds | 12 worlds; 24 retained written rows, 24 filtered transient rows, zero emitted transient clauses; Q75/Q78/Q84 and production Q80/Q88 assertions passed. |
| Actual user Send source/emission | 12 worlds; 12 ordinary bounds retained and emitted under genuine Q76 Known(None), across four editions and original/emit/wrapper. |

Actual per-world rows and grouped counters were captured during the run in
artifacts/validation-current-v523/observed-control-counts.json. That raw file
was subsequently removed after terminal closure. The table above preserves
the observed grouped counts; it is not a fresh parse of currently available
raw rows. The removed
implied-graph cardinality assumption is not part of this pass. These are actual
component observations, not the earlier intended counts. Core transient bounds
themselves remain ordinary facade source; a generated authentic transient bound
and dyn runtime behavior are not directly exercised. Changed ordinary impl and
Q84 item-output encoding call sites have source review but no separate execution
observation here. Epoch-then-withdrawal scope remains as stated above.

Only unchanged repr128 was replayed in owning Rust 2015 with embedded facade.
The batch terminated 1, CLI exit 2, executor LRR-E-OP-TYPE:
`operator requires supported concrete primitive operand descriptors`. It has no
runtime-check acceptance and zero new runtime gain. Imported originals, manifest
and binary stayed unchanged; native_execution=false. Exact argv/capture/source
hash and compact fault were captured in runtime-summary.json; the original
report was /private/tmp/lrr-v523-predicate-emission-runtime/report.json. Both
disposable outputs were subsequently removed after terminal closure. Constructor, broad
control, native/compiler-reference and full-suite replays were not run.

After-build and closing after-runtime guards terminated 0 at exact V523
fingerprint/HEAD, 8,734 inputs and zero changed source files. Actual test/CLI
hashes match after-build and the separate fresh digester closing guard matches.
All Cargo/control/runtime processes are terminal; source/target ownership was
returned before further work. Reproduction uses the maintained build commands,
run_controls.py with 2026-10-20-predicate-emission-controls.json, and the runtime
command above with only --case tests/ui/enum-discriminant/repr128.rs and a new
report directory. The original raw receipts were captured in
artifacts/validation-current-v523 and removed after terminal closure. Current
durable evidence is
[the V522/V523 capsule](../evidence/2026-10-20-v522-v523-predicate-emission.json),
reconstructed from the lead's actual prior terminal/tool observations. It
preserves source/binary identities, observed terminals and grouped counts;
it does not reproduce unavailable raw rows, invent removed-log hashes or
claim a new execution. Exact historical command details that were not retained
remain unknown; maintained commands above are reproduction instructions.
Caller/application/export/Q90 runtime use, source-factory/root admission,
guarded result commit, obsolete fallback removal and interpreter/JIT gates
remain held or unmeasured. This checkpoint is not feature completion.
