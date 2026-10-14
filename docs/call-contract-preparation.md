# T032 call contract preparation — work in progress

Contract preparation consumes checked SourceCallable identity and fully evaluated
argument type views. A view contains a symbolic source term and a separate runtime
descriptor. Cumulative source-view bytes, term nesting and argument counts are
bounded. Zero descriptors, unresolved local Infer in source views and primitive
source/descriptor disagreement reject before publication. This stage evaluates or
moves no guest argument.

Parameter templates use their existing rigid structural facts or checked defining-
scope type lookup and alias demand. Only concrete scalar/definition-parameter
contracts currently become ready; other alias/name/type needs remain typed
outcomes. Generic binding processes explicit type arguments, known generic-result
hints, then actual arguments. Local symbolic unification and concrete descriptor
maps are separate. Reached mismatches do not widen or default. Unused parameters/
bounds stay unforced. Prepared parameter/result source views remain the declared
callee terms even when their runtime substitution is concrete.

A reached already-concrete output check reports LRR-E-CALL-RETURN-TYPE on descriptor
mismatch. Unresolved return source syntax is not forced at call entry. This helper
currently requires deferred return resolution or return-only binding to be supplied
by the later return machine; it cannot substitute a unit/default result.

Three controls exercise separate generic source/runtime maps with unused bounds,
explicit/hint/argument conflicts, primitive view integrity and Infer rejection,
primitive alias parameters, unresolved type outcomes and reached return checks.
They use actual demand-selected source signatures and no body/thunk/constant
queries. They are contract metadata tests, not executed source-call acceptance.

T032 remains in progress. Nominal/layout/structural/owner/member/native coercions,
complete aliases/binders/consts, SourceView/ParamEnv/RuntimeSubst IDs and dictionary
ownership, full sealed CallPacket/continuation, argument/return-only inference,
actual guest evaluation/frames/return/cleanup/fuel/pause and generated integration
remain mandatory. macOS is primary; release/Linux remain unverified.
