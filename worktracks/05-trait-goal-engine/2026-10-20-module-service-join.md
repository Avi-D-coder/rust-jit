# Pending compiler configuration to actual module capture

Read-only V531 source-freeze analysis. No production/tool/vendor/fixture/Cargo/
Git changes or execution. WT05 / M019-M020 source-query prerequisites; this
does not admit a trait root or claim a runtime/module fixture pass.

The missing service join is concrete. SourceDiscovery's retained
`compiler_preparation::SourceCompilerPreparation::NeedModule { discovery,
demand }` already owns the exact pending compiler configuration. Its
`PendingCompilerConfiguration` retains the actual provider/catalog/source/route
inputs and import demand. `resume_compiler_preparation(self)` accepts no new
configuration and refuses started/sealed/failed discovery.
`pending_compiler_module_demand()` exposes the actual pending demand read-only.

Use the existing `ModulePreparationDemand::declaration` over actual
`SourceDiscovery::unresolved_ordinary_module_sources()` rows. This joins the
resolved missing unit/ModulePath to the shortest unresolved ancestor scope,
using each declaration's real owning edition and rejecting duplicate owners.
It is not a path resolver and grants no capture. `SourceModuleDeclaration` was
constructed by `from_captured` from actual external-module syntax, outline
Module DefKey/range/scope, and matching ModuleSource textual producer. Its
`producer()`, `source()`, `scope()`, `request()`, `range()` and `capture_origin()`
supply the actual declaration/caller identity. No guessed module/file names or
trait-specific source catalog are necessary.

The existing ordinary coordinator path in
`PreparedCompileInputs::discover_with` already performs the needed effect:
`ordinary_module_capture_caller(producer)` and
`prepare_ordinary_module_source(producer, authorities)` derive the real request
and parent directory. `SourceCapture::capture_module(request)` records every
candidate's presence/absence/non-file identity, repeats observations, and checks
the opened source root before returning. `ExpansionCoordinator` uses the actual
Outline(caller.outline), PhaseIdentity and InputRequest, then issue/start/
accept_module/adopt_module; adoption validates the same request and exact
source_revision before `Database::publish_module`. Only then may
`stage_ordinary_module_source` consume the adopted record. This validates parent
selection, producer, actual owning compile environment and child syntax.

Smallest next service slice: factor this producer-bound capture/adoption/stage
operation from discover_with, and drive the retained SourceCompilerPreparation
before entering ordinary discovery polling. Current discover_with immediately
calls discovery.poll(); poll explicitly refuses compiler_preparation Some, so
it cannot currently service the typed configuration demand. The continuation
must consume/return the retained discovery (resume consumes self), preserve the
existing compiler/procedural catalogs and never replace them through a ready
boolean. Stage uses `install_module_routes_for_preparation`, which already
retains further configuration demands without calling a parallel resolver.

Two progress details must be preserved:
- Resume compiler configuration immediately after genuine staging. Do not wait
  for complete_ordinary_module_namespace first: readiness waits a real parse_job
  in parsed_modules, and ordinary poll_inner is the operation that marks it.
  Once configuration is actually Ready, existing ordinary discovery drives that
  parse/annotation/macro work and complete_ordinary_module_namespace unchanged.
- unresolved_ordinary_module_sources retains a staged ancestor until its full
  namespace is complete. A pre-route demand loop must retain exact producer ->
  adopted CaptureModule requests and exclude those already staged producers
  when selecting a further physical declaration; otherwise shortest-prefix
  selection can choose the same staged ancestor forever. If no new actual
  declaration/progress exists, retain pending. Never infer membership complete
  from capture, or poll an unfinished compiler configuration to escape the hold.

Unit/source-role boundary: PreparedCompileInputs owns one root unit's frozen
environment/IncludeAuthorities/SourceCapture. A missing SDK/dependency module
cannot borrow that authority. `ordinary_module_syntax_input` expressly rejects
macro-export role parents. Existing
`cli::macro_export_modules::ConfiguredMacroExportSources::prepare` already has
the exact configured-role demand.declaration -> original_module_declaration ->
actual parent facet -> prepare_original_module_source join, checks actual
manifest candidate inventory, adopts the request, and refreshes/remints all
ancestry through MacroExportModuleAdoption. Its refresh intentionally uses
publish_macro_export_sources without ordinary source/parse registration. Reuse
that role-aware mechanism; do not promote SDK roles into ordinary input leases.
That cold entry opens captures and requires db.require_source_preparation.
Independently, Database::publish_macro_export_sources in db/macro_export_inputs.rs
explicitly rejects run_active. It cannot be invoked unchanged during an active
run even with a previously opened SourceCapture.

Suspended-run answer: retaining a SourceCapture opened during authorized
preparation is mechanically compatible with existing derived-input guards.
Database::require_coordinator and publish_module/publish_discovery forbid active
pure queries; they do not forbid run_active. Module/discovery adoption advances
the general revision without replacing source_revision. The existing module
registry tests explicitly establish unchanged source_revision, current adopted
source/parse leases, idempotent publication, and rejection after a real source
replacement. No guard needs weakening for a coordinator continuation of that
same prepared source service. However, SourceCapture has no Database argument:
the service must check require_coordinator BEFORE capture, not rely only on the
later publication guard. Opening/re-preparing a new capture via the CLI Type or
configured SDK entry remains refused by require_source_preparation while
run_active. Do not use a retained handle as an excuse to re-enter those cold
entry points or invent a foreign source root.

This establishes possible immutable module-record adoption, not complete runtime
resumption. CLI::run_source currently turns RunPoll::NeedInput into an explicit
unsupported result; no runtime input-driving service loop exists there. A
RunMachine also retains immutable CompletedSourceEnvironment/source/body leases;
adoption does not replace that environment or authorize a new callee/source.
RunMachine::poll_runtime_macros_inner still requires installed source_revision and every
active body lease current. A reached trait attempt must revalidate/reprepare its
actual memo/packet dependencies after relevant derived publication. The missing
ordinary runtime source/call continuation must preserve the suspended frame and
use those actual leases/attempt owners; capture alone cannot certify it. A
configured-role service additionally needs its own genuine suspended-run
publication contract before any role refresh could be legal: the existing DB
run_active guard directly refuses it. An already-open exact owning facet capture
does not establish that missing publication contract. Do not relax that guard
or call finish_run/begin_run around a suspended machine as an unproved workaround.

Unchanged official physical-module witness:
`tests/ui/modules/mod_file_with_path_attr.rs` and actual `mod_file_aux.rs` are
already imported. The real `#[path = "mod_file_aux.rs"] mod m;` requires the
declaration's stored request and caller directory; deriving a file from the
module spelling m would be wrong. Root SHA256
72913e13c2c2f7e2d357f3de4bc596378df4063fb1ae6bbbf114025e02b108c5;
aux SHA256 aac4a03551431eb21fa2bd06a5b26b5e21cf70d174d634758692fd4193387820.
This is a physical-source contract witness, not a claim that its unchanged
root's imports themselves force NeedModule. A separate LRR route control should
exercise a retained glob-demand, nested ancestor staging, repeated resume,
source/epoch invalidation and an actually suspended machine without modifying
the imported original. No such control was run in this review.

Source fingerprints inspected:
- service/source_preparation.rs 2006dafa9fedff8546e3d79c3162cfe47b121c954da6bd83e345cb55914d910f
- service/source_capture.rs 34cdf5be7d619c036547d5bda19780c356f48bff81de61264801e6e956db5061
- service/expansion.rs 9074e917c751f03f230084c30af083b9cb0d62c897aa5d052c287f1d7240798f
- frontend/module_aliases.rs 00ebd653c3b0b3f752e5e05665abfaff9eb594a160cab00e102f51e22609d5ef
- frontend/source_module_declarations.rs 81145d5fccea261fc2c19f5792c3329fc3039c739a51b622f689ac640ac01d27
- expand/source_discovery/compiler_preparation.rs 1f883613f3299d510f3bebc8c02d149dff1d88a24cf515b86019ebee4c194363
- expand/source_discovery/original_modules.rs 74cb506ea69c54d7bc0a465a6f19e799a7c42cfdfd16025ae3fa352dc0a955e9
- expand/source_discovery.rs 7a9b5b5ea7da8c5c8cfa401150665639d460949c6528e3c9459ef6b6d50fc428
- db/memo.rs 908e9c8d06df2823e027429a6547244a77999433cc9886045db7f4319ee734bb
- db/macro_export_inputs.rs 2b13deddf2830630bda522b7e1137d05af4132138c6072eb71c8a84e253ca806
- cli/macro_export_modules.rs e3bf475bc3f120d99b8f5d77aedcee130a5c23c59e07bf0acca6fc0170f6654b
- engine/run.rs 1942c42579b40fa8a8ad277926a68bd5de9d90363712eec95c33f481757c19c0

## V532 frozen precode: bounded ordinary service continuation

Production remains frozen. Proposed next slice only: service/source_preparation
and a tightly owned helper there, plus one minimal accessor in the existing
source_discovery/compiler_preparation child. No root/solver/numeric code.

Proposed service entry consumes actual SourceCompilerPreparation (rather than
accepting an arbitrary demand/producer) and either completes ordinary discovery
or returns an opaque held continuation with its real preparation state and typed
reason/input. Resume takes that same continuation and same PreparedCompileInputs;
it cannot replace the compiler catalog, source graph, environment or epoch. The
continuation privately retains the existing ExpansionCoordinator/DemandContext,
phase index and producer -> exact InputRequest map. Factor the existing
discover_with implementation into a common internal driver with that state;
the old discover/discover_with wrappers still create it and use the same driver.
The new entry first resolves the retained compiler-configuration phase, then
enters that common normal driver. No second capture, completion or macro engine.

Retaining this coordination state is necessary, not an extra certificate layer.
After pre-configuration staging, the original parent producer remains pending
until ordinary namespace completion. Dropping the request map and starting the
old discover_with afresh would make its Incomplete branch capture that same
module again. The common driver must inherit the real requests/phase state, and
held/resumed preparation must preserve them. Source revision, owning prepared
generation/environment and discovery epoch must still match before each effect.
Call db.require_coordinator before capture, not only on eventual adoption.

Minimal discovery API needed before implementation:
`prepare_pending_compiler_ordinary_module(db, owning_unit, authorities)` returns
QueryPoll of an internal typed enum: Capture { caller: SourceSelection,
input: InputRequest } or Held(reason). It reads only its own retained
pending_compiler_module_demand, joins actual unresolved ordinary declarations
through ModulePreparationDemand::declaration, and excludes already-staged
producers using the existing module_source_progress(producer).is_some() evidence.
The compiler_preparation child can share private ordinary_module_binding and
original role bookkeeping; there is no need to expose a ready bool or permit a
raw caller-selected demand/producer. Before Capture, classify foreign unit and
configured macro-export source role as typed holds, check actual parent DB
selection/capture ancestry, and use the existing prepare_ordinary_module_source
and ordinary_module_capture_caller construction. Reuse/refactor the same parent
currentness check currently in ordinary_module_syntax_input, not a looser copy.
No declaration or blocked already-staged dependency remains held. Invalid source,
ambiguous declaration or changed DB parent remains an actual diagnostic.

Service effects then follow the unchanged issue/start/accept/adopt/stage path.
After staging, consume resume_compiler_preparation before any normal poll. If
another real pending module appears, process that source-owned declaration; if
no new physical producer exists, return Held with all actual state. Do not mark
parse_job complete or call complete_ordinary_module_namespace to manufacture
configuration readiness. Normal discovery subsequently owns parsing, expansion,
namespace completion and final immutable publication. SDK role publication stays
held under its existing active-run guard.

Witness remains unchanged mod_file_with_path_attr.rs + mod_file_aux.rs above;
its actual stored #[path] must drive capture. LRR-only controls outside originals
must cover pre-poll missing-module demand, duplicate resume without capture,
nested staged ancestors, exact caller/#path, foreign unit/SDK role typed holds,
changed parent/revision before capture, pure-query refusal before I/O and cold
function bodies. A successful metadata/control result would not establish the
unchanged main or suspended VM adoption contract. No tests/Cargo ran in precode.

All-eight planned-source review: (1)-(3) extend the existing source coordinator,
not evaluator/macro/numeric selection; (4) ordinary original/generated module
ownership retains existing phase-specific routes and genuine holds; (5) only
source headers/module namespaces are prepared, no pattern/body static checking;
(6) retained continuation/request rows prevent repeated capture/planning on
resume; (7) exact demanded declaration/current parent, no new dispatch namespace
census; (8) share existing source/parsed handles, no BodyIndex/file-scan copies.
Exact implementation fingerprints and separate source review remain required
after root releases V532 and code actually exists.
