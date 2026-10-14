# E-M015-005: expected-panic metadata accepts stable name-value strings

The unchanged official rustc `tests/ui/test-attrs/test-should-panic-failed-show-span.rs`
uses six tests: two bare expected-panic attributes and four stable name-value
`#[should_panic = "message"]` attributes. Its retained run-output files describe
body execution expectations, which the LRR metadata control does not execute.
Upstream `compiler/rustc_attr_parsing/src/attributes/test_attrs.rs` also declares
Word, List and NameValueStr forms stable and decodes the latter as a string reason.

With the disclosed fixed Rust2024/test-cfg metadata adaptation, the original
whole source and a real declarative expansion retaining its exact root prefix
and item suffix each reach an actual seal and six final identities. All four
name-value forms carry `LRR-E-TEST-DISCOVERY: invalid expected-panic attribute`
and no expected reason. The two bare forms remain valid. Both strict selected
controls fail after confirming real seals, source/definition identity and zero
body queries. The final metadata table is retained; a seal alone is not correct
attribute interpretation. No stock run-fail execution, macro host or native
preparation ran. The fixture's 2015..2021 edition coverage remains unexecuted.

The collector accepted only bare and list forms. The private repair recognizes
the exact three-token name-value shape and uses the same existing pinned string
decoder as list reasons, retaining list trailing-comma checks and typed refusal
for non-string reasons. Parent supplements require escaped and raw literals to
decode identically and never invent a panic reason for an integer literal.
Neither the collector change nor the component grants guest execution or unwind
support. Native panic execution, full fixture and integrated milestone acceptance
remain separate requirements.

Official worker witness: artifacts/macos-first/m015-catalog-panic-forms-v1-failure-worker-delivery.json.
Parent witness: artifacts/macos-first/m015-panic-name-value-parent-witness-v1-failure.json.
Private repair: artifacts/macos-first/m015-panic-name-value-parent-v1-source-snapshot/manifest.json.
Parent joint candidate passed all 11 checks, including 179 library, 67 scan,
two each panic-form/nested-route/edition/facade/root-cfg and seven enclosing
controls, format, strict Clippy and architecture. The unchanged worker assertion
harness passes both original/generated cases and strict checks after copying only
the exact parent engine repair. The integrated macOS milestone passed 1974 host/doc checks against 1144
frozen inputs, with gates 00–03 and all nine checks successful. M015 is not complete.
See artifacts/macos-first/m015-arena-panic-main-acceptance.json.

Joint repair: artifacts/macos-first/m015-arena-panic-joint-v1-acceptance.json.
Worker repair: artifacts/macos-first/m015-catalog-panic-forms-v2-worker-delivery.json.
