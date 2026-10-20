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
