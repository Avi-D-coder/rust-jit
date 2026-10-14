# T032 callee parameter cells — work in progress

Frame::initialize_parameters accepts an already sealed argument packet and
prepared concrete descriptor/mutability contracts before body execution. It
checks arity/descriptors before allocation, polls the shared run meter, reserves
parameter vectors and all destination cells, delivers the batch once, and records
cells in declaration order. Parameter slot ownership follows the body's value and
local-binding ranges, with checked arithmetic. Rejected descriptor/arity/quota
admission leaves the sealed batch available to retry or discard. Initialization
requires a fresh frame and cannot run twice.

Function entry with parameters now lowers a bounded CheckParameters guard before
scope entry. A standalone entry without an accepted batch still rejects; it does
not invent argument values. Simple source parameter paths lower ReadParameter by
declaration ordinal after lexical-local lookup. Existing locals therefore shadow
parameters. Current scalar values copy; the supported managed String owner moves.
Receiver ordinals remain reserved when present; complete receiver/member semantics
remain deferred. Parameter assignments and destructuring are not yet implemented.

Root-scope cleanup retires parameters in reverse declaration order after ordinary
locals. Each occupied parameter cell requires fuel before destruction/retirement.
An interrupted cleanup retains the remaining cells and scope. The existing direct
zero-argument dispatcher now uses the same initialization path for its empty batch.

Three controls exercise actual source thunks over a sealed scalar parameter batch,
copy reads and local shadowing, rejected arity/descriptor/quota with no argument
loss or fuel charge, and reverse managed-owner cleanup interrupted between two
parameters. Existing source/VM/CLI controls retain exact opcode expectations, with
two zero columns for the new guard/read operations.

T032 remains in progress. Written/excess argument evaluation is not connected to
the dispatcher yet. The source parameter test explicitly supplies an already
sealed batch; it is not full source-call argument acceptance. Complete patterns,
parameter assignment, SourceView/ParamEnv/substitution/dictionary context, aliases,
coercions, generated/native calls, user destructors and release integration remain
mandatory. Validate macOS first; Linux acceptance remains deferred.
