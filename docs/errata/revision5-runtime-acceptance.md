# E-R5-004 — Runtime semantics are the interpreter prerequisite

User-authorized policy decision: preserve lazy reached execution; do not implement a borrow checker or full static type checker. Users run cargo check separately. This replaces E-R5-003's all-category pre-JIT prerequisite and the earlier broad AGENTS.md wording.

The minimal witness supplied and verified in the discussion is unchanged upstream source equivalent to:

```rust
fn f() -> isize { true } // upstream expects E0308
fn main() {}
```

LRR successfully executes main without reaching f. That satisfies reached execution and cannot pass the upstream diagnostic assertion. Treating the diagnostic as an interpreter prerequisite would contradict the mandated architecture. No new runtime reproduction is claimed by this documentation change.

Section 1.2.1 now requires all applicable upstream runtime contracts, including unsupported runtime features and dependencies, plus applicable interpreter correctness gates before guest JIT. Compile-time diagnostics, static type/borrow checks, MIR/IR/assembly/rustdoc/tooling outputs have separate outcomes and do not block that gate. Classify assertions rather than directories; mixed runtime assertions remain mandatory while full-fixture acceptance remains incomplete until every original assertion passes. Preserve complete inventory, all editions, host applicability and provenance.

Reports name upstream_runtime_acceptance, diagnostic_compatibility, compiler_tooling_compatibility and full_suite_compatibility. No subset, external cargo/rustc result or correct generic unsupported diagnostic can satisfy required runtime execution. LRR does not gain an implicit cargo-check invocation or a compiler-backed validation mode. No broader static checker implementation is required by retaining compiler-only inventory rows.

CA037 remains pending and now includes positive/negative classification controls: cold E0308/static borrow/MIR/rustdoc assertions do not gate JIT, unknown or unsupported runtime cases do, and mixed runtime-only evidence never manufactures a full pass. Passing these harness controls does not itself establish the upstream runtime gate. Existing task order, statuses and receipts remain historical; no implementation task is completed by this policy.

Document/ledger checks and hashes are retained in artifacts/spec-revision5-runtime-acceptance/. They establish policy/document consistency only, not interpreter runtime acceptance.
