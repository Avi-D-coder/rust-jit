# M019/M020 / LOCAL-049: pinned multi-root fulfillment component

Preimplementation source review, baseline `7ea94b8f6b3ff5b9a0793a414e2040aa2828d0ff`.
This bounded assignment owns new `infer/fulfillment.rs` and a new reproducible
projection tool only. The lead owns module registration, manifest/provenance
registration, actual source-fact admission, root ownership, integration and tests.

The invariant is that mutually constraining demanded roots share the same actual
upstream inference owner and rustc fulfillment policy. Evaluating separate roots
once, writing an LRR retry scheduler, or replacing them with a synthetic
`WellFormed` root would violate it. The minimal witness is two demanded roots
where the second assigns a variable on which the first root has stalled. The
unchanged upstream semantic witness is pinned
`compiler/rustc_trait_selection/src/solve/fulfill.rs`, SHA256
`b847f09571a9b182c1456ea0c3fc9ccae43457ac7b12f01f1d28c0573e06ad7f`,
at Rust revision `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`.
Applicable unchanged language fixtures and execution remain the lead's acceptance
work; this component alone supplies no language-fixture pass.

The common owner already exists: `infer::delegate::SolverContext` owns one
`InferenceContext` and dereferences to it. The pinned
`SolverDelegateEvalExt::{evaluate_root_goal,goal_remains_stalled}` blanket
implementation already accepts that owner. `InferenceStorage::num_open_snapshots`,
`InferenceContext::{shallow_resolve,sub_unification_table_root_var}` and the real
`Interner::recursion_limit` require no new accessors. The actual shared crate's
`solve::fast_path::compute_goal_fast_path` supplies registration. Keep its stalled
state and its root-resolution update; do not duplicate its canonicalization,
candidate transactions, projection or search graph. Source facts enter through
the immutable prepared provider and must already be admitted. Missing source is
not semantic failure. Arbitrary replacement-goal admission is not authorized.

The port will keep generic caller cause metadata, the actual predicate/ParamEnv,
origin span and recursion depth. Compiler-only diagnostic derivation, HIR/MIR
region-dependent replay, and compiler coroutine-typeck drain are outside this
interpreter component. Retain typed original-obligation NoSolution and ambiguity
outcomes. Pinned fulfillment progress overflow is fatal at exactly rustc's
configured recursion comparison; translate the diverging diagnostic into a typed
terminal outcome and prevent further processing. No independent work/count cap
is introduced. Preserve upstream `MaybeInfo`, including search overflow, through
the stalled result rather than promoting it to success or NoSolution.

The ordinary frontend/member consumer must supply demanded roots from the same
source/type service for handwritten, arbitrary user-macro, wrapper and standard
macro nodes. This component parses, lowers and executes no source, never examines
cold bodies and owns no guest continuation. The actual source/root/VM join,
dictionary witness extraction, and legacy semantic route removal remain held.
Pending roots and their inference stalls are local to one snapshot. Registration
is a fast-path computation; fulfillment has pinned `ThinVec::retain_mut` passes
until the pinned progress criterion stops. No global namespace scan, plan
reconstruction or immutable body copy is added. No speed claim follows.

All eight architecture modes reviewed before implementation: (1) no assertion
evaluator; (2) no macro-output executor; (3) no macro-specific member or numeric
rules; (4) no generated-source gate; (5) no pattern engine or cold-arm executable
check; (6) no parsing/planning callback; (7) no namespace map/scanning or cache
invalidation code; (8) no BodyIndex ownership/search. This is a bounded source
review of the new adapter, not removal of MA001-MA008 or acceptance of neighboring
production paths. After generation record exact new-file fingerprints and a
separate source review. Runtime, full fixture, milestone, interpreter, JIT,
performance, and Linux gates remain open.
