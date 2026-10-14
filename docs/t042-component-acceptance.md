# T042 ownership component acceptance

T042 has component acceptance under Section57/E-57-001. Full T042 acceptance
and release_complete remain false. The fixed order places nominal source-policy
and guest destructor integration after this ownership component.

The implementation preserves one owner for String/mutable-reference and complete
non-Copy structural moves; intrinsic scalar/shared-reference reads copy. Closed
tuple/array Copy uses distinct owning field storage and bounded resumable guards.
Partial moves retain siblings and cached initialization; reinitialization never
revives old location epochs. Assignment checks aliases, cleanup admission and its
complete reservation batch before consuming the RHS. Deferred nominal policy is
never treated as Copy or NoDrop.

The current macOS MAIN gate passed2308 project test bodies with no failures or
ignored tests, including all73 T042 controls exactly once; gates00–03, strict
Clippy, formatting and architecture passed on unchanged3737 source inputs.
These controls are included in2308, not an additional count.

The original integration helper did not record its CLI runtime hash, and Cargo
selected a different binary from the preceding fresh build. Its pass remains an
observation with that limitation. A separate hash-verified fresh executable ran
the same four arithmetic originals (all29 original assertions), move-arg and Unit;
all six child exits were0. The suffix driver exited1 during a terminal-writer
NameError after all children completed; independent read-only collection exited0.
The original failure and unknown helper hash remain recorded. This is segmented
evidence, not a claim that the original CLI identity became known.

[MAIN collection](../artifacts/macos-first/t031-t034-t040-t041-t042-copy-main-milestone-v1/independent-collection.json)
and [fresh CLI suffix](../artifacts/macos-first/t042-fresh-cli-identity-controls-v1/independent-collection.json).

The task ledger retains ten full-integration fixtures as pending and every later
dependency. Mandatory remaining behavior includes selected nominal Copy/Drop
impls, certified NoDrop partial moves and exact PARTIAL-MOVE-DROP diagnostics,
ordinary field cleanup, full tuple/array source resolution and original-program
execution, auto/force JIT comparisons and the final fixture matrix. Unsupported
policy refusal is never counted as a positive trait result.

Official inventory remains145 cases/252 unchanged files with original licenses.
Twelve earlier/current program observations exercise53 original assertions under
the explicit Rust2024 adaptation; full rustc compiletest fixture passes remain0.
The post-transition exact task-ledger test passed once (1 passed,24 filtered),
with independent collection and unchanged source. T043 follows in fixed order.
