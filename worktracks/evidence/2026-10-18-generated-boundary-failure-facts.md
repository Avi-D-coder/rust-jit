# Actual generated body boundary failure facts

Root LOCAL-045/WT04/T032, E-R5-005. V17 ordinary constant-pattern VM execution
passes stock const-meth-pattern and issue-17718. The all-origin LRR initializer
control fails at GeneratedSourceDiscovery::generated_body_macro_boundary's actual
output/owner check. Its single error conflates owner, discovery seal, epoch, cfg
and selected record allocation; source alone cannot justify weakening any check.
Retain the same condition/error code, adding failure-only actual equality facts
and real identities. No fallback, synthesized source, guessed owner, new query,
permission or hot-path work. The next exact capture must identify the failed join
before implementation changes; no architectural feature closure is claimed.

Source review all8: pure failure reporting only, no evaluator or macro-specific
semantics (1-4), no patterns/cold prep (5), no repeated planning (6), no namespace
scan (7), no BodyIndex/source copy (8). Reuse existing generated ownership control
and actual failed origin matrix, not a replacement miniature suite.

Source snapshot `crates/lrr/src/expand/source_discovery.rs`: `15252564e32ed47b333e9931b0fe40b48fa2b2c38681fdfe12f77b2d7ceccaaf`. Failure-only facts; all admission checks unchanged.
