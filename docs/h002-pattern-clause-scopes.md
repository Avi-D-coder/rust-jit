# H002 retained pattern clause scopes — in progress

Body indexing now reserves deterministic ClauseScope metadata for each written
top-level let condition clause. The first clause is parented by ConditionalScope;
later pattern clauses nest under the previously retained pattern clause. Later
boolean clauses and the consequent inherit the retained prefix. Else retains its
independent ancestry. Metadata allocation is checked and does not execute or
resolve scrutinees; malformed let grammar remains a reached ConditionPlan check.

Only a reached let clause enters its scope. Its boolean test result belongs to
its parent scope, while its committed bindings and scrutinee storage belong to
the clause. This allows existing short-circuit recipes to return their actual
result while retaining successful bindings for later clauses and the consequent.

ScopeLeave at a ConditionalScope admits only an entered ClauseScope prefix and
drains it last-to-first using the existing frame scope cleanup. It refuses to
cross a live consequent block or any unrelated scope. Within a pattern clause,
committed bindings retire before remaining temporaries. Other scope kinds retain
their existing cleanup order and innermost-only leave rule. General user Drop
suspension is still assigned to the later cleanup milestone.

Seven controls exercise deterministic ancestry, bounded metadata refusal, actual
successful and failed-prefix VM cleanup, skipped later patterns, distinct cells
across continue/break iterations, and condition-leave admission. A kernel storage
control explicitly initializes owned strings through GuestStore/Frame setup;
with fuel for one retirement, the last clause's binding retires first while
remaining temporaries and earlier bindings retain ownership. Cancellation retires
nothing. This proves storage ordering and fault preservation, not source string
lowering or guest destructor execution.

Existing condition metadata assertions now include the intervening ClauseScope.
The reached pattern/consequent panic control includes the additional entered
clause in its exact cleanup count. No failure is reclassified as success.

Short-lived boolean-clause scopes, general refutable/reference/aggregate patterns,
let-extension and owning iteration lifetimes, source owning-value execution,
actual destructor/effect observations, cleanup suspension and full E24 off/auto/
force acceptance remain pending. H002 remains in progress and no full LRR fixture
or release is marked passed.
