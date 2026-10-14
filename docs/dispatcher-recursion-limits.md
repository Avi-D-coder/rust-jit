# T033 dispatcher recursion and interruption — work in progress

Guest recursion remains on the dispatcher's owned frame vector. CallLimits now
sets independent frame and retained metadata limits before a run starts. Defaults
are 4,096 frames and 32 MiB of call metadata; supported configuration caps are
100,000 frames and 256 MiB. Invalid or late changes preserve the existing limits.
Neither setting replenishes fuel or clears interruption state.

Frame admission counts the root and every suspended/active guest frame. Exceeding
the configured depth reports LRR-E-RUNTIME-RESOURCE with guest frame limit exceeded
before sealing arguments or publishing the next call. The retained metadata
budget remains separately checked. A peak counter records actual entered depth.

The factorial control executes fact(20) with concrete i64 boundaries. The deep
control executes down(2000) on a 256 KiB host thread stack and records 2,002 guest
frames and 2,001 consumed call actions. It explicitly requests 128 MiB of call
metadata: the earlier 32 MiB attempt was rejected and is preserved as evidence.
A separate 64-frame quota control rejects the next call with 63 published/consumed
actions and remaining fuel. Configuration tests cover invalid and terminal changes.
Recursive cross-thread cancellation checks its resource status and exact-once
action counts, including stable counts after terminal polling. Existing shared-fuel
and deadline controls remain part of the workspace gate.

The stack-size fixture proves completion on the configured host thread stack; it
is not a sampled numeric native backtrace-depth measurement. Output/write effects,
user Drop, references/composites, generated/native/JIT/service interruption and
full revision-4 integration remain pending. T033 and release acceptance remain in
progress. Linux acceptance remains pending.
