# M020 / LOCAL-049 common query facts and actual root inputs

Baseline da1cdc0c3fa3aa4872ba4e4a7afaabe8fd974d42. Unchanged integration
witness tests/ui/enum-discriminant/repr128.rs, still executor OP-TYPE after
V516. This is the next implementation slice, not a completed repair.

Invariant: a source fact's availability is independent of other callback domains;
unprepared is not false/None/empty. The actual root must own its exact caller
ParamEnv, input equations and inference owner. Existing ReadyProvider permits an
arbitrary different goal/owner under the same Cx, while complete DefinitionFacts /
PredicateFacts and Cartesian leaves force unrelated metadata. Replace those
interfaces in the one PreparedProvider; do not introduce another source service,
solver, semantic candidate filter or unchecked sparse callback reader.

Shared entry points: prepared.rs::PreparedProvider/{publish,ready,SourceFacts},
SourceBindings/SourceGenericLayouts, actual Q75-Q88 memo plans and root_variables.
Root input transport uses the ordinary reached CallSeal source/signature service,
not assertion operands. Caller assumptions come from genuine caller own/parent
clause receipts, not the callee's obligations or None GenericEnv. Ordinary macro
publication and all editions retain the same source identity and lowering route.

Lead owns prepared.rs, new shared query-fact/root-input modules, registration
and actual source-factory/root admission. Worker owns run.rs/new private
run_call_inputs.rs packet; worker owns source_impl_outlives.rs and Q88 bindings
accessor. Reviewer/test worker is read-only until explicit validation freeze.
Existing reserved paths remain under their implementing root owner; no external
PR code or branch/worktree is introduced.

Typed unpublished/known/not-applicable domains replace collateral values. All
identity, generic parameter/order/const-Type, complete trait candidate membership,
pinned simplify_type index coverage, actual ADT shape, language-role presence /
absence, requested associated leaf ancestry and source guards remain validated.
Do not seed candidate buckets from unrelated trait flags. Complete associated
member lists and exact requested leaves are separate demands; no missing leaf
may turn into an inherited default. Required role absence and per-definition
non-language classification are genuine independent facts, not missing-map None.

Pure preparation gathers the actual callback dependency closure before entry;
callbacks only read immutable values. An admitted preparation context for the
pinned elaborator is tied to actual impl head, available input families and same
Cx, not a permission for arbitrary root evaluation. Final root admission owns
its exact SolverContext and encoded goal/environment/input variables and consumes
that session without replacement arguments. Missing source facts remain typed
Pending before entry. Host abort policy is unchanged; guest unwind is separate.

Source preparation is cold/memoized with relevant dependency/currentness guards;
no source parsing, world census, plan rebuild or body clone during installed
execution. Typed table lookup and upstream inference happen only on demanded
source constraints. No claim about measured performance is made. No extra
resource limit, source sandbox, borrow/privacy/const-context static checker is
added.

Required evidence: real upstream root and same-owner integer resolution before
fallback, unchanged repr128 runtime contract and source/origin/edition route
controls; wrong owner/goal/env, missing demanded fact, unrelated missing fact,
complete candidate bucket, requested leaf, actual language identity/absence,
source/epoch invalidation and exact-once guarded commit. Do not replace the
runtime witness with a fake evaluate/WellFormed goal. A partial query factory or
packet remains held until its actual consumer and obsolete-route removal pass.
All eight recurring failure modes require a final fingerprint-bound review;
MA001-MA008 remain separate and no gate is closed by this plan.


## Pinned impl query integration (continuation after V519)

V519 compiled the source-fact availability split, Q89 and raw reached-call
transport; it ran no test body or guest fixture. Next lead change extends
`PreparedProvider::prepare_impl_super_outlives`, its ordinary SourceFacts
`explicit_implied_clauses_of`/`is_trait_lang_item` methods, and Context's existing
immutable list interning. It runs pinned `rustc_type_ir::elaborate::elaborate`
with actual Q89 head and filtered query inputs, then applies the exact
`rustc_hir_analysis/src/collect/item_bounds.rs:530-542` outlives filter.

The private `ImplElaborationInputs` scope owns only certified callback domains;
its guard drops before source facts can be frozen. It neither publishes dummy
DefinitionFacts nor permits arbitrary solver/root entry. Unprepared const-bound,
escaping-alias and bound-input callback dependencies refuse before the iterator.
Construction generics remain actual source layouts. Context-owned spanned lists
share immutable clauses; callback reads do not clone trees, parse source or
reconstruct plans. The configured source fingerprint must match the actual
provider configuration. No root search cache runs during elaboration.

An actual-source control is assigned to the reviewer for unchanged repr128
primitive PartialEq impls plus the nonempty Foo:'static witness in
`tests/ui/self/builtin-superkinds-self-type.rs`. It must observe actual pinned
output, including genuine parameter identity/Static region for the nonempty
case. Runtime root/caller assumptions/response consumption are still held.
All source changes require a new fingerprint and asynchronous validation;
V519 evidence must stay attached to its original source.


## ADT query domains

The unchanged repr128 goal normalizes DiscriminantKind for an actual enum.
`discriminant.rs::pinned_discriminant_ty` calls AdtDefinition::is_enum and
::enum_discriminant_type; it does not demand destructor/copy/sizedness proofs.
The prior mandatory `AdtInfo` forced unrelated complete metadata to answer
these source facts. Lead changes `model::AdtInfo` to preserve genuine nominal
kind with separate QueryFact slots for every other callback domain, including
variants, flattened fields, flags, constraints and destructor. No semantic false,
empty or None is fabricated. Existing AdtDef consumers read their exact slot;
missing slot remains a preparation invariant, never a semantic answer.

Publication keeps the real DefinitionReceipt + PreparedAdtVariants/schema/name/
kind check. Published variants validate names/order/count; a published flattened
view additionally requires/validates that genuine variant view. Published enum
repr retains its exact original receipt check. Missing unrelated domains cannot
silently supply unsizing or destructor absence. This is the same provider/model,
not a new discriminant evaluator. The existing pinned discriminant client remains
unchanged. The ADT worker supplies actual source producer values into these slots;
root admission must still prove all domains reachable by its specific goal.
No runtime/feature completion or speed claim follows from this representation.
