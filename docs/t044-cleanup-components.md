# Current status — bounded return and constructor cleanup milestone accepted

T044 remains in progress. Return cleanup retains the returned owner and suspended caller until bounded scope cleanup completes. Partial constructor guards drain completed fields in reverse completion order and preserve exact child ticket/location acknowledgements across dispatch and interruption. Source-selected user Drop and general trait integration remain pending.

The fresh integrated macOS milestone passed 2,362 project test bodies across 179 executables with zero failed, ignored or filtered bodies. All 17 distinct commands completed across two segments, including formatting, strict Clippy, architecture, gates 00–03, upstream integrity, a fresh CLI and seven unchanged stock-program probes. All 73 T042, 41 T043 and 21 T044 controls passed once; eight H002 aliases and the 2,000-frame control passed once. Root independently verified 34 command log hashes, terminal/release receipts, CLI identity and unchanged source membership before updating this documentation.

The first runner exited 1 solely on a stale wrapper predicate expecting seven T044 events; gate03 itself exited 0 with all 21. Its failure is preserved. Only the nine unexecuted suffix commands ran afterward, without repeating the gate. A failed collector-only attempt is also preserved; corrected read-only collection passed. No single 17-check runner exit 0 is claimed.

MAIN retains 164 official cases / 283 unchanged original files. Recorded stock-program observations remain 13 / 54 original assertions across tested snapshots; today's seven probes repeat existing observations and add no unique passes. Full rustc compiletest passes remain 0. The separate V7 candidate failed four library controls (465 passed / 4 failed); its formatting and Clippy passed, while dependent checks stayed unexecuted. The failures are retained. V9 fixed those setups and passed 471 of 472 library bodies; one unit-pair control assumed a single-step scope scan and failed. V11 repairs only that assumption and is under separate validation, carrying the 471 unchanged passes. Bounded structural Discard and the mutable V10 local-replacement draft remain excluded from MAIN. User Drop, replacement, panic/unwind, general traits, guest JIT, release and Linux acceptance remain incomplete.

Evidence: [independent milestone collection](../artifacts/macos-first/t044-return-guard-integration-v5/validation-v1/milestone-collection-v1.json), with terminal and source release sidecars.

# T044 bounded cleanup components

T044 remains in progress; full revision-4 acceptance and release completion are false.
The frozen ScopeLeave V4 candidate supplies seven acceptance controls registered in
compat/fixtures.toml. Runtime/VM ownership transfer retains exact pending child
location and ticket until cleanup completion; fields retire in declaration order,
partial moves skip extracted fields, and the parent cannot overtake a pending child.
Every executed ScopeLeave advances at most 256 transitions before Dispatch. Resume
preserves its exact PC, operation identity and entry charge. Fuel/cancellation refusal
keeps unprocessed owners and pending continuation state. Query providers execute no
guest callbacks. Nominal cleanup without a selected source Drop witness is reached
unsupported, never an inferred NoDrop decision.

Original rustc struct-field-drop-order.rs supplies declaration order and the exact
first drop(noImpl) statement recipe. Fields/its completed call frontier are disclosed
intrinsic String/tuple/257-element-array adaptations; Field/Rc/Cell/println/destructors
and full expected output remain unexecuted. struct-partial-move-1.rs supplies original
declaration ordinals and p.y extraction, adapted to intrinsic String tuple storage;
struct update, S/derive/FnMut and full assertions remain unexecuted. The additionally
retained destructuring-assignment/drop-order.rs is imported and unexecuted. All copied
originals retain the pinned commit, hashes, directives and MIT OR Apache-2.0 licensing.

Scoped validation: format/strictClippy passed; initial library447pass/1PermissionDenied
failure and one separate exact retry pass; corrected ledger1pass; 17 integration
binaries177pass; importer/audit passed. All 121 registered T042/T043/T044 events passed
once in their selected accepted phases, including the eight T043 H002 aliases. The
separate H002 control and 2,000-frame test are independently counted. This is not a
single all-green448-test library invocation or a full rustc fixture pass. A fresh MAIN
milestone is required after integration.

Still required: selected Drop guest frames and self activation; M026/M029 predicate
fulfillment, conditional applicable/ambiguous Drop, cold-body/invalidation evidence;
return, replacement, partial constructor and panic/unwind integration; exact ordinary
Drop transcripts across interpreter/JIT/native tiers. Separate return and constructor
successors are unvalidated and excluded from this integration. Production guest JIT,
full rustc fixtures, release/security/performance and Linux acceptance remain false.

Fresh integrated MAIN milestone independently collected:2348 project bodies /179 executables, zero failed/ignored/filtered;17 completed checks; all121 T042/T043/T044 events once; eight H002 aliases within T043; 2000-frame event once; fresh CLI and seven repeated stock probes. Initial missing-tomllib failure and corrected Python3.14 continuation remain preserved phases. Stale raw report.unexecuted_checks is reconciled against terminal/status and all completed commands. Full task/stock/JIT/release/Linux acceptance remains false.
