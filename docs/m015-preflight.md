# M015 preflight — pending M014 component acceptance

M014 main validation is still running. This is an integration review, not M015
implementation or acceptance.

The existing `engine/test_catalog.rs` collects original source metadata and refuses
unresolved transforming attributes/derives and macro blockers. The CLI filters
this catalog before selecting test bodies. Generated call selection already uses
the existing declaration contexts, source/header locators and owned generated
syntax through `frontend/callables.rs`; catalog integration must preserve that
identity and revalidation path.

M015 must enumerate final active test declarations only after TestCatalogSeal.
Replacing attributes can remove or rename an original test, and derives or
function-like macros can add tests. Ignore and expected-panic metadata come from
the final attributes. Required expansion failure precedes filtering and cannot
become an empty catalog. Unsupported generated async bodies stay visible; their
execution restriction is checked when selected. Discovery must not fetch function
signature or executable body recipes merely to list metadata. Unselected invalid
signatures remain metadata under Section25.2.

Relevant unchanged official rustc sources to reuse after the prerequisite passes:

- `tests/ui/test-attrs/test-vs-cfg-test.rs`: preserves the distinction between
  `--cfg test` and requesting a test harness. Retain its reference/directives.
- `tests/ui/test-attrs/run-unexported-tests.rs`: a private module test remains a
  discoverable test; retain its native run-fail/check-stdout directives.
- `tests/ui/test-attrs/test-cant-be-shadowed.rs` and its `test_macro.rs` auxiliary:
  ordinary macro textual bindings must not replace the compiler test attribute.
- `tests/ui/test-attrs/tests-listing-format-default.rs` and its expected stdout:
  sorted test metadata includes ignored attributes. Preserve native stdout as
  an oracle; LRR's structured CLI output and selection rules are tested separately.

These files are reviewed in the official checkout, not yet imported or counted
as passes. Copy original bytes, auxiliaries, directives and expected outputs under
the existing rustc licenses when implementing this task. Add separate LRR controls
for actual generated names/attributes, required-expansion failure despite filters,
source/epoch invalidation and visible unsupported bodies. MX031/MX032 remain pending.
