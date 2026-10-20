# M020 immediate filtered trait clauses

Before edits: LOCAL-049 / WT05-02 / M020, after V513/V514 checkpoint work;
source checkpoint f4a74d3bf0f8ae1b0dc276c5f87cbdd7e708fda4.
Worker owns only new traits/upstream/source_trait_clauses.rs and this record.
Lead owns common frontend collector/filter/default views, source_types sites,
Q78/Q84/Q82 integration, registry and tests. No Cargo/tests/commits/new agents.

Invariant and unchanged witness: repr128's actual projected PartialEq<U>
candidate needs compiler explicit_implied_clauses_of during true impl_super_outlives
elaboration. Lowering all Q78 own requirements first can incorrectly force an
unrelated Rhs or HRTB requirement that the compiler filtered query never requests.
The immediate filtered query must collect actual selected Self/associated subjects
before ordinary Type/bound lowering. It cannot obtain a complete answer by
discarding Pending rows after lowering everything or substituting supertrait text.
DiscriminantKind's real output bounds, PartialEq and the existing HRTB parent
witness supply genuine source contexts; new fixtures are not owned here.

Pinned policy ae0fe511b15ab760d7e963b13389d1bc2ab42b18:
compiler/rustc_hir_analysis/src/collect/clauses_of.rs618–750 maps Super to SelfOnly,
Implied on ordinary traits to SelfAndAssociatedTypeBounds and Implied on trait
aliases to All. implied_clauses_with_filter lowers real colon superbounds,
applies implicit sizedness/default-trait policy for actual TraitSelf, then probes
matching where bounds. Positive immediate trait clauses describe recursive
same-family demands. The compiler also forces those demands for cycle detection;
this bounded source query returns their descriptors and does not claim that
transitive closure or cycle validation occurred. The pinned solver/elaborator
continues to own semantic elaboration and cycles.

Common route: Q75 authentic DefinitionHeaderPlan and actual DeclarationGenerics
-> shared DeclarationParameters real implicit Self -> retained direct trait AST
and source owner/header/context -> frontend::source_predicates shared filtered
collector PreparedWrittenBounds -> Q84 TraitSelf default policy with actual
colon/default provenance -> common SourceBindings/encode_recipe -> scoped shared
IR. No Q78 full own-bound query or body/signature preparation is requested.
Generated/ordinary/wrapper declarations use the same real scope/hygiene/edition
source receipts. Unknown associated subjects, nested associated constraints,
default language roles or generic layout remain explicit pending dependencies.

Q88 proposal: SourceTraitClausesKey {inventory,definition,filter};
SourceTraitClauseFilter::{Super,Implied}; PreparedSourceTraitClauses retaining
header/environment/default/view/bindings roots, immediate recipes and positive
dependency descriptors with actual trait IDs and written spans. Query-specific
pending does not erase retained written rows or certify a full compiler query.
No independent trait solver, canonicalizer, alias normalizer, elaborate algorithm,
namehash DefinitionId, sparse-registry absence or fake complete provider.

Cold costs: one demanded pure memo plan over the selected declaration's retained
AST bound sites and selected source/type dependency closure. Arc-retain source
roots; no execution parsing/planning, body clones, dispatch namespace census or
resource accounting. Immediate positive-dependency enumeration is proportional
to retained selected rows. Full recursive query closure remains separate and
held. Acceptance requires actual source controls for filters/aliases/default
spans/unknown constraints plus memo/currentness and all owning editions/origins;
this worker runs none. Final all-eight source review/hashes will be recorded.

## Implemented component boundary

Q88 key/result/module are SourceTraitClausesKey / PreparedSourceTraitClauses /
source_trait_clauses. Super/Implied selects the pinned SourceBoundFilter policy
using actual Q75 TraitHeader.is_alias, not a name or an assumed Q85 default.
DeclarationParameters::from_upstream(0) authenticates the real implicit Self
source parameter; missing or non-Type slots remain Parameters pending.

SourceTraitClausesPending distinguishes Header, NotTrait (invalid compiler query
domain), Parameters, Environment, AttributeExpansion, AliasParameterDefaults,
Defaults(DefaultBoundsPending), and Recipe(SourcePredicatePending). Source and
default pending are independently observable. recipes() retains raw immediate
rows even when held; pending() must be checked before using them as a complete
immediate answer. encode and generic_layouts enforce that hold internally.
No result certifies the transitive query forcing/cycle behavior of rustc's query.

Shared PreparedWrittenBounds retains actual source-site provenance and produces
default_inputs(TraitSelf); Q84.apply validates definition/header/owner/site before
applying policy. TraitSelf input contains actual colon rows only: Self: Sized in
a where clause does not move a default from item span to where-bound span.
The shared view's with_defaults method places generated TraitSelf rows before
colon rows and matching where rows. The immediate producer does not infer
that boundary by comparing lowered subjects. Written and default rows preserve
their actual spans, including unresolved associated constraints.

Pinned probe_ty_param_bounds_in_generics982–1055 adds type-parameter defaults for
filter All and ignores lifetime/const parameter defaults. The alias Implied All
case with own Type parameters is explicitly AliasParameterDefaults pending until
that complete producer is present. Its existing written and TraitSelf rows remain
visible, not an invented complete answer. Ordinary trait Super/Implied must not
lower unrelated Rhs inline/where rows before filtering. Filtered All also cannot
blindly include region-only predicates skipped by the pinned BoundPredicate loop;
the shared collector owns that classification.

positive_dependencies() carries real trait definition, same target filter,
origin and actual span for every immediate Implemented row, including real
default rows. It describes the pinned positive recursive query demand; no Q88
recursive fetch or custom cycle walk is introduced. pending_dependencies()
carries origin/span/PredicatePending for every unresolved row, retaining nested
associated/equality constraint evidence without inventing a trait identity.
The common require_recipe gathers authentic source/type dependencies for known
selected rows. A Pending row holds encoding even when other bindings exist.

Q88 does not fetch DeclarationSourcePredicates/Q78, SourceDeclarationClauses/Q80
or full Q85 flags. Q75/header, completed environment, Q84 and selected
SourceBindings are retained as immutable roots. Actual parameter/Self/trait
generic identities and original/generated context receipts are verified before
pure scoped encode_recipe translation. No callback IO, source query, solver
entry, body/checker, initializer evaluation or independent elaboration is added.

## Separate eight-mode source review

Component review source SHA256:
2c16f1aa5ae53f3067f6e3cf67c05cca7ee4efbb5aca50a5381510198ae77817.
The lead's shared collector/default/registration edits require their own review;
this is not a claim that this component compiled or that any control passed.

1. No assertion/expression compiler: prepare consumes actual header and shared
   filtered collector; encode uses existing source_predicates::encode_recipe.
2. No literal/break macro path: canonical declaration/header/context and source
   owner are shared by original/generated/wrapper source; no special macro ID.
3. No context-specific method/numeric inference: filter is the pinned compiler
   query enum, not an operation/trait spelling or fixture case. No goal is solved.
4. No generated-source exclusion/hygiene fallback: Q75 real source parts and
   selected declaration owner preserve token edition, scope, context and origin.
   Missing evidence remains typed pending; no module fallback is introduced.
5. No pattern split/eager executable checks: only trait declaration bounds are
   selected; no fn signature/body, unchosen arm or borrow/static checking.
6. No execution planning: one demanded memo plan retains Arc roots and the shared
   view; pure encoding never receives Database. CurrentSourceTraitClauses uses
   PreparedInputGuard for actual memo dependency revocation.
7. No dispatch/world census: selected declaration sites and selected known-row
   dependency closure are cold work. Completed environment/catalog is reused;
   execution performs no source map construction/comparison. Cost is linear in
   retained rows plus existing demanded source binding work; scaling unmeasured.
8. No BodyIndex/deep body clones/whole-file search: one retained syntax_at on the
   actual declaration, shared collector view and Arc query roots; no executable
   body structure or separate parse is created/copied.

Remaining alternate routes: legacy full own source transport and Q80/Q81 remain
their actual distinct query domains. They are not repurposed as filtered answers.
Their eventual replacement/removal requires real root integration evidence;
this slice adds no runtime dispatch and does not promote a held path.

Expected controls (validator-owned, currently unexecuted): unchanged issue62529-1
Inject Self-where Sized provenance; unrelated QueryStorageOpsAsync Rhs HRTB stays
cold under Super/Implied; selected AsyncQueryFunction nested associated equality
remains actual-span AssociatedIdentity pending; all editions/origins and memo/
currentness/epoch revocation. Trait aliases/type-param defaults, binder remapping,
negative/const/equality lowering, complete transitive closure and full runtime
query factory remain held. Repr128 root, interpreter/JIT gates and host abort
policy are unchanged. No new runtime gain or fixture acceptance follows.

Exact pinned compiler source inspected:
compiler/rustc_hir_analysis/src/collect/clauses_of.rs SHA256
465f5dd0f397f87716aedc65048a12dce7ec3af51d88e584ea5ad902b2251bf3.
No raw original or generator is added by this worker; this is bounded source
query policy transport using existing retained licensed compiler algorithms.

## Authorized actual-source encoding observer follow-on

Lead subsequently authorized only a cfg(test) observer in the worker's existing
module. `check_actual_filtered_trait_encoding(db,key)` returns
`Result<(CurrentSourceTraitClauses, Result<usize,SourceTraitClausesPending>),
Diagnostic>`. The nested result distinguishes an actual encoded count from a
typed pending refusal; refusal is not counted as an empty clause list.

It captures the real Q88 memo/currentness guard, authentic catalog and actual
catalog-root SolverConfiguration. Ready plans use their actual SourceBindings
generic_layouts with the existing with_source_context construction service and
call the SAME plan.encode production function. Each selected positive Self row
must retain actual full DefinitionId and source trait digest, positive clause
polarity, Self source parameter key, upstream Self index zero, argument count,
origin and source range. Relaxed rows are not counted as clauses. No query occurs
inside the encoding closure, checked against the real Database trace.

For the genuine nested-associated pending plan, the same actual catalog/config
context is constructed without construction generics; production encode must
return exactly plan.pending() at its early gate. No Type constructor or missing
callback is invoked, and no fake provider/facts, root/dummy goal, source recipe or
semantic fallback is introduced. The control returns its real Current guard.
No actual source control, build or Cargo test was executed by this worker.

Independent reviewer source-read this helper and its CLI call sites, finding no
bounded type/lifetime/semantic blocker. A final added assertion also checks actual
upstream Self parameter index zero. Current worker source SHA256:
d9d5b54648966f9965e8d9f5fbfbd51f3ed658d9fb948b110d1bcf680022a9dc.
The earlier 2c16f1aa... review remains attached to its pre-observer production
module; production behavior is unchanged by this cfg(test) addition. The eight
architecture modes remain as reviewed above: this observer calls the shared
encoder without another parser/evaluator, mutable guest state or full fact/root
entry. The selected controls now contain encoding assertions but remain
unexecuted until validator-owned commands terminate.

## Separate read-only shared collector/default-view pass

Inspected SourceBoundFilter/preparation, PreparedWrittenBounds/default_inputs/
with_default_sets, Q84.apply and Q78/Q82 view integration without changing any
lead-owned file. `prepare_filtered_trait_bounds` filters actual owner plus colon
or direct Self where subjects BEFORE shared Type/name/binder lowering. Ordinary
Super/Implied keep unrelated Rhs inline/where HRTB cold. Parenthesized direct Self
is recognized without a second parser. Alias All skips region-only predicates as
the pinned BoundPredicate loop does and retains its own type-default hold here.
Selected nested associated constraints reach the common AssociatedIdentity
Pending row in both filters; no later post-lowering drop claims completeness.

The source view records actual owner/site and exact bound range before lowering.
Duplicate sites and lowered Explicit rows without a site are errors. Thus
default_inputs cannot silently discard a lowered Explicit/Pending row due to an
unmapped site in these prepared views. TraitSelf selects colon sites only;
TypeParameter selects the genuine direct parameter key (including parenthesized
direct parameter occurrences); AssociatedType selects actual output sites.
Rows outside the selected default context remain in the complete written list.
Q84 checks sealed definition, HeaderRecipe fingerprint, actual owner range,
context and actual site origin before policy reads. with_default_sets retains
TraitSelf default, colon, parameter default and remaining written order. Q78
collects real parameter default sets separately; Q82 retains its output view.

Boundary: the pre-existing raw actual_declaration_bound_syntax collector still
uses `else { continue }` for an AST generic parameter that cannot join supplied
generic metadata. The authenticated Q75/DeclarationGenerics own-trait mapping
protects this Q88 route; this pass is not a proof of all unrepresented associated
or legacy contexts. No broader source readiness claim follows. The changed HRTB
parent outcome can correctly become Recipe(Bound(HigherRanked)) once the default
view stops demanding an unrelated where row: its real source recipe is retained,
not reclassified as success or removed. Pending hierarchy/root holds remain.

Inspected shared source hashes at this pass (later edits require rechecking):

| Path under crates/lrr/src | SHA256 |
| --- | --- |
| frontend/source_predicates.rs | c6b1f75f3f5448c1ebf417778e8f5a113780acdd6d6de9092134c1b1f201efff |
| traits/upstream/source_default_bounds.rs | 65c3b09fd15c9861ca0aa758249a3c3fb00013f71291b37d8faec9282c036a98 |
| frontend/declaration_source_predicates.rs | b38ccc994526eec9d2a62e33efb6bdc1313a5f8f6428f6ce400512e58cb47d36 |
| frontend/source_item_bounds.rs | 1a3d48a29b6679ffd977417089a94e8f5f225a50be6f2b590886ef1795d9790c |
| frontend/source_types.rs | 9bea037849e7cd28952ab3548f788860095457fff5a40f5e94883cad174b2b21 |

## Shared explicit-Sized ordering follow-on

Lead authorized a final narrow Q88 integration of the shared
PreparedWrittenBounds::order_explicit_sized policy. Q88 now makes its retained
view mutable and calls that method after actual Q84 key validation, before
default application. The common service uses the authenticated Q84 Sized language
identity and actual owner/bound-list site, reproducing repeated front insertion
within each real list. It does not order traits by name or combine separate where
lists. TraitSelf defaults precede the ordered colon list, then matching where
rows. The previous colon-before-default description is corrected above.

An ordering failure is retained in default_pending. Successful subsequent apply
cannot erase it or replace the actual written list with a falsely complete one;
every written row remains visible, and encode/generic_layouts refuse the pending
plan. An apply failure also preserves an earlier ordering hold. All query spans,
positive/pending descriptors and source receipts remain those of the actual rows.

Final worker source SHA256:
2abfb8c0e90c85f934c0b1ca51dc0d4647fa1028c472436c6f7c5590ecc1f0d7.
The d9d5b546... observer review remains attached to its prior source; the new
production change is only this shared ordering call/pending propagation and the
corrected order comment. Inspected common frontend/source_predicates.rs at this
follow-on: a6dbdfe9768ae462cca1ea88a078dd1287879940476c5a4537fc5384a6460412.
All eight reviewed architecture modes remain unchanged: existing source service,
authentic role identity, no additional parser/solver/query callback/runtime route,
no new hot work or immutable body copies. Actual order/encoder controls remain
validator-owned and unexecuted here. No Cargo/build/tests/commit was run.
