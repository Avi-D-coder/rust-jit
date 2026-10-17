# Actual unresolved glob frontier — LOCAL-024

October 16, 2026. M014/M017/M018. Required unchanged full-core source witness
on scheduling source0759f083 fails218a37/1 before discovery polling with
`glob module scope remains unresolved`. Four source preparation controls pass;
full-core/runtime acceptance does not. The missing module path is not printed,
so identifying `prelude` or another module as the failure would be speculation.

Extend only the existing `frontend::module_aliases::graph_with_compiler_imports` unresolved glob
diagnostic with the actual captured SourceImport identity, leaf ordinal/range
and missing module membership key. Preserve the exact refusal and worklist.
All reported fields are bounded integer/digest facts; do not render unbounded
source names, invent a producer, skip the glob, or add a core-name exception.
This is cold error-path evidence, with no new success-path scan/allocation,
parser/evaluator, macro semantics, type rules, pattern route, authority grant,
immutable-body copy or namespace invalidation. All eight recurring modes and
MA001–MA008 remain held; this diagnostic does not repair module discovery.

Sole-validator reproduction is the same required positive on the exact prior
frozen source plus this one leaf. It must remain a failure until actual module
source/adoption/declaration closure is implemented. Map the actual identity and
range back to unchanged pinned core source before selecting the next repair.
