# E-R5-005 — Macro output uses the common language engine

October 15, 2026. Explicit user correction: “You cannot make macro-specific logic. The macro system is the macro system.” This replaces the old Section18.1 interpretation that permitted dedicated runtime macro recipes. Keep the pinned frontend, hygiene/expansion scheduler, lazy preparation, eager execution, capability model and all official editions.

The concrete source witness is:

```rust
macro_rules! expression { () => { 1_i32 + 2_i32 }; }
fn main() { let _value = expression!(); }
```

The committed generated-expression route decodes a real expansion, accepts only Literal/ParenExpr and directly builds an Op::Literal recipe. Its BinExpr output reaches a separate unsupported branch. This is verified from source; no new guest execution is claimed. Assertion operands separately use an AST evaluator with their own match/if/call/borrow handling. The [source audit](../audits/macro-specific-semantics.json) records eight confirmed architectural blockers, exact files/lines/hashes and limits.

All macro output must enter the ordinary source/body graph and language lowering. There is one expression engine and one generic transactional pattern engine, shared by match, conditional patterns and generated Rust. Macro-specific numeric inference, method whitelists, direct-break/literal executors and opaque scalar phase/verifier exceptions must be removed through an actual common-path migration, not renamed. Preserve source identities, per-token editions, hygiene, invalidation, exact-once effects, continuation/suspension, lifetimes, fuel, cleanup and capability authority while replacing them. Actual compiler expansion facilities remain in the pinned adapter and return generated syntax; ordinary runtime helper calls use the selected library/facade mechanism.

Pending acceptance uses unchanged upstream macro/pattern/edition cases, their auxiliaries and expected outputs. It additionally compares one expression/pattern/control operation across handwritten source, an arbitrary user macro, a macro wrapper and standard library macro expansion. Assert that all use ordinary indexed nodes and normal verifier/dispatch paths; no macro-specific language evaluator may execute. Cover caller binding capture, labels, false-guard alternative retry, exact-once calls, failure-only formatting, panic/cleanup and suspended inputs. Supplemental LRR structural regressions must inspect the common route rather than merely accepting the same final value.

Existing green component/stock receipts remain truthful observations of the old implementation. They do not establish this architecture. Canonical shortcut code is not fixed by this erratum. Cumulative-v2 and the new assertion/or successor must not be promoted, even if the frozen V123 historical gate passes. No old fixture is erased, and no newly failing runtime contract becomes inapplicable. Task statuses remain incomplete; JIT remains gated.
