# Official rustc test sources

This directory is licensed under the original rustc **MIT OR Apache-2.0** terms.
See [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE), and the copied
[COPYRIGHT](COPYRIGHT) notice. Copyright belongs to The Rust Project Contributors.
Original per-file notices remain in place.

The files under `tests/` are unchanged copies from the user's local official
checkout at `rustc/rust`. [manifest.json](manifest.json) records the exact upstream
commit, Git blob identities, SHA-256 hashes, sizes and directives. Helpers and
expected-output snapshots accompany each selected case. LRR harness code and
adaptations belong outside this directory.

Reproduce or extend the selection with:

```sh
python3 tools/import_rustc_tests.py tests/ui/proc-macro/derive-expand-order.rs
```

Importing a case is not an execution pass. Component-level parsing of a case
does not certify its procedural execution, runtime behavior or full compiletest
expectation. Follow the current M013 evidence for exercised scope; the manifest
keeps full native and LRR acceptance pending.
