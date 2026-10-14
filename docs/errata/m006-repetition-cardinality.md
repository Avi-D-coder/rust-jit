# E-M006-001: unequal declarative repetition cardinalities

Pinned upstream commit `bcf0514497034e1156f3cc2bb72b99cc9634a949` silently
truncates this transcription to its first pair:

```rust
macro_rules! pairs {
    ($($a:literal),*; $($b:literal),*) => { [$(($a, $b)),*] };
}
fn main() { let _ = pairs!(1, 2; 3); }
```

The pinned backend emits the literals `1, 3` successfully. Rust 1.99.0 on macOS
ARM64 rejects it because `a` repeats twice and `b` once. That contradicts MX008
and Rust's repetition cardinality rule. The failing backend regression and native
oracle witness are preserved in `artifacts/macos-first/m006-cardinality-pinned-witness.log`
and `artifacts/macos-first/m006-cardinality-native-witness.json`.

The correction retains the pinned matcher and transcriber. Each active repetition
axis observes the lengths of the nested bindings used at that axis and reports a
binding error when they disagree. It also checks the mismatch before normal
end-of-repetition handling, so zero-versus-nonzero bindings cannot silently pass.
Nested axes have independent state for each outer iteration; this preserves
legitimate ragged inner repetitions. No replacement frontend, pattern matcher,
type checker or native compiler is used in production expansion.

`vendor/mbe-cardinality.patch` is the exact source patch. The original upstream
archive and original inventory remain unchanged. `vendor/UPSTREAM.toml` records
patch, preimage and postimage SHA-256 values; bootstrap audit reads the preimage
from the pinned archive, applies the exact hunks in memory, and compares the
computed postimage to the working source tree. Unknown modifications still fail
source provenance validation.

Regression: `pinned_declarative_repetition_preserves_order_and_rejects_cardinality_errors`.
Expanded-output value/order, zero-length and ragged-nested cases remain part of
M006 acceptance work. This erratum corrects the pinned backend's behavior to the
existing revision-4 language contract; it does not change the contract.

## E-M006-002: fixed iteration cutoff and missing repetition driver

The same pinned transcriber has a fixed 65,536-iteration recovery cutoff. A
65,537-literal repetition is well below revision 4's generated-token and wire
limits. Rust 1.99.0 compiles and executes it, reporting all 65,537 elements;
the pinned backend returns `LimitExceeded`. The native source, command and
execution receipt are retained in
`artifacts/macos-first/m006-repetition-cutoff-native/`; the failing backend test
is `artifacts/macos-first/m006-repetition-cutoff-witness.log`.

The audited patch now removes this unrelated fixed cutoff. Termination follows
the matched binding lengths already tracked per repetition axis. A repetition
without a driving nested binding is a typed binding failure, rather than a loop
that emits copies until a recovery cutoff. This also covers a nonrepeated
fragment incorrectly placed inside a repeated transcription. Empty bindings
still terminate normally, and nested axes retain their separate lengths.
`Count` binding traversal records its nested lengths consistently with ordinary
fragment traversal. The selected matcher/transcriber and dependency pin remain
unchanged; the combined exact patch receipt reconstructs the modified file from
the original archived preimage.

Regressions: `legal_repetition_above_old_backend_cutoff_preserves_every_binding`
and `repetition_without_a_driving_binding_refuses_before_repeating_output`.
This fixes iteration semantics only. Backend output allocation limits are still
pending and this record does not claim M006 completion.

Current combined transcription patch also includes [E-M007-002 marked span/ancestor admission](m007-marked-byte-preflight.md). Original archived preimages and historical checkpoint receipts are preserved.
