# T032 source argument hints — work in progress

prepare_argument_hints consumes a revalidated selected SourceCallable and its
paired immutable source catalogue. It resolves parameter source templates in the
defining scope, preserving rigid generic/region/structural terms. Known primitive
templates supply a concrete descriptor; generic or other known symbolic terms
supply no descriptor. Missing names, unresolved syntax, alias frontiers and input
suspension remain typed outcomes. No actual argument, generic substitution, bound,
layout or guest body is evaluated to discover a hint.

Alias resolution and primitive fallback use the same source-identity rules as
final call contracts. A primitive-looking alias does not receive a descriptor from
its spelling. Final prepare_contracts now uses this shared template resolver
rather than a second implementation. The cumulative owned template budget is
checked before retaining another term; vector reservations and term nesting/size
checks preserve the existing metadata limits.

Three controls exercise mixed concrete/generic hints with unused bounds and no
body/thunk/constant queries, source aliases shadowing primitive names, unresolved
name outcomes, and stale callable rejection before template resolution. Existing
actual-argument contract controls continue to pass.

T032 remains in progress. This is pre-argument metadata preparation; it does not
execute written arguments. The dispatcher must next retain the selected identity
and argument phase across nested calls and pauses, install these hints without
forcing unknown generics, evaluate every written/excess argument in order once,
then prepare contracts and seal/deliver the existing parameter batch. Full source
views, coercions, generated/native/member calls, dictionaries and release
integration remain mandatory. macOS is primary; Linux acceptance remains deferred.
