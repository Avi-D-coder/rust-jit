# E-R5-002 — Interpreter preparation and tier-transfer contracts

October 13, 2026. User-authorized specification amendment. This changes requirements only; all seven new acceptance fixtures remain unexecuted. It does not start guest JIT work, complete an implementation task or establish a speedup.

| Minimal witness or gap | Required correction | Pending acceptance |
|---|---|---|
| The integer branch of `vm/step.rs::prepare_literal` rescans suffixes, normalizes text and parses digits on repeated execution; §11.3 formerly limited recipes to spelling data. | Decode immutable lexical data in a reached recipe; retain fresh evaluation bindings, source-aware inference and reached diagnostics. | CA027, T022/T024 |
| A loop has executed N iterations in the interpreter, but exit maps alone do not define import into an optimized header. | Verified entry map, pre-entry guards/reservations and single ownership commit; rejected entry continues at the same PC. | CA028, T070/T073/T085/T086 |
| Background compilation finishes while native backedges never return to the dispatcher. | Nonblocking readiness checks at existing bounded polls; explicit TierTransfer status/helper; owner validates and installs outside the native loop. | CA029, T088 |
| Baseline runs after 32 interpreter traversals; an interpreter-only observation stream cannot then reach the later optimization threshold. | Actual native traversal/backedge deltas feed eligibility at bounded safepoints without double counting. | CA030, T071/T074 |
| One-op suppression allows the same failing variant to be entered on every next loop iteration. | Exact shape selection, bounded failure counters, cooldown and probation; fifth shape uses fallback. | CA031, T073/T086 |
| Scalar replacement eliminates a managed allocation while §12.2 still requires deterministic handle/live-byte limits; deopt after an effect may otherwise need an unavailable allocation. | Separate logical charges from physical backing; preserve logical failure points and reserve reconstruction before optimized ownership. | CA032, T070/T085/T086 |
| §62.11 permanently excluded another automatic attempt after one nonprofitable observation, even if later workload behavior changed without a source edit. | Retain costs, require fresh evidence and profitability, double cooldown intervals, cap retries across variants. | CA033, T074/T089 |

These are minimal source/design witnesses, not reproduced JIT runtime defects. Entry maps and TierTransfer update the pending ABI2 digest, closed helper registry and cache compatibility. Public service/internal framing versions and the Section 57 task order are unchanged. Existing component statuses and receipts stay historical; per-task amendment acceptance is separately pending.

Reproduce document validation with Python 3.11+:

```sh
python3 tools/spec_consistency_audit.py
python3 -m unittest discover -s tools -p test_spec_consistency_audit.py -v
cargo +1.99.0 test --locked --offline -p xtask --bin xtask tasks:: -- --test-threads=1
```

Mutation controls remove required clauses, remove a normative fixture row, reintroduce permanent exclusion and incorrectly promote new acceptance using an old component pass. These are textual/ledger checks, not semantic proof or runtime acceptance. Results and source hashes are retained in `artifacts/spec-revision5-tier-transfer/`. No runtime milestone gate is claimed by this document amendment.
