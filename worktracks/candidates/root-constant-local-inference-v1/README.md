# Source-owned constant-local inference successor

This eight-file source-only patch follows the interpreter-constants canonical v66 candidate; its touched before bytes also match v68. Verify all exact before hashes in SOURCE-AUDIT.json before applying it. All30 controls are uncompiled; five unchanged official cases remain pending. REVIEW.md describes the semantics, quotas and required integration work, including reachedness-aware contextual hints and the original basics.rs GCD/prime function selections.

Use tools/import_rustc_tests.py to merge the unchanged licensed originals into the complete canonical fixture inventory. Preserve their directives, revisions and snapshots. Do not replace the full manifest with this packet subset. No Cargo target, validation log or whole-workspace source snapshot is included.
