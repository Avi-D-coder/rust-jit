# E-M007-002: marked transcription and ancestor byte accounting

M006's admission provider represented root contexts only. A marked span refers
to a context with parent records, and every ancestor must appear in the schema2
wire table. Counting only its immediate context underestimates final bytes.
The old count-only marker also left template spans unmarked, so it could count
different metadata from admitted materialization.

Register the definition's actual mixed-site marks through the pinned hygiene
store before transcription. Capture a closed span-substitution map. Both passes
use that same map at the original transcriber's marking sites; the counter uses
an explicit preview callback and does not invoke the caller's materialization
marker. Arguments keep their original spans. Marked records keep source unit,
content/path, coordinates, source edition and origin; definition owner and
context edition come from explicit source-index authority, not token location.
Private source slots remain anchored at their original location. Parse/match
edition lookups use the database which actually owns the registered contexts.

Extend the existing metadata undo log to walk ancestors through a captured
parent/cost provider. Count each stable context once, including shared roots,
and roll back newly introduced ancestors with speculative repetitions. Use the
same canonical context field writer as the actual encoder. No ancestor vectors
or wire buffers are allocated per output span. Final export includes exactly
the reachable ancestor closure, and its measured codec length must equal the
admitted byte count. Preserve the original matcher/transcriber and original
archive/inventory; regenerate the existing combined patches for the two
affected upstream files rather than installing a second patch for either file.

Seven macOS regressions exercise template/argument identity, explicit definition
ownership independent of token unit/edition, generated marked definitions,
the $crate definition authority, shared ancestor accounting, empty repetition rollback, exact reduced limits,
and the real 32 MiB boundary plus byte+1. Rejected output reports zero main
materializations. Legacy unmarked matcher fixtures remain component tests;
live source resolution must supply the full expansion/definition context to
the hygienic entry. Source/import integration and full MX011/MX012 resolution,
including the native E0659 witness, remain pending. Request-wide deadlines,
cancellation and total arena accounting integrate in M012; these tests do not
claim that later acceptance or a release.
