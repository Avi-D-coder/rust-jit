# Reached numeric facts in ordinary source constraints

Bounded claim: WT04-02 / WT04-04; T032, T033, M018, M025, M030.
Owner `/root/closure_callable_resume`; root implements separate initializers,
other implementation worker owns atomics. No Cargo by this worker.

Actual unchanged upstream witnesses remain
`tests/ui/autoref-autoderef/autoderef-vec-box-fn-36786.rs` and
`tests/ui/autoref-autoderef/autoderef-arc-boxed-closure-call.rs`, with their known
independent constructor/library blockers. Immediate regression is the unchanged
LRR source-Deref exact-once/macro-origin control added by the previous bounded
slice. V4 fails on a bare numeric literal after generic callable adjustment;
source-Deref structural-result refusal has advanced. Never modify its literals
to hide the problem or claim an upstream pass from this control.

Invariant: an unsuffixed numeric expression can use a type equality already
proved by the common retained source-constraint component and a related value
that ordinary reached execution has already produced. No assertion identity,
callee spelling, private wrapper layout, blanket borrowed-literal fallback,
second evaluator or preparation/execution of a cold call may select its width.
Future source annotations and selected member signatures retain their precedence.

Current shared route: SourceConstraintPlan immutable facts ->
frontend::binding_constraints::Component (expressions paired with Via projections,
including tuple/match/pattern/binding relations) -> PreparedBindingConstraints ->
PreparedBindingPlan retained input guard -> db::binding_constraint_thunk::literal
-> ParsedFile::initial_thunk_with_literal_constraints -> ordinary literal recipe
-> vm::step/frame_hint -> existing materialization/storage. Handwritten and
expanded syntax already join the same indexed facts and node/slot identities.
Wrapped runtime closure calls have no statically selected signature, so current
Component::runtime_callee leaves that relation unanswered. The actual ordinary
call result nevertheless already exists when a later tuple/comparison-related
literal is reached.

Extend the same immutable plan with related value slots and structural type
projection paths from Component.expressions; unresolved numeric literal recipes
can retain that relation as an ordinary dynamic HintRecipe. Runtime reads only
live, already-created values in the same frame and projects their descriptors;
missing/cold/nonlive slots supply no fact and never cause execution. Only an
actual numeric primitive of the literal's family supplies a hint. No new DB
runtime input, query side effect, global namespace census or parser execution.
Relevant prepared source dependencies remain attached through the existing plan
guard; descriptor facts are checked on each actual literal entry, not stored as
an invariant primitive in a shared generic recipe.

Supported projection operations must be actual retained structural relations
(reference pointee, tuple field, array/slice element and nominal type argument).
A source-selected nominal field or overloaded Deref relation cannot become a
private runtime layout projection; unresolved operations retain a gap. Existing
ordinary source/member resolution remains authoritative. Conflicting actual
facts must not choose an arbitrary representation. Closed static primitive
answers take precedence over this conditional runtime join.

Cold work is one retained component walk and hint selection; hot work is the
retained related-slot/projection list, proportional to the actual demanded type
relations, with no source/body parse, body clone, execution of calls, resource cap
or sandbox check. Existing operation continuation and guest cleanup remain owner
of values; observing a descriptor performs no move/borrow/drop/effect. Failed
readiness yields no hint, preserving the current unsupported result rather than
force-running a cold branch. Loops and repeated call activations consume current
live frame slots, not cached runtime types.

Acceptance requires the existing unsuffixed source-Deref control and ordinary
numeric/upstream source constraints, function-value and generic call regressions.
The stock Arc/Vec prerequisites remain separately outstanding unless actual
execution proves otherwise. Full macro-origin/cleanup/suspension and all8 source
review are pending until implementation and terminal validation. No task/JIT gate
closure is implied by this plan.

Implementation shape chosen before edits: retain an immutable `RelatedNumericPlan`
inside the existing HintRecipe, listing actual BodyIndex expression node IDs and
explicit reference/tuple/element/type-argument projection steps. Each node ID
resolves directly to that same frame's value slot; only active-scope live cells
and actual descriptors contribute. The plan is conditional evidence rather than
a guessed primitive in the source memo. Static resolved primitives remain first.
The prepared input guard is retained for this conditional plan just as for a
static primitive answer. Literal preparation/admission stays ordinary; only its
hint changes. No macro-specific runtime consumer is introduced.

Unsupported source member/variant/iterator-item projection steps are not mapped
to runtime layout. They retain their existing missing-source-service gap. No
selected callee body is parsed/lowered merely to obtain an observation, and an
unexecuted related slot cannot cause a call. Actual descriptors that disagree do
not yield an arbitrary width. Existing parent hints and source signatures retain
normal conflict checks. The old blanket-fallback temptation is explicitly rejected.

## Separate exact-source all8 review before first build

1. No assertion evaluator: ordinary BindingConstraints produces the plan for any
   indexed numeric literal. No macro names or caller spelling are examined.
2. No generated scalar shortcut: literal lowering stays initial_thunk_admission /
   ordinary Op::Literal for every source origin; no new generated executor.
3. No context-specific selection or inference: candidate equalities are the
   existing Component.expressions/Via typing relations. Only actual numeric
   descriptors of the same family provide conditional evidence. Selected source
   primitive answers remain preferred. No method table/whitelist is introduced.
4. No generated exclusion: owning indexed node/value slots and source-plan
   structure/currentness checks are shared; generated nodes have the same route.
5. No pattern duplication/eager execution: the existing common pattern facts
   state type relations. No matcher, guard/body evaluator or call entry is added.
   Unexecuted/inactive/moved slots and the currently entering literal are skipped;
   the mechanism cannot execute or prepare a cold arm/callee to supply a value.
6. No installed source planning: one immutable Arc plan is retained with the real
   prepared input guard. Preflight and execution each observe its bounded-by-plan
   list without parsing/plan reconstruction. This is descriptor evidence only;
   no move, borrow, guest effect or cleanup cursor is consumed during observation.
7. No namespace census or unrelated invalidation: existing source input closure
   and component member services are retained. Hot reads are relevant same-frame
   slot/type metadata. No query runtime input or full-world guard is introduced.
8. No body copies/searches: node IDs index retained BodyIndex directly; each frame
   owns only its existing execution slots. The new plan uses Arc immutable handles.

Projection safety: only reference/builtin Box contents, tuple fields, array/slice
members and nominal type-argument descriptors are projected. Named/variant fields
and associated iterator/deref results are never read from private runtime layout.
The verifier requires an unsuffixed literal entry of the same numeric family and
actual indexed source value nodes. Runtime checks current same-frame scope/live
state; unavailable or conflicting facts yield no guessed width. Generic recipes
retain relations, never a primitive from a previous frame. No limit/accounting,
sandbox, static diagnostic rejection or unsafe code was added.

Wire: recipe schema15/domainv15; related plan explicit stable domain/projection
encoding; PreparedBindingConstraints domainv2 includes the retained plan. Existing
wire controls migrated. These are source findings, not measured performance or
passing runtime evidence. The existing unsuffixed control and upstream originals
remain unchanged; first async compile/test is pending.

SHA-256 leaves reviewed (including inherited source present at this checkpoint):

- `crates/lrr/src/frontend/binding_constraint_observed.rs`: `da9fd583e50a4b9a743444f2c9cdbbe4a67bf00e1b3f019e09b33dcc7c946d0b`
- `crates/lrr/src/frontend/binding_constraints.rs`: `7220c1b50c19c69f297448073413446e09315dc459ba8fb253ef11c2cad58e38`
- `crates/lrr/src/frontend/prepared_binding_plan.rs`: `5db5a36a9ce053af3e673dd8f7823cb70b1783fd25ba65c7f70e06484f67e78e`
- `crates/lrr/src/db/binding_constraint_thunk.rs`: `9620a1e1165d18da96400387443bbf240b33c34c1b648f44dfcca86897c5ba64`
- `crates/lrr/src/syntax/thunks.rs`: `5bd36b8681511b10d13c1423175852f8d077cbcc78e6d6b333ee796fa35fa6ad`
- `crates/lrr/src/ir/op.rs`: `864bd0b6c0cf0cdb3a741162d49854e6705e2496c458dfd53b9263958c9eeae0`
- `crates/lrr/src/ir/recipe.rs`: `dd79eec2a3a62eac855921b203fc4326525f6430eee2a7291581f788c0db22a6`
- `crates/lrr/src/ir/validate.rs`: `5a7a2adebef05ad5bea40da52c29188346d014fd218913171c3bdef6f4eb1c12`
- `crates/lrr/src/ir/parameter_place_wire_tests.rs`: `15070689b8f3dc5cd8e2ce9061096ce894e9e33e333a09188c603b0e3a7961a5`
- `crates/lrr/src/ir/definition_numeric_wire_tests.rs`: `7e8efa26813867281a0b5aa5093371869d244048964e09ddc75226f22d9e8cdb`
- `crates/lrr/src/vm/frame.rs`: `1ccae60ad29d1be1016f34a87462af8ff786bb2735b229b4a9a115c843cc05a5`
- `crates/lrr/src/vm/frame_related_numeric.rs`: `e96e1028ab504879012147ca1dc0c58830133c236398d995e967dc97486abdd2`
- `crates/lrr/src/vm/step.rs`: `e2132770e9b7aa628bf1291844920cb8753d0997e471d4b49559adb5fa354985`

Pre-validation projection review correction: AutoDeref must not silently stop
at an unhandled nominal and then read that nominal's own type arguments as its
source-defined Deref target's arguments. Such a candidate now supplies no fact;
source-directed Deref stays with the common member service. Direct completed call
results, builtin references/Box and tuple/array evidence are unchanged. Updated
frame_related_numeric.rs SHA-256: `1199f5b575a744fe0a7b3eb891e228bd8406b8140e2b9698a5d88d6e8943dc7d`.

## V6 actual terminal evidence

Sole validation worker compiled unchanged source set `3d5608832ad2a0610c3d3ff5be25d9657825fe0e058993e1cd1cd8eaacb8736e`: library no-run exit0 in30.45s, CLI build exit0 in22.12s, no source changes during either build. Test binary SHA256 `4e6624bc0d6fa36155380c1b8d9e694362076fdac38ff1b3cff146754e6ac74a`.

`cli::callable_value_tests::callable_autoderef_uses_source_deref_once_before_arguments_across_macro_origins` PASS, exit0,18.06s. Existing unsuffixed ordinary/user/wrapper forms plus Rc closure program run, check stdout ordering, and inspect CalleeDeref opcode execution. This is an LRR route/exact-once control, not a new stock/full-fixture pass.

Related regression terminals: `array_iteration_nested_in_a_standard_macro` PASS18.599s; definition-numeric recipe schema and explicit context tags controls PASS. Canonical local receipt is `artifacts/validation-resume-2026-10-18/candidate-v6/report.json`; actual callable terminal `control-0.log`. All six selected controls passed; whole owning stock const-int and viawhere roots still faulted at separate i128 and explicit generic ValuePath prerequisites. No new distinct upstream runtime pass claimed in V6. Later result-template/explicit constructor source changes require successor validation and do not rewrite this evidence.
