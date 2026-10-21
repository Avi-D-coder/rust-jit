# M020 demanded runtime catalog coordinator: frozen-source design

## Scope, invariant and witness

V528 is frozen: this record is read-only production analysis, with documentation
as its only output. M020/Section53 unchanged repr128 remains the primary witness.
The missing invariant is a real host metadata continuation: an entered call may
request an authentic full implementation catalog after arguments are evaluated;
the coordinator prepares/publishes it and resumes the same pending call. A Value
scope seal, caller-selected impl subset or source-factory component success is
not a full catalog. No ordinary-root admission or solver authority API is designed
here; that separate slice belongs to the assigned owner.

The current concrete join is cli/mod.rs::standalone_captured (the actual name;
there is no standalone_source_captured symbol). It retains PreparedCompileInputs,
root view, one ProceduralRun, snapshot and runtime macro contexts. It completes
Type and Value discoveries, consumes Value discovery in
RunMachine::start_with_discovery, begins the run, then polls only once. NeedInput
becomes unsupported, or is packaged immediately by capture mode. In contrast,
SourceDiscovery::for_impl_catalog is currently reached only by preentry-loader
controls. The existing host already owns the right services; no second VM, IO
provider, macro run, source capture service or parser is needed.

## Minimal split and truthful graph identities

configured_catalog.rs::for_impl_catalog currently combines (1) genuine captured
membership/provenance/key construction, (2) new_domain and dependency registration,
and (3) compiler-route preparation. Split at those boundaries. A concrete proposed
API, with private fields/constructor and names subject to implementation review:

- `PreparedImplCatalogDemand::prepare(compiler, root, epoch,
  Option<Arc<PreparedProcCatalog>>, db) -> Result<Self, Diagnostic>` builds the
  real immutable plan and retains shared captured source handles. It invokes the
  existing PreparedLiveImplCatalogSources::capture and
  PreparedCompileInputs::impl_catalog_sources provenance checks. It does not
  invoke new_domain, prepare_declarative, discovery.poll, expansion, file capture
  or executable body preparation.
- `request_key(&self) -> &SealKey` exposes only the genuine demanded key. That
  key is a locator/request, never a completed seal. Retain this dormant plan in
  the CLI request owner and make the exact key available at the normal metadata
  request boundary through the other owner's existing pending-call work.
- `begin(self or retained-state, compiler, db) -> Result<SourceDiscovery, ...>`
  revalidates the real captured publication/compiler/environment/epoch, creates
  the existing new_domain discovery, installs its original UnitCompileEnv stamp
  before polling, and transfers the authentic private ImplCatalogPlan. Compiler
  configuration then uses the existing common preparation route below.

The plan must distinguish two real identities rather than conflate them:

1. Compute and retain `source_closure` exactly as current constructor does from
   actual SourceDependencies and sorted original source selections/outlines.
   Keep it, generation, environment, engine, compiler profile and facade identity
   in the catalog plan/domain-id stable encoding. Preserve the original-source
   completeness loop and actual payload certificate unchanged. Bump the plan and
   domain identity encoding version when adding/changing these fields.
2. Set SealKey.unit_graph to the actual ordinary macro graph: with procedural
   tools, derive it from the retained PreparedProcCatalog::graph(), whose private
   fields originate in the authorized MacroRun catalog; without procedural tools,
   derive the same root crate graph identity that standalone's Type/Value routes
   already use from root.selection().outline.crate_key. Do not accept an
   arbitrary caller Digest or caller-selected source vector for this identity.
   Feed the same actual graph to PreparedLiveImplCatalogSources::capture.

For the procedural case, plan construction must check actual catalog consumer
against the owning root unit, epoch against the published expansion epoch, and
environment against PreparedCompileInputs::environment_identity. Retain the
actual catalog Arc/fingerprint (its stable encoding includes preparation and
exports) in the plan/configuration identity. At begin/configuration, require the
same current catalog and compiler provenance. The real same ProceduralRun owns
execution/publication, not mere possession of this immutable catalog.

This resolves the current mismatch: for_impl_catalog hashes source closure into
unit_graph, but ProceduralRun::graph derives ToolManifest.graph.fingerprint.
SourceDiscovery::prepare_declarative and with_procedural_catalog correctly require
catalog.graph == key.unit_graph, plus consumer/epoch/environment. Keep those
checks. The source closure remains authenticated separately, so using the actual
manifest graph neither loses membership dependencies nor relabels a catalog.
The existing none-procedural constructor can remain a thin wrapper over this
split for controls, rather than becoming a second implementation.

## Retained configuration and host continuation

Add a common `prepare_with_declarative_compiler_procedural` sibling returning
SourceCompilerPreparation instead of forcing require_ready. ProceduralRun needs
a preparation-returning configuration entry point using its own retained catalog
and expansion ledger, analogous to its current configure; the ledger must be
attached before discovery execution. An immutable catalog getter or a single
prepare-demand method on ProceduralRun can provide the catalog Arc for key
construction. Do not copy/recreate the manifest or start a new MacroRun.

A host state owner can retain Dormant(plan), Preparing(configuration),
Discovering(SourceDiscovery) and Completed(actual key/input/guard) states. In
standalone_captured, surround the existing poll with a host loop. On exactly the
retained catalog Discover key, perform the following outside VM callbacks and
pure queries:

1. Begin the dormant actual plan once and finish common compiler configuration.
2. Drain through the existing PreparedCompileInputs::discover_diagnostic_with,
   passing the same ProceduralRun as SourceExpansionExecutor. It already
   coordinates include/env/declaration/compiler requests, module captures,
   ordinary/generated adoption and diagnostic source snapshots.
3. Publish through ProceduralRun::publish (which uses
   MacroRun::publish_discovery_retained) when procedural, otherwise through
   discovery.publish. These preserve actual full fixed-point/payload completion.
4. Obtain the actual published input/dependency under that exact key, retain it
   for the metadata consumer, and repoll the same RunMachine continuation. Do not
   replace its Value RunNames::Completed graph or rebuild argument preparation.

Other requests require their already-authorized common services; this catalog
service must not claim to implement every InputRequest. Actual unavailable typed
semantic producers cannot be converted into external input success. Repeatedly
requesting a catalog already published/current is not permission to fabricate
missing facts or rerun a completed plan. Preserve a precise pending/unsupported
result where the corresponding producer is absent; no retry-count cap is added.
Capture packaging belongs after the terminal poll, preserving genuine suspension
observations and faults, rather than declaring the first NeedInput terminal.

## Real preconfiguration module gap

SourceCompilerPreparation::NeedModule is not itself runnable discovery.
prepare_declarative constructs local SourceDeclarative providers, builds routes
and can return NeedModule before storing providers into self.declarative. The
current service loop's prepare_ordinary_module_source requires self.declarative
and a registered pending producer. Thus simply passing the returned discovery to
discover_with cannot fulfill this configuration demand.

The common repair must retain a typed compiler-preparation continuation containing
the actual providers/configuration/routes needed for module preparation. Join
ModulePreparationDemand::declaration against actual SourceModuleDeclaration
identities (existing exact scope-prefix join), then prepare/capture/adopt through
the existing source/module protocol, and resume that same compiler configuration.
The module demand alone grants no capture authority. A narrowed first component
may truthfully return its existing diagnostic if this continuation is not yet
implemented, but cannot claim arbitrary full-catalog production coverage. This
shared module configuration prerequisite is separate from normal solver-root
admission and must not be hidden by treating a missing module as absent/empty.

## Dependencies, authority and costs

The lazy plan binds actual root/source-publication ownership, original member
selections, source dependency identity, compiler generation/profile/environment,
facade identity, expansion epoch and optional real procedural catalog. Revalidate
these at demanded preparation/publication, not by scanning the world at every
operation. The complete actual candidate bucket still comes from ImplTraitCatalog;
this coordinator does not perform semantic matching or select implementations.

PreparedCompileInputs opened and retained SourceCapture before the run.
Database::require_source_preparation forbids new source-preparation initialization
in active guest runs; preserve it. The retained common discovery services do
coordinator work outside the pure query/VM callback and use the already prepared
capture service. Database::publish_discovery stages derived inputs and advances
ordinary revision without changing source_revision; MacroRun::check_database
correctly permits this while requiring its original source revision/epoch.
Whole publish_inputs changes source_revision and refuses active-run replacement;
never reset inputs, finish/restart a guest run or weaken the guard to evade it.

Root/call owner must preserve the same pending attempt, selected callable,
argument cells/slots/types and retained body/receipts while external metadata is
prepared. This record only specifies the host boundary; it grants no root goal,
argument relation fulfillment, numeric inference or call commit certificate.
One actual MacroRun/cache/session serves Type, Value and requested Catalog seals;
publication stays retained, not terminal. Existing expansion caching and exact
source keys must be reviewed/tested for no repeated procedural effects when a
new catalog requests a producer already reached by Type/Value discovery.

Dormant plan work is one cold pass over already captured source handles and
provenance. Actual requested declaration/macro-producer fixed point is cold;
executable bodies/cold branches remain unprepared. Hot execution retains the
locator and ordinary pending-call guards. No world scan, hash reconstruction,
filesystem IO or source planning occurs in dispatch or the upstream callback.
No new resource accounting/limit is proposed.

## Next implementation ownership and required evidence

Boundaries for a later authorized implementation are configured_catalog.rs and
its impl_catalog reexport (split plan, encode both identities),
compiler_preparation.rs/source_discovery.rs (common typed configuration/module
continuation and preparation-returning procedural entry), cli/proc_macros.rs
(retained catalog and real MacroRun configuration), service/source_preparation.rs
(reuse/extract common module coordinator), and cli/mod.rs (retained request owner
and same-VM service loop). db/poll.rs can keep existing Discover; it needs no new
fake seal variant. The engine's normal-root/pending-call admission slice remains
owned by the other worker; coordinate only the authentic key and published-input
handoff. Do not edit root factories or implement a second caller evaluator here.

Review against all eight architecture failures: this extends the common source,
macro and host continuation services, adding no alternate evaluator, literal-only
generated route, context-specific member/numeric semantics, generated exclusion,
pattern engine, cold executable checking, repeated operation planning, world scan
or immutable body clone. The once-only CLI unsupported continuation is the
specific production route to replace; full scope catalog controls remain component
evidence until this path is exercised. NeedModule and unknown semantic producer
holds remain explicit. No task or architecture gate closes from this design.

Required parent-owned evidence: actual unchanged repr128 full candidate catalog;
real runtime NeedInput/resume with an argument effect occurring once; procedural
producer requested in multiple seals with actual invocation/output receipts;
compiler include/module continuation; true full catalog membership including
unrelated supported candidates; changed relevant source/compiler/epoch rejection;
unchanged unrelated input retention; no cold executable preparation; capture
records actual terminal result. None was executed in this read-only design.

Inspected frozen fingerprints:

- configured_catalog.rs: c3ce0d0920ae1c2728e0af7a9b19efa3bf1e212935ac9fb87df96aa89eabcd79
- service/source_preparation.rs: 2006dafa9fedff8546e3d79c3162cfe47b121c954da6bd83e345cb55914d910f
- cli/mod.rs: 81c5c61dd1a4222ba3ad31ea051f86d0c18b3098f303aed3f26c7e17e5d57501
- cli/proc_macros.rs: 2f8db5f0453fe7289d6984931f17dcdcaf8e440ea98ed81a1c9ee131a93cd3ca
- compiler_preparation.rs: 3e0cf2461f8dac2b375a5ce6f4d58c7c2791acbe20fcfec3b4e05327af8d8584
- frontend/module_aliases.rs: 00ebd653c3b0b3f752e5e05665abfaff9eb594a160cab00e102f51e22609d5ef

No production/tools/vendor/fixture/Cargo/HEAD edits, tests or Git commands ran.

## Next bounded slice: exact dormant-plan API, awaiting source release

Parent reserved only configured_catalog.rs for this worker's next implementation;
production stays frozen until explicit release. No CLI/ProceduralRun, root
admission, service loop, solver or module-continuation edit is part of that slice.
The following API makes the split useful without performing configuration early:

- New private-field `PreparedImplCatalogDemand` in configured_catalog.rs, retaining
  an Arc<ImplCatalogPlan>, an actual shared PreparedSourceView root lease, the
  already authenticated captured source handles, and exact published source,
  environment and epoch dependencies. No constructor accepts rows/key/graph.
- `SourceDiscovery::prepare_impl_catalog_demand(compiler: &PreparedCompileInputs,
  root: &PreparedSourceView, epoch: u64,
  procedural: Option<Arc<PreparedProcCatalog>>, db: &Database)
  -> Result<PreparedImplCatalogDemand, Diagnostic>` performs only existing actual
  membership/compiler/facade authentication and identity formation. It checks the
  real published epoch and optional catalog consumer/epoch/environment. It records
  no DiscoveryInput and builds no name/import/compiler-route graph.
- `PreparedImplCatalogDemand::key(&self) -> &SealKey` supplies the exact request
  locator. `procedural_catalog(&self) -> Option<&Arc<PreparedProcCatalog>>` can
  expose the retained real catalog for the later host's existing configuration.
  These are reads of captured metadata, not currentness or completion promises.
- `require_current(&self, compiler: &PreparedCompileInputs, db: &Database)
  -> Result<(), Diagnostic>` checks exact DB source lease/publication identity,
  retained published input stamps, actual epoch, compiler generation/profile/env
  and optional procedural catalog facts. It does not reparse or recalculate a
  complete plan. Holding the same immutable root publication authenticates its
  captured members; no full-world scan on this guard is needed.
- `into_discovery(self, compiler: &PreparedCompileInputs, db: &Database)
  -> Result<SourceDiscovery, Diagnostic>` consumes the dormant handle, validates
  it, invokes existing new_domain once, installs the actual ImplCatalogPlan and
  UnitCompileEnv stamp before polling, and returns an unconfigured discovery.
  The later CLI owner must configure it with the exact real procedural/compiler
  facets/ledger before discovery can complete. This method does not call
  prepare_declarative or claim SourceCompilerPreparation::Ready.

Existing SourceDiscovery::for_impl_catalog remains a thin no-procedural wrapper:
prepare authentic demand, consume into_discovery, invoke the existing
prepare_with_declarative_compiler. Existing test callers thus continue exercising
the same production builder; there is no new test-only constructor. Its graph
identity now matches the actual no-procedural root identity, while the real source
closure is retained in the ImplCatalogPlan/domain identity as described above.

ImplCatalogPlan needs source_closure, facade identity and optional actual
procedural catalog fingerprint retained/encoded independently of key.unit_graph.
Its completion gate should require any expected real procedural catalog in the
actual input's procedural_catalogs and the unchanged environment/engine/full
producer/header/payload checks. A different catalog cannot complete a dormant plan
merely by sharing a digest supplied by a caller. Actual catalog equality derives
from immutable authenticated catalog data; the later MacroRun still separately
verifies execution/currentness and matched native server authority.

One minimal visibility change is required: impl_catalog.rs currently privately
nests configured_catalog and only reexports ImplCatalogCompletion crate-wide;
ImplCatalogPlan is parent-only. Add a crate-visible reexport of the new dormant
handle so the later CLI owner can name a retained service state. This reexport
needs parent coordination; no other shared-file change is needed for this bounded
slice. PreparedSourceView has no Debug implementation, so the new handle should
not derive Debug over its leases; preserve existing ImplCatalogPlan/Completion
Debug behavior and avoid dumping captured source contents.

Implementation must not attach a fake seal, expose raw completion fields, relax
procedural graph equality or bypass NeedModule. Its acceptance is compile/source
component evidence plus existing catalog controls under parent-owned validation;
the host loop, module continuation and unchanged repr128 execution remain held.

### Released implementation boundary (before production edits)

Parent reports V528 all terminal and explicitly releases configured_catalog.rs
plus the dormant-handle reexport in impl_catalog.rs. Implement exactly the dormant
prepare/key/currentness/consuming-discovery split above, preserving existing full
completion checks, with domain/plan v2 and independent source-closure/facade/
procedural identities. No common compiler-route preparation, IO, CLI continuation,
module continuation or root authority enters the dormant constructor. The old
for_impl_catalog becomes the single thin no-procedural compatibility wrapper.
Source guards retain actual source and input provenance; no caller metadata or
arbitrary graph constructor is exposed. No Cargo/Git by this worker.

### Dormant split implemented: source review and bounded handoff

Actual API matches the proposed boundary. The crate-visible handle is reexported
as source_discovery::impl_catalog::PreparedImplCatalogDemand. Preparation retains
its real source publication lease, authenticated source handles, four genuine
published dependency stamps (root CapturedSource, SourceDependencies,
UnitCompileEnv, ExpansionEpoch), the actual optional PreparedProcCatalog Arc and
private Arc<ImplCatalogPlan>. It cannot be built from a raw SealKey, source vector
or graph digest. Getter reads do not assert currentness.

`require_current` rejects active pure-query coordination, changed/missing retained
stamps, changed root publication identity, compiler generation/profile/environment
or engine changes, and differing procedural identity. `into_discovery` consumes
the dormant handle, validates it before new_domain, and installs the actual plan
and original environment stamp before any polling. It returns an unconfigured
SourceDiscovery; no compiler-route/configuration fixed point runs in preparation.
The no-procedural compatibility wrapper calls this same builder then the existing
prepare_with_declarative_compiler. Existing callers already publish expansion epoch
before entry; missing/different epoch is held rather than inferred.

Plan/domain encoding are v2. The source closure hashes actual source dependency
identity plus sorted original source selections/outlines; source dependency,
source-closure, facade and optional actual procedural catalog fingerprints remain
explicitly retained in the plan. unit_graph instead uses the actual retained
procedural graph, checked against actual consumer/root, epoch and environment;
without procedural tools it uses the owning root unit, matching ordinary Type and
Value discovery. No caller-selected graph argument exists in the public API.
Existing source_discovery procedural graph/epoch/environment and expansion-ledger
checks were left unchanged. Completion keeps its actual seal/payload/compiler
facets/all original sources/full producer/import/index checks and additionally
requires the real source dependency identity and expected procedural catalog in
the actual publication (none requires none). No fake completion can be produced
from the dormant handle or unconfigured discovery.

Separate source review against the eight recurring failures:

1. Only source metadata/plan construction and the existing common discovery entry
   changed; no interpreter/assertion/expression evaluator was added.
2. Original/generated declaration producers still join existing discovery,
   including its real expansion records and complete indexes; no scalar route.
3. Neither catalog membership nor semantic trait/numeric matching is selected by
   context or name; membership comes from actual prepared source publication.
4. Actual source handles and ordinary discovery retain generated identity; there
   is no generated-source exclusion or currentness bypass.
5. No executable body, pattern engine, unselected arm or cold branch is prepared;
   existing NeedModule remains a genuine configuration hold.
6. The dormant plan captures handles/dependencies once; into_discovery consumes
   it and does not recapture membership/rebuild its identity. Normal operations
   never invoke this constructor implicitly in dispatch.
7. Cold prepare authenticates actual captured membership and hashes it once;
   require_current checks four retained stamps, source publication identity and
   actual compiler/procedural metadata. No VM callback/hot-path world scan or new
   global invalidation mechanism was added. The procedural fingerprint recheck
   is cold only. No performance measurement is claimed.
8. Source views share immutable publication storage; no BodyIndex clone or
   repeated whole-file executable-node search was introduced.

Changed symbols are ImplCatalogPlan::encode_fields/complete, new
PreparedImplCatalogDemand getters/require_current/into_discovery,
SourceDiscovery::prepare_impl_catalog_demand and thin for_impl_catalog; the only
other production change is the dormant type reexport. CLI/ProceduralRun host loop,
shared compiler-configuration module continuation and normal-root admission remain
separately held/unimplemented. No old complete-catalog route was narrowed or
removed; its entry now uses the common split. This is a source component handoff,
not actual catalog coverage, unchanged repr128 execution, procedural exact-once
or M020/architecture acceptance. Parent owns direct validation. No Cargo/tests/Git
commands ran in this slice.

Reviewed fingerprints:

- configured_catalog.rs: 45b8ca429e54c8835d288ce63bd1184ccf1cf96694a42125bafe8909dbfb16e3
- impl_catalog.rs: a8c209b6c23e626f90e5dfaed8792c35797e1e97cc14350e91f3fae652a75469

Independent review requested against these files; production is held unchanged
pending review and parent validation ownership.
