# T043 provisional component acceptance

The lexical scope, reached temporary borrow and source-context component is accepted
on macOS ARM64/Rust 1.99.0. This records the implemented component in the fixed
Section 57 order; it does not fulfill the complete T043 language contract.

Fresh integrated MAIN membership fd49c1fc4a4a84939131e9e0183dcfa66cc267961560d2ffdf950ba264525980
passed all 17 commands with runner session 19130 exit 0: 2,341 host bodies across
179 executables, no failed/ignored/filtered tests; all 41 T043, 73 T042 and separate
H002 controls and the 2,000-frame regression passed once. Seven unchanged stock CLI
probes passed, repeating existing observations. Upstream integrity verified all
157 cases/276 files at ae0fe511b15ab760d7e963b13389d1bc2ab42b18 with both licenses.

The first independent collector failed on a schema assumption about the CLI path.
The retained correction completed with actual exit 0 and no test or Cargo rerun.
Collection and release hashes bind the unchanged source, logs, fresh gates and CLI.

The component covers indexed lifetime storage, original local declaration order,
reached computed temporary borrows, selected ordinary-call barriers, simple initialized
ref/ref mut bindings, borrowed target contexts and lexical cast type admission.
Unknown constructor frontiers refuse explicitly. Positive source constructor/Fn-value
integration, arbitrary extending patterns, source-selected Drop transcripts, complete
edition/generated-source combinations and full fixture oracles remain required.
All final fixtures and dependencies remain explicitly pending in docs/tasks.json.
Full rustc compiletest passes, full T043 acceptance, release and Linux acceptance
remain false. Production JIT recording/lowering/transitions remain unimplemented.

[Independent collection](../artifacts/macos-first/t043-stock-namespace-integration-v15/current-main-milestone-v1/independent-collection.json).
