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
