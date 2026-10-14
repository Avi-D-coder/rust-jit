# H002 native baselines before VM lifetime changes

The eight fixture directories E2401–E2407/E2410 contain original Rust source,
Section28 contracts and actual checked native stdout. The explicit native
preparation uses Rust1.99 on macOS, edition2024, cfg(test), opt-level0, overflow
and debug checks, and panic=unwind. It removes exactly the frozen root test-control
annotation and appends a native-only wrapper. The wrapper suppresses the native
panic hook and writes completion/panic status separately from source stdout and
stderr. It is never fed into LRR. This is a fixture-specific controlled wrapper;
it does not substitute for M043's general oracle/harness implementation.

Host regression controls repeat native compilation and execution, require the
real child success and separate completion file, compare complete stdout/stderr,
and assert the normative cleanup relationships and exact-once event counts.
Compiler diagnostics and libtest framing are not compared to source output.
Native records use runner=native_rust, scope=native_only, full_lrr_passed=false.
No pending LRR fixture is marked passed from these comparisons.

E2405 has a labelled valid native counterpart with a later function that would
print a forbidden event if reached. Its relaxed source retains the required
undefined later call and marker. Rustc cannot provide a native-pass oracle for
that intentionally relaxed source. Its LRR no-preparation/evaluation assertion
remains mandatory. E2410 supplies migrated narrowing baselines; interpreter versus
forced-JIT/drop/effect equivalence remains entirely pending.

The current VM has no completed H002 condition-clause or tail-temporary machinery.
H002 is in progress. Add and exercise guest success/failure scopes, binding
visibility, cleanup ordering, let-extension and fresh while-clause storage before
claiming runtime acceptance. Object Drop, references, patterns, output capture,
generated sources, actual JIT tiers and full fixture grading retain their named
later ledger requirements. macOS is primary; Linux acceptance is deferred.
