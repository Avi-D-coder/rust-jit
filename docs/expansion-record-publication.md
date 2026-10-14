# M008 immutable expansion input publication

ExpansionRecord validates the owned LRRT transport schema and exact expansion key,
then owns the encoded token bytes and their stable digest. Its fields are private.
ExpansionInput queries read an immutable matching record or return typed Expand
input demand. A completed query returns the key/output/size descriptor; token bytes
remain in the input record, avoiding another payload in the memo. Consumers decode
through the record's pure accessor. Grammar-specific generated AST parsing is M013.

Coordinator adoption requires the frozen token context, matching base source
revision, exact key and admitted output digest. Derived publication appends without
replacing frozen source inputs, preserves the base source revision and advances
only the query-input revision. Identical records are idempotent; differing records
under one key fail. Base publication cannot inject expansion records. Publication
from an active provider dependency stack fails. During a run it obeys that run's
query metadata cap; rejection neither publishes input nor releases a saved phase.

Successful reply admission is distinct from installation. A saved phase remains
waiting until checked record adoption succeeds; only then can its one-use token
release owned state. Cancellation and context mismatch still refuse adoption and
resumption. Input-record capacities and retained handles are charged by the query
cache accounting. Descriptor copies contain no native/guest pointer or helper handle.

The MX013 fixture invokes the actual pinned declarative expander once outside
SourceDb, adopts its output, re-enters the pure lookup and observes memo reuse.
Its selected value matches an actual standalone macOS Rust 1.99 native program.
Saved fixture payload ownership is retained before adoption and released once after.
No actual guest effect, ordinary VM evaluation or procedural helper is executed.
The pure-provider audit now rejects environment, clock, macro service/adapter and
VM dispatch imports in addition to filesystem/process/effect-broker paths.

M008 component acceptance covers the specified polling, dependency-stack unwind,
cancellation/late-reply and saved-continuation interfaces. Procedural sessions and
authority/helper lifecycle are M009; generated-source grammar/discovery is M010–M013;
concrete interpreter/JIT suspension and effects are later T/M integration gates.
Release and Linux acceptance remain false.
