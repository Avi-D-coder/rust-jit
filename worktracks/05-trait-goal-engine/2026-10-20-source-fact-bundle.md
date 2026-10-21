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

## Source handoff and separate source review

Candidate worker source SHA256:
`aaeb2c39beae9e31ba45f0f07ba0fe5f460fe4f5ee8a1258499aa55fca135ce5`.
Only the new module and this record were changed by this worker. No Cargo,
build, test, runtime, registration, Git checkpoint or push was run here. Parent
owns validation and publication/admission integration. The prior source
checkpoint is `edd7f85`; this candidate is unvalidated.

`SourceFactRequest` seeds caller/callee IDs, actual input Types and explicit
candidate trait IDs. `SourceFactBundle::prepare` captures exact immutable memo
roots and relevant `PreparedInputGuard`s. `candidate_catalog` retains the actual
catalog, while `candidates(trait)` distinguishes not demanded, pending complete
membership and actual complete bucket. Every actual bucket row retains its raw
and canonical source identities; unsupported impl heads/predicates stay present.
Trait IDs discovered while preparing input or predicate metadata do not
implicitly demand candidate fulfillment. Source query dependency traversal
retains exact filter keys and does not make a semantic closure/cycle conclusion.

`definitions` retains actual Q75/Q83/Q76 plans and receipts. `traits`,
`trait_clauses`, `implementations`, `own_clauses`, `hierarchy` and `adts` expose
existing typed source plans. The caller/callee Q80 hierarchy retains actual
associated parent clause plans. `generic_layouts` combines actual input,
declaration, impl-head, impl-predicate and filtered-clause SourceBindings;
mismatched or pending generic metadata remains a typed failure. It certifies
argument layout availability only. A Q83 opaque/const/parent field or a Q85 dyn
compatibility hold does not become a global bundle hold. Q90 is captured with
`fields=false`; field-dependent callbacks remain not demanded. No source scalar,
clause, optional field or candidate absence is invented.

`capture` checks memo key and cacheability, retains each key once and captures
its actual input closure. Genuine generic-Type roots created by direct
`SourceBindings::prepare` calls are also separately guarded and compared by Arc
identity before retention. `require_current` checks those guards and the exact
catalog fingerprint. Retained immutable source plans are Arc-shared; no BodyIndex
or parser tree is copied. Bundle-level query registration/caching, typed fact
publication, caller application, inference owner, demanded root activation,
response commit and runtime migration remain the parent's unfinished work.

Separate source-review pass against the fingerprint above:

1. `SourceFactBundle::prepare`/`capture` collect existing source plans only; no
   assertion/expression evaluator or alternate Rust execution is introduced.
2. `SourceDefinitionFacts::header` retains Q75's actual source identity and
   common original/generated definition route; no literal/break executor.
3. `traits`/`implementations` expose actual declaration/impl identities without
   name matching, inference, candidate matching or context-specific selection.
4. Actual generated declarations use the same Q75/Q83/Q76 services. No generated
   exclusion is added; unsupported source joins retain their existing outcomes.
5. No pattern handling, cold-arm lowering, body inference or unused-bound
   fulfillment occurs. Explicit request traits alone seed candidate buckets.
6. `capture` retains each memo key once. Preparation walks metadata dependencies;
   retained getters and `generic_layouts` do not reparse or query source. Parent
   must retain the prepared bundle across suspension rather than reprepare at
   each installed operation.
7. No new namespace/world census or installed dispatch guard appears. The
   existing complete `ImplTraitCatalog` query is the cold membership prerequisite;
   candidate preparation traverses only explicit actual trait buckets.
8. Query roots and source metadata share Arc handles. No immutable body clone
   or repeated whole-file node lookup is added; existing Q75 owns direct syntax
   lookup. Cost is proportional to retained demanded metadata plus exact
   candidate buckets, with ordered maps/sets. No measured performance claim.

These observations address the eight recurring architecture modes, not closure
of the separately numbered MA001-MA008 historical audit obligations. All runtime,
origin/edition, source withdrawal, memo reuse, suspension, exact-once and milestone
requirements remain unexecuted for this candidate.

Independent reviewer correction before validation: the first source fingerprint
retained direct generic-Type roots but omitted explicit guarding of Q77
`TraitAssociatedTypes` roots held by direct SourceBindings projections. The
source-binding retention pass now retains both query families, verifies exact
expected Arc identity and captures each actual memo's complete relevant input
closure. Q77 scope members/blockers/generated-source lookup dependencies are
therefore subject to `require_current`, including input/per-definition bindings
not wrapped by another retained QueryValue. This supersedes the prior source
fingerprint for implementation review/currentness conclusions.

Corrected source SHA256:
`1c4d161e8d390df23bc8265d452536d231c0fb1a19fc7b7d0919f3f6f0e453c9`.
No Cargo/build/test/runtime command has been run by this worker. The all-eight
review above was rechecked for the corrected retention pass: it adds only
query-root guards, no semantic route, execution preparation or namespace census.
Actual Q77 invalidation execution remains untested until parent validation.

Final source-review attribution correction: `SourceBindings` retains a pending
reason but not the exact definition that caused it. A bundle generic-layout hold
therefore reports `definition: None`; the first already-ready definition in its
map is not evidence for the pending owner's identity. No semantic path or API
shape changed. Final candidate SHA256:
`65f8cce2b8a627df7bda5f8a0b6c8411823fa5e76dff80ba41901558b50ff922`.
The all-eight source review and the Q77 guard correction remain applicable;
compilation and execution remain untested by this worker.
