# E-M015-017: the Test marker must retain its selected compiler action

M015 requires final test metadata after exact Attribute-kind resolution. A
compiler `derive` definition imported as `test` cannot be certified as Test by
its spelling. The former fast paths checked prepared procedural Attribute
candidates but omitted the selected compiler descriptor's action.

An actual CLI probe on the frozen main build listed `should_not_register` for
`use core::prelude::rust_2024::derive as test; #[test] fn
should_not_register() {}`. Exact Test and FunctionLike contrasts also listed
correctly. Native Rust1.99 on macOS ARM64 rejected the derive variant with E0774.
The commands, source/executable hashes and observations are retained in
`artifacts/macos-first/m015-compiler-test-action-cli-probe.json` and the separate
`m015-compiler-test-action-native-reference.json`. Native reference preparation
compiled no selected LRR live runtime unit and executed no resulting program.

The stock source `tests/ui/test-attrs/issue-20823.rs` supplies the unchanged public
test body. A single explicit namespace import is an LRR adaptation outside the
licensed source. Its strict CLI regression failed with exit0 instead of required
discovery failure, on 3388 fixed private source inputs and membership. The exact
adapted stock-body native reference again gives E0774. Receipts are
`m015-compiler-test-action-baseline-witness.json` and
`m015-stock-public-test-action-native-reference.json` in artifacts/macos-first.

The private correction consults exact compiler descriptor/action and procedural
Attribute metadata in original/generated/cold Test certification, prefix
consumption and seal replay. Non-Test Attribute bindings retain their own handler
and cannot become Test metadata; legitimate Test and bang contrasts remain
eligible. Checked identity validation and revised Test-proof encoding domains
preserve source/action provenance. The correction performs no macro execution,
query-provider build, native symbol lookup or live consumer compilation.

Private core V1 passed202 library and29 CLI controls, including unchanged stock
execution and the required alias/filter refusals, but failed one generated
empty-derive compatibility control. No complete V1 acceptance is inferred;
remaining checks were unexecuted. V2 validation is running with fixed source.
M015 remains in progress; full MX031/MX032, compiletest, JIT, release and Linux
acceptance remain incomplete. All remaining work is owned by the primary agent.

Private V2 behavior controls pass, including the stock-body action/filter refusals and all eight generated-empty-derive controls, but strict Clippy failed an obsolete unused method. This remains a partial result; the collected failure is preserved. The removed private wrapper changes no action semantics; V3 source is frozen for a complete rerun.

Private V3 passed all420 controls, formatting and strict workspace Clippy on3395 fixed source/include inputs and membership. Same-source native/provenance retry V2 passed all30 controls plus importer and unchanged stock empty-derive interpreter checks; earlier linker ENOSPC is retained and had no test execution. The correction is integrated, with fresh full macOS milestone pending: artifacts/macos-first/m015-stock-test-action-integration.json. Full fixture/release/Linux acceptance remains incomplete.

Fresh integrated macOS component milestone passed all nine actual child checks and2080 host/doc tests; all132 named M015 controls observed. Source3539 bytes/membership unchanged; parent exec exit explicitly unobserved after truncation, all actual child results/no-live-driver/independent final source check retained. Receipt: artifacts/macos-first/m015-stock-test-action-main-acceptance.json. Full fixture/release/Linux acceptance pending.
