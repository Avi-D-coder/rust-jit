# T031 numeric preflight — work in progress

NumericBinary, NumericUnary and PrimitiveCast now run immutable preflight before
fuel is charged or operation counters start. GuestStore previews a scalar at its
existing binding or reached expected width, validating owner/state/range without
allocating result tickets or fixing the literal record. Frame validation checks
active output scope, existing temporary state and strict destination descriptor
without creating a physical output cell.

The numeric kernel and preflight share pure binary/unary/cast descriptor-admission
functions. They check supported primitive pairs/operator classes and distinct
word identities. They do not perform guest arithmetic. Actual execution after
charging still materializes required records, reserves output storage and invokes
the checked operation. Arithmetic overflow/division/shift failures therefore
retain charged GuestPanic behavior.

A raw integer shift-count record is admitted by its reached u32 descriptor,
without trying to convert a negative/oversized magnitude during preflight.
Charged execution performs the existing sign/width check and produces InvalidShift
before count materialization. Float or noninteger counts still fault in preflight.
No value is evaluated to discover its execution order or replayed afterward.

Two added boundary controls prove a wrong numeric pair spends no extra unit and
reserves no output cell, and that an unsupported unsigned negation leaves a
previewed shared literal unbound so a subsequent i64 use succeeds. Existing
exhaustive/random arithmetic, cast matrices, signed minima, raw invalid shifts,
source contexts, panics and control-flow tests remain required regressions.

T031 remains in progress. Scope/hint/branch/unit state is now covered by [state preflight](state-preflight.md).
Literal/local/transfer/discard/exit runtime checks still need separation before
charging. Cleanup
and aggregate per-element charging, operation activations, supervisor/service
statuses, multiframe and JIT integration, final audit and release evidence remain
unfinished. This numeric checkpoint does not prove those requirements. Linux
acceptance remains deferred.
