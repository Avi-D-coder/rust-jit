# T032 ordered source arguments — work in progress

Original unqualified source calls now evaluate written primitive-valued arguments.
CallPrepare selects the exact source identity before any argument, obtains its
source hints, and installs soft hints on argument slots. Unknown generic hints
remain absent. Each returned argument advances an explicit ordinal; the final
phase checks arity/contracts, prepares only the selected body, seals the argument
cells and publishes the Call exit. Excess arguments run before arity failure;
arity is checked before demanding a descriptor from an excess untyped literal.

Selections are indexed by call site in the active dispatcher frame. Nested calls
save the caller's pending selections with its continuation and restore them on
return. Query NeedInput before a call phase leaves its completed argument cells
and PC intact; generated/macro revalidation integration remains pending. An early
return from an argument abandons and retires its unfinished metadata context.
Selected context and retained call/frame metadata use checked budget admission.

Accepted argument batches initialize callee parameter cells in declaration order.
Parameter reads, lexical shadowing and reverse cleanup use the previously verified
storage path. Explicit primitive argument mismatch is diagnosed at the final
contract boundary after all arguments, without widening or defaulting. Pending
context metadata is not a full revision-4 CallPacket or SourceView environment.

Five controls exercise unsuffixed typed hints, aliases and nested argument calls;
left-to-right assignment effects with exact call/argument counters; excess-argument
panic before arity failure, excess untyped arity, missing identity before effects,
and mismatch after arguments; early return inside an argument across repeated
calls; and an actual CLI subprocess with nonzero parameter frames. The initial
counter-shape compile failure and Clippy tuple-complexity failure are retained;
exact counters gained two zero columns and call-start facts gained a named type.

T032 remains in progress. The current argument source view path supports builtin
primitive descriptors; full rigid/nominal/reference/dyn/native source views and
result/explicit generic hint binding remain mandatory. Complete source-aware body
hints, receiver/member/function-value/qualified calls, parameter patterns and
assignment, coercions, dictionaries, action/origin/context IDs, generated-source
suspension, user destructors, chunked operation activation and release integration
remain pending. No arbitrary symbols or selected live-unit compilation are used.
Validate macOS first; actual Linux acceptance remains deferred.
