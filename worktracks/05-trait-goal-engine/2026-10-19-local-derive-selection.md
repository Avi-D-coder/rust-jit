# Local annotation selection and origin join — M019/M020

Solo root, LOCAL-049 with existing root-owned LOCAL-037/044 interfaces. Baseline
fed34ceb5605a5fea8ef9d10cea347148ddb6f30. Required unchanged witness is
tests/ui/coherence/coherence-impl-in-fn.rs in all four editions. Its body-local
enum now has a genuine canonical target, but prepare_group and
DeriveDeclarationOrigin::validate_database only accept module Outline producers.
These are the next original-source assumptions to remove. E-R5-005 and the
macro audit apply, especially recurring failures 4, 6 and 8; no audit finding is
closed by this bounded prerequisite.

Extend PreparedSourceView's actual original-item target with a retained
producer-to-attachment index. Resolve existing ordinary module producer IDs and
actual DeclarationScan attribute producer IDs to that same target. Preserve
their real IDs rather than rehashing an item as an expansion. Cache the pure
range index under exact source/publication/parser/cfg identity through the
existing publication_prepared cache. No source view/owning publication is stored
inside its own cache. Unknown local/container parents remain pending, never a
no-impl/no-item conclusion.

source_derives::prepare_group must use that join for module and body producers,
then run its existing path/definition/helper selection, source occurrence input
construction and pinned builtin/procedural request constructors. Origin replay
must resolve the same captured source and actual producer/target and check the
canonical item/header identities, retaining normal database dependencies. No
macro-name shortcut, fake producer, runtime expression evaluator or standalone
derive compiler. Procedural helper origins use the same validation mechanism.

The later source index/prefix/append/completion work remains required:
SourceMacroIndex currently owns module textual scopes; SourceDeclarative's
derive prefix and completion methods require those real producer events;
SourceDiscovery::append_source_derive_output looks in .calls, and
complete_source_derive restores a module declaration/frontier. A cold job needs
a genuine common source-item and lexical macro scope handoff before these can
complete. Do not fake a module call, drop output macro bindings or mark the
attribute inert to bypass those prerequisites. The global catalog matrix and
M019/M020/interpreter/JIT gates remain held until actual completion evidence.

Cold costs: construct one immutable producer/range index per captured source,
then ordered lookups and the retained canonical item plan. Existing item/body
identity validation remains; no new dispatch/installed-operation planning,
global namespace scan, source IO, guest operation, resource accounting or static
checker. Required evidence: actual configured source/compiler namespace,
unchanged local/module witnesses across editions, real selected pinned derive
output admitted through GeneratedDeclarations and ordinary generated header
projection, source/header/producer mismatch rejection, no executable-body/query
demand, and exact-source eight-mode review. Partial source selection/expansion
is a component result, not runtime/full-fixture acceptance or catalog closure.


The actual admission join also exposes the enclosing grammar distinction:
DeclarationScan records a local item's annotation in Statements, while derive
output is Items. Keep that true scan row unchanged. Extend the shared
GeneratedDeclarations::admit_attached with a privately constructed real
OriginalItemTarget insertion proof, obtained by SourceDeriveSelection from its
exact retained source/origin and canonical scope. Validate parent parse/cfg/path,
actual attribute attachment and original provenance; then scan output at the
canonical insertion scope using the parent's real lexical scope kind. Other
callers retain their exact grammar checks. This is a common item-insertion
provenance join, not macro-specific runtime semantics. Ordinary generated header
projection must agree with the canonical scope and reject the coarse scan owner.

The pure configured-source control replays selection in the real compiler/name
world and actually expands/adopts its selected output through GeneratedItems,
GeneratedDeclarations and GeneratedHeaderPlan. It does not append this result to
the discovery coordinator, synthesize completed occurrence receipts, publish a
catalog, execute a guest or reinterpret discovery failure as success. The real
catalog result is printed separately. Complete item-group ownership must map
all of a local item's scan annotation jobs to its genuine ordinary item producer
before coordinator integration; dispatching every Meta as a separate full-item
derive group would duplicate effects and is not an acceptable handoff.


V257 built both binaries and ran all ten controls. The new local control
selected real output, validated original provenance and admitted the actual
expansion, then FAILED at GeneratedHeaderPlan::prepare's module-only
scope.owner refusal. The global source matrix remained FAILED; other eight
controls passed. Preserve these terminals. Do not drop the required ordinary
header projection from the test or replace it with an isolated range helper.

Extend the SAME GeneratedHeaderPlan::prepare_using with an admitted
GeneratedDeclarations receipt, checking exact record/cfg/epoch/insertion scope.
Bare owned scope keys still fail. Its prepare_admitted entry reads scope from
that receipt, then uses ParsedFile::outline_item_fragment -> the existing
outline_in_root kernel with the true scope. Ordinary module source derives
also use this admitted entry in append_source_derive_output. Other entry points
retain their old provenance checks. contained_scope must distinguish the real
local root owner from its associated-item owners; do not substitute a module
root for a body owner. This is immutable header preparation, not entry into an
executable body or permission to publish an unfinished catalog. Generated
nominal/name publication across all local contexts remains a separate open
integration obligation.
