# M018/T040 — actual body alias normalization

Solo root, LOCAL-049 with existing same-owner LOCAL-037/044/046/047 interfaces;
baseline 35f36e738df6d7968a8c7f6c721f4119dfb8dc1c. Unchanged witnesses:
type-alias/static-method-type-alias-11047.rs and structs-enums/struct-aliases.rs.
Their actual mains fail at lexical-call lowering and constructor TypeContext,
respectively, all editions. These failures remain blockers until main completes.

Invariant: an alias's target is resolved once by the ordinary source Type/alias
service in its actual declaration scope. Neither local primitive-only token
handling, fake NameDefinition unit/edition fields nor method/constructor name
rewrites may select its semantics. Alias normalization must preserve source
generic identities, actual target source lookup and caller namespace precedence.

Actual routes: entered body ParseFile/BodyIndex -> SourceLocalPlan/LocalTypePrefix
-> body_type_declarations -> AliasContext; signature/named Type consumers ->
AliasNormalizer -> TypeAlias/DeclarationGenerics retained actual target AST ->
WrittenTypeDeclaration/resolve_actual_type -> mixed Substitution. Runtime
constructors use bind_prepared_record_constructor and ordinary SchemaNames;
associated calls use EnteredValue root selection and ordinary associated_of.
Thunk/verifier/Frame continuation/cleanup remain their ordinary owners.

First prerequisite: generalize the existing checked local nominal ownership
join into a shared local Type-declaration owner receipt (including aliases),
keeping the nominal wrapper restricted to its old kinds. Reuse actual owning
body/publication/plan/source/header/name/edition checks, not scope owner bits.
AliasNormalizer consumes that same real declaration join when a selected alias
belongs to an entered body; keep cold declaration receipts and module/generated
receipts on the same normalization loop. Remove the unconditional local alias
refusal only where the checked owning receipt replaces it.

Extract the existing selected-definition finalizer from prepare_selected so a
real opaque body prefix can pass its selected identity without manufacturing
NameDefinition fields. Replace SourceLocalPlan's primitive/one-token shadow
heuristic with that common service; delete superseded token/primitive routes.
Allow the retained target AST service to use a local declaration only through
the actual owning receipt. Generic aliases remain explicit nonclosed templates/
pending until the ordinary argument/constructor/call constraints bind them; do
not invent a closed Type or fresh runtime descriptor from an alias name.

Then connect the resulting target to existing constructor/member consumers,
preserving alias argument-to-target relationships rather than forwarding alias
arguments positionally or re-spelling the target path. Keep the syntax guard
until an actual entered rooted-call receipt authorizes the common call recipe.
This prerequisite alone cannot close aliases, calls, constructors or M018.

Costs: immutable parser/body plans stay shared; existing local declaration-row
lookup/currentness is cold. No parser/evaluator, body clone, installed namespace
census or new resource cap/accounting/static checker. Real input/query/source/
epoch dependencies must flow to caller memos. Existing alias target walks in
legacy nonmemo callers remain named retention debt, not a speed claim.

Evidence: actual original/emit/wrapper source in all editions; module/local,
primitive/nominal/generic aliases; shadowing/import and stale/wrong/no owner
controls; no executable callee-body preparation during target normalization.
Use unchanged upstream originals, adaptations outside them and separate source/
runtime outcomes. Audit MA003/MA006 context-dependent selection and all eight
recurring failure modes; do not claim MA001/MA002/MA004/MA005/MA007/MA008 repairs.
Exact source review and asynchronous terminals precede checkpointing. No new
stock pass or task/milestone/JIT acceptance from a helper or partial execution.

V391-V393 exposed and preserved two control setup mistakes (missing actual body
publication, then misuse of an impl-catalog seal for entry selection). With the
real main BodyHandle and coordinator local-plan publication, Ham resolves but
StringVec remains deferred: resolve_declaration_name_at only consults cold
DeclarationNames for local header occurrences. It does not join the already
entered owner's ordinary header prefix. Extend DeclarationFieldTypes' existing
header identity/generic metadata to aliases; use the parser's actual
DeclarationTypeName receipt and the retained HeaderItem to enter the same
SourceLocalAdapter lookup_prefix_inner used by fields/parameters. No new Type
walk, reparsing, module fallback or alias-only name rules. Extract prefix
construction from prepare_header_name; preserve its existing finalizer and Self
handling. A generic source name and an actual module tail remain distinct.
This is cold declaration preparation; installed execution does not use it.

Separate source review found that the entered-name fallback also serves named
function/constant/trait headers. Keep the canonical checked owner kernel general
for those named declarations; retain Type-only and Struct/Enum-only wrappers.
This avoids making the new name join reject an existing non-Type header.

V395 passes7/8 controls but preserves a genuine explicit-Type regression:
`type u8=u16;callee::<u8>()` still enters the normalizer's single-token target
branch, which uses the old occurrence-free local name API and returns owner
pending. Remove that affected token-only lookup. Factor the existing retained
TypeAlias/DeclarationGenerics target join so both simple-name chain resolution
and compound target lowering consume the same real AST/header context. Simple
chains continue through normalize_traced's existing cycle loop; their actual
NameRef goes through resolve_declaration_name_at and the common header prefix.
Do not restore the deleted primitive shadow heuristic or recursively bypass
cycle detection through a second alias evaluator.
