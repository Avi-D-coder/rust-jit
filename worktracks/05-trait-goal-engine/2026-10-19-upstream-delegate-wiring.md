# Concrete next-solver delegate wiring (M020, LOCAL-049)

Baseline552082e78c367eab379e154efe415b7fd039f5ef; solo implementing owner.
This prerequisite is incomplete until real source clients and ReadyProvider
publication exist. Do not count compiling this adapter as root-goal, runtime,
M020, interpreter or JIT acceptance.

Invariant/witness: the shared evaluate_root_goal must run with the existing
LRR source interner/inference owner and upstream canonical/transaction/region
algorithms. The unchanged traits/multidispatch1.rs still fails with ambiguous
get selection (V356). Its ordinary member path remains named migration debt;
no first-candidate or runtime-tag workaround is permitted. The generic
evaluate_prepared API currently cannot be called with a concrete LRR delegate.

Entry: PreparedProvider::ready -> evaluate_prepared -> upstream root, with the
same context/provider equality guard. Original/generated declarations already
join through actual retained source/generic metadata. Use one owning safe
SolverContext over InferenceContext, no copied inference state or unsafe
transparent reference cast. Canonical child contexts use the existing actual
compiler build_with_canonical/instantiate implementation. Wire fast paths,
fresh variables, real leak check and region-response collection to existing
pinned implementations, not rust-analyzer's placeholder returns.

Remaining required client contracts are const evaluation with its normalization
callback, demanded WF goals, hidden-opaque bounds, eligible associated items
and transmutability. Make these explicit required typed client functions in
the existing un-defaultable Clients record. Do not install fake functions,
empty/no-op semantic answers or fabricate source facts to exercise root entry.
These functions still require actual pinned compiler algorithm integration;
moving their contract is not their implementation or acceptance. Const
normalization's generic E is transported through a typed callback adapter,
retaining its actual error; it must never become NoSolution or a fake token.
Overflow warning emission records diagnostic evidence under E006, not a new
lint/static rejection. Exact compiler diagnostic compatibility stays separate.

Cold costs: one owning wrapper, original canonical builder and local inference
operations. No source parsing/planning, callback IO, guest effects, macro
execution, full-world scan or per-operation dependency reconstruction. The
required callback functions consume immutable admitted source facts; only the
existing coordinator prepares missing inputs. No performance claim.

Delete the generic public root dispatch alternative in favor of this concrete
delegate. Existing legacy Unifier/run_trait_calls routes stay held until real
source goal/answer/member/dictionary consumers migrate. Keep source readiness
closed and the actual failed stock witness recorded until then.

Evidence: compile the complete upstream SolverDelegate trait implementation;
drive actual source Type/TraitRef controls through its real methods, including
canonical construction, non-root fresh namespaces, rollback, real leak failure,
and deduplicated region constraints. Source data readiness must remain refused
where incomplete. No synthetic metadata/client provider and no new guest test
suite. Record exact sources/terminals and all8 recurring architecture modes;
metadata-dependent callbacks/root and origin/runtime matrices stay unexecuted.

Proof-tree continuation: replace the old arbitrary Clients.proof_tree function
pointer with the actual shared evaluate_root_goal_for_proof_tree_raw_provider
specialized to this concrete SolverContext. Retain constant-time actual
context/index-publication preconditions in the callback; do not re-run ready's
classification or any source queries inside it. Required compiler client
algorithms/source producer are still absent, so this concrete root provider
is compiled but must remain unexecuted. No fabricated facts/ready token to
obtain a trivial proof-tree pass. Default canonicalizer-state allocation and
release use the original upstream trait defaults; no custom canonicalizer/cache.
