# M007 original for-pattern scopes

Explicit body discovery now records for-pattern candidates in the existing loop
body block scope, using a new explicitly encoded origin tag (For=3). It admits
names only when visiting the pinned AST's actual loop_body, rather than every
block child of the ForExpr. The iterable retains its enclosing scope, including
its own nested block. Nested loops get distinct body scopes and leaving a loop
restores the outer binding. Whole-loop cfg filtering removes both iterator and
body facts. Missing iterator/body boundaries fail entry explicitly, avoiding
misclassification of a pinned parser recovery block.

Pattern macros retain their original enclosing invocation scope and source range.
They also register a declaration-producing demand in the body scope at its entry
frontier. Local body-name lookup therefore cannot turn an unpublished pattern
expansion into successful absence. The existing typed producer outcome is used;
no expansion executes here. Pattern scanning is iterative and bounded, pruning
expression/item/token-tree subtrees and admitting traversal frontiers before
allocation. Binding candidate and source-local index limits remain in force.

Three source checks cover tuple/nested-loop bindings, iterator and post-loop
visibility, cfg, pattern-macro demand/attachment scope and malformed boundaries.
An actual macOS Rust1.99 program produces six corresponding observations. Pinned
root hygiene and original local lookup select the matching source identities;
fixture literal initializer/iterator elements are read from the full original
source forest and agree with native values. This checks selected binding scope,
not guest loop execution, general destructuring or iterator evaluation.

Pattern-expansion publication, additional binding forms, closure capture,
pattern equivalence and query/service/VM integration remain pending. M007 and
release remain incomplete. Evidence:
artifacts/macos-first/m007-for-bindings-checkpoint.json.
