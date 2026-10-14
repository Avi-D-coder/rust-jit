# M007 original local-binding candidates

Explicit function-body entry now retains owned candidates from parameter and
`let` patterns. Each record includes the semantic NFC name, original raw spelling,
name and pattern ranges, lexical block ordinal, visibility position, origin form,
stable identity and syntactic classification. Identity encodes the containing
definition, block ordinal, form, semantic name and duplicate ordinal with fixed
tags; initializer bytes and snapshot-relative ranges are attachments.

Parameters are visible at the entered body start. Let candidates become visible
after the entire statement, keeping a shadowing initializer and let-else branch
outside the new binding's visibility. cfg/cfg_attr filter parameters and complete
let statements. Nested functions, closures and templates retain their existing
opaque-entry rules. Explicit nested-function entry collects its own parameters
and lets. Pattern traversal prunes expression, item and token-tree subtrees,
while the ordinary body traversal assigns any entered nested block its own scope.

Mut/ref/at patterns and record shorthand are explicit syntactic bindings. Other
identifier patterns remain BindingOrConstant candidates because source constants
and constructors require later resolution; their names are not automatically
published as local variables. Alternative pattern occurrences retain separate
source identities and a common pattern range. Equivalence/validation is pending.
This avoids assigning bindings by spelling or doing live rustc type checking.

Checked limits admit at most 100,000 candidates, 32 MiB candidate metadata and
one million pattern syntax visits per entered catalogue. Pattern traversal and
cfg queues use bounded checked allocation frontiers. Three source acceptance
checks cover parameters, tuple/at/ref/record/or patterns, visibility, scopes, cfg,
raw/NFC names, stable identities across initializer edits and nested opacity.

Receiver/loop/match/let-chain bindings, pattern-constant resolution, alternative
binding equivalence, additional source declaration-rib integration and generated-local integration
remain pending. No VM execution or native differential claim is made for these
source facts. M007 and release remain incomplete. Actual macOS evidence:
artifacts/macos-first/m007-original-bindings-checkpoint.json.

Original binding/rib lookup now has separate source/native acceptance in
[source-local-lookup.md](source-local-lookup.md).

Original for-pattern scopes now have separate source/native acceptance in
[for-binding-scopes.md](for-binding-scopes.md).
