# Reproducible validation and disposable outputs

Use Python3.11 or newer for repository tools that read TOML (`tomllib`).
macOS's system `python3` may be older: choose a suitable interpreter explicitly
(for example `python3.12`) and use it consistently for provisioning/audits.
The pinned vendor audit is `python3.12 tools/bootstrap_vendor.py`, with no flag.

Git owns the inputs: implementation, unchanged licensed fixtures and their
provenance, selections, validation scripts, toolchain/dependency pins, and concise
evidence recording exact commands, source/binary hashes, outcomes and limitations.
`artifacts/`, `target/` and caches own generated outputs. A fresh checkout must
not require an old artifact folder to continue implementation or rerun a check.
System toolchains and dependency downloads are reconstructed from the checked-in
pins; executables and full dependency caches do not belong in Git.

The full optional upstream checkout is another reconstructible input, not an
artifact or a Git payload. The current licensed fixture manifest and metadata
probe use rust-lang/rust commit `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`.
For a machine without that checkout, provision it from the official repository:

```sh
git clone --filter=blob:none --no-checkout https://github.com/rust-lang/rust.git rustc/rust
git -C rustc/rust checkout --detach ae0fe511b15ab760d7e963b13389d1bc2ab42b18
rustup toolchain install 1.99.0 --profile minimal
cargo +1.99.0 fetch --locked
```

Run clone only when `rustc/rust` does not already exist; preserve any existing
changes. Selected runtime fixtures are already committed under their original
licenses and do not need the full checkout. The actual-library metadata probe
does need its library tree. Additional compile-time preparation uses the pinned
planner in `tools/cargo-planner.toml`; matched proc-macro server preparation is
reproducible with `python3 tools/prepare_macro_server.py --allow-tool-preparation`.
Preparation outputs are disposable; executing prepared macros retains the
existing separate authorization. These provisioning commands are documented
reproduction steps, not a claim that a clean-machine bootstrap was tested here.

Keep required helpers under `tools/`, never solely in a run directory. Capture
selections as small JSON files here or in the owning worktrack. Put durable
acceptance summaries in `worktracks/evidence/`, including failures and unsupported
contracts. A local raw-log path is optional detail, never the only description of
how to reproduce a result. Full raw logs may be removed after those facts are
recorded. If a future result needs large non-reproducible evidence, record its
hash and externally stored location; do not silently make a local path mandatory
or introduce Git LFS for build caches.

## Starting without an artifacts folder

Work directly in this checkout on macOS with Rust 1.99.0. Create output directories
as needed; their previous contents are not inputs. Use one Cargo owner and launch
builds/tests asynchronously, continuing independent work while they run. Collect
actual terminal results before the next Cargo operation or a passing claim.
Do not alter tested sources, generator inputs or HEAD while their users run.

The commands below describe the sequential build phases; launch each long command
in the background and collect its terminal before starting the dependent phase.
Choose a new run directory for every attempt. A custom `CARGO_TARGET_DIR` requires
passing the corresponding binary paths to the maintained runtime harness.

```sh
mkdir -p artifacts/macos-first artifacts/validation-current
python3 tools/validation/source_manifest.py artifacts/validation-current before
cargo +1.99.0 test --offline --locked -p lrr --lib --no-run --message-format=json > artifacts/validation-current/lib-build.jsonl 2> artifacts/validation-current/lib-build.log
cargo +1.99.0 build --offline --locked -p lrr --bin lrr > artifacts/validation-current/cli-build.log 2>&1
python3 tools/validation/source_manifest.py artifacts/validation-current after-build --cargo-messages artifacts/validation-current/lib-build.jsonl --cli-binary target/debug/lrr
python3 tools/validation/run_controls.py artifacts/validation-current tools/validation/selections/source-preparation.json
python3 tools/validation/source_manifest.py artifacts/validation-current after-runtime
```

`source_manifest.py` fingerprints source, fixtures, generators, tools, Cargo inputs
and HEAD. It discovers the actual library test executable from Cargo JSON rather
than retaining a previous hashed executable name. It refuses to overwrite phase
evidence and returns failure when inputs change. `run_controls.py --dry-run`
checks selection and input identity and prints commands without executing tests.
Host control success is component evidence, not an upstream runtime-contract pass.

For unchanged upstream runtime cases use the existing maintained harness:

```sh
cargo +1.99.0 build --offline --locked -p xtask --example lrr_source_digests
LRR_EMBEDDED_FACADE=1 python3 tools/rustc_checkout_runtime.py --lrr target/debug/lrr --source-digester target/debug/examples/lrr_source_digests --report-dir /private/tmp/lrr-source-preparation-runtime --case tests/ui/macros/concat-rpass.rs
```

The report directory must be new. Append `--case` for each selected original in
the tracked selection's `runtime_cases`. Build the digest helper before taking
the immutable build receipt for a combined run. The maintained harness validates
actual imported directives, owning editions, auxiliaries and runtime effects;
unsupported contracts remain explicit blockers. It never compiles a native
reference. Its selected report does not complete a suite, diagnostic, JIT or
release gate. Its existing harness bounds and missing contract adapters remain
separate limitations; this housekeeping change does not certify their compliance.

## Fast interpreter binary for validation runs

The `dev` profile compiles `lrr` at opt-level 0, so even `fn main(){}` spends
about five seconds of CPU in source preparation and a runtime batch is several
times slower than it needs to be. For validation runs (the maintained harness,
`tools/validation/regress_retained.sh`, selection batches) use the separate
`fast` profile, which optimizes `lrr`, `lrr-expand-adapter` and the vendored
solver crates (`opt-level = 2`; the heavy pure dependencies keep their
`[profile.dev.package.*]` opt-level 3) and keeps `debug-assertions` and
`overflow-checks`, so behaviour matches `dev`. Keep `dev` for day-to-day edit and
build: the `fast` build is a separate subdirectory of the shared target
(`target/fast`), takes minutes cold (run it in the background) and rebuilds
incrementally.

```sh
cargo +1.99.0 build --offline --locked --profile fast -p lrr --bin lrr
LRR_EMBEDDED_FACADE=1 python3 tools/rustc_checkout_runtime.py --lrr target/fast/lrr --source-digester target/debug/examples/lrr_source_digests --report-dir <new directory> --case <original>
tools/validation/regress_retained.sh target/fast/lrr
```

The harness takes any binary path and records its SHA-256, so a `fast` binary is
an ordinary receipt; build it from the fingerprinted source like any other.
Measured on one frozen source snapshot built both ways (user CPU seconds and
instructions retired, minimum of three runs, `LRR_EMBEDDED_FACADE=1`, edition
2021; October 21, 2026, aarch64-apple-darwin, loaded host):

| Program | `dev` | `fast` | CPU ratio |
| --- | --- | --- | --- |
| `fn main(){}` | 5.41 s, 53.1 G | 1.74 s, 20.8 G | 3.1x |
| `for i in 0..40u64 { sum += i; }` | 6.95 s, 64.8 G | 2.61 s, 24.8 G | 2.7x |
| 80 `add(a, b)` calls in a `while` loop | 8.03 s, 58.8 G | 2.40 s, 22.8 G | 3.3x |
| `tests/ui/union/union-generic-rpass.rs` | 15.55 s, 113.8 G | 5.19 s, 47.8 G | 3.0x |

That is about 3x less CPU time and 2.4-2.6x fewer instructions. `opt-level = 1`
gave 2.4-2.6x CPU; `opt-level = 3` and `codegen-units = 16` gave nothing beyond
`opt-level = 2`. Results are identical to `dev`: `regress_retained.sh` printed the
same 32 PASS lines for both binaries, and 175 of 176 unchanged upstream programs
(the round-3 selection plus every `drop/` and `moves/` run-pass program) produced
identical status, exit code, program output and opcode counters; the 176th,
`moves/move-3-unique.rs`, completes under `fast` but exceeded the 400 s per-run
limit of that comparison under `dev`. This is a speed and equivalence
observation for the tooling, not an acceptance result.

## Cleanup and historical recipes

After recording a terminal outcome and concise evidence, remove generated fixture
projects, comparison programs, proc-macro products and private Cargo targets.
Keep active outputs until their owner finishes. Do not store worktrees or unique
source in `artifacts/`; preserve and relocate any inherited repositories or source
before making the entire old directory disposable. Do not blindly delete a
directory containing an active owner's input or a Git repository.

`historical/2026-10-18/` preserves the exact formerly untracked helper scripts and
their hashes for inspection of old evidence. These are archival recipes, not the
maintained workflow. They contain historical executable names and narrower
observational checks; the old stock runner's `runtime_contract_pass` field alone
does not establish a directive-aware runtime-contract or full-fixture pass. Use
`tools/rustc_checkout_runtime.py` for new acceptance. Old outcomes retain their
original source fingerprints and scope; no old result is promoted by this move.
