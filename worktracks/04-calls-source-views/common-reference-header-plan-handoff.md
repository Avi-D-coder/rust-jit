# Retained common reference header plan — bounded architecture record

Owner `/root/common_reference_sources`, coordinator `LOCAL-024`; October 15,
2026. T014/T024/T032/M014/M017/M018, WT04-01/02 and WT10 query interface.
Successor to immutable combined43 packet
`24f51daa9c8ef0704a7b91b189ce07369a7c82ee634c38cb5c9b2d8af7e821ed`;
its exact prior evidence stays attached to those frozen bytes. This record
precedes coding. The sole validator owns Cargo/native tests. No agents, test
commands, commits, pushes or ledger changes are authorized to this worker.

The violated invariant is repeated reconstruction of immutable reference header
syntax at reached call preparation. `ClosedReferenceParameter::capture` and
`ReferenceResultContract::capture` each demand the current parser and invoke
`closed_reference_parameter/result`; those functions recapture the complete
selected function signature before selecting the exact reference node. A
parameter/result signature with several references repeats this work. Moving
that signature into a macro must retain the same ordinary plan and authority.
Primary unchanged witnesses are already imported `regions-infer-call.rs` and
`regions-addr-of-ret.rs`, official rustc commit
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`; original bytes/directives/licenses stay
unchanged. LRR reuse/invalidation/caller-isolation controls remain outside them.

Extend the existing pure `QueryKey`/`QueryValue`/`SyntaxProvider`/`Database` memo
framework with `ReferenceHeader(definition) -> Arc<ReferenceHeaderPlan>`.
The actual current function `DefKey`, DefinitionSource/Header/Context/Body and
FunctionSignature/ParseFile queries establish the source plan. A cache key is
not a global namespace map/version. ParseFile retains actual original/retained/
generated source, edition, origin/epoch/seal/currentness dependencies; pure
providers only observe those inputs and never run expansion or file/tool work.

The parser captures the exact selected Fn header once, then its parameter/result
RefType/NameRef syntax once into one immutable bounded plan, including declared
lifetime syntax and the input-reference count. It uses existing indexed exact
range selection, not a whole-file scan or display reparse. Supported and
unsupported syntax shapes remain distinguishable without preparing any callee
executable body. Phase/parameter selection borrows retained plan entries instead
of rebuilding complete headers. Ordinary source Type/member authority remains
separate and current at the existing boundary; a syntax plan is not Type or
runtime authority.

Parameter/result source proofs retain Arc handles to this same plan and its
immutable entries. Actual caller region substitutions, physical argument target
receipts, runtime descriptor/ticket ownership, mutable frame state and return
cleanup remain per-call and are never memoized in the header query. Replace the
two frontend direct reconstruction routes; their redundant direct parser
methods are deleted, and source controls also use the common memo.

Exact dependencies are the selected declaration's current source/header/context/
body locator and pure FunctionSignature/ParseFile dependencies. Type namespace
facts and actual input/ticket bindings must not enter this source-only key.
An epoch/source/header/context change invalidates through tracked dependencies;
unrelated namespace publication must not force source-plan reconstruction.
Existing current Type certificate/refusal behavior remains sound and separately
held for its namespace-census/unrelated-invalidation debt.

Use explicit stable query/value tags and deterministic plan encodings, checked
size/accounting, and existing portable-query refusal/wire classification for
source-dependent opaque plans. Add weak pins/allocation accounting under the
same retained immutable-handle framework so a source proof keeps its plan charged
after memo retirement. Independent pattern plans will use this same framework;
coordinate the shared DB files rather than create a second cache.

Cold cost: one indexed selected header capture plus bounded reference syntax
walk for a new semantic source publication. Warm selection: existing memo
currentness/dependency checks plus direct ordinal/result handle lookup; no
source header reconstruction. Runtime source/ticket binding remains its existing
per-call checked work, and installed return/cleanup execution does no source
planning. No speed/scaling claim follows without measurements. Metadata counts
plan allocations and retained handles without copying immutable BodyIndex data.

Before integration, separately review all eight modes: common call/return route
rather than evaluator (1); actual generated ordinary syntax (2); source Type
service rather than macro/method/layout inference (3); exact generated-source
proofs (4); no eager cold body/arm or duplicate patterns (5); actual memoized
header plan reused across parameter/result requests (6); selected declaration
and tracked source/seal dependencies, no full-world cache key (7); shared Arc,
exact syntax ranges and bounded retained allocation accounting (8). MA001–MA008
are separately numbered existing audit obligations; this source plan alone
closes none of their alternate evaluator removal gates.

Required controls: unchanged upstream header capture through the new query;
repeated parameter/result demands share one immutable allocation and one query
execution; unrelated namespace publication reuses that source plan; changed
source/header/context/edition/retained epoch invalidates or precisely refuses;
negative syntax entries remain actual unsupported outcomes; no BodyStructure/
Thunk/executable body work; retained handles remain charged through memo
replacement/eviction; two actual caller source regions/tickets never share
runtime evidence through the cached declaration plan. Run scoped controls,
strict checks and current milestone only on the combined frozen successor via
the sole validator. Source-only/format results are not test, runtime-contract,
stock/full-fixture, architecture, integration or JIT acceptance.


## Final bounded source handoff — uncompiled and unexecuted

The fourteen owned source leaves below are frozen for the coordinator's combined
successor. No Cargo/native execution, test pass, original mutation, commit, push,
ledger/status update or feature closure was performed by this worker. The prior
combined43 V1 packet remains unchanged. The only local command evidence is
`rustfmt --check --edition 2024 --config skip_children=true` over all fourteen
leaves (exit 0) and `git diff --check -- <the same leaves>` (exit 0).

`QueryKey::ReferenceHeader(Digest)` and `QueryValue::ReferenceHeader(Arc<...>)`
use explicit stable tag 31. `PatternPlan(PcKey)`/its opaque Arc value use tag 32.
Serde derives preserve symbolic query transport; service `query_bytes` accounts
the keys, and CLI pre-entry audit admits the header metadata query while refusing
pattern executable preparation. A header plan is parser-owned source evidence,
not a wire-decodable proof or an authority constructor.

`SyntaxProvider::execute` delegates the two keys to pure
`db/source_plan_provider.rs::{reference_header,pattern_plan}`. The reference
provider records current FunctionSignature, DefinitionSource/Context/Body and
ParseFile dependencies, including absence of a body locator. ParseFile owns the
existing retained/generated origin, epoch and current seal dependencies. The
pattern provider requires its actual owning Entry key and exact published body
handle before calling the peer's shared parser plan. It records BodyStructure,
DefinitionBody/Source and ParseFile dependencies; there is no external I/O,
publication, macro execution or tool side effect in either provider.

`ParsedFile::reference_header_plan` walks only the exact selected Fn signature
and parameter/result type nodes. The common signature consistency recapture runs
once on a cold plan miss (in addition to a cold FunctionSignature query's own
capture); repeated parameter/result requests perform neither capture. One
signature fingerprint is retained for constructing all entry proofs. The plan
stores actual RefType/NameRef ranges, parameter/result role, declared lifetime,
owning ParseKey, signature fingerprint and input-reference count. Unsupported
source shapes have retained None entries. It caps parameters at 100,000 and total
plan metadata at 32 MiB with checked sizes and typed metadata-limit outcomes.
Existing per-reference 128 KiB syntax limits remain.

`ClosedReferenceParameter::capture` and `ReferenceResultContract::capture` call
`closed_reference_calls::reference_header`, retain the same Arc plan plus its
selected entry, then perform the existing current source Type/name admission.
The old `ParsedFile::closed_reference_parameter` and
`ParsedFile::closed_reference_result` production reconstruction methods are
removed; generated-source controls consume ReferenceHeader too. Result elision
uses the plan's actual RefType input count. Both contracts keep actual caller
regions, store sessions, exact input target receipts/enclosing chains and ticket
checks outside the cache. Signature, source revision, Type namespace/seal,
source-role and runtime record checks remain mandatory.

`QueryPin`, `Database::memo_owners/cache_usage/evict_between_runs` extend the
existing retained-handle framework. Header roots and pattern roots use Weak pins;
pattern roots also retain their exact parser/index allocations. Known cached
plan-owned parser/index handles are counted independently of dependency wrappers
and weak registry rows. Allocation pointer sets deduplicate charged immutable
storage; pointers do not grant source authority. Cached ownership counts separate
internal Arc handles from external execution handles, including shared plan roots.
Retired outer query wrappers and active inner plan handles remain charged. During
between-run eviction, dropping a parser/body wrapper preserves shared storage;
the final plan owner releases its nested allocation charge. This fixes the
source-reviewed undercount where a cached plan outlived both dependency memos and
pruned weak rows. Accounting remains capacity-based estimates, not RSS or measured
allocator evidence. Its cache traversal occurs in the existing cache-accounting/
between-run reclamation route, not installed guards or source namespace lookup.

The shared Thunk provider invokes peer
`db::pattern_plan_provider::thunk` before primitive-name/current numeric/ordinary
fallbacks. The obsolete scalar-binding branch that invoked
`initial_thunk_with_scalar_binding` directly has been deleted. The peer helper
selects the actual match/condition owner, replays the PatternPlan query's semantic
dependencies and selects retained plan rows. Existing reached binding admission
and namespace witness remain separate from source-only structural planning. The
helper permits equivalent immutable parser/index memo reallocation; an allocation
pointer mismatch is not a source-authority refusal. Peer separately source-reviewed
this API alignment and production fallback removal. Root owns normal thunks' held
direct-planning refusal and the shared node-range index. Those sources must be
included and fingerprinted in the combined validation packet.

### Source-aware eight-mode review

This is a separate final source-review pass over the fingerprint below and the
coordinated APIs, not execution evidence. Recurring modes 1–8 are distinct from
the JSON audit finding IDs MA001–MA008.

1. `reference_header`, both capture methods and `run_return`/GenericEnv's existing
   result certificate route retain ordinary source call/return execution. No
   expression evaluator, assertion dispatch or value-producing query was added.
   MA001/MA007/MA008 alternate macro evaluators remain held outside this slice;
   this metadata memo does not establish their removal.
2. `SyntaxProvider::ParseFile` and the new source-plan providers join original,
   retained and actual generated syntax before ordinary source proof capture.
   They contain no literal/break macro output executor. MA004/MA005 are not closed
   by these header controls; the standard macro runtime matrix stays unexecuted.
3. `SignatureNames`, `type_name_namespace::require_primitive_name` and
   `generated_result_namespace::require_reference[_type]` still admit actual
   selected Type names. No macro identity, name whitelist or descriptor/layout
   inference enters source planning. MA003/MA006 remain separate held removal
   work; cached syntax alone does not permit primitive/runtime authority.
4. The same query covers all source origins. Actual generated NameRef range/role,
   owning header/context, origin Type seal and epoch remain required. No generated
   source refusal was blindly deleted. Existing static/str/general-reference
   frontiers remain unsupported; there is no claim that the stock macro reference
   matrix now passes.
5. Header queries perform no BodyStructure or Thunk request. Unsupported cold
   executable bodies do not affect header facts. PatternPlan uses the peer's one
   shared structural pattern plan and VM transactional route; no second pattern
   evaluator is introduced. MA002 and all let/match/runtime contexts require the
   peer's combined evidence; the header slice alone cannot close that audit gate.
6. `ReferenceHeader` retains one immutable complete header plan, direct ordinal/
   result selection and a cached input count. Old per-request signature capture
   methods and the provider's obsolete direct pattern-binding fallback are gone.
   Warm capture still hashes owned signature metadata and performs existing Type
   admission/currentness; it performs no parser/header reconstruction. Existing
   generated primitive *non-reference* result replay and namespace-admission
   preparation remain named mode-6 debt outside this source-plan repair. No claim
   covers all ordinary call preparation or all installed dispatch costs.
7. Source-plan keys/dependencies contain no namespace-world map, global namespace
   version or caller runtime data. Unrelated Type bucket publication has a reuse
   control. Existing lexical_namespace_stamps, Type-certificate source census and
   conservative packet revision invalidation remain held mode-7 debt. No safety
   refusal was removed to claim unrelated call invalidation solved.
8. Plan/index/parser handles use Arc; no immutable BodyIndex deep clone was added
   in production. Header selection uses existing indexed exact syntax ranges.
   Cache retirement tests cover the actual nested allocations, including removal
   of dependency wrappers and weak rows. Root's indexed `thunks::id_for` successor
   and peer source plan need their own combined fingerprint/review; whole-system
   search/copy removal is not inferred from this bounded pass.

Cold header cost is O(selected signature/type syntax) plus bounded allocation,
with a cold consistency signature recapture retained for authority. Warm selection
is a pure memo lookup/current dependency validation plus direct ordinal/result
lookup and Arc cloning, followed by existing per-call source Type/runtime work.
Signature fingerprint hashing is O(signature metadata), and existing namespace
admission/census costs remain. Metadata accounting is O(retained cache entries
and known nested plan handles), with pointer sets for allocation deduplication.
No release performance or scaling claim was measured.

### Reproducible controls and remaining validation

`db::memo::source_plan_tests` contains unchanged rustc regions-infer-call and
regions-addr-of-ret header requests in all four editions; same allocation/single
provider execution; no body/thunk preparation; unrelated Type bucket reuse;
source/header/context/body-locator invalidation and restoration; unsupported
slice-reference syntax with an unsupported cold body; retained header quota
accounting; parser/index ownership after all wrappers retire; cached plan-owned
roots after dependency wrappers and weak rows disappear; wrong owner/phase
refusal and symbolic query serde roundtrips. The nested allocation controls use
unchanged imported binding/expr-match.rs (prepare-only, no stock fixture run).

`closed_reference_calls::tests::shared_header_plan_does_not_share_actual_caller_regions_or_target_receipts`
continues ordinary VM execution from one Borrow to a second Borrow, prepares both
source result contracts against the shared header plan, compares their actual
source regions and rejects the second target ticket against the first receipt.
No positive caller source/ticket is fabricated. Generated direct/wrapper header
controls now also assert a single ReferenceHeader provider run. Existing source
Type shadow, foreign owner, static source, epoch, result provenance, wrong record,
wrong context and source-result hint controls remain required.

All these controls are UNCOMPILED/UNEXECUTED by this worker. Sole validator should
compile/check the combined peer/root+fourteen-leaf successor and run scoped DB,
reference, generated-source and pattern controls, then required current milestone.
Rust/Cargo diagnostics may require source corrections; no pass is inferred from
rustfmt. Keep the earlier immutable packets' actual terminals attached to those
bytes. Whole unchanged regions-addr-of-ret main still depends on ordinary shared
println/formatting expansion/library execution; the unchanged f probe is only a
runtime-body component. The all-four-context handwritten/user/wrapper/standard
macro runtime matrix, suspension/exact-once/cleanup and full stock execution
remain required unexecuted acceptance gaps. Interpreter/JIT/architecture closure
is not claimed.

### Exact owned source fingerprint

The fingerprint is SHA-256 of UTF-8 compact JSON (`sort_keys=True`, separators
`,`, `:`) over the path-sorted records `{path,sha256,bytes}` below. It excludes this
handoff to avoid self-reference. Any source change needs a successor fingerprint
and affected source review/validation again. No worker edits follow this freeze.

Fingerprint: `8535e1aa26b3496a52906fdb4861de134f8ac47928c4283c707073994e6ab011`

```json
[
  {
    "path": "crates/lrr/src/cli/preentry_loader.rs",
    "sha256": "dce92f16af852eaf1ce337cb9270f8f22256c9c56100b27db6dda34c01eed0de",
    "bytes": 20943
  },
  {
    "path": "crates/lrr/src/db/key.rs",
    "sha256": "5ef7a31b2992fbca24038c16fadc7622a2c1ecadf20d5bcb5ffb6bc06d23b70b",
    "bytes": 7366
  },
  {
    "path": "crates/lrr/src/db/memo.rs",
    "sha256": "79c8ada4bde89ea9634ff1a52ed604810b491f5784ee5cf96597b60c07d209f3",
    "bytes": 120936
  },
  {
    "path": "crates/lrr/src/db/mod.rs",
    "sha256": "f086eae89e4a9f50eb4f798036ca3414372ddd07583402c9d9ea4c72e32cfdc6",
    "bytes": 264
  },
  {
    "path": "crates/lrr/src/db/provider.rs",
    "sha256": "0da2dc900408be08d5ba7df156fc205e20e0d15e2f98e545f578d998e1026ee9",
    "bytes": 50944
  },
  {
    "path": "crates/lrr/src/db/source_plan_provider.rs",
    "sha256": "6f6cf12b04b4c73102be97d459d5817445f23e9abd01e17cd825864911f5c999",
    "bytes": 4541
  },
  {
    "path": "crates/lrr/src/db/source_plan_tests.rs",
    "sha256": "2bb1d9d0b4707b22e8d13bbaf356a0b7da9d7c928dd55baa6e7688b21bc4148d",
    "bytes": 14042
  },
  {
    "path": "crates/lrr/src/db/value.rs",
    "sha256": "f2f1e1463b8f8634dd63db9360358fe9ea6ad72fd60472864fa1e4b859e6cf1b",
    "bytes": 10235
  },
  {
    "path": "crates/lrr/src/frontend/closed_reference_calls.rs",
    "sha256": "c3abc45af2a3a0558fb50c4e47eaff19c3a4829f5e475ab65678e3b07c68b624",
    "bytes": 45112
  },
  {
    "path": "crates/lrr/src/frontend/closed_reference_results.rs",
    "sha256": "d5f1e267cc178221b3aa688a9a791b63ba081272259370b2efbe5484c855732e",
    "bytes": 11733
  },
  {
    "path": "crates/lrr/src/frontend/generated_reference_source_tests.rs",
    "sha256": "d9693fccc9659a0295e097c523c0c39559880b390137731c4486e9a1edea4a17",
    "bytes": 7339
  },
  {
    "path": "crates/lrr/src/service/expansion.rs",
    "sha256": "1ed97f69273b302fdf95f26a0840da077ed11386ccde0f31fd03e1fd0f71ade2",
    "bytes": 22056
  },
  {
    "path": "crates/lrr/src/syntax/closed_reference_signatures.rs",
    "sha256": "335da58880aadc5a3ca894c319d75507d4cce87d226fecf3e7d4ca3165a81d17",
    "bytes": 6551
  },
  {
    "path": "crates/lrr/src/syntax/reference_header_plan.rs",
    "sha256": "c2acd80a774e01e625d180b00e9cb4b3a38e622d88aaeb361cf58d0676575792",
    "bytes": 7489
  }
]
```
