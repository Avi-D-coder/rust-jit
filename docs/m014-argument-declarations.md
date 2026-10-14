# M014 audited original intrinsic arguments

Task and invariant: enumerate declarations in every actual argument of a resolved
compiler intrinsic without executing ordinary statements or preparing cold bodies.
Transport preserves original token spans and contexts through the pinned expression
bridge. Argument syntax identities are distinct from expansion records; scanning
an original argument does not charge a macro expansion.

The previous milestone failed three qualified compiler-facade CLI cases. The
repair lets qualified paths proceed to the existing namespace resolver; compiler
identity and source readiness still govern selection. All 93 include CLI tests
passed in the isolated draft on macOS ARM64 with Rust 1.99.0.

The draft scans all parser-selected argument ranges, including formatting tails,
and only settles an intrinsic when every argument inventory has no pending
producer. Declarations join the structural impl inventory. Argument summaries
are typed discovery inputs with stable encodings and bounded retained metadata.
Publication checks the source selection and cfg, reparses actual argument ranges,
reconstructs original source tokens, and compares the resulting owned inventory.
Changed source rejects the old publication.

Three controls exercise a cold impl with invalid ordinary statements publication
invalidation, and non-block expression delimiters with unresolved nested syntax. Nested macros or transforming annotations inside arguments remain
unresolved in this component; their recursive expansion is pending. Complete impl
catalog seals, reached symbolic selection, guest JIT and full upstream fixture
acceptance remain pending. M014 stays in progress. The last accepted frozen
checkpoint remains 1733 host/doc passes; a newer full gate must succeed before that
count changes. Linux acceptance remains pending.

The first integrated run passed formatting, component controls and both strict
Clippy checks, then failed architecture audit because the database directly
referenced the adapter. That failure is retained. The corrected draft reconstructs
source tokens through a typed expansion helper. Expression arguments use an
invisible transport group around their original tokens so block delimiters remain
part of the actual expression. New validation remains required.

The corrected full run exposed an inherited statement-attribute diagnostic assertion.
M014 now discovers that producer before runtime preparation, so the regression
requires CATALOG-INCOMPLETE for its unresolved annotation and retains UNSUPPORTED
for entry parameters. The failed full run is retained. No new checkpoint is claimed.

The subsequent gate exposed raw macro definition spelling discarded by declaration
scanning. Preserve written r# spelling until edition-aware validation canonicalizes
the semantic name. A cold raw-keyword regression and the existing generated
qualified raw-keyword case cover this repair. The failed run remains retained.

The final corrected milestone passed 1748 actual macOS host/doc tests and all
64 M014 controls, plus formatting, strict runtime/host Clippy and architecture.
Seventeen unchanged upstream cases retain 47 component assertions; full fixture
acceptance remains zero. Final manifest audit is the remaining checkpoint step.
