# T023 checked scalar and ownership storage

ValueRepr has the prescribed C layout (16 bytes, alignment 8), fixed kind tags,
and fixed scalar descriptors. Constructors and admission validate widths,
Unicode scalar values, boolean bits and f32 upper bits. Guest integer widths
stop at 64 bits; u128 is used only for internal literal magnitude checking.

Typed append-only arenas encode run epoch and index. Zero, stale epochs,
out-of-bounds indices and tombstones fail closed. A compile-fail acceptance
check prevents crossing different record arenas. The runtime has one arena per
record kind; this is not a general identity check between separately constructed
arenas of the same record type or independently constructed stores sharing an
epoch. Worker ownership and external protocol admission remain later integration.
Reset advances the epoch and invalidates prior cells, objects, tickets, literals
and frame anchors. Tombstones never reuse indices and still count against the
cumulative handle budget. Owned bytes are released on retirement; arena capacity
remains charged. Allocation sizes, counters and configured maxima are checked.

Cell mutation validates state, storage contract and ownership before commitment.
Moves reserve a transfer ticket before modifying the source. Replacements reserve
an outgoing ticket and preserve location epoch for same-type scalar replacement.
The outgoing ticket represents a later cleanup obligation. String storage is an
actual non-Copy ownership control: copying its host value bits cannot adopt a
second owner, and consumed tickets cannot transfer again. Location epoch overflow
retires the old cell before a fresh location can be initialized.

Numeric literal copies share a run-owned record and one concrete binding.
Materialization checks range before binding and never supplies an implicit width.
A cell retains its literal record so a subsequently materialized alias fixes its
reassignment contract. Non-numeric replacement fails without consuming the ticket
or changing the cell. Full source spelling normalization and contextual hints
remain frontend/execution integration.

Frame anchors provide typed storage ownership, not execution frames. References,
aggregate/iterator/native/Box/dyn storage, recursive Copy policy, scope cleanup,
VM entry and generated-body integration remain mandatory later work. Public
storage tests exercise real mutations; they do not claim full guest fixtures.
Linux acceptance and release/performance/security evidence remain pending.

Runtime faults prohibit further guest cleanup under Section 11.6. The T021/T022
diagnostic manifest flags were corrected accordingly during T023; their earlier
structural evidence never exercised runtime cleanup. No backend contradiction is
claimed for that metadata correction.

Component acceptance: Gate01 passed 596 actual macOS host/doc tests; strict
Clippy, format, architecture and manifest audits passed. See
[checkpoint evidence](../artifacts/macos-first/t023-storage-checkpoint.json).
The ledger records component completion with full guest acceptance pending.
