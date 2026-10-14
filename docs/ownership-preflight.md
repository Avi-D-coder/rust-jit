# T031 local and ownership preflight — work in progress

GuestStore now exposes immutable admission for move, Copy, new storage contracts,
replacement and discard. Move/Copy/replacement execution reuses these checks before
resource reservation and state changes. Admission validates cell epoch/state,
value ownership and descriptor contract; Copy rejects owning objects. It does not
create tickets, move cells, drop values or bind unresolved numeric records.

Frame preflight now covers DeclareLocal, BindLocal, ReadLocal, AssignLocal,
ResultTransfer and Discard before fuel charging. It checks lexical declaration
state and physical-slot bounds, incoming value/contract, Copy admission, output
scope and initialization state, and mutable replacement. A reached contextual
read may preview an unresolved literal's concrete scalar without committing the
binding; actual materialization remains after charge.

Failed replacement leaves the RHS live rather than moving it before checking the
target. Failed transfer/bind does not allocate an output cell. Destination and
source state remain unchanged when descriptor or ownership admission rejects.
Resource exhaustion during later reservations is still a fatal runtime fault;
this checkpoint does not claim atomic multi-resource reservations or general Drop.

Two added controls prove uninitialized read has no failed-op charge/output handle,
and immutable replacement preserves RHS, fuel, handles and bytes. The prior rejected
transfer regression now expects no destination allocation and no started count
for the failed opcode, preserving its existing live-source/fault assertions.
Its first outdated assertion failure is retained; this is a required expectation
change under the specified preflight ordering, not a backend contradiction.
Storage ownership/epoch/literal sharing and source block/control/fuel suites
remain mandatory regressions.

[Exit preflight](exit-preflight.md) now checks branch/return/control boundaries.
Literal admission now has [a preflight path](literal-preflight.md).
T031 remains in progress. Cleanup/aggregate element charges, operation activations, full multiframe/call/effect/JIT/service/supervisor
and generated/source-view integration, final audit and release evidence remain
unfinished. Copy/move/Drop/partial object semantics still require T042/T044; this
admission uses the current scalar/literal/owned-string storage inventory. Actual
Linux acceptance remains deferred.
