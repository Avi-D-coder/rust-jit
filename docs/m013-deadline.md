# M013: shared expansion deadlines

Revision 4, Sections 50.6 and 57 milestone C; macOS ARM64 Rust 1.99.0 first.
M013 and release acceptance remain incomplete.

Invariant: the run-owned expansion ledger freezes a cumulative 60-second wall
deadline at first work. Each external invocation receives at most 10 seconds,
capped by the remaining cumulative allowance. Explicit reductions are accepted
before work; zero, excessive and late configurations are refused.

Discovery checks the shared deadline before polling, output admission and database
publication. Accounting checks again after integration and before charging. Expired
work cannot publish a seal or become an admitted successful output. This is a
boundary check; it does not preempt arbitrary in-process parsing or undo private
source staging completed during a transition that expires.

MacroRun obtains a fresh per-invocation absolute cap from the shared ledger. It
checks again after preparation verification, refuses an expired startup as a timeout,
and passes the absolute deadline to startup. The matched external session caps invocation
I/O by the absolute deadline through the existing owned process supervisor.
Startup time does not become the permanent timeout of subsequent invocations;
their caps are freshly derived from the same cumulative ledger. A
failure closes the run and sessions without retry, adoption or a seal certificate.
The preparation file hashing itself is synchronous and not preempted by this cap;
complete CLI/service supervision and classification of failures during
helper startup remain required.

Four acceptance controls cover late output refusal and frozen configuration,
maximum bounds, actual matched-server termination for a hanging proc macro with a
4-second cumulative / 6-second invocation reduction, and resetting the session cap
for a second successful invocation after the first cap has elapsed. The hanging
helper must report LRR-E-MACRO-TIMEOUT, execute once, leave database revision and
adopted-output count unchanged, refuse retry, and fail final input certification.

The original 700-ms native attempt is preserved as a failure: preparation checking
consumed that allowance and a zero-duration session was classified as output failure.
The regression now exercises native execution with a longer explicit reduction;
the missing post-verification timeout boundary was fixed independently.

Full impl/test catalogue plumbing, origins, all cancellation/resource paths, full
CLI/procedural integration, broader cycles, type/JIT/security/release and Linux
acceptance remain mandatory. These controls do not complete M013.
