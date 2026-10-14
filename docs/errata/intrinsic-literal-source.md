# Intrinsic Bool and Char source identity

M017/T032 distinguish actual literal source identity from printed aliases and propagated expected context. The unchanged rustc shadow-primitives and issue-26251 bodies supply genuine Bool/Char producers; earlier literal writes lost that identity before a local read and source-aware return. The interpreter candidate binds ordinary indexed literal kind, PC, span and destination before attaching the actual Primitive source after a checked write. Numeric, byte and generated literal routes retain their own rules.

Strict storage/source annotations still refuse incompatible literals before entry. Nonstrict hints in an empty temporary are expected call/return contexts: an intrinsically typed value retains its own source, then faults at the reached call/return boundary if incompatible. T032/L006's existing generic return mismatch control remains unchanged. An already allocated temporary cannot pass literal preflight; existing actual rigid/nominal values cannot be overwritten by this route. Immutable comparisons borrow source metadata; full ticket/cell admission precedes fuel and publication.

Eight new source/fuel/quota/forgery controls and 549 unfiltered library bodies initially passed after two incorrect diagnostic adapters were corrected. Wider source-run validation then exposed an overly strict propagated-context guard; its production repair and the unchanged return-boundary assertion are under validation. No full rustc fixture, MAIN milestone, task or release pass is claimed.

Evidence: artifacts/macos-first/root-intrinsic-literal-source-v1 and root-alias-transitive-trace-v1. Earlier failures and source manifests are preserved.

## Integrated component checkpoint

The final combined frozen component run passed 552 library, 166 focused integration and 28 CLI bodies plus strict workspace Clippy and formatting. Required source/identity/fuel/limit regressions passed; original failures and exact manifests are retained. Tested code and assertions are integrated in MAIN. The new macOS milestone is pending, affected task requirements remain in_progress, and full rustc compiletest passes remain zero. See artifacts/macos-first/root-alias-transitive-trace-v1/combined-component-acceptance-v2.json and root-return-and-attribute-v1/MAIN-integration-v1/integration.json.
