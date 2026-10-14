# T031 lazy branch phases — work in progress

Original scalar if and boolean short-circuit expressions now use entered phases
and verified LRIR branch exits. Entry schedules the condition/left operand with
bool context. Operand(0) selects the next PC without requesting successor thunks.
BranchHint reads the actual bool and updates only the selected child hint: if
arms inherit the reached parent's primitive context, while &&/|| RHS uses bool.
Unselected child slots receive no hint, storage or semantic query from this op.

An entered if arm runs its block, transfers its result, discards condition storage
and continues. Else-if uses the same phase path. False if without else produces
unit; a reached true arm must pass CheckUnit before transferring its result.
An unresolved numeric record is non-unit without requiring a width. Wrong unit
results fault only when reached. String literal execution remains unsupported,
so its earlier diagnostic is checked separately rather than claimed unit evidence.
The initial failed test assertion for that unsupported literal is retained; it
was a test-scope mistake, not a backend contradiction or specification erratum.

For &&/||, the selected short path transfers the actual left bool and discards
its storage; the other path executes the right child once, transfers its bool
and discards the left. No NumericBinary eager boolean evaluation substitutes for
short circuit. Both new opcodes have explicit stable tags, typed slot admission,
effect classifications and separate counters. The normative main retains eight
operations with zeros in the added columns.

Five controls exercise actual integer/float/boolean values, nested else-if,
no-else unit contracts, early return, selected RHS mutation once, unsupported
selected calls, selected hint admission/identity and explicit zero-thunk traces
for cold entire arm/RHS ranges. Source and frame/query state remain separated;
query providers construct immutable owned recipes and do not execute guest values.

T031 remains in progress. Original while phases, repeated iteration storage,
explicit backedges and the developmental quota boundary are now integrated in
[the while checkpoint](while-phases.md). Loop, break values, labels and scalar continue cleanup are now integrated in
[the control transfer checkpoint](control-transfers.md). Precise fuel/cancellation
accounting remains required. Reference/user Drop/pattern/if-let/
generated/source-view/epoch/JIT integration remains the later cross-ledger work.
The current dispatcher step limit is not claimed as complete instruction fuel,
cancellation or watchdog acceptance. Gate03 and full release are not proved by
this partial branch checkpoint. Linux acceptance remains deferred.
