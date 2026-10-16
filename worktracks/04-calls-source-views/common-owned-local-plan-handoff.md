# Owned entered/local source plan

Pre-code LOCAL-024 WT04 T014/T024/T032/M014/M017/M018 successor to bodyType2
copiedb11958/0. Invariant: a retained local-prefix plan owns the actual complete
entered source/module/boundary/scope/declaration/hygiene/ancestry/producer environment;
source/body equality is not authority to rebind old tables to another environment.
Use the existing local/import/frontier/lookup algorithm once, share immutable maps,
and keep Type/generic declaration NameRef authority separate from stored metadata.
Unchanged official Point/x_coord, const/enum source witnesses remain primary.

PublishedMacroIndex will share its actual immutable SourceMacroIndex/runtime-outers
via Arc; EnteredMacroIndex uses Cow parent ownership plus Arc local maps and owned
RetainedMacroScope/producers/Namespace roots. Legacy borrowed entry stays an adapter;
owned snapshots contain no borrowed ancestor references. COW protects previously
retained snapshots at the existing no-item publication mutation. No stripped/fake
parent index, root reconstruction, alternate resolver or BodyIndex clone.

SourceLocalIndex becomes an adapter over one Arc<SourceLocalTables>. The new
PreparedSourceLocalPlan cold builder owns the entered environment and those same
tables, retains actual PreparedBodyContext/source view leases, and accepts no raw
resolution/publication grant. Unresolved source classifications/frontiers remain.
Queries select against stored roots after current source/boundary checks; there is
no public source/body-only rebinding API. Metadata observations are not completed
Type/item/generic/module authority: exact use/declaration occurrence and publication
proof debts remain typed before any future promotion.

Root owns DB/provider/memo/input publication and allocation registration. New plan
must expose exclusive metadata, actual shared Arc roots and cached source/resolution/
publication identity for that integration; no private side cache or hot rebuild.
Cold O(reached entered facts + existing local table construction), then Arc snapshots
and one existing lookup kernel. Runtime/CLI and original files are untouched; no
Cargo/native/extra agents/commit or gate closure by worker. All8 review and exact
source fingerprint follow implementation, with remaining authority gaps explicit.

## Exact source component and remaining joins

Source7 fingerprint `007e56a6de7cd726c67bed19d884bd825be2d3dac5ae5e2414c29dffc53583a0`.
The complete sorted paths/SHA256/bytes and separately hashed required predecessor
leaves are in `worktracks/evidence/2026-10-15-owned-local-source-roots.json`
(manifest SHA256 `9a6069344881f9f1f6de309c969f40ce432e0d50ead2359ef8dda73039a61e41`).
All seven source leaves are held unchanged for root copy. This is uncompiled and
unexecuted by this worker. Copied bodyType2/import6/constructor3 and their actual
failed/partial evidence remain attached to their prior frozen bytes, unchanged.

The primary unchanged official witness remains
`fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-borrow-scope.rs`:
243 bytes, SHA256 `b296c6d78fbf362d7ab196a4dd3418f8bc12f8a6c365bbfe5c0a5a75b6473979`,
upstream commit `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, Git blob
`2f25d0010833b01fc533e3bd5eeefadf5892ec1b`. Its source was read and byte-hashed
again in terminal cff875/0; it is not modified or executed. Existing complete
const/enum originals remain additional constructor source witnesses. Full Point
main needs Box, standard macro/library source and all ordinary constructor joins;
metadata observations do not replace that main with a supplemental identity pass.

### Shared APIs and ownership

`PublishedMacroIndex` now holds its actual immutable SourceMacroIndex and runtime
outer vector in Arc. Its Clone shares those roots and the real PreparedSourceView
lease. `EnteredMacroIndex` retains Cow<PublishedMacroIndex>, Arc scopes/keys/
declarations/producers/retired-no-item maps and captured Namespace, owned genuine
RetainedMacroScope outer ribs and Arc ancestor producer maps. The existing
build_entered_with_context/child_from_facts algorithms and source order remain.
The existing accept_no_item_proof mutates only through Arc::make_mut; a retained
older environment remains immutable. RetainedMacroScope receives the actual
scope/declaration/hygiene/ancestry roots and selected scope/before anchor. No fake
empty SourceMacroIndex, root-hygiene replacement, source/body-only rebind or body
copy is introduced. into_owned moves that exact environment and shallow-shares
its parent; same_environment compares actual roots and selected anchors only for
allocation/environment comparison, not a semantic key or source admission.

`SourceLocalIndex` is the short-lived existing lookup adapter over one
Arc<SourceLocalTables>. The ordinary local_index_inner builder remains the only
table-construction algorithm. Its bindings/imports/items/frontiers/resolutions and
canonical pattern-equivalence behavior are unchanged. SourceLocalTables owns the
former immutable tables; private fields remain inaccessible to consumers.

The new nested `source_locals::owned_plan` module exposes crate-only:

- `PreparedSourceLocalPlan::prepare(db,&PublishedMacroIndex,Arc<PreparedBodyContext>)
  -> QueryPoll<Arc<Self>>`: cold explicit preparation checks current source, enters
  the actual admitted context, builds existing local tables once, retains their
  same Arc and moves the exact entered environment to owned storage. It accepts no
  caller-supplied emitted declarations, pattern resolution or expansion grants.
- `context()`, `tables_root()`, `allocation_roots()`, `exclusive_metadata_bytes()`
  and `source_identity()`. The last is a cached source/known-expansion fingerprint
  under the new explicit domain `lrr/v1/owned-local-source-environment`. It is NOT
  a complete publication/namespace cache key; independent actual environments can
  have equal source fingerprints. Root must key/version genuine coordinator-owned
  environment publication inputs and retain all their actual dependencies.
- `SourceLocalPlanRoots` exposes source lease, actual SourceMacroIndex/runtime-
  outer/scopes/declarations/scope-key/retired/producer/Namespace roots and ordered
  RetainedMacroScope handles plus actual hygiene/ancestry. It does not expose an
  entered-index handle for stale historical lookup. Context exposes its already
  admitted parser/index/input/publication/boundary roots without cloning them.
- `inspect_prefix_metadata(db,OrdinaryNameSite,&NameNamespaces)` checks current
  context and actual source/body, borrows the stored tables and executes the same
  lookup_ordinary_prefix kernel. It returns LocalPrefixMetadata, with mandatory
  typed CurrentEnvironmentPublication, ExactEnteredOccurrence,
  DeclarationNameContext and ModuleNamespacePublication debts. Its optional
  NameLookup is historical metadata only, never a completed opaque proof. There
  is no production constructor or runtime caller. No namespace graph is built by
  this API; the external diagnostic preparation adapter supplies the old graph.

The metadata distinction is substantive: old item_candidates and header generics
lack exact Type declaration NameRef contexts, and OrdinaryNameSite lacks the
actual token edition/occurrence certificate. Those old spelling-based candidate
rows must not mint new Type/generic authority. The final lookup needs genuine
entered use-site and declaration occurrences, edition/hygiene checks and relevant
module/import dependencies. Original/retained/generated share that same hold.
CurrentModuleName is only the module tail after that opaque local/import prefix;
this component neither fetches it prematurely nor turns module absence into Type.
The legacy graph-based adapters remain named historical debt until the completed
replacement and matrix permit removal. Their unchanged behavior is not promoted
as completed common Type resolution.

LocalScopes also requires genuine expansion-key -> MacroDefinition metadata before
crossing a marked rib. The owned cold builder obtains retained definitions directly
from the actual admitted PreparedSourceSyntax map. For generated records, it joins
the current actual record and genuine GeneratedBodyBoundary parent records to the
exact selected declaration identity and edition retained in current/outer/module
MacroDeclarations/SourceMacroIndex roots. `definition_by_identity` and
`retained_definition` only read actual existing metadata and reject contradictions;
no token mark, diagnostic origin or record layout fabricates a defining unit.
Missing selected definition metadata, including unresolved procedural producer
identity, is retained as typed ExpansionDefinition(Key) debt before metadata lookup.
This is not a generated-only language refusal or expansion side effect. A body with
mixed retained expansion order still needs the separately named real structural
occurrence receipt; the existing context.reference_origin refusal stays intact.

### Accounting, preparation and costs

The plan exclusive byte count covers its inline owned entered environment, cloned
module/source selection metadata, retained-scope/ancestor-handle vector capacities
and missing-definition vector. Checked ceilings are 32 MiB exclusive and 64 MiB
exclusive plus new local-table estimate. SourceLocalTables::metadata_bytes counts
its own object, all original owned local maps, nested BTree storage, candidate and
import-index capacities and names. LocalScopes::metadata_bytes excludes its shared
Arc<HygieneFacts>; facts_root exposes that exact allocation. The estimates use
checked conservative per-node multipliers and are accounting bounds, not allocator
measurements. Namespace::metadata_bytes covers its actual captured facts and map.

Shared roots are separate: context/parser/BodyIndex/input/source publication/
boundary; source macro index; its four actual scope/declaration/hygiene/ancestry
roots (immutable_roots + exclusive_metadata_bytes); runtime outer Arc vector and
each actual scope/declaration allocation; entered scope/declaration/key/producer/
retired/Namespace allocations; ordered retained macro roots; local table allocation
and its actual hygiene root. Root owns weak-root dedup/control-block accounting and
memo retirement retention. None of these interfaces asserts accounting is already
registered. SourceMacroIndex and Namespace existing size methods have their stated
object-size conventions; the root registrar must avoid double-counting inline
objects and charge the actual Arc allocation/control block once.

Cold cost is the existing O(reached entered facts + original local table/index
construction), bounded ancestry/declaration-key joins and one stable source encoding.
into_owned/adapter retain Arcs and clone only source-selection/path/handle metadata,
not immutable AST/index/maps. Metadata table sizing walks owned tables once cold.
An observation uses existing relevant source/currentness and lookup/import work;
no parser, table builder or index construction occurs there. It remains a cold
query/preparation API, not installed operation dispatch. The legacy runtime_snapshot
still clones some immutable macro scope/definition data during explicit snapshot
preparation; it is not replaced or claimed fixed here. No speed/scaling measurement
or hot-path performance acceptance is claimed. Memoized plan transport, actual
publication dependencies and relevant module-tail query integration remain root's
prerequisites before production switching; no private cache is added.

### Controls and separate source review

External existing source-body authority controls now cover all four editions with
both a real outstanding body producer and no producer. They prepare one owned plan,
retain its actual tables/context/root handles, drop the original PublishedMacroIndex
and borrowed entered/local adapters, then query twice through the stored adapter.
They require the exact existing producer NeedInput or metadata candidate as
appropriate while requiring all four authority debts. An independently rebuilt
environment must not pass same_environment despite equal source fingerprint.
Stale actual declaration header replacement must refuse before metadata reuse.
These are new unexecuted preparation controls, not upstream main or runtime passes.

Eight-mode source review against exact source7 above, separate from MA001–MA008:

1. source_local_plan/SourceLocalIndex only prepare/select metadata. No expression
   parser/evaluator, assertion wrapper or runtime operation is introduced. MA001
   completion is not claimed.
2. Published/EnteredMacroIndex preserve actual ordinary source roots and selected
   scope/before order; no generated literal/break executor, new opcode or macro-only
   phase namespace. MA004/MA005/MA007 remain separately held where applicable.
3. retained_definition is an exact identity/edition metadata join, not method or
   numeric selection. Old Type/generic spelling rows remain typed proof debt;
   no primitive fallback or fixture field/name whitelist. MA003/MA006 not closed.
4. One preparation/lookup adapter serves original/retained/generated admitted
   contexts. Genuine missing boundary/expansion/declaration evidence stays explicit;
   no manufactured root, diagnostic-origin authority or new source-origin rule.
   The existing mixed-retained occurrence gap remains, not a compatibility pass.
5. Existing canonical pattern equivalence and local source frontiers are unchanged;
   no new matcher or eager executable arm/field/body lowering. MA002/MA008 pattern
   completion is not implied by retaining their ordinary local metadata.
6. Exactly one local table builder in the new plan cold path; observations only
   Arc-borrow stored tables. Existing direct borrowed adapters remain held until
   root pure-query integration/removal. No installed operation calls this plan.
7. No world namespace construction/comparison or new invalidation key in this
   component. The old NameNamespaces diagnostic adapter remains explicit debt;
   complete current environment and relevant module/import query proofs are still
   required before authority. No unrelated-invalidation performance claim.
8. Actual parser/BodyIndex and macro/local maps are retained through Arc/Cow;
   snapshots have real owned outer ribs instead of borrowed/fake rebuilt parents.
   No BodyIndex deep copy or per-occurrence whole-file search. Existing explicit
   runtime_snapshot deep copies remain named preparation debt.

Source-only checks: rustfmt terminal0b6e84/0,7eb8f3/0,559ea5/0 and final check
cff875/0; selected git diff --check559ea5/0. The final Python source check confirms
one cold local_index_with_pattern_index call and no observation-path builder; it
is a structural observation, not compiler/validation evidence. No Cargo/native
command, guest operation, strict check, milestone or fixture ran from this worker.
The sole validator must compile the coherent copied predecessor+source7 packet,
run the existing authority/local/body/import/producer/refusal banks and strict
checks, then the required ordinary/user/wrapper/standard source/trace matrix and
full stock prerequisites when their real joins are present. Root registration and
proof transport may change these conclusions; later bytes need separate review.
