# Existing or-pattern route control correction

LOCAL-024, T024/T050/T051; October15 source-only successor to frozen70.
The actual source-boundaries gate terminated `ad127a/1`: 3313 listed,
2894 pass,1 fail,418 unstarted; architecture boundary audit passed.
Its sole named failure was the old scalar_or_patterns.rs:331 expectation
that every range/guard phase emit Unsupported. No case label/operation dump
was printed, so source analysis identifies the stale contract without claiming
a dynamically observed iteration. The exact old source/results stay sealed.

Minimal source witness is its existing `0 | 0..=1` range Entry. The frozen
ordinary `literal_match::prepare_inner` already emits lower Literal/Ge and
a branch to upper Operand(0); its range tests and actual supported source
contract contradict the blanket negative observer. Restoring Unsupported
would regress implemented semantics. The later common transaction candidate
also supports guarded multiple alternatives, so its literal Commit prepares
Pattern::Preview and the actual guard continuation. It must not evaluate that
guard, the scrutinee, another thunk or cold upper bound during preparation.

Only the existing external LRR route-control body changes: assert exact
shared Pattern begin/lower comparison/deferred-upper branch for range; assert
shared preview, scope entry and exact Call Entry continuation for guard; keep
real unresolved binding-header refusal, no source ReadLocal/ReadParameter,
no eager guard/other thunk query and the per-case non-effectful constraints.
Original stock files/directives/output oracles remain unchanged. No separate
macro evaluator, language fixture, runtime semantics or authority bypass added.

All-eight source review: modes1–4 introduce no runtime/compiler/type or
generated-only dispatch; mode5 asserts the existing common transaction route
and genuine unresolved binding gap; mode6 keeps preparation/child queries cold;
mode7 changes no namespace/invalidation logic; mode8 adds no source/index
copy or production scan. The actual unchanged-source71 scoped/strict/milestone
validation belongs solely to the asynchronous validator. No test pass, stock
contract, full-fixture, task, architecture closure or JIT readiness claimed.

Frozen packet `artifacts/macos-first/common-source-plans-v2/SOURCE.json`, SHA-256 `57189be565b3a74adc18f4a90b9f81ccc51576849db33971d0e7b33b3337137e`.
