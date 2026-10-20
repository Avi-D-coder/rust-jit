# M020 independent GAT and variance source review

Reviewer `/root/wf_validation_review`; implementation baseline
`3d25d2bcec3dad543861a193258d99b07f968c77`. This separate read-only pass covers
the V502 source fingerprint
`a88c1b2a2a97acc4cb397d4a1524b7196bb9f79c6b42756b136eee81878c1a9d`
(8,695 validation inputs). Source changes after this fingerprint require a
new bound review and validation record. This is a staged M020 component review,
not a complete source factory, solver-root, runtime, full-fixture or JIT claim.

## Shared routes and source evidence

`frontend/source_types.rs::written_generic_argument` and
`written_generic_arguments` replace repeated Type/region argument collectors at
the existing nominal, module-qualified, trait-positional and associated-item
call sites. Type arguments use the ordinary `resolve_actual_type` walk; regions
use the owning `WrittenTypeDeclaration::region`. Trait equality bindings retain
their separate existing collector. Const argument metadata and unsupported
positional equalities remain gaps. `shorthand_projection` moves its own argument
vector into one resulting ProjectionKey: the qualified branch returns after its
single call, while the shorthand branch calls once. The FnOnce capture does not
need cloning or a second projection implementation.

The shared field route remains AdtSourceFields -> SourceBindings/Q77 actual
associated declaration -> SourceGenericLayouts actual parent-first binder ->
source_terms encoder -> pinned AliasTy constructor and trait/own split. The new
controls use unchanged `streaming_iterator.rs` Foo.bar with own `'static`, and
unchanged `issue-87750.rs` Node's recursive `P::Pointer<Node<T,P>>` field. Actual
publication IDs are selected independently from Outline auxiliary member scopes;
the two are never substituted for each other. The test helper observes source
and encoded identity, argument kinds/order, parent count, Self and own values.
The helper's Struct/Enum expectation and argument expectations are cfg(test)
observations, not production source rules.

`source_variance_queries.rs` Q79 obtains an actual DefinitionHeaderPlan and
DeclarationParameters. Authenticated total count zero precedes role dispatch;
nonzero ordinary traits and explicit associated aliases are NotApplicable, not
missing vectors or solver NoSolution. Functions, including foreign placement,
remain FunctionInference pending. DefinitionFacts no longer contains arbitrary
variance and alias-variance slices: its privately constructed completed query
handle borrows the retained source result. Publication checks actual receipt,
inventory SealKey, generic count and Known-vector length. The SourceFacts
callbacks read retained data only. The shared `opt_alias_variances` dispatch was
compared to pinned `rustc_middle/src/ty/util.rs`; explicit associated aliases
cannot pretend to be compiler-created RPITIT declarations. Its dispatch and the
source-role adapter are independently source-reviewed; the algorithm generator
does not mechanically extract those helpers.

`variance/mod.rs::prepare_nominal` follows actual demanded field dependencies,
retaining source field, language identity and configuration query Arcs. Root
crate membership and actual DefinitionId locality hold foreign dependencies;
they are never recomputed as invented local facts. `SourceLangIdentity` receipts
provide only the exact three pinned override roles. The typed adapter checks
actual source binder identities for Type, region and const parameters, not just
matching numeric indexes. Unsupported foreign variance, free-alias expansion,
signatures and other unproduced shapes stay typed pending before algorithm entry.
The terms, constraint walk and fixed point remain the complete registered pinned
port. Demanded closed nominal enumeration replaces HIR crate enumeration;
functions/constructors and broader typed input production remain unexecuted.

Before freeze the lead identified a real false-completeness defect: written
generic syntax omits anonymous argument-position impl Trait parameters.
`DeclarationGenerics` now retains actual parameter-Type ImplTrait sites and
unexpanded MacroType frontiers, excluding Expr/Item subtrees and return/body
syntax, under stable domain v4. `DeclarationParameters::prepare` holds before
count computation; `DeclarationGenericTypes` also retains Parameters pending.
It does not invent anonymous IDs or equate written zero with upstream zero.
Two unchanged imported APIT fixtures cover direct and nested syntax in the new
control. Actual macro Type expansion, anonymous identity production and the
legacy SignatureParameters/FunctionSignature route remain separate gaps.

## Eight recurring architecture modes

| Mode | Inspected route and remaining scope |
| --- | --- |
| 1. Separate assertion/expression evaluator | Argument collection, Q79 and the pinned variance producer prepare source metadata only. No assertion identity, synthetic executable owner or expression evaluator is introduced. Existing runtime alternate routes are not removed or accepted by this work. |
| 2. Literal/break-only macro execution | Original, arbitrary emit and wrapper sources enter the same admitted declaration/field/query path. No new runtime macro executor exists here. Standard/procedural macro and runtime matrices remain unexecuted. |
| 3. Context method/numeric rules | Source Type arguments use ordinary name/type services; variance uses pinned constraints. Pair covariance and mutable-pointer invariance expectations live only in host controls. No fixture name selects production semantics. |
| 4. Generated-source rejection | Actual generated IDs, auxiliary parent scopes, edition and epoch receipts survive the existing joins. Missing APIT/macro/foreign facts are common typed preparation holds for all origins. No original-only branch is added. |
| 5. Duplicate patterns/eager executable arms | No pattern engine, borrow/lifetime checker, function body or cold-arm validator is added. Parameter-Type scanning excludes executable expressions and nested items. Diagnostic originals are metadata witnesses only. |
| 6. Repeated parsing/planning | Q79 retains immutable source and computed results; installed callbacks do no source queries, parsing or reconstruction. The argument and parameter scans occur in demanded cold source preparation over retained AST nodes. |
| 7. Global namespace scans/invalidation | New consumers use existing dependency-scoped queries and guards. Nominal dependency closure is cold preparation, not a runtime namespace census. No new per-operation world comparison is added. |
| 8. Immutable-body copies/whole-file searches | Source plans retain Arcs and direct syntax ranges. The patch adds no BodyIndex clone or whole-file executable node search. Cold Type vectors and the pinned term arena are algorithm inputs, not frame body copies. |

Cold costs include argument syntax traversal, one parameter-Type scan per
demanded declaration and nominal field dependency closure. The compiler
constraint walk/fixed point runs during preparation; only borrowed result reads
occur in callbacks. No speed or scaling acceptance is claimed. Legacy runtime
normalization and source text fallbacks elsewhere remain named migration debt;
no new root invocation or readiness relaxation establishes their replacement.
The nominal producer currently retains the target vector for each demanded Q79
key. Separate roots with overlapping field closures can repeat cold constraint
inference, despite sharing source query Arcs. This is not installed execution
planning; cross-root inference-result reuse and representative scaling remain
unmeasured future work.

## V502 actual terminals

The sole validator launched direct-checkout
`cargo +1.99.0 test --offline --locked -p lrr --lib --no-run --message-format=json`
asynchronously. It terminated 101 with six mechanical errors: generated variance
terms/constraints import one too many `super` levels for identity; `ty::Pattern`
is absent from that crate API; two diagnostic formats require Display for the
model Ty; and the nominal Ty::new_adt trait method lacks an in-scope trait.
No source repair was made by the validator. No V502 CLI/digester build, selector
or interpreter run was attempted after the failed library build. All seven
selectors in `2026-10-20-gat-variance-controls.json` remain unexecuted at V502;
the previous source's successful controls cannot be carried across this failure.

All six source audits terminated zero using `/opt/homebrew/bin/python3.12`:
`sync_upstream_inference.py`, `bootstrap_trait_solver.py`,
`upstream_lang_names.py`, `upstream_transmute.py`, `upstream_nominal_type.py` and
`upstream_variance.py`. Separate `bootstrap_trait_solver.py --verify-upstream`
also terminated zero. These provenance results are source evidence, not compile
or runtime acceptance. Commands/logs and Cargo JSON are retained in
`artifacts/validation-current-v502`; required sources/tools/selections remain
outside that disposable directory.

After all processes were terminal, both after-build and after-runtime guards
matched the before fingerprint and HEAD with zero changed inputs. The failed
build receipts explicitly retain only the old V501 CLI SHA256
`730c905983e5df71ec8157fdf9592ef6fe5d52eb27472ac12bd67290781eff91`;
it is not a V502 executable. Its closing hash matched. No fresh test artifact or
digester success is claimed. Source/HEAD/binary ownership was returned to the
lead after those guards; this documentation update is outside the source guard.

## V503 mechanical delta review

V503 is separately bound to
`0998106d093e229941e497120ac13fe83fd301bba96eae3e58acf18248f6b61f`
(8,695 inputs; same HEAD). Comparing the captured manifests found exactly four
changed inputs: variance terms.rs, constraints.rs, mod.rs and their generator.
Generated imports now use the actual parent DefinitionId; Pattern uses the
Interner associated Pat type; Ty diagnostic formatting uses Debug; the nominal
producer imports the inherent Ty trait. Raw originals and source/GAT/query-domain
logic are unchanged. These representation fixes remove no branches or source
contracts. All eight source-review conclusions carry with their stated holds;
no V503 build/control/runtime result is inferred from the correction. Validation
is running under a new freeze and its terminals will be recorded separately.

## V503 actual terminals and scope

All processes are terminal at the V503 fingerprint above. Direct-checkout
offline locked Rust 1.99.0 library no-run, CLI and digester builds terminated
zero; Cargo recorded 42.46 seconds for the library and 33.73 seconds for the
CLI. All six source audits and separate bootstrap `--verify-upstream` terminated
zero using `/opt/homebrew/bin/python3.12`. Upstream verification recorded 227
files, 3,703,000 bytes, zero retained-original Rust changes and verified Git
objects at pinned `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`.

The seven exact selectors each ran one actual test body and terminated zero
with unchanged validation inputs. Counts below come from completed observation
messages, including the first message prefixed by the Rust test harness.

| Control in tracked selection order | Terminal | Seconds | Actual observations |
| --- | --- | --- | --- |
| StreamingIterator own static region | 0 | 33.270 | 12 edition/origin worlds |
| Recursive local nominal own Type argument | 0 | 30.553 | 12 edition/origin worlds |
| Actual variance query domains | 0 | 63.846 | 24 worlds; terminal test also asserted 96 declaration checks and exact Pair covariance |
| Direct/nested argument impl Trait completeness | 0 | 61.093 | 24 worlds with unknown count/typed pending and real zero-generic main |
| Mutable-pointer nominal variance | 0 | 29.929 | 12 worlds with exact invariant vector |
| Existing common nominal fields | 0 | 94.055 | 36 worlds, 96 encoded fields, zero pending declarations |
| Existing generic trait associated field | 0 | 30.673 | 12 worlds, one observed field each |

Every matrix contains owning editions 2015, 2018, 2021 and 2024 with original,
arbitrary emit and wrapper origins. New GAT controls exercised real alias ID,
parent/own argument split, constructor compatibility, immutable memo reuse,
cold executable preparation, epoch and source withdrawal checks. Covariant and
invariant vectors are actual source-backed outputs of the pinned algorithm.
The NotApplicable panics in the variance-domain log are the specifically caught
compiler-invariant controls; they do not turn arbitrary errors into passes.
APIT identities remain pending rather than becoming fabricated zero-count
results. The type-macro frontier, anonymous-ID producer, legacy signature path,
constructor/function/opaque and broad algorithm/input matrices remain held.
Cold assertions cover the demanded query work after configured world/discovery
setup; they do not independently certify the complete discovery path cold.

The same two unchanged owning-2015 runtime cases ran with
`LRR_EMBEDDED_FACADE=1`, fresh V503 CLI/digester and new report directory
`/private/tmp/lrr-v503-gat-variance-runtime`. The batch terminated one:
`generic-derived-type.rs` remained a runtime-contract pass with all 13 actual
capture/entry/dispatch/cleanup checks true and observed stdout `10\n10\n`;
`repr128.rs` remained interpreter failure `LRR-E-OP-TYPE`, with no check record.
The report guards imported originals, manifest and binaries unchanged, records
no native execution, and leaves selected-batch/runtime/JIT acceptance incomplete.
There are zero new stock runtime gains, and these component results do not
establish GAT normalization, a complete semantic factory/provider, a new root
invocation, full upstream fixture or diagnostic compatibility.

Before/after-build/after-runtime guards matched source and HEAD with zero
changed inputs. The fresh actual Cargo-JSON test executable was
`target/debug/deps/lrr-df1e3f56d083c63c`, SHA256
`a4bebbd43e713737f985446a8fa39db050e835280c9c62ae1a43ec003dcf4406`.
CLI SHA256 was
`cf677ba379f98c7e3ff1313ed9463369cc550c319895aa8b581d459a636a38ce`.
Both matched their after-build identities. A separate digester after-build and
after-runtime comparison matched SHA256
`0288fe353b015782dec8c6d99e82e41fb34441200193e97b6eae0eccf0c7bd77`;
the runtime report independently records that same digester hash. Source, HEAD
and all binary ownership were returned after all terminals and closing guards.
Compact observed-control counts and digester comparison are preserved beside
the standard receipts in `artifacts/validation-current-v503`. This worktrack
review is outside that source manifest; V502's failed evidence stays unchanged.
