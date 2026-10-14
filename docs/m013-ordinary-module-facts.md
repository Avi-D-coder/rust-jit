# M013: ordinary module declaration facts in adopted sources

Revision 4 / Section 57 milestone C; macOS ARM64 Rust 1.99.0 first.
This is a necessary source-declaration component, not completed nested ordinary
module capture/adoption. M013 and release acceptance remain incomplete.

Invariant: an ordinary mod declaration retains its actual captured definition,
textual producer, source identity, cfg, edition, header position and physical path
request. It cannot be represented by a fabricated generated expansion/header.

The existing retained syntax module traversal now returns source-order ranges
with external module requests. The old directory-sorted request API remains
available for the original loader. Raw/Unicode names use the existing canonical
semantic-name adapter, matching the declaration outline. Captured views expose
these owned structural facts without file access, reparsing, executable lowering
or macro execution.

SourceModuleDeclaration binds each request to the exact existing DefKey and
source-module producer event. Its stable encoding includes exact outline/source
context and position. The actual staged physical source retains these facts,
charges their metadata, and exposes unresolved ordinary declarations separately
from generated demands. This does not discharge their existing producer frontiers
or publish a partial seal. Coordinator capture/adoption integration follows next.

Three acceptance controls cover actual definition/producer/source/cutoff,
inline/path syntax and ordering, cfg-disabled declarations, canonical raw/Unicode
names, and source-byte/position changes preserving definition identity while
invalidating exact fact identity. The focused tests pass. Gate/audit results are
recorded only once observed in the checkpoint.

The actual CLI/native witness in m013-ordinary-module-witness is pending: native
Rust 1.99.0 macOS returns 2053 for an ordinary external module inside an adopted
child; LRR returns catalogue-incomplete before guest entry. comparison_passed is
false. The native build uses an isolated copy; no selected live runtime unit is
compiled. This implementation limitation is not a specification contradiction.

Ordinary nested capture/adoption, broader ordinary/generated/included context
combinations, complete quotas, service/facets/cache/runtime/type integration,
full MX027/MX028, JIT/release and Linux remain mandatory. M014–M016 follow in the
fixed cross-ledger order for nested declaration producers and catalogues.
