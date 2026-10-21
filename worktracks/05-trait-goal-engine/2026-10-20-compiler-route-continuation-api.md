# M020 compiler-route continuation: concrete next source slice

V529 production freeze; read-only API/file handoff, no implementation/Cargo/Git.
Extends the existing coordinator design only with newly inspected state/resume
requirements. Unchanged repr128 remains the witness; no root admission or runtime
acceptance follows from this metadata continuation.

`SourceDiscovery::prepare_declarative` currently returns NeedModule after
build_deferred_graph and prepare_compiler_import_graph, but before storing:

- `providers`: actual SourceDeclarative indexes with installed procedural catalog,
  macro-export role bindings, compiler facets, captured module links and original
  compiler-attribute preparation;
- `prepared_profiles`: actual PreparedSourceCompilerFacets and prepared nested
  metadata state;
- `compile_env`, `engine`, retained compiler Arc, actual procedural consumer and
  compiler-attribute reservation mode;
- computed route rows and the configuration's cache eligibility/key.

Only self's built base graph, source/work state and original module declarations
survive. Calling prepare_declarative again rebuilds those immutable indexes;
calling normal discover_with lacks the declarative environment/providers that
ordinary-module preparation/staging expects. Neither is a valid continuation.

Suggested bounded interfaces in compiler_preparation.rs + source_discovery.rs:

1. Private `PendingCompilerConfiguration` retains the above actual context once:
   `(compile_env, engine, providers)`, prepared profiles/nested state, compiler
   Arc, procedural consumer and compiler-attribute mode. Route rows are retained
   until a relevant module/source change requires the existing route builder.
   SourceDiscovery owns `pending_compiler_configuration: Option<_>`; its normal
   declarative-ready tuple remains unset until actual route preparation succeeds.
2. Add `SourceDiscovery::resume_compiler_preparation(self)
   -> Result<SourceCompilerPreparation, Diagnostic>` which consumes the retained
   phase and resumes at the common route/import graph step. It never reconstructs
   SourceDeclarative indexes, reinstalls facets or redoes annotation capture.
   The successful finalizer sets existing route_install/imports/compiler_profiles/
   declarative fields and reserves attribute candidates once; NeedModule retains
   the same private state and yields its next actual ModulePreparationDemand.
3. Add `prepare_with_declarative_compiler_procedural(...)` returning the same
   SourceCompilerPreparation; retain existing configure/with_* require_ready
   wrappers for callers deliberately demanding immediate completion. Preserve all
   actual graph/epoch/consumer/environment and expansion-ledger equality checks.
4. Internal module-only context accessors borrow actual pending-or-ready compiler
   context for capture/staging/inheritance. Reuse them in
   prepare_ordinary_module_source, ordinary_module_syntax_input,
   module_projection_inherited/stage_module_projection and
   complete_ordinary_module_namespace. They are preparation helpers, not permission
   to execute regular discovery while configuration remains incomplete. Normal
   poll and publication must reject a pending configuration; only genuine Ready
   releases the ordinary source discovery worklist.

One additional common return type is necessary. stage_module_projection and
complete_ordinary_module_namespace currently call install_macro_routes_with_modules
which calls compiler_import_graph(...).require_ready(). A second still-missing
module therefore becomes Err and self.failed, losing resumability even if the
providers are saved. Extract a typed route-install preparation sibling returning
`Installed(Arc<MacroNamespaces>) | NeedModule(ModulePreparationDemand)`, reusing
prepare_compiler_import_graph and install_prepared_macro_routes. The existing
require-ready helper can remain a wrapper. Module staging must retain its genuine
adopted syntax/provider/jobs/stamps and pending route phase on NeedModule; it must
not mark discovery failed or treat the missing route as absence. Actual error
handling remains unchanged. Source/namespace mutations stay behind existing
preflight/commit boundaries, not a parallel importer or evaluator.

Important cache requirement: `configured_key` is computed before the first
NeedModule. After adoption, module_routes/staged source dependencies differ.
Recompute configured_graph_key at actual resume/finalization (it already returns
None when module_routes is nonempty), or explicitly clear the old eligibility.
Never store the module-enriched final ConfiguredGraph under the old pre-adoption
key. This is a dependency soundness condition, not a performance optimization.

Later service/CLI owner integrates, rather than this source slice: join the actual
ModulePreparationDemand::declaration with unresolved_ordinary_module_sources;
if there is no authentic physical declaration, retain an explicit pending/error
instead of inventing a capture. Select real caller/layout, use already retained
SourceCapture + capture_module/adopt protocols, existing ordinary syntax/stage/
namespace completion methods, then consume resume_compiler_preparation. The same
PreparedCompileInputs/ProceduralRun/executor owns effects outside VM callbacks and
pure queries. A missing route may depend on another real producer; a demand alone
does not prove a physical file or grant capture authority. No CLI restart or input
universe reset is authorized.

Costs/architecture: retain existing immutable source/compiler indexes once; retry
only the affected route graph on genuine module publication. No executable body
preparation, custom trait solver, alternative source interpreter, static checker,
new namespace hot scan, BodyIndex copy or resource cap. All eight recurring modes
remain subject to the parent coordinator record's review. This slice only makes
common metadata preparation resumable; actual full-catalog sealing, arguments'
exact-once effects, procedural reuse, source invalidation and repr128 execution
need parent-owned validation. No such execution occurred here.

Frozen inspected fingerprints:

- expand/source_discovery.rs: c7a312e0c3687e45f72f9cf18621c29a220bc473678a3172257a501994d7bf47
- expand/source_discovery/compiler_preparation.rs: 3e0cf2461f8dac2b375a5ce6f4d58c7c2791acbe20fcfec3b4e05327af8d8584
- service/source_preparation.rs: 2006dafa9fedff8546e3d79c3162cfe47b121c954da6bd83e345cb55914d910f

## Next implementation reservation, before edits

Parent assigned the bounded continuation to this worker after V530 terminal,
but asked production to stay unchanged until its checkpoint commit terminal.
Current claim LOCAL-049/WT05-02 remains root-owned; assigned files are
expand/source_discovery.rs and its compiler_preparation.rs/import-route helpers.
Dormant catalog source stays unchanged unless a concrete glue requirement arises.
No CLI, service executor, root, solver, runtime, fixture, vendor, Cargo or Git work
is part of this slice.

Implement the actual configured providers/profiles once, retain a private pending
configuration phase, and guard normal poll/publication until the same route step
returns Ready. Existing module capture/staging methods may use retained actual
provider context while that phase is pending; they cannot make it runnable.
A typed route-install sibling preserves real NeedModule during this configuration
continuation, with one finalizer for routes/imports and original attribute
reservations. Existing immediate-completion wrappers remain named/held as such;
ordinary already-running module coordination interfaces are not expanded here.
The public resume API consumes the retained source/environment/compiler/procedural
context without raw replacement arguments and recomputes cache eligibility after
actual module adoption. No fake Ready flag or synthetic discovery certificate.

Remaining prerequisites: actual host service/CLI resume loop; module capture under
retained owner service; normal root/source-query closure and once-only argument
commit. Unchanged repr128 remains acceptance witness, with macro/edition/effects/
invalidations unexecuted here. All eight recurring failures reviewed at precode:
no second evaluator, generated-only route, name-based numeric/member rule,
generated exclusion, pattern/cold executable checking, repeated immutable provider
capture, hot namespace scan or BodyIndex copy. Cold cost is retained real metadata
and route recomputation only when relevant module facts change; no performance
claim or new accounting/limit. No production edit before commit release.

## Implemented continuation handoff after checkpoint 680653d

Production edits are confined to source_discovery.rs and its
compiler_preparation.rs. configured_catalog.rs remains at its prior dormant-plan
fingerprint 45b8ca429e54c8835d288ce63bd1184ccf1cf96694a42125bafe8909dbfb16e3.
No CLI/service/root/upstream/vendor/fixture/tool change, Cargo, source copy or Git
command was made by this worker.

Compile-relevant interface additions:

- `SourceDiscovery::prepare_with_declarative_compiler_procedural(self, env,
  engine, Arc<CompileInputCatalog>, Arc<PreparedProcCatalog>)
  -> Result<SourceCompilerPreparation, Diagnostic>` preserves the actual existing
  procedural graph/epoch/consumer/environment checks and yields typed NeedModule.
- `SourceDiscovery::resume_compiler_preparation(self)
  -> Result<SourceCompilerPreparation, Diagnostic>` consumes retained context,
  accepting no replacement source/configuration/authority arguments.
- `pending_compiler_module_demand(&self) -> Option<&ModulePreparationDemand>` is
  only a borrowed actual metadata demand, not a module declaration/capture grant.
- Existing SourceCompilerPreparation variants/fields and stage/complete signatures
  remain unchanged. Repeating existing preparation with exactly the same actual
  env/engine/compiler/procedural Arc identities resumes the retained phase, which
  preserves the existing repeatable-declaration-demand control. Changed identities
  reject. Existing immediate-completion wrappers retain their actual diagnostic.

Exact changed source boundaries: SourceDiscovery adds/initializes one private
pending configuration field; prepare_declarative retains actual provider tuple,
profiles and prepared nested state once, then delegates to resume. The old
inlined cache/miss/configuration finalization block moved to the retained phase.
with_compiler/with_procedural_catalog reject replacing an incomplete phase.
install_macro_routes_with_modules delegates to the new typed
prepare_install_macro_routes core; both use the same common import graph and
install_prepared_macro_routes. stage_module_projection and
complete_ordinary_module_namespace now use the phase-aware installer. poll and
publish reject an incomplete phase. No other call sites or semantic routes changed.

PendingCompilerConfiguration retains actual compiler/procedural Arcs and mode;
PendingCompilerRoutes retains cheap actual provider/module route/source/inert
handles. An unchanged demand reuses the retained route inputs/demand; genuine
module/source/inert input changes rebuild only the affected common route plan.
The original SourceDeclarative indexes, installed facets and annotation captures
are never rebuilt during resume. NeedModule inside configuration module staging
is retained as its actual typed demand, not self.failed or absence. Ordinary
already-ready require-completion route callers preserve their existing contract;
this source slice does not change the active service loop.

There is one private Ready finalizer, reached only after cached actual installed
routes or successful typed route installation and ordinary import resolution. It
checks a real namespace memo and nondeferred imports before reserving original
attribute candidates once. No caller readiness bool/token or fabricated seal was
added. Recomputed configured_graph_key becomes None after adopted module routes,
so an enriched graph cannot be cached under its old pre-adoption key. Source,
compiler/environment/procedural equality checks and publication/currentness
mechanisms are otherwise unchanged.

The existing module capture/syntax/stage methods now find the retained actual
provider tuple, but still require genuine producer/declaration/caller/source
lineage. The service does NOT yet join pending compiler demand to an actual
physical declaration via demand.declaration(unresolved_ordinary_module_sources()).
No guessed name/producer match, SourceCapture initialization/run guard relaxation,
service executor, CLI loop, source-query/root authority or VM activation is present.
A missing physical declaration or unsupported producer remains a prerequisite;
normal discovery polling cannot run until the configuration returns Ready.

Separate source review against all eight modes: (1) no expression/assertion or
second interpreter; (2) ordinary original/generated source producers still share
real provider/index/route machinery; (3) no trait/member/numeric name matching;
(4) retained actual source/role/procedural identities and guards remain, no generated
exclusion; (5) no pattern engine or executable cold-arm/body checking; (6) immutable
providers/facets/annotations are captured once and route handles retained through
phases; (7) no dispatch source/namespace work or new invalidation policy, route
comparisons/rebuilds are cold configuration only; (8) no BodyIndex copying or
whole-file executable-node search. Legacy immediate-completion callers remain
explicitly immediate; the actual host/module-demand continuation and root work
remain held. No resource accounting or new caps were added. Existing inherited
metadata measurements were only retained where they were already computed.

Cold costs: actual provider/facet capture once, shared route-handle snapshots and
actual input comparisons during explicit resume; route graph work only after a
relevant change. No mutable guest state or procedural execution occurs here. No
performance measurements, complete module-resume acceptance, M020, runtime gain,
macro matrix, interpreter/JIT or Linux gate is asserted.

Reviewed source fingerprints:

- source_discovery.rs: 7a9b5b5ea7da8c5c8cfa401150665639d460949c6528e3c9459ef6b6d50fc428
- source_discovery/compiler_preparation.rs: 1f883613f3299d510f3bebc8c02d149dff1d88a24cf515b86019ebee4c194363

Independent reviewer notified against these corrected fingerprints. Source is
held unchanged for combined parent-owned asynchronous validation after handoff.

### Existing controls recommended to validation owner

Use the existing library test filters below with the normal direct macOS/offline/
locked validation command; they were inspected, not executed by this worker:

- `actual_core_missing_module_is_a_repeatable_declaration_demand_before_installation`
  and `unchanged_complete_auxiliary_uses_same_ready_graph_as_strict_constructor`
  in expand/source_declarative/compiler_module_preparation_tests.rs: exact changed
  repeat-pending and ready/strict phase contracts.
- `unchanged_core_prelude_yields_real_capture_demand_before_compiler_routes` and
  `original_role_module_demand_rejects_wrong_preparation_and_missing_role` in
  original_module_discovery_tests.rs: genuine source/role module demand authority.
- `unchanged_original_live_modules_capture_stage_complete_and_publish_with_actual_parent`
  in source_discovery/original_module_staging_tests.rs: common module stage/complete
  call sites and actual parent/publication identity.
- `actual_type_fixed_point_proves_absence_without_preparing_executable_bodies` and
  `type_preparation_rejects_run_and_query_before_cold_capture` in CLI Type controls:
  normal compiler readiness path and unchanged cold-capture/run/query boundaries.
- `unchanged_roots_publish_real_global_catalogs_through_common_discovery_and_header_queries`
  plus `global_catalog_rejects_changed_owning_compiler_input_before_publication`:
  actual none-procedural catalog wrapper and current compiler dependency guards.
- `all_procedural_forms_expand_once_through_the_ordinary_pipeline` in CLI procedural
  controls: actual procedural configuration/ordinary macro path where available.

No existing inspected control proves pending compiler configuration -> actual
physical adoption -> successful Ready with repeated producer effects; that needs
the missing exact service join and parent-owned evidence. These recommendations
are distinct from the unchanged repr128/full runtime acceptance witness and do not
replace it. No test result is claimed here.
