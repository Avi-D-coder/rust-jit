# E-M015-016: compiler-owned Attribute aliases compete with builtin inert flags

M015 and revision 4 sections 49/50.4 require resolved Attribute kinds before
final test metadata and TestCatalogSeal. Compiler-owned Attribute definitions
participate in that invariant alongside procedural exports.

A read-only review found that E-M015-015 consulted PreparedProcCatalog only.
The parent pure Test-proof witness then failed on unchanged integrated production:
compiler-owned `test` aliased as `ignore` was certified as inert metadata.
The collected session 22354 exits 101 with zero passed and one failed test.
Source bytes and membership remain unchanged; retained witness receipt:
artifacts/macos-first/m015-required-admission-compiler-inert-alias-v1-witness.json.
The fixture checks both compiler `test` and `derive` aliases and both inert names;
the first failing case is the minimal witness, so the remaining loop cases are
not individually inferred to have executed in the baseline failure.

Two separately authorized isolated native reference compilations confirm that
`use core::prelude::rust_2024::test as ignore` and its `should_panic` variant
produce E0659 ambiguity on Rust 1.99.0/aarch64-apple-darwin. They are disclosed
supplements outside copied rustc source; no selected LRR live unit is compiled,
no code executes, and no stock fixture pass is claimed. The official unchanged
ambiguous-builtin-attrs closure remains under MIT OR Apache-2.0 with commit,
Git blobs and SHA256 retained.

The bounded candidate checks exact BuiltinAttributeCatalog identity/provenance
alongside PreparedProcCatalog identity, using the existing compiler facade.
Compiler Test/derive aliases yield LRR-E-MACRO-AMBIGUOUS. Actual compiler bang
aliases remain valid negative Attribute-facet bindings. All nine original,
generated, cold and seal-replay certification calls receive the captured compiler
facet; the stable proof domain changes. No spelling supplies compiler authority,
no macro callback or query-provider side effect is added, and pending namespaces
remain pending. Declaration scheduling behavior is unchanged.

Current private core session 19590 passed 202 library, 61 source-discovery,
67 declaration-scan and eight catalog tests, formatting and strict Clippy.
Current independent native compatibility session 80476 passed all nine controls
plus source provenance, with exactly one legitimate prefix callback and zero
body queries. Both ran against 3528 fixed inputs and membership. Receipts:
artifacts/macos-first/m015-required-admission-compiler-inert-alias-repair-v1-acceptance.json
and artifacts/macos-first/m015-required-admission-compiler-inert-alias-native-v1-acceptance.json.
Worker V4 source-backed baseline confirms all six strict contradictions:
original2 and actual MBE-generated2 incorrectly seal flagged catalogs; cold2
incorrectly seal empty root catalogs. Two real source bang contrasts and source
provenance pass. Earlier V1/V2 were compile-only harness mistakes; V3 generated
controls stopped at an observer identity mismatch, so no generated production
failure is inferred from V3. All those histories remain preserved independently.
Repaired compiler-alias source discovery controls, late-output transitions and
fresh integrated milestone acceptance remain pending. The main E-M015-015 gate continues against its own frozen source.
M015 component transition, full MX031/MX032, guest JIT, full stock fixtures,
release and actual Linux acceptance remain incomplete.

Worker V5 on the first repaired facet helper caught a separate compiler-only
startup bypass. Its generated2/cold2/bang2/source1 controls passed; original2
still incorrectly sealed. V7 exercised all four later-alias flag/placement
combinations independently: nine controls passed, static-original2 and
late-original2 failed. Each late original admitted its actual alias record before
the incorrect seal. Its 165-member immutable witness archive was independently
verified (164 indexed members plus the byte-exact embedded member manifest) and
retained in artifacts/macos-first/m015-compiler-inert-alias-v7-witness-worker-retention.json.

The parent startup candidate routes free Test functions through the same exact
annotation owner on early and late compiler installation. Procedural reservations
keep their selected consumer; compiler reservations scan every source once, and
already-reserved calls require the same ordinal and complete invocation identity.
An initial over-broad version stole module/const Test annotations from the existing
declaration scanner: 201 library tests passed, one existing refusal failed, and
later checks were unexecuted. That phase is preserved. Free-function-only compiler
reservation retains scanner target refusal. Its compatibility phase passed202
library/62source-discovery/67scan/8catalog/fmt/strictClippy, but a read-only review
found an unnecessary per-unit rescan; the current single-pass version requires
fresh validation and independent unchanged worker source controls.

The corrected single-pass core passed in collected session14268. The worker's
unchanged thirteen controls then passed V9 after capacity recovery, with all
3529source inputs and membership fixed, zero bodies/procedural callbacks/live
compilation, and real returned MBE records for late aliases. Its 213-member
immutable acceptance/oracle/history archive was independently verified and
retained in artifacts/macos-first/m015-compiler-inert-alias-v9-witness-worker-retention.json.
V8 compiled no controls because the host exhausted disk; its equal source
postmap and explicit failed infrastructure receipt remain preserved. CARGO_INCREMENTAL=0
was recorded for the same-source V9 retry as tooling, without semantic/profile changes.

Main session75422 and parent startup/native session14429 also failed during that
disk exhaustion. Source bytes and membership stayed fixed. The main milestone
was not completed and its earlier2046-test gate files remained unchanged. The
native suite observed5pass/7fail: six evidence-directory ENOSPC failures plus an
earlier server-handshake mismatch before source discovery, not classified as a
source semantic failure. Same-source parent retry32561 remains pending. The
accepted combined production cannot integrate until that result is collected;
a new full main milestone will cover both facet and startup corrections.

Same-source parent retry32561 passed source1/native11 with13 actual observations, five legitimate callbacks and zero body queries. The earlier handshake error did not recur; no semantic or test expectation changed. The independently accepted current correction and all regressions are integrated; fresh full macOS milestone remains pending.
