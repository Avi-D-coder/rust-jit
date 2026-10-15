# Combined interpreter successor for review

This single source patch reproduces 129 net source/test/fixture/ledger leaves from the recorded canonical preimages, folding the cumulative byte/string interpreter candidate and its assertion-constant, cast-context, local-match, opaque-guard/result-context and observer successors. It includes the real Type-preparation, test-diagnostic, static-owner-inspector and exact selector-history follow-ons. No build products, caches or complete source snapshots are included. SOURCE-AUDIT.json records the component hashes and every exact preimage/result hash; zero-offset/zero-fuzz replay is checked.

Apply only this combined patch on its exact canonical preimages; do not then apply the component packets:

```sh
git apply --check worktracks/candidates/root-interpreter-cumulative-v2/candidate.patch
git apply worktracks/candidates/root-interpreter-cumulative-v2/candidate.patch
```

This is a source candidate, not an integration. V108's full milestone failed with 3,126 project passes, four failures and 207 unstarted. Later scoped evidence is separate: V111 assertion-constant200 controls, V113 cast9, V116 local-match39; these are different immutable sources and are not additive. Complete unchanged const-fn-nested and issue-26251 mains have all-edition component evidence on their respective sources. The original cast main still stops at a nested reference; the original overlapping-match main remains a runtime failure at deferred-local inference after its first assertion. Its explicit partial host witness is not a full-main/upstream pass. Later statics and aggregate/pattern support remain required.

The expected-failure observer preserves every literal/regex output assertion. A retained panic message alone is incomplete evidence. Reclassifying the old115 captures preserves46/69 and920data refusal controls; no new guests ran in that scope. The final rebuilt CLI characterization, run-fail observations, final audits and fresh full milestone remain pending. No runtime-suite, full-fixture, task, Linux, JIT or release completion is claimed.

Use pinned Rust1.99.0 on macOS aarch64 first. Keep final validation source unchanged, run tests asynchronously under one Cargo target owner, collect actual terminals, and require the fresh final `cargo +1.99.0 run --offline --locked -p xtask -- gate 03` before canonical promotion. Original rustc inputs remain unchanged and licensed MIT OR Apache-2.0, with both license texts preserved. This does not permit guest JIT before complete applicable upstream runtime semantics and interpreter correctness acceptance.
