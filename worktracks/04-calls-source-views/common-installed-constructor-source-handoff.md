# Immutable completed source graph and installed constructor binding — LOCAL-024

Pre-code record, October16, M017/M018, T040-T045, E-R5-005.
The actual unchanged local A/B and qualified-record witnesses expose missing
shared Type paths; source review additionally finds every ConstructStart reruns
prepare_constructor after recipe installation. Owning SourceLocalPlan memo alone
does not retain the complete binding. Preserve the existing common constructor
shape/privacy/field-Type binder and normal verifier/guard/ownership engine.

Pattern worker owns opaque CompletedSourceEnvironment minted once by the real
sealed SourceDiscovery; actual immutable names/source roots and seal/epoch remain
accounted through BodyEnvironment's existing retained-root framework. Root owns
engine/run.rs and test_catalog.rs joins, then the common pure binding query and
installed recipe payload. No graph reconstructed/cloned per body, operation or
query. Never use a graph pointer/source-only hash as namespace authority. Actual
prepared Type occurrences and their positive/negative dependencies still select
names; the retained graph supplies the same existing metadata/catalog kernel.

Runtime source graph must remain immutable. run.rs currently contributes context
names and resolves imports at every poll. Blind Arc::make_mut would add a deep
copy after guest entry. Distinguish owned legacy unsealed preparation from an
actual completed graph consumed by start_with_discovery or sealed test catalog.
Only the former preserves its existing context contribution path. Completed
discovery has already closed ordinary declarations/imports; it consumes the
shared graph without rebuilding or contributing context names. Existing actual
context-source validation remains before dispatch; no macro-specific name rule.
The legacy path remains held for removal, never evidence of architecture closure.

Cold discovery graph mutation may use Arc::make_mut before completion; immutable
handles are retained only after genuine published completion. Source/epoch
changes invalidate the real receipts, and installed operations must eventually
guard the exact preparation dependencies with no planning, global map or source
parsing. Runtime hints, allocation, guard state and cleanup remain per frame/run.

Eight-mode source review is required on frozen bytes. No macro evaluator or
context-specific type rule is added (1-5); removing source planning from installed
execution is mode6, relevant guard footprints mode7, immutable sharing mode8.
Current broader MA001-MA008 holds persist. Required evidence includes unchanged
stock main and module-qualified constructor, original/user/wrapper/std contexts,
repeated loop/call entry without preparation, stale relevant/negative dependency
rejection and unrelated publication reuse, budgets/retired roots, cleanup and
suspension exact-once. The present graph ownership component alone closes none
of those architecture or runtime acceptance gates.

October16 implementation continuation (source only until frozen validation):
`BoundRecordConstructor(PcKey)` (stable tag35) will consume the existing
ConstructorPlan -> SourceLocalPlan -> prepare_body_type -> SchemaNames binder,
using that plan's genuine CompletedSourceEnvironment. Its opaque ready payload
owns only the exact plan and selected BoundNominalConstructor; it contains no
runtime TypeId, mutable frame, namespace reconstruction or executable callee.
Deferred/unsupported outcomes keep their existing diagnostics and do not install.
The metadata-only preentry service must reject the new execution demand.

InstalledRecipes will retain the exact outer Arc<QueryValue> using the existing
query-pin accounting. A bounded cold dependency traversal will flatten the
successful pure memo's actual transitive input stamps, including missing inputs,
into an immutable prepared-input guard. Dispatch will compare only those cached
published stamps, O(D log I), without fetching providers, reparsing, rebuilding a
plan or constructing a namespace map. Source revision and existing verified
recipe guards remain. Relevant namespace-footprint coarseness inherited from
source services remains an explicit mode7 hold; it is not fixed by this cache.
No ambient/world census is added or substituted for actual query dependencies.

The VM retains a typed query handle for immutable nominal facts and only owns
runtime materialization/hints/construction state. Mapping/hint checks and failure
ordering still precede guest guard allocation/fuel. The old non-prepared-entry
constructor route remains explicitly held; it must not be expanded. Evidence
must cover exact unchanged upstream constructor witnesses plus LRR cache reuse,
missing/relevant-input changes, memo eviction while a query handle survives,
NeedInput before atomic install and per-frame hints/cleanup. All remain
unexecuted at this pre-code record. This is T040/M018 infrastructure repair,
not a replacement for source-selected Drop/trait prerequisites and not task
closure. Pure metadata functions have no IO, compiler, allocation in GuestStore
or VM effects; VM materialization retains its existing classified effects.

October16 retained-query control follow-up: frozen `8a733a43` compiled
(`4f4f3d/0`), six controls passed and final zero-budget reclamation failed
(`5cee85/1`). Source review identifies a stale cached SourceLocalPlan retaining
its prepared-source graph after BoundRecordConstructor becomes an error. The
current evictor treats that reachable graph as an irreducible source/pinned floor
before trying the evictable owning memo. Root will preserve all actual charges
and external/retirement pins, but first remove genuinely evictable cached roots
under pressure, prune their dead leases/weak registry entries, then recount and
enforce every cap. This is cold between-run work only. No hot execution, name,
type or ownership authority changes. Existing cold accounting is a full retained
DAG walk per victim; improving that cost is separate, not a speed claim.
Required controls include the exact failed retained-query case plus real external
handle refusal and exact final reclamation. All eight language failure modes
remain held as above; this cache correction grants no new runtime semantics.
