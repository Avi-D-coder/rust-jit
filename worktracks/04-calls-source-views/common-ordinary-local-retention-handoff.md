# Ordinary callable retained roots — LOCAL-024

Bounded WT04/T032/T033/M017/M018 review: same genuine private Value39 proof
retains actual parent Arc plan and input guard; do not create another ownership
registry or runtime semantic route. Official unchanged suffix remains witness;
root controls user/wrapper execution, sole validator owns all Cargo/evidence.

Actual shared accounting already covers the parent: QueryValue::EnteredValue
source_roots -> SourceArtifactRef::LocalPlan -> SourcePlanPins registration;
count_owner/has_external_owners detect the ordinary proof's external Arc.
completed_environment_input_roots traverses that actual plan DAG before pruning;
Roots::plan accounts context/environment/index/hygiene and ancestor roots.
Retirement wrapper remains until between-run pruning after callable release.
No extra SourceArtifactRef variant, registry or graph traversal is needed.

One production expression adds the guard Arc header (two usize) to the existing
checked OrdinaryLocalAdmission metadata sum. PreparedInputGuard::metadata_bytes
already charges its struct, vector and dependency-key heap, but no Arc header.
SourceCallable::retained_lexical_metadata_bytes and selected argument call bytes
charge this immutable proof/guard against existing call limits. The signature
is conservatively charged for its actual copies. Parent graph stays under DB
allocation dedup and query budgets. No plan traversal moves to installed ops.

New source control leaves actual callable as sole external parent/guard owner,
revokes input authority, clears memo wrappers, prunes quiescent roots, checks
parent graph remains pinned/accounted and callable authority is stale, then
drops callable during an active run. Guard owns stamps only and retires then;
parent graph survives until next quiescent registry prune and then releases.
No test/runtime execution was performed; rustfmt and whitespace checks only.

Separate all-eight source pass: (1-3) no evaluator/scalar/method/numeric rule;
(4) no origin gate; (5) no pattern or executable/cold-body demand; (6) O(1)
checked metadata addition, existing bounded allocation traversal only cache
maintenance/preflight and no repeated preparation; (7) no namespace lookup or
unrelated invalidation introduced; (8) same Arc plan/index, no copy or new
whole-file search. Existing legacy call selector and full normal engine join
remain root-owned and held. No runtime/architecture/task closure claimed.

Publisher review is separately recorded in
worktracks/evidence/2026-10-16-ordinary-local-publication-currentness-review.json.
The conservative query_revision refusal remains unchanged. Skip publication for
already exact bodies only with callable exact-row/guard checks; this alone does
not solve first entry of a different missing body after unrelated publication.
Root must settle reusable immutable callplan currentness before production join.
