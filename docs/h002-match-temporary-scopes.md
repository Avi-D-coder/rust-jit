# H002 match guard temporary boundaries — in progress

Structural match arms now carry an optional MatchGuard scope. Its ordinary boolean
expression uses the existing boolean entry adapter and keeps its result in the
arm scope. The arm stays parented by the enclosing expression boundary. The match
scrutinee retains that enclosing boundary; indexing introduces no independent
scrutinee temporary scope.

Reached guard entry performs a bounded pure grammar check against the exact
body/profile/source. Pattern and let-chain guards return LRR-E-UNSUPPORTED before
any boolean operand or scrutinee call thunk is prepared. Nested blocks/control
expressions retain their own condition grammar, as in the existing condition pass.

Three controls prove deterministic metadata, exact query-level grammar refusal
with absent call thunks, and actual source-backed scalar guard evaluation in an
explicit VM kernel context. The kernel supplies root/arm entry and a verified join:
guard retirement precedes the arm body operation, selected result transfer
precedes arm retirement, and the scrutinee remains live in the enclosing scope.
This context deliberately does not execute Match Entry or select/commit patterns.

Full match selection, pending bindings, false-guard/or-pattern retry and general
pattern dispatch remain assigned to T050/T051. Constructor admission, source
ownership/references, user Drop suspension and full E24 off/auto/force acceptance
also remain pending. H002 remains in progress and no full LRR fixture or release
is marked passed.
