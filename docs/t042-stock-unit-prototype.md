# Current status — whole structural moves and field reinitialization integrated

T042 now transfers complete non-Copy structural tuples/arrays without cloning
their fields or forcing a cold nominal sibling's policy. Projected intrinsic
reference/String replacement and moved-field initialization preserve exact
owners, cached completeness, inherited permissions and reference epochs.
Computed temporary receivers still require a live completed operand before
place publication. Copy/Deferred aggregates and source-selected nominal Drop
remain pending. T042 stays in_progress; full release acceptance remains false.

The final frozen candidate passed525 scoped project tests, all60 T042 controls,
formatting, strict Clippy and architecture on unchanged source. Earlier failures
and segmented repair evidence remain retained. The preceding MAIN macOS milestone
passed2273 project tests and all11 checks; a fresh integrated milestone is pending.

MAIN retains143 official rustc cases/250 unchanged originals with both licenses.
Twelve unchanged-source programs have completed interpreter execution under the
explicit Rust2024 adaptation, exercising53 original assertions. The Unit program
adds no assertions. These observations include tested predecessor/candidate CLIs;
fresh integrated CLI execution remains required. Full compiletest fixture passes0.
General aggregate Copy, trait/source Drop integration, production JIT, complete
service/native/security/performance acceptance, release and Linux remain pending.

Earlier frozen candidate history follows.

# T042 original Unit assignment and whole-program candidate

Unvalidated and unintegrated separate stock-Unit successor to frozen initialization V1. It adds no runtime or VM production behavior. It retains the unchanged official run-pass type/unit-type-basic-usages.rs and original directive, with upstream commit/blob/SHA provenance and both licenses. Private inventory141 cases/248 originals; no fixture pass is claimed.

One scoped control uses the exact original `_u2=()` AssignLocal recipe, supplies completed f(u1) and Unit RHS, then checks owner transfer, unchanged target epoch and charged assignment. Full unchanged-source CLI execution and native2015/2024 reference compilation/execution remain required separately. The source contains no assertions, so a new original-assertion count cannot be invented; full completion/call/assignment observations must come from actual execution.

The earlier unrun alias-only V1 candidate proposed a redundant result/RHS guard. Review of existing Frame::preflight_write_value and GuestStore::validate_initialization_value established that a live aliased RHS already refuses before effects; Frame::prepare_write cannot dispose a live temporary. There was no demonstrated implementation bug. The redundant guard and its implementation-mirroring test are removed, and that frozen candidate is retired without running it or changing its bytes. Existing meaningful RHS/target alias admission remains intact.

Initialization V1 validation is separate; T042/general aggregate Copy/nominal Drop/projected moved-field reinitialization/JIT/release/Linux remain incomplete. This candidate's test helper only extends retained AST facts to the original Unit source; it does not provide an alternative frontend. No current check is claimed passed.

V3 harness repair: frozen V2 passed formatting/strict Clippy and 375 library tests; the new Unit control failed only at its final fuel assertion (actual2 versus expected1). Ownership/value/epoch assertions before that point passed. The source recipe has one AssignLocal and one Next exit; unchanged vm/step.rs931 charges each operation and vm/step.rs983 charges each exit. V3 corrects the harness expectation to2 and asserts one completed operation and exit, without changing runtime/VM code or any upstream source. V2 failure remains retained. No V3 check is claimed passed.
