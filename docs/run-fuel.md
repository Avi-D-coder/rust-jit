# T031 run fuel and interruption — work in progress

RunFuel owns a bounded quota (default 10,000,000, maximum 1,000,000,000), exact
used/remaining counters, checked poll counts and a terminal stop reason. Polling
without an opcode does not consume fuel. Charge checks interruption and credit,
then spends exactly one unit. Exhaustion, cancellation and deadline have distinct
FuelStop values and developmental runtime-resource diagnostics. Once stopped,
the meter cannot resume. Frame fuel limits cannot reset after execution or stop.

The interpreter now charges each ordinary opcode and each recipe exit, replacing
the previous whole-recipe limit. The dispatcher checks credit/interruption before
thunk resolution, and the meter polls before every opcode/exit charge. Current
scalar/storage cleanup polls at scope boundaries and charges each occupied cell. A cancellation token uses
safe host atomics; its observed charged count lets a host monitor distinguish
active work from a pre-execution cancellation. No guest handle is exported.
A monotonic deadline can be installed by the host.

The initial source main has eight ordinary opcodes and eight exits: it succeeds
with 19 units, including three cleanup cells, and faults with 18. A bounded unending loop uses exactly its given
quota. Already cancelled/expired runs execute and query no thunks. A second thread
observes at least 256 real charges in a running guest loop before requesting
cancellation; the run terminates with Cancelled rather than exhausted fuel.
Core tests cover exact charges, terminal stop, quota bounds and cross-thread/
deadline interruption. Five new controls are registered in the task ledger.

This is partial fuel integration. Numeric preconditions now have an immutable
[preflight path](numeric-preflight.md) before charging and literal binding.
Scope/hint/unit state also has [a preflight path](state-preflight.md).
Local/transfer/discard state now has [ownership preflight](ownership-preflight.md).
Exits now have [immutable runtime admission](exit-preflight.md), and literals
have [preflight admission](literal-preflight.md). Cleanup/aggregate helper element
charging for general aggregates and staged cleanup activations remain unfinished.
Current scalar scope retirement has [element charging](cleanup-element-fuel.md). Fault locations still use the current
recipe/PC architecture, with bound operation identity/activation integration in
the later dispatcher/tiering tasks. Calls, effects, JIT regions and long managed
helpers must adopt this shared meter without separate budgets or repeated charges.
The current successful scalar count must not be used as proof of those contracts.

Cancellation/deadline currently surface through the development runtime-resource
fault path with a distinct meter stop reason. Full service status/CLI exit3,
worker supervisor deadline/grace enforcement, broker indeterminate effects and
release telemetry remain later integrations. The tests do not prove those paths.
T031 original-source control-flow component is accepted; see the
[acceptance audit](t031-acceptance-audit.md). Full integration remains pending.
No release, JIT agreement, aggregate resource/performance/security or Linux
acceptance is claimed by this checkpoint.
