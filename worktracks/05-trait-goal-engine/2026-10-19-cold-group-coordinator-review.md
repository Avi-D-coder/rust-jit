# Cold original group coordinator source review

Solo root; M019/M020, LOCAL-049; baseline c0878d52871e6bfcd0070eabb38d8e3d88635e16.
Review in progress; final source fingerprint and command results follow.

1. No assertion/expression evaluator added. The changed paths stop at source
   discovery and ordinary GeneratedDeclarations/DeclarationFragment completion.
   Runtime operations and macro compiler selection are unchanged.
2. No literal/break executor. SourceDiscovery::poll_original_derive_group calls
   the existing source_derives::prepare_group; emitted items enter
   append_source_derive_output and poll_item_declarations, including ordinary
   producer closure and source syntax retention. Original Meta/group/occurrence
   IDs remain distinct. Never create a fake module invocation.
3. No method or numeric inference special cases. Group registration is by real
   whole-item annotation syntax, not Copy/Clone or fixture/macro name. The
   actual compiler/procedural facet still selects each derive path.
4. GeneratedMacroCatalog::for_admitted_output/prepare_owned and
   GeneratedHeaderPlan::prepare_using retain actual local scopes only with a
   matching opaque declaration/catalog/pending-parent receipt. Raw owner keys
   remain insufficient. The old unsupported local named-declaration publication
   remains explicit at NameNamespaces::define_declared_item; no module path
   flattening or silent successful omission. This is a required remaining
   integration gap, not completed ordinary/generated semantic parity.
5. No pattern machinery or arm/body preparation added. The source catalog
   control must show no BodyStructure/Thunk/PatternPlan/FunctionSignature query.
   Full runtime pattern/cleanup/cold-branch gates remain separately open.
6. New preparation is cold and immutable; installed VM operations do not call
   it. OriginalAnnotationGroup receipt uses retained canonical declarations.
   Existing generated catalog/header/route preparation still reconstructs
   syntax from SyntaxEvents and outlines in several cold phases. These
   pre-existing duplicate plan routes remain required performance/removal debt;
   this change does not close the prepare-once architecture gate.
7. No dispatch guard/global namespace census added. SourceDiscovery's existing
   route fixed point remains cold, and NameNamespaces frontier keys retain
   complete ScopeKey including owner. Relevant source/epoch/receipt validation
   is required before completion. No scaling claim.
8. Original group and output inventories share Arc handles. Adoption clones
   DeclarationMacroIndex cold publication metadata and retains scan handles;
   no BodyIndex clone or per-operation whole-file syntax search added. Actual
   retained declaration-prefix scopes clone cold scope/declaration/hygiene
   structures once into Arc roots; source provider snapshots still clone
   existing cold metadata maps. No constant-time or memory-performance claim.

## Transaction and remaining holds

Actual source output selection is acknowledged after
complete_original_group_output adopts the real CompletedDeclarationFragment.
complete_original_annotation_group validates the exact adopted occurrence set
against replayed completion proof before retiring original Meta frontiers.
Aliases wait on their real group leader. Sealing replays original group
selection through revalidate_original_groups. Source syntax assembly retains
actual item outputs and nested completed outputs. No generated name can be
made a module member merely to let this witness complete.

Replacing attributes, procedural execution, associated containers, named local
output publication and arbitrary local-output macro origin/body matrices remain
required and unexecuted in this bounded change. M019/M020, actual solver use,
full interpreter runtime acceptance and guest JIT remain incomplete.

## Exact reviewed V268 source

Fingerprint: 7c3193d6d3fdb8b9b78e602081eb84a435e662c4c33bd2b10776cf01bc9e2891
(8,477 manifest inputs; parent c0878d52871e6bfcd0070eabb38d8e3d88635e16).
Separate source inspection follows the implementation/build pass; this review
is bound to that fingerprint and must be rechecked after source changes.

Additional inspected call sites: SourceDiscovery::poll_generated_fragment uses
the retained preceding source provider; SourceDeclarative::catalogue_context,
routes_using and revalidate_module_catalogs_in_context replay the existing
derive_prefixes map. declaration_prefix_snapshot retains the actual
DeclarationMacroIndex::retain_original_derive_prefix boundary, validated by
actual group/scan identity and private prefix marker.
CapturedMacroContext::{append_source_scopes,retain_source_scopes} use that
real boundary and reject a different local owner. Completed roots keep these
prefixes without publishing lexical definitions through SourceMacroIndex.
No module-only expansion key or fabricated call is inserted.

This is related to audit MA007 (a catalog must select genuine source expansion,
not runtime semantics) and MA008 (no context-only executable/verifier tracks).
MA001/MA003/MA004/MA005/MA006 runtime evaluator/numeric/method findings and MA002
pattern machinery are not changed or closed here. These MA IDs are distinct
from the eight recurring review modes above.

Source review found one classification hazard: a derive-shaped annotation can
resolve an actual replacing Attribute facet. Before any output exists,
poll_original_derive_group now relinquishes speculative aliases and resumes
the existing poll_nested_declarations cold attribute path. This branch is
source-reviewed but not procedurally executed in the current matrix. Mixed
replacement after actual derive outputs stays pending. No claim of complete
local macro lookup/body execution follows from the local Copy/Clone catalog
fixture. Full required origin/epoch/suspension/cleanup matrices stay open.

V266's catalog matrix is12/12but its old component inspector fails by asking
for mutable access after sealing. The V267/V268 test-only correction reads
the actual retained route_install namespace snapshot; it does not clone a
mutable graph or weaken the seal. The component now requires actual catalog
completion and tests typed local admission, wrong owning scope, wrong epoch,
sibling receipt mismatch and a child without a real pending request, alongside
existing zero/partial/duplicate-output custody checks. Execution results must
be collected separately before reporting those assertions as passing.

## Collected V268 terminals

Library36281/0, CLI73621/0, control runner43481/0:8/8controls pass.
Actual global catalog matrix12/12source worlds, with997headers for the local
fixture in each edition; all selected wrong scope/epoch/sibling/no-request
admission and exact-custody assertions pass. Existing unchanged Copy runtime
regression passes12origin/edition worlds. Source/HEAD/binary guards match.
No procedural execution, full fixture/suite total or new distinct stock runtime
gain follows. Full local-name/origin/currentness/cleanup and prepare-once gates
stay held. Detailed reproducible commands/selectors and failures: ../evidence/2026-10-19-v268-original-group-coordinator.json.
