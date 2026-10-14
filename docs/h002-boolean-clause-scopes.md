# H002 short-lived boolean clause scopes — in progress

Each structurally indexed top-level boolean clause now has a BooleanClause scope
parented by the current retained pattern prefix. Its result slot belongs to that
parent; its operands and temporary storage remain inside the short scope. Written
parentheses remain a single clause. A block or nested-if clause may enter its
ordinary inner scopes without changing the outer boolean scope boundary.

Source and compiler-runtime-macro entry builders share a pure verified-recipe
adapter that enters only the reached boolean clause. Condition and short-circuit
continuations validate the boolean and leave its temporary scope before entering
a later clause or selected branch. Successful earlier pattern scopes remain live.
While backedges allocate fresh cells; skipped clauses have metadata but no scope
entry or operand recipe execution. Existing panic/control cleanup uses the same
scope stack.

The compiler-intrinsic predicate-call verifier admits the exact indexed boolean
scope entry prefix while retaining its original seal and argument-shape checks.
A valid no-item certificate for an expression-position intrinsic now revalidates
against its retained invocation without trying to publish an absent declaration
producer. Statement producers still require the original matched producer kind
and publication. Source/body/compiler identity checks remain mandatory. The
regression exposed LRR-E-MACRO-SCOPE before dispatch; after the fix, the actual
predicate call publishes and consumes once, then the assertion's unit result
preserves LRR-E-OWNERSHIP when used as a boolean. This relaxed-source fault case is
not a native Rust oracle or an accepted boolean assertion expression.

Seven controls exercise deterministic metadata and budget refusal, failure/cold
successors, retained bindings, block/nested clause results, repeated while cells,
actual reached macro panic with its extra entered scope, and the macro call/fault
boundary. Existing static panic controls are rerun without weakening assertions.

General refutable/reference/aggregate patterns, let-extension and owning iteration
lifetimes, source owning-value execution, user Drop suspension, actual destructor/
effect/native observations and full E24 off/auto/force acceptance remain pending.
H002 remains in progress and no full LRR fixture or release is marked passed.
