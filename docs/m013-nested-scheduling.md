# M013 nested generated producer scheduling

M013 remains in progress. An admitted generated child no longer occupies its
parent's sole pending slot until completion. Its exact ExpansionKey remains in a
bounded parked-request index. Child frames retain explicit parent identities and
stable source-position paths. A deterministic depth-first scan may revisit a
blocked child, then inspect independent parent siblings. Completion returns the
child to its actual parent and uses the immutable invocation position. Retained
child catalogues still undergo full strict selection revalidation before sealing.

Depth/count, metadata, duplicate identity/order and exact request checks occur
before generated headers are published. Paths use bounded source offsets and an
explicit sentinel to order descendants before parents. Depth allows at most 256
catalogue frames including the root; frame count is bounded at 100,000. Metadata
shares the existing 320 MiB retained discovery budget. A parked request is never
expanded a second time just to permit sibling progress.

Three new controls execute a parent sibling that supplies a blocked child, keep a
true nested import cycle incomplete after independent progress, and execute two
parked descendants with distinct parent chains. The pre-repair failure and first
successful run are retained. The native macOS matrix now includes both successful
nested examples, bringing it to ten actual successful VM/native comparisons;
five earlier origin ambiguity rejection comparisons remain. Native oracle copies
are explicit isolated preparation, never selected live runtime compilation.

Full origin overlap/invalidation, Cargo/world/definition-unit facets,
generated-body runtime macro contexts, type/quota/failure boundaries, complete
MX027/MX028 fixture/JIT acceptance and release remain pending. M014–M016 follow
in fixed Section 57 order. All evidence in this checkpoint is macOS ARM64 on Rust
1.99.0; Linux remains a later port.
