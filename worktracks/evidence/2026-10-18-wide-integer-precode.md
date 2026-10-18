# Wide integer shared-runtime prerequisite

Owner root; WT02/WT04, T023/T025/T032. This is required by the October 17
AGENTS override (everything rustc compiles must run), superseding the older
Section 5 live-profile i128/u128 exclusion. It does not widen the native ABI;
Section 61 native-signature exclusions remain separate. No new architecture or
exception to E-R5-005 is authorized by this record.

Actual primary witness: unchanged `tests/ui/consts/const-int-arithmetic.rs`,
V6 exact source `3d5608832ad2a0610c3d3ff5be25d9657825fe0e058993e1cd1cd8eaacb8736e`,
fails after 60 source calls/8,231 dispatches at the live-profile i128 refusal.
The unchanged already-imported `tests/ui/numbers-arithmetic/{i128,u128}.rs` and
`i128-min-literal-parses.rs` cover the underlying language operations. No fixture
modification, skipped i128 branch, pure-evaluator arithmetic detour, truncated
u64 result or method-specific expected answer is allowed.

Invariant: a selected numeric Type has its full Rust width through literal
materialization, primitive operators, casts, storage, references, Copy, formatting
and ordinary library calls. Preserve the 16-byte ValueRepr ABI. 128-bit values
need checked immutable worker/run-local backing; their payload is a checked arena
handle, never the low 64 bits masquerading as the complete value. Reuse existing
literal backing/handle epochs for immutable wide bits; use one ordinary numeric
operation pipeline for every origin. No boxed aggregate identity/destructor or
new evaluator is appropriate for a Copy primitive.

Existing route: owning syntax/hygiene -> primitive Type/constraint/signature
services -> common LiteralRecipe/HintRecipe and NumericBinary/Unary/Cast LRIR ->
vm::step -> vm::numeric -> GuestStore scalar cells/tickets -> ordinary call
contracts and formatting. Generated/user/wrapper/standard macros already join
ordinary indexed nodes. Extend primitive identity/descriptors and scalar backing,
then the same numeric consumers. Remove explicit i128 refusal only when its
replacement path is sound. Keep unimplemented consumers held explicitly.

First bounded slice: runtime/value.rs kind code, runtime/shape.rs primitive
identities and reserved descriptors, runtime/cell_wide_integer.rs immutable backing
and checked bit access, runtime/cell.rs scalar validation/read/Copy/cleanup joins,
frontend/call_contracts.rs mapping delegating PrimitiveIdentity. Dynamic descriptor
indices must be offset beyond added reserved primitive IDs; never collide with
worker-local nominal descriptors. Width/overflow guards must avoid host shifts by
128 and signed MIN negation. Copies share immutable backing, no guest drop glue.

Follow-on integration (not closed by a helper): literal decode/materialization,
VM operator and cast execution, source integer-library generation and constants,
format/hash/byte paths, reconstruction/native boundaries. Worker callable slice
currently owns step/context/frame/run.rs; root will join execution only after
ownership release. Exact files and source-aware all8 review are required before
validation/integration. No new count, byte, fuel/time limits or sandbox checks.

Cold cost: reserve primitive descriptors once and immutable numeric backing per
materialized wide result. Hot operation: constant-time checked handle lookup and
native typed i128/u128 operation after exact source/runtime Type selection; no
parsing, source planning or namespace census. OS bounds allocation. No measured
performance claim; backing retention lasts for the run just as literal records.

Acceptance remains actual unchanged whole upstream executions and existing
arithmetic/literal/storage regressions, with guest panic and effects preserved.
Source-only/helper/compile success is intermediate evidence, never an i128 feature
pass or a completed interpreter/JIT gate.

V7 actual frontier: both builds pass; unchanged i128 MIN whole main completes,
Vec/closure original completes after common result-template fix. Constarith now
reaches63calls but lrr_wadd128 entry fails certified written root hint: shared
syntax::thunks::hint recognizes only narrower primitives, emitting the nominal
DefinitionWrittenResult fallback for actual u128. Extend that same syntactic
primitive classifier (still no namespace permission) and ordinary runtime builtin
argument source mapping to I128/U128. No loosening of return/source proof checks.
Files syntax/thunks.rs and engine/run.rs are released from callable worker.
