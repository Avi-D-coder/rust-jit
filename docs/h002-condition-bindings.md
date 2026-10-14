# H002 irrefutable condition bindings — in progress

Identifier and wildcard condition patterns now use separate source-backed test
and commit phases. Entry evaluates the scrutinee once with its ordinary hint.
The return phase produces the successful boolean test for an irrefutable pattern
without moving or discarding the source. Commit uses existing BindLocal move
semantics, or existing Discard for a wildcard. No new opcode or AST evaluator is
introduced. Explicit ref and @ forms remain typed unsupported until general
pattern integration.

Lookup uses the owned ConditionPlan to identify preceding top-level pattern
clauses. A binding is visible in later clauses and the consequent, but excludes
its own scrutinee, else and following statements. Nearest local and condition
bindings retain written shadowing order. Source paths, assignment and runtime
macro binding reads share this lookup. Mutable while bindings are recreated by
the existing condition-scope retirement and control-transfer machinery.

Seven acceptance controls exercise source scrutinee effects once, bindings in
later clauses, shadowing and non-leakage, mutable continue/break iterations,
wildcard effects once, and actual verified VM test-before-move state transitions.
The scalar source remains Live after its test and becomes Moved at commit.
An owning source string witness exposed the existing literal milestone refusal;
its exact typed LRR-E-UNSUPPORTED outcome is retained, rather than treating it as
pattern mismatch or a passed owning-value fixture. Failed experimental setup
logs remain in artifacts. No owning source or destructor acceptance is claimed.

ConditionalScope currently retains committed storage. Individual boolean-clause
and retained ClauseScope lifetimes, general refutable/reference/aggregate pattern
test/commit, syntactic let-extension, actual destructor/effect observations and
full E24 interpreter/JIT/native acceptance remain pending. H002 remains in
progress and no full LRR fixture or release is marked passed.
