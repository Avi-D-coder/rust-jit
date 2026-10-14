# T031 scope and hint preflight — work in progress

The interpreter now runs non-mutating state admission before charging ScopeEnter,
ScopeLeave, SetHint, NumericHint, LocalHint, BranchHint, Unit and CheckUnit.
Scope ancestry/innermost-scope checks and hint-target liveness are shared frame
helpers used by execution. Numeric/local hint values are computed by immutable
helpers, then applied only after charging. Branch hint admission validates its
actual bool and only the selected child's state/context; the cold child is not
queried or modified.

Unit uses the shared type-aware destination validator without allocating a cell.
CheckUnit examines a live result's unit kind/type without materializing unresolved
numeric records. Invalid checks do not increment started counters or consume
fuel, and preserve existing scope/value state. Source queries remain separate
owned-recipe construction and do not evaluate these runtime facts.

Two added controls prove zero-fuel invalid leave, one charged entry followed by
an uncharged duplicate entry, and exact diagnostic/no-extra-charge behavior for
hinting an executed slot or branching on unit. Prior VM/source/numeric suites
remain regressions; the normative successful main remains eight ordinary ops and
eight exits, using sixteen units before cleanup element accounting is integrated.

This is partial preflight coverage. Declaration/read/bind/assignment and transfer/discard now have
[ownership preflight](ownership-preflight.md). Literal and exit runtime checks
still require separation before charging.
Cleanup/aggregate element charges, staged activations, full call/effect/JIT/service/
supervisor and generated/source-view integration, final T031 audit and release
acceptance remain unfinished. ScopeLeave admission currently covers active-scope
state, not the full element/ownership/cleanup machine. No complete fuel, user Drop,
reference or Linux acceptance is claimed by this checkpoint.
