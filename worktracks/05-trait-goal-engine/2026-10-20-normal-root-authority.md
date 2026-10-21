# M019/M020 / LOCAL-049: private normal-root lineage join

V528 source-frozen design/source review only. This record proposes the next
bounded implementation; it does not activate a root, implement a solver, certify
query closure or record runtime acceptance. Parent owns engine packet/commit
integration and the source-fact worker owns catalog/query producers. No production,
tools, vendor, fixture, Cargo or Git changes belong to this review.

## Concrete next slice and unresolved prerequisite

Do not implement the whole design below as a reusable certificate framework.
The next useful code slice is a private relation-fulfillment consumer taking
original RootRelationParts with the exact owner/RootVariables. It retains all
actual goals, registers them with the existing FulfillmentContext once genuinely
admitted, preserves typed error/pending/overflow, and produces a separate
SettledRootApplication only from successful real fulfillment. The numeric demand
adapter can then accept that settled view without changing the immutable
outstanding_relation_goals count or trusting a caller's completion flag. No raw
goal/environment parameter or external callback is needed for this concrete API.

Actual entry remains blocked by two present source gaps: ReachedCallInputs has
currentness checks but no ordinary production source-root consumer, and the
factory/provider has no exact demanded QueryFact-closure check. `ready()` demands
unrelated domains and cannot replace the missing check. A new opaque receipt that
only repackages the index/world/currentness would be fake evidence. Here “closure
receipt” means the retained result of that actual future check, not a new trust
mechanism or a privately named ready bool. The parent must join the ordinary
packet/attempt guard; the actual metadata preparation must establish the needed
callback values before any lineage can become active. Until those two joins
exist, the concrete relation work can remain retained under typed source-pending
requirements, but cannot issue a settled application or invoke the solver just
to make a component control green. This hold is an implementation prerequisite,
not a static-checking requirement or a substitute solver.

## Invariant and exact source

An actual reached call's original inference owner may enter pinned root
fulfillment only for its original relation goals and admitted source input-
inference goals under the genuine caller ParamEnv. Source-fact publication,
complete indexes, or sharing that Context must not admit a replacement goal,
replacement environment or unrelated inference owner. Canonical descendants
created by pinned evaluation must remain usable despite different owner addresses.
No source, guest or application-supplied callback runs while solving.

The minimal control is two SolverContexts constructed in one prepared Cx: a
source-bound root uses the first; calling the normal solver on the second must
fail its admission precondition even while the first root is active. After the
scope ends, both normal and cached reentry must fail. A stalled obligation and an
immediately successful fast path also need this guard; graph-only checks miss
them. The unchanged runtime witness stays `tests/ui/enum-discriminant/repr128.rs`,
with no fixture/type/function-name admission logic.

Inspected local SHA-256 values:

| File under crates/lrr/src | SHA-256 |
| --- | --- |
| traits/upstream/root_relations.rs | 1b17566242f395b334fc164b0e0d449cb550454c41f50a7e9ee23cd34e6f3eb0 |
| traits/upstream/infer/fulfillment.rs | 03ff24c31c25e9a5e04a675296cc4afbb06b5068db5784b0b6659b9a4895fc9d |
| traits/upstream/infer/delegate.rs | 0cf91947b833fbb373ab70ee541aefaf6264678930d2666403230d5617c51a76 |
| traits/upstream/infer/context.rs | b260cd6f4bff82aa257c9c27dc918cebfc29d2653f3e9232eef33b31d68cc960 |
| traits/upstream/infer/client.rs | c6c83468655f6f7958b9917a495ea64fc632bc9f91da57e481cb2c87c5d9866b |
| traits/upstream/context.rs | e1c3c7b3b522d2e848314a77b12e28fa1adcc0651af4471c0e9066f7b02832d6 |
| traits/upstream/prepared.rs | 8dc69f71a841663d589465ecd5aa57e5db1fc2504c947778f39e8edabec053f6 |
| traits/upstream/source_runtime_demands.rs | 56a69cbb11e4892af1b08d28e8521456d78a5d28880c72f605ca21c7e8aad2d0 |
| engine/run_call_inputs.rs | 7f464692a29479941f5dde1c4d39d67c2957e7cb94a73a64473eba8da75778be |

Pinned upstream references use revision
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`.

## Current paths and the additional early-return gap

`PreparedProvider::publish` freezes real source values/indexes. Its `ready`
returns only PreparedIndexSnapshot after a coarse census; neither proves the
specific reached root's query closure. `with_provider` initializes root_active
false, and there is no true-setting path. Its only positive check is
`SourceFacts::evaluate_root_goal_for_proof_tree_raw` in PreparedProvider.

Pinned `SolverDelegateEvalExt::evaluate_root_goal` in
`solve/eval_ctxt/mod.rs:232` first calls rerunning_stalled_goal_may_make_progress,
then possibly compute_goal_fast_path_cold, then `EvalCtxt::enter_root`, which owns
a new SearchGraph. Normal evaluation does not call the proof raw callback.
Pinned `fast_path.rs:54` first reads disable_trait_solver_fast_paths and can
return WontMakeProgress without calling SolverDelegate::cx. Thus an override of
cx alone still permits an unauthorized stalled Maybe result. `goal_remains_stalled`
has that same early path. Local `infer/client.rs::disable_trait_solver_fast_paths`
is a genuine adapter seam before these returns; its actual configured flag must
be unchanged after the admission check.

Local `SolverContext::compute_goal_fast_path` directly delegates to the inference
client; fulfillment registration can discard a Yes obligation before graph work.
`try_evaluate_obligations_with_inspector` can skip all stalled goals or finish an
empty batch. `Context::with_global_cache` currently borrows the cache without a
provider guard. `SolverContext::build_with_canonical` currently builds a new owner
from Cx alone. Pinned `EvalCtxt::enter_canonical` calls this constructor for actual
search descendants, including normal/erased reruns. Original-owner pointer checks
cannot be imposed on those descendants.

## Smallest private implementation contract

1. Add one private source-root runner/module. Its input is an engine-certified
   actual reached packet plus a genuine demanded-query-closure receipt, retained
   caller/configuration/layout plans and the real source obligations. It must not
   expose `run(Goal, ParamEnv)`, a ready setter, an owner-returning activation API
   or an arbitrary callback/inspector. The engine receipt must have private fields
   and an engine-owned constructor following ReachedCallInputs::require_current;
   SourceRootCallInput's public crate fields alone are not such a receipt. Packet
   attempt, caller/environment, source signature and result expectation checks
   remain the engine owner's work. Missing receipt/closure keeps entry held.
2. Replace the provider's root_active bool with an initially empty scoped state
   holding a privately allocated identity, exact Cx/provider/world/configuration
   and the admitted packet/closure lineage. Use existing ScopedCache/RAII lifetime
   discipline. An Rc identity avoids integer IDs, overflow and invented caps; it
   is transient pointer identity, never a portable source key or semantic cache
   key. Provider Eq/Hash must continue comparing stable slot identity, never the
   changing active value. Reject nested/reentrant activation in this synchronous
   owner rather than silently replacing the state.
3. Add private lineage to InferenceContext: new contexts start unadmitted. The
   runner attaches the current identity only to the exact original owner after
   RootVariables::require_owner, SourceRootApplication::require_owner, snapshot,
   signature/callee/configuration and closure checks. SolverContext constructor
   itself grants nothing. A second context in the same Cx stays unadmitted.
4. Guarded `SolverContext::build_with_canonical` obtains that private identity
   from the currently executing provider scope and attaches a Canonical lineage
   to the actual context returned by the existing pinned constructor. Do not
   reproduce canonicalization/instantiation, compare child pointers to the root,
   or register source root variables on children. Each subsequent check requires
   the same active identity and Cx/provider. Closing the scope invalidates every
   retained descendant; a later session has a fresh identity.
5. Check this lineage in local SolverDelegate::cx, compute_goal_fast_path and
   InferCtxtLike::disable_trait_solver_fast_paths; guard concrete inference fast-
   path entry too if it remains callable directly. Fulfillment construction,
   registration and evaluation check the exact original owner/session/snapshot
   before any empty/stalled/fast return. A required SourceFacts hook guards
   Context::with_global_cache before borrowing the cache; provide no permissive
   default. Existing upstream cache keys, responses, recursion/overflow, cycles,
   candidate transactions and progress loops remain unchanged.
6. Keep raw proof entry closed in the initial normal-root scope. If source-bound
   proof inspection is later needed, its dedicated private operation must use
   the actual source root and the same identity/closure, with no canonical-goal
   replacement argument. A normal scope must not make the current raw callback
   available just because it is active. Pinned canonicalize_goal is private to
   the upstream crate; copying it locally or selecting the first interned input
   as an assumed proof root would be an unjustified alternate admission route.

The no-callback/non-escape condition is essential. An ambient identity cannot by
itself prove an arbitrary Goal came from the packet: blanket solver methods take
raw goals, and build_with_canonical receives Cx/input without its parent delegate.
The small implementation isolates all actual root calls inside the private
runner; external code cannot run during that scope. Do not advertise this as a
general exposed solver capability. If a consumer needs a public owner callback
or arbitrary proof root, this design is insufficient and that API stays held.
Direct upstream raw-provider calls outside the scope fail descendant/cache
guards; no user callback can invoke them from inside the private scope.
This distinction must stay explicit in controls: the guarded provider raw callback
is rejected during a normal session; the public upstream raw-provider function
must be rejected outside any session. A test that deliberately calls the vendor
function directly from inside the private runner would bypass the provider's
route check. The no-callback/private-runner contract excludes that route; it is
not defended by lineage alone. If actual production needs such reentrant access,
first isolate the SolverDelegate implementation behind a private admitted
delegate type; do not claim the ambient lineage solved that broader API problem.

## Actual obligations, outputs and prerequisite boundary

The runner consumes RootRelationParts, preserving its exact Args, ParamEnv,
signature, source spans and all relation child/coercion obligations. It registers
these through existing pinned FulfillmentContext and examines the actual typed
errors/pending/overflow. Only fully fulfilled original relation goals can create
a privately constructed SettledRootApplication; no bool/decrement may clear
outstanding_relation_goals. SourceRuntimeDemands::after_relations currently
requires zero, so it cannot consume an unfulfilled application. Its future
settled overload must require the new typed proof, not reinterpret the old count.

Then genuinely selected numeric-connected source clauses retain original
callee clauses, real EarlyBinder arguments and caller ParamEnv and run through
the same owner/pinned fulfillment. Ground unused bounds remain cold. If a general
case needs mutually constraining goals before relation fulfillment, retain a
typed hold until their real joint-input transport is available; do not invent an
LRR retry scheduler or force every callee clause to bypass the hold. Demand/query
closure producers remain the source-fact worker's separate slice.

Canonical response application already updates original variables and external
constraints; no replay/unifier belongs here. Export only after actual fulfilled
Yes roots, retaining all residual/overflow distinction and original inference
state. Capture actual regions/Verify/outlives/assumptions/opaque state as described
in the root-relations record; empty external constraints on Maybe prove nothing.
Return an owned response, never borrowed IR/session handles. Engine rechecks its
actual attempt/currentness/result contract and once-consumed commit independently.
NeedInput happens before solver scope; resumption revalidates the retained packet
and source query receipt before allocating/entering another nonescaping scope.

## Minimal controls and architecture review

Required controls after source release: actual packet success with real canonical
descendants; foreign owner sharing Cx rejected; wrong snapshot/provider/attempt/
signature or changed result expectation held; inactive normal fast/stalled/cache
entry rejected; provider raw callback rejected during normal scope and direct
upstream raw-provider entry rejected outside a session;
scope unwind clears identity and permits fresh valid session; old descendants
cannot reenter; altered caller environment and arbitrary replacement roots have
no accepted constructor/API. Controls must exercise cache hits as well as misses,
the empty actual batch and actual pending Maybe/overflow distinctions. No fake WF
root is an admission witness. Preserve the unchanged repr128 runtime acceptance
target and separate component/full-fixture/diagnostic outcomes.

Source review against all eight failures: (1,2) ordinary reached-call source and
shared upstream solver, no assertion/generated evaluator; (3) demand incidence
uses existing shared source service, no method/numeric solver; (4) no origin
exclusion; (5) no patterns/cold-body checking; (6) preparation stays retained and
solver callbacks immutable, no reparsing; (7) no namespace census on entry/cache
checks, only exact identity/receipt checks; (8) borrowed owner/context handles,
no body clone/file searches. Costs are O(1) identity checks at adapter boundaries
plus the existing demanded fact/goal work, with one scoped identity allocation.
No measured speed claim follows. Existing ungated ordinary entry and proof-only
bool guard are named replacement routes, not accepted precedents.

The source hash/read terminals exited 0; a few exploratory rg/sed commands used
nonexistent upstream paths before locating eval_ctxt/fast_path and canonical/mod.
No Cargo/tests ran, no gate passed, and no proposed symbol exists yet. This record
does not assert that current factories have the necessary query-closure receipt.
