# E-M007-001: expose the pinned mark algorithm without a source query

This is a mechanical adapter API correction, not a change to Rust hygiene.
The archived `hir-expand/src/hygiene.rs` exposes root-only span constructors;
its mark application entry is `pub(super)` and takes `SourceDatabase`. Applying
a mark to an existing definition context requires that private entry. Its only
source-dependent operation obtains the already-resolved call-site context from
`MacroCallLoc`. The remaining algorithm uses the Salsa span interner.

Expose `apply_mark_with_call_site` with that context as an explicit argument and
the narrower `salsa::Database` interface. The original entry retains its opaque
fast path and original call-context lookup, then delegates. The algorithm from
call-site normalization through hierarchy replay and context interning is the
original archived implementation. A source-proof regression compares that
entire suffix byte-for-byte, allowing only the private helper's database trait
change. The original archive and inventory remain unchanged; the fifth audited
patch reconstructs this one file from its original preimage.

LRR supplies safe database-owned input identities for expansion marks. Those
IDs remain private to the adapter and never enter HIR expansion/source queries.
Owned context records retain expansion identity, transparency, defining unit,
definition-context edition and root ancestry. Item normalization drops mixed
and transparent marks; local normalization retains mixed marks. Import rebuilds
the recorded final hierarchy, so it does not replay call-site inheritance a
second time. Imported roots remain unit-aware even though upstream root context
slots are shared by edition.

Acceptance exercises the actual interner/normalizers and mixed-site replay,
including different definition/call-site editions and units, private-slot
remapping, and typed refusal of inconsistent input. This does not yet prove
marked token transcription, exact marked-span byte admission or final name
resolution. In particular, the macOS E0659 witness in
`fixtures/macros/hygiene/expanded-name-ambiguity.rs` still requires context-aware
namespace integration. M007 and the release remain incomplete.
