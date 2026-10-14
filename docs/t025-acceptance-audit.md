# T025 numeric component acceptance audit

Current H002 statement-scope integration adds ordinary scope entry/retirement.
The normative main now executes 12 operations and uses exactly 23 fuel; 22 fuel
faults. Eight-operation and 19/18-fuel figures below describe the historical
checkpoint. See [the current boundary integration](h002-statement-scopes.md).


Revision 4 Section 27 T025 and Sections 11.1–11.6 define this component.
Section 57 places it before T030. Acceptance uses the fixed-order component
rule in [E-57-001](errata/e57-component-integration-order.md); completion of this
component does not prove its later control/call/generated/trait/JIT integrations.
The final checkpoint, rather than this document alone, proves executed checks.

| Requirement | Implementation and exercised evidence |
| --- | --- |
| Every supported concrete numeric primitive | `vm/numeric.rs` enumerates matched i8/i16/i32/i64/isize/u8/u16/u32/u64/usize/f32/f64 operations; bool bitwise/comparison and char/unit comparison use explicit entries. Missing pairs return typed faults. Guest i128/u128 remain excluded. |
| Exhaustive i8/u8 arithmetic | `exhaustive_i8_and_u8_arithmetic_matches_widened_mathematical_oracle` checks 655,360 operand/operation pairs against widened mathematical range results, including zero division and signed minimum divided/remaindered by -1. |
| Random wider arithmetic and casts | The seeded arithmetic control checks 40,000 cases. `seeded_wider_casts_match_mathematical_modulo_and_signed_interpretation` checks 100,000 source/destination pairs using independent i128 modulo and signed interpretation, including distinct word descriptors. Cast boundary/saturation/float controls add fixed edges and a double-rounding witness. The wider oracle types are test-only. |
| Explicit contexts and strict unresolved literals | Source controls exercise primitive local/return/other-operand hints, shared binding, no future/default inference, comparison operand context and casts that cannot infer their source from their target. Width-sensitive operations never execute as universal numbers. Reservation/range failures do not bind the shared literal record. |
| Signed minimum syntactic pair | Source minimum controls construct signed minima directly, including parenthesized pairs; negating an already materialized minimum invokes the checked kernel and panics. |
| Shift contracts and word identity | All i8 values are tested against boundary/mixed descriptor counts. Source controls exercise u32 unsuffixed count context, negative/oversized panic, explicit RHS preservation and distinct usize/u64 faults. |
| Float and cast semantics | f32 executes each operation at f32 precision. IEEE comparisons and NaN behavior, float saturation, truncation/sign extension, native-width conversions, negative zero and matched source/destination casts are exercised. No pointer or representation cast is admitted. |
| Literal frontend boundary | Pinned syntax tokens decode char/byte escapes and normalize decimal floats; owned recipes retain original spelling/span. Admission rejects mismatched decoded payloads and stable recipe identity includes those payloads. Source and CLI controls exercise actual values. Literal overflow fails before binding; arithmetic may yield NaN/infinity. |
| Panic versus fault | Actual numeric recipes and source/CLI controls distinguish overflow/division/shift GuestPanic from descriptor/context RuntimeFault. A failed operation starts but does not complete; later operations/exits do not execute. Scalar panic cleanup runs once; fatal faults do not invoke cleanup. Source origins and CLI exit1 versus exit2 are exercised. |
| Lazy LRIR execution and gate integrity | Queries emit owned verified recipes without evaluating guest values or requesting successor thunks. Existing source/installed-cache controls preserve cold-body deferral. Gate02 freshly runs Gates00/01 and observes every registered T020–T025 pass; its guard rejects ignored tests and absent task inventories. |

The full component inventory lives in `docs/tasks.json` and `compat/fixtures.toml`.
The final macOS checkpoint must record the actual test count, all twenty-eight
numeric controls, Gate02 and strict audits before the ledger marks completion.
The normative T024 main retains its eight operations; numeric opcodes add separate
counters rather than replacing interpreter execution with an AST evaluator.

Pending full integration remains explicit. T030/T031/T032 supply assignment,
selected arm/block and argument/call contexts; T040/T041 supply aggregate/index
contexts and nonprimitive coercions. M014/M016/M017 supply generated bodies,
expansion epoch revalidation and symbolic/source-aware type resolution. M026/M029
supply selected trait operators and required predicates. T044/T045 supply general
user Drop and panic cleanup, and T090 onward supplies interpreter/JIT agreement.
M038/M043 supply world/cache/feature regressions and release acceptance. Primitive
spelling resolution is provisional until the symbolic type pipeline handles aliases
and source shadowing. No whole-body inference, macro authority, native loading or
selected live-unit compilation is introduced by this component.

Gate02 is early scalar component evidence, not the final revision-4 milestone
release gate. MX014/MX029 full-LRR integration remains pending in the ledger.
Complete quotas, service contracts, security/performance/release artifacts and
actual Linux platform acceptance remain unfinished.
