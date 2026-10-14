# M008 demand tokens and saved phase ownership

Coordinator demand context freezes request/session/unit, source revision and
generation, optional native generation, expansion epoch and compile environment.
Each token additionally binds its pure query key, exact frame/body/phase identity,
input request and checked sequence. Explicit stable encodings contain no pointers.
Issuance validates expansion epoch/environment and capture unit/environment.
Retained key capacities, request metadata, counts and failure messages are bounded.

Duplicate waiters share their token; distinct query/phase waiters share one input
operation. A start decision marks scheduling only: it does not grant native macro
authority, invoke a helper or capture files. The operation records the token that
started it. Reply admission checks the frozen context, token and expected input,
and requires that originating waiter and a running operation. Mismatched,
unstarted, wrong-waiter or duplicate replies cannot alter the pending state.
Successful output and failures share within this context without another start.
Reply admission alone does not release saved state: checked expansion-record
adoption into the matching base source revision is required first.
A new request/epoch cannot adopt an old reply.

Cancellation closes the coordination scope and clears retained records. Subsequent
issuance, starts, reply admission and phase claims fail. Actual helper termination
requires the later session owner. SavedPhase owns its payload without cloning it;
waiting and rejection return that same ownership for retry or cleanup. A successful
phase claim consumes its token exactly once, preventing another continuation from
using a copied token to repeat the phase. Other waiters can reuse the same input.

Nine framework checks cover grouping, reply identity fields, cancellation, linear
phase ownership, shared failures, invalid consumer context and capacity limits,
new-epoch late replies and originating-waiter admission. Payloads are owned fixture
values, not guest VM cells. Expansion-record adoption/query re-entry is integrated separately in
docs/expansion-record-publication.md. Actual helper authority/server/session
execution and concrete VM phase dispatch remain later task integrations.
The coordinator currently supplies operation grouping; the session owner must
establish the required serial procedural-macro scheduling. M008 component acceptance is complete; release remains incomplete. No actual macro execution or helper containment claim is made.
