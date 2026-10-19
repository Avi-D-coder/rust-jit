# Cold original annotation completion -> ordinary declaration publication (held M019/M020)

Owner /root, LOCAL-049, parent 23032fe13c. This is the next prerequisite of
the failed complete 32-head control, not acceptance of the solver or interpreter.

Invariant and unchanged witness: `tests/ui/coherence/coherence-impl-in-fn.rs`
contains a local enum with real Copy/Clone output. Discovery completes the actual
annotation group, but Database::publish_captured_declaration still returns
NeedsProducer for that original declaration. A source fact cannot be fabricated
by dropping its attribute or substituting the module scope. Full 32-head
acceptance currently fails at 20 ready; its assertion remains enabled.

Actual shared route: original_items::OriginalAnnotationGroup and the retained
DeclarationScopePlan identify the item; source_derives::SourceDeriveCompletionProof
and DeclarationMacroIndex::{adopt_completed_child_prefix,
complete_original_annotation_group} prove real output custody and closure.
SourceDiscovery::publish_cold_catalog_declarations must transport this evidence
to Database::publish_declaration_metadata, the same kernel used by entered and
cold declarations. Publication remains source/header/body/cfg metadata. No
BodyStructure, binding/pattern inference, executable preparation or name grant.

Implementation: derive an opaque completed-original-annotation receipt only
from the actual completed DeclarationMacroIndex, exact source/group/proof and
appended output occurrences. Preserve the existing no-receipt refusal. The cold
publisher must validate its current captured parent and exact retained item
before accepting a receipt; wrong source/item/body or an unsettled group fails.
Use the same kernel for annotated and unannotated rows, publishing every named
local declaration from actual canonical facts rather than only impl headers.
The coordinator may supply a receipt only for a genuinely completed group.

No alternate evaluator, parser, local Type resolver, method/numeric rule or
runtime dispatch is added. The next required mechanism remains a declaration-only
view of the existing ordinary local Type tables/rib/import traversal; do not
add a solver-only lookup or delete signature_names' owned-scope guard. Complete
M019/M020 and source/origin/edition/currentness/runtime matrices remain held.

Costs: once-per-publication traversal of actual retained cold declaration scopes
and their rows; reuse immutable plans. Receipt replay checks real groups/output
IDs and indexed source facts on this cold boundary. No per-VM-operation work,
full-world dispatch scan or BodyIndex clone. No speed claim without measurements.

Evidence: unchanged original across four editions; actual local enum metadata,
shared no-receipt NeedsProducer behavior, wrong-source/body/definition receipt
refusals, retained source/currentness guards, no executable-body queries, real
source catalog and existing Copy interpreter regressions. Keep the 32-head
failure distinct from this bounded metadata control. Separate eight-mode review
and exact source fingerprint required before checkpoint; no feature closure.

## Independent review while asynchronous validation runs: next common mechanism

Source inspection confirms `signature_names::prepare_type_name` deliberately
returns NeedOwnerContext for owned declarations. `generated_result_namespace::
lookup_declaration_inner` also requires a module scope/Type receipt. Neither
guard may be removed to force the local heads through module name lookup.

The existing ordinary local engine is `SourceLocalIndex::lookup_prefix_inner`
and its import_routes/import_lookup/glob_lookup/anonymous_import helpers.
LocalTypeNames supplies actual hygienic item/generic declaration occurrences;
HeaderTypeSite already supplies declaration-owned Type positions without
pretending the position is an executable expression. Currently these borrow
PreparedBodyContext/EnteredMacroIndex, even for declaration-only facts.

The next bounded slice must factor the declaration-table construction and
Type/item/import traversal out of that entered wrapper and share it with a
real retained CapturedDeclarationScope view. Keep Value bindings and runtime
entry with their existing owners. The cold view needs the actual original or
generated Type occurrence, exact lexical scope/ancestry, owning source/hygiene,
current published declaration metadata, genuine completed producer state and
relevant module positive/negative dependencies. Generated local names must
join their actual admitted insertion/producer origins. Do not collect only
original items and claim all generated-source/name parity, build a fake entered
body or add an independent solver-only lexical resolver. The publication work
above supplies metadata; it does not yet supply that entire Type-prefix proof.

Source review also found a performance trap in bulk publication: calling the
selected-row publisher for every row would scan D rows D times. The implementation
now validates the captured scope once, visits its actual item/member rows and
uses the same private row-publication kernel as selected entry. This avoids
introducing that quadratic selection walk. Existing global cold comparisons
and broader prepare-once acceptance remain separate debt.

Final V283 evidence: both builds pass; 13/14 controls pass. New actual local
metadata publication and the strengthened real-group receipt controls pass all
four editions. The complete source-head control remains failed at 20/32; no new
stock runtime gain or actual solver invocation. Source/HEAD/binary guards match.
See the separate source review and bounded V282/V283 evidence; earlier test-only
compile failure remains attached to its original source fingerprint.
