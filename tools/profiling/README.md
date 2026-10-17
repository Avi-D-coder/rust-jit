# Developer CPU profiling (macOS)

`sample`, `lldb` and Instruments cannot attach in the sandboxed agent
environment. `lrr-sampled.rs` is an in-process SIGPROF frame-pointer sampler
that is **not** part of any Cargo target (it contains `unsafe` and is never
committed into `crates/`). To use it:

```bash
cp tools/profiling/lrr-sampled.rs crates/lrr/src/bin/lrr-sampled.rs
CARGO_INCREMENTAL=0 cargo +1.99.0 build --offline --locked -p lrr --bin lrr-sampled
rm crates/lrr/src/bin/lrr-sampled.rs
LRR_SAMPLE_OUT=/tmp/samples.txt target/debug/lrr-sampled --edition 2021 run FILE.rs
python3 tools/profiling/symbolize.py /tmp/samples.txt target/debug/lrr-sampled
```

`symbolize.py` symbolizes with `atos` (no process attach needed) and prints
self and inclusive percentages. For coarse stage/query timing without a
special binary, run the ordinary `lrr` with `LRR_PROFILE=1`
(`crates/lrr/src/core/timing.rs`).
