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
