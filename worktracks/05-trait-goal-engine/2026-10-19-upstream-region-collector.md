# M020 — modern compiler region constraints and visibility transport

Solo root / LOCAL-049 / WT05-02; baseline6ced27f7d3897b281e5fa7e9b79d4f500b382ab3.
E-R5-014/spec53.0 requires real upstream region semantics before implementing
InferCtxtLike/SolverDelegate. Source-confirmed gap: the pinned older RA collector
has a four-case Constraint enum, no visibility bit and no origin on constraints;
its equality helper always emits two subregion edges. Current rustc uses seven
kinds, direct equality, explicit source origin and VisibleForLeakCheck. The RA
solver client additionally returns unconditional leak-check success and empty
region responses: these stubs remain excluded, never acceptable fallbacks.

Minimal host witness: record var-var equality with actual source Span and
VisibleForLeakCheck::No, then inspect a single compiler VarEqVar with preserved
origin/visibility and its exact iter_outlives expansion; rollback must remove it
and undo its variable union. Existing actual source Type/header worlds and
unchanged methods/method-two-trait-defer-resolution-2.rs and
traits/multidispatch1.rs supply the language source witnesses; their runtime
contracts remain outstanding and are not passed by these host data controls.

Retain exact compiler region_constraints/mod.rs. Replace the old enum with the
compiler ConstraintKind/Constraint/iter_outlives definitions and replace selected
add_constraint/make_eqregion/make_subregion/combine_vars plus region resolution
operations with their pinned bodies. Preserve actual Span beside each constraint:
InferCtxtLike supplies Span, so the selected SubregionOrigin inputs project to
that real Span; full rustc diagnostic-origin classifications are not claimed.
Existing RA per-variable source spans remain. Compiler Known-region values have
no span field; compatible RA's extra optional value-span stays None as before,
while the real constraint origin and variable source origin are retained.
No origin is guessed from fixture name, dummy span or missing source facts.

Keep one existing collector, stores and combined undo log. Remove old four-case
enum and two-subregion equality implementation; no compatibility wrapper or
second region data evaluator. Preserve the old compatible RA Verify/member
records until their own actual producer/migration; do not advertise a complete
modern compiler collector. Update actual callers to explicit origin/visibility
and canonical current names. Combination helper uses the compiler's own explicit
Yes policy. No new algorithm, type/borrow/privacy checker, success shim, resource
limit, native loading or unsafe. Do not modify a suspicious upstream LUB rule
independently; preserve actual pinned behavior and keep broad acceptance open.

Ordinary/generated source Q69/Q70 / Encoder / Cx / InferenceContext remains the
shared path. Collection receives retained IR only: no parsing, query execution,
macro IO, global namespace scan, BodyIndex copy or VM/frame change. Cold costs
are actual upstream vector/union operations and iteration; no speed claim.
Retain original source hashes/license; register deterministic projections in the
existing inference generator. Review all8 recurring modes separately from MA IDs.

Required evidence: async frozen macOS1.99.0 builds and actual all-edition/origin
source matrix; explicit span/Yes/No retention, direct equality kinds and upstream
outlives iteration, static/reflexive cases, canonical region resolution and
rollback/commit including variable allocation/unions. Controls are supplemental
host IR, not new guest-language fixtures or stock runtime passes. Full graph/SCC
helper reuse and actual compiler leak checker remain next prerequisites, together
with complete inference/generalizer/relations/canonical/source-fact/delegate/root
and runtime migration. No M020/interpreter/JIT/diagnostic/Linux gate closure.

Final V346 library77919, CLI19454 and controls9572 terminal exit0;3/3controls pass, source/HEAD/test/CLI guards match.140Types/136TraitRefs/48worlds and20generic worlds/48rows/36consts/24early regions observed. V345 library85603 exits101 with11mechanical errors; no controls ran. Source stayed unchanged after final review/validation. All new region cases are supplemental host IR; no root/stock-runtime/gate acceptance. Exact commands, hashes, errors and terminals: worktracks/evidence/2026-10-19-v345-v346-upstream-region-collector.json.
