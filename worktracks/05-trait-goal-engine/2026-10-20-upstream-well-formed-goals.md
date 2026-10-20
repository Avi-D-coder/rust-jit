# M020 pinned well-formed goal client

Baseline43bad0f0e822464533c06be318777d89ab98f11c; solo /root, macOS first.
Previous goal turn progressed with a compiled/audited pinned eligibility port;
the actual source/leaf producer and upstream root still remain unimplemented.
Unchanged repr128.rs continues to witness the generic operator/associated
projection gap. Do not solve it by new inference heuristics or a fixture rule.

Required invariant: SolverDelegate::well_formed_goals must produce the pinned
compiler's unnormalized obligations for a demanded symbolic term. Reuse the
full transitive algorithm from compiler/rustc_trait_selection/src/traits/wf.rs,
plus its compiler helper dependencies. Do not implement an independent type
checker or a reduced collection of accepted Type shapes. This path produces
source type goals only; it does not check unentered function bodies or perform
borrow/static checking. Rustc diagnostics/tooling remain separate acceptance.

Shared entries: source/header/generic/ParamEnv facts -> existing upstream
InferenceContext and TypeIR -> unnormalized_obligations/visitor -> ordinary
upstream Goal predicates -> the same root/delegate evaluation. Retain binder,
region, nominal parent clauses, array/tuple/reference/function/closure/coroutine,
dyn/projection/inherent, pattern and constant behavior. Follow compiler queries
with actual immutable prepared metadata; missing inputs remain preparation
dependencies, never successful empty goals or semantic NoSolution.

Only diagnostic cause/spans discarded by the compiler's final as_goal mapping
may be adapted out, with registered exact transformations. The selected source
client uses the next solver; legacy solver normalization is not a substitute.
Compiler helper implementations must be copied/mechanically registered too,
not replaced by host guesses. No new source-specific/macro-specific evaluator,
profile exclusion, type guess, resource cap, unsafe or query framework.

Cold dependencies include required clause/ADT/variant/field/value metadata and
actual source/member/epoch identities. Callback execution may traverse shared
IR and emit goals but may not parse source, execute macros, perform IO or run
guest initialization. Reached const/static values keep ordinary VM ownership;
preparation constants keep their explicit pure contract. Share immutable
structures; no BodyIndex copies or namespace census in callbacks/dispatch.

Evidence: exact retained compiler originals/licenses/hashes and mechanical port
audit; asynchronous macOS1.99 builds; source-aware TypeIR/inference controls
with ready still closed while other prerequisites remain; unchanged official
runtime witnesses. Require genuine upstream root/answer/member/VM integration
before claiming solver or fixture acceptance. Review all eight recurring modes
against an exact source fingerprint. Intermediate source-only port work must
name unimplemented helpers/data/branches, remain held, and cannot close M020.


Current integration uses the user-authorized three Sol6.1/high agents on
separate slices: value helpers, immutable ADT metadata, and asynchronous
validation/review; the lead implements the visitor/generator/delegate and
source-aware route controls. No checkout/target copies were made: direct
checkout workflow supersedes older frozen-copy guidance; source is unchanged
for each captured validation cohort and one agent owns Cargo.

The required SolverClients.well_formed_goals function pointer is removed.
The delegate now calls the full retained upstream algorithm and maps the
returned obligations to shared Goals. `tools/sync_upstream_inference.py`
includes its generated module audit; the value-tree helpers have their own
registered generator. `AdtField::ty` is also mechanically checked against the
compiler FieldDef implementation with its source binder already prepared.
No root-ready token or successful missing-source fallback has been added.
