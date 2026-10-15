# Supported range versus unsupported binding/guard frontiers

V87's actual combined milestone failed solely because the existing `scalar_or_patterns.rs` control still expects `0 | 0..=1` to lower to Unsupported. Scalar ranges now have real reached lower/upper-bound recipes. The frozen source was unchanged; the failed gate is retained, not relabeled as passing. This is an obsolete harness contract, not evidence that a new interpreter failure can be ignored.

Keep all three exact existing source cases. For the supported range Entry, require its encoded range marker, exact source origin, decoded lower literal 0, source-declared i8 hint, Ge comparison, discarded bound temporary, distinct result slots, and the exact branch to upper-bound Operand(0) or Exit. The binding and guarded-alternative cases retain their original Unsupported and no-effects-op assertions. Every case retains the no-successor-query assertion. Rename the test to reflect this mixed supported/refused contract; no production or upstream original changes are made.

This one-leaf follow-on is against the exact V87 source. Exact replay and Rust 1.99 formatting passed; strict compilation, the unfiltered scalar-or-pattern bank and a fresh integrated milestone remain pending. Previous failed evidence remains failed. No runtime-contract count, full-fixture pass or task completion is added.
