# T032 explicit scalar type arguments — work in progress

Unqualified source calls now preserve explicit generic syntax in CallPrepare and
CallSeal recipes. Generic argument strings have stable encoding and checked count/
byte admission. The callee's source identity is selected before resolving explicit
types or evaluating arguments. Named scalar types and transparent primitive aliases
resolve in the caller's declaration scope, not the callee module. Supplied type
positions map to declared type-parameter ordinals while skipping lifetime entries;
unrelated unsupplied parameters/bounds remain unforced.

Explicit concrete bindings supply soft argument hints for their declared source
parameter. Caller result hints fill only still-unknown hints. Final contract
preparation processes explicit bindings, result context and actual arguments in
the established order, rejecting conflicts without widening/defaults. Generic
return/body recipes keep their declaration parameter identity; runtime substitution
remains separate. Explicit zero-argument calls use the same preparation path.

The dispatcher now resolves the source catalogue's ordinary import worklist before
call lookup. A retained failing imported-call fixture exposed the missing step.
Lexical local callee lookup extracts the bare path name even with turbofish, so a
local value cannot fall through to a module function. A body-local type shadow is
explicitly deferred until lexical type namespace integration instead of silently
using the declaration/module scope. Explicit context vectors and prepared generic
maps are included in checked retained metadata estimates.

Four controls exercise explicit scalar bindings and lifetime ordinals, return-only
calls without caller context, caller-scope aliases across an imported function,
primitive-name alias shadowing, explicit/argument/result conflicts, too many types,
missing identity before argument effects, local value/type shadow guards, and
actual CLI execution with unused bounds.

T032 remains in progress. Explicit lifetime/const/associated/structural/nominal
arguments, inferred placeholders, forwarding caller rigid parameters, complete
lexical type views, qualified/member/function-value calls, coercions, dictionaries,
full context/action/origin IDs, generated suspension, chunked activation, user
destructors and release integration remain mandatory. This is scalar type-argument
execution, not the full revision-4 generic system. Validate macOS first; actual
Linux acceptance remains deferred.
