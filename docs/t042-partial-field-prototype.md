# Integrated scoped T042 ownership work

The implementation described below is now integrated from tested V7 bytes.
Formatting/strict Clippy and scoped suites passed: library358 (all21 controls),
lrir14, vm_kernel20, source_run67, standalone_cli27 and architecture. The unchanged
move-arg.rs CLI completed with its original assertion once, with native2015/2024
reference passes. A fresh MAIN milestone is required. T042 remains in_progress;
full fixture/JIT/release/Linux acceptance remains false. Earlier draft wording
below is historical and does not describe current acceptance.

# T042 structural partial-field draft — untested

This draft extends the unintegrated V2 owning-read draft. It is not a T042
completion record and has no test or full stock pass evidence.

The unchanged rustc tuple-field-move-with-drop-borrow.rs supplies the exact
consume(x: (&mut (), WithDrop)) parameter and x.0 tail. The adaptation prepares
the parameter/place outside the licensed directory, then exercises direct checked
place extraction and the original verified ProjectTuple/ReadPlace recipe. It
retains the source-bound unit-like WithDrop sibling without selecting its Drop
body. Argument delivery, general tuple source binding, returned-reference
lifetimes and the guest Drop transcript remain pending.

An owning field read resolves every enclosing owner. Mutable-reference extraction
moves one lease owner while preserving the referent, sibling cells and parent
epoch. It refuses extraction through a dereference, and only tuple ancestry has
intrinsic NoDrop permission. A nominal enclosing owner still requires a selected
source Drop policy: a missing descriptor witness never proves NoDrop. The required
LRR-E-PARTIAL-MOVE-DROP diagnostic and positive nominal NoDrop/Copy witnesses remain
future source/trait integration work, with no component completion claimed here.

A bounded explicit stack checks initialized aggregate fields before an owning
whole-value read, without resolving unrelated Copy/Drop headers. Moved locations
report LRR-E-USE-AFTER-MOVE for value reads; existing stale lease checks remain
separate. No raw owning ObjectId is bit-copied and no guest call enters a helper.

Seven supplemental controls exercise the unchanged original context: four runtime
checks for sibling/referent preservation, repeated and whole-value read refusal,
direct child ancestry, reference access and ticket quota; three actual original
VM recipe checks for exact-once transfer, read fuel refusal and result-type
mismatch. The original fixture is not a whole-program pass. The newly copied
move-out-of-tuple-field.rs and move-out-of-field.rs are retained inputs for later
Box/nominal extraction, with no execution claim.

V4 corrects two pre-existing fixture-helper calls to the actual SourceRange::checked
API. V2 formatting passed, but strict Clippy failed before any ownership control
ran; the failed frozen V2 and unrun V3 remain preserved. V4 combines all 21
controls for validation on one immutable snapshot and is not yet accepted.
