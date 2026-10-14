# H002 original condition plans — in progress

ConditionPlan is a pure owned source/body-affine model of the reached if/while
condition. Boolean clauses retain their node IDs; let clauses retain clause,
pattern and scrutinee IDs. The pinned AST supplies these identities. Iterative
top-level && flattening preserves written order; it does not enter operand bodies,
resolve names/types, test patterns, execute expansions or mutate guest state.
Clause and preparation metadata limits use checked sizes and bounded allocation.
Stable encoding includes source/profile, body, conditional node and explicit tags.

Entered conditional Entry thunk preparation validates this grammar. Let clauses
inside ||, negation, parentheses or ordinary call/arithmetic operands are refused
as unsupported. Ordinary parenthesized boolean clauses remain intact. Nested
control/block bodies have their own conditions and are not flattened into the
outer condition. Whole-file cold condition validation is not performed.

Actual guest controls execute false-first if/while let chains with an undefined
later scrutinee. The later call has zero query events, no thunk memo, no installed
recipe origin and no published/consumed call action. Reached misplaced let grammar
faults before boolean execution; the same grammar in an untaken branch remains
unprepared. These tests exercise normal LRIR dispatch and do not compile the live
source or introduce an AST evaluator.

This increment preserves the existing boolean short-circuit recipes. It does not
yet implement reached pattern test/commit, ConditionalScope/ClauseScope storage,
binding visibility, failure cleanup, TailScope, let-extension or per-iteration
binding lifetimes. The full E2405 fixture still needs output and native-marker
channels, not merely this partial preparation evidence. Native-only baselines
remain separate; H002 and every full E24 fixture/tier acceptance remain incomplete.
