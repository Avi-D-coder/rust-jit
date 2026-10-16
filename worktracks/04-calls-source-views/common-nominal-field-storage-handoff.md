# Common nominal field storage — LOCAL-024

October 16, 2026. T024/T032/M014/M017, under the existing interpreter-first
architecture hold. The unchanged upstream witness is
`tests/ui/borrowck/borrowck-assign-to-subfield.rs` (pinned checkout
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`). Actual production baseline
`sol-common-local-type-production-baseline-v1`, source `a60940cf`, failed at
the outer constructor, before either assignment; no full fixture passes.

Invariant: a declaration-selected nominal field must retain its source type and
schema through construction, address projection, intermediate ownership checks
and reference leases. A runtime descriptor or field spelling cannot supply source
authority. `A.w: B` cannot be treated as a primitive or require an owning read of
`w` merely to address `p.w.a`.

Shared route: `SchemaNames::{bind_variant_with,bind_named_field_with}` and
`source_types` select declaration types; `Frame::prepare_nominal_constructor`
supplies literal constraints; `GuestStore::begin_nominal_construction` uses the
existing typed construction guard and ownership transfer. `project_bound_field`,
`resolve_place_for_use`, and `validate_source_projection_steps` check the same
source-selected field identity along the entire address chain. Extend those
routes, replacing their primitive-only storage assumptions. No separate nested
struct evaluator, macro rule, special test name, or alternate ownership path.

Cold preparation may fetch only the selected nominal schema (not recursively
bind its fields, execute its body, or run trait/build/macro producers). Bound
immutable field types are retained with the existing constructor/member plans;
worker descriptor interning occurs in the existing execution store. Projection
validation checks registered type identity and object/owner/epoch chains, with no
source parsing, namespace scan, or new query during installed execution. Account
owned source-type metadata with checked sizes and encode its stable identity.

Remaining production dependencies are explicitly held: genuine owning-body local
environment publication/memoization and declaration field occurrence authority;
local A/B lookup/publication; generic/compound types; source-selected Drop/no-Drop
and recursive scope cleanup. This storage change must not fabricate those facts
or claim whole-program acceptance. The old original-outline constructor adapter
remains held until its genuine common body environment replacement is integrated.

Acceptance: unchanged upstream main with both assignments and final cleanup,
plus LRR authority controls for same-shaped foreign nominal fields, stale
intermediate owners, source mismatch and exact-once transfer. Reuse existing
Point/field/constructor controls. Compare ordinary/generated/wrapper/standard
contexts through the same route; unexecuted contexts remain gaps. Sole validator
owns Cargo asynchronously on frozen sources. All eight recurring modes and
MA001–MA008 remain open; a separate source review and actual terminals are required
before integrating or closing any architecture/task gate.


## Source review of frozen storage10

Source fingerprint `75ab10f89da0a3c87e2e7df9b5368956c5a875a4b0418cd6fb57943123c95a9a`;
manifest `572caeeefc43d141f92f5447f10a0f4d3976e045b45a655c8aa3543a2928e18a`.
Formatting terminal6115ed/0 is not compilation. This includes the previously
uncompiled constructor source core60073b0a; its exact syntax/constructors.rs is
unchanged, and its existing frontend binder is extended here. Base must be
selected-local-declaration v3/a60940cf, NOT transport194/consumption200 alone,
which lacks body_type_declarations.rs and its owning-plan predecessor.

Separate source review of actual shared functions after freezing:

1. `SchemaNames::current_name` calls the existing `source_types::prepare_selected`.
   There is no assertion AST walker/compiler, synthetic body or runtime evaluator.
2. `prepare_nominal_constructor`, `begin_nominal_construction`, and
   `project_source_field` retain ordinary constructors/places. No generated scalar
   decoder, new opcode or macro-origin grammar is added.
3. Field classification consumes the owning declaration Type lookup, including
   its pending/ambiguous result. No numeric relation or method name depends on a
   macro. Primitive constraints come from exactly the bound field's source type.
4. Original/retained/generated sources share `SchemaNames` and `SourceType`;
   existing missing body environment/declaration occurrence authority is not
   bypassed. Those facts still block body-local A/B, and the runtime matrix is
   unexecuted. No original-only refusal is introduced by this component.
5. No pattern or arm execution/lowering changes. Selecting nominal B captures its
   TypeSchema identity but does not recursively bind B's fields or run its Drop.
6. Source work occurs in the existing dispatcher preparation. Installed storage
   and lease operations consume concrete immutable proofs/TypeIds without source
   planning. Existing constructor local-index reconstruction and binder/query
   memoization gaps remain held; prepare-once acceptance is NOT established.
7. No new namespace map or census is constructed. Existing declaration namespace
   code still uses conservative lexical stamps/catalogue scans, and selected
   nominal/alias preparation scans the supplied catalogue on the cold path. This
   is known debt, not constant-time source dispatch/performance acceptance.
8. BodyIndex/ParsedFile handles remain shared; constructor parser changes use the
   existing direct entered-node/range index. New SourceType payloads own checked
   DefKey/TyTerm metadata, bounded cumulatively before growing the variant plan.
   Runtime SourceFieldProjection stores only a registered worker-local TypeId,
   retained with nominal parent/schema/variant/field facts. Every address/reference
   use rechecks the complete object-owner/location-epoch chain and child TypeId.

Runtime representation remains the existing nested aggregate owning cells and
construction transfer protocol. Descriptor interning is store preparation and
metadata-budgeted, not source inference or a guest effect. Frame mapping, hint
slot and plan-size checks precede interning; failures install no constructor facts.
A failed intern can retain valid monotonic type metadata, without creating an
object/guard/owner or retiring an incoming value.

New external LRR control `nominal_field_storage_tests` uses the entire unchanged
upstream issue-19244 declarations and actual selected NESTED_S literal in all four
editions. It checks current nominal field proof, wrong-typed transfer refusal,
consumed tickets, nested address projections, wrong parent witness and stale full
lease after outer move. Teardown is explicitly raw harness cleanup, NOT a selected
Drop/no-Drop witness. No main/const evaluation/full-fixture pass is inferred.
Required sole-validator compile and nearest constructor/field/store controls are
queued asynchronously. No source in storage10 may change during its validation;
new corrections must form a separate successor packet. All eight recurring modes
and separately numbered MA001–MA008 remain open, with their old routes held.


## Actual first validation and correction

Combined source2216a609 compilation a6d862/1 executed zero tests: existing generated
member assertion needed Option wrapping; reference-return validation still read
primitive projection data; peer's contextual test needed mutable DB. Frozen
API-fix2 replaces reference-return validation with the SAME source-selected field
TypeId check and updates only the old member assertion. Manifest
`0dc89430c6a9bdde2856305fed05c167c2e37f1cb47d98c87b65e5a3b1d04f18` includes
an exact a609-derived test leaf, excluding unrelated live successor controls.

Corrected combined source854f56cd compiles342448/0 with17warnings. New nested field
storage control ee91e9/0 passes all4 edition iterations. Nearest15 constructor,
field, reference-result and store regressions pass0d8fee/0. Old actual opaque
module/declaration positive passes917cf7/0. These17 current controls overlap no
other bank counts. Contextual setup05a012/1 and actual whole hygiene mainf787c0/1
still fail; latter is preentry blockedassert_eq in first2015, othereditions not
entered. No scope cleanup/fullmain/stock/task/architecture/milestone acceptance
is inferred from raw harness teardown or component tests.
