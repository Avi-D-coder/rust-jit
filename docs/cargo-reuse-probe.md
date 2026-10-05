# Cargo reuse experiment (H004)

The complete Section 45 experiment passed on Linux x86-64 with Rust/Cargo 1.99.0.
The final report is artifacts/probe/report.json. All command output, selected
artifact paths and content hashes, actual fresh flags, native outputs, and exact
input source files are retained. The unchanged repeat reports both units fresh.
The first bridge reuses a byte-identical baseline native library; wrapper edits
reuse that dependency; native edits rebuild both units and change scale(6) from
18 to 24. Deliberately invalid live Rust is never built. Live-only edits execute
the existing native library with zero Cargo calls and do not execute live source.

This is a scalar probe, not the production native ABI or interpreter. It provides
no throughput result, sandbox guarantee, or universal artifact compatibility claim.

The first bind-mounted run passed (attempt-001), but repeated runs exposed an
intermittent timestamp issue (attempts 002, 003, and 005 retain failed assertions;
attempt-004 passed). In attempt-005 Cargo itself reported:

```text
Dirty lrr_probe_bridge ... the file `lrr_probe_bridge/src/lib.rs` has changed
(1791162511.495756964s, 495756964ns after last build at 1791162511.000000000s)
```

The source was not written between those two builds. The probe added no compiler
flags, wrapper, delay, retry, timestamp rewrite, or suppressed freshness assertion.
Cargo's verbose output supplies the diagnosis; no private incremental data was read.

Replaying the identical experiment with its generated workspace under /tmp on the
container's Linux filesystem passed all assertions. That replay is preserved in
linux-filesystem/ and is the controlled gate evidence. This isolates the observed
failure to the development bind-mount environment; it does not prove that every
filesystem or Cargo configuration shares its behavior.

Use the original Section 45 command on Linux storage. When developing on macOS,
keep the probe workspace inside the Linux container and copy only its evidence
back to the repository. --trace-fingerprints adds Cargo diagnostics and verbose
Dirty/Fresh explanations without changing compilation flags.

Source module path handling was checked against the
[Rust Reference module rules](https://doc.rust-lang.org/reference/items/modules.html),
with a pinned-rustc witness confirming that an explicit external path override
uses the source-file parent for its child's default module search directory.
