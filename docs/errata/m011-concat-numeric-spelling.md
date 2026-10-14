# E-M011-002: pinned concat retains integer spelling

Minimal witness on Rust 1.99.0 macOS ARM64: `concat!(0x10)`. The actual pinned
syntax-only concat calculation returns `0x10`; the native executable prints `16`.
The extraction's source-proof regression confirms that its calculation is the
original archived algorithm, not an adapter replacement. Current native witness
artifacts are referenced by the M011 concat checkpoint.

The archived integer branch appends `Literal::text()` without semantic radix
normalization. This is an actual contradiction with the required native behavior.
It requires a narrow pinned-source correction plus native regressions for integer
radices, suffixes and relevant literal/error cases. The pinned correction now normalizes integer radices/separators through checked
u128 conversion and removes float separators. It validates the supported numeric
suffixes and reports integer overflow. Positive native cases cover signed values,
suffixes, binary/octal/hex/decimal, floats and u128 maximum; negative native cases
cover overflow and invalid integer/float suffixes. Complete literal/error semantics
and nested expansion remain pending. No alternate evaluator, source pin,
frontend, backend or relaxed language profile is authorized by this erratum.

The witness check passes only when it observes this mismatch. It is evidence of an
unrepaired defect, not a passing numeric acceptance fixture. String/character and
the sampled decimal/boolean component comparisons remain separately scoped.

The same archived-file patch now includes the two audited numeric branch changes
and normalization helper. The source-proof regression compares every other byte
of the archived calculation body. The runtime concat fingerprint domain advances
to numeric-v2 so pre-repair constant results cannot share its input identity.
