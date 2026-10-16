# Retained ordinary pattern source plan — bounded prerequisite

October 15, 2026, WT03/T050/T051/H003, LOCAL-024 coordination. This is a successor
to frozen transaction packet V2, SHA-256
`90ca357f770b7d03d813ec1c95f7e940cc30c7be80aa50b565cd36e276492b44`.
That packet and its pending validation remain unchanged.

Violated invariant: `literal_match::prepare_inner` reconstructs the complete
match source plan, indexed range rows, enabled arm list and flattened alternatives
for every phase. `condition_ranges::prepare` reconstructs the same scalar/header
plan for each condition phase. A two-ended range with a false lower comparison
must also leave upper decoding cold. Primary unchanged witnesses are
`tests/ui/binding/match-range.rs`, `match-range-infer.rs`, and the transaction
frontier `tests/ui/or-patterns/search-via-bindings.rs`. General structural forms
of the last fixture remain unsupported, not component-pass evidence.

Extend the existing pure Database query/memo route with PatternPlan keyed by the
actual ordinary owning Match/LetExpr Entry PcKey. Its dependencies are the actual
DefinitionBody, DefinitionSource, ParseFile and BodyStructure inputs/results;
source edition, profile and generated attachment facts retain their existing
dependency owners. Plans retain Arc handles to immutable parsed/indexed source
and typed indexed rows, never red-node trees or deep clones of BodyIndex.
Reached endpoint decode attaches only the exact retained source range. Phase
selection consumes the immutable shared plan; it never reconstructs the full
plan. Namespace binding admission remains a separate reached proof service.

This worker owns pattern_plan.rs, pattern_tests.rs, literal_match.rs and
condition_ranges.rs plus plan controls. Root owns thunks/module glue and parser,
source/hygiene preparation. Reference peer owns all DB key/value/provider,
stable-wire/accounting and deduplicated query pin integration. Production thunk
pattern paths must consume cached plans exclusively. Direct ephemeral adapters
may exist only under cfg(test), invoking the same preparation/lowering code.
There is no context-only planner, hidden side cache or installed source query.

Cold preparation remains bounded structural work over the demanded owner, with
one checked row-sort/arm walk per memo result. Plan metadata and retained source
handles must be accounted under Section32.2, deduplicating shared parser/index
allocations. Reached decoding validates actual source identity before literal
decoding; cold upper endpoints/result bodies remain unexecuted and undecoded.
Installed transaction execution retains its prepared-op/frame-only contract.

Closure needs exact frozen source review against all eight recurring failure
modes, query reuse/currentness/metadata controls, reached-only upper decoding,
normal ordinary verifier/operation route evidence and the coordinator's current
milestone on the sole validator. No Cargo/native tests, extra agents, commits,
pushes or ledger completion are authorized for this worker. This prerequisite
does not close all pattern forms, macro repair findings or performance gates.

## Frozen worker source and integration dependencies

The seven Rust leaves are frozen in
`worktracks/evidence/2026-10-15-retained-pattern-plan-source.json`, worker packet
SHA-256 `0245b2694cca0af8558abecb0c689e5db9760a8bdabf0f0c674465e352f10b7e`.
The manifest lists exact paths, bytes and individual hashes. Earlier worker
fingerprint `bd46780caf1729acbaaa8f2252f14d66e8225c1401975c7137bbafb2f236e74e`
is unexecuted historical source: separate review found that a MatchExpr used as
an enclosing match guard must retain the existing ordinary
`require_boolean_match_guard` Entry check. Only pattern_plan.rs changed for that
correction, from `b8ec682847723296dd03e801a8f89afdfa19497f1970fa7a8cd8f3fd668fdb61`
to `4beb7b22259eebad2d2910dc39059c447ccf6933f1b1091ae68607c46010c6df`.
The new call runs once in source-plan preparation; it invents no guard rule.

Actual shared entry points are `PatternSourcePlan::owner_for`,
`ParsedFile::prepare_pattern_plan`, `PatternSourcePlan::lower`,
`literal_match::{plan,prepare_inner}`, `condition_ranges::{plan,prepare_inner}`
and `ScalarTestPlan::{from_source,first,upper}`. The provider joins them in
`db::pattern_plan_provider::thunk`. `owner_for` normalizes only Match,
MatchArm/alternative and indexed LetExpr owners; an arbitrary source expression
inside a match result cannot enter the pattern planner through ancestry.

The reference peer owns the necessary QueryKey/QueryValue tag32 registration,
`source_plan_provider::pattern_plan`, normal Thunk provider call, query pin and
shared allocation accounting, wire classifiers and supplementary cache tests.
Root owns public module/re-export glue and removal of obsolete production thunk
fallbacks. Those dependencies must be included in the coordinator's exact
combined freeze; this worker packet alone is not buildable integration evidence.
Their final source fingerprints and combined validation remain coordinator-owned.
Root's subsequent BodyIndex range-lookup prerequisite is separately identified;
this packet requires no source changes in that disjoint slice.

## Separate source review against all eight failure modes

This pass inspects the frozen worker bytes above, the shared provider call/API
join and root's production fallback removal. The numbered modes below are the
AGENTS architecture modes, not the separate MA001--MA008 audit identifiers.
Existing audit-specific alternate macro evaluators are unchanged repair debt.

1. **Assertion/expression evaluator:** both ordinary owners lower their retained
   plans to the existing normal verified recipes. The new provider helper never
   executes guest expressions, creates an operand function, tests a macro name
   or chooses an assertion-specific operation. Existing unrelated macro-specific
   evaluators remain held; this source pass does not close those audit findings.
2. **Literal/break-only generated execution:** no generated dispatcher, index,
   parser, verifier namespace or operation was added. Plans retain the actual
   ParsedFile and BodyIndex from the same provider queries for original and
   admitted generated source. Tests of all applicable macro origins and standard
   macro nesting still need actual execution on the combined source.
3. **Context-specific inference/member rules:** source hints retain their prior
   ordinary local/parameter/primitive authority and now prepare once. The
   immutable plan cannot mint a parameter, binding namespace certificate, type
   authority or method selection. Reached plain-binding admission remains
   separate through ScalarPatternBindingAdmission and the existing namespace
   witness. Its internal source/name proof preparation costs remain held debt.
4. **Generated-source rejection:** the new planner contains no generated_origin
   exclusion. Intrinsic scalar syntax follows the same retained route. The
   existing plain generated identifier binding namespace and generated Type-name
   proof refusals remain explicit gaps; no spelling-based replacement or pass
   claim is made. Wrong body/profile/owner/phase queries fail explicitly.
5. **Duplicate pattern machinery/eager cold arms:** one ScalarTestPlan stores
   typed exact endpoint ranges and one shared transaction operation still owns
   test/stage/preview/finish/retry. Match source preparation stores typed rows,
   header shape flags and deferred unsupported outcomes without lowering result
   bodies. It inspects token families/suffixes for source constraints, not literal
   magnitudes. `first` and `upper` attach and decode their reached endpoint only;
   a false lower comparison cannot decode the upper endpoint. No second cached
   scalar semantics or context-only evaluator exists.
6. **Repeated source planning:** normal Thunk preparation fetches PatternPlan at
   the actual owner Entry, then selects retained sorted rows. It never calls the
   complete match/condition plan constructor per phase. The obsolete production
   direct-scalar-binding provider branch and thunk wrapper are removed by their
   owners. Pure direct test adapters are cfg(test), invoking the same source
   constructors and lowerers. Installed Pattern/PatternRetry execution remains
   frame/store/prepared-PC only. Source fingerprints, row sorting and structural
   walks occur once per plan memo computation; rebuilding after relevant
   invalidation or eviction is ordinary authorized preparation.
7. **World scans/unrelated invalidation:** the new query depends on existing
   DefinitionBody/DefinitionSource/ParseFile/BodyStructure results; the index
   already depends on owning signature/declaration context/cfg. The helper
   performs no namespace-map construction, whole-index comparison or full-index
   fingerprinting per phase. Binding namespace currentness remains the existing
   reached service, not part of the immutable structural plan. Unrelated input
   publication reuse has an explicit external control, pending execution.
8. **Immutable copies/node searches:** plans retain Arc clones of the actual
   immutable parser/index, never deep copies. Match arm/alternative and owner
   continuation rows use checked sorted Vec storage with binary phase selection.
   Scalar endpoints retain numeric source ranges and attach only their exact
   source location through syntax_at, with no whole-file search or AST red-node
   retention. Owner lookup walks only bounded indexed parents. Root's existing
   binding_let_statement direct source attachment is reused during cold source
   preparation; further id_for range lookup work is root-owned.

The same source review checked entry/continuation authority: query keys require
exact normalized Entry; lower checks actual indexed owner/body and valid phase;
the query provider supplies current source dependencies. Allocation addresses
serve accounting only. Equivalent memo recomputation may produce a new Arc
allocation without semantic invalidation, so the helper intentionally checks
selected source/body/profile identities and uses dependency fingerprints rather
than Arc::ptr_eq or a repeated index hash. A changed real body input invalidates
the plan. Root result-source and transaction cleanup authority from V2 remain
unchanged by these seven leaves.

## Costs, evidence and remaining acceptance

Source-derived costs: one demanded Match plan performs the existing O(N log N)
body-row sort plus bounded pattern/arm structural work, one index fingerprint
and one checked retained selection-table sort. Subsequent phase selection uses
O(log A) retained row lookup, bounded indexed owner ancestry and a constant
number of exact syntax attachments for reached endpoints. Conditions retain
fixed typed rows/hints once. These are source-derived bounds, not measured speed
or scaling results. Unsupported general structural patterns, reference/@ binding,
non-Copy guard previews, let-else and complete generated binding/type authority
remain held prerequisites.

Metadata: match plans account actual retained Vec capacities and deferred
diagnostic heap with checked arithmetic and a 32MiB limit. Condition success
contains only fixed inline typed fields, scalar ranges and None/Primitive/
DefinitionParameter hints; size_of PatternSourcePlan includes that payload.
Error/disabled payloads account diagnostic or verified-recipe heap. Shared parser
and index allocations are excluded from the plan's own bytes and must be charged
once by the peer's deduplicated query/root pin accounting, including after query
wrapper eviction. No new unbounded handle table or hidden cache is introduced.

Three new `db::pattern_plan_provider::tests` bodies exercise unchanged official
match-range-infer.rs owners across all four editions, one-time query identity and
no endpoint decode during structural preparation, one decode per selected
endpoint, common conditional adaptation of the actual upstream pattern,
normalized phase/body rejection, unrelated publication reuse, equivalent index
allocation and changed-body invalidation. Existing service route controls still
cover handwritten/user/wrapper generated source, false guards, cold results and
effects. The conditional adaptation and deliberately inconsistent changed-body
negative control are external LRR controls, never full stock runtime receipts.
Reference peer adds cache eviction/accounting controls from another unchanged
official original. Standard macro, panic/cleanup suspension/resume, full fixture
execution and milestone requirements remain unexecuted gaps for this successor.

Executed worker checks: rustfmt --check on the seven owned Rust leaves and
git diff --check, both exit0. No Cargo/Rust/native test was run. Sole-validator
plan on exact combined frozen bytes: `cargo test -p lrr pattern_plan_provider`,
`cargo test -p lrr source_plan`, the prior transaction/wire/range/condition/service
filters and xtask guard/binding/scope controls, then the coordinator's milestone
gate. Actual terminal results must remain attached to that exact combined
fingerprint. This is a source-only prerequisite handoff, not feature closure,
runtime acceptance, full fixture compatibility or performance acceptance.


## Root integration review: cached production entry

Root connected the public opaque `syntax::pattern_plan::PatternSourcePlan` type
and its thunks child module. Direct match and selected condition preparation in
`initial_thunk_admission` / `initial_let_condition_thunk` is now test-only; a
production call that misses the existing memo provider refuses explicitly rather
than reconstructing a plan. The worker owns the same pure plan constructors and
retained-row lowerers used by these ephemeral unit adapters. The reference worker
owns routing every actual pattern phase through `db::pattern_plan_provider::thunk`.

Source review against the eight recurring failures: (1–4) no assertion, macro,
method, inference or generated-only language dispatch added; (5) both adapters
use the common pattern planner/lowerer, with no additional pattern semantics;
(6) production fallback cannot reconstruct a source plan, while cached-provider
routing and dependency replay remain review/validation obligations; (7) namespace
admission remains the existing separate service, whose census debt stays held;
(8) this glue passes retained handles and adds no source/index copies or scans.
No architecture finding, runtime contract, fixture or task is closed.

Validation: rustfmt completed successfully; no Cargo or language tests run by
root. The sole validator continues its unchanged frozen source. These glue bytes
are a later source-only successor, excluded from cumulative frozen packet V3.
Exact source SHA-256 values at this review:

- `crates/lrr/src/syntax/thunks.rs`: `dedde249262f0cfed3eca2f2a30547b58e26f4db8a6be4f4c147de8d4ed96d66`
- `crates/lrr/src/syntax/mod.rs`: `0c2e4a2f55adc6ed5bc726da3cc35022a94b74fda057eb74685e1e46c050fc92`
