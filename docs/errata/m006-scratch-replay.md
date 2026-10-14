# E-M006-005: admitted replay without discarded iteration output

A minimal witness is an empty repetition whose transcription emits three static
literals before visiting its binding. The admitted final array has no leaves,
but the original replay appends all three leaves and then discards them. The
actual failing counter assertion is retained in
`artifacts/macos-first/m006-scratch-witness.log`. A large static prefix or repeated
large fragment can amplify this scratch work despite a small admitted output.

The same pinned algorithm's counting pass now records the accepted iteration
count for each repeat in preorder. It removes nested recipes belonging to a
rejected after-end trial. Only a fully admitted candidate replays that immutable
recipe against the same bindings/template. Replay skips zero-iteration bodies,
stops before an after-end trial, and avoids inserting the last separator that
would be discarded. The count pass still performs the original binding/error
checks, including zero-length/cardinality mismatches; no matcher or fragment
parser is replaced. Recipe allocation and cursor increments are checked.

Replay validates recipe exhaustion and its final token count before building
output. Materialized leaf append counters must match actual final leaves.
Temporary repetition wrappers are still needed by the pinned builder; owned
definitions have at most 256 group levels, so these add at most 256 raw token
slots beyond the admitted final count. A checked bound of admitted tokens plus
256 is enforced before each raw append/extension. A mismatch/excess returns a
typed resource failure and drops partial output. Actual peak slot counts are
recorded separately from final wire bytes; these are logical token slots, not
an allocator-capacity or resident-memory measurement.

A depth-80 nested repetition matches the real Rust 1.99.0 native oracle. Its
empty input materializes zero leaves and peaks at the root's one raw slot;
one nested value materializes exactly one leaf and peaks at 82 slots. Reduced
limits, ragged/nested/empty groups, editions, syntax errors, native binding order,
real two-million-token and 32 MiB refusals remain in the current acceptance gate.
The same four-file patch set remains reconstructed from original archived
preimages. No unsafe token-storage change or dependency substitution is made.

This completes the main output candidate's token/byte admission and discarded
iteration scratch repair. It does not claim marked hygiene/namespace integration,
macro service deadlines/cancellation, guest VM execution or full release
acceptance. Those remain their ordered M/T tasks. Final M006 component acceptance
is audited separately; full goal/release completion remains false.
