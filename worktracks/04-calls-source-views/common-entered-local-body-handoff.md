# Reached local function source context — LOCAL-024

Pre-code bounded record, October16 2026. T032/T033 and M017/M018 common
source/call authority. Primary unchanged witness is official rustc
`numbers-arithmetic/integer-literal-suffix-inference.rs`, whose actual local
`id_i8` is selected at its real parent call node. LRR controls outside the
original inspect source entry/currentness/absence of cold child preparation.
This does not replace or claim that program's full main execution. Actual A/B
main now reaches a separate source-selected Drop prerequisite; it remains a
failed complete runtime contract in the parent's exact evidence.

Invariant: once the ordinary reached call service selects/adopts a lexical
function, its actual entered source BodyIndex must join the current owning
source plan and real lexical outer scope, even though the declaration is not a
module-outline item. No outline fallback, guessed rib, source-location owner,
new evaluator or original-body rewrite is allowed.

Shared entries: existing SourceCallable/LexicalAdmission validates actual selected
source/signature/caller/body; capture_local joins that proof to private derived
LocalDeclarationOwner and the parent's actual current canonical BodyItem. It
retains the real parent PreparedSourceLocalPlan Arc and owner input, exact current
child source/header/context/body/cfg rows, source parser and already-entered
index. Existing AdmittedBodyContext currentness delegates parent authority.
Existing EnteredMacroIndex::child_from_facts supplies the same parent namespace,
textual declaration scope/before anchor, producer ancestry and retained macro
outers. Extend it to accept the actual prepared child source context; do not add
a second lookup kernel. SourceLocalPlan child preparation reuses the parent's
actual lexical-module proof/Outline Arc after the canonical child join rather
than searching module outlines for a local function.

The selected child index is demanded only after actual ordinary callable
selection/entry; parent preparation may register function headers but never
fetches nested BodyStructure, Thunk or executable recipes. No lexical admission
rule is broadened by spelling or origin here. Historical recursive caller/
generated lexical Value selection still needs common occurrence/prefix work.
Missing genuine selected call receipt is a typed refusal, not absence.

Dependencies: genuine current parent BodyEnvironment+SourceLocalPlan, real
LexicalAdmission, atomic actual owner/header metadata, ordinary callee body
publication, current parser/index/cfg/source/origin/epoch and macro environment.
No raw NameLookup/TypeId or public body digest mints this proof. Parent owns
reached no-I/O derived environment publication and engine call-entry integration;
this component must not relax active-run/query/effect guards or count a helper
as installed production admission. Parent/peer own root traversal integration;
expose the actual retained parent Arc to the existing accounting collector.

Costs: cold exact canonical item join O(I), selected source/index queries already
belong to actual entry, unchanged AST facts/local-plan preparation once through
existing memo. Share parser/index/Outline and parent macro/local roots. Clone
only bounded module key/scope/source-selection metadata for the child proof;
charge actual capacities. No full graph clone/world census, new registry or
per-operation parent preparation. Currentness checks bounded parent authority
DAG, actual owner and declaration stamps. Strict proper source containment and
checked depth prevent cycles/unbounded ancestry. Root must account both current
and retired child-owned parent roots through existing SourcePlanPins/QueryPin
collectors. Existing global lexical census and repeated resolver debt stays held.

Acceptance needs exact-current positive original and actual retained/generated/
wrapper entry proofs where their ordinary selected lexical authority exists;
wrong owner/body/source/current parent/epoch refuses, no cold nested executable
queries, and actual runtime continuation/cleanup/suspension after parent engine
join. Sole validator runs copied source. No Rust/Cargo/native tests by this
worker; all new controls uncompiled/unexecuted until terminals. All8 review and
exact hashes follow after implementation; no feature/gate acceptance.

## Implemented source/API and guard boundary

`source_body_context::capture_local(db, parent: Arc<PreparedSourceLocalPlan>,
callee: &SourceCallable, entered_index: &Arc<BodyIndex>, entered_pc: &PcKey)
-> QueryPoll<AdmittedBodyContext>` now uses capture's SAME current child
DefinitionSource/Header/Context/Body/Cfg, actual ParseFile and BodyStructure
verification. It retains the canonical query BodyIndex Arc and exact real local
parent/callee metadata instead of asking any module outline/map to contain the
local function. `LocalBodyContext` is private; mint requires opaque lexical
plan/canonical parent row agreement, published current parent environment,
proper source containment, actual derived owner value/fingerprint and bounded
ancestry. Current source admission does not repeat Value lookup or call
`callee.revalidate`'s legacy whole-world census. The ordinary selected-call
contract still owns callable revalidation and execution permission.

The actual parent plan/owner InputValue are retained. The local identity includes
parent environment envelope, parent semantic source identity, actual owner,
canonical declaration/parse/scope/body; it excludes a call-site or global
query/source revision issuance as irrelevant body preparation dependencies.
`AdmittedBodyContext::local_parent()` exposes the actual Arc solely for existing
root accounting and shared scope building. `retained_inputs()` exposes its actual
owner handle. `has_current_inputs` is only a reuse observation; authoritative
checks consume the real current parent plan/environment, owner and existing own
source rows/leases. Generated/retained local source receipts are inherited from
that actual parent, never guessed from diagnostic spans. The actual parent
reference origin/macro hygiene/ancestry/root handles remain authoritative.

`PublishedMacroIndex::enter_prepared_body` recognizes this genuine local parent
and delegates `PreparedSourceLocalPlan::enter_prepared_local_body`. It validates
the actual retained parent environment and uses the single existing
`EnteredMacroIndex::child_from_facts` kernel (factored with an optional real child
PreparedBodyContext). Both adapters retain ordinary Namespace::capture, exact
textual scope/before anchor, macro outers and producer ancestry. The old source
adapter's missing-context refusal remains; a prepared generated parent can use
its genuine retained boundary without pretending it has a module root rib.

`LexicalTypeModule::prepare` uses the actual parent module proof/Outline Arc for
that certified proper child. Only bounded key/scope/source-selection metadata is
cloned, conservatively charged under its existing checked metadata bound. No
local rib is stripped into a fabricated module key. Ordinary nonlocal module
proof preparation remains unchanged. Admitted-body identity stable domain moves
v1 -> v2 to encode this new explicit proof discriminant; source-plan/environment
fingerprints transitively include it.

A separate source review caught recursive guard duplication: the old plan guard
checked context.source, then environment.source again. A retained parent DAG
would double work at each nesting depth. The plan now consumes one exact current
published opaque environment/source identity via
PreparedBodyContext::require_current_in_environment, then the same retained
syntax lease check. Non-environment historical controls retain their original
context guard. No namespace/source/lease condition was dropped or replaced by a
caller boolean. Each parent environment occurs once per chain, rather than an
exponential duplicate traversal. Constant-factor repeated callers remain cold.

## Controls and exact missing integrations

Two new LRR source controls in existing `db/lexical_inputs_tests.rs` use the full
unchanged upstream numeric source. The positive all4-edition control first
prepares only the main environment/header facts, asserts no child BodyStructure
or Thunk, actually selects/adopts id_i8, then fetches its entered index and checks
source admission/shared allocation, identical identity across a real derived
revision, actual nested namespace/module proof and absence of child environment
publication. The negative control checks exact non-entry PC refusal, manufactured
owner and revoked actual parent environment; it does not interpret any refusal
as successful Rust execution. The prior three lexical publication controls are
unchanged except formatting. All five remain uncompiled/unexecuted on this
successor until validator terminals.

Parent owns the new private reached no-I/O BodyEnvironment publication boundary,
existing active-run guard preservation, call-entry use, normal callable contract
validation and interpreter continuation/cleanup. Peer/root must add the narrow
existing Roots::admitted -> plan(local_parent) traversal and actual memo-owner
counts before cache/lifetime acceptance. No new registry is appropriate. Existing
Outline inner-Arc pin/retirement evidence remains applicable; new parent-plan
retention/accounting itself is unexecuted. Current helper/source controls prepare
no published child environment and execute no child operation. They are not
production entry acceptance or whole-stock success.

Historical lexical numeric selection still uses module-outline callers and a
legacy full namespace census. Recursive local Value selection and arbitrary
nested signature Type authority (`type_name_namespace::lookup_name` still asks
module declaration membership) need the same genuine ordinary occurrence/prefix
services. Actual retained/generated/wrapper selected-local proof positives are
unexecuted/held until that existing private admission can supply their real
proof; no public SourceCallable/NameLookup constructor or language whitelist is
introduced here. Full unchanged A/B main remains failed on source-selected Drop
in exact a23a4364 evidence, not repaired by this body-entry component.

## Separate all8 source review

Reviewed after formatting against the exact six-leaf manifest
`worktracks/evidence/2026-10-16-entered-local-body-source.json`.

1. Source admission and shared ordinary call/body preparation only; no assertion
   compiler/evaluator. MA001/MA007 removals remain broader holds.
2. The source proof retains the actual body/index and ordinary continuation;
   no literal/break-only output executor (MA004/MA005 not closed).
3. Private selected source signature/call contracts and existing member service
   remain separate from source admission. No method/numeric/fixture identity
   dispatch or alternate inference (MA003/MA006 not closed).
4. New local branch uses actual current parent facts/receipts, before unrelated
   module maps. No origin-specific bypass or new refusal is introduced. Existing
   generated/retained lexical-selection authority gaps stay explicit and are not
   counted as four-origin acceptance.
5. No pattern change (MA002); child BodyStructure is demanded only after actual
   selection/entry. Cold main/header preparation asserts that sibling/local
   executable indexes and Thunks remain undemanded.
6. The existing pure memo remains the owner of each complete local plan. Capture
   costs O(I + N + H) for canonical row, existing index identity and own facts;
   module metadata copy O(P), and inherited Namespace::capture costs its bounded
   relevant ancestor inventory once. No installed-op planning or per-phase
   reconstruction is added. New parent currentness traversal is linear in
   bounded depth, while existing global lexical admission preparation debt is
   unchanged. Parent engine publication integration is required for reuse.
7. New source receipt does not re-run callable name selection/full census. Exact
   owner/current parent environment and actual source inputs are checked; normal
   selected-call revalidation is unchanged. Existing DiscoverySeal/query guards
   retain their actual relevant/legacy dependencies. No new global graph build
   or weakened Type-absence guard, and no mode7 closure claimed.
8. Actual parser/index/module Outline/source/macro/local roots are shared through
   Arc; no BodyIndex or source-file deep clone, no whole-file node search. Entry
   consumes the existing direct indexed node/current query Arc. Actual retained
   parent allocation needs the queued existing accounting traversal. MA008
   alternate verifier/index architecture is not claimed removed by this slice.

Only rustfmt and source/hash inspection were run by this worker. Source
components, runtime-contract execution, complete fixture execution, diagnostics,
strict/milestone and architecture acceptance remain distinct. Preserve frozen
lexical2/owner7/predecessor evidence and all failed sources unchanged.
