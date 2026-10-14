# E-M007-009: statement frontiers and opaque entered-body ranges

This corrects the original entered-body implementation, preserving the specified
pinned frontend and explicit entry semantics. `ast::Item::cast` accepts the
MACRO_CALL syntax kind even when the same node is an expression call inside a
let initializer. Treating every such cast as an outward producer incorrectly
suspended later textual lookup. A producer frontier must use the actual pinned
AST statement position, while its token tree remains opaque.

Two initial regressions are preserved in
`artifacts/macos-first/m007-entered-body-before-call-context-fix.log`. Parent
StmtList/ExprStmt facts now distinguish standalone statement producers from
expression calls. The native block-shadowing fixture confirms discovered
selection [17,17,41,29] before actual pinned source-slice expansion.

Skipping nested function/closure discovery alone also left their source ranges
covered by the outer frame. A lookup inside an unentered nested executable could
therefore resolve through the wrong frame. The retained failing assertion is in
`artifacts/macos-first/m007-entered-body-before-opaque-entry-fix.log`. Owned opaque
ranges now reject those lookups until separate entry. Macro definition syntax
and other item interiors likewise remain opaque. Tests prove nested functions
and closures contribute no declarations/producers to the entered frame and
cannot be queried through that frame. Later closure/nested-item entry and guest
execution remain separate required integrations.
