# T032 direct source-call selection — work in progress

CallResolver pairs the existing source namespace graph with checked immutable
source views. Module/qualified lookup first selects an exact value-namespace
identity under the caller's declaration scope. Missing direct identity faults
before any argument machinery runs; ambiguity, pending producers and owner
contexts remain explicit. Non-function definitions request callee-value handling
rather than being guessed into a source function or rejected as non-callable.
Executed local ribs/callee expressions must route before this module-only API.

Only the chosen function's FunctionSignature is demanded. SourceCallable retains
its exact DefKey, owned signature, optional body handle, source locator, declaration
context and database source revision. Revalidation rejects changed header/source/
body/context/revision before later publication. Inherent/trait/local owners require
their source-aware member/context path rather than module function classification.
Bodyless declarations retain their metadata; they grant no native symbol access.

Three controls exercise an unknown unused bound and an invalid cold signature
without body/thunk/constant demands, import/qualified identity agreement, deferred
function-valued constants, missing/ambiguous/producer outcomes before signature
fetch, and stale selected-body rejection. No argument or source body executes in
these controls. Query capture is not a proof of ignored-bound behavior in an
actual guest call.

T032 remains in progress. Full callee-expression/local/member/native resolution,
source/concrete substitutions and coercions, actual ordered/excess arguments,
complete sealed CallPacket context/continuation, callee frame publication, return
contracts, pause/resume/fuel/cleanup and actual guest-call acceptance remain
required. macOS is primary; no release or Linux acceptance is claimed.
