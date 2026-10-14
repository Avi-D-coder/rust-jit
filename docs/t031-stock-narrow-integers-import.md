# T031 official narrow integer arithmetic follow-up

Collected scoped execution; original files are now integrated. Three unchanged official run-pass
programs (u8-incr, u8-incr-decr, i8-incr) retain original directives and both
licenses at rustc ae0fe511b15ab760d7e963b13389d1bc2ab42b18. All have explicitly
annotated primitive locals and original assertions; they exercise width-specific
arithmetic and assignment without unresolved numeric-width defaulting.

The previously collected combined-v3 CLI executed all three original programs
and their three assertions, with its exact binary hash/size/mtime verified before
and after. Separate explicitly authorized native edition2015 and2024 references
also passed on Rust1.99.0 macOS ARM64. The preceding arithmetic programs and
failed isize batch were not rerun for this observation.

LRR2024 execution is a disclosed edition adaptation and does not establish full
compiletest/JIT/release/Linux acceptance. With the subsequent division/float/u32
imports, MAIN retains127 cases/221 original files. Fresh combined MAIN validation
is pending. [Collected observation](../artifacts/macos-first/t031-stock-narrow-integers-v1-scoped-observations/index.json)
and [six-file integration](../artifacts/macos-first/t031-stock-six-additional-originals-integration-v1/integration.json)
retain the exact scope and provenance.
