# T041/T044/M018 — separate replacement cleanup source review

Baseline8fefce6624498038f94720a4766b318ad74c7401; candidate V461 fingerprint
`a3f99d2e875426a9e0824b4cb72a8d110bfcd57ad5ed554b8d7d35c745b40cad`.
Source conclusions only until actual terminals/guards are recorded below.
MA002/MA003/MA006 are separate audit IDs; no full architecture closure.

Inspected runtime cell::{ReplacementCleanupRoot::admit,replace_for_cleanup,
replace_place_for_cleanup,replace_with,validate_replacement_cleanup_root,
initialize_replacement_cleanup_root,admit_replacement_with}, memory::{
prepare_memory_view_write,commit_memory_view_write,read_memory_view};
vm frame_scope_cleanup's AssignLocal/AssignPlace activation branches and
cleanup::{prepare_root,PreparedCleanup::relocate_replacement_root,start}.

1. No assertion/expression compiler/evaluator. The receipt transports actual
run-local owner facts after the existing physical replace/root installation.
No Rust expression, source token or pattern is interpreted by it.
2. No generated scalar/break executor or alternate VM/verifier namespace.
Original/user-macro/standard-macro assignments all enter the same verified
AssignLocal/AssignPlace operations and existing cleanup state machine.
3. No method/inference/macro identity rule or Type-only cleanup substitute.
The exact store session, prior source cell/epoch/ValueRepr and actual destination
cell/epoch/detached ValueRepr are checked. A reconstructed image can have a new
ObjectId only through the real store transaction. Actual source Drop selection
is unchanged; direct union field writes still suppress field Drop and nested
values keep their existing destructor obligation.
4. No generated-source exclusion or fabricated source/scope/epoch receipt.
This is an internal, non-Clone, non-serialized, run-local ownership receipt,
not a query value or source admission. Source/header/type/member guards stay
with existing producers. Private fields have no production constructor outside
the checked store module; callers receive the result of the actual replacement.
5. No pattern or cold arm/callee preparation changes. Source Drop method entry
and panic/suspension cleanup stay the existing dispatcher/cleanup continuation.
Image reconstruction only reads the selected old value before writing new bytes;
wide/provenance/enum/partially moved image gaps remain explicit blockers.
6. No source parser/query/plan in receipt admission or installed operations.
Incoming View image bytes/ownership/contract/ranges are prepared before an old
owned snapshot is allocated. The existing image encoder may materialize runtime
pointer address metadata; it executes no guest callbacks and consumes no source
owner. Only the common commit writes bytes and retires the incoming physical
source graph, without running guest Drop on the transferred new initializer.
7. No namespace census, unrelated invalidation or global map. Receipt checks
are O(1) handle/session/stamp/value checks plus existing lease/owner validation;
view replacement retains O(old/new image) work. No speed/scaling claim.
8. No BodyIndex/AST copies or whole-file node searches. The receipt moves a
small execution packet; image vectors are actual mutable guest data. No new
resource accounting/cap/fuel/sandbox/static check/unsafe/parser/query framework,
backend or custom trait solver. Actual StoreSession supplies cross-run identity,
even when another store has equal arena handle bits and values.

Removed alternate assumption: cleanup relocation no longer requires a shared
view metadata ObjectId to equal its real detached old owning image. It consumes
the store-issued exact source-to-destination proof, then retains that detached
owner and its location epoch for the same prepared cleanup start. Ordinary
replacement/root initialization remain the same shared physical kernel; raw
replace users still get their old owning ticket through the existing API.
Prepared cleanup still refuses another owner/run/lifetime; it does not simply
accept the same Type or rename the old failing equality as a successful check.

Held requirements: complete union/image lifetime/alias/partial move/provenance/
wide/enum cleanup; explicit in-place drop of a View outside replacement;
source/epoch/suspension/cleanup origin matrix beyond actual tested rows;
remaining raw logical-state readers and legacy source/member routes; full
upstream solver/interpreter/JIT acceptance. This partial checkpoint cannot close
any task/architecture/milestone/release gate. Runtime evidence pending below.

V461 final actual evidence: both macOS Rust1.99.0 builds0;8/8exact controls0,
including correct receipt adoption, five mismatch cases plus same-handle foreign
store refusal without owner inventory changes, ordinary replacement/partial
move/suspension/panic regressions and16edition/origin union Drop worlds. Actual
AssignLocal16/AssignPlace34 counters are observed in each matrix world. The
unchanged owning2015 union-drop-assign main completes317matched dispatch/outcomes,
2recorded source-call boundaries,1initializer, unit/empty streams/cleanup;
union-drop and union-manuallydrop-rpass complete518/36operations. The recorded
source-call metric is not asserted to count destructor entries; original CHECK
assertions and inspected ordinary NeedWitness/Drop continuation establish the
selected destructor effects. Only union-drop-assign has a measured failing
comparison here (prior211dispatch owner refusal), so one distinct new stock
gain is claimed. The other2pass observations have no parent/morning comparison.
Closing source/HEAD/test/CLI guard0; full evidence retains failed V458/V459
builds and lib-only V460 without promoting them to runtime acceptance.

Incoming-image preflight precedes old snapshot allocation, but this checkpoint
does not prove every assignment fault/partial-image/provenance cleanup path.
Those and explicit in-place View destruction remain holds, as do the complete
origin/epoch/currentness/cancellation/source/solver/interpreter gates. No source
changed after V461 validation; no task/architecture/JIT gate is closed.
