# M007 original-source textual macro index

The existing pinned parser's `ParsedFile::outline` now emits owned textual
facts alongside headers: original `macro_rules!` identities, lexical scope
openings and producer frontiers. It honors existing cfg filtering and never
enters function bodies or macro token trees. Definition identities remain the
existing stable DefKeys. Positions are snapshot-relative attachments; these
positions are not added to semantic header/outline fingerprints.

`frontend/source_macros.rs` builds an immutable index from that outline and
owned pinned hygiene facts. It validates the source root unit and edition,
joins discovered definitions to item-name classes, freezes parent visibility at
scope openings, and routes selection through the origin-aware textual resolver.
Definition events retain exact source name syntax separately from semantic
DefKey names. MacroName validates that spelling before raw/NFC lookup
canonicalization, including Rust 2024 reserved `r#gen`. This stage indexes
original source only, so declaration origins are source roots. Generated tokens
need their own per-token contexts and expansion-order facts at publication.

Producer kinds distinguish ItemMacro, Attribute and ModuleSource input. Queries
return NeedInput and expose the producer kind; they perform no expansion,
source loading, compilation or procedural-server execution. Derives and unknown
transforming attributes are pending because their output can add declarations.
A transforming scope's own opening frontier remains visible. An unloaded
external module is a source demand, never an empty complete scope. Unreliable
outline declaration boundaries reject index construction. Record/path limits
and checked opening positions bound owned work.

The native macOS acceptance uses actual original source, pinned parsing and
outlining, adapter-to-runtime name facts and selected source slices fed into the
pinned MBE. Module-parent source order and cfg filtering yield the same values
as Rust 1.99. This proves original-header selection, not guest VM execution.
Further work must connect the index to immutable source publication, entered
body discovery, selected external modules, source-wide canonical name buckets,
generated source epochs, modular/glob imports and macro exports. These remain
required; M007 is in progress.
