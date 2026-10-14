# Source-owned local contexts in array lengths — uncompiled

The three-leaf complete.patch follows the earlier eight-path root-constant-local-inference-v1 proposal. Alternatively coherent-v68.patch contains all nine resulting source paths directly against canonical v68 c011cdae; do not apply both. Verify exact before/after hashes in the matching JSON manifest. Its36 combined controls are uncompiled. Patch reconstruction and pinned formatting are metadata checks only.

The graph replays the exact existing array-length admission before supplying usize context and excludes outer runtime locals/parameters, generics, closures and foreign syntax. The positive run-pass component obtains root attribute evidence through actual source/compiler/Value/Type preparation. REVIEW.md describes the bounded source probes, immutable green ownership identity, evidence and remaining runtime/header-query prerequisites.

Seven unchanged official cases/12 original-license files consolidate the predecessor and successor fixtures. Merge them through tools/import_rustc_tests.py, retaining the complete canonical inventory, upstream commit, licenses, directives, revisions and snapshots. No Cargo target, validation log or complete workspace is included. The separate frozen v68 milestone is unchanged. No task, integration, configured runtime/full-fixture, JIT, Linux or release acceptance is claimed.
