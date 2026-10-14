# E-M006-003: token admission inside the pinned transcriber

Current combined patches also contain the M007 marked-span and ancestor-byte
integration described in [E-M007-002](m007-marked-byte-preflight.md). Historical
M006 checkpoint receipts retain the implementation tested at that checkpoint.

Revision 4 Section 50.6 requires checking generated-token limits before output
allocation. The original pinned transcriber immediately constructs an unbounded
`TopSubtreeBuilder`; LRR's previous export/adoption checks happened after that
buffer was built. This integration adds an explicit bounded entry point without
changing the selected matcher, fragment parser or transcription algorithm.

The pinned algorithm now targets a private output facade. Its count-only mode
retains checked token counts and delimiter/restore-point state, with no token
output builder. Repetition rollback, invisible-group flattening and emitted
fragment groups use the same control flow in both modes. A bounded invocation
first runs this algorithm in count-only mode. It checks completed top-level
operations and committed repetition rounds, then verifies the final count. Only
an admitted candidate constructs the output builder and replays transcription.
The original unbounded entry point remains available to upstream users; LRR
always uses the bounded entry point with at most 2,000,000 tokens. Rejected
candidates return typed `LimitExceeded`; partial output is never adopted.

The acceptance witness uses a compact repeated input and a 1,000-token static
suffix to produce more than 2,000,000 tokens. Actual counters report preflight
calls and **zero materializations**. Tests compare admitted counts with actual
owned output for exact boundaries, empty repetitions, delimiter groups, ragged
nested repetitions and expression fragments. The 65,537-element regression
continues to exercise the same backend. Failed definitions/requests keep their
existing typed behavior.

The combined transcriber patch and three supporting mechanical patches are
reconstructed from their original archived preimages by bootstrap audit. The
`tt` change only reexports its existing restore-point type; no token storage or
unsafe implementation is changed. `mbe` adds the explicit bounded entry point
and threads its admission configuration/statistics to the same transcriber.
The original archive and source inventory remain unchanged.

Scope: this enforces the final generated-token limit before materializing an
output candidate. Exact output-wire bytes, context/span table admission and
speculative scratch allocation limits inside transcription remain pending.
The measuring codec already guards later candidate byte admission, but that is
not proof of backend byte admission. M006 remains in progress.

Token-only preflight uses a no-op span marker: token structure/counts do not
depend on hygiene marking, and a caller's marker must execute only during the
admitted materialization. Invocation/materialization telemetry counters use
checked increments. A limit failure in the first matching arm returns
immediately, with no fallback to another arm and no materialization. The
transient flattening failure log is retained as a regression witness; counting
now excludes temporary repetition wrappers at open time, so inner committed
rounds cannot falsely exceed an exact final-output boundary.

E-M006-004 now adds exact final schema2 byte admission to the same pass. See
[m006-byte-preflight.md](m006-byte-preflight.md) for the shared record-cost writer,
metadata undo log, actual 32 MiB boundary and zero-materialization evidence.
Speculative scratch elimination and M007 marked-context preview remain pending.
