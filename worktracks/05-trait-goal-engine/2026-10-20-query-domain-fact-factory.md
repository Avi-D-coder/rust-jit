# M020 query-domain fact factory migration outline

LOCAL-049 / WT05-02 / M020; read-only production review during V513 freeze.
Baseline HEAD `3d481c9d4eaca5326b23b9a30c1c59b7f7afcd0a` plus the frozen
candidate below. This document is a proposed implementation sequence, not an
implemented factory, root admission, test result or gate change. Worker owns only
this new document; no production, tool, test, fixture or Cargo edits were made.
Pinned compiler revision is ae0fe511b15ab760d7e963b13389d1bc2ab42b18.

## Concrete problem and witness

The unchanged tests/ui/enum-discriminant/repr128.rs calls discr with an actual
Signed/Unsigned enum and an unsuffixed integer. The ordinary reached call must
transport the declared projected PartialEq<U> requirement to upstream inference
before CallSeal's literal fallback fixes U. A source-aware component control has
actual canonical DiscriminantKind/Discriminant/PartialEq identities and a real
integer variable owned by SolverContext. That input transport is not root solving
or runtime acceptance. Q85/Q86/Q87 add genuine query components; none creates a
complete compiler fact factory.

prepared.rs::DefinitionFacts requires generic, variance, type, signature, opaque,
constant, coroutine, parent, ten predicate-family and scalar fields together.
PredicateFacts conflates simultaneous availability with membership in one struct:
explicit_item, item, item_self, item_non_self, all, own, explicit_super,
explicit_implied, const_conditions and implied_const_bounds are mandatory.
ImplFacts similarly requires trait ref, polarity, const/default/reflection/unsized,
super_outlives and specialization. TraitFacts requires flags, associated members,
supers and complete candidate membership together. Real unavailable query data
cannot lawfully fill those fields with false, None or empty values.

PreparedProvider::publish currently also checks, for each trait impl, every entry
in trait_data.associated_items has an associated_leaves row. A proof of an integer
PartialEq trait obligation need not select its eq/ne bodies or their associated
leaf results. The preflight Cartesian requirement is broader than that root's
semantic demand. It remains a safety hold until replacement query admission is
sound; deleting that loop alone would turn missing callbacks into aborts.

ReadyProvider currently contains only PreparedProvider and Cx. ready(cx) checks
publication and builds the pinned constructor/blanket index; evaluate_prepared
then accepts any goal/span plus any SolverContext having that same Cx/provider.
It does not establish the actual goal's query closure, caller ParamEnv or inference
owner. After moving to partial query tables, that API cannot authorize an
arbitrary different goal using the same token.

## Keep identity and completeness separate from query availability

Retain the existing prepared/provider world and arena model. This change needs no
new namespace, parser, solver, candidate matcher or query framework. Source
preparation stays in existing memo providers, with retained Arc roots and guards.
The root assembler is an ordinary ahead-of-entry consumer of those plans.

Suggested default-free data partition in prepared.rs (names illustrative):

* DeclarationMetadata: authentic DefinitionReceipt, actual name/span and real
  role/parent classification. Store a compiler parent answer separately when
  required; source lexical scope is not a compiler module DefId.
* PreparedGenerics: actual DeclarationParameters/DeclarationGenericTypes and
  context-owned Generics, retaining exact identity/order/parent count/const Types.
* DefinitionQueryTables: independent typed maps for declared Type, hidden Type,
  fn signature, opaque identities, coroutine queries, scalar query answers,
  variance query domains and alias argument layouts.
* PredicateQueryTables: separate typed maps for each SourceFacts predicate
  callback. Full clauses_of, own_clauses_of, explicit item, item/self/nonself,
  explicit super/implied, const conditions and implied const bounds never share
  an availability bit or implicitly share an empty vector.
* ImplQueryTables: actual head, polarity/default/conditional constness and
  independent reflection/guaranteed-unsized/specialization/super-outlives answers.
* TraitQueryTables: each Q85 flag independently, complete candidate membership,
  complete associated-type/member enumeration and supertrait queries separately.
* Existing ADT/source shape, parameter, language role and terminal const/transmute
  tables remain distinct domains rather than collateral defaults in a definition.

Use typed query keys: (DefinitionId, scalar query enum), (DefinitionId, predicate
family enum), actual AliasTermKind, and exact (impl, trait member) leaf key as
appropriate. Mixed value maps must have type-specific accessors or variants so a
source clause list cannot accidentally satisfy an implied-clause request. Never
key a projection by a name hash; retain Q77/Q87's real associated declaration ID.

At source preparation, Known(T), NotApplicable and Pending(typed dependency) stay
distinct. Known(None), Known(false) and Known([]) are real semantic answers only
when the source producer proves them. NotApplicable preserves the pinned invalid
query contract; it is neither false nor permission for an actual callback to run.
An unpublished table key is a preparation dependency, never NoSolution, ambiguity,
an error token, no candidates or successful absence. Published answers retain the
query-specific source proof/Arc roots and genuine owned value, not just a boolean
that a caller labels complete. No Default implementation is needed.

## Checks from publish/ready that must survive the split

1. Every published ID belongs to the exact source world; a real definition
   receipt matches its source/header/context and admitted original/generated
   attachment. Query references require authentic identities, not complete rows
   for every unrelated query of the referenced definition.
2. require_generic_value, receipt.matches_generics and actual source parameter
   count/order/parent/const Type validation remain. APIT or unexpanded parameter
   metadata cannot become a zero-generic proof. Q87 alias layouts must retain the
   real kind/Self/impl namespace and Q77 parent join, not an ordinary-layout guess.
3. ImplTraitCatalog.matches_catalog and require_translation retain the complete
   source membership boundary for each demanded trait. Every raw candidate maps
   to its authentic canonical owner/head and belongs to the actual trait. No
   caller-selected candidate subset becomes a complete bucket.
4. The existing pinned simplify_type(TreatParams::InstantiateWithInfer) index
   builder and constructor/blanket comparison remain once per frozen table set.
   candidates.rs remains the relevant-impl traversal, including upstream integer
   variable handling. Do not create primitive lists or a source-level applicability
   classifier to reduce closure work. A later incremental index design needs its
   own source completeness proof; it is unnecessary for this first migration.
5. ADT PreparedAdtVariants/source shape and enum discriminant receipts remain
   attached to any demanded ADT field/layout/discriminant answers. A missing field
   plan cannot stand for an empty struct/union/enum. Variance queries preserve
   their actual role/count domain and exact values.
6. Source language-role identity, complete requested registry membership/absence,
   owning crate configuration and actual feature options remain separate proofs.
   Sparse role maps cannot prove role absence by get()==None.
7. Present associated leaves retain the current source ancestry/container/name,
   real item identity, specialization/finalizer validation. The Cartesian
   existence loop is removed only after exact requested-leaf availability is
   incorporated into root closure admission; it is not replaced with guessed
   inherited defaults or None. Complete associated enumeration, when queried,
   remains a separately certified list, not a Q77 type-only prefix.
8. PreparedInputGuard/current source/catalog/ExpansionEpoch checks remain at
   preparation completion and immediately before guest-visible commit. Unrelated
   namespace publication must not force a hot census; retain the query dependency
   fingerprints and real positive/missing lookup dependencies already in use.

## Admission must own the actual root input

Replace evaluate_prepared(ReadyProvider, arbitrary delegate, arbitrary goal, span)
with a private prepared-root boundary owning or borrowing exactly one actual
SolverContext, encoded Goal (including caller ParamEnv), span, inference input
map and frozen query tables. RootVariables already checks exact owner pointer;
same Cx alone does not establish inference ownership. Its Integer/Float/Type
distinctions and source-key mapping must survive this boundary.

An illustrative PreparedRoot is not a certificate constructor available to a
caller: only the actual assembler/admission check may produce it, after all
required source proofs and callback closure checks. Entry consumes that exact
root and accepts no replacement goal/delegate. The session must prevent inference
mutation between checking and entry (private ownership of the inference context
and input map is preferable to a fingerprint over mutable variables). If mutable
state is exposed, a genuine inference input revision check is additionally
required; comparing goal interned identity alone is insufficient. Include typing
mode and response/opaque/region policy in the session identity.

The caller environment is main's actual assumptions after real call substitution,
not discr's own PartialEq<U> requirement copied in as an assumption. For the
repr128 call, instantiate actual T with the actual reached enum, preserve U as
the same owned integer variable, and encode the requirement via the existing
source_terms::encode_root_predicate. Otherwise an assumed obligation could appear
to prove itself without reaching its source impl or constraining U.

With the unchanged panic=abort host policy, missing callbacks cannot be recovered
through catch_unwind or an exploratory upstream run. Preflight may return typed
Pending before entry; callbacks read frozen answers only. Unexpected invalid
callback/domain use remains an invariant fault, never a semantic answer. A query
table and demand descriptor alone are not closure certification. Do not authorize
a root by running a side solver or guessing which candidate the real solver will
select. Do not silently change host unwind policy to discover missing inputs.

The closure proof must conservatively cover actual upstream branches from this
root and caller environment, including every potential candidate, implied bound,
projection, const/alias branch and child obligation it can reach. Mechanical
upstream call/dependency policy may describe those edges; source plans supply the
actual complete values. If branch applicability or transitive keys cannot be
established without missing data, admission stays Pending. Descriptors enumerate
work, not proof of its completion. It is acceptable to overprepare immutable
metadata for the demanded complete trait bucket; it is not acceptable to check
callee bodies/cold arms or invent applicability to avoid missing query domains.

## Concrete Q85/Q86/Q87 assembly and next slice

Q85 SourceTraitSemantics supplies actual auto/marker/coinductive/alias/fundamental/
unsafe/deny-explicit/default/sizedness flags with independent configuration and
language identity dependencies. Dyn compatibility remains pending unless an
actual decisive incompatibility attribute proves false. Do not synthesize true
dyn compatibility for an ordinary trait because no body was prepared.

Q86 SourceImplSemantics supplies authentic raw/canonical impl membership,
modifiers, compiler conditional constness, retained source typed head and generic
layouts. Its inferred_outlives_of(Impl) is genuinely [] by pinned outlives/mod.rs.
Its complete_clauses_of null branch is also a genuine independent query answer:
real Impl, actual parameter count zero, no written generics/where predicates,
nondefault, no clause-producing/unproduced transform attributes, actual owning
crate configuration with generic_const_exprs off. EmptyImplClauses proves parent
None/clauses []; it supplies no other predicate family. Non-null branches remain
specific pending dependencies.

impl_super_outlives is NOT inferred_outlives_of and is not proven by those empty
clauses. Pinned collect/item_bounds.rs530 maps the actual impl trait reference
through rustc_type_ir::elaborate, then retains TypeOutlives/RegionOutlives. Q86's
descriptor reports actual impl/trait and direct callback families: Sized language
identity, explicit implied clauses, explicit implied const bounds and alias
variances. The complete transitive values and admission proof are absent; Q86
correctly returns ElaborationClosure pending. No algorithm entry/certificate was
added. outlives.rs::declared_bounds_from_definition has an item_self_bounds read,
but it is not directly called by this particular push_outlives_components path;
other actual root branches may independently demand item_self_bounds.

Q87 SourceAliasLayouts supplies actual free/projection/inherent Type/Const
argument layout domains and uses real associated identity where supported.
Projection const member identity is still pending. It does not produce projection
values, item bounds, complete associated members or specialization leaves.

The smallest implementable factory patch is therefore the representation/admission
split for source-authenticated metadata and these independent query answers:

1. In prepared.rs, add typed per-query tables/readers and transfer the preserved
   identity/generic/catalog/index validators to those domains. Start with existing
   actual Q75/Q79/Q83/Q85/Q86/Q87 source plan roots, not a fixture-specific map.
   Keep the old complete-factory path held until a real consumer exists; no dummy
   evaluate/WF goal should be added to exercise the new tables.
2. Add a root assembler for the actual reached call input, using Q80's actual
   encoded projected predicate, real substitution and RootVariables owner. Have
   it return the precise missing query dependency set before upstream entry.
   Preserve the old entry hold when no sound callback closure is available.
3. Implement the common full explicit-implied source query producer needed by
   impl_super_outlives and its true transitive closure. Reuse pinned collect
   predicates/bounds filtering and existing Q78/Q80/Q82 source collectors; do not
   replace full implied answers with written supertrait text or Q80 own clauses.
   Preserve associated-type/GAT mapping and missing language/default facts.
4. Once that query closure is actually available, call the retained upstream
   elaborator/filter algorithm on the authentic encoded impl head in a private
   admitted preparation context, producing the genuine super-outlives list.
   Its no-callback-gap proof must be real before exposing that algorithm method.
5. Cover actual builtin DiscriminantKind normalization/enum discriminant source
   facts and remaining actual candidate query branches in the same root closure;
   that is another owned prerequisite, not something Q86's null clauses answer
   implies. If any domain is still missing, report pending rather than entering.
6. Replace the arbitrary-goal ReadyProvider entry only when the genuine root
   admission owns the exact goal/ParamEnv/inference session. After entry, apply
   upstream's response to that same owner, export only representable resolved
   inputs, validate guards and atomically install U before literal fallback.
   Keep unresolved/ambiguous/NoSolution distinct. Then retire superseded eager row
   and Cartesian-leaf requirements for migrated domains, with their real negative
   availability/currentness controls.

Pinned trait_goals.rs::consider_impl_candidate60–126 confirms candidate head,
DeepReject/default/polarity, fresh args, equality, clauses_of and impl_super_outlives
before evaluating added obligations. Reflection metadata is conditional on real
typing mode; eq/ne signatures/bodies and all method leaves are not unconditional
requirements of an ordinary PartialEq trait proof. The real conservative closure
must still include other candidate assembly/merging, caller assumption and alias
normalization branches; this one function is not the whole root closure proof.

## Acceptance and eight-mode source review of this plan

Unchanged repr128 is the runtime witness; existing source projection controls are
inputs, not substitute root acceptance. Required implementation controls include
actual original/user/wrapper and all four editions, same-owner integer resolution,
different goal/ParamEnv/owner refusal, missing demanded query pending before entry,
unrelated query absence tolerated, complete candidate membership, relevant
addition/withdrawal and epoch revocation, exact requested leaf missing, no body
prep for proof-only metadata, and no guest mutation until successful response
commit. Existing runtime/full-fixture/diagnostic/tooling reports remain separate.
No new measured result, runtime gain or release/JIT gate follows from this plan.

Eight-mode review: (1–2) no expression/assertion/generated evaluator proposed;
(3) no primitive/member shortcut or independent inference/candidate matcher;
(4) existing actual generated identity/parent/hygiene receipts retained;
(5) no pattern split, body staticchecking or cold executable demand;
(6) source memo Arc plans are retained and reused, installed execution does no
planning; (7) relevant source dependencies and once-per-table index validation,
not dispatch census; (8) no BodyIndex clone or whole-file node search. Cold
preparation may traverse the demanded complete trait bucket and dependency graph;
cost/scaling remains unmeasured. No resource accounting, caps or new unsafe code
is proposed. All implementation and runtime validation in this outline is held.

Frozen inspected source SHA256 (documentation does not alter V513 source):

| Path under crates/lrr/src/traits/upstream | SHA256 |
| --- | --- |
| prepared.rs | 90039056bb1ef45203933fe5bdb10bb4a66bc4b163d778f0dbbfe38c0a2ff8d2 |
| facts.rs | 763c0316f0b00f95050d6c86db18da016aea44fef46801839b7093d733e1ddc1 |
| source_trait_semantics.rs | f7df4dcc558851d60bd167ae81d4ebe2cab51202373cd7cad3a6e78d0631210f |
| source_impl_semantics.rs | 7ffed5357c46800dcb7362755f8c019ec7fe3e13ed1df4d24e0bbd0ac06e416a |
| source_alias_layouts.rs | 9ad1b4094a96ba594eb81d9d569cf75129d3d0965ee1719fd34617f8f046d205 |

## V515-frozen read-only actual reached-call join

No production/test/import/build changes were made for this follow-on analysis.
The factory/readiness code remains the same inspected version above; Q88 now
exists as an additional genuine immediate source producer. The smallest common
runtime join is still ahead of the ordinary reached CallSeal fallback, not an
operator-name/repr128/assertion-specific rule.

engine/run.rs's CallSeal branch currently loops over reached argument cells,
calls GuestStore::bind_literal_fallback, obtains effective_type, then constructs
ArgumentType {source,descriptor} and prepare_source_call contracts. The public
frontend/call_contracts.rs ArgumentType already requires a concrete TypeId, so it
cannot represent an open integer at that point. engine/run_trait_calls.rs::
selection_argument_typed already demonstrates the actual data distinction:
concrete source Type versus Literal {integer,behind_reference} versus Unknown.
Reuse that authentic reached-cell/literal-family observation as input data;
do not reuse select_by_arguments' custom candidate/fallback heuristics as the
new solver. Some deferred trait Self selection also falls back early and is
the same migration boundary, not permission for a second call engine.

The required common input packet must retain selected SourceCallable/current
signature identity, actual call identity/arguments, known source/runtime bindings,
open numeric keys/families and the caller's actual instantiated environment.
Known T comes from the reached enum descriptor/source view; open U must be the
same genuine integer variable at each source occurrence. A callee's own declared
PartialEq<U> requirement is an obligation to discharge, not an assumption to add
to its caller ParamEnv. Existing SignatureSubstitution/PreparedCallContracts
source_substitution are structural transport/runtime bindings; they are not an
upstream ParamEnv or a proof of trait obligations. Preserve their common boundary,
but do not promote their Unifier/member bool paths into another trait solver.

Available preparation joins, without pretending they are complete facts:

| Existing source plan | Actual use before entry | Still missing |
| --- | --- | --- |
| Q75 header + DeclarationParameters/GenericTypes | Actual callee/trait/impl IDs, spans, role, source order/Self and generics transport | Runtime owner/session assembler and per-query typed publication |
| Q76/Q77/Q87 | Real DiscriminantKind/Discriminant role/member/parent IDs and alias namespaces | Requested role availability and projection value query closure; no name-hash fallback |
| Q78/Q80/Q81 | Actual declared obligation recipes/shared encoding and separately demanded parent requirements | Full own/all/inferred compiler query families where required; actual caller environment construction |
| Q79 | Actual applicable variance values/domain and parameter-count proof | No license to fill unrelated queries/opaque domains |
| Q82/Q84 | Actual output/default rows, language/configuration/site proof | Associated mapping/elaboration/completeness outside supported immediate producer domains |
| Q83/Q85 | Real scalar/trait flags independently known or pending | No whole-definition/TraitFacts readiness or dyn/opaque guessed defaults |
| Q86 | Authentic impl typed head/generics/polarity/default/conditional constness; genuine null clauses branch and inferred_outlives [] | True impl_super_outlives and non-null compiler clause branches remain pending |
| Q88 | Immediate Super/Implied selected clauses, exact ordering/spans and positive/pending dependency descriptors, shared scoped encode | Actual transitive query dependency preparation/callback closure; not a recursive/cycle certificate |

Existing enum_repr::PreparedEnumDiscriminant, SourceAdtFields and source_variants
proofs must populate the demanded real ADT/discriminant domain for builtin
DiscriminantKind normalization. prepared.rs::discriminant_type already delegates
to pinned_discriminant_ty; extending the actual source/fact join is appropriate,
not replacing it with an enum-size/primitive shortcut. The facade's omitted
PartialEq Rhs=Self head is already completed by the common source Type route;
adding another generic-default evaluator is not the missing join.

Concrete next implementation sequence: first publish genuine independent
metadata/query domains and actual complete source candidate bucket/index data;
next use Q88's positive dependency rows to prepare their actual requested source
query inputs with the existing dependency framework. Pending rows remain
admission dependencies. A visited data-dependency set may prevent repeated
preparation; it must not decide solver cycles, applicability or normalization.
Only after that actual query closure exists can the retained pinned elaborate/
filter algorithm produce Q86 super_outlives. Do not equate that query with
inferred_outlives [] or Q86's complete null clauses result. Q88 alone therefore
does not unblock PreparedProvider::publish/ready or the actual root entry.

The actual runtime session must privately own the exact SolverContext,
RootVariables map, encoded Goal/caller ParamEnv and source/currentness inputs.
ready currently proves provider/Cx/catalog/index agreement only, and
evaluate_prepared still accepts an arbitrary goal/delegate sharing that Cx.
Consume one genuine prepared-root session, without externally replaceable goal
or inference owner, after actual required callback closure validation. Same Cx
or a goal hash is insufficient when inference state can change. Missing closure
must return typed pending before entry under the unchanged abort policy; no
panic/catch/replay or synthetic assumed goal establishes readiness.

After real upstream evaluation, RootVariables::resolved reads the original
variables in that same owner; GoalEvaluation.goal is not a substitution answer.
Only representable resolved inputs may be exported to owned source terms/runtime
descriptors, with current dependency guards rechecked before an atomic ordinary
call/literal-binding commit. PreparedCallContracts/inferred_equations and existing
frame environment capture are natural retention/guard points. No caller cell,
frame hint or guest store mutation belongs inside a pending solver attempt.
Only then may ordinary fallback apply to variables the real solver leaves open.

Legacy numeric/member shortcuts in binding_constraint_generics.rs (including its
NATIVE_PRIMITIVE_TRAITS fallback), select_by_arguments/MemberIndex applicability
and source SignatureSubstitution Unifier remain named migration debt. They are
not source-fact certificates and should be removed for a migrated demand only
when its real common upstream route/response/invalidation evidence exists.
No new hot source scan, body checker or context-only semantic route is needed.

Inspected frozen source hashes for these new caller observations:

| Path under crates/lrr/src | SHA256 |
| --- | --- |
| engine/run.rs | a87de19e1e6098c53d38e8a5ce3acf04b729bee3c04cf17af0e0f8de6f107857 |
| engine/run_trait_calls.rs | 89770b4bd9c9e56714a45a5addc95a06c91b7f28af48c6300c968b5426dd7735 |
| frontend/call_contracts.rs | 44f6b4b4c7f957266b2043d90ae3d0f76d50032d23f52e082d722424a806345f |
| traits/upstream/root_variables.rs | ee3e3b731a8129dd599cacba037cd122e26a39da0a241221e91f089499f3a72a |
| traits/upstream/source_trait_clauses.rs | 2abfb8c0e90c85f934c0b1ca51dc0d4647fa1028c472436c6f7c5590ecc1f0d7 |

This is a read-only bounded implementation plan. V515 observations belong to the
validator; no success, source-root execution, runtime gain or gate is inferred.

## Queued bounded pre-fallback packet slice (plan only)

LOCAL-049 / WT05-02 / M020. Current freeze permits reading/reasoning/docs only.
Implementation ownership proposed after explicit release/interface agreement:
engine/run.rs plus a new private child module engine/run_call_inputs.rs. Lead
owns shared upstream root input/admission types and prepared.rs/query tables.
No standalone replacement language tests, fixture edits, source code, build or
test run is authorized by this plan. Primary runtime witness remains unchanged
repr128, with ordinary/user/wrapper routes sharing the same call boundary.

Minimal invariant/witness: at the selected ordinary source CallSeal, the reached
enum argument determines the actual T shape while the unsuffixed argument remains
an actual integer-family variable. Capture must preserve those inputs before
fallback mutates the literal binding; it must not select an i128 overload,
coerce by fixture name or assume the callee's PartialEq obligation in ParamEnv.
Capture is not solving and cannot claim to fix the existing runtime fault.

Existing route: selected SourceCallable + current explicit/owner-application
guards -> ordinary CallStart/CallPrepare argument execution -> CallSeal ->
concrete ArgumentType/prepare_source_call -> PreparedCallContracts -> GenericEnv
capture/ordinary argument packet delivery and callee continuation/cleanup.
Generated syntax reaches that same selected callable/call seal with its actual
source origin and runtime-call ordinal. The packet extends that route before
the concrete-only ArgumentType loop; it does not create a macro-specific caller.

Proposed engine-private transport API:

* ReachedCallInputs::capture (implemented as a RunMachine helper so it can reuse
  existing selected-call/cell/source observations) returns an immutable owned
  packet or explicit representability/currentness hold. Unsupported input is
  not a semantic negative, guessed Type variable or proof of no obligations.
* Retain the actual PcKey, CallIdentity including runtime-call ordinal, selected
  Arc<SourceCallable>/signature fingerprint, argument slot/cell and validated
  observed value snapshots, explicit argument proofs/owner-application guard,
  result-context receipt, actual source site and current source-world handles.
  Retain Arc handles; do not clone BodyIndex or reconstruct argument syntax.
* Arguments retain either actual source Type + TypeId or open numeric literal
  evidence. One source InferVarKey corresponds to each actual validated shared
  literal record identity, not spelling/slot/argument ordinal. Repeated views of
  that record share one Integer/Float variable. Unknown nonliteral shapes remain
  representability holds; they do not become arbitrary Type variables.
* Generic input tuples are actual callee ParamKey -> TyTerm (including the
  original source InferVarKey), with a separate exact variable-family map.
  Known explicit/already-proven bindings retain authentic signature mapping.
  Unbound declared type parameters remain Type-family keys. The shared upstream
  session allocates its actual variables and relates them with reached arguments.
* Retain raw formal/reached source relation tuples and actual source site/kind.
  Actual argument positions are coercion sites (including existing reference,
  pointer and unsizing proofs); explicit generic bindings are equality inputs.
  Preserve result expectation direction from the existing source contract.
  Do not impose Eq at a coercion site, invoke SignatureSubstitution::Unifier,
  normalize projections or pick a composite binding in the packet. Repeated or
  contradictory relations remain relations for the SAME upstream owner.
* The caller descriptor retains current BodyKey/definition independently of
  optional Frame::shared_generic_context() (Arc<GenericEnv>, real owner,
  signature/application identity and its actual substitutions). Closure lexical
  owner can differ from current body owner; retain both rather than flattening.
  None is not an empty ParamEnv certificate. Even nongeneric functions may have
  concrete where clauses. Lead preparation joins actual caller own/parent clause
  receipts before constructing ParamEnv; callee clauses remain obligations.
* require_current/revalidate_before_commit checks actual PC/call/callee identity,
  relevant source guards, slots/cells/value snapshots and literal binding/version
  state. Reuse the active frame/call lifecycle; no arbitrary global owner token
  or source-world scan is needed. Packets never escape into facts with CellId,
  Frame or ValueRepr types: only authenticated source tuples/families/caller
  descriptor and source sites cross the shared upstream input boundary.

The existing selection_argument_typed observation already distinguishes concrete
Types from unbound numeric families and admit_move is read-only admission here.
Reuse/factor its actual observations where ownership permits, rather than create
a second value classifier. The packet must additionally retain the validated
shared literal identity and cell snapshot for deduplication and later commit.
Existing source/input guards are checked after all argument effects and again
after any preparation suspension. Literal binding, caller hints, tickets and
guest cell ownership are unchanged during capture/preparation.

Source spans come from retained indexed call/operand data and genuine generated
call source identity. A missing precise source join is explicit pending;
never reparse display text, search the whole file per argument or label an outer
macro span as an authenticated inner call. Source-owned call operands and actual
runtime-call ordinal remain part of the common owner key.

Integration after release: capture once at the reached selected source call
before the current CallSeal fallback loop. Retain it across an actual preparation
NeedInput with the existing call/frame lifecycle, invalidating/refreshing only
when a real dependency/input changes. Initially the existing fallback behavior
stays until the actual root session can consume/resolve the packet; capture
alone is an explicitly held adapter component, not acceptance or a repaired
repr128 result. Unsupported capture metadata must not newly refuse previously
supported calls while this transport is held. A later successful upstream
response is committed through the ordinary existing contracts/literal binding
path after guard validation; no speculative guest mutation occurs beforehand.

Missing producer dependencies are named: actual caller ParamEnv/clauses (Q80/Q81
written lists alone are not full compiler queries; Q86 null proof is Impl only),
true Q86 super-outlives/Q88 transitive closure, builtin projection/ADT source facts,
and the lead's query-domain factory/owner-bound root session. Capture does not
remove these holds or invent empty data. The exact callable assumption and
response transport use the existing source receipts and parent-owned shared
input types; no engine-private frames/values enter compiler callbacks.

All-eight review obligations for the eventual patch: (1–2) only existing ordinary
call input transport, no assertion/generated evaluator; (3) no method/numeric
choice outside genuine upstream inference, no fallback coercion; (4) preserve
original/generated/wrapper source/hygiene/edition/ordinal joins; (5) no body or
cold arm checks/pattern machinery; (6) capture once, memo source preparation,
no installed planning; (7) only relevant input guards, no global hot census;
(8) Arc-retain immutable bodies/source, direct indexes rather than whole-file
searches. Cold capture costs O(reached argument count + declared generic inputs
+ actual source term sizes), plus existing demanded source preparation; no timing
or scaling result is claimed. No resource accounting or new unsafe code.

Evidence required after implementation: actual unchanged repr128 input capture,
same literal identity/family across repeated occurrences and suspension, exact
caller versus callee assumption separation, actual owner/currentness checks and
ordinary/user/wrapper source joins. Root/runtime gain requires real solving and
guarded response commit; source packet observation alone is insufficient. No
new standalone controls may substitute for the actual runtime witness.

Implementation release: lead explicitly released engine/run.rs and new private
engine/run_call_inputs.rs after pushed da1cdc0c3fa3aa4872ba4e4a7afaabe8fd974d42.
The first adapter captures only upon an actual consumer request; the legacy
CallSeal path must allocate no unused packet and retain its behavior. The parent
owns shared root DTO/factory integration. Initial callee Type parameters retain
packet Type-family keys; raw argument coercion relations connect them to concrete
source terms or actual shared-literal Integer/Float keys in upstream inference.
Only actual explicit argument mappings become known source substitutions here.
No packet Unifier, projection/composite guess or caller-assumption fabrication.
There is no source/runtime acceptance claim before genuine consumption/evidence.

### Released packet implementation handoff

Owned production changes are engine/run_call_inputs.rs plus six lines in run.rs
(private module registration and the exact pre-fallback integration anchor).
The actual root consumer is still held/unwired. The legacy CallSeal execution
does not invoke capture and allocates no unused packet; fallback, source call
contracts and the repr128 outcome are not changed by this adapter alone.

Stable callable engine API:
`RunMachine::capture_reached_call_inputs(&mut self, db, site:NodeId,
arguments:&[SlotId], result_hint:Option<&ArgumentType>) ->
Result<ReachedCallInputs,Diagnostic>`. Its non-Clone engine-private packet exposes
pending(), source()->Result<SourceRootCallInput<'_>,ReachedCallInputPending>,
site()->&BodySpan, source_site()->Option<SourceCallSite>, and
require_current(&RunMachine,&mut Database). The actual consumer owns retention
across NeedInput under an authentic call-attempt/activation identity and must
call require_current before committing a response. The existing reusable PC,
CallIdentity and arena cell IDs do not prove that activation, even when bytes
and selected callable match. Actual attempt retention and result-expectation
currentness remain consumer obligations, so this is not a full commit certificate.
No engine-owned cell, slot, value or frame type appears in SourceRootCallInput.

The adapter consumes the lead's shared root_input DTOs directly:
SourceVariableFamily, SourceCallRelation/Kind, SourceCallSite,
CallerSourceContext and SourceRootCallInput. There is no parallel shared schema.
Callee Type parameters begin as actual source-keyed Type variables. Authenticated
explicit type arguments retain their actual mapping and Equal relations; unused
initial Type-family keys are removed. Argument/result relations are Coerce,
preserving direction (callee result toward actual expected result). No structural
Unifier, projection/composite inference or literal-fallback coercion is invoked.
Formal templates come from the selected actual signature; missing represented
templates remain ParameterSource/ResultSource holds. No complete anonymous/APIT,
const/region generic namespace is inferred from this written Type map; the lead's
actual generic query preparation remains authoritative.

Open numeric input uses the existing selection_argument observation. Before
deduplication, GuestStore::admit_move/validate_value and effective_type verify a
real live numeric LiteralRecord handle. The (kind,payload) key is that validated
record identity, not numeric value bits, spelling or slot. Two views of the same
record share one Integer/Float InferVarKey. Each packet records actual PC and
runtime call ordinal in its source variable scope; exact inference ownership is
still the lead session's job. Unknown nonliteral observations remain explicit
ArgumentSource holds rather than invented Type variables.

The packet captures current BodyKey separately from optional Arc<GenericEnv>
owner/signature/application and actual source substitution rows. It supplies no
caller ParamEnv, callee-bound assumption or absence proof for a missing environment.
Caller DefinitionSource/DefinitionBody attachments must authenticate the actual
BodyKey and retained BodySpan before conversion to an absolute SourceCallSite.
A real runtime source call also validates its existing call/argument/phase/source
identity. Missing or mismatched source joins remain CallerSource holds with no
fabricated span; the adapter does not newly refuse a legacy generated call.
No source/body scan or display-text reconstruction is used.

Snapshots retain actual slot/cell, CellState, validated ValueRepr and effective
Type (including literal binding). Revalidation checks active PC/CallIdentity,
selected callable Arc/signature and argument slots, typed explicit/qualifier
guards, immutable caller body/environment and source-attachment handles, and
all current cells/values/literal binding state. It reuses the existing callable
source-revision safety check; the initial independent packet revision snapshot
was removed during separate review. It adds no full-world comparison. This inherited
global revision invalidation remains broader than ideal relevant guards, not a
new performance/completeness claim. Guard checks allocate no argument vector.

All-eight implementation source review: (1–2) no evaluator/assertion/macro semantic
route, only existing call/literal observations; (3) raw relation data only, no
trait/member/primitive choice; (4) actual original/generated source attachment
and runtime ordinal retained, missing joins held; (5) no bodies/static checks/
patterns/cold arms prepared; (6) capture is demand-only and the consumer must
retain it across actual suspension, installed code does no preparation; (7)
direct relevant handles/slot snapshots, no namespace/body census; (8) Arc body/
callee/environment retention and direct BodySpan lookup, no immutable BodyIndex
clone or whole-file search. The source map conversion and snapshots are read-only;
no literal bindings, hints, moves, tickets or argument ownership are changed.
No resource accounting, host-policy change or unsafe code was introduced.

Current worker source fingerprints before parent integration/review:

| Path under crates/lrr/src/engine | SHA256 |
| --- | --- |
| run.rs | 3a21ecb4966f7f3e499aa64f019378d20d1cd7b0c08497a16b54305307e6cdab |
| run_call_inputs.rs | 7f4746815d24b0d822eebe632fd495d63347dc57d83fe32189f7f6aa538cd019 |

No Cargo/build/test/guest command was run. Actual capture, literal dedup across
suspension, generated/edition matrix, caller clause join, root factory/admission,
response commit and the unchanged repr128 runtime witness remain unexecuted/held.
The integration anchor is not an installed root consumer or a completed repair.

### Caller assumptions source preparation — bounded follow-on before code

M020 / LOCAL-049 / WT05-02 extends only new
`traits/upstream/source_caller_clauses.rs`. The unchanged repr128 call from
ordinary `main` needs a genuine caller environment before callee obligations
can enter upstream. Missing `GenericEnv` is not an empty-environment receipt.
The common route is actual Q75 declaration identity, Q81 retained Q80 own/parent
source hierarchy, Q83 conditional constness and owning solver configuration.
Original/generated declarations share those services; no alternate parser,
body walk, generated exclusion, callee-bound assumption or independent solver.

Pinned ae0fe511b15ab760d7e963b13389d1bc2ab42b18 `rustc_ty_utils/src/ty.rs::param_env`
instantiates complete clauses, adds default trait-method RPITIT assumptions and
conditional const effects, then normalizes. `collect/clauses_of.rs` gathers
written/implicit clauses, GCE and unstable-feature additions; `outlives/mod.rs`
returns no inferred outlives for ordinary Fn. The initial certified source
branch is only a genuine standalone, zero complete-parameter-count Fn with no
where predicates/transform or unstable-feature attributes, no GCE, no conditional
constness and an empty authenticated source hierarchy. No parent text, generic
application, closure/typeck child, inherited or nonempty explicit source bundle
is promoted to a complete ParamEnv. These remain typed pending, retained with
their source plans. The eventual caller application/root owner must authenticate
the packet's source descriptor, not treat this plan as root admission.

Cold cost is existing memo demands plus one retained direct declaration AST
lookup and header checks. Encoding reads retained immutable plans, does no DB
work and performs no solver callbacks for the proven empty branch. Relevant
query input guards protect currentness; no new global source revision gate or
namespace census. Existing legacy fallback remains named and held until an
actual consumer, owner/session closure and unchanged runtime evidence replace
it. All eight architecture modes require a separate source review; actual
ordinary/user/wrapper/edition execution and suspension remain unexecuted here.

Caller module source handoff: `SourceCallerClausesKey = DefinitionHeaderKey`;
`PreparedSourceCallerClauses::prepare(db,key)` retains individual underlying
memo input guards and catalog Arc, with `require_current(db)`. There is no new
query registration yet. `hierarchy()` retains actual own/parent source plans,
`encode_written` delegates to the existing Q81 encoder without a completeness
claim, and `encode(cx,configuration,&CallerSourceContext)` produces ParamEnv only
for the proven empty branch. All optional runtime generic/application descriptors
currently cause `CallerApplication`; their absence is accepted only after the
independent source proof. Nonempty generic callers, associated/default trait
methods, lexical local owners, foreign and typeck children, missing configuration
and nontrivial normalization/application remain typed holds. Source preparation
performs no signature/body lowering and reads only the retained declaration node.

Separate all-eight source pass against module SHA256
`8520b7333569a6e58f60239184400856604e42ba4e614dd7185073f5ad125afd`:
(1–2) `prepare` and `encode` transport ordinary source metadata, no assertion or
generated evaluator (MA001/MA004/MA005); (3) no member/numeric decisions or solver
(MA003/MA006); (4) Q75/Q81 share genuine original/generated joins, no origin test;
(5) no patterns, executable branches, synthetic signatures or body walk
(MA002); (6) retained memo roots and pure encoding, input guards rather than
per-phase reconstruction; (7) no namespace census or new revision-wide guard;
(8) Arc roots and direct `syntax_at(context.range)` access, no BodyIndex clone or
whole-file search (MA008). No macro identity dispatch is added (MA007). This is a
source review only, not evidence that these historical audit findings are closed.
No Cargo/tests or guest execution; null-branch source observation, matrix,
suspension and actual root/runtime acceptance are still unexecuted.

### Owned root-response transport — bounded follow-on before code

M020 / LOCAL-049 / WT05-02 owns only new `upstream/root_response.rs` in this
slice. `source_terms.rs::Encoder` supplies portable-to-IR construction, but no
production reverse converter exists. The shared next-solver owner applies its
real response; this adapter reads only `RootVariables::resolved` afterward, not
the original goal or a fabricated solver result. It consumes the owner-retained
`specifications()` map, authenticates the same SolverContext and translates
representable IR to existing TyTerm. It does no inference, normalization,
candidate selection, canonicalization or source preparation.

The unchanged repr128 witness needs genuine primitive nominal arguments and
integer variables. Source nominal/member identities must invert authenticated
SourceBindings/Q77/DeclarationGenericTypes receipts, including parent-first
projection argument split. Unresolved Type/Integer/Float families stay separate:
only an actual original source key with the matching upstream inference family
can name a representative. Unknown solver-created variables and unavailable
source/binder/region/constant metadata are typed pending, never fabricated
keys, numeric defaults, erased-to-static lifetimes or spelling hashes. Runtime
commit, caller admission and same-call activation remain independent holds.

Cold export is one traversal of demanded resolved values plus retained binding
maps; it runs only on an actual consumer response. No new per-op source parsing,
world census, deep BodyIndex clone, evaluator or macro-origin dispatch is added.
No legacy path is replaced in this slice, and no unused export is installed on
ordinary calls. Required closure is source review plus actual same-owner response
controls and unchanged runtime execution; all remain unexecuted before validation.

Implemented response API: `export_root_response(owner,&RootVariables,
&SourceGenericLayouts)->Result<OwnedRootResponse,RootResponsePending>`. It uses
the lead's owner-retained `specifications()` accessor; no caller-supplied family
map or public arbitrary-Ty conversion is accepted. Existing materialized
`SourceGenericLayouts::{type_bindings,argument_kinds,projection_argument_kinds,
projection_parent,parameter}` supply inverse identity and split metadata, so
export does not repeat DeclarationParameters/source preparation. Actual Q77
lookup reconfirms member declaration identity using its retained map only.
`OwnedRootResponse` owns values for original source keys and a separate map of
unresolved representative families, with source-world identity. It is not a
solver-success, root, caller, activation or commit certificate.

Representable forms are portable primitive/bool/char/unit, str/never, tuple,
slice, reference with genuine static/early-source region, existing raw-pointer
transport, nominal with validated argument namespaces, source parameters,
nonrigid projection with authenticated member and parent split, and the existing
u64 usize constant/array representation. F16/F128, fn/dyn/opaque, rigid aliases,
unscoped bound/placeholder types, erased/inference/late regions, nonrepresentable
constants and unknown solver variables remain typed pending. A Type variable
resolved to an open Integer variable exports through an actual original Integer
key; it is not renamed as a Type variable. Multiple unified keys use the first
matching-family actual key in deterministic source-key order. Numeric fallback
is never performed here.

Separate all-eight response source pass against SHA256
`d37d63dc68ddecc90a28267f3345c8755b265900ae47841498003e599c11c9b4`:
(1–2) data conversion only, no evaluator/assertion/generated executor
(MA001/MA004/MA005); (3) upstream owner alone resolves variables, no replacement
solver/member/numeric inference (MA003/MA006); (4) actual source IDs/retained Q77
join with no origin branch; (5) no body/pattern/cold-arm work (MA002); (6) existing
resolved reads and materialized source layouts, no source planning; (7) selected
maps only, no namespace/global-world census or new invalidation gate; (8) no
BodyIndex copy or source searches (MA008), no macro-identity dispatch (MA007).
Each projection currently scans only the retained demanded projection map to
find the unique inverse member; no scaling claim is made. Formatting/diff checks
only; no Cargo/tests or actual response/runtime execution in this slice.

### Frozen V520/V521 read-only caller application review

No production edits or executions. Inspected `vm/context.rs::GenericEnv`,
`frontend/signature_subst.rs::SignatureSubstitution`,
`frontend/call_contracts.rs::PreparedCallContracts`,
`vm/frame.rs::{install_context,shared_generic_context}`, Q81 and pinned
`generic_clauses.rs`/`rustc_ty_utils/src/ty.rs::param_env`. Existing receipts:

* `GenericEnv::capture_current` revalidates the actual selected SourceCallable
  and its specialized result/reference proofs, then owns the immutable actual
  FunctionSignature fingerprint plus source/runtime Type substitutions.
  `Frame::install_context` requires that signature definition to equal the
  entered BodyKey definition. `shared_generic_context` can instead return a
  lexical environment from a different creating definition; preserve the
  distinction already present in CallerSourceContext.
* `application_identity` hashes signature plus BOTH symbolic and run-local
  runtime descriptor rows. The DTO contains only symbolic rows, so its digest
  cannot be independently reconstructed/verified from that DTO. The packet's
  retained GenericEnv Arc and actual active owner must authenticate the runtime
  application; a digest string alone is not authority. `require_current` checks
  that environment's signature/specialized proofs, not compiler clause closure.
* `checked_bytes` validates each present Type binding's namespace and corresponding
  runtime row, but does not certify that all actual generic parameters are
  present. Runtime GenericEnv stores Type substitutions only, and permits no
  transaction Infer/placeholder terms. Missing region/const/Type bindings do not
  become identity, defaults or successful empty substitutions.
* `SignatureSubstitution` is legacy custom Unifier state. Its binding/constrain
  methods and region-erasure comparison view cannot certify upstream inference,
  clauses, region identity or normalization. Reuse its already-authenticated
  source data only; do not run it again to build a caller ParamEnv.

Smallest next common production step is a caller-application identity join at
the existing reached-call preparation boundary: source-authenticate
`generic_definition` and `signature` against the actual retained
FunctionSignature/Q75/DeclarationGenericTypes metadata; retain its relevant
input guards alongside the same actual GenericEnv Arc and active call owner.
For an ordinary nongeneric called function, prove the complete count is zero
and both actual substitution tables are empty; after that join, a Some GenericEnv
can reuse the existing genuine empty caller clause branch. This removes the
current blanket `CallerApplication` hold only for an independently proven case,
without requiring a new clause producer or solver. No signature or source work
belongs in installed execution. Lexical closure contexts stay held until the
actual typeck-root/creating-owner join exists; do not equate their body and
generic-definition digests.

For a nonempty generic caller, the next prerequisite remains complete clause
availability for each actual own/parent declaration. Q81 is explicitly written
source transport, not full compiler `clauses_of`: const-parameter
`ConstArgHasType`, GCE/unstable effects, inferred outlives, trait identity and
default-method RPITIT additions cannot be assumed supplied by its current recipe
enum. Once that real domain is admitted, encode actual arguments in the already
validated SourceGenericLayouts parent-first order, transport their genuine
Type/Region/Const namespaces, and reuse pinned
`GenericClauses::{instantiate,instantiate_own}`/EarlyBinder over the retained
parent closure. No new substitution/unification algorithm is needed. Then reuse
the real upstream ParamEnv normalization and conditional-const effect route,
with demanded fact closure prepared before entry. Callee clauses remain separate
obligations; they are never inserted as caller assumptions. Existing
`rebase_parent_parameter` and Q81's source-parent receipts supply identity
transport, not completion by themselves.

Response re-review binds `root_response.rs`
`d37d63dc68ddecc90a28267f3345c8755b265900ae47841498003e599c11c9b4`.
No missed family-collapse issue was found: Type-to-open-numeric resolution
requires an actual original matching-family numeric key, unified keys share a
deterministic representative, and unknown solver-created keys remain pending.
Projection export authenticates actual associated DefinitionId via retained Q77
and validates parent argument kinds/count; the existing portable ProjectionKey
still stores the legacy member-name digest. It must never be consumed as a DefId.
The eventual consumer retains/revalidates the actual layouts/Q77 receipts;
OwnedRootResponse's world field alone does not prove their currentness. Export
may observe an owner before solver success and is not a Certainty/response-
constraint/commit receipt. The independent validator's bounded source pass found
no mechanical blocker, but V520 failed elsewhere before export acceptance and
V521 was running frozen validation when this review was recorded. Caller module
now includes the lead's actual Cx configuration fingerprint check (SHA256
`587ddd7beb9da615a7cfb9abf4c04ac25d677e42ea810c4e236929b0c5451964`).
Activation identity, result-expectation currentness, actual response consumption,
all source-origin/edition matrices and unchanged repr128 runtime remain held.

### Next caller-application receipt design — source still frozen

Authorized next owned production file is `upstream/caller_application.rs` only;
no implementation has started during V521 freeze. Proposed preparation receives
the actual Arc<GenericEnv>, actual Arc<SourceCallable>, retained
Arc<PreparedSourceCallerClauses> and CallerSourceContext. It returns either a
private VerifiedCallerApplication or typed pending. The token retains those
exact handles plus an actual FunctionSignature query input guard. It compares
the full source signature with both environment and callable, Q75 actual
header/context/source identity, complete DeclarationParameters zero count and
genuine empty source/runtime substitution tables. DTO body/generic/signature/
application/substitution fields must describe those actual inputs exactly.
SourceCallable::revalidate, GenericEnv::require_current, caller source-plan
guards and the signature guard remain required. Token revalidation compares
the actual active Arc handles; source application is not activation/commit or
solver authority. No app-digest-only, Unifier or source-row matching proof.

One shared read-only inspector is needed after unfreeze: GenericEnv currently
exposes only key lookup, so external code cannot prove both private retained
tables contain zero rows. A narrow `has_empty_bindings()` over both owned tables
or borrowed row slices supplies direct evidence. The lead owns that shared edit.
The current `application_identity == None` recipe is insufficient on its own for
this requested boundary, and cannot expose hidden extra rows.

Existing ordinary call integration can obtain the actual callable with O(1)
`RunMachine.calls.last().call.callee` and the active environment from
Frame::shared_generic_context. `enter_callee` retained the former and installed
the latter; no new active-callable table, global lookup or legacy-path allocation
is necessary. `call.closure.is_some()` and mismatched body/generic owners stay
typed pending, as do nonempty generic/application rows and missing source
receipts. Root main with no call stack/environment keeps the independent
source-proven missing-env path. The lead will integrate token consumption into
caller encoding only after source ownership returns; frozen caller/run/packet
modules are unchanged by this design note. Builds/tests remain validator-owned.

Caller-application source release is now authorized after checkpoint
`7ea94b8f6b`. Before-code scope remains only new `upstream/caller_application.rs`
and this record. The source-only token will join retained caller-clause plan,
actual entered environment/callable Arcs, guarded FunctionSignature query and
direct empty binding-table inspector. Preparation/mismatch are typed outcomes,
not a guest static refusal. The lead owns shared inspector, registration and
caller encoder integration. Generic/nonempty/lexical cases and actual consumer
activation remain held; no Cargo/tests are assigned to this worker.

Implemented only `upstream/caller_application.rs` after that release. API:
`VerifiedCallerApplication::prepare(db,Arc<PreparedSourceCallerClauses>,
Arc<GenericEnv>,Arc<SourceCallable>,&CallerSourceContext)` returns a guarded
private token or typed CallerApplicationPending through QueryPoll. The lead's
`GenericEnv::has_empty_bindings()` reads both actual retained arrays. Preparation
also checks actual complete count0, full current FunctionSignature equality with
environment and callable, Q75 actual header/context/layout and the exact DTO.
No signature is synthesized, and no app digest or custom Unifier certifies data.
FunctionSignature query input guard, exact source plan/environment/callable Arcs
are retained. `matches_caller` accepts only the same retained source plan and
descriptor. `require_current` receives current actual environment/callable Arcs,
compares their identities, and revalidates all retained query/source/callable/
environment evidence. SourceCallable's inherited broad revision refusal remains
existing debt; no new global guard is introduced. Source mismatch, unsupported
lexical/nonempty namespace, bindings and currentness remain typed pending.

Separate all-eight source pass against SHA256
`9b0b39f1c6849322988a2010be6253b24f4f84aa3febde8f20344a159c5a97a2`:
(1–2) no evaluator or special generated/assertion execution
(MA001/MA004/MA005); (3) no method/numeric selection or inference
(MA003/MA006); (4) actual Q75/signature receipts and common SourceCallable, no
origin predicate; (5) no body lowering, static checker, patterns or cold-arm
preparation (MA002); (6) retained memo guard and Arc owners, no parser/phase
reconstruction at consumption; (7) current exact handles and input guards, no
namespace/world census; (8) Arc retention and actual source metadata, no body
clone/search (MA008); no macro identity dispatch (MA007). Prepare executes only
on a consumer request, at source preparation before solver entry. No legacy
call allocations or execution semantics change without the lead's integration.
The token authorizes only source application: the actual run/attempt/activation,
caller ParamEnv domain, inference owner, root readiness and commit remain
separate. Source-caller encoder, run/packet and registrations were not edited.
Formatting/diff checks only; actual application controls, origin/edition matrix,
suspension and runtime behavior are unexecuted. No Cargo/tests by this worker.
