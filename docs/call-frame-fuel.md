# T032 run fuel across source frames — work in progress

RunFuel now holds a worker-local reference to one run meter. A callee created with
Frame::new_in_run shares the caller's limit, consumed credit, polling count,
cancellation token, deadline and terminal stop. No frame can replenish a shared or
previously charged meter through set_fuel_limit. Cell-based scalar meter updates
require no borrow guards or host locks; only the cancellation/observed-charge
signal crosses threads. This is runtime state, not portable recipe data.

Callee construction requires a running caller and a live caller frame anchor in
the supplied store. It polls cancellation/deadline and requires remaining credit
before allocating the callee anchor. Accepted-call/source/store identity checking
belongs to the dispatcher; this constructor alone does not accept a call packet.
Cleanup charges the same meter before retiring each callee cell.

Three controls cover verified VM recipes charged across two physical frames,
shared cancellation/deadline/terminal stops, and callee-owned String cleanup plus
interrupted/exhausted entry with no new guest handles. A rejected initial test
recipe had zero ordinary operations; the retained failure was repaired by using a
verified SetHint operation. The build's initial redundant Into conversion failure
is retained separately.

T032 remains in progress. The dispatcher still executes a single source frame.
Actual call/return continuations, full context-bearing sealed packets, ordered
argument evaluation and generated/native integration remain mandatory. These
checks establish the shared meter needed by that integration, not executed source
call acceptance. macOS is the primary validation platform; Linux and release
acceptance remain pending.
