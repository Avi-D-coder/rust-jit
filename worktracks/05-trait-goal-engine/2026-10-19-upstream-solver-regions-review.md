# M020 solver region state / forall — separate solo source review

Baseline65c56a235b782ec511a758e74d7daa4d7c646cc0; root/LOCAL-049/WT05-02.
Reviewed candidate V344 source fingerprint
`d57e3423a205c5d6b239547f4638be308b4e34bd2554c32512e197bc67896114`.
Source review is not runtime or full inference acceptance. Actual validation
terminals are recorded separately after completion.

## Selected upstream entry points and ownership

solver_region_constraints.rs retains the actual compiler storage definition:
Option<SolverRegionConstraint>, new(None), get_constraint Some.clone / None.true,
and overwrite(Some). The latter state is genuine initially empty conjunction;
it is not a source-fact, trait-goal or absent callback success. SolverRegionConstraint
is the already shared RegionConstraint<Cx,actual source Span> type.

get_solver_region_constraint/overwrite_solver_region_constraint are exact
selected compiler infer/mod.rs bodies. register_solver_region_constraint is the
compiler outlives/obligations.rs body. The latter invokes upstream build_and,
clones the original formula and records OverwriteSolverRegionConstraint before
mutating the storage. Its original performance FIXME is retained. No local graph,
formula canonicalizer or relation algorithm. Original whole compiler files are
retained, with exact pin/hash/license provenance; the compiler itself is not built.

The existing InferCtxtInner gets the compiler field/init; its existing combined
UndoLog gets the exact compiler variant and reverse arm. All existing snapshot
start/commit/rollback ordering and one log owner remain. No second transaction
engine, separate solver snapshot or clone of an inference world is introduced.
These selected compiler additions project into the already compatible RA client.
TypeOutlivesConstraint remains the existing RA record; compiler registration with
SubregionOrigin is not imported or claimed, avoiding silent origin erasure.

higher_ranked.rs copies compiler enter_forall_and_leak_universe/enter_forall.
It retains the no-escaping fast path, creates exactly one universe when needed,
uses existing FnMutDelegate -> Context::replace_bound_vars_uncached -> the shared
BoundVarReplacer, and creates the actual upstream placeholder types for all3
bound namespaces. No local shifting/substitution loop, rigid-variable inference,
whole-body checking or leak acceptance rule. The compiler FIXME about placeholders
escaping the callback is retained; this method alone is not a leak check.
InferCtxtLike's wrappers, solver-region span transport and complete callback
implementation remain future joins. No trait implementation with stubbed methods.

The compiler region unit test original remains unchanged in vendor. Its exact
And/Or ambiguity assertion body is registered separately with only TyCtxt -> Cx
translation and actual source spans supplied by the host matrix. This is an
adapted compiler component witness, not an unchanged-test or runtime pass.
New host controls supplement actual fixture source worlds with transaction/
IR/route assertions; they do not add synthetic guest language fixtures.

## Eight recurring modes (not interchangeable with MA audit IDs)

1. No assertion/expression evaluator: new operations manipulate compiler IR only.
   Source Q69/Q70 / source_terms::Encoder / actual Cx and InferenceContext remain
   the shared route. VM/evaluator/assertion dispatch is untouched.
2. No generated-output mini-engine: original/user-emitted/wrapper Type inputs all
   use the same source context/client modules; no macro identity/grammar branch.
3. No context-only numeric/member logic: no selected member, numeric fallback or
   inference rule change. New forall placeholders are upstream rigid values;
   creation does not concretize definition params or select an actual-type method.
4. No generated-only refusal: existing source/edition/epoch/identity and prepared
   dependency guards remain. No origin condition or fake ReadyProvider added.
5. No duplicate patterns/eager bodies: no FunctionSignature/BodyStructure/thunk/
   PatternPlan demand is added. New helpers take retained IR; no guest body walk.
6. No repeated planning: register/overwrite/forall operate only on current stored
   formulas/interned terms. The exact upstream full-formula clone and fold costs
   are cold inference costs, not installed operation source planning. Formula
   size/folding can matter; no speed/scaling claim follows from source inspection.
7. No full-world namespace scan: no global namespace/candidate-map comparison or
   invalidation is added. Context/source currentness owners stay unchanged.
8. No immutable BodyIndex copy/full-file search: only upstream formula values and
   small inference fields are touched. No frames, body indexes or node search.

Separate MA004/MA007/MA008 architecture closure stays open. Source-term matrices
cannot prove complete runtime cleanup/invalidation/authority or arbitrary macro
function binder behavior. No local unsafe, budget/resource limit, sandbox/static
borrow/type/privacy/lint check, native execution or early guest JIT.

Required remaining work includes real source function/binder classification,
visible region constraints/leak checking, full opaque/binder/region matrices,
InferCtxtLike/generalizer/relations/canonical responses, full source facts and
parameter IDs/ParamEnv/lang-items/currentness closure, actual SolverDelegate/root,
source-selected member/dictionary/runtime migration and superseded route removal.
No M020/milestone/interpreter/runtime/JIT/Linux/performance gate is closed here.
All new source matrix mains remain unexecuted; zero new stock runtime gain and no
refreshed suite total. Existing missing interfaces are not filled by empty or
successful defaults.

Final V344 library83856, CLI23301 and controls20930 all terminal exit0.3/3focused controls pass with source/HEAD/test/CLI guards. Actual140Types/136TraitRefs in48source worlds and20generic worlds/48rows/36consts/24early regions are observed; new region/forall checks are supplemental host IR. Adapted compiler ambiguity assertion body is explicitly labeled, not an unchanged original execution. Source stayed unchanged after review/validation. No root invocation, new stock runtime gain, full M020/milestone/interpreter/JIT/Linux/performance acceptance. Exact source, commands and terminals: worktracks/evidence/2026-10-19-v344-upstream-solver-regions.json.
