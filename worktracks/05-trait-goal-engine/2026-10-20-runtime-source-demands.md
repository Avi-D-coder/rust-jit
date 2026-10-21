# M020 / LOCAL-049: numeric-connected source clause demand metadata

## V531 read-only handoff: common future-use metadata prerequisite

This is an implementability analysis during the V531 production freeze, not an
implementation or runtime result. M020 / LOCAL049, Sections 11, 52.2 and 53,
unchanged owning-2015 `repr128.rs` remain the task/invariant/witness. The concrete
next boundary is a source-relation producer shared with BindingConstraints;
another primitive or candidate inference algorithm is prohibited.

Actual APIs already available:

- `db::source_constraint_provider::prepare` / `QueryKey::SourceConstraints(body)`
  fetch retained `BodyStructure`, actual DefinitionBody/DefinitionSource and
  ParseFile, then produce `SourceConstraintPlan`. `fact(node)`, `uses(node)`
  (sorted reverse child edges), `name_of(node)`, source annotations/type shapes,
  path argument nodes and pattern occurrences are available without guest thunk
  preparation. `SourceName::{paths,bindings,hidden}` are candidate filters only.
- `binding_constraints::{selected_local,pattern_binding,binding_pattern,
  declaring_let}` join those candidates through actual `EnteredValue(PcKey)`
  hygienic binding identities. `Component::declared` visits all same-binding
  uses, including later uses, after this join; hidden/unrecorded uses are held.
- `Component::{inward,outward}` and `Via/Step` describe source connections.
  `binding_constraint_generics::relate_signature_type` relates the actual
  selected declaration's parameter/result occurrences, using declared terms;
  `source_parameter_goals` / `source_return_goals` produce genuine definition,
  signature and ordinal-checked terms. Existing query variants include
  DeclaredParameterGoal, ContextParameterGoal and DeclaredReturnGoal. None needs
  a selected callee's executable body.
- `PreparedBindingPlan` shows the existing memo+PreparedInputGuard retention
  pattern, but its payload is a solved legacy result, not a reusable unsolved
  relation plan. `PreparedBindingConstraints` exposes primitive/written/inferred
  answers and scalar member patterns; it does not expose the complete source
  relation component.

Do not call `BindingConstraints` merely to obtain this graph. `Component::solve`
interleaves discovery with `constrain`, candidate collection and fallback.
`relate_parameter` reaches `parameter_bounds` / `trait_bound`, which enumerate
MemberIndex primitive candidates and recursively interpret blanket bounds.
Those historical semantic paths are migration debt. `RelatedNumericPlan` is
also insufficient: it is made only on certain unresolved literal outcomes,
omits named/variant/Item projections, and describes related reached runtime
descriptor observations rather than complete authentic source obligations.

Smallest shared producer change (proposed API, not present): factor a retained
`PreparedNumericSourceRelations` out of the existing Component source discovery,
keyed by phase-zero source occurrence `PcKey`, with `relations()` and explicit
`pending_leaves()` plus actual query guard. It records indexed endpoints,
source spans, hygienic bindings, declared signature IDs/ordinals and source
operation/structural-projection requirements. It returns no primitive, inferred
argument, candidate set, default or solved flag. Both the old BindingConstraints
consumer and the new upstream adapter must use that one retained producer;
copying Component into another visitor is not the repair. Discovery must not
convert an overloaded operator, coercion or projected field into a proved
equality: retain the authentic requirement for the pinned relation/trait owner,
or a typed missing common-source leaf. Existing source relation forms must be
ported incrementally with their holds; no universal graph-closure claim follows
from the scalar subset. Arbitrary nonnumeric open-type future-use inference is
outside this Section52.2 numeric exception.

Actual reached packet gap: `run_call_inputs::capture_reached_call_inputs` maps
live `(ValueRepr.kind,payload)` LiteralRecord handles to private original numeric
InferVarKeys, and retains slots/cells/value snapshots, actual PC/body/callee,
attempt Arc and result expectation. It does not retain numeric-key -> indexed
source-expression occurrences. `LiteralRecord` has number/spelling/binding only,
so source identity cannot be recovered from its spelling or handle payload.
Extend the same capture with authenticated argument-expression nodes from the
actual call's BodyIndex/SourceConstraints operands (Call has callee offset;
MethodCall/LangCall use their genuine parameter order), joined to the exact
observed slots/ordinal and retained body/attempt. A local path argument can then
join its declaration/later uses through EnteredValue, rather than needing the
original literal's allocation site. Actual generated-call inputs require their
same ordinary indexed occurrence join; the legacy runtime-call argument slot
array or borrow-only span helper does not certify arbitrary argument nodes.
Missing join remains typed pending, not an original-only exception. Keep this
mapping in the private packet/currentness owner, separate from pure memo keys;
revalidate the new source guard alongside existing `require_current` checks.

Separate Q80 fragment prerequisite: `SourceDeclarationClauses::prepare` demands
every recipe's SourceBindings and PreparedClauseEmission, and its whole pending
gate also guards `encode_recipe_at`. Therefore merely choosing ordinals before
calling that accessor does not avoid an unrelated unsupported bound. The common
collector already has `ActualBoundSite`, actual syntax locations and the owning
WrittenTypeDeclaration, but `PreparedWrittenBounds` does not expose a retained
pre-name-resolution parameter-use index. Extend that collector with authenticated
fragment identities and source ParamKey incidence, using actual binder-aware
parameter resolution rather than spelling. Resolve each possible connected
fragment through the same `prepare_written_rows`, Q84 defaults, SourceBindings,
PreparedClauseEmission and `encode_owned_recipe` policy; retain real source
ordinal/site and per-fragment pending. Do not feed a caller-supplied recipe
subset into a fake complete SourceDeclarationClauses value. A selected Type
parameter default needs its actual relaxation/default context, not an assumed
Sized row. Unknown dependency incidence cannot certify disconnection.

The current same-owner actual callee Args and original numeric family keys can
identify which retained source parameter occurrences may contain a still-open
numeric root, using pinned instantiation/resolution. This is source-demand
selection only. A source parameter that instantiates to a ground nonnumeric Type
does not make all its other bounds necessary merely because it occurs beside U
in one selected clause. Semantic inference/normalization/fulfillment remains
exclusively in the existing pinned root owner; no primitive enumeration or
independent candidate/constraint solver belongs in the collector.

For actual repr128, the reached second argument at `discr(Signed::Zero, 0)`
provides the original Integer key; the authentic `v:T,value:U` signature relates
U to that key through upstream coercion fulfillment. The actual Q80 row
`<T as DiscriminantKind>::Discriminant: PartialEq<U>` is connected by U and must
be transported with exact caller ParamEnv/callee Args/source span. T remains the
actual Signed source Type. The clause's complete parent/outer candidate buckets
are metadata prerequisites, not an inferred width. No body call to
discriminant_value or assertion identity is needed to select this source demand.

Cold cost is retained indexed source discovery and relevant hygienic/signature
queries; read each connected source occurrence/fragment once behind memo guards.
Subsequent phases select from immutable handles. No guest thunk, branch execution,
full-world dispatch map, body clone or repeated parser plan is authorized.
Eight-mode source assessment: (1) no expression evaluator; (2) ordinary indexed
generated/original join; (3) no method/numeric selection in metadata producer;
(4) missing generated joins typed; (5) numeric metadata may read cold syntax but
must not prepare its executable arms; (6) one retained source plan; (7) relevant
name/query dependencies only; (8) shared Arc indexes/direct NodeIds. The existing
Component semantic/deep projection routes remain explicitly held migration work,
not endorsed by this design.

Recommended existing source controls to preserve: unchanged
`integer-literal-suffix-inference.rs` via
`unchanged_suffix_shadowed_bindings_use_actual_later_parameter_source_goals`
already checks actual later parameter Types, memo reuse and no Thunk/PatternPlan
or foreign callee BodyStructure queries. Retain the existing
`unanswerable_numeric_relations_stay_explicit_and_never_guess` distinctions.
Add new producer/consumer assertions outside imported originals for unused
unsupported callee bounds, connected later uses, shadowing, cold branch
literal-to-source-call changes, genuine generated/wrapper source identities and
NeedInput/currentness; runtime exact-once/VM commit remain separate. No control
was run here. No production/source/Cargo/Git edit occurred during this analysis.

Inspected source SHA256: `binding_constraints.rs`
`008f5da74ae56faf1a14231f576408b02c21875dfc546c2a31db369280aa8eb3`;
`binding_constraint_generics.rs`
`410279d968022da849d2b87db4626dd18e310f4bed470bcb07e83936d1d7a5e4`;
`syntax/source_constraints.rs`
`73d88ea7e0aea13aa79249ede1724b17e4040e663d6419b773a25c2553d74991`;
`run_call_inputs.rs`
`7f464692a29479941f5dde1c4d39d67c2957e7cb94a73a64473eba8da75778be`;
`source_declaration_clauses.rs`
`cd801eae04fb7eae8daed1ff7ee870abd0e294b59e903ba97ce56852eefe0513`;
`declaration_source_predicates.rs`
`98ebf0ee045f29de94fda295a5b3d379a432ad36e3661622ad4d597039486fe4`.

Bounded precode reservation: new `traits/upstream/source_runtime_demands.rs`
and this record only. Parent owns registration, source factory/root/engine
integration; the assigned fulfillment worker owns the applied root application
view. No Cargo, Git, production module registration or existing source edits.
V526 terminal and closing guard completed before source creation was released.
The primary unchanged runtime witness remains
`fixtures/upstream/rustc-checkout/tests/ui/enum-discriminant/repr128.rs`.

Section 11.3 and 52.2 require relevant source numeric constraints before fallback,
including later uses, without executing uses or preparing their guest thunks.
Section 53 requires the actual pinned next solver for semantic inference and
trait fulfillment; an ordinary unused callee bound must not be forced merely
because the function is called. The missing prerequisite here is a retained
source-clause demand description connected to actual still-open original numeric
variables after actual upstream argument relations.

The shared entry points are Q80 `PreparedSourceDeclarationClauses::encode`,
`PreparedRootRelations::apply`, the nonconstructible owner-checked
`SourceRootApplication` supplied by that operation, `RootVariables` original
Integer/Float keys, and pinned deep resolution / EarlyBinder / TypeVisitor.
Retain actual Q80 key/fingerprint, clause ordinal and original span. Instantiate
with the actual retained callee Args from that same owner, never a rebuilt or
externally supplied argument vector. Read original numeric variables through
pinned resolution and inspect their actual canonical still-open representatives.
Only clauses with those actual numeric occurrences become demands. No trait
matching, primitive candidate enumeration, width choice, fallback, fulfillment,
normalization or semantic closure loop belongs here. Implicit Sized clauses may
mention a numeric variable but cannot manufacture its width.

Encode Q80 once in a provenance-bearing retained input, then select from that
immutable input after relations. Missing Q80 data remains its typed pending
outcome; neither a partial vector nor unsupported syntax becomes empty facts.
Ground unused source clauses produce no demanded root goals. The output is
metadata, not a proof that every numeric source constraint is discovered or
fulfilled, and never a root/VM commit certificate. Bundle currentness/source
receipts remain owned by the actual root preparation owner.

Cold costs are one actual clause encoding, then pinned instantiation/deep
resolution/visiting of retained clauses and original numeric variables. No
source planning, parser/body cloning or namespace census belongs to installed
execution. Common future-use source discovery still needs the ordinary
`syntax::source_constraints::SourceConstraintPlan` and
`frontend::binding_constraints::Component`/`BindingConstraints` join. This helper
must not become an independent primitive inference engine or substitute for
that connected source service.

Parent owns compilation, actual type/trait goal admission, solver fulfillment,
source invalidation/suspension/exact-once/origin/edition evidence and unchanged
runtime acceptance. Source-only metadata and a larger list of selected clauses
cannot close M020, interpreter/JIT, a full fixture or performance gates.

## Source handoff and separate source review

Candidate `source_runtime_demands.rs` SHA256:
`010e55030a60d7793fe6cd54a55d2af23f7d704f1d0016affd4a0f57a7dfce16`.
No production file outside that new module was changed by this worker.

`EncodedRuntimeSourceClauses::prepare(cx,configuration,&Q80)` is the sole input
constructor. It retains the actual Q80 plan by borrow, its exact key/fingerprint,
real source DefinitionId and same-context encoded clause/span vector. No raw
clause-vector constructor is exposed. This can be prepared once before argument
relations; encoding needs actual available Q80 metadata but does not fulfill any
callee bound. A pending whole Q80 plan remains pending; this component cannot
certify a partial supported subset as a complete vector.

`SourceRuntimeDemands::after_relations(owner,variables,&SourceRootApplication,
&encoded)` accepts only the actual nonconstructible application view supplied
by the assigned relation owner. It checks owner, snapshot, context and exact
callee DefinitionId. Upstream EarlyBinder instantiation uses that view's real
retained callee Args. Returned ParamEnv is that same actual caller environment;
callee bounds never become assumptions. The application records argument
relations, not successful fulfillment of all emitted child/coercion goals.
Parent must preserve those outstanding goals and their actual owner.

Original Integer/Float keys retain their families; an original general Type key
is never relabeled numeric. Pinned deep resolution canonicalizes actual merged
numeric roots and ignores already-concrete numerics. The occurrence visitor
records every original numeric key sharing each actual still-open numeric
representative. Each selected clause retains its actual source ordinal and
original span plus instantiated TypeIR; full source identity is Q80 key and
fingerprint. Clause order/duplicates/spans remain as encoded, and inputs/outputs
use context handles or immutable borrows, not copied parser/body structures.

The helper inspects direct structural numeric occurrences after existing
inference equalities, including within retained alias arguments. It does not
normalize aliases or discover semantic/transitive connections between other
clauses. Selecting a Sized bound conveys a demand only, never a numeric width.
A supported but ground unused bound is not selected. No numeric original left
open means no numeric source demands. None of those outcomes certifies that the
complete source numeric component is closed, suitable for fallback or already
fulfilled; ambiguous projections/other numeric constraints remain separate work.

Separate all-eight source-review pass at the candidate fingerprint:

1. No assertion/expression evaluator or alternate Rust interpreter is present.
2. No generated-output literal/break path or macro-dependent execution.
3. No trait name whitelist, primitive candidate enumeration, matching, numeric
   width/default or independent relation algorithm; pinned inference resolution,
   instantiation and structural visitor APIs alone supply the observations.
4. Q80 and actual root application carry ordinary/generated source identity;
   no generated exclusion, fake span, synthetic source body or rebuilt Args.
5. No patterns/static checker/body preparation/unused-bound fulfillment. Only
   metadata encoding and numeric occurrence inspection happen; ground bounds
   never become requested solver goals merely by appearing in this plan.
6. Actual Q80 encodes once into retained input; the selection phase reads it.
   No DB/source parsing/planning or complete input reconstruction in the visitor.
7. Exact source/world/application owners are checked. No namespace census,
   full-world map or unrelated publication guard is introduced in execution.
8. Source plan is borrowed; clauses/Types are immutable context handles. No
   BodyIndex clone, parser clone or repeated whole-file node lookup is added.

Cold costs are proportional to actual original numeric variables and retained
clause TypeIR visited, with ordered original-key sets and keyed representative
lookups. There is no semantic worklist/fixpoint and no measured speed claim.
Future use discovery must connect ordinary SourceConstraints/BindingConstraints,
its Component graph, real call application/source identities and the existing
runtime literal binding/continuation owner. Implementing another primitive
candidate inference engine here would violate the intended boundary.

Parent plans to compile this helper during V527 but the existing actual factory
control does not execute this helper. Therefore a successful compilation would
remain compiled-unexecuted component evidence for this module. No new helper
execution, root, unchanged repr128 runtime, origin/edition, numeric fallback,
suspension or exact-once acceptance is asserted by this handoff. No Cargo/Git or
runtime command was run here. The new source is held unchanged for parent freeze.

### Corrected argument-relation precondition before parent validation

Parent review identified an activation gap in the first fingerprint: actual
`PreparedRootRelations::apply` applies Equal constraints immediately, but it may
emit equality-child goals and Coerce goals whose inference effects have not yet
occurred. A callee U could therefore still be a general Type variable when an
original Integer variable should constrain it after actual coercion fulfillment.
An empty incidence list at that stage cannot prove absence of numeric demand.

The relation owner now supplies immutable
`SourceRootApplication::outstanding_relation_goals()`, computed solely from the
actual successful apply's emitted obligation count and preserved across typed
parts transfer. No caller bool, raw constructor, setter/decrement or fabricated
fulfillment receipt is exposed. The helper checks this genuine count before any
numeric incidence observation and returns typed
`SourceRuntimeDemandPending::RelationGoals(count)` when nonzero. It does not
solve/decrement those goals. Future actual same-owner fulfillment must provide
its own certified continuation before this activation hold can be removed.
An actual zero count still is not a complete numeric/root/VM certificate.

Corrected helper SHA256:
`56a69cbb11e4892af1b08d28e8521456d78a5d28880c72f605ca21c7e8aad2d0`.
The earlier fingerprint/review remains historical and lacked this precondition;
the structural all-eight review above was rechecked for this correction, which
adds only an actual origin-count hold and no inference/solver/effect execution.
No compile or execution evidence exists for either helper fingerprint here.

Independent source review found no local semantic shortcut in the first helper,
but explicitly flagged a separate activation limitation: its input constructor
encodes the whole Q80 plan before incidence filtering. An unrelated unsupported
callee bound can therefore hold the entire metadata plan. Ground supported
unused clauses are not fulfilled; unsupported-source partial demand still needs
the common source-clause fragment producer before runtime activation. Do not
claim this component closes that lazy preparation contract, swallow pending
rows, narrow full source coverage or grow a parallel evaluator. The corrected
relation-goal hold does not repair this separate source-demand limitation.

### V527 frozen-source production join audit (read only)

M020/Section53 unchanged repr128 remains the acceptance witness. The violated
join is concrete: normal standalone entry gives RunMachine the completed Value
source environment, while `SourceDiscovery::for_impl_catalog` has only
preentry-loader test callers. `cli/mod.rs` polls the actual RunMachine once and
returns unsupported on `RunPoll::NeedInput`; capture mode observes that pending
poll immediately. No successful source-factory component can supply the missing
production continuation by itself. This audit changes no production source and
claims no runtime acceptance.

The smallest common extension is a retained lazy catalog plan plus a host poll
loop, rather than another interpreter or IO provider:

1. Split configured_catalog.rs::for_impl_catalog into authenticated plan/key
   preparation over the existing PreparedCompileInputs/source publication and
   actual discovery start/drain. Retain the genuine original selections,
   generation, source dependency identity, compiler profile/environment/engine
   and epoch already stored by ImplCatalogPlan. Expose an opaque authentic
   catalog request key/locator to the pending source-factory path, without
   publishing a seal or completing all producer closure at startup. The existing
   InputRequest::Discover { key } can then request that exact plan on demand.
2. Keep the existing RunMachine, ProceduralRun and PreparedCompileInputs alive
   across a host `poll_with_runtime_macros` loop. On the matching Discover request,
   run the existing compiler.discover_diagnostic_with/discover_with coordinator
   outside the VM callback and pure queries, publish its actual result, and
   repoll the same RunMachine. These service loops already own real module,
   include, declaration expansion and procedural execution coordination. Do not
   create a second expansion/capture service or re-evaluate arguments.
3. Carry the new full-catalog Arc/input dependency separately to the existing
   source factory. RunMachine::start_with_discovery currently consumes the Value
   discovery into RunNames::Completed and real runtime macro/source records;
   that Value seal is not an ImplCatalog certificate and must not be relabeled.
   ReachedCallInputs/PendingCall must retain their exact active attempt, selected
   callable, body, argument slots/cells/value/type snapshots and guards during
   the suspension. Retrying metadata preparation resumes that same pending call.
4. Record capture results after the actual terminal poll, while preserving
   suspension observations. A missing supported external input can suspend;
   unavailable semantic producer metadata remains typed pending/unsupported and
   must not be fabricated by the coordinator.

There are two specific interface prerequisites. First, current full-catalog
construction computes key.unit_graph from the actual source dependency closure,
whereas ProceduralRun uses the actual ToolManifest graph. SourceDiscovery's
prepare_declarative/with_procedural_catalog correctly require catalog.graph to
match key.unit_graph (also actual consumer/epoch/environment). Blindly attaching
that existing ProceduralRun to current for_impl_catalog fails this check. The
catalog plan needs an authenticated macro graph/profile adapter retaining both
actual source-closure identity and actual manifest graph; weakening equality or
relabeling a procedural record would be unsound. Second,
SourceCompilerPreparation::NeedModule carries the actual ModulePreparationDemand
but the current catalog test rejects it. A production coordinator must retain
this preparation state, join the demand to its actual module declaration, use
existing module capture/stage/complete, and resume configuration before polling
catalog discovery. No guessed module or synthetic completion receipt suffices.

Authority/currentness inspection: Database::require_coordinator disallows pure
query reentry; require_source_preparation additionally disallows opening source
preparation during an active guest run. Existing PreparedCompileInputs opens and
retains SourceCapture before the run. Its discover_with loop uses that retained
service, and publish_discovery is a coordinator-only derived publication which
advances ordinary revision without replacing source_revision. Thus the existing
source-callable captured source_revision can survive actual derived publication;
whole publish_inputs would instead replace the universe, advance source_revision
and is prohibited during the run. Do not finish/begin a guest run merely to evade
these guards. Any genuinely new capture initialization needs an explicit
suspended-run coordinator boundary, not query IO or a global guard relaxation.

Cold cost is preparation over the actually requested catalog's declaration and
macro-producer closure; no executable bodies/cold branches are lowered by this
join. Hot dispatch keeps retained handles and version/attempt guards, with no
full-world comparison, scanning or repeated source planning. No obsolete
production route can yet be deleted: the once-only CLI unsupported continuation
is the exact route to replace when the common loop is implemented. All eight
architecture failure modes were inspected: this proposal adds no evaluator,
macro-specific numeric/member logic, pattern engine, generated exclusion,
per-operation preparation/global scan or immutable-body copy. Actual authority,
exact-once argument effects, suspension/resume and source invalidation witnesses
remain unexecuted requirements; no Cargo/runtime commands were run in this audit.

### V527 zero covered buckets: additional actual head holds (read only)

Parent reports materialization/publication returned and complete Q65 candidate
count matched, but covered_buckets was zero. source_factory's complete-bucket
publication requires every actual member's encoded impl_trait_ref Known. One
unavailable head therefore truthfully keeps the whole bucket unavailable.

The parent-identified explicit-reference preflight refusal is corroborated:
frontend/source_types.rs::symbolic_type_pending rejects every RefType and every
Lifetime outside LifetimeArg before the actual declaration walker. Real
core/cmp.rs reference heads at 210/221 and array.rs reference cross-comparisons
use declared explicit lifetimes. WrittenTypeDeclaration::region resolves these
by actual declaration ParamKey, SourceGenericLayouts::early_region authenticates
kind/layout/name, and SourceTerms::region encodes ReEarlyParam. Unknown names
remain unresolved; `'_` becomes Local and stays RegionMetadata. Generic array N
already follows the actual const parameter path and SourceTerms::constant accepts
ConstTerm::Param with the genuine const layout, including in argument position.
This does not prove every individual array head ready.

An independent definite blocker is the actual facade/core/ptr.rs:162 declaration
`impl crate::cmp::PartialEq for fn()`. SourceTerms::Encoder::ty unconditionally
returns FunctionMetadata for TyTerm::Function. The actual source walker does
produce that term; its current FunctionSignature contains only parameters/result
and its fn-pointer walker explicitly erases ABI/safety, including through aliases.
The existing check_erased_function_pending control intentionally requires this
hold. Consequently fixing references alone cannot certify a full bucket that
contains this implementation. Required repair is genuine qualifier/binder-aware
source function metadata across the common declaration/alias path, then exact
pinned FnPtr encoding; globally guessing safe Rust ABI from Function is invalid.
No candidate omission, signature-name whitelist or partial bucket is acceptable.

If the actual profile includes alloc/string.rs:203, its actual
`impl PartialEq<String> for &str` also has an omitted lifetime and will correctly
remain held under an explicit-lifetime-only repair. Membership must be read from
the actual full catalog and observed diagnostics, never narrowed for this witness.
These are source-confirmed potential/definite encoding holds, not terminal test
observations or a claim that no other head is unavailable. No production source
changed during the validator's closing-capsule ownership.
