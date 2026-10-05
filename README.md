# LRR (development)

This repository implements the fixed architecture in
[lrr_rust2024_hybrid_cargo_implementation_spec.md](lrr_rust2024_hybrid_cargo_implementation_spec.md).
It is under construction. The guest interpreter, production JIT, permission
broker, Cargo-native ABI, service, and release fixtures are not implemented yet.
The binaries fail explicitly rather than report a successful guest run.

The bootstrap includes an edition-2024 syntax-only parser probe, actual Cranelift
native-code and registered-helper probes, a child-process ambient-symbol rejection
probe, exact direct dependency pins, the narrow vendored JIT patch, an architecture
audit, and a complete task ledger. These are infrastructure checks, not guest
execution or release evidence.

Use Rust 1.99.0 on Linux x86-64:

```sh
cargo +1.99.0 test --workspace --locked
cargo +1.99.0 run --locked -p xtask -- gate 00
```

The gate writes artifacts/bootstrap.json and artifacts/gates/00.json. Failed or
unimplemented gates return nonzero. docs/tasks.json records remaining work.
No benchmark or sandbox claim is made by the bootstrap.

A macOS ARM64 development host can run the release-target checks in an emulated
container:

```sh
docker run --rm --platform linux/amd64 \
  --mount type=bind,src="$PWD",dst=/workspace \
  --mount type=volume,src=lrr-rust199-cargo,dst=/usr/local/cargo/registry \
  -w /workspace -e CARGO_TARGET_DIR=/workspace/target/linux-x86_64 \
  -e RUSTUP_TOOLCHAIN=1.99.0 rust:1.99.0-bookworm \
  cargo run --locked --offline -p xtask -- gate 00
```

On the first container invocation omit --offline to fetch the locked packages.
Emulated execution does not establish native x86-64 performance targets.
The runtime rejects macOS/ARM64 startup with LRR-E-HOST-TARGET.

The Cargo reuse experiment is implemented in tools/cargo-reuse-probe/probe.py.
Its full passing Linux-storage evidence and the retained bind-mount failures are
explained in docs/cargo-reuse-probe.md. Stable identity and immutable source-capture
components are also implemented internally; milestone 01 is still in progress.
