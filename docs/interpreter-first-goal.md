# Standing implementation goal

Implement the revision5 LRR interpreter on macOS ARM64/Rust1.99.0 until every applicable pinned upstream runtime contract and interpreter correctness requirement passes. Compile-time diagnostics and compiler/tooling contracts have separate gates; full-suite compatibility remains incomplete until every category passes. Use the pinned parser/query/VM architecture and the Section57 cross-ledger order. Drive fixes with unchanged tests from rustc/rust; preserve original auxiliaries, directives, revisions, expected files and MIT OR Apache-2.0 licensing/provenance. Keep LRR authority, suspension, resource, invalidation and seal controls separate.

Do not start guest JIT implementation before complete upstream_runtime_acceptance and applicable interpreter correctness gates pass under Section1.2.1. Run tests asynchronously on immutable source and continue independent code/review; collect actual terminal results before integrating or claiming a pass. Classify whole-program observations, full fixtures, failures, unsupported and unexecuted cases separately. Imported files/component checks are not full stock passes. Current full rustc compiletest acceptance is not established. Finish with reproducible artifacts and an honest report; Linux is a later port and requires actual separate acceptance.

The latest user instructions and revision5 specification override the historical revision3/Linux objective text stored in the active goal's metadata. The implementation goal remains active and incomplete.

## Edition scope amendment — October 13, 2026

Support every released official Rust edition: 2015, 2018, 2021 and 2024 for roots, dependencies and macro origins. Preserve the owning edition in parsing, lookup, prelude and macro contexts, patterns, temporary lifetimes and unit/cache identities. Standalone --edition selects it explicitly (default2024); Cargo target declarations remain authoritative. The experimental future selector is excluded. This supersedes former root2024-only acceptance. Use unchanged official rustc cases for edition-paired interpreter acceptance; parser admission and scoped program controls alone are not full language/compiletest compatibility. The active implementation goal remains incomplete; interpreter convergence still precedes guest JIT.

## Revision 5 adoption — October 13, 2026

The user explicitly requires the new specification while continuing interpreter-first work. Sections61–62 and the G2 ledger batch are mandatory later work. Preserve historical revision3/4 component receipts; no such receipt proves the changed ABI, optimization or execution-contract acceptance. Guest JIT, LLVM and native ABI implementation remain deferred until interpreter convergence. The goal remains active and incomplete.
