# Pinned associated-item eligibility client (bounded M020 continuation)

Baseline5e3e92d08a2c41e174d146d0f7dffda3c2dd48c5. Solo /root, macOS first.
Previous goal turn progressed: shared ordinary discriminant initialization was
implemented, validated and pushed. Full unchanged repr128.rs still fails its
generic operator goal; this must be solved through the actual upstream bridge,
not a discriminant/assertion-specific inference or fallback.

Required invariant: associated projection eligibility is rustc's full pinned
SolverDelegate::fetch_eligible_assoc_item policy, consuming actual prepared
specialization leaf metadata. Current infer/delegate.rs instead requires a
caller-supplied eligible_assoc_item function pointer; the actual compiler
algorithm is absent and root readiness remains closed. Remove that semantic
callback by registering a mechanically audited complete port from pinned
compiler/rustc_trait_selection/src/solve/delegate.rs:439. Do not implement a
replacement eligibility/normalization/unification algorithm.

Shared entries: actual source definition/generic receipts and completed trait/
impl membership -> PreparedFacts publication -> SourceFacts immutable reader
-> SolverContext::fetch_eligible_assoc_item -> upstream projection evaluation.
Keep rustc's final/default, typing-mode, deep resolution, further-specializable,
erasure and actual source error branches. Prepared leaf outcomes must retain
real item/defining/finalizing identities from source preparation. Missing leaf
facts are incomplete preparation, never NotFound/NoSolution/default eligibility.
Specialization graph/leaf production is still a named integration prerequisite;
do not forge a leaf or full root-ready provider for a control.

Add immutable leaf results to the existing fact publication/readiness contract,
with actual world/definition/container/ancestry membership checks and required
trait-associated member coverage. Preserve the graph's existing pinned source
producer obligation; do not extend the old custom solver. Compiler-only static
impl-constrained checks are not an interpreter prerequisite under E006/E014.
This client consumes already prepared graph facts and executes no bodies.

Cold: source graph/leaf metadata is prepared before shared solver entry and
keyed by actual relevant impl/member identities and source dependencies. Hot:
immutable relevant map lookup plus pinned inference resolution/flag inspection.
No parser, macro execution, IO, VM effects, full-world namespace reconstruction,
new limit/accounting, unsafe or BodyIndex copy. Ordinary and generated source
join the same existing definition/parent receipts. No context/macro/fixture
exception or eager executable preparation is authorized.

Evidence: exact original/source hashes and mechanical generator audit; async
macOS1.99 library/CLI builds, existing source/type/inference controls and actual
unchanged repr128 runtime status. Direct policy/leaf controls must use real
source declarations; absent source/root execution stays unexecuted. Review all
eight recurring architectural modes against an exact source fingerprint.
This is a staged client integration component, not completed M020, a trait
solver/root invocation, a new stock pass or permission to start guest JIT.
