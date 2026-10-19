# Original item-group ownership and cold prefixes — M019/M020

Solo root, LOCAL-049, baselinec700e0939a469b34720c5abfa14b3b65a5565aef;
root-owned LOCAL-037/044 interfaces remain coordinated by the same writer.
Unchanged witness: tests/ui/coherence/coherence-impl-in-fn.rs, all editions.
Selection/output/header insertion now work, but the coordinator still cannot
append and complete its local item group. The actual canonical body-item
producer is not the DeclarationScan producer of each individual Meta. Treating
every Meta as a whole-item group would duplicate derives and effects.

Extend OriginalItemTarget using the real BodyItem.attribute_input (or existing
module potential_attribute_producer), retaining its actual scope/range/header
frontier. The publication's pure attachment index must map that genuine group
identity and its real ordered scan annotations to the same canonical item.
Do not rehash an item into a fake module call/expansion, choose a group by macro
name or store a SourceView inside its own publication cache. Retain a privately
constructed OriginalAnnotationGroup with source, canonical item/header, actual
call and scan identity/rows. Unknown canonical parents stay pending.

Use the existing DeclarationPublication::adopt primitive for both original
and generated declaration indexes. Original DeclarationMacroIndex needs the
same pending-preserving append and immutable prefix view as generated parents;
do not clone an alternative macro environment into SourceMacroIndex. Both
parents must retain emitted macro definitions/bodies/hygiene, preserve pending
frontiers after each append and expose the prefix only to the matching owner.
Factor shared publication behavior; do not label a derive inert or discard its
macro bindings. Final group completion requires an actual replayed completion
proof and must settle only its own scan producers after all outputs complete.

Production integration still includes SourceDiscovery's source calls/jobs,
GeneratedFragment prefix ownership and output adoption, SourceDeclarative's
completed catalog storage, GeneratedMacroCatalog's admitted local owner route,
and group completion/frontier replay. Existing module handlers remain until
their common replacement is evidenced. No local item names are restored into
the module namespace as a shortcut. Correct body/member/type source publication
remains required. This record is not a completed handoff or release claim.

Cold costs: one attachment/group map per exact publication/parser/cfg, retained
canonical plans, and the existing transactional prefix publication. Only small
keys/rows are copied; no BodyIndex clone, parser/evaluator, per-operation source
preparation, global namespace guard, resource cap, static checker or guest
operation. Existing body identity validation/cold cloning costs remain visible.
Primary evidence must use real selected upstream derive outputs, actual
GeneratedDeclarations/GeneratedDeclarationMacroIndex and shared publication,
check stable group/Meta identity, ordered prefix visibility and no early
completion, and keep full catalog/runtime/origin/milestone results separate.
Review all eight recurring modes against exact source fingerprints. Full
M019/M020/interpreter/JIT acceptance remains open until actual integration.


V259 library build passed; source review before tests found that a group proof
must also require actual adopted occurrence custody. V260 library build passed
with per-Meta admitted occurrence records and exact-set completion guards; no
runtime/control test was run for either source. V261 removes a redundant whole
scan/body replay from group require_source in favor of the actual retained group
receipt. Both final builds pass and 11/12 controls pass. The unchanged local
witness expands and adopts both real outputs, completes their ordinary producer
closures and exact group across all editions. Completion rejects zero or only one adopted output; duplicate appends and
repeated completion are also rejected. The four-annotation upstream procedural source has
one shared source-order group in all editions; its procedural execution is not
claimed. Existing source origin and 12-world runtime copy regression pass.
The unchanged whole catalog matrix STILL FAILS: four local-derive worlds remain
pending, with eight other worlds successful. No coordinator/cold-group/fixture
completion or new stock runtime gain is established. Source/HEAD/binary guards
match. A production coordinator handoff is the remaining next step, not a new
language feature. Reuse these actual grouped/prefix primitives and preserve all
output bindings when integrating it.
