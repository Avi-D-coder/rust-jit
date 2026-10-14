# Original-byte restoration successor — frozen source only

Corrected candidate source digest: `caf168227cdb14b5858110b12e08fc33a3dc43e7b54ff25a52b30852a06ec8d6`.

The frozen v2 exact-original claims are withdrawn. I failed to check the imported raw bytes again after formatting. Its manifest and source digest recorded actual files but did not establish that those files remained the official originals. The previous `a38d0fff...` identity is a quarantined content identity, not valid exact-upstream provenance. No compilation or runtime execution was performed, and none is claimed by this restoration.

## Evidence and bounded cause

All 52 original assets were checked against their import manifest, the exact `ae0fe511b15ab760d7e963b13389d1bc2ab42b18:path` Git object selection and the actual Git blob bytes. Exactly 19 frozen v2 assets differed; all 19 actual bad bytes remain in the old packet and are preserved separately here under `quarantined-fixture-bytes/`. The coherent39, runtime8 and qualified4 predecessor fixture sets have zero manifest differences. The v2 post-copy assets are the first affected set.

Formatting a disposable non-fixture pristine copy of each affected file with pinned Rust1.99 `rustfmt --edition 2024 --config skip_children=true` reproduced all 19 corrupt byte sequences exactly, with all formatter exits zero (actual `9182b1/exit0`). This establishes that the observed mutation is the formatter output, not an importer selection change. The preserved final freeze script submits only explicit crates/xtask leaves with `--check`; its later all-file hashing cannot detect pristine-provenance failure. The exact earlier mutating command is not retained in the three saved assembly scripts; do not invent its argument list. The demonstrated pipeline failure is that copied originals reached a mutating formatting step and no final raw-Git assertion guarded the freeze.

## Corrected packet and guard

Only the 38 source leaves already frozen in v2 and the original assets are copied. All 38 source bytes, before bytes and source patches remain identical. `complete.patch` remains SHA256 `0d2a9d9ffe0671857b450cf481e890c8d8fe08c3f9c53bf900efe374ad5d4538`. Root's owning-runtime-world guards, all shared publication hooks and the closure merge's expected source baselines are unaffected. Prototype model files remain preserved in the old immutable packet, not registered here or promoted into readiness.

All 52 assets are now reconstructed from their exact pinned Git blob, with matching commit path, blob ID, SHA256 and byte length. Both MIT and Apache license texts and COPYRIGHT are retained unchanged. `original-byte-restoration.patch` (SHA256 `e406d98818ad66d3c243ce43fd50caa65c77aaf148fbdd9f59143bffb91573fe`) is an explicit recovery delta for only the 19 corrupt imported paths; upstream whitespace is preserved byte-for-byte. The 38-path source patch and 19-path asset recovery were each applied in isolated scratch directories and compared exactly (`fcbcb1/exit0`). The source manifest covers 91 candidate files: 38 LRR leaves, 52 raw upstream assets and the unchanged original import manifest.

`verify_and_format.py` is the only formatting entry point for this successor. It takes the exact 38 source paths from the source before/after manifest, requires crates/xtask `.rs` paths, rejects fixture/original paths, traversal and symlinks, and checks source hashes before invoking the pinned formatter with `skip_children=true`. It verifies every one of the 52 actual commit/blob byte sequences before and after formatting. Its default is `--check`; optional `--write` still cannot select upstream paths and requires regenerating the source manifest before a later freeze. Actual default terminal `37014/adb2f6/exit0` completed both raw-byte passes and source formatting check. No immutable fixture was passed to a formatter. The intentionally formatted forensic probes are explicitly disposable non-fixture copies.

## Acceptance and paused work

The 63 v2 live controls remain uncompiled and unexecuted; raw original restoration grants no result. The sole validator must compile and execute against these exact restored inputs after the explicit shared Type-v4/Value-v3 successor is assembled. Preserve the existing full runtime/diagnostic assertions, including trait-method and static privacy obligations. Header/query components do not pass an original whole fixture. No JIT, static checker, Linux, layout/Copy/Fn/ABI or native equivalence is inferred.

The disjoint `sol-alias-consumer-replay-v1` two-leaf code/test draft is paused. Its attempted original-byte assertion caught the same invalid Font bytes (`3c92df/exit1`) before writing a provenance manifest. Its partial copied original is explicitly quarantined; it is neither frozen nor ready for integration. No compiler, runtime, shared publication, canonical source or target edits occurred there. Root can resume that consumer join separately after reviewing this correction.
