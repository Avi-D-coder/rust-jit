# M020 — actual compiler hidden-type bounds client

LOCAL-049/WT05-02, solo owner. Baseline64a307c7073980b487e36220eafd7585751c31f7.
Invariant: SolverDelegate::add_item_bounds_for_hidden_type must use the pinned
compiler's complete bound instantiation/substitution/conditional-const goal
algorithm, never an empty callback or runtime tag/name lookup. M019/M020 and
the full interpreter/JIT gates remain open until actual source producers,
root/answer/member/dictionary/VM integration and obsolete-consumer removal pass.

Primary unchanged witness: impl-trait/nesting.rs, actual generic and nongeneric
opaque returns with Into bounds, const lengths and assertions/output. Existing
source encoder still reports OpaqueMetadata instead of creating an opaque ID;
do not fabricate an opaque DefinitionId, empty bounds or a ReadyProvider merely
to make a host probe pass. Record any runtime refusal honestly. No new guest
language suite; source-context host controls are supplemental.

Actual shared route: ordinary retained source/signature/generic metadata ->
source_terms/PreparedProvider -> concrete SolverContext -> upstream goal/root.
Replace SolverClients.hidden_type_bounds function-pointer contract with the
actual pinned compiler infer/opaque_types/mod.rs method. Copy the complete
compiler ty/fold.rs::BottomUpFolder instead of writing another folder. Preserve
both the projection branch for the old-solver mode and the same opaque ID/args
replacement, ordered explicit bounds, initial WF goal and conditionally const
HostEffect goals. The active upstream client already fixes next_trait_solver
true. No independent solving, static body checking or new source execution.

Exact source dependencies: explicit item clauses (including actual spans) and
the declaration's actual conditionally-const flag, distinct from a list of all
item bounds or alias_has_const_conditions. Add required non-default facts and
reader methods; they still need a genuine source producer. The existing
explicit_implied_const_bounds service preserves ordered predicates but omits
spans; the compiler method discards those spans, so adapt only that loop's
tuple binding, never synthesize spans or conflate other predicate collections.

Cold/hot costs: compiler folder visits retained typed IR and reconstructs
immutable interned handles; no source parsing, callback IO, macro/guest
execution, namespace scan or BodyIndex copy. No extra resource cap, local
unsafe, new solver/backend or performance claim. Missing facts still prevent
ready/root entry; no implicit negative answer.

Evidence: deterministic source/hash audit and async macOS1.99.0 builds; existing
unchanged-source Type/TraitRef and const/region context controls, with a genuine
shared-IR postorder-folder control, and unchanged runtime nesting witness.
Actual opaque source construction/positive bound and conditional-const branches
remain unexecuted unless the real producer is available; do not infer them
from a folder or neighboring passing case. Record all8 architecture modes and
keep full origin/currentness/suspension/cleanup matrix and feature gates held.
