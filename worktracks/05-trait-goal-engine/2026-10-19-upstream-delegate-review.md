# Concrete SolverDelegate: separate source review

Baseline552082e78c367eab379e154efe415b7fd039f5ef; candidate V373 source
563567ece87f592ce9d69197ee67015859b60764ceed920bca752f31abe462bc.
M020/LOCAL-049/WT05-02 remains held. This is a complete Rust trait-interface
implementation, not complete source client semantics or actual root execution.

Inspected entry points: infer/delegate.rs::SolverContext::{from_inference_context,
Deref,SolverDelegate implementation}, existing InferenceContext::{canonical,
fast_path,region_response} methods and RegionConstraintCollector::leak_check;
prepared.rs::{Clients,PreparedProvider::solver_clients,record_solver_overflow,
evaluate_root_goal_for_proof_tree_raw,evaluate_prepared}; upstream/mod.rs::evaluate;
tools/upstream_solver_delegate.py and sync_upstream_inference.py extension order.

Copied compiler delegate methods fresh_var_for_kind, leak_check,
instantiate_canonical and instantiate_canonical_var from exact retained
solve/delegate.rs hash ad43375618c2e3a7c5ff6f69abfc8a4af33ae1d003cb003d350aea0bb0d17095.
The builder/fast-path/response methods forward to already retained complete
compiler implementations. New wrapper owns one existing context; no unsafe
transparent-reference cast or cloned mutable inference store. The leak call
uses the actual collector with the same outer/current universe and no snapshot,
matching the compiler's enabled-check branch. LRR currently exposes no
no-leak-check/skip-leak-check profile; nondefault opt-out transport remains
unimplemented, not inferred from default-profile controls. No RA Ok(()) leak
stub, empty WF/region answer or unconditional transmutability is imported.

Five mandatory SolverClients functions are interface contracts only: const
evaluation, WF goals, hidden-type bounds, eligible associated items and
transmutability. There are no implementations/installations or Default.
No test creates a fake client or publishes fake source facts. They remain real
compiler/source integration obligations, not accepted semantic callbacks.
Const normalization transports the actual generic E using an Option error
slot and FnOnce callback; the unit-error channel means only normalization
failure, never NoSolution/missing-input/constant-evaluation failure. That path
is compiled but unexecuted. Future real clients must honor its exact contract.

Clients.proof_tree is deleted. The provider now calls the actual shared
evaluate_root_goal_for_proof_tree_raw_provider specialized to SolverContext,
with constant-time retained context/index-publication preconditions. It neither
walks catalogs nor calls ready() from inside evaluation. Complete facts and
required client algorithms still prevent real entry, so no proof-tree/root
result is claimed. The public evaluate_prepared path is now concrete and keeps
its exact context/provider comparison. Private generic evaluate is only the
shared upstream entry helper, not an alternate interpreter.

Overflow warning recording retains Span/message evidence without lint rejection
under E006. Compiler proof-chain/lint diagnostic fidelity remains outstanding.
Canonicalizer allocation/release use the actual upstream trait defaults; no
independent canonicalizer or new cache. No speed/scaling claim.

1. No assertion/expression compiler or evaluator: changes stay in one typed
   source/inference bridge, no syntax/VM/assertion dispatch mutation.
2. No generated literal/break route: all source origin controls enter the same
   concrete type/delegate methods. No macro identity or fixture dispatch.
3. No context-specific selection/inference: shared compiler operations, source
   Types and genuine TraitRefs are used; five missing clients are not defaulted
   into compatibility results. Runtime member selection remains old/held.
4. No generated-source exclusion or readiness bypass: no fake IDs/source facts,
   reduced catalogs or synthetic ready token. Same immutable provider authority.
5. No duplicated patterns/eager executable checks: no body/PatternPlan changes;
   WF callback contract alone does not perform whole-body static checking.
6. No parsing/planning/IO/macro/guest work in delegate operations. Canonical
   child construction uses upstream typed metadata operations. Callbacks must
   consume prepared immutable facts; their real implementations remain held.
7. No full-world scan on dispatch: root provider uses retained O(1) preconditions;
   the original cold readiness classification remains outside the callback.
   No new final-answer cache or unrelated namespace invalidation.
8. No BodyIndex clone/search: wrapper ownership moves an existing inference
   context; source/IR handles are retained. No frame/body representation change.

V370 builds the interface; no controls executed. V371 fails two compiler
diagnostics for one host UFCS call treating SolverContext as InferCtxtLike.
Correct the borrow to &*owner; do not implement a second inference client on
the wrapper. V372 builds; no controls executed. V373 adds the actual proof-tree
provider dispatch, preserving all source readiness gates. Final terminals and
source controls are recorded separately in the evidence record.

Controls use existing unchanged-fixture Type/TraitRef contexts. Canonical
builder/answer instantiation now crosses the actual SolverDelegate trait;
fast-path controls and genuine source unresolved-Self traits use the wrapper.
Supplemental namespace/leak/dedup controls allocate actual shared inference
state, observe a real NoSolution leak, roll back and observe success. These
are metadata operations, never actual trait/root answers or original mains.
Metadata-dependent positive trait/dyn/ADT/opaque/associated/const/WF clients,
full source fact/ParamEnv producers, root/dictionary/VM integration, cache
currentness/suspension/origin/cleanup and obsolete runtime consumers remain
incomplete. Latest failed multidispatch1 runtime witness is V356; untouched
runtime dispatch is not rerun or claimed repaired by this interface change.

V373 actual terminal: both macOS1.99.0 builds and2/2selected source-context
bodies pass, source/HEAD/test/CLI guards all true. The first covers140Types/
136genuineTraitRefs/48original-user-macro-wrapper edition worlds; the second
retains20worlds/48generic rows/36const Types/24early regions. Expected host
source-identity/canonical-constructor refusals retain their original assertions;
no guest fault is accepted as success. The full trait interface compiles but
five required client algorithms and proof-tree/root entry are unexecuted.
Vendor audit:8crates/183files/2742155bytes,0retained-original Rust changes,
5existing stable projections. Generator audit reproduces exact output.
No new stock/runtime fixture gain, refreshed suite total or gate closure.
