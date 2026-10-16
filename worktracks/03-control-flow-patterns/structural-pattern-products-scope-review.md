# Pattern component successor: source storage and scope edges

WT03/T050/T051/H003, LOCAL-024. This is a separately identified source-only
successor to coordinator's immutable
`common-pattern-products-component-v1/overlay+COMPONENT.json`, copied with actual
terminal7cadac/0. Its component23 fingerprint remains
147f3a776cb4af00de823cf15a45091cc2c055ae43f28b045dfc386e8ce85031.
Do not replace that source or attach later conclusions to its validation.
Production owner selection, binding proof, and Product memo ownership remain
held. No task, milestone, runtime fixture or architecture gate is closed.

The bounded review found and corrected these concrete protocol gaps:

- `Frame::preflight_pattern_decision` EnterAttempt previously accepted an active
  arm scope with a live nested scope. It now requires that active arm to be
  innermost, or uses the ordinary lexical entry preflight before reentry.
- Fail(None) previously needed only Matched cursor state. It now requires both
  ordinary guard and arm scope cleanup completed. Cold verifier `exits` requires
  exactly one ordinary arm ScopeLeave in that guard-failure recipe; duplicate
  or omitted leaves are refused before recipe execution.
- The same source slot was insufficient to pin storage across a retained cursor.
  DecisionTransaction now captures actual CellId and location epoch at Begin;
  every decision, continuation exit, Preview/Finish/Rollback and generic pattern
  owning read validates that constant-size receipt. Moving/restoring a value or
  replacing the cell cannot revive the old receipt. Begin additionally checks
  the source's actual unique physical FrameSlot owner.
- Generic `preflight_read_place` now joins the active decision's exact ready
  node/depth/place/phase/value scratch and original source receipt before
  ordinary source-certified intrinsic Copy. A cached place alone cannot bypass
  the retained source cursor.
- A successful guarded pattern needs a distinct guard-start PC from final arm
  Commit after a true guard. Coordinator added MatchArm Operand0 admission only
  for actual guard_scope. Product capture now retains that matched continuation
  for guarded arms, Commit for unguarded arms. Arm.Exit remains false guard;
  PatternCleanup remains exhausted arm. No macro/raw phase namespace was added.

`VerifiedRecipe::product_roots()` is a crate-private read-only flat iterator of
actual `&Arc<PatternProductPlan>` handles, one per Decision op. It neither clones
nor deduplicates. Root can count actual source/recipe handles, deduplicate product
allocations for their one nested index Arc and register weak external/retirement
pins. This helper alone does not implement memo accounting or authorize the
production switch. The root owns all DB edits.

Added two Frame test bodies outside originals. One checks a valid replacement
cell and an identical value moved/restored through a new epoch; refusal must
leave source/cursor/accounting untouched. The other injects only the pure
cursor's completed-test event, then uses actual ordinary scope cleanup/verified
PatternNext recipes to check omitted/duplicate leaves, pending nested scope,
false-guard cleanup and one arm reentry. It executes no stock binding or Rust
guard body and does not claim a guard-count/runtime contract pass. Existing
wire-body controls now check the roots iterator preserves two actual handles to
one shared allocation. Total component bank is14 bodies: graph4, cursor/intent4,
Frame4, wire2. All are uncompiled/unexecuted by this worker.

Separate source review against all eight recurring failures, for the exact
successor manifest fingerprint:

1. `frame_pattern_decision` invokes ordinary Frame/store/scope services; no
   assertion or other expression compiler/evaluator was introduced.
2. Product source rows still represent actual tuple/@/or/scalar locations.
   Source-cell and scope guards are ordinary runtime authority, not an expanded
   literal/break evaluator. Required four-context execution remains unexecuted.
3. No method or numeric inference service changed. Type descriptors still only
   check already admitted source membership. The reference peer explicitly
   confirmed nominal/ref composite children cannot be admitted merely from
   TyTerm/TypeId/identity; opaque certificate closure across all composite
   producers/GenericEnv packets is a separate common prerequisite. Existing
   structural nominal/ref refusal is retained, with no layout-derived shortcut.
4. No generated-source exclusion was added. Prepared body/profile/index and
   physical storage receipts apply equally. Generated binding authority remains
   held behind the common cached source/namespace proof, not spelling fallback.
5. Guard-start/final-commit/failure/exhaustion PCs are now distinct. Scope leave
   happens through the ordinary bounded cleanup activation, and reentry is
   exact once. However old match/condition production schedules remain selected;
   new Binding Stage is still unavailable. No duplicate-pattern repair closure
   or structured binding execution is claimed before their replacement/removal.
6. New guards read fixed identities, retained source-cursor metadata and current
   store liveness. They perform no parser, source graph construction, full-index
   encoding or query. Coordinator's cached semantic_identity remains the index
   receipt. Future owner batching must still avoid repeated body admission.
7. No namespace scan/currentness census was introduced. Relevant bucket proof
   and the old global capture removal remain the root's coordinated prerequisite.
8. The iterator exposes retained Arc handles without copies. Product/Frame keep
   their shared immutable indexes; memo owner/weak-pin accounting is explicitly
   pending and must prevent hot Finish from becoming the final graph destructor.
   No whole-file node search or immutable body clone was added.

Source cursor review also checked malformed tuple arity and missing/wrong source
views: ordinary tuple/source membership refuses before committing guest binding
or advancing cursor. Projection preparation commits its cursor only after the
existing place/source publication succeeds; one tuple ordinal is processed per
charged step. Invalid sources may leave a cleanup-owned place annotation after
refusal, not a published binding. Panic/return cleanup retires transaction metadata
with its original source scope; actual panic/suspension/resume/epoch matrix is
still unexecuted. Ref/owning binding modes and tuple source certificate closure
are explicit gaps, not reduced to the primary Boolean witness.

Formatting/scoped diff check completed with exit0 only. No Cargo/Rust/native
execution or extra agents. The sole validator should apply this successor only
to a new immutable source; run the same four filters in the v1 handoff, now
expecting Frame4 and total14. Existing wire, Frame scalar pattern and storage
banks require recheck because shared methods changed. Exact owned delta and
coordinator BodyIndex/guard oracle dependencies are in
`worktracks/evidence/2026-10-15-pattern-products-scope-source.json`.
