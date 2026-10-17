# Genuine generated-child declaration closure — LOCAL-024

Read-only preimplementation handoff under E-R5-005/Section57. Worker owns this
new document only, no production edits, Cargo, imports or runtime acceptance.
Root owns the direct item impl bridge and subsequent child integration. Audit
MA003/MA004/MA006/MA008 are distinct from the eight recurring failure modes.

Current implementation witness: frozen V8 `7d239fbc` passes the complete stock
ordinary and arbitrary-emitter program in all four editions, plus three
authority controls. Its wrapper actually fails before interpreter entry with
`generated child requires further declaration discovery` (`8cf446/1`). The
earlier adapter E0753 failure below is historical; the corrected exact source
has separate Rust compiler metadata evidence and successful direct-emitter
interpreter execution. This repair now targets that actual wrapper failure.

Root integration retains the exact original record-to-declaration-job relation,
captures each child's same immutable active projection and actual parent item
record, and uses the existing recursive completed declaration fragment. Prepare
the ancestor output/scan/record/header indexes once, on demand, and share them
between child waiters. Never rebuild the complete recursive output index for
each child. Child jobs are enqueued by the catalog's completion update; the
catalog does not wait on those jobs, preserving declaration closure ordering.

V9 `9f0a2a5f` compiles, then the whole stock wrapper fails strict database
publication (`6590cf/1`) with `discovery certificate dependencies differ from
complete publication`. The child scheduler completion incorrectly contributed
its internal waiter fingerprint as if it were a separately published input.
`Database::publish_discovery` reconstructs the actual completed fragment's
canonical `(root invocation, root output fingerprint)` dependency, plus every
actual expansion record, source and cfg dependency. Reuse that existing
canonical fragment dependency after the same exact completed-Arc, parent-chain,
projection and inventory validation; the direct impl completion handler already
uses it. The waiter is an internal derived proof, not a new external input.
Do not weaken certificate equality, add an unvalidated dependency whitelist or
drop source/record/cfg checks. Preserve the V9 failed receipt and recheck this
publication join in the exact successor review.

The invariant: a generated item-position call must keep its real declaration
producer/frontiers until the SAME recursive declaration closure has admitted
its actual nested syntax and inventories. Moving a source from an arbitrary
user item macro into an arbitrary wrapper must not select another parser,
evaluator, name rule, or original-source identity. A syntax/header fact or
matching impl count does not establish namespace/impl completeness.

Primary unchanged upstream witness is the complete run-pass
tests/ui/resolve/type-param-local-var-shadowing.rs: cold generic Foo/Box and
impl<Bar> syntax, reached main outer let f, inner fn f() and f(). The outside
original controls emit the same complete payload through user/wrapper item
macros, preserving root attrs/docs at their proper source location. Root is
separately repairing the test adapter's rustc E0753 doc placement; that compiler
reference failure is not evidence of a production runtime error. No execution
terminal was collected by this reviewer. The generated-child refusal below is
source-confirmed and root-reported as the next wrapper prerequisite.

## Actual current boundary

SourceDiscovery::admit_expansion for an original item call already creates
GeneratedItems::declaration_facet -> GeneratedDeclarations::admit_item_output.
The facet binds the genuine original scan producer, actual adopted record,
source/cfg/grammar/epoch/context. It registers deterministic v2
item-declaration-job(output fingerprint), nested_jobs/item_declaration_inputs,
and the existing ScanNestedDeclarations work queue. poll_item_declarations
waits for a live generated catalog fragment to finish before obtaining
provider.completed_root_catalog and starting DeclarationFragment::from_item_output
with_completed_item_owner. No callee executable body is prepared here.

By contrast admit_generated_expansion validates exact pending request/key and
external compiler/procedural adoption authority, admits GeneratedItems, obtains
the actual parent catalog/active child, pending_scope/position, source base,
creates child_for_discovery and GeneratedHeaderPlan::prepare_catalog. It
recognizes calls/outward macro identities, imports and external module demands.
Any other blocked id triggers “generated child requires further declaration
discovery”. It does not register a child declaration completion waiter. For a
wrapper child containing the unchanged impl, this is the common missing join;
removing that refusal alone would drop genuine declaration authority.

The moving root bridge now exposes GeneratedHeaderPlan::impl_frontier, a
dedicated stable record/scope/epoch identity. Impl-owned scopes keep that
frontier; unsupported imports/other owner scopes keep their separate existing
blockers. This distinction is required for child integration too. Never retire
the expansion invocation blanket blocker merely because some impl headers were
found. Root direct-item completion checks actual completed inventory header
identities before releasing the named impl frontier. Its exact frozen candidate
review is pending; observations here are source-only against the hashes below.

## Existing shared recursive mechanism

GeneratedMacroCatalog::complete_child retains child route metadata and the
actual child catalog/record in selection_children/completed.
DeclarationFragment::poll_with_namespaces already asks the completed item owner
for completed_declaration_call(frame.output(), declarationProducerIdentity).
That helper finds the genuine matching parent catalog by output record identity,
requires actual cfg/record equality, and correlates two admitted materializations
through call site plus structural occurrence ordering and full occurrence
coverage. It checks record invocation and token edition against actual
GeneratedItems call capture, then uses GeneratedDeclarations::admit_completed_item_call.
The fragment constructs GeneratedDeclarationMacroIndex with
DeclarationMacroParent::Generated(actual parent frame), charges the same shared
expansion budget and continues the SAME declaration traversal. No macro is
re-executed merely to discover its declaration subtree.

Important identity distinction: GeneratedCallRequest.key.invocation is the
actual GeneratedCallKey fingerprint; DeclarationScan producer.identity is a
different capture identity. admit_completed_item_call accepts the actual scan
producer separately; its existence is not permission to pass record.invocation
as that producer. Diagnostic SpanRecord equality alone is also insufficient:
identical/copied origins can occur several times. Preserve the existing
structural occurrence+coverage proof or factor its common kernel.

DeclarationFragment::from_item_output currently requires an original
DeclarationMacroIndex whose scan fingerprint equals output.parent_inventory.
Do not feed it a generated output with a convenient original parent. Existing
recursive GeneratedDeclarationMacroIndex construction already carries the
correct generated parent scope/context.

## Minimal integration proposal

Prefer reuse of the already registered original root declaration job and its
recursive completed item owner. Retain a bounded link from each live root
GeneratedFragment to that exact existing declaration job; child frame ancestry
is already explicit in GeneratedChild.parent/order/catalog. When a child
header plan exposes an actual supported impl frontier, register a typed waiter
for that frontier bound to the actual child record/descriptor, owning child
scope, epoch/cfg and its original ancestor declaration completion. Preserve all
other opaque ids unchanged; producer-kind completeness is not inferred from
“remaining blocker” count. This is a common owned-output contribution waiter,
never wrapper identity logic.

After catalog closure, the root declaration job runs the existing recursive
fragment. Before releasing a child impl frontier, require its complete
CompletedDeclarationFragment and an actual matching child syntax_output for
that record (same descriptor/key/epoch), with its correct generated parent
inventory/invocation chain. Bind each actual generated Impl header through the
shared item_impl_headers relation to the genuine retained child DeclarationScan
ScannedImpl header identity; require those identities in completed inventory.
A root fragment being complete or containing an equal-shaped impl is not enough.
This relation may be prepared cold at child admission or finalized against the
completed actual child output; it must never guess an impl identity from a
name or a shared display range. Missing output/parent correspondence is Pending
or a precise authority error, never empty contribution.

Use existing source base inheritance for filesystem/include context but keep
actual child pending_scope/owner and request parent_context/token call edition.
The frontier's release scope is the genuine child insertion scope, which may
be an inline generated module; do not substitute the outer source call scope.
GeneratedDeclarations::admit_attached intentionally permits generated token
call edition to differ from parent parse edition only because the actual owned
call bridge validates that context; do not add origin synthesis or force the
enclosing edition.

Do not put the impl frontier into child.release, whose macro catalog completion
currently retires call/outward frontiers. Keep the child producer/work waiter
live until the declaration completion proof exists. The catalog can complete
its macro binding job first; the separately tracked namespace declaration
frontier must still prevent sealing. Avoid a dependency cycle: the declaration
job waits for macro catalog completion, while macro catalog completion must not
wait for that same declaration job merely to retain its pending impl frontier.
Queue/block using the existing work scheduler and actual prerequisite ids.

If eager child GeneratedDeclarations capture is needed instead, expose one
common pending/completed owned-call projection on GeneratedMacroCatalog that
accepts the actual parent GeneratedDeclarations and exact adopted child record.
Factor completed_declaration_call's occurrence correspondence rather than
copying it. The returned child facet binds the actual parent scan producer and
parent inventory. A genuine Generated DeclarationMacroParent must then be
carried; never relabel it Source. This is larger than the existing recursive
job reuse and should be justified by an actual missing service.

Preflight all job/map conflicts, record equality, scope/epoch/cfg, depth/count,
Vec/map/retained metadata and queue updates before publishing graph facts,
parking the exact parent call or mutating the child frame. Record/outputs stay
Arc owned, with existing shared expansion budget accounting. No unchecked
absence or forgotten blocker is allowed after a failed partial admission.

## Costs, evidence and eight-mode review

All work is cold declaration discovery. Reusing one root recursive closure
avoids a second full declaration parse/traversal of every child and does not
execute/lower guest bodies. Existing completed_declaration_call currently builds
bounded origin/producer lists, sorts them and checks coverage; retain/account
that cost or an indexed equivalent. item_impl_headers currently performs cold
header/scan attachment work; a peer is separately reviewing retained-scan reuse.
New waiter metadata should be fixed record/scope/job ids or shared existing
handles, deterministically encoded and bounded under Section32.2, not a cloned
BodyIndex/parser or world namespace snapshot. No installed operation or ordinary
call entry can replay this closure. No performance measurement claimed.

Evidence required: same unchanged full source ordinary/user/wrapper/stdmacro
where applicable; exact record/child ancestry/parent scan producer/scope/edition
and impl header identities; no duplicate macro execution; wrong record/epoch/
cfg/scope/identical copied call-origin negative controls; impl plus unrelated
blocked owner/import must remain blocked; root job completion ordering has no
cycle; actual completed inventory remains distinct from ImplCatalogSeal/Drop
authority; pure closure does not query executable BodyStructure/Thunk; full
source runtime/call exact-once/suspension/cleanup/invalidation matrix separately.
Imported fixture and source inspection are not passing stock runtime evidence.

All8 separate review: (1) no assertion/expression evaluator, declaration jobs
feed ordinary syntax/name/Type/call engine; (2) actual immutable child records,
no scalar/break lane; (3) no macro/wrapper/numeric/empty-body rule; (4) actual
generated parent identity replaces a missing join, not an original-only gate;
(5) declaration metadata/required producer closure only, no eager unselected
executable arm or callee body/pattern engine; (6) once cold shared declaration
job/immutable completed tree, no phase replanning or re-expansion; (7) actual
producer/frontier dependencies and scopes, no hot namespace census; (8) Arc
record/tree/index roots and existing generated-parent traversal, no deep body
copy or whole-file per-node execution search. Legacy generated opaque refusal
is held until the exact typed contribution waiter/common closure replaces it;
unsupported other blockers remain held afterwards. No repair gate is closed.

## Source fingerprints

Moving source, not a frozen candidate review; later edits require rechecking.

- `crates/lrr/src/expand/source_discovery.rs`: `62356c4d27d01129aef06243bf773c224cdf373f773eabc579f4f1c3cf76c00b`
- `crates/lrr/src/expand/generated_declarations.rs`: `d9b6b508363c89851500adf304a6f0501f546a12592825514f9732d7a24a2ff7`
- `crates/lrr/src/expand/declaration_fragment.rs`: `5ddff003dd7e6c247d1b8a6359e017b6919577fef59f9e1dc229cd7e35a07f0c`
- `crates/lrr/src/frontend/generated_macros.rs`: `097184e6964424b3d15469f3d3e8e93884864c3146a8d0024f9dd70c7937365b`
- `crates/lrr/src/frontend/generated_headers.rs`: `1536d78d1e1b2127ccae054c8d9eff783ebe9d9488c954da2a61306b252fa874`
- `crates/lrr/src/expand/generated_call_request.rs`: `e6e1b52ed326c730bb44e677c3125956a5c153acd52be61787abd015795b9721`
- `crates/lrr/src/expand/generated_items.rs`: `379038cfaca9befd94bce7b4969593f34164124a5a5c2f29e6045d90f45440dc`
- `crates/lrr/src/expand/source_discovery/producer_closure.rs`: `44c6c37c8bbfb6f86c65d009090b24d754af73d5d124711dc86757669e4aff4c`
- `crates/lrr/src/frontend/impl_index.rs`: `b68da8e748bc676f1dc99af3c9c1c8bca91d241ef388d169362a453a35f2a5ea`
- `fixtures/upstream/rustc-checkout/tests/ui/resolve/type-param-local-var-shadowing.rs`: `c90c918b7d21b237f7137bd8a744022bf5b63c5fd61253b30fd7d893ad259d23`
