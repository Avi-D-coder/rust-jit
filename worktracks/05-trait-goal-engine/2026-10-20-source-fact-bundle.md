# M020 / LOCAL-049: retained actual-source fact bundle

Bounded worker reservation: new `traits/upstream/source_fact_bundle.rs` only;
parent owns registration, typed `PreparedProvider` publication, root admission,
checks/checkpoints and runtime consumption. Existing source is inherited. No
Cargo process, target copy, worktree or fixture mutation belongs to this slice.

Before implementation: Section 53.0/53.2 requires the actual pinned next-solver
with complete relevant source candidate membership. The unchanged upstream
repr128 call still lacks source/type/inference admission; matching a hand-picked
impl or supplying empty neighboring query domains would violate that contract.
This component supplies source metadata, never a trait answer or root token.

Common entry points are Q75 `DefinitionHeaderPlan`, Q83
`SourceDefinitionSemantics`, Q76 `SourceLangIdentity`, Q85
`SourceTraitSemantics`, Q80 `SourceDeclarationClauses`, Q81 own/parent source
hierarchy, filtered `SourceTraitClauses`, Q86 `SourceImplSemantics`,
`ImplSourceTypes`, `ImplSourcePredicates` and Q89 retained outlives inputs.
`ImplTraitCatalog::candidates` supplies complete actual raw/canonical membership
for each demanded trait; requesters cannot select a subset. Retained source
`SourceBindings` and `SourceGenericLayouts::from_bindings` own genuine
per-definition argument namespaces. Caller/callee identities and input Type
terms, rather than fixture names, seed preparation.

The retained bundle must keep query-specific pending/not-applicable outcomes.
An unavailable opaque-domain field in Q83 cannot make an unrelated known bool
unavailable. Missing candidate membership cannot certify an empty bucket; missing
source generic metadata cannot become a zero-argument layout. Unsupported
clauses remain their typed source holds. Existing legacy consumers are not
removed here; their eventual removal remains a parent integration obligation.

Preparation follows only demanded source/type/predicate metadata dependencies
and retains immutable memo roots with actual prepared-input guards. It does not
lower bodies, execute guest calls/macros, invoke a static checker, introduce
semantic candidate matching/cycle scheduling or add resource limits. The source
metadata closure is ordinary set traversal; upstream owns semantic recursion.
Cold work scales with demanded source metadata plus actual demanded candidate
buckets. Retained consumers read indexed Arc roots; no parse/body clone/world
scan belongs to installed operations. The existing catalog completion remains
a prerequisite, not new per-dispatch work.

Closure evidence required from parent: compile the registered module on macOS
Rust 1.99.0, exercise complete/empty/pending catalog buckets and same-world
identity, source/generic merging, repeated memo retention and source withdrawal,
then actual owning-context publication and root consumption. Original/user
macro/wrapper/standard macro and editions, cold branches, exact-once effects,
suspension and final unchanged runtime witness remain unexecuted here. No task,
milestone, runtime, full-fixture, interpreter, JIT or performance gate closes.

Separate all-eight source review will record final file fingerprint and inspected
symbols after implementation. MA001-MA008 are the distinct audit identifiers;
this prerequisite does not claim removal of those historical macro routes.
