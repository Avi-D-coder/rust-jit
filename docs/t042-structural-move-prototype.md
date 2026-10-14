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

# T042 certified non-Copy structural transfers draft

Unvalidated private successor to moved-field V2. MAIN remains unchanged under its active macOS validation hold. Invariant: a structural tuple/array whose existing descriptor has a final NotCopy certificate transfers its one complete object owner on an ordinary owning read; it is never bit-copied. A cold nominal sibling can remain Deferred because one intrinsically non-Copy member already proves the structural value cannot be Copy. Missing cached Drop witnesses are not NoDrop. No nominal policy is selected from shape alone.

The draft adds a reached intrinsic owning-read case; incomplete roots still fail cached-completeness admission, moved roots still report use-after-move, partial reads through array/deref/nominal parents retain existing restrictions, and whole Copy/Deferred aggregate reads remain refused pending bounded ordinary Copy integration. It does not run a guest destructor, clone members or traverse their bodies.

Acceptance controls must reuse the existing exact source-bound tuple-field-move-with-drop-borrow shape, including its mutable Unit-reference member and cold WithDrop nominal sibling. Supplemental transfer-chain, quota and duplicate-owner regressions will exercise the backend; they are not whole original-source execution or a compiletest fixture pass. Five registered, unexecuted controls cover a whole-tuple move with stale root lease/use-after-move/duplicate-ticket refusal and preserved cold sibling, quota atomicity, borrowed whole-move refusal, a disclosed single-element array wrapper with forbidden indexed extraction, and Deferred structural policy refusal. These use the unchanged original tuple/WithDrop fixture and supplemental LRR ownership states; no language program or destructor is synthesized as a stock pass. General Copy, source-selected Drop, full T042, JIT, release and Linux acceptance remain false.
