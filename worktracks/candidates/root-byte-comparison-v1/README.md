# Immutable byte comparison successor

Historical status: source-only, uncompiled and unexecuted. The [byte assertions successor](../root-byte-assertions-v1/README.md) now includes this physical delta and adds its source/member and VM consumer; that successor is also uncompiled. Do not apply both patches.

Status at this draft: This four-file delta follows the complete [interpreter byte candidate](../root-interpreter-bytes-v1/README.md); verify its exact before hashes. It is a physical prerequisite for `T041`/`T062`, not a completed library method or passing rustc program. Claim `LOCAL-024` owns the draft.

The run-owned cursor compares genuine readonly byte-literal roots through shared array/slice leases. Each dispatch revalidates store incarnation, epoch, operand cell/value, reference lease, immutable allocation membership and object identity. Each visited field checks its ownership edge and U8 value. Work is capped at 256 pairs per dispatch, charged once per pair, and cancellation is polled even for empty, identical or unequal-length operands. Completed and failed cursors cannot replay. It allocates no guest storage and changes no guest owner.

The unchanged `tests/ui/lexer/crlf-in-byte-string-literal.rs` supplies the actual four tokens and pair order for one component witness across 2015/2018/2021/2024. Four store regressions cover dispatch/fuel bounds, interruption, short circuiting, stale/replaced operands, cross-store handles, readonly roots and damaged field ownership. These are five unexecuted test bodies, not five upstream passes. The original file and license texts already reside in the pinned fixture inventory.

Before production integration, implement the source-resolved `PartialEq` member admission and VM activation/result consumer. The physical cursor must not choose traits from runtime descriptors, accept rigid source types because their layout happens to match, or enter a source/query cache. Bind it to the actual PC, operation, destination, source publication and operand source terms; include `metadata_bytes()` in activation accounting. Preserve the caller's single opcode entry charge while charging compared elements. This draft deliberately retains unused private APIs until that consumer exists; strict production acceptance is still outstanding.

The public original semantics inspected are pinned `core/src/cmp.rs`, `core/src/array/equality.rs`, and `core/src/slice/cmp.rs`, with hashes in `SOURCE-AUDIT.json`. General mutable arrays and user-defined element equality remain required work. Only compiler-owned U8 contracts may use this immutable physical implementation.

Static evidence only: root tool `9e612f/0` performed exact four-file patch replay and pinned Rust 1.99 rustfmt checks. No Cargo or runtime command ran. The active V72 milestone source was unchanged. Apply neither this draft nor its predecessor to an active validation source. The assigned validation agent owns future asynchronous builds and acceptance.
