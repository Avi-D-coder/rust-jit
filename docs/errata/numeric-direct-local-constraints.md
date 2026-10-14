# Direct later numeric contexts and hidden let-else constraints

This is an interpreter repair under the existing Rust compatibility policy, with general inference still pending.

The minimal failing binding `let x=3+4; let later:i64=x;` previously committed an i32 result too early and failed ownership admission. A bounded source pass now selects agreeing primitive annotations on direct reads of the exact indexed binding in the same statement list. It preserves source names, cfg and namespace guards; aliases, calls, assignments, macros and other unproven uses remain explicit frontiers. It never runs a later producer, thunk or build tool.

The unchanged `infer-binary-operand-behind-reference.rs` exposed a separate shared shift-count issue: a u8 left value still permits an independently inferred i32 right count behind a shared reference. The repair applies an actual reference hint to the count's i32 pointee with its original local region/span, rather than assigning a primitive contract to the reference. Explicit suffixes, mutable/raw/nested borrows, names, casts and producers retain their ordinary paths.

Review of the pinned `let-else-irrefutable.rs` and `infer-result-type-through-dead-code.rs` found that a direct typed let-else read can hide additional constraints in an unreachable else block. The frozen baseline reached the later let-else frontier after arithmetic had already run. The repaired source pass inspects only captured syntax and refuses that unproven context before Literal/NumericBinary operations or producer entry. Both original review cases and expected files are preserved unchanged; whole-program/native compiletest execution of those review cases remains unexecuted.

Actual segmented private acceptance covers eight numeric controls, nine stock numeric controls, 534 unfiltered library bodies, strict changed-code Clippy and formatting. Both selected unchanged inference/cast mains complete in all four editions. The typed wildcard place adapter explicitly stops after the numeric result and two bindings commit; it is partial program evidence. The original float-nan control now observes its first NAN binding and the exact following unsupported scalar-method assertion, also partial evidence. Full rustc compiletest passes remain zero, task acceptance remains incomplete, and MAIN integration is tracked separately.

Evidence: `artifacts/macos-first/root-local-numeric-constraints-v1/component-acceptance-v1.json`. Baseline failures, disk-full never-executed suffix and adapter migration failures remain preserved.

## Integrated component checkpoint

The final combined frozen component run passed 552 library, 166 focused integration and 28 CLI bodies plus strict workspace Clippy and formatting. Required source/identity/fuel/limit regressions passed; original failures and exact manifests are retained. Tested code and assertions are integrated in MAIN. The new macOS milestone is pending, affected task requirements remain in_progress, and full rustc compiletest passes remain zero. See artifacts/macos-first/root-alias-transitive-trace-v1/combined-component-acceptance-v2.json and root-return-and-attribute-v1/MAIN-integration-v1/integration.json.
