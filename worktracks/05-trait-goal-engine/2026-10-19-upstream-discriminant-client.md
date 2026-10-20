# M019/M020 — pinned discriminant Type client and actual enum representation

Baseline b7b5245a59dfd3d3ea4c903c38d125b87b35507a; solo LOCAL049. The
unchanged union-generic-rpass.rs still fails265operations at nested Iter<_>.
Its partial application/projection needs real upstream inference; do not extend
the legacy structural solver to fix it. Actual root entry still requires the
remaining compiler semantic clients and complete relevant source facts. This
bounded prerequisite supplies one whole compiler Type client, not a runtime
pass or a substitute for finishing the root/member/VM join.

Invariant: DiscriminantKind answers come from the complete pinned compiler
Ty::discriminant_ty algorithm and actual source enum repr metadata. Enum source
integer identity must not be reconstructed from physical byte width (usize
and u64 differ); no stub integer/semantic fallback or native rustc invocation.
Non-enum primitive/coroutine/pattern/unsafe-binder cases use their exact pinned
rules; parameter/alias/type-variable cases retain the real lang-item projection.

Common routes: actual TypeSchema/header/DefinitionReceipt -> shared declaration
repr parser (also used by ordinary RunMachine layout preparation) -> retained
source enum integer record -> source-validated AdtInfo -> existing SourceFacts::
discriminant_type/ordinary inherent Ty API -> pinned compiler client. Original
and generated declarations already join TypeSchema; keep its source/header/
context/edition guards. Extract the existing repr parser rather than duplicate
it, retaining physical layout behavior and align/packed combination. Remove the
Clients.discriminant function pointer; retain the reflective-field and other
required semantic clients until their full implementations exist.

Port all branches from ae0fe511b15ab760d7e963b13389d1bc2ab42b18
compiler/rustc_middle/src/ty/sty.rs. Reuse its complete coroutine discriminant
rule and compiler bound-region instantiation methods through the registered
binder adapter. Record mechanical concrete-IR/API spelling changes in the
existing generators and verify originals/hashes; no new solver/binder algorithm.

Cold work: the existing schema demand/attribute token walk once per prepared
source record; no function body preparation or guest execution. Solver callbacks
read retained immutable metadata and walk only the demanded Type. No IO, source
parsing/global namespace census/per-operation source planning, BodyIndex clone,
new static checking, limits/accounting/sandbox or unsafe. Runtime/currentness
admission still belongs to the source/goal coordinator; this record grants none.

Evidence: unchanged repr/discriminant fixture source metadata all4editions and
original/user-macro/wrapper source contexts; real type/header currentness and
withdrawal controls. Run the complete pinned client over actual retained source
IR and supplemental binder/coroutine representations, marking all unexecuted
lang-item/ADT/root/VM branches explicitly. Existing interpreter originals keep
runtime-contract versus full-fixture outcomes distinct. Full source fact/root/
member/inference/VM/origin/architecture and task/milestone/interpreter/JIT gates
remain open. Review all8recurring modes separately from MA002/MA003/MA006 at the
final exact source fingerprint before checkpointing.

V475 both builds pass. Four-control batch terminal1: source matrix fails the
actual std::intrinsics::discriminant_value import; actual source-IR/client and
both existing layout controls pass3/4. Unchanged discriminant_size/repr128 fail
that import before execution; union-generic still265failed, union-align passes.
All processes terminal and closing source/HEAD/binary guard0. Earlier V471/
V472/V474 build errors remain recorded; V473 compile-only source was superseded
by stronger source/header/context receipt comparison before acceptance.

Reached M007 library prerequisite: pinned library/std/src/lib.rs:552 re-exports
core::intrinsics. LRR facade/std/lib.rs omits it, while core::intrinsics is the
existing closed facade module. Add that exact ordinary source re-export, using
the existing module/import/name service and SourceDiscovery publications. No
new std-only resolver branch, import stripping, builtin function ID or runtime
semantics. Existing LOCAL049 facade source reservation is reused. New source
matrix/runtime checks establish whether the actual alias joins; keep failures
visible. Re-run affected source/import matrix and stock originals, retaining
unaffected V475 pinned-client/layout evidence at its original fingerprint.

V476 both builds pass but affected source control and all4runtime originals
fail import core::intrinsics from the new std re-export; even union-align is
regressed. All processes terminal and closing guard0. Preserve this failed
attempt; no acceptance/integration is claimed.

Actual shared entry-point diagnosis: facade::functions::install already calls
NameNamespaces::declare_module for the closed module path. resolve_named_imports
successfully resolves and installs the actual module alias, but its final
readiness uses only root_aliases or a synthetic item definition. A non-root
module disclosed solely by ModulePaths remains falsely missing. Extend the
common readiness to the already checked installed alias set, remove the
root-only tracking and retain producer/ambiguity/access checks for every
namespace. No special core/std/intrinsic-name case or manufactured item DefKey.
Add LRR graph-membership/pending/invalidation controls for a fully disclosed
non-root module without a synthetic item; run the existing upstream ambiguity
control. This changes import resolution only, not runtime/member Type meaning.

A second source prerequisite is real public intrinsic metadata: the closed
entry is lrr_discriminant_value, while rustc tests import discriminant_value.
Add core/intrinsics.rs through the existing embedded crate manifest and an
ordinary module declaration. Its public function keeps pinned core/intrinsics/
mod.rs:2512's exact generic/reference/projected result signature and forwards
through the ordinary VM to the existing closed helper. Do not guess an isize
public signature or add a new builtin ID/runtime dispatch branch. The helper's
existing fixed isize result and missing upstream projection/root integration
remain explicit runtime blockers. Metadata/import success is not execution.
Source identity/edition/macro handling, lazy cold bodies, query/cache framework
and all8mode obligations remain unchanged. Existing same-root reservations are
shared; no concurrent agent is involved.

V477 both builds pass; source metadata matrix passes60edition/origin worlds/
180enum records (156unchanged-fixture observations plus24external usize/u64
header controls). Actual shared Type encodings, memoized schema roots, cold
bodies and input withdrawal are checked. Existing upstream ambiguity control
passes. New module control fails only its expectation that the raw producer
ID precedes the actual unfinished import ID; both are legitimate live source
prerequisites, not a resolved value. V478 changes only that host oracle to
require one actual pending import and NeedInput for one of those two recorded
prerequisites, then restored exact value after retirement. No production change.
V477 runtime union-align is restored1462matched dispatches/25source calls;
discriminant_size advances from import0 to1188dispatch attempts/1187outcomes/
26calls then ownership/type fault, repr128 reaches28/27ops/3calls then primitive
operator Type fault. Generic union stays265failed at nested inference. All
V477 processes terminal and closing source/HEAD/binaries guard0. No newly
complete upstream runtime case or refreshed suite count. Runtime result typing
and upstream source/root/member/projection/VM integration remain open.

V478 final sourcecb42ddcc83d9ae3175fef1ca29f032607dd6b19621a80d7121fedff46d8c7f2b: both builds/2affected controls0,
all terminals collected and closing source/HEAD/test/CLI guard0. Only the host
pending-ID oracle changes fromV477; production and CLI are byte-identical.
Prior source/runtime/client/layout evidence stays at its own exact fingerprint.
Full task/root/member/runtime/interpreter/JIT/architecture gates remain held.
Evidence: worktracks/evidence/2026-10-20-v471-v478-upstream-discriminant.json.
