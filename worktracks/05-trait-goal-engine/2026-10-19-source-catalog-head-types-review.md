# Source catalog / declaration Type transport review — HELD M019/M020

Solo root, LOCAL-049; parent0eb7a147a157ef7d3e9bebfce701775484648603.
Reviewed V281 source fingerprint:
c5a2b85bd9a5c417f2fbac3182a7a15f60226a3f44ba804a3361bc7805338dbe
(8,479inputs). This is a separate source-review pass after implementation;
source changes require a new fingerprint and relevant revalidation.

## Actual shared route

SourceDiscovery::publish retains actual TypeScopeReceipt facets from the
existing certified_scope_in batch on the real configured ImplCatalog graph.
DiscoveryInput::require_scope_certificate_layout first requires its private
ImplCatalogCompletion; missing/changed completion or facet payload fails.
SealedName accepts only actual Type buckets of that domain, preserving existing
source/profile/bucket validation; it does not mint a Type Scope seal or Value
completion. CurrentModuleName and ordinary declaration name services consume
these same indexed certificates. No fixture or trait-name special case.

SourceDiscovery::publish_cold_catalog_declarations uses genuine
PreparedOriginalAssociatedDeclarations::declaration_scope and
Database::publish_captured_declaration. That shared publication kernel validates
current owning source/header/body/cfg and exact canonical row. It publishes
metadata, not executable preparation or lexical name authority. Pending actual
annotation producers remain missing metadata; they are not acknowledged here.
Macro-export-only sources are excluded by their existing role ownership.

WrittenTypeDeclaration::{resolve_name_at,resolve_name},
type_name_namespace::resolve_declaration_name_at and the existing generated
declaration lookup share classification and namespace logic. A parser-created
DeclarationTypeName validates the actual NameRef in a PathType and its nearest
item owner; it binds ParseKey, definition, scope/range and header identity.
GeneratedRows::unit_range maps retained locations before the common
lookup_declaration_inner origin checks. Scope/header/source changes cannot reuse
the receipt. No caller-supplied range alone authorizes lookup.

source_type_paths::lookup is shared by nominal qualified Types and trait path
projection consumers. Normal paths retain the actual scope; `$crate` obtains
its defining unit through ParsedFile::dollar_crate_root on the real parsed
token and tail-component editions through token_edition. No spelling-to-unit
map, macro whitelist, placeholder header text or independent parser is added.

GeneratedSyntaxInput::prepare_using validates the real admitted declaration
record/cfg/epoch/scope before permitting owned roots. SourceDiscovery passes
the actual item_root_declarations/item_declaration_inputs receipt. The ordinary
outline_item_fragment kernel retains that scope; associated children still
require their actual container. The already computed Arc<Outline> is shared
and the obsolete fallback outline reconstruction is deleted.

## Eight recurring architecture modes

1. No assertion/expression evaluator added; changes stop at ordinary source
   discovery/header/type queries. Runtime expression evaluation is unchanged.
2. No scalar/literal/break-only generated executor. Every emitted impl header
   remains ordinary admitted syntax, with real canonical declaration/source IDs.
3. No method-name or numeric inference rule added. `$crate` is a language path
   root interpreted from genuine hygiene, not selection by a derive's name.
4. Owned generated query roots retain actual source scope with a typed receipt.
   Missing cold local Type prefix still returns Pending(SelfType). The enabled
   complete32-head control must remain failed until all required contexts work;
   do not relabel those heads inapplicable or supply a module fallback.
5. No pattern engine/arm/body preparation changed. Explicit demanded header
   queries must show no BodyStructure/Thunk/PatternPlan/FunctionSignature demand.
6. No display-text reparse or new per-phase planning. Pure memo queries retain
   their results; Type name proofs use direct syntax_at ranges plus ancestors.
   Remaining generated syntax construction in several cold phases is debt.
   An attempted declaration-parser reuse used a different grammar ParseKey and
   regressed V280. It is reverted, not papered over by a key alias. No full
   prepare-once architecture or performance gate closes.
7. New world/scope enumeration is only existing cold catalog publication.
   CurrentModuleName uses the retained relevant bucket/selector path. Existing
   generated receipt validation still traverses current source/record inputs
   during cold Type queries; installed VM dispatch gains no census or parser.
8. Actual plans/outline/parsed query handles remain shared. Cold publication
   borrows canonical declaration plans; no BodyIndex deep copy or repeated
   whole-file node search is introduced. Type proofs allocate small metadata
   per cold lookup; path ancestry work is proportional to actual path depth.

These modes are separate from audit MA001–MA008. MA007 catalog/source selection
and MA008 one source/index/verifier route are relevant review constraints;
no audit finding is closed by this bounded work. MA001/MA003/MA004/MA005/MA006
evaluator/numeric/method routes and MA002 pattern machinery are not changed.
The legacy member_index::header_text placeholder/reparse route and string-only
whole-header origin API remain held migration debt; neither is extended here.

## Required remaining integration

Full cold local lexical Type prefix still lacks the common pure declaration
rib/import/generic and completed-producer join. signature_names owner guards
must not be deleted in favor of a module lookup. Reuse SourceLocalTables/local
resolver mechanisms and retained DeclarationScopePlan/CapturedDeclarationScope;
do not synthesize an entered function, prepare executable bindings or add a
solver-only local Type evaluator. Retain relevant absent/present dependencies
so a pending answer changes when its real input is supplied.

Mixed/opaque/retained header Type origins, local generated named declarations,
procedural/associated/closure source cases, full editions/origin/currentness/
suspension/cleanup matrices and broad runtime milestones remain incomplete.
Actual typed SourceFacts production, compatible pinned inference/delegate,
real upstream root-goal evaluation and ordinary selected-member/dictionary/VM
integration remain required. Full M019/M020, interpreter and JIT gates stay open.
Source/review/build evidence does not substitute for those executions.

## Collected V281 terminal evidence

The reviewed source fingerprint remained unchanged through both builds and all
controls. Library session 97325 and CLI session 97500 exit 0. Runner session
70039 exits 1: 12/13 selected controls pass; the complete head control fails
with 20/32 ready and twelve local SelfType results pending. Eight generated
occurrence positive/negative control worlds pass within that aggregate.
Global catalog matrix passes 12/12 source worlds; existing unchanged Copy
interpreter regression passes 12 origin/edition worlds. No new distinct stock
runtime gains, suite-total refresh, upstream solver execution or feature closure.
The after-runtime source, HEAD and binary guards all match.

Commands, exact selectors, build terminals, binary/input fingerprints and
bounded log excerpts are retained in [V281 evidence](../evidence/2026-10-19-v281-catalog-head-types-held.json).
Prior failed sources retain their separate evidence; the complete failing
control is neither removed nor narrowed. No production source changed after
this review. Checkpointing this held candidate does not accept M019/M020.
