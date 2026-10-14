# M013 component acceptance audit

M013's available discovery/worklist component is complete under Section 57,
E-57-001. This permits the next ordered task, M014. Full MX027/MX028 acceptance
and release remain pending; component evidence never substitutes for the final
off/auto/force, fresh-worker and full-fixture matrices.

| Requirement | Current implementation and exercised evidence |
| --- | --- |
| Section 50.1 deterministic work and frontiers | `expand/discovery.rs` uses stable source/module order, typed completion tickets, sorted ready/blocked indexes and checked metadata/count growth. Tests preserve active work on refusal, prevent duplicate/reordered tickets and refuse seals while producers remain. |
| Complete membership and dependency identity | Private seals bind domain, unit graph, epoch, sorted members and dependencies. Scanned sources and expansion records remain dependencies even when they add no chosen name. Certified absence invalidates on edits. Wrong owner/domain/epoch and post-seal changes are refused. |
| Section 50.2 source and generated fixed point | Captured original/ordinary/generated modules apply cfg, retain actual source identities and integrate headers/imports/macros. Blocked names wake after actual output admission. Demand-driven macro definitions stay lazy. Generated import chains, nested/outward macros, dependency aliases and originating reexport cycles are exercised. |
| MX027 available interpreter acceptance | `mx027_discovery_generated_import_and_macro_reach_fixed_point` executes the real CLI source: an earlier macro generates an import and a later-needed macro; reached generated code returns 2129. An explicitly isolated native reference matches. This is interpreter component evidence, not JIT acceptance. |
| MX028 available interpreter acceptance | `mx028_discovery_refuses_unexpanded_cycle_and_resolves_originating_reexports` requires catalog-incomplete, nonzero exit and empty stdout for an unresolved producer cycle. A separate originating reexport cycle resolves and executes generated code returning 2131, with a native reference. No partial seal or guest execution is accepted for the unresolved case. |
| Authorized procedural discovery | MacroRun owns frozen matched sessions and expansion budgets; pure providers suspend. Exact source/output/epoch/owner checks gate admission and publication. Generated and original attribute/derive worklists preserve ordered append output, helpers and canonical source identities. Printing macros use the registered matched-source server repair; the installed-server failure remains recorded. |
| Section 50.5 immutable publication | Completed source/generated records and private freshness certificates publish atomically through the owning run. Incomplete discovery cannot publish a partial certificate. Actual source/tool/environment edits invalidate readiness. Later membership changes fail seal-violation rather than restarting completed guest effects. |
| Section 50.6 current bounds and failure paths | Shared budgets enforce nesting/invocation/token/output/cumulative limits, and 10-second invocation/60-second cumulative deadlines. Import/header/work metadata growth is checked before integration. Current token/depth/aggregate refusal, startup timeout/reaping, stale ownership, denied authority and bad protocol controls pass. Larger stress and containment matrices remain M045 work. |

The preceding macOS checkpoint contains 1684 host/doc test passes and all 731
M013 controls, including the two named CLI cases. It verifies thirteen unchanged
rustc imports and 32 file hashes, with 36 component assertions. Full rustc
compiletest and LRR fixture acceptance are zero. Existing accepted controls were
added and executed before this transition; this audit does not add a test that
merely mirrors ledger fields. The transition Gate03 passed again with 1684 host/doc tests and all 731 M013
controls; the existing ledger/release validator checks component and pending
integration fields. Formatting, strict Clippy for runtime/host tool, architecture
and first/final manifest audits pass. The restricted empty-derive interpreter/native
reference was rerun. Fifteen cases and 35 unchanged files are now imported; the two
M014 cases remain unexecuted and do not increase the 36 component assertions.

The following obligations are preserved, with explicit later dependencies:

- Section 50.3 nested impl/expression/closure/async/anonymous-const producer scans
  are M014. Complete associated headers, symbolic impl patterns and dispatch
  invalidation also require the later term/goal/dictionary milestones, through
  M026. Current ScopeSeal evidence cannot prove ImplCatalogSeal completeness.
- Section 50.4 transformed test enumeration, final-name filtering and unsupported
  generated test entries are M015. Scope membership cannot stand in for a test
  catalog. Section 50.5 publication of all required impl/test seals before guest
  execution must integrate those tasks before full acceptance.
- Complete generating chains, edition/context diagnostics, redaction and exact
  legacy/helper diagnostic compatibility are M016. Current owned provenance and
  typed refusal evidence do not prove their rendering contract.
- Actual guest JIT equivalence, witness barriers, portable/fresh-worker cache
  eligibility and no duplicated guest effects require M035-M037.
- Full live/build-host/proc-macro-host/native graph integration is M038. The
  current matched macro-host service does not establish the native runtime bridge.
- M043 must execute every MX027/MX028 variant through the full LRR oracle. Both
  final fixtures stay pending with no full_lrr evidence paths.
- M045 supplies exhaustive process/protocol/aggregate-resource fault injection.
  Existing bounded controls are retained without claiming unexercised stress,
  whole-request containment, complete native/JIT behavior or Linux acceptance.

No current requirement is removed, skipped or marked passed by these deferrals.
M014 follows this component transition in the fixed cross-ledger order. The full
revision-4 project goal remains active on macOS first.

See [the component checkpoint](../artifacts/macos-first/m013-acceptance-checkpoint.json) and [M014 preflight](m014-preflight.md).
