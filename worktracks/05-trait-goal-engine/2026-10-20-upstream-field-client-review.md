# M020 field/client source review

Baseline a3e36397f8dd26c0bc5797d7e42908bce0726a26. Reviewed V494 fingerprint
2c6df57f071480fb9d412c5dea184e089eebfe7b64223633101856125c0952c6.
Lead review and independent wf_validation_review source inspection. Runtime
terminals and closing guards are separate evidence; no complete solver or
interpreter milestone follows from a compiling component.

Common entries are SyntaxProvider::AdtSourceFields ->
PreparedAdtSourceFields::prepare -> SchemaNames::source_variant_fields ->
SourceBindings/SourceGenericLayouts -> source_terms::encode_declared ->
EarlyBinder fields -> Context retained immutable slices. The completed memo
capture carries its full PreparedInputGuard, including field source/generic/
lookup dependencies. Actual publication/root owner must require it current
and retain one typed result per context; neither that owner nor complete ADT
facts are implemented by this source-plan component.

PreparedProvider::field_representing_type_info now calls the complete pinned
AdtDefinition::pinned_field_representing_type_info. Clients.reflective_field is
removed. Exact FieldRepresentingType definition identity replaces rustc's flag
representation; world/actual ADT checks are structural only. The real lang-item
producer must establish actual source attributes and confirmed absence before
Option::None has semantic meaning. No name/argument-shape heuristic is used.
ReflectionSome branches remain unexecuted with no invented complete provider.

The compiler Value u32 ordinal, enum name/field binder, tuple indexing and
malformed-input invariants remain in the full registered method. Decimal
symbols use context-owned interned text rather than global compiler Symbol
IDs. tools/upstream_reflection.py selects/hashes the exact original helper and
checks the full registered Context integer-symbol body; the first-ten-static
symbol optimization becomes the same canonical decimal-string result. This
adds O(decimal digits) formatting/interner work during a demanded source goal,
not a guest dispatch parser, and no measured scaling claim.

SolverContext::evaluate_const now uses the full pinned try_evaluate_const and
its actual replacement/typing/query helpers; SolverClients.evaluate_const is
removed. PreparedConstEvalInputs holds typed terminal query and owner ParamEnv
results. An absent key remains a distinct ConstEvalDependency; it never turns
into semantic None/NoSolution. The delegate treats an unadmitted missing input
as an integration invariant failure. Actual query result provenance, admission/
suspension/continuation, complete factory and upstream root invocation remain
unimplemented. No terminal query executes a compiler or a guest initializer.
Reached guest const/static initialization remains owned by the ordinary VM.

## Eight recurring modes

1. All three components prepare source/TypeIR or compiler goals; no assertion
   operands, synthetic executable body, expression evaluator or macro identity.
2. Full pinned const/reflection algorithms retain their upstream branches;
   supported source field shapes use the same ordinary type service regardless
   of origin. Unsupported projections stay explicit pending, not a new scalar
   output language or accepted empty field list.
3. Field source Type/generic identity, shared binder substitution and exact
   lang-item definition IDs are used. No method-name whitelist/numeric hint.
4. Handwritten, user-emitted and wrapper-emitted declarations join TypeSchema
   and SchemaNames. Standard-macro item nesting, full reflection origin matrix
   and complete root answers are unexecuted; no inference from nearby cases.
5. Only demanded declaration/field Type IR is prepared. Controls explicitly
   reject FunctionSignature/BodyStructure/Thunk/PatternPlan work; no pattern
   engine is duplicated and no executable branch/body is selected or checked.
6. AdtSourceFields is one pure immutable memo (tag74). Materialize/encode is
   cold owner preparation; callers must retain its result, not re-enter it in
   callbacks/installed operations. Retained interning is not that owner cache.
   Const and reflection callbacks do no IO/source parsing/plan construction.
7. Existing per-field membership checks make source gathering O(F squared)
   cold work; source/generic/query dependency closure is retained. Callback
   reads use keyed immutable metadata and selected ordinals, never a namespace
   census. Complete relevant-version publication/absence handling remains held.
8. Context owns field/variant Vecs and canonical name Strings in safe pools;
   returned slices/Symbols and existing parsed/query Arcs are retained handles.
   No BodyIndex deep copy, full-file per-operation search, unsafe or leaked
   string is introduced. There is no tuple/field count or memory cap.

The preentry loader's source trace still forbids transitive TypeSchema before
entry. The new field control runs after actual catalog preparation and cannot
prove preentry/root field admission. GenericArray and Pair/U controls cover
source metadata transport; they do not execute unchanged main. MaybeItem's
negative admits several explicit preparation gaps; without its exact enum
logged, aggregate pending counts cannot prove ProjectionMetadata was reached.

Existing custom interpreter resolution remains an alternate route held for
replacement. MA001–MA008 are separate audit IDs, not the eight mode numbers
above. No legacy macro architecture obligation or release gate is closed.
