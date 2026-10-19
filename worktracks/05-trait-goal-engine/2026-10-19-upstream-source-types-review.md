# M020 source types and settings — bounded source review

Solo /root, baseline `35b509c7e0a732127218a33be8e343a6b23d126a`.
Reviewed V323 source fingerprint:
`d4af496333e5fc7acbe55765147263953c009f530229af14280560a947e3b7f4`.
This review is separate from test terminals. M019/M020 and solver integration
remain held; constructing shared IR is not solving a goal.

1. **No separate expression evaluator.**
   `ParsedFile::solver_crate_configuration` uses the retained pinned AST and
   existing `attributes::effective_metas`; it only captures actual root settings.
   `PreparedImplSourceTypes::prepare` consumes ordinary `ImplHeadTypes` and
   `DefinitionReceipt::observe`. Its `Encoder` translates already owned terms
   to the existing `Context` shared-IR family. It neither parses expressions nor
   evaluates guest Rust. No assertion/macro operand path was added.
2. **No restricted macro-output executor.** Actual original and generated impls
   enter the same `ImplHeaderSyntax`/`ImplHeadTypes`/`ImplSourceTypes` query route.
   There is no output runtime namespace, opcode or verifier extension. The new
   actual Type matrix includes ordinary declarations, arbitrary item-emitting
   macros and a wrapper in every official edition. Standard-macro-generated
   impl Type construction is not exercised by that matrix. MA004's wider
   generated-output execution obligations are not closed by this component.
3. **No context-only selection/inference.** `SourceParameter::from_impl` validates
   actual canonical ownership, parameter ordinal and kind. Nominal/trait IDs
   come from retained actual declaration receipts and the real source world.
   Trait argument lists contain actual Self followed by source arguments. No
   method name, macro name, runtime tag or fixture path chooses semantic rules.
   No inference variables, placeholders, defaults or solver answers are invented.
4. **No generated-source exclusion.** The same source bindings and pending
   outcomes apply regardless of origin. `FunctionMetadata` is a common hold for
   all erased function terms, including ordinary aliases; it is not an original
   versus generated distinction. Wider mixed-edition, parent generic, replacing
   attribute and arbitrary generated reference/binder matrices remain open.
5. **No pattern duplication or cold executable demands.** Configuration queries
   request root ParseFile/cfg/source identity; source Type queries demand only
   header Types and referenced declaration receipts. No new body, thunk,
   pattern, FunctionSignature or executable-arm preparation is requested by
   these constructors. Source-only controls inspect the actual query trace.
   General unused-bound and cold-branch solver behavior remains unimplemented.
6. **Prepare once.** `SolverConfiguration` and `ImplSourceTypes` are normal pure
   memoized products with appended stable tags67/68. Their source/config and
   actual header/declaration dependencies are observed by the existing database.
   The scoped encoder does no database/source queries; currentness checks occur
   before scoped context creation. Repeated construction reuses interned nodes.
   No installed operation, call or dispatch route was changed. Immutable scoped
   IR cannot escape the higher-ranked context closure.
7. **No full-world dispatch scan.** New per-head binding capture walks that
   head's owned type tree and only referenced declarations. It retains the real
   `Arc<InputValue>` catalog, and `Settings` borrows its checked `DiscoveryInput`
   instead of cloning the entire catalog. `with_context` checks the actual
   current source, cfg, epoch, root and catalog Arc identity; it does not construct
   or compare a namespace map. Future root-goal relevant dependency/currentness
   closure, including retained plans after source changes, still needs integration
   evidence before a ReadyProvider can execute. MA007 is a separate macro-catalog
   audit obligation, not established by these Type constructors.
8. **No immutable body copies or whole-file node search.** Parser/header/cfg/
   catalog/receipt inputs are retained through Arc handles. Declaration kind
   checks use the existing `syntax_at` locator at the actual source range. No
   BodyIndex is cloned or constructed. Query/value encodings use explicit stable
   domains/tags; no new resource accounting, sandbox or static checker is added.
   MA008's old opaque macro index/verifier path is not added or closed here.

Costs: cold configuration preprocessing visits actual root attributes; cold
binding capture walks the demanded head's type nodes and observes distinct
referenced declarations through ordered maps. Shared IR construction walks the
demanded terms and interns them once per scoped context. It is preparation work,
not guest-operation work. No scaling, throughput, memory-use or solver-performance
claim follows from this source review.

## Real gap found and kept explicit

V322's constructor guessed safe Rust ABI for `TyTerm::Function`, whose ordinary
runtime representation intentionally erases safety/ABI under E-R5-008. An alias
can hide unsafe/extern qualifiers from a direct impl-syntax scan. The actual
pinned `lint/fn-ptr-comparisons-some.rs` contains an unsafe extern C alias.
V323 removes the guessed function constructor and returns `FunctionMetadata`;
the external control appends an own trait impl for the real alias while keeping
the imported original/expected warnings unchanged. Full ABI/safety/signature and
alias provenance must join the ordinary Type service before enabling this family.
The witness does not justify changing ordinary runtime pointer behavior or adding
alias/fixture special cases. V322 green Type controls do not validate that removed
function route and are not promoted as architectural acceptance.

Upstream `TraitRef::new_from_args` invokes its actual generic-layout metadata
guard. That guard is preserved: the bridge retains real shared Args plus the
actual definition ID, named `EncodedTraitArguments`, until genuine typed source
facts can be published. It does not forge a TraitRef literal, install dummy
SourceFacts callbacks or pass a false ready state. Likewise current standalone
solver settings derive from actual cfg/root attributes and cited pinned defaults
for unsupported -Z options; future Cargo/rustflags configuration is unfinished.

Missing integration: complete source-backed DefinitionFacts/generics/ADT/trait/
binder/ParamEnv/predicate/layout/lang-item factory and transitive demand closure;
actual compatible inference/delegate; ReadyProvider/root evaluation; source-bound
selection evidence and runtime dispatch migration; removal of old semantic
consumers after replacement. Dyn/opaque/projection, inference/placeholders,
local regions, const-value type metadata and function signatures retain explicit
pending outcomes. Broad primitive/container/ref/array/bound branches that are not
exercised by these witnesses remain unverified, even when they compile.

No new distinct stock runtime gain, suite total, full-fixture/diagnostic/tooling
acceptance, complete M019/M020, interpreter/JIT gate or Linux acceptance is claimed.
Final build/control terminals and per-cohort source fingerprints are recorded in
the linked bounded evidence; earlier failed attempts retain their own sources.

Collected final V323 terminals: actual Cargo library build success (original shell receipt unavailable, not inferred); CLI16514/0; controls84136/0,8/8. Source/HEAD/test binary guards match the reviewed fingerprint. CLI build input/source guard also matches; CLI main was not separately executed. Type constructors68heads/24worlds, source settings12worlds, cfg_attr and unsafe alias holds, complete997membership and32head equivalence pass. Existing local main/12Copy runtime regressions pass; zero new distinct runtime gains. V322 earlier2/2controls/44heads and compile failures retain their own sources. Document/plan and checkpoint hygiene checks are separate evidence.
