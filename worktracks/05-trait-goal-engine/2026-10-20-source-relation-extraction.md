# M020 / LOCAL049: first shared unsolved source-relation extraction

Precode analysis during V532 freeze, 2026-10-20. Production implementation is
held until explicit root release. This record proposes a bounded first slice;
it does not claim a complete numeric component or an admitted upstream root.
Primary witness remains unchanged `tests/ui/enum-discriminant/repr128.rs`:
actual `discr<T,U>(T,U)` argument relations connect original numeric input to U,
and its actual Q80 projected PartialEq<U> requirement supplies the numeric
obligation. Section52.2 additionally requires connected later source uses before
fallback without executing those uses or preparing their guest thunks.

The violated invariant is that the ordinary source component has no retained
unsolved relation API. `binding_constraints::Component::solve` mixes metadata
discovery (`inward`, `outward`, `declared`) with `constrain`, candidate collection,
`parameter_bounds`/`trait_bound` and final primitive fallback. `Via`, `open` and
declared Type queries also change which branches can propagate. Exposing
PreparedBindingConstraints or simply renaming Component would expose legacy
semantic answers, not the required common source inputs. Extracting all of that
in one pass would be a broad graph/solver rewrite and is outside this slice.

## Exact first boundary and files

Proposed production ownership after release:

- New `crates/lrr/src/syntax/source_constraint_relations.rs`: immutable source
  relation kinds/rows and construction from already retained SourceFact nodes.
- `crates/lrr/src/syntax/source_constraints.rs`: private submodule declaration,
  one retained relation table constructed during existing
  `SourceConstraintPlan::prepare`, checked source/body identity accessor and
  deterministic stable encoding/schema version update.
- `crates/lrr/src/frontend/binding_constraints.rs`: replace corresponding
  elementary `Component::{inward,outward}` adjacency selection with reads of
  that same table. Existing solve/fallback and unrelated semantic helpers are
  unchanged and remain migration debt.

No db query-key/provider/value expansion is needed: the table belongs to the
existing pure `QueryKey::SourceConstraints(BodyKey)` memo. No engine, root,
source predicate or member-index file is in this slice. The lead owns authentic
numeric-key/argument-NodeId capture in the existing reached-call packet.

Proposed API names are provisional until owner agreement:
`SourceConstraintPlan::relations() -> &SourceRelationPlan`, and plan accessors
for a node's inward relations and its outward consuming relations. A relation
retains source node endpoints/operand ordinals, source/body structure identity
and a typed relation kind. It cannot contain runtime values, inference-owner
handles, selected implementations, resolved primitives or an all-ready flag.
The owner validates the ordinary SourceConstraints memo/guard; this view never
mints declaration, solver-root or VM-commit authority.

Initial actual forms to extract are parenthesis/block value transport, if/match
result relations, numeric literal/suffix source leaves, discarded-value
boundaries and actual let/call/return boundaries. Absence of an else or operand
is an explicit missing/source-shape boundary, not a fabricated equality. Match
arm result transport is a coercion/source requirement; it is not a successful
unification proof. Let initializer-to-pattern and call argument/result rows state
the authentic source-service requirement; they do not resolve patterns or
select a declaration. Ordinary Call callee offset, MethodCall/LangCall actual
operand ordering, Return and Discard distinctions must stay explicit.

Do not flatten `SourceOperator` into a universal equality edge: overloaded
operators require the normal selected operation/member semantics. Prefix,
binary, borrow/deref, tuple/array projections, fields, indexing, closure,
constructor, pattern and generated/hygienic binding dependencies stay typed
service boundaries until their existing source rule is moved, with its exact
projection/coercion conditions, into this common producer. A later increment
may move those rules but must remove the superseded Component branch at the
same time. The table is a partial source plan with explicit holes; it cannot
prove the whole numeric component closed or authorize fallback.

Component remains a legacy consumer: its existing scalar/open/strict Via
conditions decide how it interprets the retained relation rows, and existing
semantic leaf handlers continue to answer its old query. The new upstream
consumer may inspect the same retained source rows and ask ordinary metadata
services; it must send actual relations/obligations to the pinned root owner.
No legacy resolved answer, candidate-cardinality choice or fallback crosses
that boundary. The source table itself contains no worklist deciding primitive
types or selecting trait candidates. Connection discovery is source adjacency,
not another semantic solver.

## Dependencies, cost and next prerequisites

Original and admitted generated source already meet at BodyStructure and
SourceConstraints. Actual EnteredValue/SourceLocalPlan facts still own hygiene
and local identity; spelling rows are filters only. Unexpanded/hidden syntax and
unimplemented boundary families remain typed holes, including generated
argument-node joins not supplied by the ordinary route. There is no new
generated-source exclusion or name whitelist.

SourceConstraintPlan already walks the entered body's indexed metadata once.
Derive relation rows from those existing facts without another AST parse, body
scan or executable preparation. Retain immutable rows once; later phases select
node-local rows. Costs are proportional to recorded source nodes/edges at cold
preparation and node-local relation reads at consumers. No full-world namespace
map or new memory accounting/resource cap is authorized. No measured performance
claim follows from this structural analysis.

Remaining prerequisites are separate: authentic reached numeric-key/source-node
mapping (lead); ordinary hygienic binding/use and selected declaration boundary
resolution; exact source projection/field/pattern transports; demand-scoped Q80
fragment preparation before whole-plan pending; same-owner upstream inference,
callback closure and fulfillment; once-consumed VM response commit. The current
direct-occurrence SourceRuntimeDemands is not silently promoted to complete
future-use closure by this first extraction.

## Required review and evidence

Review all eight architecture modes at the final source hash: no expression
evaluator; no literal/break macro-only execution; no context-specific member or
numeric selection; no origin exclusion; no duplicated pattern engine or cold
guest preparation; one retained plan with old adjacency branches removed;
relevant memo dependencies without full-world dispatch scans; shared immutable
source handles without BodyIndex copies or whole-file node searches.

The parent validator should preserve the existing unchanged
`integer-literal-suffix-inference.rs` source-goal control
`unchanged_suffix_shadowed_bindings_use_actual_later_parameter_source_goals`,
including memo reuse/no Thunk/no foreign callee BodyStructure assertions, and
the existing explicit missing-relation controls. New metadata controls outside
imported originals must inspect that existing BindingConstraints consumes the
same relation table; table leaves contain authentic source identities and typed
holes; if/match discarded/return boundaries retain their actual distinctions;
and generated/wrapper source uses the same table. The unchanged repr128 Q80
control and actual original runtime witness remain separate upstream/root/VM
requirements. No source implementation, Cargo, test or Git command was run for
this frozen precode record.

## Released bounded implementation handoff

V532 became all-terminal and root released production edits after pushed
checkpoint `a7e64f3`. Implemented only the four named production files, plus
root's subsequently granted narrow existing source/constraint control files.
No Cargo, Git, fixture, tool, solver or engine file was changed by this worker.

Actual API: `SourceConstraintPlan::relations() -> &SourceRelationPlan` exposes
the retained table in its existing pure memo. `SourceRelationPlan::require_source`
checks actual BodyKey and structure identity only; the actual memo/currentness
guard is still required. `inward(NodeId)` returns `Option<&SourceRelationInput>`;
`outward(NodeId)` returns `Option<&[SourceRelationUse]>`, so an invalid node is
distinct from a real node with no recorded consumers. Empty consumers never
certify closed constraints. Every outward row retains the exact original
`SourceUse { child, parent, ordinal }` beside a `SourceRelationConsumer` kind.
Input rows retain numeric syntax, pass operands, actual if condition/branch
endpoints, exact match scrutinee/pattern/body operand order, or a typed
`SourceRelationBoundary` with actual original operands. WrittenSuffix contains
the actual source token's parsed suffix, not a legacy inferred primitive answer.

`SourceConstraintPlan::prepare` constructs the table from its existing completed
facts/reverse-use table once, after original indexed capture. Construction checks
each actual parent operand ordinal against the retained child endpoint. Stable
domain is `lrr/v1/source-relation-plan`; enclosing SourceConstraints schema is
now 16, also naturally affecting the existing query-key encoding. No new query
kind/provider/framework or caller-created source plan constructor was added.

Actual ordinary-consumer removals: `Component::inward` now selects numeric source
leaves, Parenthesis/Block pass operands, If result endpoints and Match arm bodies
from retained rows; the corresponding old FactKind branches were removed.
`Component::outward` now consumes retained Pass, IfValue, MatchValue, Discard,
LetInitializer and ReturnValue rows; those old parent-kind/ordinal branches were
removed. The actual remaining SourceUse boundary still enters the existing
ordinary declaration/operator/field/pattern handler. Match scrutinee-to-pattern
and pattern interpretations, operators, nominal projections, call signatures,
hygienic lookups and the legacy primitive/bound/fallback paths are explicit
remaining boundaries/debt. No upstream consumer is activated in this slice.

The separate behavior-preservation source pass checked existing scalar/open/
strict Via and Item preconditions: static_candidate still precedes the same
numeric/branch processing; bare integer/float families remain distinct; unknown
suffix remains NumericFamily only for scalar input and ExpressionRelation for
a structural input; absent pass/if operands keep the prior typed defer; Match
bodies preserve loose Via and original order. Outward extracted rows preserve
all exact use ordinals, If missing-else deferral, Match result looseness,
initializer pattern entry, discard termination and return goal ownership.
These are source observations, not executed regression results.

Extended existing controls, with no new miniature-language fixture:

- `db::source_constraint_provider::tests::unchanged_suffix_source_constraints_retain_real_call_and_binding_nodes_once`:
  unchanged official suffix source in all four editions; actual table/body/
  structure match, stale structure rejection, invalid-node distinction, every
  exact indexed reverse edge, actual Call operands and typed argument-use hole,
  existing same-Arc/cold assertions, plus genuine PreparedInputGuard currentness
  and fresh source-input withdrawal rejection.
- `db::source_constraint_provider::tests::cold_source_call_shape_does_not_prepare_its_signature_or_executable_body`:
  existing outside-original literal-to-source-call cold branch adaptation now
  inspects exact retained If condition/then/else endpoints; existing no Thunk,
  no FunctionSignature and no foreign BodyStructure assertions remain.
- `unchanged_suffix_shadowed_bindings_use_actual_later_parameter_source_goals`:
  unchanged official source's actual initializer edge and direct memo dependency
  on the shared SourceConstraints fingerprint, with existing resolved source
  answers, memo reuse and no executable preparation assertions preserved.

Recommended parent validation also includes existing
`generated_and_handwritten_numeric_relations_share_one_route`,
`numeric_relations_select_the_connected_concrete_primitive`,
`unanswerable_numeric_relations_stay_explicit_and_never_guess`, and
`closed_numeric_components_take_rustc_integer_and_float_fallback`, followed by
the actual Q80/complete bucket metadata controls. These preserve legacy behavior
while source-only future upstream consumption remains held. No test was run by
this worker. `rustfmt +1.99.0 --edition 2024 --config skip_children=true` on the
new module exited 0; formatter `--emit stdout` to `/dev/null` parsed the other
five modified files with exit 0 without rewriting them. That is syntax/tool
evidence only, not Rust type checking or compilation.

Separate eight-mode source review against the exact fingerprints below:

1. No assertion/expression compiler/evaluator: this is retained indexed metadata
   used by the existing Component, with no source execution or value evaluation.
2. No literal/break macro-only route: original/admitted generated source joins
   existing BodyStructure -> SourceConstraints -> shared table -> same Component.
3. No context-specific method/numeric selection: the table has source leaves and
   typed requirements, no candidate matching, primitive enumeration or default.
   Existing legacy semantic handlers remain explicitly unpromoted migration debt.
4. No generated-origin exclusion: table construction is independent of origin;
   missing ordinary hygiene/signature/projection facts remain typed boundaries.
5. No duplicate patterns or executable-arm checking: Match rows retain source
   endpoints and defer pattern interpretation; cold syntax remains metadata only.
6. Table construction occurs once in the existing memo, then inward/outward read
   node-local rows. Corresponding old elementary adjacency branches were removed;
   no repeated AST parse, source planner or unused parallel producer is added.
7. No namespace/global invalidation path: the same SourceConstraints inputs and
   future ordinary relevant source queries own dependencies. No map census occurs
   in dispatch, installed recipes or this new accessor.
8. No BodyIndex/parser clone or whole-file node search: rows retain direct NodeId
   endpoints from indexed facts. Added immutable metadata is O(recorded nodes +
   edges); consumers share the enclosing memo handle and borrow its table.

Actual source SHA256, held after handoff:
- `syntax/source_constraint_relations.rs`: `2fba490e1c2a9b2c10d7ef9e681e4167e2a438599562fccc8fbe5a50ce3ec1af`.
- `syntax/source_constraints.rs`: `6aa160a5fc7a06a31a78fca98dec9b2998f0c752eb4ab481aecc02bc182abebb`.
- `syntax/mod.rs`: `8a31eef5d65de7fb26c7812b0333f63c77a72a38cdc0d7573123bfcf13e081c7`.
- `frontend/binding_constraints.rs`: `525b69c177a8821507148aff8d33736d9c18622175d207251589060703404dfc`.
- `db/source_constraint_tests.rs`: `d896ad0240eed34860fd90711c744c976d542792dfbac588dd0f4c8fcc8286b1`.
- `frontend/binding_constraint_tests.rs`: `9be11dd56d49235b0ff6586361457d96daf5a1b59970be02e420099d8c4d7809`.

Compilation, updated control terminals, generated/wrapper matrix, complete
future-use component production, normal-root/VM activation and unchanged
repr128 runtime acceptance remain unestablished by this handoff. No milestone,
runtime/JIT, full-fixture or performance gate is closed.

## Independent bounded implementation review

October 20, before any next validation freeze. No actionable source issue found. Read-only per-file stamps: new `syntax/source_constraint_relations.rs` `2fba490e1c2a9b2c10d7ef9e681e4167e2a438599562fccc8fbe5a50ce3ec1af`; `syntax/source_constraints.rs` `6aa160a5fc7a06a31a78fca98dec9b2998f0c752eb4ab481aecc02bc182abebb`; `frontend/binding_constraints.rs` `525b69c177a8821507148aff8d33736d9c18622175d207251589060703404dfc`; `db/source_constraint_tests.rs` `d896ad0240eed34860fd90711c744c976d542792dfbac588dd0f4c8fcc8286b1`; `frontend/binding_constraint_tests.rs` `9be11dd56d49235b0ff6586361457d96daf5a1b59970be02e420099d8c4d7809`; registration `syntax/mod.rs` `8a31eef5d65de7fb26c7812b0333f63c77a72a38cdc0d7573123bfcf13e081c7`. This is not an aggregate freeze receipt. No Cargo or test body ran; build/mechanical correctness remains unvalidated.

Compared the moved Component inward/outward branches to the actual retained producer rows. Numeric scalar/open family handling, non-scalar suffix ExpressionRelation hold and missing-suffix NumericFamily hold preserve the prior consumer behavior. Parenthesis/block transport, If then/else result transport, Match even body ordinals with loose Via, initializer ordinal1 to its actual pattern, Return and Discard retain the original distinctions. Corresponding old adjacency branches are removed from those consumers. Operators, calls, methods, LangCalls, names, patterns and structural projections keep their existing semantic services; the table records explicit boundary metadata rather than supplying an inferred answer. Existing legacy solve/constrain/fallback remains named migration debt.

SourceRelationPlan is constructed once from existing facts/uses inside the same SourceConstraints memo. Every outward row retains the real child/parent/ordinal and validates parent.operand(ordinal)==child. Node-local access distinguishes an invalid node from no recorded uses, and owner/body structure must match. Some(empty) has no closure implication. Schema16 encodes the actual immutable relation table with explicit tags, endpoints and source owner, so legacy BindingConstraints depends on the changed shared memo. No root, candidate decision, runtime value, inference owner, fallback permission or all-ready accessor exists in the producer.

Eight-mode assessment: (1) no alternate expression evaluator; (2) no macro-only execution or expanded literal vocabulary; (3) no selected primitive/member/trait semantics moved into this table; (4) ordinary/generated metadata shares SourceConstraints, while unsupported services remain explicit boundaries; (5) no new pattern executor or cold executable preparation; (6) one retained table replaces corresponding repeated consumer adjacency interpretation, without another AST parse; (7) source/body/query dependency scope remains actual and no world namespace census is added; (8) node/edge metadata is derived once from retained facts, with no BodyIndex deep copy or whole-file search per consumer. Cold table construction/storage is proportional to actual source nodes/edges; reads are node-local. No measured performance claim or new resource cap is introduced.

The extended unchanged integer-literal-suffix-inference control inspects genuine exact call edges, Call boundary kinds and operands, initializer consumer rows, actual SourceConstraints dependency fingerprint, memo reuse, foreign-body/Thunk coldness and source withdrawal; invalid node/structure checks stay separate. Existing cold If control compares actual retained endpoints without demanding signatures or guest thunks. These assertions were source-reviewed only. No new run receipt exists. Generated/wrapper, distinct repeated-key occurrences, Method/LangCall ordering, full Match/Return/discard matrix and actual pre-fallback repr128/root/VM integration remain unexecuted requirements; neighboring V532 passes do not cover this schema16 source.
