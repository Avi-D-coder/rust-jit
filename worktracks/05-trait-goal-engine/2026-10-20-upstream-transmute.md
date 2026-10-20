# M020 bounded pinned transmutability client

Baseline `48792aa3361d872566cebb9a91c41e1fd12fbfe7`, delegated worker
`/root/wf_value_helpers`, lead `/root`; existing WT05 claim, macOS1.99 first.
Own new `infer/transmute.rs`, `tools/upstream_transmute.py`, retained/generated
`vendor/rustc-trait-solver/compiler/rustc_transmute/` and this record.
Lead owns existing package/delegate/generator/provider/manifest integration;
validation worker owns Cargo/test execution. No commit or subagents here.

Invariant: SolverDelegate::is_transmutable must reuse the full pinned
rustc_transmute tree/pruning/DFA/reference-condition algorithm, including all
Assume fields and exact compiler answer mapping; no primitive/equal-size or
fixture shortcut. Pin `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`.
Unchanged upstream repr128 and existing transmute-const remain witnesses, not
new runtime acceptance claims from this component.

Shared route: guarded actual source/type/constant/ADT metadata -> shared TypeIR
and region erasure -> pinned Assume::from_const -> compiler layout-tree adapter
-> complete generic rustc_transmute algorithm -> same upstream certainty or
NoSolution mapping. Original/generated syntax join before TypeIR; no parser,
macro-specific rule, evaluator or guest-code execution is introduced. Reached
const/static initialization stays ordinary VM under E-R5-007.

The last `SolverClients.transmutable` pointer is the alternate route for lead
removal. Compiler adapter retains all from_ty/tuple/struct/enum/variant/tag/union
and reference branches. Actual target/ABI LayoutData, variant-layout, tag,
primitive tag-Type and discriminant query results are mandatory typed immutable
leaves. Their real source/target/epoch/dependency producer remains a hold.
Missing data must return typed preparation dependencies, never supported=false,
unknown-layout/NoSolution or fabricated equal-layout results. Actual compiler
layout error categories remain distinct from missing preparation.

Generic crate adaptations are mechanical: same indexmap/FxBuildHasher aliases
instead of compiler reexports; Infallible instead of unstable generic !; stable
Option iterator flattening; public generic interfaces for the separate adapter.
Original compiler files/licenses/hashes remain retained, every transform audited.
No independent automaton/search algorithm, allocator limit, unsafe or backend.
The ABI Layout wrapper borrows the complete pinned LayoutData; no local layout
calculation or memory-size heuristic is introduced. Prepared queries retain
exact environment/type/variant identities and actual source ADT field types.

Cold work: demanded layout queries and typed source/Assume preparation. Client
execution traverses only demanded retained IR, builds the pinned trees/automata
and compares them with upstream memoization. No namespace census, parsing,
BodyIndex copies, runtime source planning or IO in callbacks. No performance
claim before measurement; no budgets/caps beyond pinned arithmetic semantics.

Evidence required: retained exact originals + deterministic transformation
audit, separate source-aware eight-mode review at exact fingerprint, frozen
macOS builds and source-context controls with actual facts. Complete source
producer/query closure/root/member/VM, four-edition/origin/cleanup/runtime
matrices and M020/interpreter/JIT gates remain held. Source-only algorithm reuse
cannot establish guest/runtime acceptance or compiler diagnostics/tooling parity.

## Frozen source handoff and separate review

Generated adapter SHA256
`099fac458a220a0a022622388c74b7c042dc72acc0f766b9d8d3c696ea52646f`;
generator SHA256
`78367846d36060be206c9a8900fe66a0ba76be9e1543668a8b666427dcf70682`.
All generated package/adapter path+content fingerprints, sorted by path and
encoded as relative path/NUL/raw SHA256, have aggregate SHA256
`d63bf1a8eaf556ac1a7264c7297c9cdd858bd050bcce0ec9b97692d43f61b476`.
`python3 tools/upstream_transmute.py` exited0 with matching deterministic
transformations and exact original/license hashes. A separate Python check
compared all17 algorithm/helper originals byte-for-byte with `git -C rustc/rust
show ae0fe511b15ab760d7e963b13389d1bc2ab42b18:<path>`; exit0. No Cargo, source
control or fixture execution was run by this worker.

Every crate original is retained at its original path plus `.upstream`; both
MIT and Apache2.0 license texts are copied unchanged from the pin. `python3
tools/upstream_transmute.py --manifest-rows` emits22 exact raw/generated/license
manifest rows for lead integration. Additional newly retained compiler helper
originals are `compiler/rustc_data_structures/src/fx.rs` (1918bytes,
`4aa1903ab2e425e4651bd9fb2954ca76399c6d0cb1cc8ddda214638141605e51`)
and `compiler/rustc_middle/src/ty/sty.rs` (75431bytes,
`6c29bfe00c8de834c370bbc2cfb9d6ffd39d6d662d9145860b9c0153b37f5487`).
The generator contains all17 exact original SHA256 values. Raw source imports
and transformation equality do not establish compiler/runtime compatibility.

The stable generic crate retains every generic tree/pruning/DFA/byte-edge/
quantifier/reference-condition branch. It exposes the existing generic APIs
across the package boundary, uses the exact original FxIndexMap/FxIndexSet
aliases via their actual indexmap/FxBuildHasher definitions, represents the
uninhabited generic marker as Infallible, and replaces Option::into_flat_iter
with into_iter().flatten(). Existing ordinary compiler unit-test bodies are
retained with the same representation changes. Only the standard-library
nightly benchmark module and `extern crate test` use the explicit
`nightly-bench` feature; they are not skipped language fixtures or claimed
executed. Crate dependency is rustc_transmute(default-features=false); cached
indexmap2.14.2, rustc-hash2.1.3, smallvec1.16.2, tracing0.1.44 and existing
itertools0.15.0 support the package. Lead owns workspace/Cargo.lock changes.

Adapter source review inspected pinned `Assume::from_const`,
`ConstExt::{try_to_value,try_to_leaf}`, `Value::try_to_leaf`, `ScalarInt::TRUE`,
`Ty::{ty_adt_def,is_box}`, `AdtDef::{non_enum_variant,adt_kind}`,
`Def::has_safety_invariants`, `Tree::{from_ty,from_tuple,from_struct,from_enum,
from_variant,from_tag,from_union}`, `ty_field`, `ty_variant`, compiler
MaybeTransmutableQuery::answer and SolverDelegate::is_transmutable. Pinned
helper error categorization and safety/TRUE policies are generated directly
from original bodies, not independently specified policies. AdtInfo.kind and
existing OwnedBox language-item identity replace compiler stored-flag access;
actual field Types use the shared EarlyBinder instantiation.

`Assume` is read through its actual source ADT/lang identity, declaration field
names/order and valtree children. No positional four-bool guess or host
Assume-default is substituted. Wrong-lang-item delayed-bug/all-assumptions
behavior, non-value/non-ADT/field-leaf None outcomes and missing-field panic are
preserved from upstream. Missing source metadata is still pre-admission, not
these semantic outcomes. The full aggregate cardinality/typed-constant producer
invariants recorded in the earlier WF-values review remain required.

All eight recurring modes were reviewed against the adapter fingerprint:

1. No expression compiler/evaluator or new constant runtime route. Assume
   reading only destructures supplied TypeIR constants.
2. No generated scalar-only executor or reduced transmutability profile. The
   complete generic algorithm and compiler layout conversion branches remain.
3. No method-name/macro-specific inference. Actual source field names are used
   only to read the compiler-defined Assume ADT contract.
4. No generated/original exclusion. Actual definition/field/generic identity
   comes from the same SourceFacts. Full origin execution remains untested.
5. No parallel pattern engine or cold-body/arm preparation. Layout trees
   traverse only demanded actual metadata and source field Types.
6. No parsing/planning during client/installed dispatch. Prepared-query lookups,
   shared IR erasure and pinned tree/automaton construction are retained. Generic
   tree/answer clones are the compiler algorithm's own operations; no body plan
   is rebuilt.
7. No namespace census or unrelated invalidation added. Query keys use actual
   typing environment, Type, parent layout/variant and target identity within
   the owning prepared context. Missing leaves are explicit dependencies.
8. No BodyIndex copies or whole-file source searches. Borrowed complete ABI
   LayoutData handles have constant-time pointer identity, as compiler interned
   handles do; they are not portable cache keys or validated source receipts.

The already authorized validation/review worker independently reviewed API and
source representation. It caught the stable Endian conversion ordering; that
was corrected before the final fingerprint. Earlier lifetime-shadowing,
Option-negation, trait-method visibility, slice projection and root receiver
spellings were also corrected before freeze. No passing build is inferred from
this source review.

## Required integration API and holds

`PreparedProvider::transmute_inputs(self)` returns a required
`&'db PreparedTransmuteInputs<'db>`. The record is raw typed immutable data; it
is not a ready provider or proof that its producer exists. Fields are:

- actual `target:&TargetDataLayout` and checked
  `transmute_opts:Option<DefinitionId>` language identity; None means actual
  checked absence, never unavailable preparation;
- `layouts:HashMap<LayoutQuery,Result<Layout,LayoutError>>`, keyed by erased
  actual Type and the fully-monomorphized TypingEnv used by pinned conversion;
- `variants:HashMap<VariantQuery,Layout>`, retaining the base query, actual
  parent layout handle and VariantIdx for the compiler variant-layout query;
- `tags:HashMap<TagQuery,Option<ScalarInt>>`, where stored None is an actual
  tag-query absence, while a missing map entry is a preparation dependency;
- `primitives:HashMap<Primitive,Ty>` for the compiler tag-primitive Type query;
- `discriminants:HashMap<DefinitionId,&[Discriminant]>`, retaining actual
  source-ordered index/value/Type records. Their values are not guessed from
  enumeration or repr; the pinned layout-tree consumer ignores the value but
  the actual compiler-query producer still owns it.

Layout borrows the complete pinned `LayoutData<FieldIdx,VariantIdx>` because
compiler rustc_abi::Layout is nightly-only. Actual layout data, variant offsets,
niches, alignments, targets and tags are required; no layout calculator,
equal-size/primitive/union approximation or fake source facts are supplied.
Layout error payloads are projected only to the exact categories consumed by
the pinned Err conversion; missing data is a separate Preparation variant.
Region/Type wrappers adapt foreign trait coherence while retaining original
shared handles. Def variant/field references use actual owner+ordinal identity,
not fabricated standalone source DefinitionIds.

`is_transmutable(infcx,src,dst,assume)` returns
`Result<Result<Certainty,NoSolution>,TransmuteDependency>`. Outer dependency
variants identify missing Layout/Variant/Tag/Primitive/Discriminants records.
Inner results preserve the pinned mapping: Answer::Yes yields Yes; No and If
both yield NoSolution, including the compiler's current nested-goal limitation.
Complete actual source/lang/target/query-result producer, closure admission,
root suspension/member/VM and actual Assume/layout/reference controls remain
unimplemented/unexecuted by this worker. Lead must remove the function-pointer
route, wire the required inputs and retain existing root readiness holds.
No fixture, component execution, stock gain, interpreter/JIT, Linux, performance,
full-fixture or diagnostic/tooling acceptance follows from this handoff.

## Lead-authorized integration slice

After the frozen source handoff, lead assigned this worker the existing Cargo
workspace/lrr manifests and lock, generated infer/mod/delegate and their owning
generators, PreparedFacts/provider input interface and exact source inventory
registration. This includes bootstrap's registered packaging/audit hooks needed
for its new ninth crate and the full-inference audit. No db key/value/provider,
upstream/mod, source/lang/header/CLI-test files are owned or changed here.

Integration removes the final SolverClients callback and its now-empty Clients
container. Delegate calls the pinned client with the existing explicit
pre-admission invariant on typed missing inputs. PreparedFacts requires real
transmutability data alongside constant-evaluation inputs; no factory, empty
facts, admission shortcut or ReadyProvider relaxation is added. Source-only
full-inference/bootstrap/provenance audits are allowed; Cargo/tests remain with
the validation worker. Exact integration fingerprint/review will follow.

Integration source frozen for lead V495 build. Eleven sorted relative
path/NUL/raw SHA256 records have aggregate SHA256
`c4e3df6b9bdf800cf5708a70330625ca82fef8c135bfa5efe62cd48d670ddcd6`:
Cargo.toml, Cargo.lock, crates/lrr/Cargo.toml,
crates/lrr/src/traits/upstream/infer/mod.rs,
crates/lrr/src/traits/upstream/infer/delegate.rs,
crates/lrr/src/traits/upstream/prepared.rs,
tools/upstream_solver_delegate.py, tools/sync_upstream_inference.py,
tools/bootstrap_trait_solver.py, vendor/rustc-trait-solver/Cargo.toml and
vendor/rustc-trait-solver.files.json.

Final source/provenance terminal commands:

- `python3 tools/upstream_transmute.py`: exit0, exact full generic/client
  registered transforms and original/license hashes match.
- `python3 tools/sync_upstream_inference.py`: exit0, now includes the complete
  transmute adapter. Actual source/query admission explicitly remains required.
- `python3.12 tools/bootstrap_trait_solver.py --verify-upstream`: exit0,
 9crates,220files,3551096source bytes,14registered Rust source adaptations
 (9transmute),0retained-original changes, all upstream Git objects verified.

The first bootstrap attempt failed only source membership for the other
worker's exact pinned lang-items original. Lead authorized registering
`compiler/rustc_attr_ir/src/lang_items.rs` (41074bytes,
`947f488e46c70f47f0565d6df979e8b5126af6fb49a8c1c32fe2e4b802c7a6a0`),
which was independently byte-checked against its pinned Git blob. Bootstrap
also now includes the13 helper paths already present in the inherited manifest
but absent from its hardcoded support closure, plus this slice's fx helper and
lang-items original. No original source was altered or check weakened. Its
normal import reconstructs raw originals/generated crate and both licenses;
registered transmute transforms are checked in the ordinary source audit.

Final API: PreparedFacts::new requires owned
`transmutability: infer::transmute::PreparedTransmuteInputs<'cx>` alongside the
existing constant-evaluation input; PreparedProvider::transmute_inputs returns
a borrowed required reference. SolverClients, prepared::Clients, their field
and solver_clients accessor are deleted. The generated delegate directly calls
`transmute::is_transmutable(&self.0,src,dst,assume)` and reports an unadmitted
input as the explicit typed-dependency invariant failure, preserving the
upstream trait signature and real semantic answer mapping. No caller/default
factory, source-proof bypass, ReadyProvider relaxation or input manufacture is
added. Cargo.lock was edited as source to add the existing cached dependencies;
only the validation agent's actual Cargo --locked terminal can establish that
resolution/build acceptance. No Cargo/test ran in this worker.

Separate integration source review at the11-path fingerprint found the common
route extended through the owning generator and required provider facts,
with the old callback/container removed. Modes1-5 add no evaluator, generated
payload language, contextual inference/member rule, original-source filter or
pattern/cold-arm traversal. Modes6-8 add no parsing/planning, namespace dispatch
scan, unrelated invalidation, BodyIndex copy or whole-file node lookup. Required
query maps are cold immutable input, not source completeness evidence. The
prior adapter/generic algorithm review remains attached to its own exact
fingerprint. Full source/lang/layout/const-value producer, admission/suspension,
actual upstream root/runtime execution and all broader gates stay held.
