# M020 source-fact and pinned-fulfillment review

LOCAL-049 / WT05-02. Independent read-only source pass after checkpoint
`edd7f85ccc8e158b20cfbc02100cd1f263b06c49`; this review document is the only
owned write. No Cargo, guest execution, fixture mutation or source snapshot was
started. Final integrated source fingerprint and runtime evidence remain pending.

## Pinned fulfillment component

Inspected `infer/fulfillment.rs`, SHA256
`03ff24c31c25e9a5e04a675296cc4afbb06b5068db5784b0b6659b9a4895fc9d`,
and `tools/upstream_fulfillment.py`, SHA256
`a199b46f7dc7ab0f93a1b635c1e61129c03c4dfce8d6cd3430101be683f58758`.
The exact original is pinned
`compiler/rustc_trait_selection/src/solve/fulfill.rs`, SHA256
`b847f09571a9b182c1456ea0c3fc9ccae43457ac7b12f01f1d28c0573e06ad7f`.

`FulfillmentContext` retains one borrowed actual `SolverContext`. Registration,
evaluation and residual draining enforce the original snapshot count. Actual
upstream `compute_goal_fast_path`, `goal_remains_stalled` and
`evaluate_root_goal` supply policy. The port retains the returned predicate,
`HasChanged` progress loop and exact pinned `ThinVec::retain_mut` behavior.
The original configured `value_within_limit(depth)` comparison is `depth <=
limit`; translated overflow `depth > limit` has the same boundary. Its diverging
compiler diagnostic becomes a typed terminal overflow; later roots remain
unevaluated and processing cannot resume. No new count or resource cap appears.
NoSolution preserves the actual obligation. Residual ambiguity retains actual
`GoalStalledOn`/`MaybeInfo`; overflow is not ordinary ambiguity or success.
Pending-variable filters use current shallow resolution and sub-unification
roots, preserving the pinned conservative behavior.

Command `/opt/homebrew/bin/python3.12 tools/upstream_fulfillment.py` terminated
0 and printed `Pinned root fulfillment projection matches; source
admission/integration/acceptance remain separate.` This is a source projection
audit, not Rust compilation, root execution or a language-fixture pass. HIR/MIR
borrow-check replay, compiler coroutine draining and diagnostic derivation are
explicitly excluded from this runtime component. Actual source-fact admission,
root-active certification, response constraints and guarded runtime commit are
not supplied by the adapter.

## Source-fact bundle preliminary finding

First inspected `source_fact_bundle.rs`, SHA256
`aaeb2c39beae9e31ba45f0f07ba0fe5f460fe4f5ee8a1258499aa55fca135ce5`.
`SourceFactBundle::prepare` follows actual source metadata and complete demanded
`ImplTraitCatalog` buckets. Query-specific pending plans remain retained; source
availability does not remove candidates or certify an empty bucket. Caller,
callee, input Types and explicit candidate-read demands seed metadata discovery.
The set traversal supplies no semantic candidate matching or cycle solution.

The first pass found a guard omission before root activation: the final retention
pass captures `bindings.generics` but not `bindings.projections` Q77 roots.
`SourceBindings::prepare` directly fetches `TraitAssociatedTypes`, whose actual
member/blocker/generated-source dependencies need retained input guards when
those bindings are outside a containing memo query. Retaining only generic-Type
guards does not explicitly certify that lookup receipt. The owner and lead were
notified. This is a source finding, without
an executed stale-source counterexample or runtime regression claim.

The owner corrected the retained-root pass before validation. Re-reviewed final
source SHA256
`65f8cce2b8a627df7bda5f8a0b6c8411823fa5e76dff80ba41901558b50ff922`:
the `source_roots` map now retains actual generic-Type and Q77 projection query
values under their exact query keys, rejects inconsistent expected Arcs,
captures each memo's input guard and verifies current Arc identity. This closes
the identified guard omission by source inspection. Q77 invalidation execution
remains untested. The later `definition: None` pending attribution is accurate:
SourceBindings retains no exact owner for its pending reason. It cannot attribute
the hold to the first already-ready definition in that map.

## Cold candidate publication and first factory pass

Inspected `prepared.rs`, SHA256
`e48c76ec05e4b948b2e9cff697ca530da2177fe2b0b70ac0ba0460c87331eac5`.
`PreparedProvider::publish` still validates source receipts, actual generic
values, same complete membership publication, raw/canonical identities, actual
head/trait membership and complete demanded bucket coverage before
`CandidateIndex::prepare`. Classification uses pinned `simplify_type` and
`TreatParams::InstantiateWithInfer`. The only prepublication source callback in
that pinned function is `trait_is_auto` for an actual dyn principal after Pat
unwrapping. The new `CandidateClassificationInputs` scope admits exactly those
known automatic flags in the same Cx. Missing flags return preparation Err;
they never select a blanket or default answer. Every derived entry is compared
against the complete ordered actual bucket before facts freeze. Coarse `ready`
no longer constructs indexes and still supplies no exact root certificate.
`DefinitionReceipt::clone` shares immutable Arc source handles and grants no
currentness or entry authority. No compilation or dyn classification ran here.

First inspected `source_factory.rs`, SHA256
`a6369c1e04ae8995996d618b387903a047455870b5e94ccccbb218b8f611d2b0`.
`materialize` maps scalar query availability independently, preserves unknown
opaque/const/dyn domains and consumes authenticated Q86 null clauses/Q89 pinned
outlives values. Q80/Q81 explicit source rows are not promoted to complete
compiler `clauses_of` answers. Actual language classifications populate reverse
queries and only observed positive forward roles; absent roles remain missing,
not invented None entries. Actual ADT declaration/repr/schema payloads stay
separate from field/Copy/destructor/sizedness semantics.

The first factory pass found a missing `StableEncode` import for
`configuration.fingerprint()`, reported to the lead before any build. This is a
source-level mechanical observation, not a Cargo diagnostic. Root callback
domains such as associated Types, alias layouts and variance remain unprepared.
`MaterializedSourceFacts::unprepared` is a diagnostic subset, not a complete
query-domain list: pending scalars and untouched slots are not all appended.
Its emptiness must never become admission; actual QueryFact domains remain the
authority. Nominal type construction uses `adt_def`/`new_adt` identity handles
without prepublication `adt_info` reads. Actual factory invocation, publication,
root callback closure and runtime consumption remain unexecuted.

The lead corrected the missing trait import and added only a cfg(test) observer
over the production bundle/factory/publication path. Observer source fingerprint
`f2deb529fbcc270992f701da27753e2225261576217bf4b74eb9db9fd2ec0e47`
retains bundle guards outside the IR context, derives counts from the actual
materialized fields, invokes real `PreparedProvider::publish`, and observes the
existing coarse `ready` refusal. Its `root_held` field reports that coarse
refusal, not an executed exact-root admission check or activation proof.
`input_enum_reprs` derives original input nominal IDs only when the actual
published ADT discriminant field is Known(Some). No helper evaluates a goal or
constructs an alternate fixture fact table.

The reviewer was then assigned the existing
`upstream_projection_types.rs` file and added exactly one new owning-2015,
original-source `repr128` body. Test source SHA256
`26e33bfb28a587769ac94cde009629272cdd367584adae1f274a19eaf81cc9ce`.
It independently obtains actual main/discr/PartialEq/Signed/Unsigned identities,
compares the materialized implementation count with complete Q65 membership,
requires one covered bucket, observes null/outlives/ADT values and exact input
enum IDs, checks actual source-memo Arc/fingerprint reuse and cold executable
queries, then withdraws source directly before checking the retained guard.
There is no prior epoch mutation in this withdrawal check. The source-factory
selection contains only that exact test body. All count assertions are intended
requirements until execution; no test has run at this point.

The Q77 `definitions()` getter is pending-gated and yields actual authenticated
type-member IDs. The bundle's new Q77/Q87 captures use the ordinary exact-key
guard map and add complete Q77 members to source/generic metadata traversal.
They do not certify full function/constant member enumeration or create empty
alias layouts for missing Q87 domains. Additional pure associated materializers
are still being written and require a separate source pass before freeze.

The associated materializer pass is now complete against
`source_associated_facts.rs` SHA256
`bead464f354d766e46037265075411afb9c53b2fc5910b3151ae9a96d84eb63c`.
`associated_types` requires genuine same-world/header/source identity, exact Q77
key/inventory, actual TypeAlias role and owning Associated parent, then rejects
duplicate IDs. Known(empty) requires Q77's complete pending-gated producer;
function/constant member coverage remains independent. `alias_layouts` maps the
authenticated Q87 domain/key and actual argument-kind order, preserving separate
inherent-constant Self/Impl namespaces. Missing domain/query/layout returns a
typed requirement, never empty arguments. Both are pure representation helpers
with no new query, parser, search loop or runtime authority. Their execution and
integration remain untested.

## Reached relation preparation

Inspected `root_relations.rs`, SHA256
`97f67443ba30b28b29688a1994df0f1033723d4c2ef9ad56cd56cd4cfc316c1b`.
`PreparedRootRelations::prepare` validates the exact original inference owner
and source variable specifications. The actual source caller proof constructs
ParamEnv; a callee obligation or raw caller-supplied environment is not accepted.
`DeclarationParameters` supplies genuine callee argument order. Missing region
or const arguments remain pending. The shared source Type encoder and actual
EarlyBinder instantiate only formal callee Types, while actual caller Types
retain their own source identity. Fallible source encoding completes before any
inference mutation.

`apply` uses the existing pinned `commit_if_ok` transaction and `RelateExt` with
Invariant for Equal. A later TypeError rolls back the complete relation batch;
all actual emitted child goals retain their real predicate/ParamEnv/cause/span.
Argument coercion is actual-to-formal; the new explicit CoerceResult transport is
formal-to-actual. Both create genuine CoercePredicate obligations for the pinned
solver. These obligations do not certify HIR/runtime coercion adjustments.
Application and output transfer require the original snapshot/owner. No parser,
macro evaluator, solver scheduler, numeric fallback or runtime cell mutation is
added. Active packet/signature/currentness, exact root callback closure,
certainty/external-constraint consumption and guarded VM commit remain separate
consumer obligations. This is source review; no relation or root has executed.

## Eight architecture modes and acceptance boundary

1. Neither component adds an assertion/expression evaluator. Source facts and
   fulfillment transport actual metadata and obligations.
2. No literal/break-only macro execution, special opcode or verifier is added.
3. Neither component chooses a macro-specific member or numeric rule. Upstream
   owns semantic fulfillment, inference and search behavior.
4. There is no origin filter; generated-source matrices remain unexecuted.
5. There is no pattern engine, whole-body preparation or unselected-arm check.
6. Fulfillment has no source callback. Bundle traversal is cold preparation;
   installed consumer caching and suspension ownership await actual integration.
7. No installed namespace census is added. The projection-guard repair has source
   review; exact currentness still requires review of the eventual root consumer
   and actual invalidation controls.
8. Source/query handles use retained Arcs; no BodyIndex cloning or whole-file
   runtime search is introduced.

These numbers are the recurring review modes, not MA001-MA008. Historical
alternate runtime routes remain named migration debt. No applicable architecture
matrix, root readiness, runtime, full-fixture, interpreter, JIT, Linux or measured
performance gate closes. The unchanged `repr128.rs` reached call must exercise
the actual source/inference/root/response consumer when installed; a toy evaluate
or synthetic WellFormed goal cannot replace that witness. A fact-factory build
before that integration is component evidence only.

## V524 frozen source review and host build failure

The separate eight-mode review above binds to frozen HEAD
`edd7f85ccc8e158b20cfbc02100cd1f263b06c49`, 8,743 inputs, fingerprint
`7e6b6ff7c63c2dfcb0b9aecf47ae2c369e476158fb476f4115cf4477a6d4ea19`.
Final factory SHA256 is
`37bf21ff511af5b4f6caf47ec5af9f7cba0f4d68ed9c30cd881a05560909a5e2`;
the final bundle is
`194f9654a34d411c33051d73ebd3e4eb1f97a5d72aac744eaba7c7d1a3e5c320`.
The factory now joins the separately reviewed associated-Type and alias-layout
helpers; failed encoding records Unprepared and its actual hold. The actual
source/common IR/publication path, removed coarse-ready index construction,
retained currentness roots and absent runtime activation remain as reviewed.
This review establishes no measured performance, full matrix or runtime result.

After explicit source/HEAD freeze and sole Cargo transfer, the validator ran
the exact maintained library no-run JSON build. Cargo session 10246 terminated
101 before an LRR source compile result: ten dependency build-script linker
diagnostics report `cc` exit69 and an unaccepted Xcode/SDK license. CLI, digester,
selector listing, component control and runtime were unexecuted. No fresh
library-test/CLI/digester binary was produced. Current CLI and digester paths
are absent; no stale binary is presented as a fresh build.

The first after-build guard requested the missing CLI and terminated1 without
writing phase evidence. The corrected source-only after-build guard terminated0
at the exact initial source/HEAD/input count, with zero changed sources and no
binary receipt. All processes are terminal. License acceptance was not performed
by the validator. The compact capsule with actual expanded commands, terminals,
source identities, diagnostic count, raw log hashes and guard outcomes was
recorded before cleanup at
[V524 host-license evidence](../evidence/2026-10-20-v524-source-factory-host-license.json).
Raw outputs remain retained at capsule creation. This is a host-prerequisite
failure, not a component source failure, runtime regression or accepted test.

### V525 exact-source retry

After the user reported the host license resolved, the lead froze the exact
same source/HEAD again. The V525 initial guard terminated0 at the V524
fingerprint/8,743 inputs/HEAD above. The library build progressed past dependency
linking and reached LRR, then session39307 terminated101 with three actual
compiler diagnostics: E0599 at source_factory.rs:105 (`Symbol::SYM_ANON` requires
an explicit trait import; the local model::Symbol shadows the wildcard trait),
and two E0271 diagnostics at prepared.rs:785-786 (SliceLike's copied iterator
received another `.copied()`). The separate source review did not establish
mechanical compilation; these actual diagnostics supersede any speculation.

CLI, digester, selector listing, component control and runtime remain unexecuted.
The source-only closing guard terminated0 against the exact unchanged source,
HEAD and input count, with zero changed inputs and no binary receipt. No fresh
binary was produced. All processes are terminal and source/target ownership
returned before repair. The capsule
[V525 compile failure](../evidence/2026-10-20-v525-source-factory-compile.json)
records expanded commands, actual diagnostics, terminal, guards and raw log
hashes before cleanup. A repaired candidate requires a new frozen fingerprint
and run; the failed evidence remains attached to this source.

### V526 mechanical repair review

The V526 initial frozen fingerprint is
`b9456389da8f98ca146af6dcf866cc35a92f13af32f724a7003e44c910860521`,
8,743 inputs, same HEAD. Comparing the actual V525/V526 manifests shows exactly
`source_factory.rs` and `prepared.rs` changed. The former explicitly imports
the inherent Symbol trait as `_`; the latter obtains the checked slice and uses
explicit `<[DefinitionId]>::iter(expected).copied()` for the same complete-bucket
comparison. These are mechanical repairs to the actual diagnostics, retaining
the reviewed source/availability/currentness/publication boundaries and all eight
review conclusions. No root or runtime path changed. V526 execution evidence
remains pending until its actual command terminals.

V526 library session9505 terminated101 with one E0283 at source_factory.rs:106:
`Symbol::SYM_ANON` now finds its trait but cannot infer that trait's Interner
parameter. The previous iterator/import errors are absent. CLI, digester,
selector listing, control and runtime were unexecuted. The source-only closing
guard session46417 terminated0 at the exact V526 source/HEAD/input count, zero
changed inputs, no binary receipt. All processes are terminal and ownership
returned before another repair. The actual command/diagnostic/log hashes and
guards are retained in
[V526 compile failure](../evidence/2026-10-20-v526-source-factory-compile.json).
The intended fully qualified constant uses the actual existing
`<Symbol<'cx> as inherent::Symbol<Cx<'cx>>>::SYM_ANON` implementation; this is a
mechanical representation repair, with no root or runtime semantic change.

## Mutable successor review before V527 freeze

This successor is separate from the V526 source. Read-only review inspected
`engine/run.rs` SHA256
`1942c42579b40fa8a8ad277926a68bd5de9d90363712eec95c33f481757c19c0`,
`engine/run_call_inputs.rs`
`7f464692a29479941f5dde1c4d39d67c2957e7cb94a73a64473eba8da75778be`
and the actual trait-call constructor in `engine/run_trait_calls.rs`
`5ff19e8d9426c00fb972f3389fcf65a5583f40b9666358b48899e561efc9ef12`.
Both actual `SelectedArgumentCall` constructors initialize the new attempt field
to None. Capture lazily allocates one Arc marker and retains the pending
selection's exact token. Clones and suspension preserve that attempt; replacing
the selection creates a different token. Retaining the old packet's Arc prevents
allocator reuse while its pointer identity matters. Uncaptured legacy calls
allocate neither packet nor token. The mutable pending-map borrow ends after
token capture before subsequent whole-machine operations.

`require_current` checks the current pending token in addition to its existing
PC/body/callee/slot/source/argument evidence. It compares the retained result
source/descriptor with a separately supplied current ordinary-seal contract.
That API still requires the future consumer to freshly derive the current
contract; a copy from the old packet cannot provide authority. Actual result
slot/phase and activation ownership, plus once-consumed commit, remain explicit
integration obligations. No source query/parser/evaluator/namespace census/body
copy is added by this patch. Costs are one cold token allocation per captured
selection and relevant immutable/source/runtime comparisons at consumption.
The actual factory test does not exercise this packet path.

The retained application change was separately read at `root_relations.rs`
SHA256 `1b5e8861f529b5f915af92ac841c0120e9948a8eeebac462d4a159fb0d455c0b`.
Private `SourceRootApplication` retains the actual encoded callee Args, proven
caller ParamEnv, actual callee DefinitionId/signature and original inference
owner/snapshot. Prepared/apply/parts transfer preserve that same application
with every emitted obligation. Checked views and getters expose no raw
replacement constructor, environment or argument setter. Equality transaction,
coercion direction and upstream obligation construction are unchanged. Source
signature/currentness, exact-root admission and runtime activation/commit remain
the consumer's authority; retaining the view does not certify them. No new
semantic solver, macro rule, body preparation, repeated source planning, global
scan or immutable body clone was found in this successor source pass. These
conclusions address all eight recurring modes, not historical MA001-MA008
closure. Compilation/execution and the final combined fingerprint remain pending.

The first `source_runtime_demands.rs` pass is bound to SHA256
`010e55030a60d7793fe6cd54a55d2af23f7d704f1d0016affd4a0f57a7dfce16`.
`EncodedRuntimeSourceClauses` encodes an actual retained Q80 once; its identity
uses that real key/fingerprint/ordinal/span. `after_relations` requires the exact
original owner/application/callee/context, inspects only original Integer/Float
source keys and their real resolved representatives, instantiates genuine
callee Args, and visits actual resolved numeric occurrences. It performs no
trait-name/width matching, projection normalization, candidate selection,
semantic graph closure, goal solving or VM mutation. General Type keys are not
relabelled as numeric originals. Output is direct syntactic incidence only;
ordinary connected constraint closure remains missing. Its DTO getters require
the consumer's explicit owner/snapshot/source-currentness checks.

The reviewer identified a specific activation hold: `prepare` invokes
whole-Q80 `bindings`/`encode` before numeric incidence selection. An unrelated
unsupported/Pending callee bound holds that whole plan before `after_relations`
can select the relevant rows. The lead accepted this as a staged source
component only. Common per-recipe demand admission must leave unrelated
unsupported bounds unforced before runtime activation; no alternate predicate
evaluator is authorized. An empty incidence selection cannot establish complete
constraint closure or readiness. Actual relation-generated obligation provenance
and a conservative pending hold are also being retained before freeze; their
successor source requires separate review. The factory control does not execute
this helper, and no runtime regression or acceptance is inferred here.

Re-reviewed the relation-obligation correction at `root_relations.rs`
`1b17566242f395b334fc164b0e0d449cb550454c41f50a7e9ee23cd34e6f3eb0`
and `source_runtime_demands.rs`
`56a69cbb11e4892af1b08d28e8521456d78a5d28880c72f605ca21c7e8aad2d0`.
The private application records the actual committed `obligations.len()` before
an Applied view can be obtained. No pre-apply application getter or public
counter setter/clear path exists. After exact owner checks, numeric incidence
returns typed `RelationGoals(nonzero)` before observing clauses. Zero means apply
emitted no deferred relation goals; it proves neither fulfillment nor root
success. Future settled-application evidence remains a separate owner-bound
requirement. This correction closes that source-provenance gap by inspection;
the whole-Q80 fragment-demand and complete ordinary constraint-closure holds
remain. No helper or runtime execution is inferred from compilation or the
separate factory control.

### V527 combined source freeze

The full successor review now binds to fingerprint
`ee0c04b3dc5c75b261863451493b5107576e3f1df7b084357f2ebd0d294903ea`,
8,744 inputs, HEAD `edd7f85ccc8e158b20cfbc02100cd1f263b06c49`.
The actual V526/V527 manifest delta contains only the three reviewed engine
packet/constructor files, root_relations, the newly reviewed runtime-demand file,
upstream module registration and the fully qualified Symbol constant repair in
source_factory. No tests or fixtures changed between these freezes. The exact
reviewed helper/packet fingerprints above match the frozen manifest. The final
factory is `1c9c99d2030cc9c784d1da5829cd57d8617b1180012cd5b9169d40b6717bc4da`.
All eight source-review conclusions and the explicit whole-Q80/ordinary-closure,
activation/result/commit holds therefore apply to this combined candidate.
Build/control evidence remains separate and pending actual terminals. The single
factory selector cannot establish execution of the newly registered helpers.

### V527 actual terminals and failed coverage control

Library, CLI and digester builds all terminated0 (Cargo43.91s,43.42s,0.53s).
Actual Cargo JSON selected test binary SHA256
`51e9f8070cf40df94833c97be5b3ee24a4f6174b9376bc8677153cbd3e8d5e28`;
CLI `7dd2f24bc7537fcaa00db9cb5e84b346bbb19a894a8234561bbd0f5f026bb396`;
digester `50c25b3a3d7efb0a1a83a3d14ba71fcd8a6365a2a7234e7b96f512463c991232`.
The dry-run verified the exact selector against that binary's real test list.
Only the selected owning-2015 original-source factory body ran. Its wrapper
terminated1/test101 after6.862s, exactly one body and unchanged inputs. At test
line598 it observed `covered_buckets == 0`, requiring1.

The observer returned through real materialization/provider publication; the
preceding minimum-definition/trait and complete-source candidate-count equality
assertions passed. That does not establish a complete typed bucket. Later
null-clause/outlives/ADT/input-enum assertions, external receipt/memo/cold checks
and fresh-source-withdrawal assertions did not execute. Exact other counts were
not printed before failure and are not reconstructed from intended expectations.
There is no component pass, actual-root invocation or runtime gain. The coverage
requirement remains enabled; pending candidates cannot be removed to pass it.

After-build and closing after-runtime source/HEAD/test/CLI guards terminated0
at exact V527 fingerprint/8,744 inputs, zero changed sources. The separate full
digester closing hash matched. All processes were terminal and ownership
returned before repairs. The durable capsule
[V527 factory control](../evidence/2026-10-20-v527-source-factory-control.json)
records exact commands, binaries, failure, unexecuted assertions and raw hashes
before cleanup. No inactive runtime path was replayed. The newly registered
packet, relation, fulfillment and incidence helpers have host compilation only,
with no execution acceptance from this control.

After that release, the reviewer added one diagnostic line before the same
test's assertions to print the actual complete observation in a future candidate.
Test source is now
`e549b7a7df2dcd2512b601b99953772d5659afa32f8c1348e36786c2341cc620`.
No assertion, selection or expected coverage changed, and no test was rerun.

## V528 source successor review before freeze

Read E-R5-008 (`docs/errata/revision5-pointers.md`) before reviewing this change.
Runtime fn-pointer admission remains permissive: `TypeKey::Function` still erases
ABI/safety and `Unifier`'s structural function arm compares parameter/result
Types without qualifier restrictions. No runtime static checker is introduced.

The explicit-reference preflight change allows actual written lifetimes to reach
the existing `WrittenTypeDeclaration::region` lookup and shared Type walker.
Actual ParamKeys/Static/Local metadata are retained; omitted lifetimes and
unrepresented binders still have typed source holds. There is no new lifetime
parser, method rule or source-origin filter. This is an actual-source head
preparation prerequisite, not a reference/runtime feature completion claim.

The optional `SourceFunctionHeader` carries actual ABI, unsafe and C-variadic
syntax. Legacy constructor terms use None (including serde-default old inputs).
The function term's stable encoding moves to v3 and substitution preserves the
metadata. Original runtime descriptors/unification still erase those qualifiers.
Actual source capture uses the existing FnPtr AST and pinned string decoding;
no ABI defaults are guessed from missing legacy metadata. No extern syntax means
Rust ABI; genuine `extern` without a written string means C, following the source
grammar. Genuine symbolic_headers capture retains its actual binder/elision.

The first review found that the runtime-oriented source_types FnPtr walk could
mark its omitted-reference Static/empty-binder erasure with Some header. The
owner corrected that before validation. Re-reviewed source_types SHA256
`e9c2b5f30dc12b26ced8d0f2d4edc83b33f700ae22690a15685d403ca2e15fdc`:
Some is withheld for direct/nested ForType and any omitted reference lifetime
in parameters, results or nested syntax. This closes the identified false
binder-evidence route by inspection; None retains runtime permissiveness while
the source adapter remains pending. No executed elision regression is claimed.

Reviewed source_terms SHA256
`a1d393455352887bb677370e8448fe16483b7454a54840c5490a20d1122c081b`.
The common Encoder's Function arm requires actual Some header, parses the pinned
ExternAbi, pushes real binder kinds, encodes every parameter/result through the
ordinary child walker and constructs genuine FnSigTys/FnSigKind/FnPtr with exact
ABI/safety/variadic and no splat. Missing metadata, invalid ABI and unrepresented
child regions stay typed requirements; no safe-Rust fallback exists. The changed
alias control distinguishes actual Unsafe+C Some from a deliberately erased None
clone, but it is outside the selected factory control and remains unexecuted.

All eight recurring modes were checked: no assertion evaluator; no generated
scalar executor; no macro/method/numeric special rule; common source-origin
route; no body/pattern/unselected-arm lowering; retained cold plans with no
installed parsing; no namespace census/global invalidation; no BodyIndex clone
or whole-file runtime search. New metadata is immutable and source-keyed. These
are source conclusions with pending combined fingerprint/build/control evidence,
not closure of MA001-MA008 or any runtime/interpreter/JIT gate. The complete
bucket requirement stays1, with every actual candidate retained.

Final prefreeze producer review supersedes the intermediate producer hashes:
source_types `4fffb1dbaf67a55a7fffb7309a2d8b000baaa279c0d03fa2feb705331f821f7f`,
symbolic_headers `a1970832966235c2756bf863cfeb3dfee77372663261ad88e3c68d186f903afd`,
term `13d65e963592e71964894f60bd7e4d0308dde34e0d74dea3f3d52b4845ec9f9a`.
Syntax owns the shared AST qualifier capture, avoiding a syntax-to-frontend
dependency. The runtime walk now withholds Some for every enclosing ForType,
including wrappers, plus descendant ForType or omitted Ref lifetimes. Genuine
symbolic binder/elision capture remains the source-complete producer. The added
ABI capacity accounting was removed; no new resource accounting is introduced.
The source encoder fingerprint and all-eight/runtime-erasure conclusions above
remain applicable. Actual combined fingerprint and command results are pending.

Final V528 prefreeze successor inspection: source_types SHA256
`fa5994b573b210c0b3b8e6ef98716717fe6b64c222747225bd81f9e385ed710f`
removes the superseded unsafe/ABI source preflight exclusion, preserving dyn,
impl-Trait, omitted-region and unrepresented-binder holds. Source_terms SHA256
`3dfa449283cfeecec6241fdd26ebae2855c21078867eead083711611e96a5569`
strengthens the existing control: the actual alias must retain Some Unsafe/C/
nonvariadic metadata and positively encode to the corresponding FnPtr; only
the separate metadata-erased clone may produce FunctionMetadata. The earlier
permitted positive-hold branch is gone. The tracked selection now includes this
existing four-edition control alongside the unchanged owning-2015 factory test.
No control result is inferred from this source inspection.

V528 captured combined source fingerprint
`5c0d1dcaed2df4370652ef44d31816e5c532a93654da9bc5ca0488364652f494`,
8744 inputs, HEAD `edd7f85ccc8e158b20cfbc02100cd1f263b06c49`.
The all-eight source review above applies to this bounded successor; tests,
root invocation, live activation and runtime contracts await their own evidence.

V528 terminal evidence: all three builds passed (lib42.05s, CLI33.19s,
digester0.55s). After-build and closing source/HEAD/test/CLI guards passed;
independent digester full hashes matched. Both exact controls selected one
body with unchanged inputs. Factory test failed101 at line599: complete bucket
coverage0, required1. Its actual printed observation is103 definitions,10 traits,
66 implementations,28 null impl clauses,65 derived impl outlives,24 ADTs,7 enum
reprs,156 holds, root_held=true. The source candidate count equality passed;
subsequent currentness/memo/cold/withdrawal assertions did not execute. Printed
values after that failing assertion's position are observations, not assertion
acceptance. All156 actual hold records are in the durable capsule:102 parent
identity,38 clause-parameter, and the remaining explicit typed requirements.
One implementation head is HeadTypes; complete relevant_impls remains held.
The unsafe-C function alias control passed0 across all four editions, requiring
the actual positive FnPtr qualifiers, separate None metadata hold and cold trace.
Original main execution, lint outputs, root closure, response/activation and
runtime remain unexecuted. No unchanged repr128 runtime replay occurred.

[V528 durable capsule](../evidence/2026-10-20-v528-source-factory-controls.json)
records exact commands, source/binary/selection/raw hashes, failure/observations,
all holds, passing alias scope and terminal guards. Four generated alias fixture
directories were inspected against the unchanged original plus external witness
and removed only after capsule creation. Compact raw logs remain for the next
source-head diagnosis; no private targets or source snapshots were created.
Source/HEAD/Cargo/shared-target ownership returned after every process terminal.

After V528 ownership release, the observer alone gained pending-head source
diagnostics (source_factory SHA256
`4390b8873c595b404da8b320cfc7c6ae83d9384d963afc995fe7783e3ac5c2c4`).
For captured pending implementation plans it obtains the already-prepared
ImplHeadTypes memo by the exact existing ImplHeaderSyntaxKey and prints actual
canonical/raw identities, retained path, self/trait SourceRanges, outer source
pending and inner ImplHeadTypeResult. No production route, fact, assertion or
complete-bucket criterion changes. No new parser or source body walk is used.
No Cargo/test was run for this successor. V528 identifies canonical7540ef8d...
as outer HeadTypes only; path and inner reason remain unobserved until a fresh
source-guarded exact control exposes this diagnostic. An alloc/string omitted
reference head is a source hypothesis, not the confirmed canonical mapping.

Post-V528 inactive relation fulfillment source review: root_relations SHA256
`5e7ca4995e8742ff8a00a680cd2348c915e1d877987549124bb776c4d532cfa3`,
source_runtime_demands
`b721073cae6416b0c5ec3722f6710f4e8a06892317b0b019d21cc0996e5490fd`.
SourceRootApplication now retains the exact RootVariables pointer alongside
owner/snapshot. Consuming transfer validates both and returns original work on
mismatch. Module-private fulfill has no call sites by source search; no source
certificate executes a root. Its actual pinned registration/evaluation/drain
keeps TrueError separate from residual Ambiguity with original stall/Maybe,
and fatal outcomes retain live context, pending state and original obligations.
Only error-free drained emptiness constructs the separate private Settled;
original goal count is neither cleared nor reused as completion proof. Numeric
selection after fulfillment validates that exact application/variables; existing
zero-goal path remains distinct. Whole-Q80-before-incidence admission limitation
remains held. All-eight review: no expression/assertion parser/evaluator, origin
rule, independent semantic retry, body/pattern preparation, installed source
planning, namespace census or immutable body clone added. Host compilation and
execution of this successor are unexecuted; no query-closure/VM acceptance.

Read-only dormant discovery split inspected at configured_catalog SHA256
`45b8ca429e54c8835d288ce63bd1184ccf1cf96694a42125bafe8909dbfb16e3`,
impl_catalog `a8c209b6c23e626f90e5dfaed8792c35797e1e97cc14350e91f3fae652a75469`.
prepare_impl_catalog_demand authenticates the existing immutable publication,
compiler/facade/source dependency membership, optional actual procedural Arc,
graph/epoch and stable v2 identity. It does not construct a name graph, start
discovery jobs or read source files; into_discovery consumes the current demand
through existing new_domain and leaves route configuration to the coordinator.
Compatibility for_impl_catalog enters that same builder with no procedural
catalog before existing compiler configuration. Complete retains fixed-point,
producer/import/frontier/compiler/source closure checks and adds exact source
dependency/procedural identity. Cold closure preparation visits captured original
membership once; this does not introduce dispatch/currentness namespace scans.
No feature/runtime acceptance follows from this source pass. No Cargo ran.

V529 prefreeze leaf/helper review (unexecuted): source_types
`4b5ab7a3503c68469906510133aad84b212fdbc77f5f6ba05a77e0d504bb4103`,
DeclarationGenericTypes
`2c697cf84241bcb575c50d86f97cfd4baa99a2935f5c4e9e82e2fe1e250a2a5e`,
signatures `31b7891a5469174addedbb618a8d4a7a5ff5877d3d1bd7764b7d1e7e3daf7373`,
parameters `fcd0a4e08b0e3562c66545f7985d0c46cab24bd8eedc293c1f20d3d9e0f5b55f`,
source_terms cfg successor
`ab19091d84ea28649b5d174d039eaeea5af95531d85aa9be4616cedae62966d8`.
Shared written_generic_argument transports actual declared Const ParamKeys and
substitution through ordinary nominal/trait/projection arguments. The new cold
impl-path arity check resolves genuine target declarations and asks their real
own lifetime count; omitted path lifetime production remains typed pending.
It does not claim reference-site capture supplies hidden path sites.

The new cfg helper's exact SourceRange lookup uses ParsedFile::syntax_at
(node_lookup.rs141): retained root-to-target descent to the outermost selected
node at that exact range, without whole-file search or reparsing display text.
The producer records whole RefType or LifetimeArg ranges and the helper checks
those same AST node kinds. Its expected counts/order are fixture assertions;
actual parameter/source layouts, identity arguments and plan.encode supply every
observed IR value. No helper-created declaration facts, anonymous identity
producer, readiness token or root exists. It deliberately invokes no ready/
solver routine. All-eight review: common Type/argument/source capture extends
existing callers; no assertion or macro executor, contextual dispatch/numeric
rule, generated exclusion, pattern/body eager route, installed parse/plan,
namespace census or body clone is added. New target-generic queries occur in
cold preparation; installed execution remains inactive. Combined fingerprint,
mechanical compilation and all three controls remain unexecuted pending freeze.

V529 captured source `8b66f6097e3a5e683c82b4415216d4ded212ca4e7d7dc873391bd2318f13be8b`,
8748 inputs, unchanged HEADedd7f85ccc8e158b20cfbc02100cd1f263b06c49.
Final ordinary source disambiguation successor retains actual TypeNS-first
lookup before ambiguous TypeArg const promotion; explicit array Value-context
const substitution remains separate, absolute/qualified paths are excluded.
The actual source-layout argument-kind guard avoids recursive generic IR table
access while materializing const parameter Types. These conclusions are source
only; earlier leaf hashes remain tied to their earlier reviewed versions.

Library build failed101 with oneE0004: source_variance_queries.rs417 omits
Parameters(ImplRegionBinders) in stable pending encoding. No successful binary
receipt was created. CLI/DG and all three controls remain unexecuted. The
source-only terminal closing guard passed0 with no changed sources or HEAD.
[V529 compile capsule](../evidence/2026-10-20-v529-anonymous-impl-compile.json)
contains exact commands, actual full compiler diagnostic and raw hashes.
No controls, runtime replay, temporary fixture projects, private targets or
source snapshots were created. Ownership returned only after every launched
command was terminal and the durable failure capsule existed.

V530 bounded successor review: the exact V529 manifest diff is only
source_variance_queries.rs. Its exhaustive Parameters(ImplRegionBinders)
encoding retains outer tag1 and adds new nested tag4, preserving the original
0..3 bytes. Fresh aggregate source fingerprint
`7b65f28a6a654849285a6ea5514101e2901d46ed4f2a085a60fc2dff3fbbe3cc`,
8748 inputs, unchanged HEADedd7f85ccc8e158b20cfbc02100cd1f263b06c49.
This mechanical fix changes no source/runtime semantics; all prior bounded
source conclusions remain scoped to their inspected deltas. Commands and all
three exact component controls are pending fresh terminal evidence.

V530 terminal: all3 builds0 (lib38.39s,CLI27.32s,DG0.57s), after-build
and closing source/HEAD/test/CLI guards0, independent digester full hash match.
All3 selectors named one actual body and retained unchanged inputs. Factory
control0 completes all assertions:103defs/10traits/66actual implementations,
1complete bucket,27null clauses,66derived outlives,24ADTs/7enum reprs, exact
input enum IDs, coarse root hold, external receipt, memo/cold/fresh withdrawal.
There are154 explicitly printed encoding holds; full records remain in capsule.
No pending-head diagnostic printed, so the old7540 path/inner-reason mapping
remains unobserved; complete demanded head coverage now passes independently.
Alias control0 proves actual Some Unsafe/C/nonvariadic FnPtr encoding, separate
None FunctionMetadata hold and cold trace across4 editions.

Anonymous-region control101 is partial source evidence only. First unchanged
trait-underscore world completes2distinct sites(265..267,273..277), source
ordinals[0,1], genuine early regions/identity args/memo/cold. Second unchanged
inherent-impl completes site87..89 and associated x inherited parent prefix,
actual rebased owner, early region/identity/memo/cold. The third unchanged
infer-defaults original fails world preparation before helper execution:
LRR-E-PARSE-BLOCKS-ENTRY at353..353, expected an item in Rust2015. Mixed
written Type/Const/anonymous ordering and encoding assertions remain unexecuted.
The diagnostic original was neither filtered nor changed. No original guest
main, native diagnostic or runtime root was executed.

[V530 durable capsule](../evidence/2026-10-20-v530-source-factory-controls.json)
records exact commands/source/binary/selection/raw hashes, actual counts/holds,
the partial failure and all guards. Four run-owned alias fixture directories
were inspected against the unchanged original+external witness and removed
after capsule creation. Compact logs remain, no private targets/source copies
were created. Every command was terminal before ownership returned. This is
first actual complete-bucket factory component success; root/query closure,
actual relation fulfillment, response/activation/once-consumed VM commit and
unchanged repr128 runtime remain held and unexecuted. No runtime gain is claimed.

## Final source checkpoint review and actual terminals

V530 fingerprint `7b65f28a6a654849285a6ea5514101e2901d46ed4f2a085a60fc2dff3fbbe3cc` over8748 inputs at HEAD `edd7f85ccc8e158b20cfbc02100cd1f263b06c49` binds the source reviewed in the anonymous-region, path-region and relation-fulfillment companion records. The last mechanical change is source_variance_queries::pending_key: explicit tag4 for ImplRegionBinders preserves old tags0–3. V530 lib/CLI/digester builds terminate0; factory/alias controls0; anonymous control101 after two real source worlds, before the diagnostic delegation mixed-world assertions. This is component evidence; no actual solver root, guest replay or response application ran.

Separate read-only source audits all terminate0: tools/bootstrap_trait_solver.py (9crates,238files,4197357source bytes,14registered Rust changes,0retained-original changes,9transmute adaptations; upstream Git object verification=false), tools/sync_upstream_inference.py (exact originals/registered storage port, no local unification algorithm), tools/upstream_fulfillment.py (pinned fulfillment projection matches). These audits do not certify source query closure or runtime integration.

Eight-mode review: (1–4) source signatures, anonymous generics and ordinary source Type walks supply the same source-bound metadata, no assertion/macro identity switch, second parser/evaluator or generated exclusion added. (5) no pattern engine or executable-arm preparation added. (6) layouts and metadata use existing pure memo queries and retained immutable source handles; no installed execution path invokes the new factory or catalog preparation. (7) candidate index construction is cold publication work over demanded complete buckets, not ordinary dispatch; exact dependencies/positive classifications retained and unknown domains held. (8) Arc source/index metadata are retained, no frame BodyIndex cloning or whole-file phase search introduced. The dormant catalog constructor and relation fulfillment are source components only; their runtime/continuation/currentness matrix remains unexecuted. Existing legacy routes and all activation/closure gaps remain held.
