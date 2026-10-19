# Hidden-type bounds client: separate source review

Baseline64a307c7073980b487e36220eafd7585751c31f7. Candidate V376 source
f39263164c65329249298db2fef39c04a195d511559e30a169f2a9f4cd7d1b85.
M020/WT05-02/LOCAL-049 remains held: client body/reader integration does not
establish actual opaque source producers, root/answer/member/VM execution.

Inspected tools/upstream_hidden_type_bounds.py and sync_upstream_inference.py;
infer/hidden_type_bounds.rs::{BottomUpFolder,add_item_bounds_for_hidden_type};
infer/delegate.rs::SolverDelegate::add_item_bounds_for_hidden_type;
facts.rs::SourceFacts::{explicit_item_bounds,is_conditionally_const};
context.rs's matching immutable reader joins; prepared.rs::{PredicateFacts,
DefinitionFacts,SourceFacts implementation}; infer/tests.rs's actual source
Type/Clause folder controls. The old hidden_type_bounds function pointer is
removed from SolverClients and replaced by the copied compiler algorithm.

Originals remain unchanged in Git under rustc's licenses:
infer/opaque_types/mod.rs SHA256
5fa09869ec7c26a4065504a401729e05f588efdffe4bb74998faa14d9cd2a1a6;
ty/fold.rs SHA256
e07cb2d26e76490d9a5cb6af659d993f5a95e4ccc99088ed90f233e2842f50b4.
The complete hidden-bounds method and full BottomUpFolder struct/impl are
selected by the generator; no matching arm, branch condition or goal order
is redesigned. The initial WF goal, same-opaque definition/args substitution,
projection branch conditional on the old solver, explicit bounds and const
HostEffect goals remain. The actual inference owner uses next_trait_solver=true.

Mechanical transports: Cx/lifetime/model aliases; folder field tcx->interner;
actual copied iterable responses use shared iter_instantiated instead of the
compiler List references' iter_instantiated_copied. The source service for
implied const predicates omits spans that the original immediately discards;
only that loop's unused tuple binding changes. Explicit item bounds retain
their real Span and their distinct collection; conditionally_const is a new
required source fact, not alias_has_const_conditions or a default false.
Reader APIs do not implement those still-missing producers. No empty metadata
or fabricated opaque ID/ready token is installed to exercise this method.

Four required semantic clients remain unimplemented: const evaluation, WF,
eligible associated item and transmutability. The newly copied method itself
still needs positive source/opaque/explicit/conditional-const execution evidence.
No source encoding OpaqueMetadata refusal is bypassed. Current source-aware
folder controls rewrite genuine source TraitRef clauses and source Type handles;
they do not call the hidden-bounds method with fake declaration identities.

1. No assertion/expression evaluator: only immutable typed metadata/solver
   client paths change; ordinary syntax/AST/thunk/VM routes are untouched.
2. No generated literal/break execution: identical source Type/Clause folding
   across original/user-macro/wrapper contexts, no macro-specific dispatch.
3. No context-specific inference/member matching: exact compiler alias ID/args
   comparison replaces the callback contract. No method-name/type-tag heuristic.
   Actual method/dictionary integration remains held, not silently substituted.
4. No new source-only rejection: owning source/world/generic identities remain
   guarded; no fake opaque declarations, wildcard facts or positive readiness.
5. No pattern duplication or eager body checking: initial WF is a demanded
   type-level goal, not whole-body/static borrow/privacy/lint checking. No
   unentered branches/functions or pattern engine changes.
6. No runtime parsing/replanning/IO: compiler folder visits retained typed IR;
   bound instantiation uses actual shared EarlyBinder operations. Reader calls
   consume admitted facts, no coordinator/query/macro/guest side effects.
7. No full-world scan/new global invalidation: no new dispatch cache/currentness
   census; provider world/context and existing immutable readiness still govern.
8. No BodyIndex clone/whole-file search: typed IR folding returns interned
   handles; no mutable frame/body/source-index representation changes.

Cold cost remains compiler traversal over visited typed IR occurrences and
retained interned reconstruction. No new work/byte/depth/fuel accounting or
limit, local unsafe, backend or measured speed claim. Existing broader source
fact/currentness and full origin/epoch/suspension/cleanup matrices stay open.

V374 preserves three API compile failures: ParamEnv naming and copied iterator
reference bounds on both loops. V375 corrects those mechanical projections and
builds, with no control/main execution. V376 adds supplemental actual-source
folder controls; final terminals and any original nesting runtime refusal are
reported in the evidence record. A successful folder or neighboring runtime
fixture cannot prove opaque bounds/source producer/client/root acceptance.

V376 terminal: both builds and2/2source-context controls pass, all source/
HEAD/test/CLI guards true.140Types/136TraitRefs/48origin-edition worlds;
20generic worlds/48rows/36const Types/24early regions preserved. The new folder
checks postorder reconstructed children, nonrecursive region callback, const
and Type traversal, and actual source TraitRef Clause reconstruction. These
are supplemental controls, not positive opaque method calls or root solving.

Unchanged nesting.rs fails all4editions: CLI exit2, LRR-E-TYPE-CONTEXT,
"generic parameter `U` has no reached binding for its path call". Actual
capture22VMdispatches/2source call boundaries, no cleanup or closed capture,
guest streams unobserved. Source stack locates facade core/convert.rs and
original nesting.rs. Source inspection at engine/run_trait_calls.rs::generic_binding
shows the ordinary generic frame lookup, not a macro-only path. The missing
source-bound generic target/member/solver transport is not repaired by these
new reader/client bodies; no Into/From-name fallback or synthetic context is
introduced. Original failure stays an applicable runtime blocker. No measured
before/after stock gain or full matrix/feature/milestone/JIT acceptance.
Vendor audit unchanged8crates/183files/2742155bytes/0original edits/5stable
projections; deterministic generator reproduces the candidate.
