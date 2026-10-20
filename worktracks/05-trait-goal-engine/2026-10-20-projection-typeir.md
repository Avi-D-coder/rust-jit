# M020 actual source projection identity and shared TypeIR

Before-code bounded record. Baseline
`289995a5accac76fcb386b46d863e1118cbb6e6d`. `/root/wf_adt_metadata` owns
`upstream/source_bindings.rs`, `source_terms.rs`, `source_generics.rs`,
`source_adt_fields.rs` and construction-alias sections of `prepared.rs`.
`/root/wf_value_helpers` owns the actual trait-associated declaration query
and source predicate transport; lead owns registration and nominal-header
payload. No Cargo/tests/commits or new subagents by this owner.

Invariant/minimal witness: `TyTerm::Projection` currently retains a legacy
associated-name hash, not an actual associated declaration identity. That
hash must never become an upstream DefId. Existing shared source Types stop
at `ProjectionMetadata`; a guessed alias struct or argument-kind layout
would bypass both source identity and upstream constructor semantics.
Unchanged official witnesses are enum-discriminant/repr128.rs (projected
Discriminant type bound) and union/union-generic-rpass.rs (MaybeItem<T::Item>).
No builtin Discriminant rule, normalization shortcut or fixture semantic is
permitted. Runtime originals and all gates remain held until genuine evidence.

Shared route: ordinary actual Type/predicate source service -> symbolic
projection owner/item-name key and use-site arguments -> `SourceRequirements`
-> actual `TraitAssociatedTypes` query and `DefinitionReceipt`/generic facts
-> immutable `SourceBindings` mapping `(owning trait, item-name key)` to the
actual associated declaration -> merged `SourceGenericLayouts` with actual
associated/parent argument kinds -> `source_terms::Encoder` -> pinned
`AliasTy::new` constructor and shared TypeIR. Associated source generics and
parent joins must validate the actual owning trait. Trait arguments include
Self; associated arguments follow real parent-first layout. Missing facts or
unsupported representations remain explicit pending, never fabricated aliases,
empty layouts, full provider readiness or semantic NoSolution.

`SourceRequirements` separates real declaration IDs from unresolved projection
keys and recursively demands Self, trait and associated item argument Types.
The existing pure memo retains real lookup query handles, source receipts and
generic facts. `SourceGenericLayouts` may supply genuine construction-only
projection alias layout/parent facts before full ready provider publication;
that is a partial actual-data interface, not a default provider. Installed
callbacks continue reading immutable metadata only, with no parser, IO, guest
initializer, independent solver or whole-body/static checker.

No alternate parser/evaluator/member-selection route is added. Original and
generated syntax already join the existing actual source service and use the
same declaration-keyed query. Cold preparation builds deterministic maps and
traverses required source Types; encoding traverses retained IR and invokes
upstream constructors. Runtime dispatch must not redo source lookup or global
namespace census. Immutable query roots are shared through Arc; no BodyIndex
clone or whole-file syntax search is added. No resource cap or performance
claim is introduced.

Acceptance plan: parent/validation-owned asynchronous macOS1.99 build, actual
associated declaration IDs and source parent/generic layouts, pinned constructor
argument compatibility, unchanged projected predicate/field observations in
all four editions and original/user-macro/wrapper contexts, memo reuse and
source/epoch withdrawal, cold executable bodies. Required negative controls
include missing/foreign/ambiguous associated metadata and unsupported generic
argument metadata, never a successful name-only identity. Full root/member/
projection normalization/VM/runtime/interpreter/JIT gates remain open. Final
separate source review covers all eight recurring modes against a fingerprint.

## Source-only implementation and review, October 20

The bounded component now retains actual `TraitAssociatedTypes` query roots in
`SourceBindings::prepare`, resolves each legacy `(trait, item-name)` key to an
actual associated declaration, and recursively demands argument metadata.
`projection_parent_matches` checks retained source binder/parent joins;
`SourceGenericLayouts::from_bindings` supplies only those actual alias kinds
and parents. `PreparedProvider::source_projection_parent`, `parent` and
`alias_layout` expose this construction metadata without claiming full facts.
`source_terms::Encoder::ty` checks argument compatibility and uses pinned
`AliasTy::new_from_args` and `to_ty(IsRigid::No)`. It does not normalize aliases.
`prepare_declarations` has an explicit `BTreeMap` collection type.

The actual ADT and impl-predicate consumers now pass `SourceTypeBindings` to
the same encoder. The old supplemental `ProjectionPending` field case becomes
`ProjectedField`; historical V497 counts remain evidence for their old source.
New cfg(test) helpers are `check_actual_projected_source_fields(db, seal,
owner, expected_trait, expected_item) -> (CurrentAdtSourceFields, usize)` and
`check_actual_projected_source_predicates(db, seal, owner, expected_trait,
expected_item, expected_bound_trait) -> (CurrentSourcePredicates, usize,
usize)`, both returning `Result<_, Diagnostic>`. Predicate input is actual
`DeclarationSourcePredicates` query recipes, not a fabricated function/impl.
The helpers assert actual associated DefId, parent, Self parameter, own
arguments and projected bound arguments through the common encoder, retain
real query/currentness closures, require full provider readiness to remain
closed, and assert no queries occur while encoding. Parent-owned CLI controls
independently select source IDs and test memo reuse/withdrawal.

Separate source review against the six fingerprints below:

1. No assertion/expression evaluator is introduced. Both consumers reach
   `source_terms::encode_declared`/`encode_predicate` and `Encoder::ty`.
2. No generated literal/break executor is introduced. Associated lookup uses
   the shared source query; actual generated parent joins remain its authority.
3. No context-selected method/numeric inference is introduced. Source trait
   identities and argument layouts determine the alias; Discriminant has no
   production special case. Witness-specific assertions are cfg(test) only.
4. No new original-only branch is added. Missing source parent/associated
   evidence is explicit `ProjectionMetadata`, not a guessed identity or a
   generated-source semantic exclusion.
5. No pattern engine or executable arm/body preparation is added. This slice
   consumes declaration Type and bound recipes only.
6. Query roots, generic facts and receipts are retained in immutable plans;
   encoding traverses retained Types and invokes pinned constructors. The
   source-only controls assert no query trace growth during encoding.
7. Demands are relevant declaration/projection maps, not a runtime full-world
   scan. Cold preparation follows transitive declared metadata. Encoding uses
   map lookup plus work proportional to encoded arguments; no speed claim.
8. Query data uses Arc retention; no BodyIndex clone or new whole-file node
   search occurs in these changed symbols.

The independent reviewer inspected actual Q78 recipes, retained Q77 metadata,
source generics, strict alias identity/arguments, currentness closure and
closed readiness, and reported no fabricated provider or handmade projection.
This is source review only. `git diff --check` exited 0. This owner ran no Cargo
or tests; all edition/origin, negative, cold-path and withdrawal controls remain
unexecuted until the validation owner's actual terminals. No runtime/full
fixture/solver/interpreter/JIT gate is closed here. Complete DefinitionFacts,
own/parent/elaborated predicate publication, unsupported alias argument forms,
normalization and remaining legacy runtime projection paths remain held.

Frozen source SHA256 (paths under `crates/lrr/src/traits/upstream/`):

```
8edb31e89378e36a681898a17417b06d2ea41579d0312b0d3091252fb1bd249d source_bindings.rs
6885880fae4230269947886ee08f9b2c0e0055d76092d3bdabb59520ac4f9d48 source_terms.rs
b12592aab6040d6b2905084c8a5191cc6d2a6c9fe598a3652b324f4ce232877c source_generics.rs
155b8592fd05fff3578680235ae98cbed15646c5bbc64181fdb74a5569ac9de4 source_adt_fields.rs
e57cb8dee81abdf25c7a13e249c75cd66b75f60b6f3dcc779884bc1f46f6a653 source_predicates.rs
6f9a308c2d9b611ad177eab180dfc3f326dccc9dcdb403b917f894afd04e9f37 prepared.rs
```

## Read-only next prerequisites after the frozen candidate

The already imported unchanged `generic-associated-types/streaming_iterator.rs`
(SHA256 `88fde850c67583b37bc771e708b554779bd9610396aa2ce3867a27f71c67b1d1`)
provides an actual `Foo<T>::bar` field of
`<T as StreamingIterator>::Item<'static>`. The common
`source_types::shorthand_projection` currently refuses final-segment generic
arguments for both shorthand and qualified projections and emits empty
`item_args`. The existing ProjectionKey, actual associated generic layouts,
common TypeIR encoder and pinned AliasTy constructor can retain own region
arguments. A future common argument transport must use the ordinary Type walk
and WrittenTypeDeclaration::region and verify the real `[Self, 'a]` layout and
parent/own split; no guessed parameter kinds. GAT const literals additionally
need actual const-parameter Type/width/substitution metadata; HRT/equality and
associated predicate closure are separate missing contracts. No GAT source or
runtime behavior is added in this checkpoint.

Complete DefinitionFacts remain blocked independently. In particular, variance
must reuse the pinned hir_analysis variance terms/constraints/solve algorithm
through source adapters rather than infer covariance from field examples. Its
nominal branch ignores where-bounds but requires the actual field dependency
closure; functions require genuine signatures, and opaque/RPIT needs captures.
A complete lang registry likewise needs authenticated expanded declaration
membership and dependency/facet completeness, not per-declaration
SourceLangIdentity results or a spelling whitelist. Neither follow-up is
implemented or counted as successful source-factory/root/runtime acceptance.

### Source-review defect: query inapplicability is not missing data

Read-only follow-up found `prepared::DefinitionFacts::variances` is an
unconditional slice and `PreparedProvider::variances_of` returns it for every
role. The pinned compiler query first returns a genuine empty slice for actual
upstream generic count zero; with nonzero count it only computes supported
nominal/function/constructor or opaque/RPIT variance roles, and otherwise reports
an invalid-query contract bug. An ordinary trait's implicit Self makes its count
nonzero even if it has no written parameters. Thus demanding a completed slice
for an ordinary trait encourages a fabricated value and loses the pinned query
domain. This is an implementation representation defect, not a specification
exception or evidence of a runtime failure: no real upstream root is invoked by
this checkpoint.

A follow-on repair should distinguish authenticated `Known(variance values)`
from `NotApplicable(role proof)`, with typed Pending only during preparation,
and preserve the pinned invalid-query behavior if a reader is called outside its
domain. Actual applicable variance production remains necessary; no universal
empty/invariant arrays or source field covariance rules are authorized. Audit
other factory fields for the same distinction before implementing unnecessary
static compiler work. This finding keeps factory/admission completion held; no
source change or acceptance for this follow-up is included here.
