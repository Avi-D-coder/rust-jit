# H004 Cargo reuse probe

This standard-library-only Python experiment tests Section 45 of the specification.
It compiles a native library, adds a workspace-member cdylib, checks Cargo's fresh
flags and artifact bytes, loads the reported library in disposable children, and
repeats after live-only, wrapper, and native edits. Deliberately invalid live Rust
is never selected for compilation. It does not run live Rust or implement LRR's ABI.

On Linux x86-64 with the pinned toolchain:

```sh
python3 tools/cargo-reuse-probe/probe.py --allow-native-build --work-dir /tmp/lrr-reuse-check
```

Use a new or empty work directory. The explicit flag permits both native build
and generated library execution for this controlled experiment. Cargo runs offline;
build tools and library execution have ambient authority. Raw command output,
artifacts, hashes, toolchain, source inputs, and failures are retained under evidence/.
A failed assertion returns nonzero and retains its report.
