# Ordinary integer literal bit patterns

Root, LOCAL-043, WT02/WT04, T023/T025/T032. The unchanged upstream
`tests/ui/numbers-arithmetic/i128.rs` explicitly allows overflowing literals and
compares a cast i64 with `-0xFEDC_BA98_7654_3210`. Current V9 whole-main execution
fails VALUE-REPR before the first black_box call. Its retained-body range lacks
physical source mapping; the particular failing occurrence is not yet proved.
Source inspection independently proves this legal narrow literal is refused by
`GuestStore::integer_literal_bits` and `preview_integer`.

Invariant: once ordinary source constraints select a primitive integer Type,
literal materialization uses that Type's Rust bit pattern. LRR does not enforce
overflowing_literals or other static lints. Checked runtime arithmetic is a
different operation and keeps its overflow behavior.

Shared route: actual indexed literal -> LiteralRecipe/HintRecipe -> VM numeric
materialization -> GuestStore::literal_value -> preview_integer (narrow) or
wide_literal_value (128-bit). Both consume integer_literal_bits. Replace narrow
magnitude refusal/host i64 conversion with width masking and signed extension;
do not add another evaluator, parser, source-origin condition or assertion rule.
The same route serves handwritten and expanded syntax. Unsigned folded unary
negation remains an operator error; integer lexical magnitudes still fit u128.

Only runtime/cell.rs and runtime/cell_wide_integer.rs change. Runtime records,
ownership, continuation, cleanup and exact selected Type identity remain intact.
No cold-body demand, parse, planner, namespace scan, immutable-body copy, resource
cap, native execution or authority change. Constant-time native integer bit work
per materialization; no performance claim. No store-independent 128-bit handle
is invented: wide values keep their existing checked arena storage.

Acceptance: sole validator runs the unchanged whole i128 program on a new exact
build and relevant existing scalar controls, asynchronously. A later failure is
not a whole-program pass. All eight recurring architecture failures were reviewed
against these two shared functions: none is introduced; the existing macro and
pattern routes are not altered. Exact source hashes and actual terminal evidence
must be recorded after the build, before any acceptance claim.

## Exact scoped source and runtime evidence

- `crates/lrr/src/runtime/cell.rs`: `3ad0d4d64b01079280c0523037b26feb2281023afa949fd7b95fcfd64ad41335`
- `crates/lrr/src/runtime/cell_wide_integer.rs`: `6faf900ece091f8c17b3c87a0e6b78d52542b05a5e84c0020e6a992b02158222`

Independent source review receipt e9116d06fe0d31857ac71b621925632d3382b837261f8bfd5a336551bf84ee05. V10 both builds pass unchanged source, six relevant controls pass. Whole i128 advances from zero to 81 source calls and 31,833 VM dispatches, then fails a different shared reference/nominal contract. No full i128 pass or physical mapping of the former literal fault is claimed. [Exact V10 evidence](2026-10-18-interpreter-v10-validation.json).
