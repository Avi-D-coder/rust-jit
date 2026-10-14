# Integrated T042 aggregate Copy component

Copy V3 passed538 scoped bodies/all73 T042 controls on unchanged source, strict
Clippy and architecture. Formatting is supported by V2 whole-workspace and V3
changed-file checks. Two exact known test-output files explain V3's retained
outer source-audit failure; canonical source inputs remained unchanged.
Direct CLI probes of both new original programs exited2 on source/type-resolution
gaps. No full stock fixture, full T042, JIT, release or Linux pass is claimed.
The following frozen candidate description is retained as historical context.

# T042 structural Copy candidate — unvalidated

This private candidate implements closed Copy tuples/arrays through iterative
private destination guards and canonical frame activation. It preserves each
owning source, allocates distinct objects and field cells, shares only closed
immutable reference leases, and publishes one result ticket after cached
completeness. Nominal Copy/Drop policies remain source-selected and pending.

Opcode entry is charged once. Each source field charges fuel; every transition
polls cancellation/deadline. At most256 field/guard transitions execute per
Dispatch, including deep guard completion. The bound PC and operation index
retain progress and prevent repeating completed recipe prefixes. Result storage
stays Uninit during partial progress. Source epochs and current owning ancestry
are revalidated before each chunk transition. Fatal faults physically reclaim the
arena guards without guest code. The activation stack shares the frame's32MiB
metadata bound; destination vectors/bitsets and arenas charge actual capacity.

Primary language inputs are unchanged official rustc tests at
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`:
`tests/ui/tuple/tup.rs` and
`tests/ui/array-slice-vec/fixed_length_copy.rs`. All source values and read ranges
come from their AST. The tuple context explicitly supplies the written point
alias's isize fields. The array context explicitly supplies i32, without claiming
whole-source numeric inference. Earlier destructuring, alias propagation, later
calls/assertions, complete cleanup and whole compiletest contracts are unexecuted.
Thirteen LRR controls supplement the original reads with mutation independence,
257-element chunk stress, fuel/cancellation, quota, stale epochs, bound identity,
400 nested guards, shared leases and a verified address/read prefix adaptation.
Runtime graph expansions and reference substitutions are disclosed harness setup,
not modifications of the copied tests or additional stock fixture passes.

No tests or builds of this candidate have run yet. T042 remains in_progress;
full stock fixture passes, guest JIT, source Drop, release and Linux remain false.
