# Register entered declaration headers before execution — LOCAL-024

M014/M017/M018; October 16, 2026. After the real normalized-anchor repair,
unchanged `borrowck-assign-to-subfield.rs` main reaches the constructor and
fails on source `1c6b1fad`. The source-aware review finds that owning-plan
controls explicitly register the selected local declarations, while the CLI
coordinator currently publishes only its body environment/plan. The pure Type
consumer then correctly refuses absent declaration source/header/context rows.

Extend the existing cold `Database::prepare_source_local_plan` coordinator
entry to register the admitted, producer-free local declaration headers using
`publish_local_declaration`. That existing service validates exact immutable
body-item syntax, source/profile/context and currentness, atomically publishes
each three-row header fact, and publishes neither executable bodies nor type
schemas. All declaration kinds use that same service. No names or rustc fixture
identities select behavior. Attribute-owned rows retain their actual producer
and remain pending; registration cannot complete or bypass their transformation.

This registers metadata already captured in the actual entered-body plan. It
does not infer field types, evaluate constants, prepare a branch or enter a
callee. Source queries remain pure; no publication moves into a provider or
VM opcode. The coordinator remains quiescent before guest startup. The loop
is bounded by actual local-item inventory; each existing publication validates
its exact syntax and accounts its rows. No dispatch work, namespace census,
parser/body copies or performance claim is added. Modes 1–8 and MA001–MA008
retain their existing broader holds, especially installed constructor planning.

Change the existing owning-environment control to rely on this actual
coordinator publication instead of manually supplying its missing production
facts. The complete original upstream main remains the runtime witness; any
next field, Type-namespace, pattern or cleanup failure is still a failure.

## Exact local owner dependency (pre-code, same repair)

Source review found `source_types::local_nominal_owner` interpreted
`DeclarationContext.scope.owner` as a definition key. `body_items` instead
constructs a lexical rib digest with `lrr/v2/body-item-scope`; these identities
are intentionally different. The unchanged local A/B main exercises the bad
join on reached field access. Extend the existing atomic selected header
publication with `LocalDeclarationOwner(selected definition) -> actual BodyKey`
from its opaque admitted PreparedBodyContext. Read that exact input in the
canonical owner join, then require the current SourceLocalPlan, current body
DefinitionBody and exact selected row/header/source/context. Delete the guessed
DefinitionBody(rib) lookup. An owner locator alone grants no use/name authority.

Use the existing InputKey/InputValue framework, explicit new stable tag33,
fixed-size checked accounting and ordinary input-universe replacement semantics.
The new row holds only value keys, no parser/body roots requiring a new registry.
Like DefinitionSource/Header/Body/Context, this derived metadata does not change
namespace membership; exclude it from the existing lexical namespace census.
Do not admit it through generic base publish_inputs or generated-discovery row
publication; only the current local declaration service creates it. Existing
coordinator/source-budget/conflict checks remain atomic across all four rows.
No per-operation namespace map, inverse scan, rib decoding, parser or evaluator.

Required controls extend the unchanged-original declaration publication test to
check the actual parent BodyKey, rib != definition, four-row idempotence/conflict
and absence, plus whole A/B main field access/assignments and cleanup. Broader
MA001-MA008 and recurring modes1-8 remain held; installed binding still needs a
separate common query/recipe migration. This is a bounded source-owner repair,
not complete runtime acceptance.
