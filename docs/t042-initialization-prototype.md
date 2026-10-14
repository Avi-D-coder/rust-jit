# Integrated cached initialization components

Exact scoped-tested initialization V1 bytes are integrated. All503 scoped project
tests passed, including all38 T042 controls; format, strict Clippy and architecture
passed. Validation combined preserved command records after two external runner
errors; every authorized command exited0 and no test was repeated. The3723-input
candidate stayed unchanged. The current integration requires a fresh MAIN gate.
T042 remains in_progress; full stock fixture, Copy/Drop, JIT, release and Linux
acceptance remain false. Earlier private-draft wording below is historical.

# T042 cached aggregate initialization draft

This separate private candidate is unvalidated and unintegrated. Its parent is the scoped-validated assignment V3 candidate. MAIN remains unchanged during its current milestone. The inherited private task ledger records an earlier lineage and is not current MAIN status.

Invariant (revision 4 Sections 12.4, 12.5 and 16.7): ordinary whole-value transfers require every owning field to be initialized; moved fields retain their external owners and are skipped by aggregate cleanup. Each aggregate carries a checked field bitset and complete-child count. Mutations preflight the complete logical owning-parent chain, then update summaries without recursive host traversal, tree rescanning, Copy/Drop queries, allocation or callbacks. References terminate completeness propagation. The bitset's actual retained allocation is charged with object storage.

Construction admits only complete field values and installs all complete bits together. Ordinary moves, intrinsic owning reads, assignment delivery and call argument admission reject cached incomplete aggregate values. Field moves, reinitialization, replacement and retirement maintain ancestor summaries; argument packets reject an owning value and its descendant in either order before allocating or transferring. Cleanup has a typed one-field step (Field, Skipped, Complete), may extract partially initialized aggregates for cleanup, and never treats that extraction as an ordinary legal whole-value move. The previous looping convenience API exists only in tests.

Seven controls reuse existing unchanged rustc sources: borrowck/tuple-field-move-with-drop-borrow.rs, tuple/index-float.rs and array-slice-vec/vec-fixed-length.rs. The tuple factory retains the exact source-bound destructor sibling and prepared mutable Unit reference. Its destructor body and original consume function do not run. Direct checked-store field reinitialization, raw Unit/isize transfers, a 256x nesting stress and a 17x array-width expansion are disclosed LRR-specific ownership/cache regressions. They are not whole unchanged source executions or stock compiletest passes. No new upstream files are copied; the private inventory remains 140 cases / 247 originals at the pinned commit, with both upstream licenses retained.

The checks cover whole-transfer/argument refusal after a field move, reinitialization without epoch revival, declaration-order skipped-field cleanup, propagation through nested owners, packet ancestor overlap, quota refusal without summary changes, and the 63/64 bit boundary. Together with the parent, 38 T042 controls are selected for one frozen background scope. No current draft check is claimed passed.

General aggregate Copy/source policy, writable guest AssignPlace reinitialization, ordinary resumable guest destructor dispatch and full T042 remain unfinished. Full stock fixture acceptance, production JIT, release and Linux acceptance remain false. Integration requires actual terminal results, unchanged tested source, collection of the current MAIN milestone, checked current MAIN preimages and a fresh integrated milestone.
