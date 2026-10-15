# Reproducible cumulative interpreter candidate

This single source patch makes the current interpreter work reviewable from the canonical checkout. It folds the earlier byte/string packets, V98 source/context and bounded-startup repair, reached constant namespace preparation, final schema19 is_empty/contains candidate, and source-bound reached compiler string output with the pinned engine fixture alignment and checked indirect search activation storage. Build products, caches, source snapshots and local validation artifacts are excluded.

The packet is a candidate, not canonical integration. Final combined compilation, scoped execution and a fresh milestone remain pending. Prior V97 milestone failure, V98 scoped119 passes and V101 scoped103 passes have distinct frozen sources and must not be combined into a full-suite claim. V102 and V104 priority failures remain recorded; the V105 library1201 and seven string controls pass, but its bounded-stack oracle aborts. The reached-only storage repair is in background validation. The six historical plus five new distinct owning-edition CLI runtime programs also span different builds. Full compiletest fixture passes remain unestablished.

From the stated canonical source preimages on a separate branch, check and apply just this packet:

```sh
git apply --check worktracks/candidates/root-interpreter-cumulative-v1/candidate.patch
git apply worktracks/candidates/root-interpreter-cumulative-v1/candidate.patch
```

Do not apply its component packets again. SOURCE-AUDIT.json records every exact preimage and resulting source hash. It preserves task history/status and adds only required acceptance selectors, including the exact replaced H002 name. All imported originals remain unchanged under rustc's MIT OR Apache-2.0 licenses; both license texts remain in fixtures/upstream/rustc-checkout. New totals are509 raw files/324 pending case rows; imports and partial components are not passes.

Use macOS aarch64 and pinned Rust1.99.0. Validation is asynchronous, with one Cargo target owner and unchanged source during each run. After the accepted scoped controls, the required integrated command is `cargo +1.99.0 run --offline --locked -p xtask -- gate 03`. A fresh checkout needs its pinned dependencies available before using offline mode. Exact final validation source and terminals will be recorded separately when collected. Do not claim Linux, JIT, full runtime-suite or release acceptance from this patch.
