# Pinned inference storage — solo source review, full integration held

Baseline2d8176106566909384cb86ee6b4e24ce9613aceb. M020/LOCAL-049/E-R5-014.
Reviewed V337 fingerprint:
`f9fdf5421a1b8cdd74cb425ece57ff3d01193d86ff510bc53237338ed4f020d9`.
V335 production code is identical; its supplementary numeric conflict violated
the upstream NoError table precondition. V336 corrects that test to use a real region table cross-universe error;
V337 changes only its placeholder construction to the pinned public new_anon
API after V336 correctly rejected private placeholder fields. All8 production source conclusions were
rechecked; final validation is pending.
Build/control outcomes must be attached separately; full inference/trait selection
and runtime semantics are not established by this review.

## Actual reused code and source boundaries

Nine exact official Rust Git objects are retained under the existing original
MIT OR Apache-2.0 license: hir-ty lib.rs (span preference), next_solver/infer
mod.rs (storage/records/universe methods), type_variable.rs, unify_key.rs,
region_constraints/mod.rs, snapshot/mod.rs, snapshot/undo_log.rs and both
opaque_types storage files. `bootstrap_trait_solver.py --verify-upstream`
checks the pin, bytes, membership, licenses and original Git objects. No compiled
vendor Rust changes. `sync_upstream_inference.py` checks independent literal
original hashes and reproduces every compiled storage module with Rust1.99.0
formatting. Actual numerical/table dependencies reuse locked ena0.14.4,
indexmap2.14.2 and rustc-hash2.1.3; only direct dependency edges changed.

`infer::InferenceStorage` is an adapter owner for RefCell<InferCtxtInner> and its
universe Cell. It is not an InferCtxtLike/SolverDelegate implementation. Its
constructor creates genuine empty *fresh mutable variable storage*, not an empty
source-fact provider. CombinedSnapshot, snapshot counts/start/rollback/commit,
commit_if_ok, probe, variable length inspection and region/opaque snapshot
inspection are upstream bodies, with only owner/import/type names adapted.
`InferCtxtInner`, all unification key/value rules, type storage, full combined
undo enum/reversal, const/numeric tables and region/opaque stores retain the
selected complete upstream operations. There is no LRR rollback engine.

Mechanical API projections: old RA OutlivesPredicate becomes the actual current
OutlivesClause; ParamTy is actual SourceParameter; type/const/region/alias/binder/
opaque-key types are aliases over existing Cx and real IR. Shared Ty::new_param
replaces RA's ParamTy wrapper. Alias to_ty explicitly passes IsRigid::Yes for
this next-solver-only client (the pinned compiler GenericKind::to_ty uses
IsRigid::yes_if_next_solver). Span preference is copied verbatim; is_dummy tests
the exact existing LRR dummy representation. Custom derive macros for the plain
OpaqueHiddenType record are not imported; its same single Ty field is retained,
with ordinary Copy/Clone/Debug. Its future fold/visit consumer remains pending.

Only selected storage code is imported. `snapshot::fudge`, whole-body inference,
RA database/frontend/interner types, ObligationInspector, SolverDelegate and its
known stubbed leak_check/make_deduplicated_region_constraints are absent. No
success/empty/NoSolution callbacks are used in their place. Before a real goal,
port actual compiler-compatible leak/visible-region/assumptions-on-binders
behavior and finish relation/canonicalization/delegate/source-fact integration.
The retained RA region store alone does not establish those contracts.

## Eight recurring failure modes (distinct from MA finding numbers)

1. **Assertion/expression evaluator:** only inference client/storage and a cfg(test)
   inspection call change. Ordinary source/Type preparation remains
   DeclarationGenerics/DeclarationGenericTypes -> WrittenTypeDeclaration ->
   resolve_actual_type -> source_terms::Encoder. No expression or VM evaluator.
2. **Literal/break-only generated execution:** none added. Actual fixture Types
   enter the same existing context in original, emit and wrapper worlds. Storage
   has no macro names, syntax parser, opcode, verifier or output executor. This
   is not closure of MA004/MA007/MA008.
3. **Context-specific selection/numeric inference:** numeric unification is the
   actual pinned ena/IR table operation and copied combined undo client, not a
   new numeric fallback or matcher. Tests deliberately cause a cross-universe region
   table unification error inside commit_if_ok; that is storage rollback evidence, not a
   chosen trait candidate or live source inference answer. No method whitelist.
4. **Generated-source exclusion:** there is no source-origin branch in storage.
   Supplemental controls use actual admitted original/generated declaration
   ranges and encoded Types already supplied by the shared source matrix. No
   fake span/DefId/provider, text parser or original-only production restriction.
5. **Pattern duplication/cold execution:** no pattern service or body lowering
   changes. Existing source trace assertions still forbid FunctionSignature/
   BodyStructure/Thunk/PatternPlan demand. The new control directly mutates
   inference state, never evaluates guest branches or predicates. Broader
   pattern/unused-bound/cleanup matrices and MA002 remain separate holds.
6. **Repeated preparation:** source work stays in existing pure retained query
   plans. Each inference owner contains mutable state and shared immutable IR
   handles; snapshot operations touch variable stores/undo log and universe only.
   They perform no source IO/parsing/planning or installed-operation preparation.
7. **World scans/invalidation:** new transaction code knows no namespace/catalog
   maps, so cannot perform a world census or unrelated invalidation. Actual Cx
   construction/currentness guards stay with PreparedSolverConfiguration. Final
   answer dependencies, negative candidates and transitive currentness still
   require actual integration before runtime reuse.
8. **Immutable copies/searches:** snapshots retain undo markers and region state,
   not cloned BodyIndex/parser/source trees. Type handles remain interned/shared.
   Existing direct declaration/source locations provide real spans in host
   controls; no repeated whole-file syntax search is added.

No local unsafe, caps/accounting/fuel, sandbox or static checker is introduced.
RA region comments mention regionck/NLL but these are retained source commentary;
no borrow-checking/reporting pass or entry to them is added. The new integer
snapshot counter is upstream transaction bookkeeping, not a program resource cap.

## Evidence required / current holds

V334 mechanical compile failure is preserved, not a runtime failure: grouped
imports, private Cx re-export, old Outlives naming, removed inherent trait names
and old ParamTy/alias API transport. V335 rebuild follows their registered fixes.
The actual source matrix must check nested failure rollback, nested successful
commit followed by outer rollback, root commit, probe, universe restoration,
Type equality/root restoration, const/int/float values and region counts/edges.
These operations use the true upstream CombinedSnapshot and actual fixture IR,
with no fake source provider. Generic ordering and existing cache-owner cleanup
controls remain distinct. No new test is added to imported language originals.

Opaque registration/duplicate rollback, member/verifies/region-assumption paths,
panic/cancellation, full leak/visible-region/binder behavior, relation/canonical
responses and actual goal/delegate execution remain unexecuted. Opaque rows must
use real opaque declaration identities before acceptance; a nominal/impl ID must
not be disguised as an opaque key just to increase coverage. Full source facts,
parameter DefIds, actual selected member/continuation transport and all migration
removal/gate requirements remain incomplete. Old custom inference consumers are
held and unchanged. No new distinct stock runtime/full-fixture pass, refreshed
suite total, milestone/JIT/Linux/performance claim is authorized by this packet.

Structural costs: variable operations use upstream tables; rollback visits the
actual recorded changes, and snapshots store markers/region state rather than
immutable source copies. No runtime call/dispatch/phase source work is added.
This is source analysis, not a measured release-speed/scaling result.

Final V337 library51780, immediate CLI and controls57899 are terminal exit0.4/4controls pass, source/HEAD/both binary guards match the reviewed fingerprint. Actual storage/combined snapshot checks run for140fixture-derived Types in48original/emit/wrapper edition worlds;136actual guarded TraitRefs remain correct. No production changes after review; V336/V337 only correct the host negative-case/placeholder constructor. V335CLI binary is byte-identical to the final CLI; the final build is separately collected. Existing original mains are unexecuted by this new storage matrix. No root/inference/delegate/full-fact/runtime/milestone/JIT acceptance follows. Opaque/other unexecuted and incomplete region integration holds above remain. Source/audit/plan/document outcomes and exact failed/final terminals are in worktracks/evidence/2026-10-19-v334-v337-upstream-inference-storage.json.
