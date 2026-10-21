# Bounded full impl-clause producer

Precode record, 2026-10-20. WT05 / LOCAL049, M019/M020, E-R5-014;
unchanged `repr128` reached discriminant/PartialEq inference remains the
integration witness, not an implementation selector. No fixture execution or
root admission is claimed here.

Pinned source review (ae0fe511b15ab760d7e963b13389d1bc2ab42b18):
`rustc_hir_analysis/src/collect/clauses_of.rs::clauses_of` appends inferred
outlives and adds Self:Trait only for a trait definition. Actual Impl has no
generic parent and its inferred-outlives query is empty. Its
`gather_explicit_clauses_of` inserts the default-impl trait assumption, implicit
Type bounds and ConstArgHasType in actual HIR parameter order, then written
predicates, feature-gated const-evaluatable rows and unstable-feature rows. The
IndexSet preserves insertion order and deduplicates the (clause, span) pair.
The trait-only associated-bound filter and AnonConst default branch of
`explicit_clauses_of` are not Impl branches; const_conditions remains a separate
query and is not supplied by this producer.

`rustc_hir_analysis/src/constrained_generic_params.rs::setup_constraining_clauses`
only rearranges Projection clauses, using the real projection dependencies and
parameters_for_impl. Existing Q80/Q84 transport cannot emit Projection clauses:
unsupported associated constraints remain Pending. Thus the admitted current
clause kinds need no ordering algorithm. A future projection producer must port
the pinned helper; do not substitute a matcher or handmade ordering loop.

Shared route: Q80 DeclarationSourcePredicates retains actual source/binder and
Q84 default-bound recipes; SourceDeclarationClauses retains actual SourceBindings
and PreparedClauseEmission. A minimal owned-ordinal encoder will share its
existing row transport, permitting the full-Impl adapter to interleave actual
const-parameter Type constraints with implicit Type rows. Declared const Types
come from the same DeclarationGenericTypes query and ordinary source_terms
encoder, and use DeclarationParameters' actual upstream index mapping. Written
rows retain their real source spans. No callee bounds become caller assumptions.

Bounds: default impl assumptions, generic_const_exprs, unstable_feature_bound,
unsupported attribute transformations, missing parameter/source/query evidence
remain typed holds. The existing collector omits `where Ty:` rows, whereas the
compiler can insert a genuine WellFormed clause there: full certification must
hold such source syntax until a common producer exists. No fake WellFormed root
or empty substitute is admitted. All encoded kinds will be checked against the
bounded no-Projection domain before a full result is returned.

Ownership: source_impl_semantics.rs, new source_impl_clauses.rs and the granted
minimal source_declaration_clauses.rs accessor only. Parent owns source factory,
bundle, prepared/root authority and module registration. Existing null-branch
API must not silently cause the factory to publish empty for new nonempty rows.
No Cargo/Git or solver algorithm edits. Preparation performs source header/query
work once; encoding traverses retained clauses/const Types and pair-deduplicates
in source order. No body lowering/execution, installed dispatch parsing, global
namespace comparison or BodyIndex clone is added.

Required evidence: exact source review/all eight recurring failure modes, source
fingerprints, parent compilation and actual nonempty full-clause publication;
then unchanged reached runtime witness with real callback/query closure and
owned response/VM commit. Component source work does not close these gates.

Implemented handoff: source_impl_semantics now retains the genuine
SourceDeclarationClauses memo behind the existing source/configuration gates.
`full_clause_bindings()` supplies its authenticated binding closure for parent
context construction; `encode_complete_clauses(cx, configuration)` returns the
actual DefinitionId and complete bounded `(Clause, Span)` sequence, or a typed
hold. The old `complete_clauses_of()` remains a separate null-only API; parent
must use the new result with no fallback-to-null after a hold. Stable memo domain
is `lrr/v2/source-impl-semantics` and the new retained helper is
`lrr/v1/source-full-impl-clauses`.

`SourceDeclarationClauses::encode_recipe_at` accepts only an ordinal of its own
retained recipe list; both this method and existing whole-list encode share
`encode_owned_recipe`, existing PreparedClauseEmission and bindings. No second
emission policy/sidecar/planning was added. Actual Q84 defaults use whole Type
parameter source sites; explicit bounds use their nested bound sites. The
adapter checks the actual ParamKey and prefix ordering before grouping those
retained default ordinals. It then iterates original parameter order, emits real
declared ConstArgHasType rows, and emits remaining written ordinals. The const
Param mapping must also equal cx's actual source_parameter mapping. Pair hashing
is only membership for insertion-order dedup; it never selects candidates or
changes output iteration order. No new licensed algorithm input/tool is needed:
the current admitted kinds make the pinned Projection permutation a no-op.

Separate all-eight source pass (uncompiled source component):
1. No assertion/expression compiler/evaluator; only immutable Q80/Q84 transport.
2. No macro-specific output route; retained declaration receipt accepts original
   or admitted generated source through the same DeclarationGenericTypes path.
3. No method/numeric selection; recipes become real clauses, never solver
   answers or assumed caller evidence.
4. No original-only restriction; incomplete source/binder evidence stays typed.
5. No patterns, executable-arm preparation or body checks; actual impl headers,
   parameter Types and bound recipes only.
6. No installed execution changes. Preparation uses memoized source queries;
   encoding walks retained rows. The ordinal accessor rechecks immutable owner/
   world metadata per row but does not parse or prepare a source plan.
7. No namespace/global map comparisons in dispatch. The existing SourceBindings
   preparation requests only definitions/Types referenced by retained clauses;
   head catalog preparation and parent demand-closure policy remain separate.
8. No BodyIndex copies or file searches; exact syntax_at location is used once
   to detect the currently missing empty-where producer.

Concrete unchanged official metadata controls for the parent validator:
`tests/ui/const-generics/broken-mir-1.rs` is already imported and has the actual
`impl<T, const N: usize> Foo for [T; N]`: full output must contain genuine Type
default clause(s) first, then ConstArgHasType of that impl's N with declared
usize Type, no parent, and source parameter spans; own/all use the same list.
`tests/ui/const-generics/issues/issue-69654-run-pass.rs` adds an actual written
`[u8; N]: Bar<[(); N]>` after its ConstArgHasType row; inspect definition/args and
source ordering instead of only total count. Neither control is executed by
this agent, and these metadata assertions are not full runtime fixture passes.

Terminals: `rustfmt +1.99.0 --edition 2024` on all three owned Rust files exited
0. No Cargo, test or Git command was run. Final source SHA256:
- source_impl_semantics.rs: 451b5d86cbda503de8687654b5d99ff34129eadd2cee3b911ed9236e2578a5ab
- source_impl_clauses.rs: 345a3d1e217531f60b95f25fce51cc54aa94f249f13793ce627303ca3812d793
- source_declaration_clauses.rs: cd801eae04fb7eae8daed1ff7ee870abd0e294b59e903ba97ce56852eefe0513

Independent reviewer requested against these sources; actual compile and
nonempty source-control results remain pending. Parent owns bundle/factory
integration and normal-root lineage, query closure, fulfillment and VM commit.

Successor pre-freeze update authorized by lead: the legacy null-only accessor
now also requires the retained full collector to have no pending family. An
actual empty-where hold returns ClauseEmptyWhere; other missing full-source
evidence returns ClauseSourceRows. Its remaining callers are the generic/GAT
metadata control in cli/preentry_loader_tests/upstream_source_clauses.rs and
the actual primitive-impl null control in upstream_projection_types.rs. The
factory has independently migrated to the full result with no null fallback.

Added cfg-only `check_actual_full_impl_clauses(db, inventory, canonical_impl)`
through source_impl_semantics. It resolves the real raw impl from the inventory,
captures CurrentSourceImplSemantics, builds the actual binding/layout context,
and reports ordered actual encoded clause kinds, a TypeVisitable observation of
source ParamKeys and original spans. Const rows report their real source Type
after checking it against the actual encoded Type and parameter site. It checks
currentness before/after and no source queries during conversion. No replacement
row/candidate/Goal/environment input is accepted. Lead owns unchanged fixture
selection and assertions. This helper and those fixture controls remain
unexecuted until the validator's actual terminal.

Latest source SHA256 (supersedes the preceding pre-control stamp):
- source_impl_semantics.rs: 1e339c1f540a846b3f0d65297baa088604bce1fe50f0fa3c270376061499a924
- source_impl_clauses.rs: 3c8136abc57eff5fe3dd0d461f4623e3072b08a6bdd804d21718c8fbdabacc7a
- source_declaration_clauses.rs: cd801eae04fb7eae8daed1ff7ee870abd0e294b59e903ba97ce56852eefe0513
`rustfmt +1.99.0 --edition 2024` exited 0 again; no Cargo/Git was run.

Independent bundle/factory integration review, 2026-10-20: no actionable
finding in this bounded source pass. Exact reviewed SHA256:
`source_fact_bundle.rs` =
`f52ac7e9bf099c0829991577e9ed0919972d919670ccbf1440e183e0473093d6`;
`source_factory.rs` =
`5187810e5089ce261367832a85d1dbf51764625904f6d1a118f3b1903e615ecf`.
The separate reviewer owns the full-clause adapter review; this pass inspected
its public boundary and the actual binding/publication consumers.

`SourceFactBundle::prepare` includes successful `full_clause_bindings()`
definition keys in its real header closure, and `bindings()` includes the same
retained bindings in generic-layout merging and final exact-Arc Q77/generic
query-guard capture. An unavailable full-clause producer contributes no invented
layout; its encoding still returns a typed hold. `SourceImplSemantics` remains
the retained guarded memo root for the full producer and its transitive source
dependencies. `materialize` uses `encode_complete_clauses` directly, checks its
actual DefinitionId against the complete catalog's canonical impl owner, and
uses one retained list for all/own. A different owner is an error, and a pending
result leaves those independent query domains Unprepared. There is no fallback
to `complete_clauses_of()` or an empty list. Complete head/candidate publication
is kept separate from clause-domain availability; every complete Q65 member
remains represented even when one of its metadata families is held.

Separate eight-mode integration review:
1. No expression/assertion evaluator; only existing source-clause transport.
2. No macro-only executable route; the ordinary source receipt/binding closure
   continues into the same production materializer.
3. No local trait/numeric selection; actual clauses become facts for upstream,
   never candidate applicability answers or caller assumptions.
4. No generated-origin exclusion; incomplete metadata remains typed per domain.
5. No pattern duplication, body lowering or cold executable-arm checks.
6. Cold retained query preparation is shared; materialization encodes retained
   rows once per context and installed operations gain no source planning.
7. Only demanded complete catalog buckets and referenced source bindings are
   traversed; no full-world dispatch comparison is introduced.
8. Retained Arc queries and context lists are shared; no BodyIndex clone or
   repeated whole-file lookup is added by these consumers.

The observer's `impl_complete_clauses` counts actual Known full lists, and its
`candidate_buckets` copies actual Known canonical membership for exact Q65
comparison; neither field admits a root. Existing coarse `ready` remains held.
The unchanged repr128 source-factory control now requires nonempty generic
impl clause publication as well as exact outer/projection-parent membership.
These controls were inspected, not run. No Cargo/test or production edit was
performed by this reviewer. Full adapter source review, compilation, nonempty
metadata controls, normal-root closure, unchanged runtime witness and actual
VM response commit remain separate evidence requirements.
