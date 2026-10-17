> Review correction before integration: the empty `static_assert_exists` callee
> does not need a SourceLocalPlan merely for an observer. Root withdrew the
> unvalidated runtime hook and kept V6 production byte-identical V5. The test
> instead inspects actual executed body/source/header AST. This component remains
> source-only for future genuinely entered module name/Type demands, with
> unchanged upstream `numbers-arithmetic/shift.rs` (main -> test_expr -> casts)
> as a concrete next witness. The earlier module-fix hypothesis below is retained
> as an abandoned explanation, not a production defect established by this test.

# LOCAL-024 — reached module body environment

T032/M013/M017/M018 prerequisite, WT04/WT08, October 16. Primary unchanged
`macro-nested_stmt_macros.rs` pinned ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
V5 b4472867 compiles; actual main completes its Unit/three-CallSeal/cleanup/
zero-legacy-assert and entered Type/call provider checks, but whole test fails
later at one of two direct post-completion metadata queries. Diagnostic repeat
97a111 preserves the original 94ccb5 failure; exact metadata query is unprinted.
Source inspection identifies the module callee's missing owning BodyEnvironment
as a required shared entry prerequisite; no completed scoped test is claimed.

Invariant: every actually entered selected source function uses its real source,
canonical header/body, owning module macro facts and admitted environment. A
caller environment cannot substitute for a foreign module. Generated boundaries
must retain actual source/epoch/producer authority; existing missing joins remain
explicit until implemented, without an original-only semantic exception.

Worker owns actual CompletedSourceEnvironment per-source sealed macro facts,
SealedBodyMacroEnvironment common factory, opaque ReachedModuleBody receipt and
shared Database checked publication/header loop. The facts retain immutable
selected macro indexes once, never the completed-root Arc (no cycle). Root owns
RunMachine::prepare_source_call integration. No second name resolver/interpreter,
macro-specific recipe, source parse during installed Type/target selection or
module-by-spelling fallback is introduced. Existing unjoined macro and argument
hint planning debt stays held (E-R5-005, MA004/MA006/MA008, distinct from modes1-8).

Root integration: normal reached Local OR admitted Module call preflight, obtain
selected Arc<BodyIndex>, call the corresponding opaque source receipt through
one checked publication path, then prepare contracts and GenericEnv. Derived
publication must precede revision-bound contracts, never silently refresh them.
Recheck retained caller Type/target relevant guards afterward. Other held legacy
call modes preserve existing safeguards. Cold callees stay unentered/unlowered;
only the actual selected body gets its environment; source capture and bounded
metadata are cold/reached preparation, not an alternate runtime evaluator.

Acceptance needs exact frozen source, whole unchanged main owning2015 then all4,
callee source-plan authority, source/cfg/epoch/unrelated-publication controls,
exact-once and cold-body checks, plus separate all8 source review. The current
source component and last successful main observation do not close architecture,
full fixture, general call planning, interpreter milestone or JIT gates.
