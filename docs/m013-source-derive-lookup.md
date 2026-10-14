# M013 original-source derive lookup diagnostics

Original-source compiler annotation and derive export selection now distinguishes
settled ambiguity and inaccessible paths from absent or unfinished namespace
facts. The former produce `LRR-E-MACRO-AMBIGUOUS` and
`LRR-E-MACRO-INACCESSIBLE`; the latter retain the discovery suspension.
This applies to the empty-group proof and nonempty helper metadata selection.

Two coordinator regressions exercise duplicate imported derive aliases and a
private qualified derive alias against an actual prepared macro catalog. Both
failed before the fix because discovery stayed pending. After the fix, each
observes its exact diagnostic, zero native invocations, zero accounted expansions
and refused publication. These are LRR-specific namespace/authority controls,
outside the licensed upstream test directory. Nonempty original-source execution,
helper classification and ordered append integration remain pending. M013 is
still in progress.

`python3 tools/rustc_checkout_interpreter.py` separately exercises the unchanged
upstream `deriving-meta-empty-trait-list.rs` through the real CLI interpreter and
an explicit Rust 1.99.0 macOS ARM64 metadata reference. The harness checks imported
source hashes, retained licenses and its supported directive before execution,
records compiler identity and the LRR binary hash, and refuses changed source,
unsupported directives or unsuccessful interpreter execution. The guest source
is not compiled by LRR. Edition 2024 is explicit; this restricted harness is not
the upstream compiletest runner. The supplied checkout and pinned reference
compiler have different commits, both recorded in the report.

This records one unchanged whole-source interpreter observation and one native
metadata check. Full compiletest, guest JIT, tier equivalence, broader Rust
compatibility, release and Linux acceptance remain unproven.
