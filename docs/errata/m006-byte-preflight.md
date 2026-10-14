# E-M006-004: exact schema2 byte admission

The explicit bounded entry point now accepts a private byte-budget provider in
addition to token limits. It uses count-only mode of the same pinned transcriber
before creating its main output builder. LRR always supplies a provider built
from its already captured owned span/context records; it reads no files, runs no
queries, and executes no compiler or macro helper.

Schema2 context/span costs are measured by the same field writers used by the
owned codec. The transcriber counts group/identifier/punctuation/literal wire
fields and distinct span/context records. Literal spelling is measured through
the pinned literal formatter into a checked length sink, with no string buffer.
Raw identifiers exclude the separately encoded raw marker from their text;
spacing/scalars and table indices retain fixed schema2 widths. Counts include the
root group and all three table/root counts. Returned owned output is checked
against the exact admitted byte count; disagreement is a typed schema error.

Metadata registration is deterministic in emission order. A checked undo log
records first-use span/context insertions, so rollback removes only records
introduced after its checkpoint rather than rescanning the entire accumulated
table for every nested repetition. Temporary repetition wrappers are excluded
from output accounting. Unknown span metadata remains a schema error and is not
masked as successful or retryable expansion. Limit failures never try another
matching macro arm.

Actual regressions cover exact and one-byte-below reduced limits, empty/ragged
repetitions, expression groups, raw strings, Unicode and raw identifiers. The
real 32 MiB boundary succeeds; its one-byte-larger candidate is rejected with
zero output materializations. A compact invocation containing an 18 MiB Unicode
literal and a transcriber that duplicates it also fails in count-only mode with
zero materializations and no fallback. The fixture report preserves actual
preflight token/byte and materialization counters.

The existing four-file patch set is extended in place against unchanged original
archived preimages. All patch/preimage/postimage receipts remain independently
audited; no additional dependency or frontend is introduced.

Scope: exact final output bytes and generated tokens are admitted before the
main output builder is created. Discarded trial iterations can still allocate
speculative output during the admitted replay; eliminating/bounding that scratch
work remains required before M006 completion. Current hygiene is explicitly
root-context only; marked-context preview and context ancestry integration remain
M007 and must preserve this byte admission contract.
