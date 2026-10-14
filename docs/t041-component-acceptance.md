# T041 reference component acceptance

T041 has component acceptance under Section 57 and E-57-001 on macOS ARM64,
Rust 1.99.0. The actual combined MAIN runner exited 0; independent collection
verified all ten checks, fresh gates 00–03, 2,235 passing project host/doc tests,
all 62 registered T041 controls, and the unchanged 3,689-input source freeze.
The fresh four-program arithmetic CLI control executed 29 original assertions.

The controls exercise checked field/index/dereference projection, same-location
scalar assignment visible through references, invalidation after whole-owner moves,
shared-write refusal and serialized mutable guest aliases. Written type identity,
access, active variants and enclosing ownership remain checked. Source boxes are
included in metadata budgets; refusal precedes the guarded fuel/publication effects.

These are expression, prefix and direct-frame adaptations outside unchanged rustc
imports. They do not establish full fixture execution or compiler diagnostic parity.
The ledger preserves 13 pending full-integration fixtures and 23 dependencies for
ownership/Drop, traits and source binding, containers, JIT and the final oracle.
Full T041 acceptance, release completion and Linux acceptance remain false.

[Acceptance and control traceability](../artifacts/macos-first/t041-component-acceptance-v1.json).
[Collected milestone](../artifacts/macos-first/t031-t034-t040-t041-combined-main-milestone-v1/independent-collection.json).
