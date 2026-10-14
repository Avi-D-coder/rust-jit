# Integrated scoped T042 ownership work

The implementation described below is now integrated from tested V7 bytes.
Formatting/strict Clippy and scoped suites passed: library358 (all21 controls),
lrir14, vm_kernel20, source_run67, standalone_cli27 and architecture. The unchanged
move-arg.rs CLI completed with its original assertion once, with native2015/2024
reference passes. A fresh MAIN milestone is required. T042 remains in_progress;
full fixture/JIT/release/Linux acceptance remains false. Earlier draft wording
below is historical and does not describe current acceptance.

# Private T042 owning-read prototype

Task/invariant: T042 ordinary owning reads select an intrinsic Copy or transfer
policy before effects; String and mutable references move one owner, primitives
and shared references copy. Moved ordinary reads report USE-AFTER-MOVE, while
lease resolution retains STALE-HANDLE. Complete destination/ticket resources are
reserved before source ownership is consumed, including numeric materialization.
This is unbuilt, unexecuted and unintegrated; T041's combined gate source stays
fixed. Fixed-order MAIN task progression still requires collected T041 acceptance.

The first draft removes the local-read unconditional-copy path and parameter
raw-kind move heuristic in favor of a common typed runtime outcome. It introduces
no frontend/backend/query/sandbox change, no unsafe, no trait-provider effects and
no default width inference. Nominal and aggregate owning reads still explicitly
require further checked Copy/source policy implementation in T042/M017-M027;
this draft does not fulfill the full T042 card.

Seven initial controls use unchanged use-after-move-based-on-type.rs. Pinned RA
AST facts decode the original String literal and original two local declarations.
The harness supplies to_string's result outside the licensed import; runtime
transfer, use-after-move, reserve refusal and single-owner publication are checked.
The double-owner property rejects a raw bitcopy model, then follows one owning
String through32 fresh locations, checking actual live cells and object owner at
every transfer. No original to_string/println, full compile-fail, user Drop, fatal
VM cleanup policy or stock fixture pass is claimed. Four VM controls enter the exact original x path through its real verified
ReadLocal recipe, with supplied original construction result. They check one
move, source/destination state, complete reservation, fuel refusal and destination
type mismatch. Primitive/mutable-reference/aggregate/source-view coverage remains
required before component completion. Full upstream diagnostic snapshots are retained.

Next: validate these seven controls once, then add primitive/mutable-reference
and source-context coverage and implement guarded aggregate Copy/partial-move admission,
then validate asynchronously after T041 acceptance. Preserve original diagnostic
and native reference requirements separately. Never classify a missing destructor
witness as proof of no user Drop.

The owning move helper currently refuses field/buffer extraction without its
checked source move plan. The two root-location modes cannot silently bypass
a user Drop restriction or authorize moving a non-Copy referent through a lease.
The source facts use the pinned RA AST text API directly; no alternate parser.
Both private formatting runs terminated with exit0. No compiler/test has run.


V2 adds unchanged borrowck-move-mut-base-ptr.rs plus its .stderr: private inventory
134 cases/235 originals; MAIN remains127/221. Scalar Copy uses the original
move-arg.rs initializer under its selected written isize parameter context.
Exact original t0 ReadParameter execution transfers a mutable reference while a
shared referent lease remains live; supplied argument setup and direct replacement
stay disclosed adaptations. Parameter source views go through the real checked
installer before ArgumentPacket delivery, limited to primitive/rigid parameter
views and closed primitive references. General composite/binder/trait contexts
remain later integrations. No unsupported installer is bypassed by positive tests.

Every parameter metadata estimate reserves the admitted owned source boxes and
uses actual retained vector capacities. Written primitive/reference descriptors
and access match before argument delivery and again before each read; differing
isize/i64 identities refuse despite equal width. Source/contract arity differs
only by an explicit CALL-ARITY refusal. A bound numeric literal retains its shared
record and uses the effective signature descriptor rather than raw type_id0.
Fourteen controls are authored, still unbuilt/unexecuted/unintegrated. No new
stock, host test, gate, release or Linux pass is claimed. Frozen V1 is superseded
without execution; V2 is the first proposed validation input after T041 acceptance.

V6 repairs the parameter storage boundary exposed by unchanged
borrowck-move-mut-base-ptr.rs: a checked closed primitive reference contract
may allocate its callee location, preserving exact target TypeId and access.
The sealed packet and source checks still precede allocation/delivery. General
aggregate, abstract and nested-reference parameter bindings remain deferred.
V5 passed formatting/strict Clippy and eight controls; its ninth failed before
move execution. The remaining controls and suites were unrun, and that failed
candidate remains retained. V6 has no acceptance evidence yet.
