# M007 original match-arm binding scopes

Explicit body discovery uses the pinned MatchArm node to create a distinct scope
for each enabled arm. Pattern candidates use the explicit stable origin tag
Match=4, preserving existing tags and identities. They become visible after the
original pattern, covering both the guard and arm expression. Nested blocks
retain their own scopes. The scrutinee and neighboring arms retain their outer
bindings. Disabled arms contribute no bindings or invocations. Missing pattern,
arrow or expression boundaries fail explicitly.

A bounded pattern scan records declaration-producing macro demands at the arm
visibility frontier. Local lookup returns typed NeedInput until those generated
names are published. The original invocation retains its source range and arm
scope; it occurs before that visibility frontier. The same scan now serves for
patterns without changing their enclosing invocation scope or body demand.

Three source tests exercise guard/body visibility, neighboring-arm isolation,
outer restoration, cfg filtering, pattern-macro demand and malformed boundaries.
A standalone native macOS Rust 1.99 program checks five original name selections.
Fixture literal @ patterns and initializer values attach values to the selected
source identities; all observations agree with native execution. This is binding
scope evidence, not guest match execution or general pattern evaluation.

Generated pattern publication, other binding forms, closure capture, pattern
equivalence and query/service/VM integration remain pending. M007 and release
remain incomplete. Evidence is recorded in
artifacts/macos-first/m007-match-bindings-checkpoint.json with 317 host/doc tests and Gate 01, strict Clippy and formatting passed.
