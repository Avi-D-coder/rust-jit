# M013: deadline propagation through procedural startup

Revision 4 Sections 50.6 and 57 milestone C, macOS ARM64 Rust 1.99.0 first.
M013 and release acceptance remain incomplete.

Invariant: native macro startup uses one absolute deadline across server version
probing, API probing, helper creation and library inventory. Every phase receives
only the remaining allowance. An expired phase reports LRR-E-MACRO-TIMEOUT;
it cannot yield a session, execute a guest macro or adopt an expansion output.
The separate macro authority guard precedes tool reads and deadline refusal.

ProcSession starts its configuration timeout once, bounds it by the run's supplied
absolute cap, and passes that cap to both handshake commands and the owned helper
process guard. Command timeouts retain their typed I/O outcome rather than being
flattened into server mismatch. Library inventory and final startup admission map
supervisor deadline termination to timeout. Reading the helper's termination state
uses a cloned process control after releasing the channels lock.
Normal server/compiler/protocol mismatches keep their existing diagnostics; no
alternate server, symbol loader or compilation path is introduced.

Four controls exercise version, protocol and inventory stalls plus expired-deadline
authority ordering. Stall fixtures wrap the recorded real matched server: successful
phases delegate to it, while only the selected phase executes a 30-second sleep.
Tests apply one fixed 4-second cap, require timeout within 6 seconds, observe the actual
stalled PID and verify ESRCH after constructor refusal. The wrappers do not forge
version or API replies. Their reports are startup failure evidence, not completed
guest procedural-macro or release fixtures. The existing matched-server execution
suite separately exercises successful native macro sessions and failures.

The pre-change version regression returned server mismatch after its old 10-second
probe deadline. That failure remains in m013-startup-deadline-before.log. The first full gate used a 700-ms test cap; under concurrent load, tool
verification expired before the selected stall and the required PID was absent.
That failed gate remains preserved. The explicit 4-second test cap allows startup
to reach the controlled stall while still requiring termination of its actual PID.
Production bounds are unchanged. The fixed
startup phases share the explicit cap and terminate/reap without automatic retry.

Synchronous preparation hashing and arbitrary parsing still lack process-boundary
preemption. Complete CLI/service supervision, all cancellation/resource paths,
impl/test/origin catalogue integration, broader cycles, body-position macros,
comparison assertion operands and full type/JIT/security/release/Linux acceptance
remain mandatory. These controls do not complete M013.
