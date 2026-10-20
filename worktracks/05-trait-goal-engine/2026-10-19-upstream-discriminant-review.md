# M019/M020 pinned discriminant Type client — separate source review

Baseline b7b5245a59dfd3d3ea4c903c38d125b87b35507a; solo LOCAL049. Current
V475 fingerprint efab295a5f3a3299ca7de931bdee6123177f5e05b324b7bae4cddc1c14364531.
This is a held source/client component, not source/root/member/VM integration.
MA002/MA003/MA006 remain distinct from the recurring modes numbered below.

Inspected upstream_discriminant.py::expected against the complete pinned
compiler/rustc_middle/src/ty/sty.rs Ty::discriminant_ty and CoroutineArgsExt::
discr_ty; discriminant::Ty::pinned_discriminant_ty; constructors inherent Ty::
discriminant_ty -> PreparedProvider SourceFacts::discriminant_type. The old
Clients.discriminant function pointer is removed. Every compiler match branch
is retained: enum metadata, coroutine u32, actual DiscriminantKind associated
projection, Pat, erased UnsafeBinder, ordinary u8 families and compiler panics
for unsupported bound/placeholder/fresh inference representation. Typed API/
namespace/iterator spelling changes are registered in the generator; the
original hash is checked. No custom inference/trait algorithm is introduced.

Inspected sync_upstream_binders.py's three complete added compiler methods,
BoundVarReplacer and existing generic ownership. Only the closure generic name
is changed to avoid shadowing the source-fact family F. The erased-region
constructor uses the existing Interner. A retained actual source Type under a
bound reference keeps that Type when the one late region is erased; original
metadata is unchanged. No binder scope/index algorithm is copied locally.

PreparedEnumDiscriminant::prepare uses DefinitionReceipt::observe/is_enum,
actual cached TypeSchema, source/header/context checks, the existing guard
builder and shared declared_repr. It retains the actual receipt/schema/guard.
Provider publication requires a real enum source and matching exact Definition-
Source/Header/Context metadata; source spelling equality alone is insufficient.
No guessed descriptor/enum identity or runtime tag grants admission. The typed
source record translates only the integer already stated by the source. Full
ADT fact production/atomic publication/currentness/member/runtime joins and
root readiness remain unimplemented; source metadata alone cannot authorize
one of those operations. Tests explicitly keep ready() rejected.

The repr token walk was extracted from engine::run_layout::declared_repr into
frontend::declaration_repr::declared_repr, preserving physical layout behavior.
Actual integer identity is retained alongside the existing byte width; usize
and u64 no longer collapse in the source record. Runtime layout still consumes
only .layout. Relevant cold schema/header preparation remains once per owning
consumer; global memo/owned source-fact publication of the enum record is still
a required producer-integration step, not claimed delivered by this helper.
Installed runtime intrinsic execution never calls this parser.

Eight recurring modes:
1/2. No assertion/macro evaluator, synthetic operand body or literal/break-only
     macro output executor. Original/generated TypeSchema ownership is shared.
3. No method/macro/fixture-name inference rule. repr integer spellings are Rust
   attribute grammar; Type/trait goals still use real source identities.
4. No original-only acceptance or generated exclusion. The exact schema/parser/
   source checks apply to generated definitions, retaining owning edition.
5. No pattern engine, executable branch/arm preparation or borrow/static check.
   Declaration field/variant syntax stays metadata; no initializer is evaluated.
6. Pinned Type callbacks read retained metadata only. Source enum preparation
   fetches memoized schemas and walks actual header attrs cold. No source IO,
   parsing, query planning or per-op reconstruction is added to installed VM
   dispatch. Remaining source-record memo/producer join is explicitly held.
7. No full namespace-map construction or global invalidation is added. New
   receipt comparisons are relevant source/header/context checks during cold
   fact publication. Runtime currentness remains the coordinator's obligation.
8. No BodyIndex/parser deep copy or whole-file node search is added. The
   existing syntax_at selects a node by walking the indexed range, not unrelated
   siblings. New records share retained Arc roots; only source metadata is owned.

No new caps/accounting, sandbox, unsafe, backend/frontend/query-framework choice
or static checking. Other legacy inference/member/runtime routes remain held.
V471 fails3mechanical API/import errors; V472 one host AllocateList import;
V473 both builds pass, no tests before a source-review fix; V474 two host imports.
V475 both builds and pinned-port audits pass. Source matrix first fails the real
unchanged std::intrinsics::discriminant_value import; it is not waived or stripped.
The pinned-source context and struct-layout controls pass at this exact source;
remaining controls/closing guard are pending here. Required non-Type/enum/
projection/coroutine/full source/root/VM branches not exercised are gaps.

Additional current-source runtime gap: Run facade DiscriminantValue always
constructs TypeId(8) (isize), independently of an enum's actual repr. The new
compiler client does not fix or count that legacy runtime result typing. Connect
it through the common source-selected result/projection mechanism with genuine
upstream facts; do not add an enum/assertion/fixture-specific return guess.

V475 final four-control batch3/4: pinned source IR/inference client and both
existing layout controls pass; the source matrix is failed at unchanged
std::intrinsics import. Runtime union-align passes; three selected failures
remain. All terminals collected; closing source/HEAD/binary guard0.

V476 source39d0f29f869279724a54544a03afdff800f187b2ffc4b773bedee2e920e3c952:
only facade/std/lib.rs changes afterV475. The ordinary source re-export
pub use core::intrinsics matches pinned library/std/src/lib.rs:552. There is
no new resolver path, facade function ID, spelling whitelist, parser, evaluator,
Type inference or runtime branch. Actual source Module/Type/Value import
services must resolve it and keep dependencies; none of the failed original
imports are stripped from the source control or runtime cases. Core's existing
DiscriminantKind declaration remains unchanged. The same shared parser/client/
binder code and its existing V475 evidence are retained at that fingerprint;
only affected source/import matrix and full originals are rerun atV476.
All8mode review conclusions above still apply; new re-export adds no hot-path
source planning, global census or immutable body copy. Scope/edition/macros
continue through the same ordinary source publisher. Full runtime result typing,
source/ADT/solver root/member/VM integration and remaining legacy routes remain
held, independent of this import's outcome.

V477 sourcef629719ee81890875beab182230d358878aca2f31f48d392ab6290a651bd923b:
inspected NameNamespaces::resolve_named_imports/module-path alias installation.
The existing ModuleResolution::Known result already proves a disclosed target,
checks producer frontiers and re-export visibility, and installs its actual
canonical route. Final import readiness now uses that installed set for both
root/non-root modules instead of requiring a synthetic item definition for
non-root modules. The separate root_aliases set is removed. Pending/ambiguous/
access outcomes for every namespace and source-module frontier checks stay in
the same worklist. There is no closed-facade/std/intrinsics-name branch. This
adds no query framework, global per-op scan, second namespace engine or runtime
preparation, and removes one cold set allocation. No speed claim is made.
The new LRR component control covers actual route/missing synthetic item,
root pending producer, retirement and restored lookup across4editions; the
unchanged upstream ambiguity control remains enabled.

The ordinary core/intrinsics source module and embedded manifest entry expose
the public discriminant_value wrapper with the exact pinned generic/reference/
projected-result signature (core/src/intrinsics/mod.rs:2512). The body is ordinary
Rust forwarding to the existing explicitly admitted lrr helper. No new native
symbol, facade ID, dispatch/evaluator or return-Type guess is added. The old
helper's isize result remains a separate unfixed runtime requirement. Std's
real pub use core::intrinsics now refers to an admitted source module. Existing
closed helper values retain their canonical source-profile identities.

Eight-mode extension: unchanged source/macro/lowering/VM paths (1/2); no context-
specific method/inference rule (3); generated and handwritten modules use the
same import worklist and source publication (4); no pattern/cold-body checking
(5); all changes are cold namespace/source preparation, not installed execution
(6); no new census/unrelated namespace invalidation (7); no deep immutable body
copy or whole-file node search (8). New public wrapper bodies remain unentered
by metadata checks. Full root/member/runtime/currentness/architecture and JIT
gates remain held, independently of a metadata/import outcome.

V478 final sourcecb42ddcc83d9ae3175fef1ca29f032607dd6b19621a80d7121fedff46d8c7f2b: both builds/2affected controls0,
all terminals collected and closing source/HEAD/test/CLI guard0. Only the host
pending-ID oracle changes fromV477; production and CLI are byte-identical.
Prior source/runtime/client/layout evidence stays at its own exact fingerprint.
Full task/root/member/runtime/interpreter/JIT/architecture gates remain held.
Evidence: worktracks/evidence/2026-10-20-v471-v478-upstream-discriminant.json.

Final V478 source review correction: the pending frontier control now requires
one actual unfinished import and an unresolved result for that import or its
actual source producer, then exact restored value after retirement. It does
not accept an arbitrary error/ID or infer success from a missing check. Only
names_import_ambiguity_tests.rs differs fromV477, verified against actual source
manifests. CLI SHA25613534b381bc54dc757e4912d6f7bd964b8f38adb89e52e9d9f23714dd97a63a9
is identical. The two affected controls pass atcb42ddcc83d9ae3175fef1ca29f032607dd6b19621a80d7121fedff46d8c7f2b;
all final input/binary guards pass. No production conclusions above change.
V475 source-context evidence is164encoded source-head observations in60logged
world rows, not the earlier historical140/48count and not164unique Types.
V477 matrix has180enum metadata observations in60worlds:156from unchanged
originals and24from explicitly external usize/u64 header controls. It is not
180upstream runtime passes. All five other mandatory semantic clients and the
full source/root/member/VM/currentness/architecture joins remain open.
