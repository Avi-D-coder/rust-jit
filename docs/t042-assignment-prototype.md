# Integrated T042 assignment components

Exact scoped-tested assignment V3 code is integrated. All31 ownership controls
passed once in the unfiltered368-test library; the scoped suites total496 tests.
Formatting, strict Clippy and architecture passed. Validation used two segments:
the first external runner returned1 because it used incorrect inherited test
module names, although the Rust command passed; independent raw-output collection
confirmed all31 controls. Only the five unrun commands ran in continuation8887/0.
The original runner failure remains retained and is not described as a successful
single runner. Source3721 stayed unchanged throughout both segments.

The preceding MAIN milestone24061/0 independently collected2256 project tests,
all11 checks and all21 prior T042 controls. This new integration requires a fresh
MAIN milestone. T042 remains in_progress, release_complete=false. Original source
assignments run with disclosed prepared declarations and RHS values; full stock
fixture, aggregate Copy/Drop, projected reinitialization, JIT, release and Linux
acceptance remain incomplete. Earlier draft wording below is historical.

# T042 assignment transfer draft

Status: separate private unvalidated and unintegrated candidate. The MAIN macOS milestone is running on the prior integrated owning-read source; these edits do not change it.

Invariant: validate the destination contract and reached cleanup obligation, then reserve the complete monotonic cell/ticket batch before consuming the RHS. Successful replacement preserves the destination location epoch, moves the RHS exactly once, and retains then disposes the intrinsic old-value ticket before publishing Unit. Reinitialization keeps the already advanced moved-location epoch. Invalid aliases, immutable replacement and unknown aggregate Drop refuse before assignment fuel/effects.

The draft adds a checked whole-batch reservation for local assignment, a pure old-value cleanup admission, and explicit destination/RHS alias refusal. It preserves RHS-before-LHS source lowering. Ordinary aggregate Drop continuations, partial-field reinitialization, general aggregate Copy and complete source/trait integration remain required; this draft does not complete T042.

Three unchanged official rustc cases and all eight original diagnostic/rustfix files are retained with both original license texts and pinned commit/blob/SHA256 provenance: issue-58776-borrowck-scans-children.rs, issue-61424.rs and assign-imm-local-twice.rs. Their full source has not run in LRR or the native oracle. The harness uses the exact original AST assignment and actual verified AssignLocal recipe with disclosed prepared declarations, prior values and completed RHS values. It does not execute closures, String methods, println/dbg, later statements, rustfix, or compiletest diagnostic matching. The upstream borrow-check error is not claimed satisfied by the relaxed lease profile.

Ten controls cover String replacement, handle exhaustion including an existing result cell, fuel refusal, deferred initialization, moved-local reinitialization, immutable first/second assignments, forged aliases and unselected aggregate Drop admission. The quota, moved-state, forged-frame and aggregate controls are supplemental LRR regressions, not stock whole-fixture passes. No control is currently claimed passed. Private inventory: 140 cases / 247 original files; MAIN remains 137 / 239. Full fixture acceptance, JIT, release and Linux remain false.

The inherited private task ledger predates the integrated T041/T042 ledger transition. It is retained for exact parent-source lineage and is not a claim about current MAIN task status. Readiness and this document describe this candidate; an eventual integration must join the current MAIN ledger and collect a fresh milestone.

V2 review correction: the unexecuted V1 aggregate admission control used an empty tuple, which the pinned type registry canonicalizes to primitive Unit. V2 uses a two-field Unit tuple and completes both constructor fields before commit, exercising actual aggregate cleanup admission. Production code and all other controls are unchanged; V1 remains preserved and unrun.

V3 mechanical repair: V2 runner session53266 terminated101 after format0 and strict Clippy compilation failed with E0599 at the two arg_list calls. No tests or architecture audit ran; the frozen V2 membership remained unchanged. The pinned ra_ap_syntax0.0.356 ast/traits.rs34 defines HasArgList and generated/nodes.rs365/1171 implements it for CallExpr/MethodCallExpr. V3 imports that existing trait; no dependency pin, production behavior or expected result changes. Independent V2 failure collection is pending.
