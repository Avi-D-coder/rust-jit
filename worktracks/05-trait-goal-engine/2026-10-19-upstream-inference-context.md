# M020 — actual source-bound inference context operations

Solo root/LOCAL-049, baselined0ae450d4e5a69248984b9616a50a6a3763ac7cb.
E-R5-014: reuse pinned upstream inference/canonicalization/relation algorithms.
The existing storage owner has no source Context or variable construction/
resolution client. Actual conditional/multidispatch fixture goals require those
operations before the real solver can instantiate their source parameters.
Primary witnesses: unchanged methods/method-two-trait-defer-resolution-2.rs,
traits/multidispatch1.rs and const-generics/type-dependent/issue-61936.rs actual
source metadata. Their runtime contracts are still blockers, not passed by this
component. Existing ordinary/emit/wrapper all-edition source matrices are the
primary source/identity inputs; LRR controls may inspect the exact IR route.

Extend upstream/infer with InferenceContext owning actual Cx plus the single
existing InferenceStorage. Nonescaping creation stays inside the real prepared
source context. Reuse complete selected pinned variable constructors/probes/
shallow resolution/deep opportunistic resolution, taint tracking and binder
fresh-variable replacement. No new unifier, generalizer, canonicalizer, region
checker, goal solver or runtime evaluator. No whole-body RA/rustc inference.
Copied binder replacement must call the already ported shared BoundVarReplacer;
never invent another bound-variable walker or erase regions to fabricate success.

Actual source generics bootstrap supplies parent-first GenericParameter rows.
Add the pinned GenericArgs constructor/fill algorithm over that retained flat
representation once; share it with identity/error/fresh construction instead of
repeating parameter order. Original source and admitted macro source share Q69/
Q70 -> SourceGenericLayouts -> actual typed generic facts. Generic-kind transport
creates fresh type/const/region variables; no fixture names, hardcoded type,
synthetic generic declaration or fake provider. Preserve real parameter ordering,
Self and source spans. Fresh allocation is per goal/inference demand, never source
reparsing in execution. Keep typed full-fact/ReadyProvider holds intact.

Source dependencies: admitted source/parser/header/edition/epoch and catalog plus
actual typed generic table. The context may not query, read files, expand macros,
lower bodies or construct a namespace world. Immutable Types/Args remain interned;
mutable variables and upstream transactions stay per context. Cold preparation
and inference term folding are separate from installed VM work; no new frame/
operation planning, BodyIndex clones or full-file node searches. OS-owned data,
no budgets/fuel/accounting/sandbox/static checking or local unsafe.

Do not implement InferCtxtLike/SolverDelegate with absent/stub callbacks. The
actual complete trait requires compiler-compatible placeholder assumptions,
visible region constraints, leak checking, generalization/relations and canonical
responses. Those, source-fact publication and selected member/VM migration remain
explicit prerequisites. RA's known stubbed region/opaque callbacks are excluded.
No new context convenience API can close M020 or count as solver evaluation.

Before checkpoint: unchanged-source all-edition/origin matrix, fresh argument
namespaces/order/regions/consts, binder replacement via the actual shared folder,
non-consuming resolution, chained type-to-numeric resolution, const/deep tuple
resolution, errors/rollback/universe/region preservation and no source queries.
Reuse existing controls and add only supplemental host route/isolation checks,
not a substitute language suite. Builds/tests async in this checkout, frozen
source per run; collect terminals. Record exact failed/final sources and review
all8 recurring modes plus separate MA004/MA007/MA008. Unsupported/unexecuted
matrix entries and new distinct runtime gains stay separate. No full task,
milestone/interpreter/JIT/Linux/performance acceptance follows from this bridge.

Final V341 source-bound context/constructor component builds and5/5controls pass;20generic declaration worlds/48rows/36consts/24early regions and140Type heads/136TraitRefs in48source worlds. Actual source/currentness/body-cold guards hold. Four new exact original compiler support files are retained; no original Rust edits. This is not full inference/solver/runtime acceptance; all prerequisite/removal/gate holds remain open. Evidence worktracks/evidence/2026-10-19-v338-v341-upstream-inference-context.json and separate review bind actual results to the exact source.
