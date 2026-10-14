# M007 supplied statement-local publications

The exact entered source index can consume an ordered supplied declaration batch
for a statement-macro producer. This is a pure input adapter: it neither expands
macros nor runs tools, reads files or mutates a prepared snapshot. It builds a new
local index and returns it only after every source, body, context, spelling and
metadata check passes. Previously built indexes retain their pending outcomes.

A publication carries its exact SourceSelection and BodyKey, producer identity,
registered expansion key, output fingerprint and ordered owned declarations.
Both current caller selection and publication provenance must match the retained
source. The linked wrapper additionally checks every retained ancestor before
construction and lookup. Binding identities use an explicit stable encoding of
declared body, producer, expansion, output and emitted ordinal. Statement output
replaces the producer row; empty output clears it. Declaration order is preserved
after the retained macro call and before the next source declarations.
Names in the original source arguments therefore retain their pre-output scope.
Checked logical positions, row counts and metadata ceilings remain in force.
Deterministic producer/expansion indexes avoid repeated whole-input scans.

Binding declarations preserve their supplied marked contexts; emitted macro ribs
and further pending producers use the existing checked hygiene kernel. Unknown
producers, missing expansion metadata, duplicate batches, invalid names/contexts,
stale provenance and excessive batches fail. Pattern and attribute frontiers
require generated declaration discovery; this statement interface cannot clear
them. General generated AST discovery, nested generated scope mapping and
coordinator epoch/output attestation remain later integrations.

Four source checks cover ordering, empty output, invalid batches, retained pending
outcomes, provenance across an edit that preserves producer identity, bounded
batch admission and pattern/attribute refusal. A linked three-file source check
covers stale or missing ancestor selections. A standalone native macOS Rust 1.99
program compares the actual pinned marked expansion's emitted local with the
caller's same-named local. The fixture reads the declaration name, context and
literal initializer from owned expansion output and publishes those facts into
the actual original source index. Both selected observations agree with native
execution. This checks supplied statement declaration/hygiene integration, not
a general generated-source parser, guest execution or service epoch lifecycle.

M007 and release remain incomplete. Evidence:
artifacts/macos-first/m007-local-publications-checkpoint.json.
