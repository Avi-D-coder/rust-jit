# M020 constructor source-transport review

Reviewer `/root/wf_validation_review`; read-only baseline
`0c11e2e3aaa11513eb8860875e97c329dc23bbb9`. The checkout was clean when
the trace began. This is a source diagnosis of the separately guarded V505
runtime failure, not a new execution or accepted repair. All source is mutable
in the next wave; no Cargo/tests are authorized until the lead's global freeze.
Final source fingerprint, changed-path review and validation remain pending.

## Actual observation and route

The unchanged owning-edition-2015
`tests/ui/associated-types/assoc-type-in-struct-constructor.rs` contains
`struct S<I: Tr>(I::T)`, `foo<I: Tr>(i: I::T) { S::<I>(i); }`, and
`foo::<u8>(())`. Its V505 CLI capture terminated 2 in the executor with
`LRR-E-UNSUPPORTED`: `constructor field type requires compound source type
binding`. The complete invocation/capture identities remain in the prior
source-declaration-clauses review and runtime report. No runtime gain is claimed.

The ordinary CLI uses `cli/mod.rs::standalone_captured`/`RunMachine::start_with_discovery`
(mod.rs line 283). `engine/run.rs::start_with_discovery` installs
`RunNames::Completed` at line 716. This is distinct from
`start_with_sources`' held `RunNames::Legacy` route; the failure must not be
explained by assuming the ordinary CLI took that legacy startup.

The reached constructor classification is
`engine/run_nominal.rs::select_constructor_call` -> the existing entered lexical
Value selection (`run_value_items.rs::lexical_constructor`) or
`select_nominal_constructor_with` -> `bind_selected_constructor` ->
`frontend/nominal_constructors.rs::bind_constructor` -> `prepare_constructor`
-> `SchemaNames::bind_variant_current`. Both selection branches join the same
constructor binder. No new constructor-specific name resolver is needed.

`constructor_context(site)` reads the result slot's already present source view,
or a runtime hint converted to a source term. `bind_selected_constructor` uses
only a matching `TyTerm::Nominal(target, arguments)` from that context to request
the specialized `QueryKey::NominalMember`. An absent/nonmatching/empty context
falls back to unspecialized `bind_constructor`. The explicit `S::<I>` arguments
are not consulted before this binding: `select_constructor_call` calls
`constructor_arguments` only at line 663, after binding/shape checks have
already succeeded. The later arguments are descriptor inputs for the VM; they
cannot rescue an earlier TypeRecipe refusal.

For Completed execution, the observed compound-field diagnostic is emitted by
the unspecialized binder's TypeRecipe outcome. The capture does not record the
intermediate context value; it therefore does not independently distinguish
`None`, a nonmatching nominal, or empty arguments. The exact source-level
transport defect is the absence of explicit source arguments from the cold
binding input, not a claimed measured value of that context variable.

## Existing source/projection mechanisms and limits

The authentic argument selector already exists:
`run_owner_arguments.rs::constructor_arguments` -> `path_arguments` ->
`frontend/explicit_path_arguments.rs::select_path_arguments`. It fetches the
retained SourceLocalPlan/SourceConstraints, uses the indexed callee operand and
final/owner path segment, and selects each actual Type occurrence with the
ordinary entered Type service. Actual caller parameter `I` is an
`ExplicitType::Parameter` source term; its run-local descriptor is a separate
activation binding. A second parser or reparsing callee display text would
duplicate this shared route.

The field source term also already exists:
`declaration_nominal_types.rs::actual_field_type` authenticates the TypeSchema,
actual declaration/header/source and exact field range/tokens, then calls
`source_types::resolve_actual_type`. Its shorthand projection service retains
the actual `I: Tr` declaration bound, source parameter/trait identity and
associated-name key. That name key is not itself the TypeIR associated
declaration ID; the actual Q77 declaration join is a separate transport receipt.
This is source metadata, not a concrete runtime field descriptor.

The unspecialized route converts the term with
`types.rs::current_compound`/`written_field_type` ->
`written_terms.rs::written_template`. A top-level Projection falls through to
an inference hole, and written_template maps that hole to None. With neither a
concrete proof nor written field recipe, SchemaTypeName::into_field_type returns
TypeRecipe, which prepare_constructor retains and bind_constructor reports.

A specialization hook exists:
`NominalMemberKey` -> `nominal_member_goals.rs::prepare` ->
`SchemaNames::bind_variant_current_with_arguments` -> `argument_bindings` ->
`instantiated_query`. It can substitute actual declaration arguments before
normalizing projected field terms. However, its present
`associated_types::normalize_template` is a legacy manual MemberIndex
unique-candidate/alias-target selector. It is not an actual upstream root or
proof of pinned projection solving. Source argument transport into this hook
must remain distinguished from the pending migration to the pinned solver and
actual source facts. Do not expand that selector or create a second projection
solver to complete this witness. An unused WF root also cannot establish the
reached projection's semantics.

## Cache, dependency and architecture obligations

`NominalRunFacts.calls` is keyed by `(BodyKey, CallIdentity, resultContext)`.
A discarded generic constructor may have no result context in multiple
activations. Adding a specialized witness after reading explicit arguments,
without incorporating their actual source application into the cache key,
could reuse the first activation's witness for another caller parameter.
The immutable source argument selection may be shared by PC, while bound
descriptor/source arguments and specialized witnesses must reflect the actual
activation. Retain their relevant PreparedInputGuards and currentness closure.

The source review covers all eight recurring modes for the proposed boundary:

1. Extend ordinary constructor/source argument services; no expression/assertion
   evaluator participates in this failure.
2. Bind the same nominal schema and ordinary VM aggregate operation across
   origins; no scalar-only macro-output executor is justified.
3. Source/member/parameter identities must select the projection; neither the
   fixture's names nor primitive u8/Unit values may select semantics.
4. Use retained generated/ordinary source evidence through the same selector;
   a missing join stays typed pending, not an original-only exclusion.
5. The demand is one reached constructor and its selected field Types, not
   callee bodies, cold arms, whole-body checking or pattern machinery.
6. Share immutable indexed argument selections and pure query specializations;
   installed dispatch must not parse/reconstruct the plan per activation.
7. Add actual source-argument dependencies to the cold cache key; no global
   namespace scan or broad invalidation belongs on installed execution.
8. Retain existing SourceLocalPlan/query Arcs and direct Type locations;
   no BodyIndex deep copy or repeated whole-file syntax search is required.

The legacy startup, custom associated-type normalizer and incomplete upstream
root/member/dictionary admission remain named holds. None is removed by this
diagnosis. Costs/performance and the full standard/procedural macro, repeated
generic-activation, negative-dependency and runtime cleanup matrices are
unexecuted here.

## Bounded post-freeze validation proposal

After final source freeze, build the library no-run, CLI and digester once with
direct-checkout Rust 1.99.0 offline/locked commands. Select affected component
controls only after the actual new item-bound/constructor/numeric APIs settle.
Replay the primary unchanged associated constructor and repr128 witnesses
because their reached production paths are changing, plus the existing
generic-derived-type regression because source argument/nominal binding is
shared. Preserve complete runtime captures and distinguish the next actual
failure from source-only evidence. If new source controls cover generated
constructor transport, require real original/user/wrapper applications and
actual shared-route/cache observations outside imported originals. No broad
suite, source copies/worktrees, mutable-source tests, fake provider or readiness
relaxation is authorized by this proposal.

Before ownership return, collect every terminal, the standard source/HEAD/test/
CLI after-build/after-runtime guards and separate fresh-digester closing hash.
Rebind this all-eight review to the actual captured fingerprint and changed
symbols; retain V505's failed observation on its original source.

## Arriving transport implementation source pass

The lead's current source separates `select_nominal_constructor_target` and
`lexical_constructor_target` from field binding. Ordinary non-constructor calls
are classified and cached before any constructor argument demand. The same
selected target then reaches `bind_constructor_application`; patterns retain
the existing context-only `bind_selected_constructor` wrapper, avoiding an
unrelated enclosing call's path arguments.

`OwnerArguments::source_terms` reuses the authentic retained Type selections and
`Frame::bound_view` under this activation before constructing the member query.
Frame::bound_view prefers actual symbolic GenericEnv bindings; its existing
fallback can reconstruct a source term from the runtime binding's descriptor.
This is activation Type transport, not a proof that a trait obligation holds.
The legacy associated normalizer is unchanged. Its unique-candidate semantics
are still held migration debt, even if reached runtime behavior improves.

The old call cache is split into declaration classification by source call
identity and `call_bindings` by source call identity, result context and actual
bound explicit source arguments. Cached witnesses retain their query guard.
This addresses the repeated-generic-activation aliasing identified above.
Explicit argument descriptors are materialized only after field binding for the
ordinary VM constructor continuation. Argument holes retain the pre-existing
independently inferred result context; new partial argument inference is not
claimed. Cold field specialization continues through NominalMember rather than
a new evaluator, parser or solver. No added source planning occurs in installed
VM construction. No source-level blocker was found in this bounded pass;
compilation, final fingerprint and all runtime terminals remain pending.

The exact draft selection is
`2026-10-20-reached-constructor-controls.json`. The first actual standalone host
control runs unchanged associated constructor content as original/whole-item
emit/whole-item wrapper in editions 2015/2018/2021/2024. It uses the embedded
standard facade and real discovery/runtime entry, with no fake provider or
unused solver root. The second existing control now adds one discarded
`Projected::<T>(value)` site entered under u8/u16 with u32/u64 projected fields;
it also retains its exact-once `touch` counter and ordinary/user/wrapper/standard
assertion contexts. These host controls are supplemental runtime-route evidence;
the direct unchanged fixture report remains separately captured by the runtime
harness. Full source/epoch/currentness and required architecture matrices must
not be inferred from their successful outputs alone.

## V506 frozen source binding

The final independent source pass is bound to
`9d57fa744233942db9fcffa61d7147d97efb22d223bb843116d4e91eae610bb7`
(8,702 inputs), HEAD `0c11e2e3aaa11513eb8860875e97c329dc23bbb9`.
The captured manifest differs from V505 in 15 Rust inputs only, covering the
constructor transport/control changes and Q82/common source collector/registration.
No tools, vendor or fixture inputs changed. Final cleanup removed formatting
churn in unchanged tails without changing the reviewed transport semantics.
All eight source conclusions above were rechecked against this fingerprint;
actual compilation/control/runtime terminals remain pending. The source review
does not establish the unexecuted architecture matrix or upstream root migration.


## V506 failed compilation and closing receipts

The sole asynchronous direct-checkout offline locked Rust 1.99.0 library
no-run build terminated 101. Its one E0277 diagnostic is at
frontend/source_item_bounds.rs:537: the new stable encoder calls e.record on
PreparedScopeImplHeaders, which has no StableEncode implementation. The
read-only pass missed this mechanical adapter error; compilation establishes
that the V506 candidate is not accepted. No source repair was made by the
validator, and fresh CLI/digester builds, four controls and three runtime
observations were unexecuted.

After the library process was terminal, after-build and after-runtime guards
both terminated zero with the frozen 8,702 inputs/HEAD unchanged and zero changed
sources. Their executable receipt names only the old V505 CLI
`a3c6b5af08aedc91fda1d8671910b4ebf84bc6a3d59145c220faf68458e84498`;
no fresh V506 executable is claimed. The separate old V505 digester hash
`6e3677fd0fed33b77a1a4de7972d9449b798d60a371215c9f213c046b508265e`
also matched its prior closing receipt. Source/HEAD/binary ownership was
returned only after these terminal closing guards. Raw receipts are in
artifacts/validation-current-v506. All source/component/runtime/full-query
holds remain; any repair requires a new fingerprint and guarded attempt.

## V507 exact repair delta

The renewed source pass is bound to
`0a10a31324d0972292bd62007d6e6b6bd0c511372dd2426ad95a259fea538dee`
(8,702 inputs; unchanged HEAD). Captured-manifest comparison confirms exactly
one changed input from V506: frontend/source_item_bounds.rs records the retained
PreparedScopeImplHeaders fingerprint instead of attempting StableEncode on its
storage. This matches the existing db::value/Q78 inventory representation and
changes no recipe, query demand, source identity, argument transport or default
policy. All eight source-review conclusions carry with their explicit holds.
Build/control/runtime results are separately pending for V507.

During the frozen build, independent review found an additional source-only
edge: explicit Some(empty arguments) collects Some(empty Vec) and overrides a
nonempty result-context application before the binder falls back to the
unspecialized route. SourceConstraints::capture_path_arguments produces an empty
Type list for `::<>` and lifetime-only generic lists. A context-known generic
constructor should retain its existing contextual Type arguments in those cases.
The proposed minimal witness is `let _: S<u8> = S::<>(());` using the primary
fixture's real projection declaration; it was not executed in V507. This is a
source-derived issue, not an invented failing runtime capture. The lead accepted
the repair after V507 terminals/closing guards, with an extension of the existing
constructor control. V507's selected pass cannot close this additional edge.
No source was changed while validation was running.

## V507 terminal receipts and remaining source gaps

The offline locked Rust 1.99.0 library no-run, CLI and digester builds terminated
zero (44.31s, 38.86s and 0.66s). Cargo JSON selected the actual test binary
`target/debug/deps/lrr-df1e3f56d083c63c`, SHA256
`491d3f89646bec6d24360e2b657392e866d8f279a48729a6096bcc84761f4e06`.
The fresh CLI SHA256 is
`4e3e053864c0255d425ee32105c70ff07fb396b1a62f7abf0892d0859129fb31`;
the fresh digester SHA256 is
`b44cd7b74d811d3e422690b5a54973f3e451f51efc2ffd71ca3b9fe05d15543c`.

The first control-runner invocation terminated 2 during selection preflight,
before any body ran: the actual test registration includes
`cli::crate_graph::builtin_derive_tests`, while the draft selection omitted
crate_graph. The validator corrected only the owned worktrack selection,
verified actual binary `--list` names and preserved the failed preflight log.
The four intended bodies then ran once with terminals 0/0/101/0:

| Control | Seconds | Successful world rows | Actual observation |
| --- | ---: | ---: | --- |
| Unchanged associated constructor | 47.660 | 12 | original/emit/wrapper x four editions; 264 observed/completed operations |
| Explicit constructor activation | 24.551 | 4 | four edition runs containing all origins and repeated projected applications; 1,592 observed/completed operations |
| Q82 associated output recipes | 2.683 | 0 | LRR-E-TYPE-CONTEXT before any successful world; no passing Q82 evidence |
| Existing Q80 projected bound | 28.546 | 12 | 36 own clauses and 12 projected subjects; actual source identities/currentness |

The Q82 failure message is `local Type declaration owning declaration is
absent`. Source tracing identifies the common prerequisite:
projection_trait_identity -> WrittenTypeDeclaration::resolve_name_at ->
type_name_namespace::resolve_declaration_name_at treats an associated trait scope
with owner Some as an executable-local rib, then local_declaration_owner requires
an absent LocalDeclarationOwner body receipt. The existing certified_module_lookup
can distinguish genuine impl/trait parents via actual source rows, but that
route is bypassed by the earlier branch. This is an authentic source-name/parent
join gap (relevant to recurring mode 4), not evidence that an associated
output bound needs a function body or a new resolver. Preserve body-local owner
proofs while repairing the common ordinary/generated source namespace join.
Q82's intended cold/memo/epoch assertions were not reached and remain unexecuted.

The three-case runtime harness terminated 1 because repr128 still faults with
executor LRR-E-OP-TYPE (`operator requires supported concrete primitive operand
descriptors`). The unchanged owning-2015 associated constructor now passes all
13 capture/runtime checks, with empty guest streams; generic-derived-type also
passes all 13 and prints `10\n10\n`. Neither failed repr128 nor Q82 has a passing
check record. The report is `/private/tmp/lrr-v507-reached-constructor-runtime/report.json`;
its original/manifest/binary guard is unchanged, native_execution is false,
selected_batch_passed is false and guest_jit_allowed is false. This is bounded
interpreter evidence through existing legacy projection selection, not an
actual pinned solver root/full-factory migration or a suite/gate result.

After every process was terminal, after-runtime guards terminated zero: the
frozen 8,702 source inputs/HEAD match before with zero changed sources and
fresh test/CLI hashes match after-build. The separately captured digester hash
also matches after-build. Source/HEAD/binary ownership was explicitly returned
only then. Parsed counts are saved in observed-control-counts.json under
artifacts/validation-current-v507. No seven-port audit rerun was performed:
unchanged pinned tools/vendor/fixture inputs and the lead's affected-path scope
are recorded above. The next repair wave must separately bind the Q82 namespace
fix and empty/lifetime-only constructor argument fix, preserve these failures,
and rerun only affected selected paths with full closing guards.

## Arriving empty argument repair and affected selection

The lead's next transport delta filters an empty collected explicit Type list
before falling back to existing contextual arguments. It changes no source
selection, descriptor construction, projection normalizer or argument cache
identity. The existing constructor control adds a context-known
`Projected::<>(5_u32)` application; direct lifetime-only projected execution is
still unexecuted despite sharing the source capture rule. The separate unchanged
empty-generic-brackets-equiv.rs runtime witness will be observed without claiming
a measured baseline gain for it. No failing baseline run was authorized.

The distinct V508 selection is
`2026-10-20-reached-constructor-v508-controls.json`: two affected constructor
controls and repaired Q82. Historical corrected V507 four-selector selection
is preserved. Its passing Q80 regression remains attached to the V507
fingerprint; the common bound collector is unchanged by the repair. The Q82
namespace repair has its separate all-eight source pass in
source-item-bounds-review.md. Final repaired fingerprint and terminals are
pending the next explicit freeze; no source edits/tests were made by the
validator between guarded attempts.

## V508 frozen source binding

The final repaired source pass is bound to
`73b4f0f96a40390f67a1a07d42d57548394bc2a41c437dcb6083c13bbae03697`
(8,702 inputs), unchanged HEAD `0c11e2e3aaa11513eb8860875e97c329dc23bbb9`.
The manifest delta from V507 is exactly four inputs: builtin_derive_tests.rs,
engine/run_nominal.rs, frontend/type_name_namespace.rs and
frontend/generated_result_namespace.rs. The bound collector, Q82 producer,
pinned algorithms/tools/vendor and imported originals are unchanged. Both
independent all-eight source passes above were rechecked against this captured
fingerprint, with the recorded cold parent-row scan and unexecuted matrices
retained. The three exact repaired selectors and four runtime reports remain
pending actual terminals. No source was edited by the validator.

## V508 completed validation and final scope

All three offline locked Rust 1.99.0 builds terminated zero: library no-run
13.00s, CLI 10.49s, digester 0.62s. The actual Cargo-selected test executable
`target/debug/deps/lrr-df1e3f56d083c63c` has SHA256
`c9e066e0b8ef49fa407534d14342bd4e61c58459906b2d2d3aafa23fc1e52972`.
The fresh CLI SHA256 is
`4eba828cdab81985c0e701e2a165db180a870ebf9923c6468ceb2bc9f7ae0949`;
the fresh digester SHA256 is
`2304e69f3b3e5e5d2e858d97df5b8d76248f1b32d59c6ba81f0f281351abaa1d`.

All three exact V508 controls terminated zero, with one actual body and
unchanged source/HEAD/test-binary inputs each:

| Control | Seconds | Actual world rows | Observed evidence |
| --- | ---: | ---: | --- |
| Unchanged associated constructor | 47.892 | 12 | original/emit/wrapper x four editions, 264 observed/completed operations |
| Context/activation constructor | 23.677 | 4 | all origins in each edition, repeated generic applications and contextual Projected::<>; 1,624 observed/completed operations |
| Repaired Q82 output source | 28.552 | 12 | zero own/12 output recipes, actual parent/member/projection/span, typed default holds, cold/memo/epoch checks |

Direct lifetime-only projected construction remains unexecuted. Successful
Q82 epoch revocation does not establish independent source withdrawal or the
wrong-parent/negative/nested-local/retained-fragment matrix. Its parent-where,
nested/GAT association and compiler default/elaboration/remapping branches are
not exercised by this fixture. The source-only holds in the separate Q82 review
remain. The unchanged Q80 pass remains V507 evidence at its own fingerprint;
it was not rerun or relabeled as V508 evidence.

Runtime reproduction uses the same fresh CLI/digester, embedded facade and
owning edition selected by each unchanged original:

```sh
LRR_EMBEDDED_FACADE=1 /opt/homebrew/bin/python3.12 tools/rustc_checkout_runtime.py --lrr target/debug/lrr --source-digester target/debug/examples/lrr_source_digests --report-dir /private/tmp/lrr-v508-reached-constructor-runtime --timeout 240 --case tests/ui/associated-types/assoc-type-in-struct-constructor.rs --case tests/ui/enum-discriminant/repr128.rs --case tests/ui/generics/generic-derived-type.rs --case tests/ui/generics/empty-generic-brackets-equiv.rs
```

The selected batch terminated 1. Associated constructor, generic-derived-type
and empty-generic-brackets-equiv each passed all 13 runtime/capture checks in
owning edition 2015. Their CLI terminals were zero; no timeout/truncation occurred.
Guest streams were empty except generic-derived-type stdout `10\n10\n`.
repr128 still terminated 2 with executor/fault LRR-E-OP-TYPE, `operator requires
supported concrete primitive operand descriptors`; it has no passing check row.
The empty-bracket original's expected compiler WARN/.stderr remains diagnostic
compatibility outstanding, separate from its interpreter runtime pass. No
measured baseline gain is attributed to that case because no failing baseline
was run. There is no full-fixture, refreshed suite, Linux or JIT acceptance.

The report retains unchanged imported-original/manifest/binary guards,
native_execution false, selected_batch_passed false and guest_jit_allowed false.
Both passing standalone constructor origin controls and the direct unchanged
runtime witness remain evidence through the existing legacy associated-type
selector; no actual pinned projection root, complete source factory or new
runtime dictionary is established by these results.

After all processes were terminal, after-runtime source/HEAD/test/CLI guards
terminated zero with all 8,702 inputs unchanged, zero changed sources and fresh
binaries matching after-build. A separate fresh-digester hash comparison also
matched after-build. Only then was ownership returned. Actual parsed receipts
are in artifacts/validation-current-v508/observed-control-counts.json; source
and binary guards/logs are in that same disposable directory, with authoritative
selection/tools outside artifacts. No extra tests or unchanged pinned audits
were run, and no source edits were made by the validator. All-eight review is
bound to the captured V508 fingerprint with its named exceptions and unexecuted
matrix retained. This completes the authorized validation slice, not M020 or
interpreter/upstream-root migration.
