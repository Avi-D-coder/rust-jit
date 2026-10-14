# T032 call action publication — work in progress

The scalar call boundary now publishes an opaque, non-Clone call action with a
checked monotonically increasing ID shared by all frames in one run. Publication
requires no outstanding action. Admission checks ID exhaustion before CallSeal
moves arguments, and the verified Call exit publishes the action after its fuel
charge. ArgumentPacket remains the store's canonical once-consumed ownership
record; action publication and argument delivery are separate checks.

The published action owns the caller PC, call site, return PC, destination slot,
and verified source origin. Dispatcher consumption checks its run identity,
current caller and continuation, pending ID and consumption order before clearing
the outstanding action. Run identity uses worker-local Rc identity, rather than a
serialized pointer or a reusable numeric run index. Caller continuations retain
the consumed ID and origin; a reached return checks them before resumption.

ActionSnapshot exposes actual publication/consumption counts and the pending ID.
The nested source execution regression observes three calls published and consumed
once, and verifies that polling a terminal run does not change the counts. Unit
controls exercise a forged replay model, duplicate publication, wrong-run and
continuation rejection, source identity mismatch and checked ID exhaustion.
Production action handles cannot clone or be fabricated through these APIs.

This component covers source Call actions with original verified origins. Complete
call-context ID tables, dictionaries, generated origins, native/effect/return/panic/
fault/yield action packets and service/JIT integration remain pending. T032 and
release acceptance remain in progress; Linux acceptance remains pending.
