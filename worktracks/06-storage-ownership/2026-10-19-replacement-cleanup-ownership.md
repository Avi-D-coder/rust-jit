# T041/T044/M018 — ownership receipt for detached replacement cleanup

Baseline8fefce6624498038f94720a4766b318ad74c7401. Solo LOCAL049 with existing
same-root reservations. Unchanged witness union/union-drop-assign.rs currently
passes union reassignment/Drop but fails `*u.a = S` after211dispatches: cleanup
relocation compares the shared-view metadata ObjectId with the newly reconstructed
owning snapshot. The old value's Type is correct; weakening equality to Type
would allow dropping an unrelated object and is forbidden.

Invariant: the one prepared cleanup continuation must follow exactly the old
owning value produced by the actual replacement transaction. Ordinary storage
moves its existing ObjectId; shared memory reconstructs the old owned image.
Both need a store-issued linear receipt tying the exact old source cell/epoch
and admitted ValueRepr to the actual detached cleanup cell/epoch/ValueRepr.
No caller may manufacture that receipt. It is consumed once by relocation;
existing start/Drop/field cleanup/suspension owners remain the same.

Shared entry points: Op::AssignLocal/AssignPlace -> Frame scope replacement
preflight and CleanupMachine::prepare_root -> GuestStore::replace/replace_with
and initialize_replacement_cleanup_root -> PreparedCleanup relocation/start ->
ordinary source Drop call and cleanup machine. Original/generated source joins
before these ordinary verified operations. Extend storage replacement/root
installation to return the private receipt; both assignment paths consume it.
Remove raw identity-only relocation, not its ownership/currentness guarantees.
All other replace users retain the same common physical implementation.

Prepare shared-memory incoming bytes/contract/ownership and destination range
before reconstructing the old snapshot, then commit through the same image
writer. This removes an error path leaving a detached old ticket after a refused
incoming image. Reuse image encoder and normal source consumption, not another
value evaluator. Direct union fields still suppress their field Drop; nested
values keep ordinary Drop. No parser/inference/method/trait-solver changes.

Boundary: receipt is run-local mutable execution transport, never a query value
or source authority. Existing Type/member/header/epoch checks stay. Runtime
preflight/commit inspect actual cells, ticket owners, stamps and image bytes;
no source planning/global namespace scan/BodyIndex clone. Cost is O(1) receipt
transport plus the existing old/new field image work; no measured speed claim.
No resource accounting, caps, static/borrow/privacy checker, sandbox, unsafe,
frontend/query/backend or parallel cleanup evaluator.

Evidence: unchanged upstream witness with exact CHECK effects, all4editions;
existing ordinary replacement/panic/suspension/exact-once owner regressions;
source/user macro/wrapper/standard expansion route where exercised. Include
LRR-only receipt mismatch/currentness controls outside originals. Record all
8recurring modes at an exact source fingerprint, separately from MA002/3/6.
Wide/provenance/enum view cleanup, generic/epoch/full origin matrices and all
full task/interpreter/JIT gates stay held until actual acceptance. No narrower
unsupported result may be counted as a pass or make a valid case inapplicable.

V458 lib build101: CellStamp's field is epoch, not location_epoch; existing
write_plain_image requires &mut self (address materialization), so the new image
preflight must carry that same mutability. No runtime/CLI/control was run. V459
fixes those mechanical API mismatches and adds LRR-only actual transaction
controls: correct old owner, wrong source/expected owner/source epoch, a cleanup
root moved out and restored, different same-Type root value and a foreign store
with identical arena handles. Rejection must leave owner inventory unchanged.
Real shared-image Drop behavior remains the unchanged original's responsibility.
V459 lib build101 only a host control TypeKey::Tuple API mismatch (Box slice,
not Vec); production compiles. V460 uses the pinned descriptor representation.
No runtime/control pass is inferred from either failed build.
V460 lib build0 only, no CLI/runtime/control; closing guard0. V461 adds a route
control outside imported originals using the complete unchanged Drop bodies and
assertions in all4editions: original, arbitrary emit, wrapper and a standard
assert around the nested replacement. Crate attributes stay outside item macros.
It checks ordinary AssignLocal/AssignPlace dispatch and empty streams; matching
values alone do not establish the full cleanup/origin/epoch/suspension matrix.
