# M013: macros in ordinary adopted modules

Revision 4 / Section 57 milestone C; macOS ARM64 Rust 1.99.0 first.
M013 and release acceptance remain incomplete.

Invariant: macro selection retains the real declaration identity and body owner,
with parent textual visibility frozen at each physical module declaration. Ordinary
source projections never impersonate generated expansion headers. Owned argument
tokens and consumer environments remain bound to their actual invocation keys.

Ordinary/generated source lineage now shares bounded inherited context traversal.
Source invocation lookup uses the existing hygienic resolver with actual retained
outer scopes, hygiene facts and expansion ancestry. Local declarations retain
precedence. Nested generated fragments, compiler include operands and generated
module requests use the actual ordinary caller rather than requiring a generated
parent header. Invocation ordering uses the retained original source ordinal.

Completed ordinary child snapshots are retained both by the source parent and
by its original unit's body-owner index, without importing textual bindings into
the original root. Body lookup traverses retained ordinary children and captured
catalogue owners with explicit depth and work limits. This allows macro-use bindings
and qualified exports to execute their actual child source bodies. Route body-owner
frontiers still defer execution until those owners complete.

Seven CLI controls exercise actual parent cutoff, refusal of a future parent macro,
local shadowing with a nested generated call, compiler include with a parent string
operand and physical source directory, macro-use child body ownership, a generated
physical module emitted by an ordinary caller, and an exported ordinary macro called
from the original root. Six successful cases are compared to real isolated native
oracles. The qualified export fixture explicitly allows Rust's absolute macro-export
lint in the identical shared source; the earlier native denial log is preserved.

Full MX027/MX028 and broader producer cycles, procedural/source integration,
complete quota boundaries, service/cache/type/runtime/JIT/security/release evidence
remain required. Body-position user expansion, comparison assertion operands and
later M014–M016 integrations remain unfinished. Linux remains deferred.
