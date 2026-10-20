# M020 / T040 reached constructor argument transport

Baseline 0c11e2e3aa, LOCAL-049 / WT05-02. Before edits, V505's unchanged
`tests/ui/associated-types/assoc-type-in-struct-constructor.rs` fails at entered
`S::<I>(i)` with TypeRecipe. Actual CLI uses RunNames::Completed. The ordinary
value resolver selects S, but bind_selected_constructor reads only a result
source/hint and falls back to unspecialized bind_constructor. The shared indexed
constructor_arguments selector runs only after this binder succeeds. Its real I
argument and the calling frame's I=u8 source binding never reach that binder.

Invariant: select a declaration once, then specialize its source field terms for
the actual reached application before materializing fields. An inferred result
and a written application must use the same nominal member query. Cache source
selection separately from activation-dependent bindings; a call site entered
with different generic bindings must not reuse the first bound field variant.

Lead owns engine/run_nominal.rs, run_owner_arguments.rs and the constructor
selection join in run_value_items.rs. Ordinary route: admitted source/macro tree
-> SourceLocalPlan/SourceConstraints indexed path Type nodes -> existing
explicit_type_arguments::select_path_arguments -> retained ExplicitType/source
guards -> Frame::bound_view/current GenericEnv -> NominalMemberKey -> existing
SchemaNames::bind_variant_current_with_arguments -> shared field Type/member
normalization -> BoundConstructor -> ordinary Frame constructor continuation.
No parser, trait solver, macro rule, runtime evaluator or cleanup route is added.
The existing legacy associated normalizer is migration debt, not extended here;
actual pinned solver factory/root and full clause/item-bound inputs remain open.

Remove the ordering that binds before reading explicit source arguments and the
call cache that conflates declaration selection with one generic application.
Only an actual selected constructor may demand constructor path arguments, so
ordinary functions with unsupported const arguments do not acquire unrelated
constructor requirements. Pattern callers retain their ordinary context-only
binder; they must not inspect an enclosing call's explicit path arguments.
Unit value constructors and call constructors share the same application binder.
This slice transports written source arguments before binding call constructors;
the existing unit-value per-PC binding cache and descriptor application remain
unchanged. Partial explicit holes still need independent result-context inference.
Frame::bound_view substitutes its currently supported structural families; a
projection/function/opaque argument is not thereby a normalized closed Type.
Those unproduced forms remain pending and cannot certify a trait proof. Its
existing reached runtime-binding fallback is Type transport, not solver evidence.
Retain source/currentness guards; no descriptor is proof of a trait obligation.

Cold cost: select each call target once and retain existing source argument plan;
specialize each actual source application once. Hot entry reads frame bindings
and checks retained guards, without source parsing, full-plan construction,
global namespace census or immutable BodyIndex copies. No performance claim.

Evidence required: frozen macOS 1.99 builds, unchanged associated constructor
runtime witness, existing constructor/explicit argument controls, all four
editions and generated origin controls including repeated generic activations.
Review all eight recurring modes at the final fingerprint. A new pending
projection/root failure is partial transport evidence, not a runtime pass or
M020/trait/interpreter/JIT completion. HRTB/item-bound worker and repr128 worker
own separate source slices; validation worker owns Cargo after global freeze.

## Separate lead source review before validation

The call classification split changes run_nominal::select_constructor_call and
run_value_items::{lexical_constructor,lexical_constructor_target}; module and
lexical classification return an actual NominalConstructorTarget before any
field Type demand. Only positive constructor classification requests the
existing indexed path argument plan. OwnerArguments::source_terms substitutes
current frame bindings with the existing Frame::bound_view. The application
binder receives those retained terms before NominalMember specialization.
The source target cache has a separate key from the field binding cache;
BodyKey/CallIdentity/result context/full optional source argument vector identify
one application. Every positive use checks source argument guards and bound
field guards; the existing run publication/body lease protects classification.

All eight recurring modes inspected independently after implementation:

1. No assertion expression parser/evaluator; source expressions and generated
   callers use ordinary constructor call selection and VM continuation.
2. No generated literal/break executor or new macro node/verifier namespace.
3. No method-name, numeric, assertion, fixture or primitive-type inference rule.
   Existing associated_types::normalize_template remains legacy migration debt;
   this change supplies its existing shared binder, not a new solver extension.
4. No original-only exclusion. Admitted original/generated path Type plans use
   the same selector and source query, with owning source/edition/hygiene guards.
5. Pattern constructor calls retain bind_selected_constructor(...None), so an
   enclosing expression's turbofish cannot become a pattern's arguments. No
   new executable arm, function body, match or cleanup plan is forced.
6. Existing path selections/guards are retained per source PC/position; target
   classification is cached once. Only activation source substitution/key work
   repeats; field query and guard are cached for each source application.
7. No new namespace census. Database memo/PreparedInputGuard dependencies and
   existing source publication leases apply; broader run guard granularity and
   unrelated invalidation performance remain unproved.
8. Bound variants share Arc data; the new route neither copies BodyIndex nor
   searches/reparses whole source files. Existing no-BodyEnvironment signature
   route and unit-value per-PC cache are unchanged migration scope.

No cap, static restriction, sandbox or unsafe was added. Partial explicit holes,
unsupported projection/function substitution, legacy contexts, full cache
currentness/suspension/cleanup and complete origin matrix remain held. Runtime
counts, full upstream solver factory/root, complete item bounds and JIT gates
cannot be inferred from this source review. Exact inspected source SHA256:

- `engine/run_nominal.rs`: `45206cef4f1de4c721089c3f88daa15b8696d929488cdc1ecc3c206244a2181e`
- `engine/run_owner_arguments.rs`: `feda3d46899496847df5a1b6172595080a317661454dc93fd5f337a0400b94a7`
- `engine/run_value_items.rs`: `ba1419d83ea56ce7551c6aaf20231b2a9d0d5dd888c5505ce352ddb2fd8c2cb7`
- `cli/builtin_derive_tests.rs`: `f0d756da10a527274001b58627d5e54c03659d4a20548c83ef24c6b8414c9a70`
- `vm/frame.rs`: `6e1637d5a3217577c78217fa719136b3c5eb4e96fa4847963246b7d8ab3cb997`
- `engine/run_pattern_names.rs`: `e128e63fecd8a39910e1f5ee5a05c47eff2231f495491e06639326b629b027c1`

## V507 results and required empty-argument correction

V507 source `0a10a31324d0972292bd62007d6e6b6bd0c511372dd2426ad95a259fea538dee`
builds lib/CLI/digester successfully. Both constructor controls pass (12 actual
original/emitted/wrapper edition worlds and 4 activation/exact-once worlds).
The unchanged associated constructor original now passes all 13 interpreter
runtime checks, versus V505's TypeRecipe failure. Generic-derived remains a
13-check pass and repr128 retains LRR-E-OP-TYPE. Q80 shared collector control
passes; the new Q82 output control fails before its first world because an
associated declaration scope is mistaken for an executable local Type owner.
That failure and the earlier exact-selector preflight failure remain evidence,
not successful component execution. Source owner worker owns the common repair.

Review also found that Some(empty written Type vector) erased an independently
known nominal result context. No V507 runtime receipt executes that edge. The
already imported unchanged run-pass empty-generic-brackets-equiv.rs lines19–25
(S::<>, E::<>::V, foo::<>, bar::<>) supplies real Rust empty-list semantics;
source SHA256 ef52000045d5cb79e9d4e037476032645f4ba4ac14dea08de0d444587939751b.
It has zero constructor Type parameters, so supplemental existing route control
now adds a contextual Projected<u8> = Projected::<>(5_u32) compound-field case.
The original remains unchanged. Exact pinned compiler expression/path lowering
(compiler/rustc_ast_lowering/src/expr.rs465–469 and path.rs477–503) uses
ParamMode::Optional and infers absent non-lifetime arguments, including empty
and lifetime-only lists. Generic inference fill is in
compiler/rustc_hir_analysis/src/hir_ty_lowering/generics.rs630–636.

Only after every V507 process and closing source/HEAD/test/CLI/digester guard
terminated, the application binder gained a nonempty-list filter before using
explicit arguments. Empty/lifetime-only source Type vectors preserve the same
independent context as an omitted list. Holes remain unguessed. The corrected
route requires fresh constructor validation; direct lifetime-only projected
execution remains unexercised. No new parser, solver or fixture rule is added.

## V508 frozen evidence

Source `73b4f0f96a40390f67a1a07d42d57548394bc2a41c437dcb6083c13bbae03697`,8,702inputs,baseline `0c11e2e3aaa11513eb8860875e97c329dc23bbb9`. macOS1.99 lib/CLI/digester builds and3/3focused controls pass. Observed12original/emitted/wrapper constructor worlds,4activation worlds including the contextual empty-list case (1,624operations started/completed),12actual output-bound worlds/12recipes. All closing source/HEAD/test/CLI/digester guards match. Exactly4source inputs differ from V507: constructor/test plus the shared original/generated namespace modules. No source edits follow this review/validation.

Unchanged owning2015 runtime3/4pass13checks each; associated constructor is1new measured distinct gain versus V505. Empty-generic has no measured failing baseline; its warning diagnostics remain outstanding. Repr128 is unchanged LRR-E-OP-TYPE. Q80's12world/36clause/12projection pass stays bound to V507, not claimed rerun at V508. Source-only/failed attempts and exact preflight failure are retained. Full root/item-bound/default/negative/local/suspension/cleanup/architecture/performance/interpreter/JIT gates remain open.

Durable evidence: [2026-10-20-v508-constructor-source-scopes.json](../evidence/2026-10-20-v508-constructor-source-scopes.json). Raw artifacts and temporary packaging scripts are disposable; all current inputs and reproduction commands are retained in Git.
