# T040 construction components

T040 remains in progress. This document describes collected component evidence and its limits; it is not a task-completion or release receipt. Validation uses macOS ARM64 and Rust 1.99.0. Linux acceptance is pending. Full stock rustc fixture passes remain **0**.


The integrated macOS constructor-component milestone passed2152 actual host/doc checks, with0failures/0ignored and all37 registered T040 controls exactly once. Gates00–03 and the initial seven checks passed on3621 fixed source/include inputs. The default interpreter row used an old binary and is excluded; a separate explicit current MAIN compilation and selected interpreter/native-reference check passed. Full task/stock/release acceptance remains incomplete. [Collected milestone](../artifacts/macos-first/t040-constructor-components-main-milestone-v1/independent-collection.json).

## Reached source binding

The integrated constructor component resolves an exact ConstructorPlan at the reached entry PC, selects the actual source/body owner and lexical type namespace, and binds a source/schema-checked nominal witness before construction guard allocation. Named fields retain written evaluation order independently of declaration-order storage. Field hints retain primitive identity, including isize versus i64. Matching completed macro contexts and checked no-item proofs are reused; physically generated tokens cannot be assigned fabricated original-source hygiene. Unresolved members, generic/owner context, compound fields and pending producers remain deferred in the frontend. The current runtime reports unsupported demands; general resumable source/type demand integration is still required.

Three exact controls passed on the frozen context candidate: original Large constructor-prefix execution, the same prefix with its matching published macro context, and missing-source-owner rejection before guard/fuel effects. The prefix has 12 original isize literals, 26 operations and 26 recipe exits, using 52 fuel. The harness enters the constructor directly and stops at that exact boundary. It does not execute the annotated let, f(), main(), or the whole upstream test. The unchanged source is tests/ui/structs/large-records.rs.

Evidence: [retained context collection](../artifacts/macos-first/t040-constructor-context-retained-failure/independent-collection.json). That driver subsequently failed a cleanup harness assertion; its three passing controls are scoped evidence, not a successful full driver.

## Partial guard cleanup

Completed builtin scalar fields transfer from the guard once, in reverse completion order, before their physical cleanup. Empty guards retire without a field-drop obligation. Unknown aggregate/destructor fields refuse scalar cleanup before cursor or owner mutation. The guard mapping charge retires only when the guard is empty. Cleanup polls cancellation and charges each completed field.

Eight exact controls, strict Clippy and the fresh architecture audit passed on the separate frozen handle-expectation repair. Its production code is identical to the context predecessor. The tests use the original unary-tuple operand plus LRR-specific injected panic-state/ownership checks; they do not prove an actual user Drop callback during a guest constructor panic. Append-only handle tombstones retain their budget charge, and a scalar cleanup transfer allocates one new ticket handle.

Evidence: [collected cleanup controls](../artifacts/macos-first/t040-constructor-panic-handle-fix-scoped-evidence-v1/independent-collection.json). The three unchanged dispatch controls were not rerun. No combined eleven-control successful driver is claimed.

## Required remaining acceptance

General tuple/array binding and expected composite contexts; repeated-array pure length/Copy handling; full source-view propagation; source/generic/member and producer demand resumption; ordinary aggregate NoDrop/Drop plans; bounded resumable cleanup and actual partial-constructor guest panic/destructor execution; complete named-field order/storage fixtures; full cold-schema checks through the integrated driver; and fresh milestones after remaining feature integration remain required. Later T041-T045 and M017-M027 integration obligations remain explicit under Section 57. No provisional component evidence fulfills the final release gates.

A historical unvalidated candidate imports tests/ui/structs/unit-like-struct-drop-run.rs and refuses live aggregates at scalar-discard admission even when their fields are empty and their cached destructor is absent. Its stock schema contains impl Drop. Only an admission regression is proposed; the upstream threaded guest body remains unexecuted. [Recoverable candidate source](../artifacts/macos-first/t040-constructor-drop-admission-source-snapshot/manifest.json).

The stock-frontier successor passed strict Clippy, architecture and four exact controls on frozen3622 inputs/39763839bytes. Original threaded unit-Drop binding retains its typed import demand. The separate unchanged destructor-run-for-let-ignore-6892.rs Foo schema binds without imports; its live empty nominal cell refuses scalar discard twice without ownership or handle changes. Neither stock main nor user Drop ran. Constructor components are now integrated; the collected fresh macOS milestone is recorded above. [Integration](../artifacts/macos-first/t040-constructor-components-integration.json).
