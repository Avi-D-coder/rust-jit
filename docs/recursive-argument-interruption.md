# T033 recursive argument interruption — work in progress

The dispatcher now accumulates each caller's completed opcode-counter prefix at
suspension and resets only that frame's accounting prefix. Its guest values,
continuation, fuel and interruption state remain unchanged. Work after resume is
counted as a new prefix. The run-wide opcode_counters snapshot therefore includes
work in suspended callers and completed callees without counting either twice.
Call-limit mutation is guarded by the run's consumed fuel, so a reset accounting
prefix cannot make an executing run appear unstarted.

The argument-interruption control performs one local assignment in the first
written argument and enters unbounded source recursion. Fuel exhaustion and
cross-thread cancellation each report their actual resource status, retain one
assignment in the run-wide counters, and never query the later argument's callee
signature. All published call actions have been consumed; terminal polling leaves
the counters unchanged. The successful-resumption control executes two assignments
and two calls, verifies the actual result, and checks report/snapshot consistency.
Existing factorial, 2,002-frame bounded-host-stack, frame/metadata limits, deadline,
shared-fuel and recursive cancellation controls remain required by the full gate.

This exercises scalar assignments and call publication. It does not establish
stdout/write effect replay, user Drop, reference/composite transfers, generated
suspension, native/JIT/service interruption or complete revision-4 release
acceptance. Those requirements remain pending. T033 remains in progress, and
Linux acceptance remains pending.
