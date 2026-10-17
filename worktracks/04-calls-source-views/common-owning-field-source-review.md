# Source review: owning local headers, field Types and cold Type entry

October 16, 2026; read-only review of root integration. No Rust/Cargo/native
execution. Exact reviewed component boundaries:

- `common-owning-field-header-integration-v1/COMPONENT.json`,
  SHA256 `16aff70bf85f7214963900acb8449b82b7bd0bfd8881ca336bd6fbb9061418e3`, source
  `c950209c0752f3aab583e744fee202b8de257ef66df844dbc4a68abe9edcc38e`
  (see immutable component for exact fingerprint; the full value there is authoritative).
- `common-local-declaration-owner-v1/COMPONENT.json`, SHA256
  `854bf5b0f9122cf6096920a17731204262f64702f6405e1459d7f7a195cbf20b`,
  source `17f855ff75fc14d19c5e534c01c3dfd32841f2896c29b1c3f7390ca122459660`.
- `common-genuine-cold-type-entry-v1/COMPONENT.json`, SHA256
  `993d1b6abce5451dc875d246eb7698dc1eea9129a1e1e0906f7012013aac5419`,
  source `e196a876c47697aefc5d98091e9574fa2c44f40e24daf7bd559192e809ec6ef9`.
- `2026-10-16-declaration-field-type-prefix.json`, SHA256
  `0a0ec2bbf56384dc6392461a51f67e9418efaf63d8a4cfb3a99a6d359e1c00ac`,
  source `5ca319c43fede4149f539a57f2a5f4220347350b6ba9ca7b0ca133fc65bc9347`.

The primary runtime witness remains the complete unchanged official
`borrowck-assign-to-subfield.rs`: local A contains B, nested construction,
`p.a = 2`, then `p.w.a = 2`. `module-qualified-struct-destructure.rs` supplies
the separate `m::S` regression. Imports/component controls are not stock passes.

## Concrete findings and shared authority

`Database::prepare_source_local_plan` now cold-registers actual producer-free
entered declaration metadata through `publish_local_declaration`; attributed
rows retain their producer. This does not admit executable bodies or TypeSchema,
resolve names, or close a scope. Each selected publication is atomic/idempotent;
the entire inventory loop is not batch atomic. Earlier valid metadata may remain
when a later row fails. The public per-item service validates actual current
context, exact parser item range/header tokens, source/profile/module and actual
metadata rows before mutation, with checked peak budget and revision.

The first source review found `local_nominal_owner` interpreting a lexical rib
`scope.owner` as a function definition. `body_items` derives that rib from its
own stable scope domain; no inversion is legitimate. Owner7 correctly replaces
this with private derived `LocalDeclarationOwner(definition) -> actual BodyKey`
from opaque PreparedBodyContext. Tag33, key/value matching/encoding, fixed-size
metadata accounting, base-publication refusal and input-universe retirement are
present. Its consumer reads the owner dependency, exact real DefinitionBody,
current SourceLocalPlan/environment and canonical item/source/header/context,
then actual declaration NameRef token edition. No fatal source issue was found
in that bounded join. The owner locator grants no use-name authority.

The same review exposed a second composition bug: registering local function
headers leaves three of LexicalAdmission's four derived rows, which the old
`publish_lexical_function` rejects as partial publication. The separately frozen
`2026-10-16-lexical-header-admission.json` adds the exact genuine-owner/three-row
join; it is not part of the earlier root6/owner7 evidence. Its controls are still
uncompiled/unexecuted. No broad partial-count acceptance is sound.

`SchemaNames::from_body_type` checks the retained plan and actual declaration
currentness before using the selected own identity. `from_current_database`
uses the common canonical local owner join for value projection. The same
`from_definition` checks current source/profile/header/context and complete
TypeSchema equality. Both use the existing structural/privacy/member binder.
`current_name` uses own-name proof only for None(field), and uses the actual
FieldRecipe occurrence for Some(field), even for equal-spelled names. The latter
calls `plan.prepare_field_type` and the common local/generic/module prefix. It
never fabricates a FunctionSignature or runtime TypeId proof. Concrete field
SourceType does not require an invented NameDefinition; deferred names remain
deferred and aliases retain their typed prior gap.

Primitive field absence needs a genuine Type namespace completion; Value
completion is insufficient. `cli/type_preparation::prepare` obtains that real
cold Type closure using existing compiler preparation and source/discovery
publication before Value/main entry. It preserves explicit standalone proc
refusal and actual adopted effect records, and retains the real Type discovery.
No TypeScopeReceipt domain or global currentness guard is weakened. Separate
Type/Value cold declarative discovery/graphs can still duplicate work. Existing
exact adopted ExpansionRecord checks prevent the helper from authorizing a
second effect itself; exact-once acceptance requires actual execution evidence.

## Nested local functions need actual reached admission

A local item inside a local function belongs to that function's actual entered
body, not the outer main body's facts. Current LexicalAdmission genuinely
certifies selected original numeric local callables; it validates an actual
module-outline caller and retains LexicalFunctionPlan. `source_body_context::capture`
still accepts original executable owners from module outlines, generated owners
from genuine generated maps and retained owners from actual assembly anchors.
It has no selected lexical-body receipt branch. Therefore it cannot yet prepare
a nested local function's BodyEnvironment from these module-only authorities.
The correct next join must retain actual reached lexical/source admission,
current parent environment and the real local BodyHandle/source/hygiene/scope;
reading a rib, manufacturing a BodyKey or eagerly admitting every local function
would be invalid. Owner7 intentionally does not claim this admission.

## All eight recurring modes (not MA numbering)

1. Header publication/Type callback only supplies ordinary source authority;
   no assertion/expression compiler or evaluator. MA001/MA007 wider removals not
   established by this review.
2. Actual ordinary constructor/field thunks and runtime storage remain the
   consumer. No literal/break generated executor; MA004/MA005 wider holds remain.
3. Existing selected schema/member/Type services and exact field occurrence
   determine field semantics. No macro or fixture name selects method/numeric
   behavior. MA003/MA006 broader obligations remain.
4. Body-item canonical publication and field occurrence kernel are shared for
   actual original/retained/generated contexts. Missing genuine generated or
   nested-owner receipts remain typed holds. No full four-origin runtime matrix
   has passed; owner7 does not prove it.
5. Cold declaration headers are metadata, not execution/lowering of local
   function bodies or unselected arms. Attribute producers are not bypassed.
   Pattern code and MA002 are not modified here.
6. Root6 coordinator registration is cold, not a query-provider side effect or
   per-op preparation. It scans I items and calls a service that scans those
   same I items, giving at least O(I²) inventory selection and repeated cap
   accounting. Repeated helper calls also revisit the list. Field proof checks
   can scan F schema fields per field, O(F²) across a bind. Existing installed
   constructor repeated binding is a separate root migration; no mode6 closure
   or speed claim follows from these receipts.
7. Field Type queries retain real module Type completion and local/generic
   source authority. The held global TypeScopeReceipt world guard has not been
   copied into hot lookups or weakened. Existing NameNamespaces construction,
   global lexical census and exact relevant footprint migration remain debts.
   Owner7 adds one relevant derived input and no global scan.
8. Parser/index/source plans retain actual shared Arc roots. Owner7 owns only
   value BodyKeys, so needs no new registry. Existing db-fetched Outline weak
   pin/retirement mechanism already accounts retained inner Arcs; no second
   Outline registry is proposed. Selected declaration syntax uses exact-range
   lookup. Cold repeated item/schema list selection remains the explicit cost
   above; no full-file executable search or BodyIndex deep copy is added.

## Candidate and evidence limits

The last actual reported frozen236V2 had 12 host controls pass but zero actual
main passes: qualified m::S failed after two dispatches and local A/B failed
preentry normalized header attachment comparison. These failures remain attached
to their old source; neighboring component passes do not close them. Root's
normalized-header repair, genuine Type-entry3, field5/consumer and owner7 required
one coherent compiled/runtime candidate at the initial source review. The actual
subsequent candidate outcome is recorded below. Qualified Type paths are independently
being repaired by the pattern peer; no old-constructor fallback on refusal is
permitted. Exact lexical publication compatibility is independently frozen for
its own controls. Full Box/library/standard-macro closure, suspension/cleanup/
invalidation/exact-once matrix and architecture milestone remain unexecuted or
held where not backed by actual validator terminals.

## Subsequent actual validator result (original source review retained)

The sole validator has now compiled exact source member
`a23a4364f3943f3d47872b4125b1ca3e7c359c3c37c673bdde74a196c9e5a445`
successfully (`c374c3/0`, 13 warnings). Permanent evidence:
[local owner runtime validation](../evidence/2026-10-16-local-owner-runtime-validation.json).
Eight distinct selected bodies ran: seven passed, one failed. The passing scope
includes the actual owner publication control, original/all-edition and genuine
generated/wrapper field prefix controls, revoked/substituted field authority,
genuine Type completion/entry guards and current published owning plan reuse.
These specific components are no longer unexecuted on that exact source.

The complete unchanged A/B main failed first in Rust2015 with
`LRR-E-UNSUPPORTED: cleanup requires its source-selected Drop witness` at retained
PC `e1a8b028809dcd71f6b7b11ad396ebfddb604a9f7481c3469f77c0adc979c6a7`.
Completion/opcode assertions were not reached; 2018/2021/2024 main runs remain
unexecuted. The full program contract failed: zero runtime-contract/full-fixture
passes. Controls terminal `a0e69b/1`; release SHA256
`2a7f00c98667d97c2940bd7e2a9a77ff4ee5ee1c00327efd3eaa9cef0dfddbfd`.
No whole-main success or cleanup acceptance is inferred from reaching this next
real common-engine prerequisite. Strict/milestone/qualified/fullcore reruns and
lexical2 remain unexecuted in this evidence. The source review and failed earlier
sources keep their original fingerprints and outcomes.
