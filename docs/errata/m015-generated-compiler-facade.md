# E-M015-004: generated extern-crate routes retain compiler façade availability

The unchanged official rustc `tests/ui/test-attrs/issue-36768.rs` declares tests
named `foo` and `core` alongside a module named `foo` and `extern crate core`.
A disclosed Rust 2024/test-cfg metadata adaptation installs the existing owned
standalone compiler catalog through the public upfront compiler builder. The
original source seals and publishes both distinct value identities. A real
declarative macro emitting the exact unchanged item suffix instead fails with
`LRR-E-MACRO-SCOPE: extern crate dependency is not captured` during generated
route construction. Neither witness lowers or executes test bodies.

The independent procedural witness reaches one matched macro callback and
adopts its actual schema-checked output before the same route refusal. The
output retains the original extern-crate declaration and both test headers,
and an LRR cfg-disabled supplement. Failure to seal is preserved; physical
headers and callback observations are not final-catalog acceptance.

Original source merged captured dependency aliases with the already installed
owned compiler facets. Generated source used only captured Cargo dependency
aliases, omitting the available core facet. The repair shares the original
bounded deterministic alias merge with generated route construction, retaining
captured-alias precedence and the existing distinction between standard prelude
routes and explicitly available std/core facets. It adds no compilation,
macro-execution, query-provider or native runtime authority.

Witness: `artifacts/macos-first/m015-generated-facade-witness-v2-failure.json`.
The earlier witness-builder failure and the repair's missing-import compilation
failure remain separately recorded. The corrected repair retains the strict
original/generated assertions; the independent procedural continuation passes all three controls on the same
fixed source, with an actual seal, distinct final identities and no
configured-away producer invocation. Its preceding strict lint failure remains
recorded; the lint-cleanup successor now passes all six worker checks, including
strict Clippy and all three serial controls. The parent joint candidate passes
two facade, two root-cfg, 175 library, 67 scan and seven enclosing controls,
formatting, strict workspace Clippy and architecture. Fresh integrated milestone
acceptance remains pending.

M015 remains in progress. These are discovery/metadata adaptations, not full
stock run-pass fixture acceptance, guest execution, JIT, release or Linux acceptance.

Repair: `artifacts/macos-first/m015-generated-facade-v3-acceptance.json`.
Worker: `artifacts/macos-first/m015-proc-catalog-cfg-names-v12-worker-acceptance.json`.
Procedural witness: `artifacts/macos-first/m015-proc-catalog-cfg-names-v9-worker-delivery.json`.
