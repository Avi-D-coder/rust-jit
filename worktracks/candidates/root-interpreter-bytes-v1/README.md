# Coherent byte-literal interpreter candidate

Status: source-only, uncompiled and unexecuted. This 48-file patch consolidates the previously prepared named-range, byte-literal, source-bound match context, byte CRLF and large-byte construction stages against `f8f4922539`. It preserves the integrated float-method receipts and dispatch revalidation. Use `SOURCE-AUDIT.json` to check exact before and after bytes; do not apply the predecessor patches again.

The implementation covers reached named range bounds, source-qualified byte annotations, immutable run-owned byte arrays and shared array/slice leases, original CRLF decoding, and bounded large-literal construction/rollback. It retains source/member authority, exact original spans, per-element fuel, private unfinished ownership and fresh result tickets. General string operations and array/slice `PartialEq` dispatch are not implemented by this patch. The [comparison draft](../root-byte-comparison-v1/README.md) is a separate, unfinished successor.

Add these pinned upstream cases with `tools/import_rustc_tests.py` before compiling; preserve the aggregate manifest, all companions and both rustc licenses:

- `tests/ui/match/match-range-char-const.rs`
- `tests/ui/parser/issues/issue-21475.rs`
- `tests/ui/parser/slowparse-bstring.rs`

The original source files are deliberately not duplicated in the patch. The slowparse case contains a 400000-byte literal; its complete `main`, not a shortened replacement, is the runtime witness. Existing CRLF/byte originals remain unchanged. Original hashes and upstream commit are recorded in the audit file.

Acceptance remains pending. After the current V72 gate terminates and its evidence is collected, the assigned test agent may assemble this successor in the existing frozen-source/target workflow. First verify all source before hashes and perform additive imports, then collect strict build and the byte/range scopes, including `stock_large_byte_literal_runtime`, `stock_qualified_range_constants_runtime` and the affected `match_no_arm_exit` targets. Stop and report an actual failure rather than restarting repeatedly. No second target owner, source mutation during validation, or full milestone repetition is authorized by this packet. Integration still requires successful applicable scopes and the required subsequent milestone.

Static evidence only: root tool `293467/0` checked every predecessor join against canonical source and replayed the combined patch to identical final hashes. The individual stages have pinned-format receipts recorded in their retained local handoffs. No Rust compilation or runtime success is inferred. Claim `LOCAL-024` retains ownership; task cards and predecessor hashes are in `SOURCE-AUDIT.json`.
