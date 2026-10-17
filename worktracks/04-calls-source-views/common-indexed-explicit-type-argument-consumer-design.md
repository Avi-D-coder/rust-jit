# Indexed explicit Type arguments through the common call route — LOCAL-024

Bounded implementation design, source-only, WT04/T032/T033/M017/M018 under
E-R5-005. No production edits, imports, Cargo or runtime evidence. Root owns
normal Type/call-contract/coordinator integration. Existing V10 validation is
independent. The feature, all-context matrix and milestone remain incomplete.
Applicable MA003/MA004/MA006/MA008 are distinct from recurring failure modes 1–8.

The invariant and unchanged witness

Every explicit call Type is selected at its actual caller-body AST occurrence,
with caller scope, source, hygiene, edition, epoch and lexical prefix. A source
name selection or matching runtime descriptor cannot supply this authority.
`fn main(){ type u8=u16; fn f<T>(){} f::<u8>(); }` must retain AliasContext or
contextual-normalization failure, never select the module primitive spelling.

Primary witness is the ALREADY imported, complete unchanged official
`fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs`
(upstream `rustc/rust/tests/ui/macros/macro-nested_stmt_macros.rs`, run-pass,
SHA256 abef2c55f007cf929838a1cae6e3e329f8bf4747979db3e78b11580d5b86eb49).
Its actual main calls grault, which calls foo to declare Bar/Baz and adds Xyzzy;
then it performs three ordinary module calls
`static_assert_exists::<Bar/Baz/Xyzzy>()`. The selected declaration is
`fn static_assert_exists<T>() {}`. The call has zero value parameters and unit
return, but unused T still requires real source binding. No empty-body/name,
unit-struct or macro-identity exception is allowed. Monomorphic nominal structs
and enums with fields use the same opaque descriptor path; field layout is not
required merely because a Type is an explicit generic argument.

Official issue-75299.rs and its actual `foo::<1>` remain a separate const
argument/purity/substitution/array-return prerequisite. No Type-only consumer
may reinterpret that ConstArg, or treat a parser TypeArg naming a const as a
Type when the selected declaration says GenericKind::Const.

Exact prerequisites and current exclusions

Future frozen GenericArg metadata packet35ecb727 (goal189258cb, tests149bd7d3)
adds actual AST observations. Module proof/goal join and base-name facts are
separate held future components, not evidence of integration into V10. The
final module admission packet a67f7de2/f2d2c530 and goal join49133ded/ccfd are
prerequisites; root must compose exact interfaces without silently replacing
historical frozen sources. Current live sources are fingerprinted in the
adjacent source record. No packet is promoted by this design.

`EnteredSourceName::arguments() -> &[EnteredGenericArgument]` preserves actual
written order. Each argument has `kind() -> EnteredGenericArgumentKind`,
`range() -> SourceRange` (absolute GenericArg AST range), and
`type_argument() -> Option<&EnteredTypeArgument>`. Indexed Type data exposes
`key() -> &PcKey`, `range() -> SourceRange`,
`indexed_range() -> SourceRange`, `is_path() -> bool`.
`EnteredTypeArgument::Pending {range,demand}` is metadata, never permission.
The AST association comes from actual TypeArg.ty in the selected PathExpr;
canonical Syntax NodeId is obtained from the actual owning BodyIndex at the
checked body-relative range. Absolute normalized retained coordinates must
never be treated as raw SourceBlob offsets. is_path proves shape only.

Current `Engine::prepare_entered_source_call` holds nonempty explicit data with
“explicit source call arguments require indexed Type or const goals”. Keep that
hold until the typed consumer is fully connected for the admitted cases. The
old `prepare_selected_argument_call(...,syntax:&[String],...)` constructs
caller FunctionSignature SignatureNames and calls unqualified(text), filters
callee generic parameters to Type, then accepts only primitive/Param layouts.
That spelling path is superseded for entered calls by this design. Do not let
an entered Local or Module call fall back into it when a typed goal is pending.
Legacy/unjoined call modes remain named and held until separately migrated.

Cold association and Type preparation

1. Require the actual EnteredCallGoal/current selected callee proof before any
   argument effects or callee BodyStructure. Module selection requires genuine
   EnteredModuleAdmission::revalidate/callable_shared, not module lookup by name;
   Local uses the same SourceCallable authority. Use actual canonical
   FunctionSignature from that proof. Signature-only discovery does not lower
   the callee body.
2. Associate each written AST argument with the actual DefinitionParameter
   ordinal/kind in that signature BEFORE querying a Type. The signature stores
   `generics:Vec<DefinitionParameter>` with real `ordinal:u32`,
   `kind:GenericKind::{Lifetime,Type,Const}`, names/bounds/defaults. The ordinary
   unambiguous Type-only case can traverse this full sequence directly,
   preserving ordinals; do not create a filtered Type list and renumber it.
   Partial inference/defaults continue through existing source constraints;
   unsupported mixed-kind/lifetime elision/associated-binding association is an
   explicit typed demand. A TypeArg/Const parameter mismatch does not query Q42
   and does not become primitive fallback. No fixture-shaped empty generic API.
3. For an admitted Type parameter and actual Indexed Type argument, fetch
   `QueryKey::PreparedTypePlan(site.key().clone())` (Q43) once. It wraps
   `QueryKey::EnteredType(PcKey)` (Q42); phase must be zero. Q42 already obtains
   actual `plan.context().body_type_occurrence(db,NodeId)` and calls
   `PreparedSourceLocalPlan::prepare_named_body_type(self:&Arc<Self>,
   db:&mut Database,node:NodeId)->QueryPoll<NamedTypePreparation>` through the
   common type_prefix/lexical/module/header finalizer. Do not call
   SignatureNames on explicit strings or parse a second Type expression.
4. Named lookup must honor the actual caller prefix and lexical Type rows before
   module/builtin absence. PreparedBodyType/BodyTypeDeclaration binds genuine
   source/header/context/AST/declaration occurrence and schema publication.
   Actual retained/generated origin may return NeedGeneratedBoundary,
   NeedRetainedOccurrence, RetainedDeclaration or PinnedAdjustment. Preserve
   those demands; Bar/Baz/Xyzzy success is NOT established from declaration
   spelling or the fact the uses survive original syntax. The use's raw-token
   provenance and each generated declaration's origin are independent joins.
5. Q42 currently accepts only NamedTypePreparation::Ready(SourceType). It rejects
   Parameter (“needs reached source substitution”), Alias (“needs contextual
   normalization”) and unresolved Name. Keep those typed gaps for this first
   concrete consumer. A body alias may fail earlier as BodyTypeDemand::AliasContext.
   A caller GenericEnv descriptor hint cannot erase its Param source. Generic
   nominal schemas/qualified/composite/infer syntax remain explicit demands.

Minimal common Type-use API extension (proposal, not implemented)

```rust
impl EnteredTypeGoal {
    pub(crate) fn source_for<'a>(
        &'a self, index: &BodyIndex, site: &PcKey,
        absolute: SourceRange,
    ) -> Result<&'a SourceType, Diagnostic>;
}
```

Factor the existing primitive_for use check, adding equality of actual Type Pc
(including node/phase), actual body/index semantic identity, exact cached Type
occurrence range and checked direct index/body-relative projection. The existing
primitive_for remains a cast-specific projection through the same use check.
Do not weaken primitive_for to accept nominal casts. A bare source_type getter
is not an occurrence-use check. For the admitted plain Named Type, TypeArg.ty
and Q42 path_range must match; nonmatching/qualified shapes stay held.

Prepare a private immutable ordered call association once: each row retains
written argument index/ranges/Type Pc, genuine callee ParamKey{definition,
ordinal}, and its actual Q43 plan/guard. A pure external prepared-call query can
combine Q44 selection, callee signature proof and all Q43 dependencies. Root
chooses the existing query integration point/key; do not invent a second name,
context schema or self-capture Q44 while it is evaluating. The old String list
may remain normal-lowerer compatibility metadata, not execution authority.

Guard closure and retention

Q43 captures the actual Q42 result with Database::prepare_input_guard, not a
global revision or namespace census. Retain the actual PreparedInputGuard and
immutable source plan. Relevant closure includes caller DefinitionSource,
DefinitionBody/BodyIndex ownership and admitted retained syntax/epoch/boundary;
actual lexical/source-local plan and relevant Type buckets (including negative
lookup dependencies); selected nominal DefinitionHeader/DefinitionSource/
DeclarationContext/Signature/TypeSchema/cfg and owning edition/context; selected
callee canonical header/signature/generic ordinals and source dependencies.
Use existing query dependencies to obtain this closure, not an ad hoc manually
invented dependency row. NeedInput must suspend normal preparation.

Before reached argument selection/effects/body demand, require Q44 full
callee/selection proof plus every actual Q43 guard. Recheck retained versions at
existing normal call/frame boundaries. Installed operations consume retained
plans; never re-fetch/reparse each explicit argument per operation phase.

Holding Arc<QueryValue> shared with a memo can hide an external parent-plan
reference from SourcePlanPins. Root must explicitly account new association
roots: either own a once-cloned PreparedTypePlan (its existing Clone shares
parent and guard Arcs, not BodyIndex) and enumerate its retained_plan, or retain
memo owners with an explicit external parent handle using the established
accounting pattern. Do not deep-copy parser/body graphs or silently add a new
registry. Bound row count, vector capacities, SourceType term heap, guard/Arc
headers and extra parent handles under existing metadata budgets; encode actual
ordered Pc/ranges/ParamKey/proof identities with an explicit stable domain.

Reached source binding and lazy descriptor

The existing APIs already preserve concrete source identity:

```rust
SourceType::source(&self) -> &TyTerm;
SourceType::find_descriptor(&self, store:&GuestStore) -> Option<TypeId>;
SourceType::materialize(&self, store:&mut GuestStore) -> Result<TypeId,Diagnostic>;
SourceType::require_descriptor(&self, store:&GuestStore, actual:TypeId)
    -> Result<(),Diagnostic>;
```

At the reached call, after the guards, borrow/clone the certified symbolic term
and find/materialize its matching descriptor only when the existing call
contract requires it. `prepare_nominal` fetches only selected TypeSchema header
metadata, verifies actual source/header/context and non-generic schema, then
constructs TyTerm::Nominal and certified TypeKey::Nominal. No TypeSchema field
Type/layout or callee executable body is evaluated here. Runtime TypeStore
intern retains DeferredFieldRecipe::Nominal and CopyStatus::Deferred; it does
not force the fields or compute Copy/Drop simply to bind unused T. This same
mechanism applies to monomorphic structs/enums with arbitrary cold fields.

Build existing `(ordinal,ArgumentType {source,descriptor})` for the genuine
callee ParamKey. `SignatureSubstitution::bind_type_parameter(ordinal,&TyTerm)`
already rejects undeclared/non-Type ordinals. `call_contracts` explicit loop
binds both source and runtime substitutions; existing preflight_evaluated_call
checks budgets/arity/no transaction variables/nonzero descriptor. Keep nominal
source proof paired with its certified descriptor; no reverse descriptor-to-Type
construction. `GenericEnv::capture_current(db,&SourceCallable,&contracts)` then
retains accepted source/runtime tables for that callee, and ordinary eager
argument order, call seal, frames, return/cleanup and suspension remain owners.

For future caller Param support, GenericEnv exposes symbolic_binding(ParamKey)
and runtime_binding(ParamKey); a common checked caller-context projection must
provide both. Current Frame::parameter_hint exposes runtime only and is not
sufficient authority. Preserve Parameter/AliasContext gaps until those common
services exist; do not transport foreign rigid keys with guessed descriptors.

Required evidence and all-eight source review

First observe the actual three indexed arguments and Q42 outcomes in complete
unchanged main, no synthetic helper body; record any actual pending declaration
origin instead of bypassing it. Then frozen pure association controls cover
wrong caller/body/index/range/Type Pc/ordinal/kind/signature, stale source/header/
cfg/epoch/namespace absence and generated declaration; alias-u8 refuses; caller
Param/Const/lifetime/associated/composite gaps remain exact. Include multiple
Type arguments and real ordinals, not only one unused generic. Verify a nominal
with unsupported cold field Type/layout can be passed as unused T without
requesting field layout or callee code before reached entry. An unrelated
namespace publication must not invalidate; relevant additions must invalidate
actual absence. Caller effects exact once across suspension/retry.

Actual full unchanged fixture all4 editions and ordinary/user/wrapper/applicable
standard-macro routes remain necessary. Query traces must show actual Q44/Q42/
Q43/canonical source contracts, one retained association, no string Type fallback,
no field-layout request and no eager unrelated callee body. All tests unexecuted
by this design; no runtime/fullfixture/matrix/milestone or guest JIT claim.

1. Same ordinary source-call lowering/contracts; no assertion evaluator.
2. Actual generated declarations and ordinary indexed Type uses, no scalar lane.
3. No macro/name/primitive whitelist or string-based numeric inference.
4. Generated use/declaration provenance must join normally; typed gaps remain.
5. Header/schema metadata only, no unselected executable/body/layout preparation.
6. One pure guarded association/Q43 result set retained across phases/entry.
7. Relevant sealed Type buckets/positive and negative deps, no global census.
8. Shared source/body/index Arcs, direct indexed Type sites, bounded metadata.

Superseded entered explicit-string resolution must be removed from that route
when this replacement is integrated; legacy independent paths remain named held
until their ordinary source consumers are migrated and evidenced.

Root implementation boundary after the stock shadowing fixes

Root owns ordinary/module admission borrowed signature getters, common entered
call site validation, a pure memoized explicit-Type argument plan and reached
call integration. The Type-use helper worker owns entered_type_goals.rs only;
the scanner worker owns trait/associated capture; the validator owns the new
unchanged full nested-statement-macro main baseline. No frozen validation source
is edited. Existing Type/const argument refusal stays until the actual indexed
consumer replaces it; no promotion from string-based nominal lookup.

The common call goal must expose the already captured canonical signature
without minting a SourceCallable or publishing/lowering a callee body. Both
Local and Module selections use one indexed call/path/TypeArg site validator;
Binding/Pending remain non-callable facts. A memoized explicit argument query
will associate actual AST argument kinds with actual signature generic
ordinals, fetch the shared Q43 Type-use plan at each real canonical TypePc, and
retain its guards and selected SourceType facts. A runtime projection may intern
an opaque nominal descriptor through SourceType::materialize; it must not demand
field layout or fabricate source identity from that descriptor. Contracts use
the existing source/runtime substitutions and ordinary GenericEnv. Preserve
partial inference and unsupported alias/Param/const/lifetime/structural demands;
do not implement an empty-callee or unit-struct exception. No installed recipe
re-enters source planning. Retained root/guard/term metadata must be bounded and
accounted; cold memo dependencies include actual caller source/index/edition,
selection/signature and each Type-use closure. Runtime entry rechecks relevant
versions without a namespace census or source walk.

This is an implementation prerequisite, not a completed generic-call feature.
Acceptance requires the actual unchanged full main, selected nominal identities
and generic substitutions, caller alias-shadow refusal, cold callee bodies,
source invalidation and the common source-context matrix. The V10 shadowing
receipt remains attached to its original source and does not prove these calls.

Guard composition: add one reusable cold `PreparedInputGuard::join` within the
existing guard framework. It accepts only already minted guards, merges actual
Input stamps in deterministic key order, rejects conflicting stamps/derived
Query stamps and bounds cumulative visits, transient map bytes and retained
capacity. This is not a lookup or authority. Q47 joins full Q44 plus actual Q43
closures once; ordinary phase recipes share that immutable joined guard. No
per-phase DAG recapture. Existing hot checks inspect only relevant versions.

Installed execution correction before freezing: fetching Q44/Q47 during
CallPrepare/CallSeal could reconstruct source after memo eviction. This approach
is superseded before validation. Q47 instead retains opaque Local/Module
admission proof handles plus an explicit parent-plan handle and actual written
metadata for operation agreement only. VerifiedRecipe retains the actual typed
plan and joined guard; SourceArtifactRef enumerates its physical parent handle.
InstalledRecipes selects it by the installed actual PC, and runtime creates the
same SourceCallable from those retained proofs. Local body locator publication
remains the existing reached coordinator action. It does not perform Value or
Type resolution. Runtime selection must contain no Q44/Q47 fetch or source walk;
the old module/string lookup remains only in explicitly unjoined legacy paths.
Each runtime clone shares immutable proof/row/guard allocations and retains its
own parent handle, never a deep BodyIndex clone. Source fingerprints and cold
query traces, including cache-eviction retention controls, are required before
claiming this invariant accepted.
