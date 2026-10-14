# M015 compiler Test attribute ordering — open implementation gap

The invariant is that actual macro kind and pinned scheduler order determine the
input delivered to a procedural attribute. A source spelling alone does not
certify a compiler attribute, and native references do not certify LRR acceptance.

The unchanged official fixture `tests/ui/proc-macro/test-count-attributes.rs`
and its same-named auxiliary are retained in the licensed checkout import. The
auxiliary asserts the number of remaining attribute tokens in the actual macro
input. Explicit Rust1.99.0 compilation on aarch64-apple-darwin succeeds and lists
five tests. See [the captured reference](../artifacts/macos-first/m015-test-count-native-hs_zm0dg/report.json).
This is compile/list reference evidence; test bodies were not executed and neither
full compiletest nor LRR fixture acceptance is claimed.

A leading compiler `#[test]` is consumed before a later procedural attribute.
Later test/should-panic attributes remain visible to an earlier procedural
attribute. The current LRR generated prefix certificate permits a preceding
compiler Test marker to remain in the materialized macro input. That input-ordering
path needs a regression against the unchanged auxiliary and a correction that
preserves source/hygiene identities and explicit macro execution authority.

The earlier replacing-attribute witness also produces native E0425 when a
following attribute removes the function after compiler Test registration. See
[the native registration witness](../artifacts/macos-first/m015-leading-marker-witness.json).
The existing LRR control exercises source-prefix replay and final item removal;
it is not native scheduler compatibility acceptance.

The correction must retain source-bound compiler-test metadata separately from
consumed macro tokens and replay its actual resolved kind at sealing. Renaming,
removal, generated/physical/cold ownership and helper scope must be exercised.
No alternate scheduler or language-profile rule has been adopted. No specification
contradiction or erratum is claimed from these observations. M015 stays in progress.
