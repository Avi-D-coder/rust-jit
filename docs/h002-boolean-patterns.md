# H002 boolean literal condition patterns — in progress

A reached true/false condition pattern now emits verified ordinary LRIR. Entry
sets a bool hint and enters the scrutinee once. The reached scrutinee return phase
materializes the source-affine expected boolean into a reserved lazy value slot,
executes NumericBinary::Eq without consuming either input during the test, then
retires the scalar operands with normal Discard operations. The actual boolean
result returns to the ordinary if/while or short-circuit continuation.

Body indexing reserves bounded deterministic pattern-test slot metadata, not
cells or executable scrutinee work. Literal pattern syntax supplies a bounded
LiteralRecipe; no AST evaluator or new runtime opcode is introduced. Non-literal
and non-boolean patterns retain typed unsupported outcomes pending the ordered
pattern implementation.

Actual guest controls select true/false branches and i64 results, execute source
predicate calls once, stop after a failed clause with no later call query/memo/
installed origin/action, and keep mismatches distinct from guest panics. While
controls execute scalar comparisons, continue and break, terminate with n=3, and
measure four published/consumed predicate actions in the call case. Ordinary loop
scope retirement permits repeated lazy pattern temporary use.

A nonboolean literal preserves the existing LRR-E-OWNERSHIP descriptor refusal;
a nonboolean callee preserves LRR-E-CALL-RETURN-TYPE; fuel exhaustion remains
LRR-E-RUNTIME-RESOURCE. None becomes a false pattern or successful branch.

This scalar, destructor-free increment does not establish general clause storage
or Drop lifetime semantics. ConditionalScope/ClauseScope binding retention,
reference/owning patterns, failure cleanup before else, TailScope, let-extension,
iteration references and actual drop/effect/native observations remain pending.
The full E24 fixture matrix and off/auto/force equivalence are not marked passed.
