# M013 captured external-module macro-use exports

M013 remains in progress. Bare macro_use on an external module now creates the
same lexical export edge as an inline module, with the module-source placeholder
retained until a lease-matched captured child can supply complete local bindings.
The shared local-binding traversal checks pending scopes even when there are no
known names, so an unknown child producer cannot become an empty successful export.

Discovery synchronizes ready child exports bottom-up through the already validated
captured-module hierarchy. Scope, profile, cfg and source-publication identity must
match. Owned declaration metadata and explicit origins are copied with bounded
admission; the placeholder retires only after publication. Namespace changes wake
blocked jobs without fabricating a new expansion or executing a macro provider.
Imported definitions are distinguished from owned definitions, so body capture
continues to select the actual original/generated source owner.

Completed macro indexes now retain the exact linked state through a bounded owned
snapshot rather than rebuilding a root-only index and losing external exports.
The existing immutable source lease remains required. No AST handle or arbitrary
native symbol is stored, and no query or macro/native authority changes occur.

Nine controls cover original/generated/nested child exports, source-order shadowing,
unknown child nonpublication, foreign lease refusal, child-edit seal invalidation,
actual native macOS comparisons and the real CLI. The edited child executes its new
value in the same database after the stale seal is invalidated and refused. Four
isolated native projects match actual VM values. A CLI capture/run returns 577 and
is also a registered acceptance test. Initial/partial failures and the corrected
explicit-binary CLI command log are retained as implementation evidence.

Extern-crate/selective macro-use forms, remaining cross-fragment/generated module
capture, complete origin invalidation, Cargo/world/definition-unit facets,
generated-body runtime macros, return/generic aliases, full quota/failure boundaries,
complete MX027/MX028 fixture/JIT and release acceptance remain mandatory. M014–M016
follow Section 57 order. All evidence is macOS ARM64 Rust 1.99.0; Linux is a later
port and no full fixture or release claim is made.
