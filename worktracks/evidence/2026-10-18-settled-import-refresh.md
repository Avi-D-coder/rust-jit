# Refresh ordinary imports after producer completion

Root, LOCAL-045, T008/M025. V12 unchanged issue-17718 with its actual aux source
remains incomplete while the failure-only current graph lookup reports its
Ordering target Defined in Type, Absent in Value/Macro. That alone does not prove
the entire import is ready. Source inspection identifies stale scheduling state:
ResolveImport jobs consult SourceDiscovery.imports; child/item impl completion
releases graph frontiers without refreshing that snapshot. With no producers
left, an import can wait on its own identity, so those completed frontiers cannot
wake it. The sealed-path refresh is too late to unblock an incomplete job ledger.

Invariant: a settled discovery ledger must use the current ordinary import fixed
point before reporting missing runtime preparation. Use the existing
NameNamespaces::resolve_imports service at the actual incomplete boundary. Retain
its full current typed result. If it newly resolves an import relative to the old
snapshot, notify the existing work ledger of namespace progress and poll again;
the real ResolveImport job must still complete through its ordinary selected
result. No successful result is fabricated from a single defined namespace, no
blocked job is discarded, and genuinely unresolved/ambiguous results stay held.

Only expand/source_discovery.rs changes. This is source preparation, never an
installed opcode/call-entry path. At a stalled boundary, the existing resolver
memo avoids rebuilding unchanged graph state. Each retry requires an actual newly
resolved identity; no time/fuel/iteration cap or infinite unconditional retry.
No new parser, resolver implementation, evaluator, static type check, macro/name
case, authority grant or runtime effect. Existing strict seal and selection
revalidation remain. All eight recurring modes reviewed: same source/name engine,
no execution/pattern changes, no dispatch planning/global census/body copies.
Full dynamic publication/ambiguity matrix is not closed by this source fix.

Sole-validator acceptance: once on next exact build, unchanged proper-aux root,
plus existing ordinary import/discovery currentness controls as appropriate.
Keep V12's actual failure and record exact successor hashes/terminals. Progress
past preparation is not a whole-runtime or full-fixture pass.
