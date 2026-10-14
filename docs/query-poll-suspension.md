# M008 typed pure-query suspension

Provider execution and database fetch now return the Section 56 QueryPoll shape:
Ready(Result<T, Diagnostic>) or NeedInput(InputRequest). The input request owns
an exact ExpansionKey or captured-source unit/environment/path/content expectation.
Explicit stable encoding distinguishes request kinds and all identity fields.
Expansion identity and bounded capture path validation run before demand escapes.
The larger expansion payload is boxed; addresses never enter semantic keys.

On demand, active query frames and temporary dependency collectors unwind. The
database removes stale memos and records only diagnostic trace data. Incomplete
input dependencies and partial answers never become completed memo entries.
A suspended child marks its parent collector, preventing a provider from ignoring
it and publishing a fabricated completed value. Memo revalidation also propagates
suspension instead of serving the old value. Re-entry performs pure computation
against newly published inputs and memoizes only completed results.

Terminal diagnostics are separate from demand. Completed deterministic error
memos still reuse and revalidate; cycle/depth failures remain non-cacheable.
Existing source/cfg/deletion/error/cache-pin fixtures now explicitly assert typed
Ready success or error and retain their original accounting and identity checks.
Suspended provider executions count as actual provider runs in trace telemetry.
Invalid request failures evict old memos and leave the stack usable.

Five framework checks exercise nested demand/re-entry, suppressed-child refusal,
old memo rejection, malformed request eviction and stable request identities.
The toy provider reads immutable supplied input; it does not execute a macro
helper or guest code. Full MX013, actual expansion-record queries, coordinator
fulfilment, epoch/cancellation/late-reply validation and saved VM phase dispatch
remain pending. M008 and release remain incomplete.

The final M008 checkpoint integrates immutable expansion inputs and MX013; see
[record publication](expansion-record-publication.md). M008 component acceptance
is complete; the remaining session/VM work belongs to later task cards.
