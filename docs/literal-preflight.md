# T031 literal preflight — work in progress

Literal admission now precedes opcode fuel, output-cell reservation and numeric
record allocation. A prepared literal retains its checked scalar or integer
magnitude/type; execution consumes this preparation after charging. Integer
spelling, excluded suffixes and concrete integer/float ranges are checked through
shared pure conversion helpers. Explicit suffixes and reached strict context
cannot reinterpret a conflicting scalar descriptor. Admission does not bind a
shared numeric record or manufacture a ticket.

Unresolved numeric literals retain their deferred type. Unsuffixed float grammar
comes from the pinned frontend's decoded literal; this checkpoint does not claim
validation of arbitrary hand-built malformed decoded float records without a
reached type. Shift-count admission preserves the existing charged guest-panic
behavior for invalid counts.

The added control checks malformed integer spelling, excluded i128, i8 overflow
and f64 literal overflow. Each rejects after the prior ScopeEnter unit with no
literal charge, output cell or numeric record. Existing conflicting-suffix/bool
controls now assert the stronger no-allocation invariant; their initial stale
assertion failure is preserved in the attempt1 log. Numeric, storage, source
execution and shift-count regressions pass.

T031 remains in progress. Cleanup/aggregate element charging, staged operation
activations and full multiframe/call/effect/service/supervisor/JIT/generated
integration require further work and acceptance. No release or Linux acceptance
is claimed.
