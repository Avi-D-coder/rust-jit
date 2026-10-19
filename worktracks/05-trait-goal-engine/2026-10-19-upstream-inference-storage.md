# M020 — reuse pinned inference storage and transaction client

Solo root/LOCAL-049; baseline2d8176106566909384cb86ee6b4e24ce9613aceb.
E-R5-014 requires upstream inference transactions, not an independent LRR engine.
The current shared-IR provider has no variable state/snapshot client. Minimal
unchanged witnesses remain methods/method-two-trait-defer-resolution-2.rs and
traits/multidispatch1.rs: candidate attempts must not leak Type/const/numeric/
region/opaque state to later attempts. They are runtime blockers, not passed by
this source component. Existing source metadata/140-head original/emit/wrapper
all-edition matrix supplies actual IR/contexts for supplemental storage controls.

Reuse exact pinned rust-analyzer next_solver/infer storage modules (same official
Rust commit), combined UndoLog and InferCtxtInner. This is compatible client reuse,
not selecting its whole frontend or invoking body inference. Retain original
licensed sources/hashes and a deterministic registered mechanical port script.
Keep all original mutation/undo/commit branches and nested snapshot behavior.
Adapt only concrete IR/lifetime/import/container API and diagnostic span spelling.
Use ena0.14.4 already selected by pinned rustc_type_ir. No guessed semantic fact,
fake provider, custom snapshot algorithm, body inference or live rustc compilation.

Source review found that the rust-analyzer SolverDelegate currently stubs leak
checking and returns no deduplicated region constraints. DO NOT port those
callbacks. This work imports storage/data only; real compiler-compatible leak
checking, VisibleForLeakCheck, assumptions-on-binders, complete relation/context/
canonical/delegate callbacks remain prerequisites before using an inference client
for an actual goal. Compiler and RA lub helper branches were compared and are
identical at this pin; no speculative semantic fix is authorized by this record.
Known incomplete callbacks cannot be silently defaulted to success or NoSolution.

Shared route: ordinary handwritten/admitted generated declaration/Type queries
-> actual source bindings/typed generic context -> future inference delegate ->
upstream evaluate_root_goal -> ordinary member/VM. Extend upstream/ only and the
existing actual-IR source control; no macro/runtime route. Old custom unifier
consumers remain held until actual end-to-end migration and removal. This source
storage client must never be represented as complete inference or solving.

Cold construction owns mutable variable state per inference context and shares
immutable IR handles. Transactions use the exact upstream undo log; installed
VM dispatch/ordinary frames gain no parsing/planning/full namespace scan or
BodyIndex clone. OS-owned vectors/maps, no accounting/cap/fuel/broker/static checks.
Before checkpoint: async direct-checkout builds, exact mechanical source audits,
actual source-backed variable/numeric/const/region/nested rollback/commit controls,
existing source/generic and scoped cleanup regressions. Inspect all eight recurring
modes plus separate MA004/MA007/MA008; bind review/results to final fingerprint.
No stock runtime gain, solver invocation, M020/milestone/interpreter/JIT/Linux or
performance claim follows from storage-only acceptance. Missing integration and
unexecuted region/opaque/source/cancellation matrices stay explicit holds.

V335 both builds pass;3/4controls pass. The actual source/storage matrix fails
on its first head because the supplementary test tried known-I32 against
known-U32 in a NoError numeric table; pinned ty_kind.rs:790-805 requires relation
resolution before calling that table. Do not change upstream to accommodate this
invalid test. Use the actual region table's Known-placeholder/Unknown-root
universe error for commit_if_ok failure and keep numeric unknown-to-known
rollback checks. Record the original failure separately.

Final V337 both builds/4of4controls pass on exact guarded source. The corrected test uses public PlaceholderRegion::new_anon and an actual unnameable region-table value error; numeric operations retain their upstream unknown-to-known precondition.Storage controls use140actual Types in48source contexts. This is storage-only evidence, with complete delegate/facts/region/canonical/runtime integration still held. See worktracks/evidence/2026-10-19-v334-v337-upstream-inference-storage.json and the separate review.
