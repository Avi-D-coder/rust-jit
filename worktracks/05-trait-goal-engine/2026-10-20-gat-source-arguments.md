# M020 bounded common associated Type arguments

Before edits: lead `/root`, baseline `3d25d2bcec3dad543861a193258d99b07f968c77`.
Own `frontend/source_types.rs`, cfg observation helper in
`traits/upstream/source_adt_fields.rs`, new CLI control
`cli/preentry_loader_tests/upstream_gat_fields.rs`, module registration and
selection. Workers independently own variance query representation and pinned
variance algorithms/provenance. Existing LOCAL-049/shared-source reservations
apply; no concurrent writers to these paths.

Invariant/minimal unchanged witness: imported pinned
`tests/ui/generic-associated-types/streaming_iterator.rs` declares
`Foo<T>::bar: <T as StreamingIterator>::Item<'static>`. Common source Type parsing
currently refuses any associated segment generic arguments, even though
ProjectionKey.item_args, actual associated generic layouts and the pinned TypeIR
AliasTy constructor can retain them. The operation must preserve those real
arguments through original/user-macro/wrapper sources in all four editions.
Full GAT normalization, inherited predicates and runtime main remain held.

Shared route: actual owning ParseFile/DeclarationGenerics + WrittenTypeDeclaration
and source name/hygiene services -> resolve_actual_type common Type walk ->
ProjectionKey(actual trait, legacy member lookup key resolved by Q77, own args)
-> SourceBindings(actual associated declaration) -> SourceGenericLayouts ->
ordinary TypeIR encoder -> pinned AliasTy argument checks and parent/own split.
Refactor the already shared syntactic Type/region generic argument collector
for nominal and associated segments. There is no second parser/evaluator,
synthetic owner, per-fixture type rule or method/solver algorithm.

Only actual Type arguments and regions supported by existing declaration
services are admitted. Unsupported const arguments/equalities remain explicit
preparation gaps; do not guess const Types, widths, source parameter ordinals or
HRT identities. Preserve selected trait defaults through the existing common
trait service. The imported original is never edited; wrappers and observation
assertions live outside it.

Cold cost is proportional to the demanded actual generic argument syntax,
through ordinary source/type queries. Results remain in existing immutable memo
plans keyed by actual source/scope/edition/epoch dependencies. No installed
runtime parsing, planning, global namespace census, body lowering or BodyIndex
copy. Full factory/root/member/VM and JIT readiness are not claimed.

Required evidence: actual field's own 'static region, real associated alias ID,
layout [Self Type, own Region], parent_count1, pinned constructor and own-arg
split, four editions/origins, memo reuse, cold signatures/bodies and epoch/source
withdrawal. Add Type-argument/shorthand witnesses from unchanged upstream tests
where available; an unexecuted form stays unexecuted. Separate all-eight source
review at exact final fingerprint, asynchronous macOS build/controls and existing
runtime regression replay are required before any checkpoint acceptance.

## Additional unchanged shorthand/Type argument witness

Imported `tests/ui/generic-associated-types/issue-87750.rs` unchanged from the
same pin, SHA256 `8ffcfd0c04408679d061ac8208076bf0f093652ad3a6175311440cdeeb7f2659`.
It is check-pass and has no auxiliaries or expected files; both rustc licenses
and COPYRIGHT were verified by the existing importer. Its actual enum
`Node<T,P>` has one field `P::Pointer<Node<T,P>>`. This exercises shorthand GAT
selection, a real nominal own Type argument with actual owner parameter IDs,
and a recursive nominal identity without recursively lowering fields. The
observation helper accepts the source ADT kind and checks actual source and
encoded nominal/parameter identities. It never supplies expected Types to the
encoder. The original compiler contract/full fixture remains unexecuted and is
not a runtime-contract pass; this is an actual source-Type component witness.
The original streaming iterator witness covers qualified own region arguments.

The shared argument collector is also consumed by ordinary nominal/module and
trait positional paths, replacing their duplicate Type/region loops. Trait
associated-equality bindings still use the existing separate constraint collector;
they are not positional arguments. Existing unsupported consts stay unsupported
pending genuine declared const-Type metadata. Neither full GAT normalization nor
trait/predicate/root/runtime completion follows from these source changes.

## Additional upstream variance edge control, before edits

Lead will add a separate actual-source control in new
`cli/preentry_loader_tests/upstream_variance_fields.rs` for unchanged
`tests/ui/variance/variance-use-invariant-struct-1.rs`. Its `SomeStruct<T>(*mut T)`
provides a strict invariant result from the pinned mutability/constraint walk,
contrasting Pair<T>'s covariant result. Runtime/static borrow error statements
inside foo/bar remain cold; full compiler diagnostics and fixture acceptance
remain unexecuted. Import its actual directives/expected stderr unchanged under
the existing original licenses. The host control supplies only expected outcome
observations to the same Q79 source/field/generic/pinned algorithm route, never
an expected value or rule to production. Cover all editions/origins and actual
memo/currentness/epoch withdrawal; root and runtime gates remain held.

## Lead separate source review (V502 input, before API repairs)

Reviewed source fingerprint
`a88c1b2a2a97acc4cb397d4a1524b7196bb9f79c6b42756b136eee81878c1a9d`,
HEAD `3d25d2bcec3dad543861a193258d99b07f968c77`. This is a source-path review,
not a passing build or runtime claim. Independent review and terminal guards are
owned by `wf_validation_review`. V502 fails the lib build at mechanical variance
adapter API joins; no new CLI, control, root or runtime execution follows.

1. No assertion/expression evaluator is added. `resolve_actual_type` uses the
   same retained AST and declaration context through `written_generic_argument`
   for ordinary nominal, trait and associated segments.
2. Generated output joins `actual_rows`/the existing admitted declaration queries;
   source preparation uses the same `DeclarationGenerics`, `AdtSourceFields` and
   TypeIR encoder. No generated scalar/literal executor is introduced.
3. `shorthand_projection` retains actual associated arguments; Q77 resolves the
   real associated member. Neither method nor numeric selection changes here.
   Q79 supplies source-authenticated query domains and pinned variance results;
   pending facts cannot become arbitrary variance slices in `DefinitionFacts`.
4. No original-source exclusion is added. The controls require actual generated
   source/definition receipts in three origins. Unproduced anonymous APIT and
   parameter macro identities remain explicit preparation gaps in the common
   metadata route, rather than fabricated zero counts.
5. No pattern or match-arm execution code changes. Parameter inspection walks
   only retained parameter Types and skips executable expressions/items. GAT
   field preparation and variance demand do not lower cold bodies.
6. The common generic collector replaces three duplicate Type/region loops.
   Q79 retains header/field/lang/configuration query roots; published callbacks
   read immutable results. No source preparation is added to installed runtime
   operations. The cold variance dependency closure follows demanded field
   Types and runs the retained pinned fixed point once per query preparation.
7. No new global namespace map or installed dispatch census is added. Nominal
   dependency closure uses actual retained field references and local identities.
   Existing catalog/discovery/currentness behavior is a prerequisite, not new
   performance evidence or proof of whole-pipeline scaling.
8. `source_field_types` borrows retained field terms; parser/query ownership is
   shared through existing Arcs. No BodyIndex is copied and no executable node
   is found by repeated whole-file search in this delta.

Remaining routes/holds: legacy interpreter trait selection is still active;
this component does not remove it or invoke the upstream solver root. Actual
source fact completion, inherited/item predicates, registry, normalization,
member/VM integration and architecture acceptance remain required. The pinned
variance algorithm retains all its branches, but only admitted nominal source
shapes are exercised here. Macro parameter frontiers, opaque/RPITIT/foreign/Fn
variance and complete GAT/runtime behavior remain unexecuted or pending.
The test-only `SignatureParameters` path does not consume the new APIT marker;
there are no production callers, and that limitation is not claimed repaired.

After the mechanical build fixes, this review must be rebound to the new source
fingerprint with the exact changed joins inspected. Actual passing evidence must
come from the asynchronous validator's terminal results and closing guards.

## Next implementation boundary after this checkpoint

Prioritize the shortest real interpreter-to-upstream-root join rather than
expanding the variance witness vocabulary. Independent read-only tracing finds
no `DefinitionFacts` factory or runtime caller of `evaluate_prepared` yet.
Ordinary operator execution still uses the legacy common member selector.
The immediately useful dependency is genuine declaration/associated predicate
production, preserving pinned own/all/parent/item/self/nonself distinctions.
Use unchanged `associated-types/assoc-type-in-struct-constructor.rs` as the
smallest source/member witness; the existing `repr128.rs` runtime remains the
projected-bound integration witness. Do not write an assertion/primitive-specific
workaround for its concrete-operand failure.

A function that only owns the reached ParamEnv does not require lowering its
body or manufacturing FnDef/signature/variance facts. Likewise, ordinary trait
applicability must not eagerly require every associated leaf if upstream never
requests it. However, removing the existing publication preflight without a
sound demand mechanism would replace a known preparation gap with an invariant
panic. Preserve that guard until authenticated query-domain/demand admission
replaces it. The worker's variance-query-domain record names the actual upstream
callback and snapshot/opaque cleanup constraints: current bare-value callbacks
cannot suspend directly, and unwinding/reusing a partly mutated inference
context is unsound. These are explicit remaining integration obligations, not
permission for a second solver or invented empty facts.

V503 mechanical-delta review: the validator captured
`0998106d093e229941e497120ac13fe83fd301bba96eae3e58acf18248f6b61f`.
The only four changed source inputs since V502 are the variance generator and
its terms/constraints/mod outputs: actual parent import, associated Interner Pat
type, diagnostic Debug formatting and the inherent Ty trait import. The lead
inspected those joins; no algorithm branch or ordinary source constraint changes.
The eight review conclusions retain their explicit scope/remaining holds.
Actual acceptance still follows the final terminal record, not this review.

Cold cost qualification: each Q79 target retains its completed demanded-domain
result, but separate roots with overlapping nominal closures can rerun the cold
fixed point. Field query Arcs share; a crate-wide shared inference result is not
implemented. No installed replanning or performance/scaling acceptance follows.
