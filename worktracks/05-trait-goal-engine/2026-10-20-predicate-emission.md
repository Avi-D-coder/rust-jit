# M020 authentic transient predicate emission

Before-code record, LOCAL-049 / WT05-02. During V521 source freeze this is a
read-only design; no producer source, fixture, tool, registration or test edits.
After explicit ownership return the assigned slice is only new
`traits/upstream/predicate_emission.rs` and this record. Lead owns all consumer
integration and registration; `SourceBindings` remains unchanged.

Invariant: actual positive `PointeeSized` bounds affect source default-bound
policy but do not become compiler IR clauses. Exact pin
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`,
`rustc_hir_analysis/src/hir_ty_lowering/mod.rs:983–1002` selects a transient
scratch vector by authenticated lang identity, after which the same lowering
continues and the vector is discarded. No validation is bypassed by returning
early. This is source-to-IR policy, not a trait proof or independent solver.

Unchanged primary witness `enum-discriminant/repr128.rs` requires actual
PartialEq whose source header declares Self and Rhs PointeeSized bounds. The
already imported run-pass `dyn-compatibility/pointeesized.rs` explicitly
witnesses removal before the solver and has a real TypeId equality runtime
contract. SHA256 `d17c1542edd487ecb0a9e84d35045f2c4ee838c298677454701046bc67e9d617`.
It exercises dyn lowering too; this clause component cannot claim that whole
fixture. Existing imported `traits/core-marker-name-shadowing-issue-2284.rs`
declares ordinary user `Send` (line12), used by `f<T:Send>` (line16), and must
retain an emitted ordinary bound with Q76 Known(None). SHA256
`947a164cfa2d31cac735e0bae09d5dd4fb5830e0b0917bbaf906463b61936f62`.
Its empty main is only a source-role control opportunity, not runtime evidence
for this producer. Neither fixture was changed or executed here.

## Exact consumer joins

| Plan / call site | Retained addition and build order |
| --- | --- |
| Q78 `frontend/declaration_source_predicates.rs::PreparedDeclarationSourcePredicates` | Preserve written/default recipes, ActualBoundSite and parameter/parent provenance. No emission filtering or Q76 lookup here is necessary. |
| Q80 `source_declaration_clauses.rs::PreparedSourceDeclarationClauses::prepare/encode` | After `require_recipe` and actual `SourceBindings::prepare`, prepare emission sidecar over `source().recipes()`. Retain it beside `bindings`; pass it to `encode_recipe` at current line185. Q81 hierarchy inherits encoded results without a second policy. |
| Ordinary impl `source_predicates.rs::PreparedImplSourcePredicates::prepare/encode` | After actual bindings, prepare sidecar over real ImplPredicates requirements, retaining it beside `requirements`/`bindings`; current line208 calls the same `encode_recipe`. No synthetic declaration. |
| Q88 `source_trait_clauses.rs::PreparedSourceTraitClauses::prepare/encode` | After source defaults/order and binding, retain sidecar over recipes. Current line445 calls common encoder. Build `positive_dependencies` only from positive rows whose checked emission decision is Emit. Retain raw transient recipes and source Pending rows separately. |
| Q89 `source_impl_outlives.rs::prepare` | Current line256 follows Q88 `positive_dependencies`; it must see emitted semantic dependencies, not discarded PointeeSized rows. Full encoded ClauseKind inspection and typed requirements remain unchanged. |
| Q82 `frontend/source_item_bounds.rs` | Keep written/default recipes and parent candidates separate. No production item-bounds encoder currently exists; that gap stays held. Existing Q84 cfg observer (`source_default_bounds.rs:926`) prepares a sidecar before its common `encode_recipe` call. Future production item-bounds encoding uses the identical sidecar/encoder boundary. |

The only production `encode_recipe` callers are the three above; the fourth
call is the Q84 cfg observer. `source_terms::encode_predicate` stays an actual
predicate constructor; transient *output publication* belongs to the optional
clause boundary, not inside Type/alias binding or numeric inference.

## Concrete prepared interface and availability

`PreparedClauseEmission::prepare(db, catalog: Arc<InputValue>,
recipes: impl IntoIterator<Item=&SourcePredicateRecipe>) -> QueryPoll<Self>`
validates the actual catalog, gathers distinct positive actual trait IDs and
fetches Q76 `SourceLangIdentity(DefinitionHeaderKey{inventory,definition})` for
each. Retain catalog and every Q76 Arc, plus checked per-ID
Emit / TransientPositivePointeeSized / Pending decisions. A successful actual
classification of another role or None means Emit; classification failure is
typed pending. No missing map entry means Emit. Ordinary shadow names thus do
not select compiler policy. Classify resolved defaults as genuine positive
trait references too; unresolved legacy defaults and written Pending stay held.

`require/decision(actual_trait_id)` returns a checked decision or typed missing
identity/capture reason. `apply_encoded(actual_trait_id,(Clause,Span))` checks
the encoded positive trait identity/world and returns Some for Emit or None
for transient; it performs no queries. Lead's `encode_recipe` first performs
the existing full real predicate/argument encoding, propagating unsupported
Types/aliases/regions/pending, and only then discards the transient output.
It must not return None before encoding or erase pending written arguments.
Relaxed-source handling and unsupported negative/qualified bounds retain their
existing explicit availability contracts; they cannot become positive clauses.

Stable encoding includes inventory/catalog fingerprint, actual per-ID Q76
fingerprints and precise decision/pending tags. Existing enclosing memo guards
retain all nested Q76/receipt inputs. No complete-world lang registry or global
absence proof is needed. Important acyclic order:
owner source plan -> SourceBindings complete -> emission sidecar ->
Q76 -> Q75 -> SourceBindings. Since SourceBindings never reads this sidecar or
Q76, there is no recursive language classification installed in generic/type
construction. Q84 may return before language capture under no-implicit-bounds;
this separate positive-clause policy still needs authentic Q76 and must not
interpret that early return as an empty or ordinary-language classification.

Cost: one memo lookup per distinct positive trait ID on cold plan preparation,
retained Arc roots, and logarithmic sidecar lookup per encoded clause. At solver
entry all clauses/dependencies are frozen; no parsing, source walking, namespace
census or re-preparation occurs in callbacks. No performance measured.

Eight recurring modes source scope: no assertion evaluator; no scalar macro
executor; no contextual methods/numeric rules; original/generated Q76/Q75 joins
remain common; no patterns/cold executable arms; one retained policy per memo,
no hot planning; relevant per-ID dependencies only; no BodyIndex clone or file
search. Historical MA001–MA008 audit debts are not closed by this component.
The real emitted-clause/default/provenance/origin/currentness and root matrix
must be validated against frozen source after implementation. Missing factory
domains, actual pinned elaboration/normalization and repr128 runtime acceptance
remain separate gates; abort policy is unchanged.
