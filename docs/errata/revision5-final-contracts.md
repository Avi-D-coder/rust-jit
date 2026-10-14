# E-R5-003 — Final resource, compiler lifecycle and acceptance contracts

Superseding user decision: [the runtime-semantics policy](revision5-runtime-acceptance.md) replaces this record's all-category pre-JIT prerequisite. Static type/borrow checking and compiler/tooling acceptance do not block guest JIT. The earlier scope discussion below is historical; the complete inventory remains required.

October 13, 2026. User-authorized specification clarification. Implementation and runtime acceptance remain pending; this record is not a performance claim or JIT authorization.

| Minimal witness or missing contract | Correction | Pending acceptance |
|---|---|---|
| §62.6 formerly released physical reservations on native exit even when reconstruction had made them the backing of live interpreter objects. | Adopt consumed backing into the canonical arena, retain it until ordinary retirement and release only unused reservation capacity. | CA034 |
| §12.2 encoded physical arena indexes while §62.6 required virtual logical identities without a binding rule. | Separate typed logical ordinals from physical bits; publish physical entries only for materialized live values, bind aliases once and retain bounded live metadata. | CA034 |
| Unit B finishes compilation while unit A loops; draining only A's applicable completion can leave B's output blocking the sole compiler. | Bounded all-job result pump, separate receiving/validation/publication/selection states and explicit stale/cancelled/disconnected disposal. | CA035 |
| Cooldown and retry constants were fixed without evidence that they were optimal or a way to identify a tuned run. | Version initial defaults as SchedulingPolicyV1, record stable effective settings and test bounded alternatives without changing guest semantics. | CA036 |
| Full-suite prerequisite coexisted with feature exclusions and differing compiler-diagnostic semantics. | Complete pinned inventory, reconciled category/revision/edition/platform reports and required unsupported/contract-blocked backlog. Gate stays closed until all applicable configurations fully pass; no subset denominator substitutes. | CA037 |

Section 1.2.1 resolves the inventory/reporting definition, not the underlying feature/diagnostic gaps. Profile conflicts require explicit witnessed specification decisions before they can pass; the pinned frontend and no-live-compilation/authority rules remain binding. Linux-only configurations remain deferred under the existing macOS-first policy and prohibit a cross-platform acceptance claim.

The specification and docs/tasks.json retain Section 57 order and historical component statuses. New per-task acceptance is separately pending. Existing tier-transfer fixtures remain required; CA034–CA037 supplement them. Mutation controls reject omitted fixture rows, inherited passes, missing lifecycle/ownership rules and the former premature backing-release clause.

Reproduce with Python 3.11+ and the pinned host toolchain:

```sh
python3 tools/spec_consistency_audit.py
python3 -m unittest discover -s tools -p test_spec_consistency_audit.py -v
cargo +1.99.0 test --locked --offline -p xtask --bin xtask tasks:: -- --test-threads=1
```

Collected document/ledger results and hashes are in `artifacts/spec-revision5-final-contracts/`. No runtime task was integrated, so no interpreter/JIT milestone pass is claimed by these checks.
