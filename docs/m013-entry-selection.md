# M013 entry selection after completed discovery

M013 remains in progress. Standalone run now finishes and publishes actual discovery
before looking up main. Entry selection queries the existing sealed value namespace
with its exact root scope, then uses the existing CallResolver to validate original
or canonical generated function identity, signature, source/context and body handle.
The selected callable is revalidated before guest startup. Only the selected body is
prepared; nested/cold functions remain outside that entry selection.

Generated main now executes with its canonical generated body key. Missing entries
are reported only after producers complete. Duplicate original/generated candidates
are retained as ambiguity and refused with LRR-E-NAME-AMBIGUOUS before execution.
No scan by displayed function name, arbitrary candidate, fabricated body key, text
reparse, query-provider macro execution or selected live compilation is introduced.

Eight new CLI controls cover generated main, nested/root disambiguation, nested-only
entry refusal, original/generated duplicate refusal, unresolved-before-missing
classification, recursive generated entry/callee records, two distinct generated
main records and cfg-disabled generated absence. All five initial controls failed
before integration; two baseline cases incorrectly executed a nested or duplicate
entry. Existing standalone CLI controls still pass. The diagnostic is registered
with both duplicate candidate controls.

M013 still requires dependency/definition-unit and complete contextual origin
integration, generated-body runtime macro contexts, remaining type and quota/failure
boundaries and full MX027/MX028 fixture/JIT matrices. Generated test catalogue work
follows M014/M015/M016 in fixed order. These tests run first on macOS ARM64 Rust
1.99.0; full release and Linux acceptance remain pending.
