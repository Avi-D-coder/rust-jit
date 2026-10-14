# M014 generated macro selection

Task and invariant: resolve nested declaration producers using actual captured
hygiene, explicit expansion ancestry and the original invocation's frozen lexical
visibility. Expanding a declaration must not prepare its ordinary runtime body or
retire an unresolved producer frontier.

The generated declaration index reuses the existing textual scopes, hygiene
resolver and expansion ancestry. It registers generated definitions with their
actual name contexts and editions, retains parent inventory identities and bounds
both ancestry depth and cumulative metadata. Nested requests use captured token
trees with the existing marked declarative expansion engine. Display text is not
reparsed to construct macro input.

All 37 targeted controls pass after the boundary correction. Five new controls exercise the unchanged rustc statement fixture's original
`foo!()` selection, generated local definitions, frozen source visibility,
unresolved predecessors, foreign parent/stale provider refusal and two levels of
generated ancestry. These are component assertions under the LRR2024 adaptation;
they are not whole rustc fixture passes.

The architecture audit caught direct adapter token access in the frontend. That
access now stays inside the existing expansion boundary; the failure log is
retained. This is a layering correction, with the same captured token facts and
resolver semantics.

The asynchronous macOS milestone passes with 1721 host/doc tests and all 37
targeted M014 controls. Formatting, strict runtime/host Clippy, architecture and
first/final fixture-manifest audits pass. The evidence is frozen in
[the checkpoint](../artifacts/macos-first/m014-generated-resolution-checkpoint.json).
The unchanged empty-derive CLI interpreter/native reference passes. The upstream
subset has 44 component assertions; full fixture and guest JIT passes remain zero.

M014 remains in progress. Recursive publication/adoption, transformed namespaces,
intrinsic summaries, complete graph/facade impl seals and reached symbolic
selection remain pending. Full fixture, guest JIT, release and Linux acceptance
remain incomplete.
