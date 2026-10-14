# M007 local-name lookup

`frontend/local_scopes.rs` consumes immutable owned `HygieneFacts`, exact macro
definition/expansion identities, source-ordered local bindings and declaration
ribs. It normalizes through supplied pinned local-context rows; it never computes
a replacement mark algorithm. Definition owner/edition mismatches and missing
context/expansion inputs are diagnostics, never successful absence.

The declaration-rib rule follows the retained upstream
`vendor/rust-analyzer-macros/crates/hir-def/src/resolver.rs` functions
`handle_macro_def_scope` and `hygiene_info`: compare local name and local hygiene;
when traversal crosses the exact emitting macro's declaration, continue with
its parent context's pinned local normal form. Merely sharing a name, owner or
edition does not remove a mark. That source is inspected as provenance only;
LRR does not depend on or invoke HIR inference or the upstream resolver.

Child scopes capture their parent's source-order position. Named function
boundaries stop outer local capture; closure/block scopes can retain it.
Bindings introduced by the same marked expansion compare in their original
context. Arguments retain their caller context. Macro template names therefore
reach locals preceding their definition rather than capturing caller homonyms.

Indexes are deterministic BTree maps, with a checked one-million logical record
ceiling. Lookup borrows names, searches name/context buckets, and iterates over
older ribs/acyclic parent scopes with an explicit record-derived bound. Source
positions are source-index logical ordinals, not persistent identities derived
solely from output byte offsets. `ScopeKey` stays structurally encoded.

Known pending producers yield `NeedInput` before a lookup could rely on their
unpublished output. A newer known binding can shadow an earlier producer for
that lookup. Publication validates the whole batch's source interval, positions, contexts,
definition identities and budget before inserting names or clearing demand.
Stale/malformed batches change nothing. Nested generated producers are published
as new explicit demands. This component does not memoize absence or execute any
macro, build tool, filesystem operation or guest effect.

The macOS fixture uses an actual marked pinned expansion containing a template
`x`, substituted argument `x` and generated local `x`; its owned selected binding
identities agree with standalone Rust 1.99.0 execution. The test supplies source
scope facts explicitly and maps selected binding identities to fixture values.
It does not claim discovery integration or guest VM execution. Additional checks
cover definition visibility, caller/generated separation, named-function
boundaries, missing/mismatched authority, atomic publication and 4,096 nested
scopes. Full MX011/MX012, source-index seals, receiver/label namespace integration
and the later VM remain separate required acceptance work. M007 remains active.

Child entries are source frontiers: generated batches cannot jump past them or
past following statements/producers. E-M007-005 retains the failing witness and
regression in [the publication erratum](errata/m007-local-publication-order.md).
