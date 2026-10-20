# M019/M020 — source conditional impl clauses into upstream IR

LOCAL-049, solo /root, baseline ce42e90240064c3d68717eb86faf37068d80ca74.
Primary unchanged witness: methods/method-two-trait-defer-resolution-2.rs,
whose T:MyCopy conditional impl remains an applicable runtime blocker.

Invariant: selecting an impl requires its actual predicates, never just a
matching runtime/self Type or method spelling. Existing ImplPredicates/Q59
retains actual source generic/where recipes, but ImplSourceTypes/Q68 transports
only head Types and genuine generic layouts. No upstream clause producer joins
those routes. Extend the common source-Type encoder; do not extend legacy
engine/run_trait_calls or traits/unify with new conditional matching rules.

Add a demand-specific pure ImplSourcePredicates query keyed by the existing
ImplHeaderSyntaxKey. Retain Q59 and its Q58 source/header receipts. Collect
required trait/type declarations through the existing SourceBindings closure,
including the canonical impl owner for real generic identities. Source facts
come only from actual captured ordinary/generated source publication. Pending
head/name/binder/projection/Sized facts remain typed pending, never omitted or
converted to an empty environment, NoSolution or positive applicability.

Encode positive trait/Sized, Type-outlives and region-outlives recipes as actual
rustc_type_ir ClauseKind values using the existing shared Type/Arg/Region
encoder and genuine TraitRef constructor. Relaxations remain source metadata,
not positive or negative goals. Preserve actual source spans and declaration
order. Do not synthesize DefinitionIds, source bindings, opaque captures,
ParamEnv, ReadyProvider, solver answers or default client functions.

Original/user-macro/wrapper items join Q59 through admitted source inputs and
use one encoder; no new parser, evaluator, runtime opcode or verifier route.
Ordinary expression lowering, transactional patterns, call continuations and
VM cleanup stay with their existing owners. The new producer supplies missing
solver facts; it is not yet their solver/member/runtime integration.

Cold cost: one memoized bound/required-declaration walk plus source queries and
context-owned typed encoding. Demand for only impl identity/head Types must
not prepare predicates. Existing captured catalog/source/epoch dependencies
and immutable Arcs govern the plan. No installed source planning, namespace
census, body clone, resource accounting, LRR limits or static enforcement.
Use new stable query/value tag71; existing tags remain unchanged.

Evidence required: unchanged conditional fixture across all editions and
original/emit/wrapper contexts, genuine clause TraitRefs/generic substitution,
shared Arc reuse, actual source spans, pending bound preservation, source/epoch
refusal and no body/signature/thunk/pattern preparation. Supplement with actual
upstream region/relaxed-bound source. Runtime probes remain separate; no new
stock pass, M019/M020 closure or JIT permission from source/IR checks alone.
Eight-mode review at the final fingerprint must name remaining legacy routes
and unexecuted compiler-client/root/member/dictionary/currentness matrices.

## V384 prerequisite exposed by unchanged lifetime source

The first actual clause control passes conditional and where-bound sources in
three origins, then fails static-outlives-a-where-clause.rs: the first impl is
HeadPending, the second retains its real HigherRanked pending bound. Inspection
finds symbolic_type_pending rejects every Lifetime AST, three shared actual-Type
argument loops discard LifetimeArg, and complete_trait_arguments rejects every
non-Type declaration parameter. Do not change expected counts or bypass Q58.

Repair explicit lifetime arguments at the common WrittenTypeDeclaration leaf:
reuse the existing declaration-bound lifetime mapper there and remove its old
duplicate from source_predicates. Preserve actual declared ParamKey (including
the existing parameter rebase), static identity and local underscore locations.
All three Type/trait argument loops retain GenericArg::Region. Share prior
default substitution through typed GenericArg bindings so a trait Type default
that mentions an earlier lifetime receives the real supplied region as well as
earlier Type bindings. Preserve unsupported const metadata, missing lifetime
arguments, unresolved lifetime names, reference-Type erasure and higher-ranked
binders as explicit pending. The symbolic guard may admit only LifetimeArg
occurrences whose common leaf now retains regions; RefType and other lifetime
binders stay held. This is source metadata translation, not region checking or
an alternate evaluator. No new runtime lifetime refusal is permitted.

Inspect all affected actual-Type, bound and default callers, retaining source
and owning edition. The same unchanged lifetime/relaxed controls in all origins
must pass; existing pending reference/projection controls must still refuse
unsupported exact representations. Full reference/binder/elision/default-
region metadata and actual runtime/member integration remain separate holds.

## Alias source prerequisite found during review

Do not extend deferred_alias_type's existing synthetic FunctionSignature ->
resolve_written_type -> parsed `type LrrWritten = ...` path to region arguments.
That path also substitutes Type-only arguments. Remove that affected route:
read the actual TypeAlias recipe and DeclarationGenerics/ParseFile receipt,
check owning header/context/generics, resolve its real target AST through
WrittenTypeDeclaration and the common actual-Type walk, then apply the existing
mixed-namespace structural Substitution once. No new evaluator or solver.
Explicit RefType lifetimes must use the same declaration region leaf rather
than erase them to static; exact direct-reference admission remains held.

Supplied lifetime/Type/Const arguments retain their real namespaces. Omitted
alias lifetimes remain non-data local metadata at their actual declared slot,
not guessed static lifetimes or free alias parameters that could certify a
proof. The exact IR encoder continues to hold Local regions. Existing local
alias-owner publication gaps and other resolve_written_type consumers remain
named debt; this component cannot close their architecture matrix. Add a
supplemental actual-alias source receipt/control from unchanged upstream input
and retain the old failure/source evidence. No runtime/alias feature completion
is inferred from the retained AST or a source Type substitution.

Extend the existing unchanged source-IR matrix with issue-22019.rs's two
GraphWalk impls. The original regression distinguishes scopes containing 'g
and 'h; test actual region arguments and alias-reference pointees through the
real compiler TraitRef constructor, not just host-supplied static arguments.
Source inspection finds Encoder::region currently constructs EarlyRegion with
name None while the genuine generics/identity-argument producer retains its
source name. Share one SourceGenericLayouts early-region leaf between those
clients instead of allowing two representations of the same declared slot.
Preserve actual owner/key/index and named source metadata, without generic
Type inference, a new solver, invented lifetimes or full-fact readiness.
