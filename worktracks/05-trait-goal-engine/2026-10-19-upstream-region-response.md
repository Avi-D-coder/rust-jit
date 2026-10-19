# M020 — actual next-solver region response collector

Solo root/LOCAL-049/WT05-02, baseline9f7b82df4826a66cfae9c19e9119da7f4c59a338.
E-R5-014/spec53.0 requires actual delegate response semantics before root entry.
Unchanged upstream runtime witness remains traits/multidispatch1.rs (V356
configured run fails ambiguous get); this component cannot pass its original main.

Extend actual source Q69/Q70 -> Encoder -> Cx -> InferenceContext/shared collector
with pinned compiler delegate make_deduplicated_region_constraints, canonical
make_query_region_constraints, with_region_constraints, region_assumptions getter
and register/take assumption operations. Existing actual TypeOutlives records,
seven ConstraintKind variants, visibility, combined undo and snapshots remain the
owners. No fake full SolverDelegate or default callbacks/empty semantic answers.

Use exact pinned licensed originals. Current compiler RegionConstraintData and
is_empty replace the obsolete RA member_constraints field and unused
MemberConstraint record. There are no producers/consumers of that vector beyond
its declaration/is_empty; current compiler membership uses solver-region formulas
already retained through InferCtxtLike. Do not ignore a legacy vector or add a
new runtime refusal to marshal it. Nonempty source opaque/formula integration
remains held; this removal does not claim that feature complete.

QueryRegionConstraint diagnostic category projects to actual incoming Span named
origin; do not invent a ConstraintCategory. Preserve equality versus outlives,
sub/sup inversion, type outlives Unreachable visibility and assumption payloads.
The actual delegate's stable insertion-order dedup merges visibility with the
upstream or method. Collection clones state without consuming it, including
inside speculative probes. No independent region solver or rollback algorithm.
Do not compile/import the legacy canonicalizer/query response solver; only the
pure response helper used by the current pinned next-solver delegate is compiled.

No parser/query/IO/macro or guest execution in callbacks. Cold collection and
dedup have the actual upstream O(n) expected map traversal/owned vector costs;
no installed dispatch/namespace census/BodyIndex clone/full-file search. One
ordinary/generated route across all editions. No local unsafe, limits/budgets,
sandbox/static checks/native guest compilation/JIT or whole-body preparation.
Source-ready guard stays closed for incomplete actual metadata.

Required evidence: deterministic original/projection audit, frozen async macOS
1.99.0 library/CLI builds; actual source Type/Span contexts with all seven region
kinds, equality/outlives distinction and direction, exact source spans, mixed
visibility duplicate joins/order, type obligations, assumptions registration/
non-consuming collection/drain/rollback/probe/independent owner, plus previous
source generic and cleanup controls. Supplemental host IR controls are not
upstream runtime/full-fixture passes. Review eight recurring modes at exact
source fingerprint and retain failures. Genuine delegate/canonical response/
full relevant facts/ParamEnv/member/VM integration and M020/interpreter/JIT gates
remain incomplete. No new stock gain or suite total without original execution.
