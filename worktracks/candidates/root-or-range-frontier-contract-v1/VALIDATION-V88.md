# V88 scoped validation: passed; full milestone pending

On macOS `aarch64-apple-darwin`, pinned Rust 1.99.0 (`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`), the exact one-leaf packet passed strict Clippy and the entire unfiltered `scalar_or_patterns` bank: **7 passed, 0 failed, 0 ignored, 0 filtered**. Actual orchestrator terminal `9a1549/0`; collector `bc802f/0`; seal `446501/0`. Root independently checked retained stdout/stderr hashes against the collected terminal records.

This evidence uses frozen source `464a6bc4ec7e08d0f75085792dde2d9ec647ad677e1ebf847e69c1fb268e2e5a`, the exact V87 baseline plus this repair. It is not evidence for an unmodified checkout of the publication commit. The 4,337 default source files, 435 explicit inputs, 501 raw upstream originals and sole target-owner record remained unchanged throughout each command. No string candidate was applied.

Commands ran in the frozen source with the existing target, `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, `RUST_TEST_THREADS=1`, dev/test opt level 0, debug info 0, debug assertions enabled and incremental disabled. Rust flags, wrappers and stack overrides were removed; no alternate toolchain, target or native compilation was introduced by this repair:

```sh
cargo +1.99.0 clippy -p xtask --test scalar_or_patterns --locked --offline -- -D warnings
cargo +1.99.0 test -p xtask --test scalar_or_patterns --locked --offline -- --nocapture --test-threads=1
```

Strict command elapsed 8.373 seconds; test command elapsed 16.285 seconds, with 0.09 seconds in the bank. These are validation timings, not interpreter performance measurements. The supported range lower-bound test, binding/guard refusals, source ordering, cold-body and typed no-arm controls all passed.

V87 remains an actual failed full milestone: 2,866 host test bodies passed, one failed, 421 were unexecuted. These are not stock rustc full-fixture counts. Its historical failure is not converted into a full-gate pass by this scoped repair. A fresh integrated milestone remains required before canonical byte-candidate promotion. No task, interpreter acceptance, full-suite compatibility, JIT or Linux completion is claimed.

The narrowly bounded scope began above 3 GiB free and retained at least 2 GiB; the separate full-gate floor remains 6 GiB. Closing free space was 3,852,312,576 bytes. No broader gate launched under this scoped resource policy.

Local retained evidence (validation artifacts stay outside Git):

- `artifacts/macos-first/root-stock-local-numeric-validation-v1/agent-validation-v88/HANDOFF.md`, SHA-256 `abf9705e24ec793fda80b5bc0bd5c231298176a0cde272c09967a01cbfbef40d`.
- `COLLECTED-SCOPES.json`, SHA-256 `58c020eb8c275642aeaed6f2a60b21768c0a269f68510c3b4be22ed73f1d8930`.
- `EVIDENCE-HASHES.json`, SHA-256 `60f3eec038c54bf8ba2c638e21e52f936da34d313021230fd2bc5fcbd657dd7d`.
- `RELEASE.json`, SHA-256 `8a104b306a51d486dcd05931f4965a754f02b930e5917ca9aedd17143356fdef`.
