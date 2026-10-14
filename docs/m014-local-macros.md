# M014 cold lexical macro demands

M014 remains in progress. This integration captures active local `macro_rules!`
definitions and lexical block/module/closure scopes without executable lowering.
Definition token trees remain opaque until an explicit demand. The scanner records
all seven invocation grammar slots from the pinned AST instead of admitting a cold
expression or statement as an item invocation.

The immutable declaration macro index reuses TextualScopes, MacroDeclarations and
the pinned source-root hygiene facts. Deterministic identity maps bind lookups to
the captured scan. Outer visibility freezes at scope opening; local definitions
shadow while sibling scopes remain isolated. Unresolved predecessor producers
suspend selection. Qualified and annotation routes remain input demands until
their namespace integration exists. No lookup executes a macro or issues a seal.

SourceDeclarative's explicit declaration demand checks parse/profile, path, unit,
root scope and cfg against the immutable source view. It captures the actual
definition and argument groups, binds their input digest and correct grammar in
the expansion key, and uses the existing pinned declarative engine. It grants no
runtime body-entry or procedural authority. Source/path/cfg mismatches refuse.
Size/count/depth checks remain in force; disabled declarations are pruned.

Nineteen targeted macOS declaration controls pass. The newly imported unchanged
rustc `macro-nested_stmt_macros.rs` resolves the cold `grault!()` and expands its
original tokens to `foo!()` and `struct Xyzzy;`, with original source spans.
This component harness uses LRR's 2024 source/call profile; the unchanged
upstream directive and its full compiletest contract remain separately pending. A cold
expression control expands an actual impl/member and retains the unresolved
ordinary body call without executing it. These observations certify expansion
transport only: statement output is explicitly refused by item-only admission.

Generated expression/statement syntax admission, nested output discovery,
transformed local namespaces, complete graph/facade impl seals and reached
symbolic method selection remain pending. MX029/MX030, full stock fixture, guest
JIT, release and Linux acceptance are not complete. The pre-correction full Gate03 passed with 1702 host/doc tests. The corrected
source passes all 19 targeted controls and Gate00/01/02 plus provisional Gate03
component with 1703 host/doc tests. Formatting, strict runtime/host Clippy,
architecture and first/final manifest audits pass. The restricted unchanged
empty-derive interpreter/native reference was rerun. Sixteen unchanged licensed
cases and 36 file hashes retain 40 component assertions; full fixture acceptance
and guest JIT acceptance remain zero.
Both original rustc license texts and unchanged copied sources are retained;
importing the new case is not a full fixture pass.

The no-whitespace adjacency regression reproduced a duplicate expansion position:
a macro definition's end offset equals the following invocation's start. Bindings
now use the definition's own start event, consistent with TextualScopes' strict
`before` cutoff. The regression expands actual generated tokens for immediate
invocations and immediate child blocks, and checks that an earlier call still
cannot see a later definition. The original failure is retained; this is an
implementation correction, not a specification/backend change. M014 remains
in progress. See the [progress receipt](../artifacts/macos-first/m014-local-macros-progress.json).

See the [corrected-source checkpoint](../artifacts/macos-first/m014-local-macros-checkpoint.json).
