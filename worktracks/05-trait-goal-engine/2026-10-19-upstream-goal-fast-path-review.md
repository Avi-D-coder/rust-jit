# M020 actual compiler goal fast paths — separate solo source review

Baseline400b3dd2a0d3fc2c7119164780592f143f392252; root/LOCAL-049/WT05-02.
Reviewed V364 fingerprint: `4526c4e39ef17cd00377c1f9c0c8cc20015aca860e1f4fbce2e35d8ebe599831`.
Source inspection establishes routes; execution evidence is attached below.

## Actual shared entry points and remaining metadata boundary

Q69/Q70 source Encoder -> retained Cx/InferenceContext -> complete compiler
compute_goal_fast_path/known_no_opaque_types_in_storage plus both stalled-result
constructors and CollectNonRegionInfer visitor. Full selected bodies preserve
NoFastPath/TriviallyHolds/TriviallyStalled, actual wake variables, subtype roots,
opaque empty/erased state and region flags. No constant fallback callback or
custom trait solver is used. Helpers are inherent on existing owner while the
full SolverDelegate trait remains unimplemented; no partial trait implementation
with no-op callbacks is presented as real integration.

Pinned compiler sizedness_fast_path and Ty has_trivial_sizedness,
is_trivially_pure_clone_copy, is_trivially_wf, ty_vid and Term is_trivially_wf are
used. Const ct_vid/WF already live in actual shared type_ir. Compiler LangItem
Sized/MetaSized/Copy/Clone maps to actual SolverTraitLangItem and existing source
services, never source spelling. Adt sizedness instantiation uses retained real
AdtDef/EarlyBinder interfaces. No fake ADT/trait/opaque identity or SourceFacts
provider is installed. Full source facts/ReadyProvider remain incomplete.

Metadata-dependent positive trait/Sized/Copy/Clone/dyn/ADT/ParamEnv branches are
compiled but unexecuted in these partial-source contexts; do not infer them from
primitive WF proofs or unresolved-Self stalls. Genuine source TraitRef constructor
is used with the actual source definition/arguments and fresh unresolved Self,
which follows the original early-stall route before metadata access. Positive
and negative IR polarity cases are supplemental state controls, not proof of
negative impl language execution. Erased empty-store helper behavior is tested
without forging opaque IDs; nonempty source opaque handling stays held. Function
signature-dependent properties and assumptions-on-binders=true profile remain
unexecuted. Unknown supplied source facts are never made complete by a fast path.

ThinVec0.2.21 is the existing locked dependency, now a direct workspace/lrr ref;
only the lrr dependency list changes in lockfile, no version/checksum updates.
Mechanical transports: Cx/lifetime/import aliases, copied TyKind patterns,
explicit stable visitor Result=(), source Span instead of ObligationCause, actual
trait-lang-item/dyn service method names and region-origin argument ordering.
V363 failed6API transport errors. Compiler auto-reorder suggestion would invert
the required outlives direction; V364 explicitly preserves original outlives.1
as sub and outlives.0 as sup, then visibility/span in shared trait order. No
algorithm or fast-path condition is changed to fix those compile failures.

The original <=3 tuple threshold is only a Copy/Clone proof optimization; larger
tuples return false from that helper and the actual trait branch falls back to
full solving. It is not a program cap/negative trait answer. The host primitive
helper threshold is executed; full Copy trait fallback needs real source lang
metadata and remains unexecuted. No performance/scaling claim or custom limit.

## Eight recurring modes (separate from MA001-MA008)

1. No assertion/expression evaluator. Only inference metadata/tooling/source host
   controls change; ordinary AST/thunk/VM/assertion dispatch is untouched.
2. No generated scalar/break route. Original/emit/wrapper/all-edition contexts
   call the same actual functions. No macro identity selects a rule.
3. No context-only member/numeric logic. Upstream predicates/type kinds and
   source lang-item identities select actual source algorithms, not a method
   whitelist, assertion operands, concrete expected output or fixture name.
4. No generated exclusion/fake facts. Existing readiness/currentness guards
   remain. Missing source trait/dyn/ADT data is an open producer dependency,
   not fake false/default metadata, NoSolution or a made-up source identity.
5. No duplicated patterns/eager bodies. BodyStructure, PatternPlan, cold-arm
   preparation and function body queries do not change. State controls are not
   a replacement language suite or original-main acceptance.
6. No parsing/query/IO/macro execution in callbacks. Actual property recursion
   follows retained type handles; CollectNonRegionInfer memoizes visited types.
   Source services consume prepared headers. No installed source planning is
   added; cold upstream traversal costs stay and no measured speed is asserted.
7. No global namespace map/unrelated invalidation. Transient state belongs to
   the same inference context; no new final answer cache/world census. Existing
   source dependencies own full metadata publication and invalidation.
8. No BodyIndex copies/whole-file searches. Only IR handles and upstream visitor
   collections are used. No frame/entry/immutable-body representation changes.

Historical MA004/MA007/MA008/common-engine matrices remain separate. No local
unsafe/resource accounting/sandbox/static checking/native guest compilation or
JIT work. Full SolverDelegate/root/canonical response/source facts/ParamEnv/
ADT/variance/function binder/opaque/member/dictionary/currentness/VM integration
and obsolete custom solver-consumer removal remain open. Latest actual stock
runtime evidence is V356 configured multidispatch1 get ambiguity; no rerun of
untouched call paths/new stock gain/suite total/full M020/interpreter/JIT/Linux/
performance/diagnostic compatibility follows from these source controls.

V364 host-control failure: the new escaping-region case used the general goal
closure's Binder::dummy around a bound region. Shared Binder correctly rejects
that malformed host IR before compute_goal_fast_path can run. Do not remove the
constructor guard or alter production semantics. Construct the actual higher-
ranked predicate with Binder::bind_with_vars and its real anonymous Region kind;
then skip_binder inside the original fast path legitimately exposes the bound
region and selects NoFastPath. This is a host witness correction, not source
language/trait acceptance. Preserve V364 failure and untouched control outcomes;
keep production unchanged, rerun only affected control after frozen final builds.

Actual remaining routes inspected: traits/mod.rs still compiles
traits/unify.rs::Unifier::{unify,register_variable,with_universe} and its validate
helper. That older structural implementation is not the upstream inference
owner. Its fixed MAX_DEPTH=256/"signature unification budget" remains explicit
E-R5-006 repair debt; this checkpoint neither calls it from the new bridge nor
extends its vocabulary. Do not promote the depth guard as the configured rustc
recursion limit. Existing runtime engine/run_trait_calls.rs::{select_deferred_trait_call,
select_by_arguments,select_trait_impl_function} and frontend/member_index.rs::
MemberIndex::{select_method,method_alternatives} still select source alternatives
without the completed upstream bound/trait-argument witness/dictionary bridge.
The V356 get ambiguity corresponds to select_deferred_trait_call's None result
from select_by_arguments (line720 at baseline). No consumer of these routes is
migrated here. Upstream root + exact selected impl/member evidence through both
ordinary/generated routes must replace their semantic selection before obsolete
local inference/dispatch routes are removed. No additional shortcut rules should
be added to them; their migration/removal and non-rustc limits stay prerequisites,
not precedent or a reason to claim this component completes trait integration.

Final V365 reviewed source fingerprint `9dee10f5c746c8db21940578711888c7026a2ada42560b5be9e1e49b6ae2b7b6`; only infer/tests.rs differs from V364, production byte-identical. Library22484/control32627 terminals exit0; CLI direct chunkfbb8bb exit0, no session.1/1affected source control passes with exact source/head/test/CLI guards:140Type heads/136source TraitRefs/48origin-edition worlds, each genuine TraitRef exercising both IR polarity unresolved-Self cases.2untouched V364 controls retain original passes; not all3at final source. V36429537/13862 exit0, controls23608 exit1 due malformed host dummy binder; V36312081 exit101/6API errors. Projection/vendor58434 exit0 verify8crates/183files/2742155bytes,0original edits/5existing projections. Only existing thin-vec0.2.21 direct dependency refs added, no version/checksum changes. Metadata-dependent branches and full delegate/root/runtime/gates stay open. Commands/hashes/terminals: worktracks/evidence/2026-10-19-v363-v365-upstream-goal-fast-path.json.
