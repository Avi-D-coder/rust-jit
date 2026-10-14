# H002 statement temporary boundaries — in progress

Entered let and expression statements now have bounded deterministic Statement
scope metadata. Their executable expression children inherit that temporary
boundary, while let-pattern bindings retain the containing lexical block scope.
The shared source/runtime-intrinsic entry adapter enters a reached statement.
Normal statement commit transfers or discards the result, then leaves the
statement scope before the next statement or block-tail continuation.

Uninitialized declarations enter and leave their statement boundary while retaining
block binding storage. Return, continue, break and panic use the existing scope
stack. Explicit unconditional ExtendedTo annotations override a statement's
ordinary value-temporary scope. Constructor-guarded extensions remain pending.

Five new controls check metadata ancestry and budget refusal, actual moved-source
retirement before following binding reads, loop/control cleanup and unentered cold
statements. An explicit kernel owning-string setup verifies ordinary storage
retirement at the statement boundary and extended storage survival to block exit;
it does not execute source borrow/call syntax or user Drop.

Existing manual storage and raw kernel controls now enter their actual statement
scopes. Jointly used raw kernel operands share one indexed statement rather than
occupying separate source statement lifetimes. Exact opcode, fuel, scope-stack and
cleanup assertions include the added enter/leave operations. The normative main
uses 12 operations, eight exits and three cleanup elements, for 23 fuel; 22 fuel
still faults. Panic and tail retirement counts include actually entered statements.
No ownership, stale-handle or fault assertion was weakened.

The fixed 100,000-node acceptance case exposed needless extension-map allocation
for expression-only bodies. The extension pass now builds its expression index
only after finding a let initializer. This retains the original metadata budget
and actual node-boundary acceptance instead of increasing a limit.

Remaining default match/guard/lazy-operand boundaries, constructor admission,
general patterns, source owning values/references, cleanup suspension, actual
user Drop/effect/native observations and full E24 off/auto/force acceptance remain
pending. H002 remains in progress; no full LRR fixture or release is marked passed.
