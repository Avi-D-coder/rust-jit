# Cold reconstruction of a missing prepared dependency — LOCAL-024

Pre-code October16, T040/M018. A current memo root can remain externally pinned
while its unpinned descendant memos are evicted. A later ordinary installation
fetches the valid cached root, but the old readonly capture refuses because its
transitive child memo is absent. Minimal witness is the existing pure synthetic
Dependencies key1 -> key2; primary language integration remains unchanged
borrowck-assign-to-subfield.rs through the actual BoundRecordConstructor query.
This is a cache-availability repair, not a new source/name authority or evaluator.

Actual API:
`Database::prepare_input_guard(&mut self, provider: &impl Provider,
 key: &QueryKey, expected: Digest) -> QueryPoll<PreparedInputGuard>`.
The normal complete memo closure immediately reuses the existing readonly
capture. A private typed Missing-descendant result, never a diagnostic string,
permits one bounded owned traversal. Only actually absent descendant memo keys
are cold-fetched through the real provider. Exact stored expected fingerprints,
cacheability, positive/negative input stamps and conflicting observations remain
mandatory. Changed output, uncacheable result, typed provider error and genuine
NeedInput propagate before installation. Missing root remains a refusal; this
API does not select a new root or refresh a stale guard by value/name/layout.

The installed guard remains readonly and performs no fetch. Root owns the tiny
InstalledRecipes cold call-site migration. No private side-cache, global name
map, body deepcopy or runtime fallback is introduced. The old capture wrapper
remains for existing controls, sharing one readonly kernel, and cannot provide
missing descendants itself. Successful cold fetches are ordinary pure memo work
and retain existing Database accounting/pins; the new temporary traversal is
checked for100000 edges and32MiB scratch/retained ownership, with explicit sizes.
Normal fast capture retains its existing checked32MiB final guard budget. A
single readonly attempt plus one owned walk avoids repeated whole-root walks for
each missing child: bounded O(V+E) cold graph work, excluding provider work.
Installed currentness remains O(D log I) over actual leaf stamps, no provider.

Controls use existing pure query test keys/provider. Retain a real root Outcome,
evict only its child under the actual pinned-root budget, require root sharing
and cold child-only repair. Changed child output must reject the stored edge;
a genuine unavailable compile-environment request must propagate without a guard.
Old guards must remain invalid after positive input replacement, even when a
fresh derived query result has unchanged semantic output. No fake guest suspend
or broad Err-pass is permitted. All controls uncompiled/unexecuted until sole
validator runs an immutable packet; this component does not claim main success.

Eight-mode source review: no evaluator (1), literal/break route (2), context
numeric/member rule (3), origin exclusion (4) or pattern machinery (5). It fixes
cold cache reconstruction only and forbids hot re-entry (6), retains actual
positive/negative dependencies rather than a world census (7), and traverses
memo keys/immutable values without copying body/index structures (8). Existing
coarse source/seal dependencies and other architecture matrix gaps remain held.


Implementation source review (exact `0b0b0dfbfdef49b55155b49c3c7a5891d25c10eab51656c810f99caf2b8ce9eb`): private CaptureFailure separates
Missing descendant from Invalid diagnostic; no message matching controls cold
fetch. Complete current closures use capture_input_closure directly. On actual
missing descendant, one cold fetch verifies that precise observed edge, then a
single owned key worklist completes the DAG. Every stored query fingerprint is
checked before visited dedup. Only absent memo keys fetch; a missing root refuses,
changed output refuses, uncacheable result refuses, provider error/NeedInput
propagates. Final actual input/missing stamps remain checked. No obsolete
production readonly-capture caller remains after root install migration; the
old public(crate) wrapper is cfg(test), sharing the readonly kernel.

Owned repair precharges query work/visited/transient key copies before reserves
and clones, input map rows before clone, and final guard storage before reserve.
Cumulative32MiB scratch+retained charge is conservative, not a speed claim.
100000-edge bound counts each visited memo's complete dependencies; repeat edges
still require exact expected fingerprints. One initial borrowed partial walk
and one owned walk are O(V+E), aside from real cold provider work. Normal complete
capture and hot readonly guard retain their prior budgets/costs. Temporary key
copies contain no immutable parser/body/index clone or whole namespace map.

Controls: new retained_current_root_cold_repairs_only_its_evicted_child retains
actual root Outcome, uses actual cache pinned floor and verifies child-only
victim/provider re-entry, same root allocation, fast complete closure no provider,
and no stale leaf authority refresh. New missing_child_changed_output_or_real_demand_cannot_mint_a_guard
uses the same deterministic pure VersionedDependencies provider for changed
positive input: child output mismatch, actual missing UnitCompileEnv with typed
CaptureCompileEnvironment request, and uncacheable output all refuse guard mint.
This is query-protocol evidence, never fake guest suspension or language success.
The old eviction test also now uses actual pinned current-input floor instead of
query_bytes0; both root and child must still disappear and guard use must add no
query trace. Preserve predecessor failed bed679/1: mandatory currentinput rows
made its zero cap invalid (source review23be53/0), not a language runtime fault.

Root installed cold caller uses prepare_input_guard with real SyntaxProvider and
propagates ReadyErr/NeedInput before source handle/recipe installation. Shared
require_current remains readonly and performs no fetch. This worker changed no
install/VM/root provider source and ran no Cargo/native tests. Source hashing,
rustfmt and separate source reasoning are not compile/runtime acceptance.
All four guard controls remain unexecuted on this successor, including two
previous controls whose prior source outcomes stay attached to predecessor bytes.

Independent eight-mode pass: modes1–5 unchanged ordinary source binder/VM,
no evaluator/origin-specific inference/refusal/pattern path added; mode6 fixes
valid cold memo reuse, with no installed planning; mode7 uses actual positive/
negative query input closure and retains stale fingerprint checks, while coarse
inherited source/seal dependencies remain held; mode8 only bounded QueryKey/input
clones for mutable cold query re-entry, no body/index clone or AST/file search.
Unchanged upstream A/B constructor/query integration is still mandatory; this
synthetic cache protocol bank is not substituted for its runtime/main/cleanup.
