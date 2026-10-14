# H002 block-tail scopes — in progress

Ordinary reached blocks now have deterministic TailScope metadata parented by
their block scope. The tail expression and its descendants inherit that scope.
Indexing reserves bounded metadata only; it allocates no guest cells and does not
inspect opaque async, generator or const block bodies.

The existing block Exit phase enters the tail scope before the tail expression.
Block Commit transfers the result to the block's destination before leaving the
tail scope, then retires the nested block scope. Root return uses the existing
frame-return cleanup. Return, break, continue and panic use the existing scope
stack and control dispatcher; this change introduces no runtime opcode or AST
evaluator.

Five acceptance controls exercise repeated index identity and budget refusal,
actual scalar VM result transfer before tail/block retirement, loop and return
transfers, panic cleanup, and cold/opaque bodies. Manual VM and storage controls
now enter the actual tail ancestry rather than assuming numeric scope positions.
Panic scope counts include each actually entered tail scope.

This component establishes scalar tail storage and control-transfer ordering.
General owning/reference pattern commit, ConditionalScope/ClauseScope binding
retention, let-extension, actual destructor/effect observations and full E24
interpreter/JIT/native fixture acceptance remain pending. H002 remains in progress;
no full LRR fixture or release is marked passed.
