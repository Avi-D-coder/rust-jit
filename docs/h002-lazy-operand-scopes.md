# H002 ordinary lazy-operand temporary scopes — in progress

Structural indexing now gives ordinary && and || operands separate short-lived
BooleanClause scopes. Parenthesized or block operands keep their result in the
parent scope while their remaining temporaries retire before advancing. Entry,
result transfer, branch validation and retirement use the existing verified
boolean-scope recipes; no new opcode or AST evaluator is introduced.

The condition index explicitly identifies flattened top-level && aggregators.
They retain the existing nested pattern-clause scopes rather than acquiring a
short boolean wrapper around committed bindings. Boolean subexpressions within a
single clause still get their own lazy operand boundaries. Skipped operands have
metadata but no reached entry, execution or thunk memo.

A reached let expression without an indexed if/while ClauseScope now returns the
exact typed unsupported grammar outcome before scrutinee or binding execution.
This prevents malformed ordinary lazy expressions from becoming an internal
missing-scope error or being treated as supported let chains.

Six controls exercise deterministic sibling scopes and budget refusal, actual
and/or decisions and cold missing-call memo absence, nested conditions retaining
bindings and executing stateful effects once, and the misplaced-let refusal.
An explicit kernel owning-string setup verifies temporary retirement before the
next sibling scope can enter; it does not execute source owning calls or user Drop.
The existing parenthesized false||true condition control now counts four actual
boolean scope leaves, including the two newly entered inner operand scopes.

Default match/guard boundaries, constructor admission, general patterns, source
ownership/references, cleanup suspension, actual user Drop/effect/native
observations and full E24 off/auto/force acceptance remain pending. H002 remains
in progress; no full LRR fixture or release is marked passed.
