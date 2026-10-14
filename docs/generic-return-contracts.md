# T032 generic return contracts — work in progress

A reached simple generic return annotation now lowers as a stable Parameter hint
containing the declaration definition and generic ordinal. Lifetime/const entries
retain their ordinals; the hint never replaces the declared T with a primitive.
Structural admission rejects foreign definitions and out-of-budget ordinals.
Accepted callee frames install the prepared concrete runtime substitution in a
separate table. The VM resolves the return hint through that table while source
recipes and prepared source-result terms stay rigid.

Known caller result context can bind a matching result parameter and supply a
concrete argument hint for that same parameter. The argument hint still retains
its declared source term. Final contract preparation processes the result context
before actual argument constraints and rejects conflicts without widening. An
otherwise unbound result parameter can bind from a concrete reached return. This
binding validates source/descriptor integrity and retains the original source
result. Unknown unsuffixed returns without context remain a type-context error.

Six controls exercise typed generic identity with unused bounds, lifetime ordinal
preservation, reached mismatch and non-returning fuel exhaustion, result-only
binding and rejection preservation, caller-context typing of unsuffixed arguments
and returns without defaults, actual CLI execution, and stable parameter-hint
encoding/foreign-identity rejection. Existing primitive source-view conversion is
shared by argument and reached-return boundaries.

T032 remains in progress. This establishes simple generic scalar call/return
contracts, not full revision-4 SourceView/ParamEnv/dictionary IDs or source-aware
body semantics. Explicit turbofish, structural/nominal/reference/native views,
complete aliases/projections/consts/binders, local generic type annotations,
member/function-value calls, coercions, context-bearing sealed packets, generated
suspension, chunked activation, user destructors and release integration remain
mandatory. No unrelated generic bounds or guest bodies are proved to discover a
return hint. Validate macOS first; Linux acceptance remains deferred.
