# LOCAL-049 / WT05-02 / M020 independent bound provenance review

Read-only review requested by parent during mutable source implementation.
Own this document only; comparison facade/provenance files remain frozen.
No source/tool/fixture edits, test/compiler/Cargo/interpreter invocation or new
agent. This review is semantic source evidence, not terminal validation.

Interim reviewed source fingerprints (SHA256):
- frontend/source_types.rs: 9bea037849e7cd28952ab3548f788860095457fff5a40f5e94883cad174b2b21
- frontend/source_predicates.rs: c6b1f75f3f5448c1ebf417778e8f5a113780acdd6d6de9092134c1b1f201efff
- frontend/declaration_source_predicates.rs: b38ccc994526eec9d2a62e33efb6bdc1313a5f8f6428f6ce400512e58cb47d36
- upstream/source_default_bounds.rs: 65c3b09fd15c9861ca0aa758249a3c3fb00013f71291b37d8faec9282c036a98
- frontend/source_item_bounds.rs: 1a3d48a29b6679ffd977417089a94e8f5f225a50be6f2b590886ef1795d9790c
- upstream/source_trait_clauses.rs: 0ee1268e0f12c656e1018f2159682b875e3c908c449ccfee510bbf601a0e2f16
These identify the interim implementation only. Source is still being repaired;
final conclusions must be rebound to actual final fingerprints below.

Confirmed blocker, sent before freeze: PreparedWrittenBounds::with_default_sets
puts colon clauses before TraitSelf defaults. Exact pinned
hir_ty_lowering/bounds.rs::add_trait_bound inserts at index0, so
`trait Tr: Copy {}` yields Self:MetaSized BEFORE Self:Copy. Additionally
hir_ty_lowering/mod.rs::lower_poly_trait_ref inserts an explicit authenticated
Sized clause at index0 within each lower_bounds invocation. Preserving source
order for `trait Tr: Y + Sized` instead produces the wrong clause/cause order.
The repair must use actual Sized compiler identity and real syntactic bound-list
groups (TraitColon, Parameter(key), Where{predicate span}); globally moving all
Sized predicates ahead of previous independent groups/defaults would also be
wrong. Parent acknowledged and owns the common repair. Empty-colon Inject
alone cannot observe either ordering obligation.

Two existing unchanged official runtime fixtures ready for source observations:
1. tests/ui/traits/default-method/supervtable.rs, Z at line14:
   `trait Z: Y + Sized`. Trait bytes [188,266), Y [197,198), Sized [201,206).
   SHA256 c41669d7f681273bf3ab076b6a16299ee22d14fb1f918fd620fcc87474306179.
   //@ run-pass, no auxiliary/feature directive; actual local Y is simple.
   Pinned immediate expected order [Self:Sized, Self:Y], source spells Y first.
2. tests/ui/union/union-manuallydrop-rpass.rs, ImpliesCopy at line32:
   `trait ImpliesCopy: Copy {}`. Trait [460,486), Copy bound [479,483).
   SHA256 def92f80ff1184837f4dd261c28d77c58153488817c737b781394424509463cf.
   //@ run-pass plus existing union reference directives, no auxiliary.
   Expected [Self:MetaSized at trait span, Self:Copy at actual bound span].
Both imported byte sequences match the official pinned checkout. Neither full
fixture nor source observation was executed by this reviewer. Demand authentic
trait ID/header/clauses only; existing methods/main/union runtime stay cold.

Soundness checks on the interim provenance design: ActualBoundSite records
TraitColon/Parameter/Where/AssociatedOutput during the existing collector, with
actual owner span. Own and filtered callers retain exact row.owner==own_range,
preventing associated-parent bounds from leaking into child own clauses.
PreparedWrittenBounds fields are private; default_inputs carries definition,
actual HeaderRecipe fingerprint, owner span and requested context. Q84 matches
all against the actual selected Q75 declaration before using the view; source
content/site bounds are independently checked. Q78/Q88 build frames from actual
Q75 source parts/context/generics, and Q82 authenticates real Q77 associated
membership/parent/own generic identity before constructing the projection.
No generated scope is switched to an unrelated parser or synthetic owner.

TraitSelf DefaultBoundInputs selects only TraitColon, so Self where predicates
cannot suppress colon-policy defaults. TypeParameter selects actual direct
ParamKey subjects across inline and matching where sites. Unsupported unrelated
rows remain in the full written bundle but cannot poison that selected default
view. Missing selected semantic recipes remain Pending. The unchanged private
noimplicit/auto/Pointee early applicability exits still preserve the caller's
written completeness hold. Q82 AssociatedType inputs select only output bounds,
with default/written/parent candidates retained separately.

prepare_filtered_trait_bounds selects before semantic lowering. All skips
region/lifetime-predicate rows as pinned probe_ty_param_bounds_in_generics does;
Self filters keep actual colon and exact Self where subjects, unwrap parens,
reject qualified/argument-bearing Self and Vec<Self>/Self::Assoc. Self is a
keyword in the authenticated owning trait frame, not a name-resolved alias.
Current SelfOnly and SelfAndAssociatedTypeBounds share lowering only because
nested associated constraints remain AssociatedIdentity pending; they are never
dropped to pretend these filters are generally equivalent. Const/binder and
selected associated rows retain QualifiedBound/HigherRanked/AssociatedIdentity
holds. Alias All type-parameter defaults are explicitly pending, not empty.
Q88 immediate positive dependency descriptors are not transitive closure proof.

prepare_written_rows records sites before lowering and rejects duplicate sites
or any Explicit recipe without an actual site. The private DefaultBoundInputs
iterator's absent-site exclusion therefore cannot silently erase a published
Explicit recipe. Stable encodings retain recipe/site/owner/header/context data;
Q78/Q82 domains are versioned for the changed meaning. Source-only root/current
wrappers remain required; an available recipe vector never certifies all facts.

Eight recurring architecture modes, separate from audit MA identifiers:
1. Separate assertion/expression evaluator: none; this is source metadata only.
2. Literal/break-only macro-output executor: none, ordinary retained AST route.
3. Context-specific method/numeric inference: none, actual parameter/lang IDs.
4. Generated-source rejection: no new origin branch; source parts carry actual
   edition/hygiene/owner identities through the same collector.
5. Duplicated patterns/eager cold-arm checks: no body/pattern/signature demands.
6. Repeated planning: lowering is pure memo preparation, vectors/maps retained;
   selectors and callbacks do not reparse installed operations.
7. Full-world hot scans/unrelated invalidation: no new hot census; existing
   relevant namespace/source query dependencies and guards remain authoritative.
8. Immutable-body copies/whole-file lookup: no BodyIndex access; direct bound
   AST nodes and selected owner ranges, no whole-file node search.
Preparation adds site-map O(n log n) work for selected declaration bounds, not
runtime dispatch overhead. No unsafe, caps, independent solver/elaborator,
normalizer, static rejection or alternate numeric route is introduced. No
performance, runtime acceptance or complete semantic query closure is claimed.

Pinned ordering caveats sent during lead repair: front insertion is repeated
for every encountered explicit Sized, so two Sized bounds at different spans
reverse their relative positions (not a stable Sized/non-Sized partition).
Scratch lower_bounds vectors are fresh per syntactic group. The add_implicit
then add_default_traits sequence also uses front insertion; the already-pinned
Q84 default vector should be prefixed without reordering its own rows. Explicit
clauses use ordered (clause,span) sets, while filtered super/implied values are
vectors: equal clauses at distinct spans must not be deduplicated together.
Relaxed/negative/const/associated constraints must retain their represented or
pending status, not be erased by an ordering pass. MetaSized and PointeeSized
are not explicit Sized: only an authenticated Sized compiler role gets this
special insertion. Parent owns the fix and validation control registration.

Final common-order repair rereview (before executable validation)

The original blocker above is addressed in lead-owned common source at:
- frontend/source_types.rs: 9bea037849e7cd28952ab3548f788860095457fff5a40f5e94883cad174b2b21
- frontend/source_predicates.rs: a6dbdfe9768ae462cca1ea88a078dd1287879940476c5a4537fc5384a6460412
- frontend/declaration_source_predicates.rs: ca42e41ed31c8a98247a15e06f0d14a5bb5e8e00ac4d506c878bc61471914d95
- upstream/source_default_bounds.rs: 65c3b09fd15c9861ca0aa758249a3c3fb00013f71291b37d8faec9282c036a98
- frontend/source_item_bounds.rs: 87869fee708c222735b390d8787f6ce62043a6f01539b36ebc1e828c9ed94197

with_default_sets now emits the actual TraitSelf default vector before colon
rows, parameter defaults after that colon prefix, then remaining actual
inline/where predicates. order_explicit_sized derives the exact existing Sized
compiler-role identity from the retained Q84 plan, never a source spelling. It
validates same definition, policy readiness and all actual Explicit site keys
before mem::take; missing role/site/context exits preserve the original recipes.
After preflight, each indexed map access is justified by the immutable site
invariant. Each contiguous (actual owner span,ActualBoundSite) scratch list has
its own start index, and repeated Sized insertion at that start reverses the
Sized rows with distinct source spans as pinned. Neither unrelated groups nor
MetaSized/PointeeSized rows are globally sorted. No row is deduplicated/dropped.
This insertion work has the same cold quadratic worst case per long bound list
as the pinned vector-front rule; site-map preparation is O(n log n). No runtime
planning or performance claim follows.

Q78 invokes this common reorder before applying defaults/building the published
recipe list, including written-only owners without own generic defaults. Q82
clears default_dependencies BEFORE adding an ordering failure and preserves that
hold if default policy later succeeds. Its written source rows stay independent
of complete item-query readiness. The existing no_implicit policy early return
may omit language-role receipts: a positive written row then genuinely keeps
LanguageIdentity pending because compiler order cannot be authenticated. This
is an availability hold, not a guessed Sized identity or fabricated empty bound
answer. Raw legacy prepare/prepare_parent_where_bounds routes are unchanged and
are not represented as complete compiler solver-query inputs by this review.

No blocking source defect remains in these five final common files within the
reviewed represented domain. The additional Q88 worker integration was still
pending at snapshot d9d5b54648966f9965e8d9f5fbfbd51f3ed658d9fb948b110d1bcf680022a9dc:
source_trait_clauses must invoke the same order helper before apply/with_defaults
and retain any ordering failure; its old colon-first comment must not describe
the final policy. Parent/worker were notified directly. This snapshot is not a
final Q88 completion certificate. Its genuine AliasParameterDefaults,
AssociatedIdentity/QualifiedBound/HigherRanked and recursive dependency closure
holds remain independently required. The eight named architecture conclusions
above continue to hold for the inspected repair; actual control terminals,
all-edition/origin/epoch matrices, complete compiler facts and real runtime root
acceptance remain unexecuted by this reviewer and validator-owned.

Review correction and final scratch-vector rereview

The validator found a real overgeneralization in my earlier grouping advice:
where_predicate_clauses uses a fresh vector per declaration predicate, but
probe_ty_param_bounds_in_generics (clauses_of.rs:988,1039) declares ONE bounds
vector before all selected generic predicates. My blanket claim that every
where predicate uses an independent scratch vector was wrong for the filtered
trait query. The earlier grouping conclusions must not be applied to Q88. Keep
the old source fingerprints/findings as attempt history, not current approval.

Final source now retains and stable-encodes BoundOrdering::{Declaration,
TraitProbe}. Every prepare_written_rows call site was inspected: prepare_selected
(own/legacy) and prepare_output_bounds choose Declaration; only
prepare_filtered_trait_bounds chooses TraitProbe. In Declaration, actual owner
plus each actual Parameter/Where/TraitColon site remains separate, matching
explicit_clauses_of/where_predicate_clauses. In TraitProbe, Parameter and Where
sites coalesce into one GenericProbe list under the same actual owner; the colon
list remains separate. Therefore a later selected where Sized can move before
earlier selected where rows within the shared probe, without crossing into the
colon/default vector. Repeated Sized insertion and distinct-span retention are
preserved. Alias All parameter-default production remains held, so unsupported
probe-default insertion/order is not claimed complete.

All actual failure exits still precede memtake; missing policy/Sized receipt or
site retains written rows. Q88 now calls order_explicit_sized before apply and
keeps the ordering hold even if implicit policy later succeeds. Its positive
and pending dependency rows remain source descriptors, not a genuine recursive
compiler query closure. No further blocking source defect was found in this
represented source domain. The previous noimplicit positive-row role hold,
const/associated/binder/alias-default holds, legacy incompleteness and all eight
architecture boundaries remain explicit. No tests/builds/runtime or matrix
execution was performed by this reviewer; actual terminals remain validator-owned.

Final reviewed SHA256 fingerprints superseding interim snapshots above:
- frontend/source_types.rs: 9bea037849e7cd28952ab3548f788860095457fff5a40f5e94883cad174b2b21
- frontend/source_predicates.rs: 10e6777945318cb5757013878ec3f5622a784d42477f03763de30cb67d7b3de1
- frontend/declaration_source_predicates.rs: ca42e41ed31c8a98247a15e06f0d14a5bb5e8e00ac4d506c878bc61471914d95
- upstream/source_default_bounds.rs: 65c3b09fd15c9861ca0aa758249a3c3fb00013f71291b37d8faec9282c036a98
- frontend/source_item_bounds.rs: 87869fee708c222735b390d8787f6ce62043a6f01539b36ebc1e828c9ed94197
- upstream/source_trait_clauses.rs: 2abfb8c0e90c85f934c0b1ca51dc0d4647fa1028c472436c6f7c5590ecc1f0d7
Only this review document was edited by this task. Comparison facade/provenance
source remained frozen throughout. This completes a bounded source review,
not the M020 milestone, recursive metadata closure or repr128 runtime acceptance.
