# M016 component acceptance

The available origin-chain and discovery-diagnostic component is accepted under
Section 57.1 / E-57-001. `completion_scope="component"`, `release_complete=false`.
The source review and every registered M016 control passed; full MX033 and MX034
remain pending without passed evidence. This permits T040 next in the fixed order.

The macOS Rust 1.99.0 milestone used one unchanged snapshot of 3,596 inputs /
39,483,605 bytes. All nine checks returned zero across two preserved segments:
the first driver exited 1 after five successful checks because the one-time clean
removed the directly invoked xtask binary; the resumed driver rebuilt that tool
and exited 0 for the remaining four checks. No single all-nine driver succeeded.
The corrected independent collector exited 0, preserving its earlier reporting
failure and verifying all 35 M016 controls and 2,115 host/doc tests with no failures.

See [joined acceptance](../artifacts/macos-first/m016-origin-repairs-main-combined-acceptance.json),
[actual terminal collection](../artifacts/macos-first/m016-origin-repairs-main-combined-collection-terminal.json),
and [requirement/source review](../artifacts/macos-first/m016-component-acceptance-review.json).
The retained evidence snapshot contains the pre-transition ledger and source hashes.
No failed ancestor was converted into a pass.

MX033 still requires a reached generated method type/operation error, real member
selection and an invocation/definition/source chain that remains correct after an
edit. MX034 still requires body-only versus impl-header edits with actual header
query reuse and catalog/dispatch invalidation. Recorded dependencies cover T040–T045,
source views/member resolution, selected trait operations, const/opaque descriptors,
JIT equivalence, portable caches, suspension, native worlds, service/freshness and
the M043 fixture oracle. Component tests do not replace these full LRR assertions.

The official inventory is 78 cases / 158 unchanged retained files at
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, under the original MIT OR Apache-2.0
licenses. Full stock fixture passes remain zero. Guest JIT, full service/native
integration, release completion and Linux acceptance remain unfinished.
