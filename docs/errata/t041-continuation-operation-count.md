# E-T041-001: continuation-only recipes have zero ordinary operations

Revision4 Section10.4 says: "Each recipe contains at most eight primitive machine
operations." The implementation instead required one through eight. That lower
bound contradicts the fixed phase/continuation architecture; it is not a required
minimum in the specification.

Minimal witness: unchanged `tests/ui/regions/regions-simple.rs`, the entry phase
of `*y` in `*y = 5`. The RHS already ran; this phase continues to the original
receiver path. Its recipe has zero ordinary operations and one valid Next exit.
The frozen deref-v1 batch's first exact assignment test failed in the provider
with `LRR-E-INTERNAL-IR: recipe requires one through eight ordinary operations`.
Actual runner7705/1 is retained; strict Clippy, formatting and architecture had
passed. No assignment result was accepted and19 remaining commands were unstarted.
The external reporting and collector-summary errors remain separate failures.

Correction: enforce the specified maximum of eight operations and retain the
existing exactly-one-exit, PC, slot, origin, dependency and size checks. An empty
phase still charges exit fuel, advances exactly once and requests no successor
recipe during preparation or execution. No dummy opcode or replay is added.

The original assignment test remains unchanged. A new stock-source continuation
regression verifies PC, single exit fuel, unchanged ordinary counters/guest
allocation, and an unrequested receiver thunk. The existing IR boundary test now
checks0 and8 valid operations,9 invalid, and missing/duplicate exits invalid.
All repair tests are unexecuted in this private successor. T041/full stock
fixtures/JIT/release/Linux remain incomplete.
