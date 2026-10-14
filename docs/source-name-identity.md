# Canonical original-source names

Source outlining uses the exact pinned AST and the existing Unicode 17 NFC
normalizer at its owned semantic-name boundary. Declaration names, logical
module paths and duplicate-name ordinals share canonical identity. Prepared
source contexts canonicalize their module paths before duplicate detection and
parse/outline-key publication. A spelling change between equivalent composed
and decomposed names preserves DefKey identity; original header tokens and
source spans retain their source spelling.

Semantic name conversion is separate from source-identifier validation. The
macro-name boundary validates source syntax before removing `r#`; AST-derived
semantic `gen` can then be canonicalized without rejecting its reserved spelling.
Full-width characters retain their identity because the normalization is NFC.
Conversion checks input/output text size, reserves owned buffers explicitly,
and checks logical module-path depth and aggregate size.

Original-source buckets now agree with the canonical names already used by
marked `$crate` routing and textual macro lookup. Direct query consumers provide
canonical owned semantic names; this change does not add an alternate lexer or
resolver. The immutable publication and source bucket tests prove canonical
context and declaration identities. The native three-package Cargo oracle
exercises a renamed/reexported macro whose `$crate::café::r#gen` helper lives in
a decomposed source module. Selection follows actual pinned marked output.
Guest VM execution, source-wide imports, entered-body discovery and generated
source publication remain required later integrations. M007 remains in progress.
