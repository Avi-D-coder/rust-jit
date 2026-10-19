# M019/M020/E-R5-005: entered original annotation completion

Solo /root, LOCAL-049; baseline7da4e0dbfa. Unchanged runtime witness:
`tests/ui/coherence/coherence-impl-in-fn.rs`, editions2015/2018/2021/2024.
V299 fails before executor observation at local_containers::resolve_header_target
(pending selection). Original cold header queries now work; ordinary body entry
still builds declaration tables without the actual annotation completion.

Invariant: one completion/name mechanism must feed cold and entered lexical
prefixes. Do not catch the header error, skip the impl or treat pending as absent.
Do not match moved source declarations by spelling, display text or copied token
origins. Preserve actual source/edition/hygiene, owning body and structural edits.

Shared routes: actual SourceDiscovery original group/fragment receipts -> completed
source environment -> BodyEnvironment coordinator -> PreparedSourceLocalPlan ->
DeclarationNameTables/LocalTypeNames -> ordinary member publication and VM.
Reuse the existing SourceOccurrences::Original initial syntax/range map to join
actual retained headers to normalized initial declarations, authenticated against
the original declaration plan. Keep original byte offsets distinct from normalized
coordinates. Prepare this immutable per-body projection once, under the actual
retained source seed and published source/cfg/epoch; query providers consume the
result rather than doing IO, expansion or source-world scans.

Factor completion projection and table augmentation so cold and entered paths
share retirement, generated names and pending scope/import handling. A receipt
without exact adopted outputs or a structurally generated/copied occurrence may
not retire an original annotation. Do not change trait/method/numeric semantics,
introduce a checker or start guest JIT. The original/generic/generated owner and
member/dictionary/solver joins remain held where not actually integrated.

Costs: cold selected-body declaration capture against the retained initial tree,
immutable Arc handles/maps and ordinary lexical lookup. No per-operation planning,
whole-world comparison, BodyIndex copy, accounting or new limit. Preserve the
existing lazy declaration discovery/executable boundary and cold-body controls.

Evidence: unchanged full main executes (not merely a header probe), all editions;
actual structural initial/header/source match, copied/generated/changed source,
wrong body/cfg/epoch and absent completion refusals; prior cold head, input/cache
and ordinary local-method/drop/Copy controls. Record all eight recurring failure
modes and actual terminals/fingerprints; named-output/import/replacement/macro
context entries not run remain gaps. Never close an architectural/task/runtime/JIT
gate merely because a new receipt or one original program succeeds.
