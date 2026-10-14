# M014 generated declaration syntax admission

M014 remains in progress. The invariant is to admit the actual expansion grammar,
retain its owned syntax and token origins, and inventory nested impls/producers
without preparing or executing ordinary operations.

The adapter reuses the pinned rust-analyzer token-tree syntax bridge. Expression,
statement, type and pattern slots use their respective parser entry points.
Items and associated items use the pinned MacroItems entry; trait/impl output
also requires the pinned AST's associated-item node classes. Invalid output and
wrong-slot declarations fail rather than yielding an empty successful inventory.
The item adapter and declaration adapter share one bounded syntax-event exporter.

GeneratedDeclarations checks the exact expansion key, captured parent producer,
grammar, call edition, cfg and epoch before admission. Its stable identity binds
the parent inventory, expansion descriptor and epoch. Every output-origin row
must refer to an actual span in the immutable output forest. Parsing does not use
rendered text: owned events rebuild the original nodes in an arena container,
without adding or reparsing a source-text wrapper. Output offsets remain intact.

The existing declaration scanner then inventories active nested impl and member
headers, cfg-prunes disabled declarations, and preserves unknown macro demands.
Generated inventories carry an explicit origin and inherit the invocation's
module/function context. The source-only macro index refuses these inventories:
generated names require their real output hygiene, not a fabricated source root.
The immutable impl index can retain source and generated headers together, while
both parent and nested producer frontiers remain pending. No catalog seal is issued.

Twenty-seven targeted macOS controls pass. New controls exercise an actual cold
expression containing an impl, unresolved ordinary body calls and a nested macro;
all grammar slots; original origin rows; cfg/parent/key/slot/epoch refusal; epoch
invalidation; and inventory limits. A regression actually observed TraitItems
becoming Items after transport; the parser root's recorded grammar now preserves
trait and impl slots for nested producers. Its failure is retained. A harness
spacing assumption was also corrected to inspect original output tokens, since
display text is not a reparsable source contract.

Two controls consume unchanged official rustc files. The statement fixture
preserves `foo!()` as an unresolved nested producer. The newly imported
`macro-named-default.rs` expands its original root macro and keeps the associated
macro demand suspended behind that root frontier. The component harness uses
LRR2024; copied directives and original license texts remain unchanged. Seventeen
cases and 37 copied files are imported; full native/LRR fixture acceptance is
pending. Gate00/01/02 and provisional Gate03 component pass with 1711 host/doc tests,
all 731 M013 controls and 27 M014 controls. Formatting, strict runtime/host Clippy,
architecture and first/final manifest audits pass. The restricted unchanged
empty-derive interpreter/native reference was rerun. Seventeen unchanged licensed
cases and 37 file hashes retain 42 component assertions. Full native/LRR fixtures
and guest JIT acceptance remain zero.

Recursive producer adoption, transformed local namespace integration, audited
intrinsic summaries, complete graph/facade impl seals and reached symbolic
selection remain pending. MX029/MX030, guest JIT, full fixtures, release and Linux
acceptance are incomplete. This syntax/inventory integration is not a substitute
for those requirements.

See [the checkpoint](../artifacts/macos-first/m014-generated-declarations-checkpoint.json).
