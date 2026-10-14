# T030 block phases — work in progress

Entered nested blocks now lower to the same verified LRIR phase mechanism as the
root block. Entry activates the lexical scope and schedules only the first
statement/tail. Each reached statement schedules its next phase; a tail receives
the containing block's hint. Root signature hints do not replace a nested block's
local annotation. Commit creates unit or transfers the actual tail value.
A nested commit explicitly leaves the inner scope and continues to its parent;
root commit returns through the dispatcher.

A block expression's result slot belongs to its enclosing expression scope.
Inner value slots and binding cells retain the entered block's own scope. Thus
inner scope exit retires its locals/temporaries while preserving the result for
the parent to consume. This is scalar/storage cleanup only; user Drop ordering,
borrow/temporary lifetime extension and generated-source cleanup remain the
assigned later integrations.

Two source controls exercise nested results, lexical shadow initialization,
outer binding preservation, i64/f32 containing hints, empty/unit blocks, explicit
scope-exit counters and return that leaves subsequent invalid statements cold.
A name declared solely inside a completed block cannot escape as a local read.
The existing normative eight-operation main and numeric source controls remain
required regressions.

T030 is not complete. The local declaration/assignment component described below
now exercises initialization/reassignment and RHS-before-destination ordering.
General place assignments, loop reincarnations and eventual reference/ownership
integration remain mandatory. Current single-name local routing is provisional
until the ordinary/hygienic/symbolic pipeline integration.
Block attributes and unsafe/async/const forms remain guarded. Labels now have
selected [control transfer semantics](control-transfers.md); generated/source-view
and full cleanup acceptance remains pending.

Uninitialized single-name declarations now emit DeclareLocal and allocate an
Uninit cell with the declared primitive/unknown contract. Initial assignment may
initialize an immutable cell once. Reassignment uses the existing checked replace
operation and requires writable storage, preserving its established descriptor.
Read-before-initialization, repeated immutable writes and incompatible values are
explicit faults. Copies of unresolved numeric records retain shared binding;
assignment does not infer from future uses.

LocalHint reads only an already executed local's storage contract before entering
the RHS. Assignment Entry schedules the RHS directly, and Operand(1) performs the
write and produces unit. A bare local destination is source identity, not an
executed LHS expression. Missing/general destinations fail at the later destination
phase; no unsupported place is silently treated as a local. Overflowing RHS
controls prove panic precedes missing/immutable destination rejection. A query
trace control observes RHS Entry before assignment Operand(1), with no LHS thunk
fetch. A nested RHS assignment supplies an actual stateful order witness.
DeclareLocal/LocalHint/AssignLocal have explicit stable tags, structural admission,
effect classes and separate opcode counters. The original normative main remains
12 operations after H002 statement-scope integration, with zero counts in these
added opcode columns. Statement boundaries add the existing ScopeEnter/ScopeLeave
operations; see [the integration](h002-statement-scopes.md).

Four further controls exercise initialization/reassignment/type/context failures,
RHS-first outcomes, query ordering and malformed IR. Full effectful field/index/
deref destinations, compound assignments, loop reincarnations, refs and destructor
ordering remain mandatory integrations. This checkpoint does not complete T030.
