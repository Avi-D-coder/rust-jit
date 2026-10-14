# M007 owned name-comparison facts

Source contexts and name equality have different roles. Roots from distinct
units/editions retain different owned context keys, but the pinned `Name` and
`HygieneId` comparison operation removes root edition after normalization.
Local lookup must not compare source authority keys as name equivalence classes.

`HygieneStore::name_contexts` emits sorted owned `NameResolutionContext` rows
with separate typed `NameContextKey` item/local classes. It normalizes through
the actual pinned API, calls `remove_root_edition`, and reuses the stable context
remapping descriptor with a comparison-only zero root owner. A distinct digest
domain encodes that descriptor. This is stable ID remapping, not an alternative
normalization algorithm. Original contexts, owners, editions and source spans
are untouched; no comparison-only contexts enter the source/span tables. No
raw Salsa ID, pointer or private database slot becomes serialized identity.

The same checked depth-limited descriptor now supplies ordinary context adoption
and comparison encoding. Comparison rows have a two-million-row ceiling,
checked allocation, complete key prevalidation and deterministic sorting/dedup.
Pairwise tests compare both classes directly with actual pinned name equality
across roots, opaque/semi/transparent marks and nesting, then import into a new
database with an unrelated preexisting mark to force private-ID remapping.

Runtime `HygieneFacts::with_name_contexts` validates source references and duplicate
rows before publishing the immutable comparison table. Missing rows are errors;
there is no fallback to source-context-key equality. Class correctness comes
from the audited adapter producer, as with other semantic input facts. Class
keys are comparison-only identities and cannot route `$crate` or select source
editions. The token wire remains LRRT schema2; these are separate owned facts.

`LocalScopes` indexes binding names by local comparison class. Traversal still
keeps its original normalized context for declaration-rib origin/owner/edition
checks; crossing a rib obtains both the parent's comparison class and its
ordinary normalized context. Atomic publication validates all class inputs
before changing names. Callers must demand comparison rows for the source
contexts and reachable normalized ancestors used by lookup.

The boundary test moves LRRT contexts and JSON comparison facts from the actual
adapter into runtime, then resolves a local across root unit/edition differences
while asserting owner/edition remain distinct. The actual marked/native local
collision acceptance continues to pass. Full compatible macro candidate index,
source seals, query suspension and guest VM integration remain pending; this
checkpoint does not complete M007.
