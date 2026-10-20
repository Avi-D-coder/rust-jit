# M020 well-formed goal client: bounded source review

Baseline `43bad0f0e822464533c06be318777d89ab98f11c`. V492 source fingerprint
`da04f55155f5811b42debdd5889d6accc306b2ba1faf6e868c66502eab159a97`.
Lead review plus independent `/root/wf_validation_review` review; validation
terminals and closing guards are recorded separately. This is a component
review, not M020, runtime-suite, architecture-gate or JIT acceptance.

`SolverContext::well_formed_goals` now calls the complete pinned
`well_formed::unnormalized_obligations` visitor directly. The former required
`SolverClients.well_formed_goals` callback is removed. The pure source client
emits ordinary shared solver `Goal` predicates in the existing inference owner;
it does not enter executable bodies, execute guest code, or check unentered
branches. Upstream `evaluate_const`, transmutation and reflective field clients,
actual source/ADT/ParamEnv factories, specialization leaves, root evaluation and
answer/member/VM joins remain required. Existing custom interpreter resolution
is still active and remains an explicitly held alternate route.

## Registered compiler representation adapters

The licensed pin is `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`.
`tools/upstream_wellformed.py` selects full unnormalized WF entry, all
transitively used helpers, the complete Type/Const visitor and object bounds
elaboration. It retains exact originals and checks their hashes. Diagnostic
causes, dummy spans and unused depth are discarded by the compiler's final
`as_goal` mapping; diagnostics-only pattern/scalable-vector static enforcement
is omitted under E-R5-006. No generated goals are dropped with these emissions.
Normalized legacy-solver helpers are not selected.

The two inherent-projection branches select the actual next-solver path.
`At::eq_trace` is transported to its exact shared `RelateExt::relate` invariant
call, preserving returned goals; this introduces no unification algorithm.
`Term::is_infer/as_const`, feature accessors and `Features::gca` are selected
from exact compiler bodies. Immutable enabled-symbol membership replaces the
compiler feature set; its query instrumentation is unnecessary inside an
already keyed, immutable source context. Compiler dyn dummy Self remains
`FreshTy(0)`; the actual dyn source producer must preserve the compiler's
reservation and cannot admit this sentinel as a natural existential input.

Compiler parent-chain predicates are represented by the shared interner's
parent-first flattened `clauses_of` result. Completeness and ordering must be
established by the real source producer; this review does not prove that
unimplemented producer. `AdtField::ty` is mechanically audited against compiler
`FieldDef::ty`, with the actual prepared early binder replacing the compiler
field-definition query. Field Type/source correspondence, generic ownership
and current dependency closure remain admission obligations. Source-schema
shape and flattened-list agreement alone do not establish them.

`wf_values.rs` preserves compiler typed value-tree destructuring and scalar
width/data checks through a separately hashed mechanical generator. It performs
no guest constant evaluation. Correct scalar construction, typed tree shape,
exact child counts, enum u32 ordinal encoding and actual source provenance
remain real producer requirements. Full nominal/aggregate/dyn/inherent/GCA
execution is unexecuted until real complete data is available. No fabricated
complete provider or fallback empty answer is admitted.

## Eight recurring failure modes

1. `well_formed::unnormalized_obligations` and its visitor produce type goals,
   not assertion/expression evaluation. No macro identity is consulted.
2. `SolverContext::well_formed_goals` traverses all upstream Type/Const shapes;
   no literal/break-only executable route or generated-output profile is added.
3. Source identity, typed member selection and numeric policy are unchanged;
   lang-item identities come from actual prepared compiler metadata.
4. `PreparedAdtVariants` captures the common `TypeSchema` query and definition
   receipt, with no handwritten/generated exclusion. The full origin matrix
   remains unexecuted rather than inferred from ordinary-source controls.
5. The visitor follows demanded immutable Type IR, including upstream closure
   upvar rules, not executable arms/bodies. No pattern executor is added.
6. The pinned `DelayedSet` shares visited types for a demand. Callback execution
   performs no parsing, source planning, macro work or guest initialization.
7. Source/schema publication checks are cold O(variants + fields). Callback
   reads use existing keyed immutable definition/lang-item facts; no global
   namespace map is constructed or compared. Relevant dependency closure is a
   remaining producer/root admission obligation.
8. Type/binder handles, borrowed variant/field slices and retained schema Arc
   are shared. No BodyIndex deep copy or whole-file node lookup is introduced.

These findings concern changed paths only. MA001–MA008 macro audit findings
remain separately numbered obligations; no macro architecture finding is
closed by this type-client port. There are no measured speed/scaling claims.


Independent validator confirmed this complete bounded review against V492's
source fingerprint after the final UFCS/import changes. The `AdtField::ty`
compiler substitution audit is included in the full inference generator; the
separate value helper audit also passes. Validation terminal evidence follows
in the linked compact evidence record; no neighboring runtime result is used
to infer any unexecuted nominal, aggregate or solver-root contract.
