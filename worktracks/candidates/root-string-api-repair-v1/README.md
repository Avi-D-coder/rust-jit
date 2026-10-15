# String opcode API and capture integration repair

T024/T061 invariant: string opcodes must use the pinned scalar representation and retain exhaustive runtime-effect classification. Read-only review of the complete string lineage found two concrete compile blockers, not a new source-language policy issue.

`static_string_byte_len` already returns `u64`, and `ValueRepr::unsigned(TypeId, u64)` accepts that width. Both the length preflight and execution path now pass the returned length directly, preserving the constructor's existing usize-width validation. The earlier unnecessary conversion to `u128` did not match the pinned API.

The exhaustive operation match in `engine/run_capture.rs` now explicitly classifies `StrByteLen` and `StrAsBytes` alongside the existing internal operations. They read worker-local immutable storage or issue worker-local leases; neither invokes an external sink. No wildcard is added. Candidate classification still requires a matching VM outcome and observed terminal cleanup before closed capture can be certified. One pending LRR observer-state control checks both required observations; its synthetic reports are not interpreter execution or fixture evidence.

Apply after `root-string-byte-view-v1`, retaining the V88 range harness repair. The existing unchanged NUL length operand and CRLF byte-view operand controls remain the runtime witnesses to execute after compilation. Exact patch replay and Rust 1.99 formatting passed; compilation, observer and upstream-derived runtime controls, Clippy and a fresh full milestone remain pending. This source-only repair adds no stock pass and completes no task.
