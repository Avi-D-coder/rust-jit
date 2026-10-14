# T032 call argument ownership — work in progress

ArgumentPacket stores its canonical state in an epoch-checked guest arena record,
not in a mutable host-local consumed flag. Sealing accepts already-evaluated cells
and already-prepared runtime descriptor contracts. It validates total arity,
distinct source locations, live ownership and concrete descriptor agreement before
moving any source. Unresolved literals stay unbound and fail type context rather
than defaulting. The packet wrapper is not Clone or protocol-serializable.

Handle allowances, ticket arena capacity, batch arena capacity and owned vector
bytes are reserved before guest ownership mutation. Ordered tickets and descriptor
vectors are charged to the run budget. Sealing then moves each cell once and
advances its location epoch. Delivery admits every destination and ticket before
consuming the canonical batch, then initializes destinations in order. Rejection
preserves the packet and every destination. Delivery/discard consume once; stale
aliases or run-reset packets fail. Discard releases current owned string payloads
and packet vector charges without calling guest code from a host destructor.

Six registered controls exercise exact arity/type/duplicate failures, handle quota
before movement, owned-string/scalar transfer and ordered delivery, destination
rollback, once-only discard/reset and unresolved numeric records. Existing storage
ownership/epoch/literal regressions remain mandatory. The reservation guarantee
is for ownership: allocator capacity admitted before a later resource fault can
remain charged; it is not a promise of fully atomic physical arena allocation.

This is the storage part of call sealing. Complete CallPacket still needs exact
ResolvedCallable/signature identity, ParamEnv/SourceView, concrete RuntimeSubst,
unforced dictionaries, result hint, origin and caller continuation. This helper
neither resolves a source function nor validates symbolic source contracts itself.
Actual ordered argument evaluation, excess-argument effects, saved pause/resume,
shared fuel/chunked activations, callee frame publication, return checks and
managed user Drop/call cleanup remain required integrations. It is not currently
an executed guest-call path. T032 and release remain incomplete; macOS is primary
and Linux acceptance is deferred.
