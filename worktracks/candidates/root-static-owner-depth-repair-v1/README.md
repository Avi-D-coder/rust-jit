# Immutable-root ancestry depth repair

T041/T062 production repair for the actual V86 combined milestone failure: `rustc_nested_tuple_initialization_summary_propagates_without_tree_rescan` failed at `move_from(leaf)` with `LRR-E-RUNTIME-RESOURCE`. The new immutable-literal guard stopped after 256 parent edges. The existing unchanged upstream-derived stress uses `nested_unit_tuple_depth(index-float.rs) * 256`, or 512 edges, with a 16 MiB budget. This failure does not establish memory exhaustion.

The guard now bounds traversal by the store's allocated handle count and detects ownership cycles without allocation. It still rejects a static root or descendant before mutation. The existing depth, budget, initialization-summary assertions and upstream original remain unchanged. A supplemental corruption control requires a cyclic owner chain to fail without changing guest state.

Two leaves apply exactly to V86 source `3f018ccc`. Formatting and exact patch replay passed; compilation, the existing nested initialization regression, immutable-root refusal controls and the combined milestone are pending. No new stock pass or canonical integration is claimed. When assembling the held string chain later, carry this guard forward into its overlapping `cell_byte_literals.rs`; do not overwrite it with the earlier string baseline.
