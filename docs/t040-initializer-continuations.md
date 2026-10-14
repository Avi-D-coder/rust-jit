# T040 initializer continuation working component

This component is now integrated after segmented scoped validation:32 selected cases passed on v3 and the added stale caller metadata control passed separately on production-identical v4. Both outer drivers and independent collectors exited0; this is not a single combined validation run. MAIN retains104 official rustc cases/191 unchanged files and55 T040 registrations. The fresh integrated macOS milestone is pending; the last completed parent milestone passed2160 project tests/45 controls. Full T040, stock fixtures, guest Drop/JIT, release and Linux remain incomplete. [Integration record](../artifacts/macos-first/t040-constructor-continuation-components-integration.json). The draft description below records its prior scoped stage.

This independently owned successor is unvalidated and not integrated. Its parent initializer-source component is integrated after scoped validation; the parent macOS milestone passed2160 project tests and all45 T040 controls. Full stock rustc fixture passes remain zero. General initializer frame entry, declared constant type resolution, constant evaluation, source/generic/trait suspension, user Drop, guest JIT and release acceptance remain pending.

Task invariant: a final initializer PC belongs only to the exact immutable original const/static root and its checked BodyIndex result. A nested field expression cannot claim that exit. Existing PC phase encodings and ordinary function/block exit semantics remain unchanged.

A single administrative InitializerReturn node (stable role tag6, previous role tags0–5 unchanged) is appended after the real initializer syntax preorder and its temporary annotations. Real syntax node IDs and written operands remain unchanged. It has an existing Entry PC, checked node/slot/metadata budgets and its own Unit temporary slot. No phase encoding is added or repurposed.

Non-block original initializer results now continue to that distinct PC. Its recipe revalidates the immutable AST boundary/body hash, executes the mandatory Unit operation in a separate non-aliasing temporary, and uses the existing Return exit for the actual root result. Constructor lowering selects it after ConstructFinish; condition, scalar and loop completions use the common source continuation path. Normal if/while Exit phases retain their original branch/cleanup meaning. This does not enter an initializer frame or evaluate a global constant implicitly.

Frame return admission now uses GuestStore's checked object move admission for kind8 values. It validates object storage/type/owner before a transfer; numeric literals still require reached scalar binding. Primitive return descriptor mismatches retain their existing check. This grants no Copy/NoDrop/Drop proof, native loading or source-view inference. Other unsupported kinds retain GuestStore's existing rejection.

Three unchanged-stock controls are staged:

- consts/issue-19244.rs: actual original STRUCT source/schema binding, one-field guard, literal42, declaration-order installation, checked root return and ownership transfer exactly once. The component harness explicitly enters initializer scope0; it does not invent an implicit IR scope entry. Returned aggregate fields remain owned/live after the moved frame cells retire; duplicate return fails. Original main, array lengths, nested constants, source member projection and aggregate Drop are unexecuted.
- consts/control-flow/basics.rs: original ABS_DIFF else-phase still leaves its conditional scope and selects its actual X - Y block. Its PC differs from the administrative return; X/Y and branches remain unexecuted.
- consts/const-as-fn.rs: actual FOO=0 literal/root return recipes. Without a resolved declaration context, the final phase accounts for its Unit operation, then refuses Return before exit fuel or ownership transfer; the original numeric literal remains unchanged. Node/slot budget boundaries and invalid administrative phases are also checked. No usize witness/default or complete compile-fail acceptance is invented.

Effect classifications:

| Path | Effects |
| --- | --- |
| BodyIndex::initializer_return_pc | Pure checked administrative PC lookup; no query or guest operation |
| InitializerReturn metadata append | Deterministic host metadata and fallible reservation within existing quotas; no guest allocation or source type demand |
| initializer_result | Pure pinned source/root/index boundary inspection; no query or guest operation |
| ParsedFile::initializer_exit | Pure checked source hash and verified Unit/Return recipe allocation; no child demand or compiler effect |
| return_pc initializer branch | Pure symbolic PC selection; no frame entry or evaluation |
| Frame::preflight_return object branch | Pure checked GuestStore owner/type/storage admission; preserves hint check; actual transfer remains the existing Return exit effect |

None of the staged controls has run. Strict Clippy/architecture, exact stock controls, existing numeric/return/constructor regressions and a fresh milestone after integration are required. The parent milestone is collected. The fixed joint successor is under separate immutable async scoped validation; this working copy remains independently owned and unaccepted.
