# M013 captured external module discovery

M013 remains in progress. This component implements original item macro invocations
in captured external modules. It uses the existing textual/hygienic resolver and
actual captured macro bodies, then executes generated functions in the tagged VM.

Each external source obtains a parent chain from captured module declarations.
Lookup borrows each parent's current index at the frozen module-opening position;
it does not copy an index before earlier expansions finish. Nearest local bindings
retain precedence. Later parent declarations remain invisible. Parent and child
must share the same immutable publication, cfg, profile and compilation unit.
Hierarchy depth and retained link metadata are checked before installation.
Missing modules remain incomplete and cannot publish a discovery seal.

Ten controls cover parent inheritance, recursive nearest-parent selection,
qualified exported owner bodies, local shadowing, missing-module refusal,
future-parent refusal, completed earlier expansion visibility, publication mismatch,
undeclared captured parents and duplicate definition identity capture refusal.
The duplicate parent witness is rejected by the existing capture validator before
module-link configuration; it is not evidence that its ambiguity branch ran.

External inherited imports, generated-fragment external parent contexts,
dependency/definition-unit routing, macro namespace query certificates,
remaining origin/quota boundaries and full MX027/MX028 CLI fixtures remain pending.
These are component controls, not full fixture or release acceptance. macOS ARM64
with Rust 1.99.0 is the validation target; Linux acceptance remains pending.
