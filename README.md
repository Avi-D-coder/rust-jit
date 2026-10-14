Contribute implementation work through the [12 parallel worktracks](worktracks/README.md): choose a bounded item, coordinate ownership, and submit a PR with unchanged upstream fixtures and reproducible evidence. The [registry](worktracks/registry.json) and [claims](worktracks/claims.json) track plans and current reservations. This contribution system is prepared locally; remote issues/PRs and merged contributions are recorded only when they actually exist. Follow the [source checkpoint guide](docs/source-control.md) to keep generated outputs local and enable the optional commit guard.

The governing [specification is now revision 5](lrr_rust2024_macros_traits_implementation_spec.md), adopting direct Rust ABI bindings, typed optimized loops, reconstruction maps, Cranelift plus LLVM tiers, relocatable code reuse and explicit precise/throughput contracts. This is a design/ledger update only: new requirements and T085–T089 remain unimplemented and unaccepted. Existing runtime receipts below retain their original scope. Interpreter-first priority and all four official editions remain required.

The [runtime-semantics prerequisite](lrr_rust2024_macros_traits_implementation_spec.md#121-complete-upstream-inventory-and-runtime-semantics-prerequisite) requires all applicable upstream runtime contracts and interpreter correctness before guest JIT. Diagnostics and compiler/tooling have separate gates; runtime-only success in a mixed case is not a full-fixture pass. Historical status entries below retain their original evidence scope.

Two Sol6.1/high implementation workers currently own macro edition semantics and generic call identity; root implements lexical call lowering and reviews integration. Three reviewed repairs are integrated after scoped acceptance, with the combined macOS milestone pending. The latest whole gate failed at a stale2021-refusal acceptance; its failure is preserved and both corrected publication controls passed. Upstream imports now retain214cases/356 unchanged original files; full stock compiletest passes remain0. No interpreter/JIT/release/Linux completion is claimed.

# Current status — all official edition roots admitted; macOS milestone pending

The spec and implementation now require Rust2015/2018/2021/2024 roots, dependencies and macro origins. Standalone run/test accepts --edition and reports the actual edition; Cargo root normalization retains its declared edition. Macro proof revalidation preserves definition edition separately from invocation syntax, and sealed runtime contexts retain captured ordinary extern dependencies.

Exact candidate scope passed694 Rust bodies in12 complete unfiltered binaries, strict workspace Clippy/format and architecture/manifest/importer checks. The unchanged official generic-fn-infer.rs program ran in all four root editions and five dependency pairs, including2015-root/2024-dependency, with actual generic callee/assertion execution. Failed witnesses and the scope runner's incorrect xtask-library selection remain preserved; the corrected suffix reused unchanged passing results.

Tested production and assertions are integrated; the required fresh MAIN milestone is pending. The preceding MAIN Cargo command passed2466 bodies/186 summaries, but its enclosing gate failed because inherited vendor subprocess stdout split two result lines. Capturing that output is now tested with one host test thread. The last fully accepted earlier MAIN milestone remains2465 bodies/185 summaries. Full rustc compiletest passes remain0;209 cases/350 unchanged originals retain their upstream licenses/provenance. Full edition, M017/trait/source-view, interpreter, release and Linux acceptance remain incomplete. Guest JIT is deferred.

# Edition compatibility successor — candidate validation pending

Selected roots must support Rust 2015, 2018, 2021 and 2024, retaining each owning edition. Standalone run/test gains --edition (default 2024); Cargo root normalization accepts the declared official edition. The unchanged stock generic program is under interpreter validation in original and paired editions. No new stock fixture, complete edition, task or release pass is claimed.

The last MAIN run completed its unfiltered Cargo test command with 2,466 passing bodies / 186 summaries and zero failed, ignored, measured or filtered bodies. The enclosing milestone failed because inherited vendor subprocess stdout split two libtest result lines. Its direct failure and independent collection are retained; it is not an accepted gate. Capturing those subprocess outputs is a separate validation correction in this successor.

# Current status — retained return-source identity repaired; macOS milestone pending

A stock generic id<T> header exposed a return-packet defect: distinct rigid source parameters were admitted at the same runtime descriptor. The new check preserves already-bound source identity for both known and deferred result descriptors, before map writes. The failing baseline is archived. Source parameter spelling changes also preserve identity within the same owning definition.

The repaired scope passed 550 Rust bodies in 7 complete unfiltered binaries and 3 Python controls, including 38 selected M017 controls once. All 61 registered M017 controls were not selected in this scope. Tested production, assertions and originals are integrated; the required fresh MAIN milestone is pending. The preceding repair milestone ended with a real ENOSPC error, not a pass; its failure and exact child-absence/source review are retained.

The latest accepted preceding MAIN milestone remains 2,465 project bodies/185 summaries. MAIN retains 209 official rustc cases/350 unchanged originals with both licenses. Full stock fixture passes remain **0**. Full M017/TR001/TR002, source-view/trait integration, interpreter convergence, release and Linux acceptance remain incomplete. Guest JIT is deferred; all subagents remain stopped.

# Current status — substitution stack repair integrated; macOS milestone pending

M017 identity substitution and zero-distance shift now preserve the admitted 120-layer stock-derived binder on the ordinary host stack. The acceptance extension reproduced a genuine stack overflow before production changed. Direct Result dispatch and branch helpers reduce native recursive frame storage while preserving all quota charges, typed identities and binder transitions.

The repaired scope passed 576 Rust test bodies in 10 complete unfiltered binaries and 3 Python controls, including all 60 M017 controls once. Exact failed baseline source/logs are retained. The tested repair is integrated; its required fresh MAIN milestone is pending. The latest accepted preceding MAIN milestone passed 2,465 project bodies/185 summaries.

MAIN retains 208 official rustc cases/349 unchanged originals; full stock fixture passes remain **0**. Full M017/TR001/TR002, source-view/trait integration, general interpreter convergence, release and Linux acceptance remain incomplete. Guest JIT is deferred. All subagents remain stopped.

# Current status — wildcard macOS milestone accepted

The macOS ARM64/Rust 1.99.0 milestone passed 2,465 project test bodies across 185 executable summaries, with zero failed, ignored, measured or filtered tests. All 60 M017 controls and the stock float wildcard control ran once. Actual root session 69531 and its gate/runner exited 0 on unchanged source 721f9fd4 after 2,000,185 ms. Independent raw-log, required inventory, canonical ownership and source checks passed before releasing the source hold.

MAIN retains 208 official rustc cases and 349 unchanged original files with both original licenses. Full stock fixture/compiletest passes remain **0**. T030/T025/T031, M017/TR001/TR002 and source-view/trait integration remain incomplete. Guest JIT, release and Linux acceptance remain deferred. All subagents are stopped; root owns implementation and asynchronous validation.

# Current status — wildcard component integrated; MAIN milestone pending

The reviewed wildcard change evaluates value initializers with their written source type and discards them without creating a local binding. The unchanged stock parser/float-literals.rs main completes with actual F64 literal materialization, one discard per declaration and balanced scopes/counters. Direct place initializers still require non-moving source-view integration; reference-operand and later-use numeric constraints remain incomplete. This is component/body evidence, not the original build-pass/edition/lint compiletest contract.

Exact scoped evidence covers580 Rust test bodies in5 unfiltered binaries plus3 Python importer controls. The existing numeric-discard and actual library reference-annotation controls ran once. A nonexistent xtask target stopped the first runner; that planning failure is retained, and only the seven untouched suffix commands followed. Independent inventories, logs, ownership, frozen source and original provenance were checked before integration. M017 production/60 controls and numeric expression fallback remain unchanged.

MAIN now retains208 official rustc cases/349 unchanged original files with both original licenses. Full stock fixture/compiletest passes remain **0**. The required new MAIN milestone is pending; the last accepted earlier-source macOS milestone remains **2,464 project bodies/185 summaries**. T030/T025/T031 and full M017/TR001/TR002/source-view/trait integration remain incomplete. All subagents are stopped; root owns background tests. Guest JIT, release and Linux acceptance remain deferred/unestablished.

Evidence: [scoped review](artifacts/macos-first/t030-wildcard-milestone-rebase-v6/root-scoped-review-v1.json). Earlier sections below are historical checkpoints.

# Current status — M017 integration milestone accepted; interpreter work continues

The integrated MAIN macOS ARM64/Rust1.99.0 milestone passed **2,464 project test bodies across185 executable summaries**, with zero failures, ignored, measured or filtered tests. All60 registered M017 controls ran once. Actual root29168/gate/runner exited0 on unchanged sourceadf2da4 after2,046,630ms; fresh gates00–03 passed with one build identity. Independent raw-log, required-name/inventory, exact-source and ownership checks passed before root released the source hold. No performance gain is claimed from this validation run.

MAIN includes owned dyn/opaque terms, source-header arrays, reference elision and canonical Unicode lifetime identity, immutable frame tables and transaction-region admission guards. The source-derived120-layer admitted binder and130-layer typed depth refusal passed on the ordinary host stack. Original edition2021 declarations use the existing recorded dependency source world; bare legacy live entry remains refused. Earlier handoff, stack, ownership and harness failures remain preserved. These are partial component requirements; M017/TR001/TR002, full trait solving and expression source-view propagation remain incomplete.

MAIN retains **206 official rustc cases/347 unchanged original files** with pinned provenance and both original licenses. Full stock fixture/compiletest passes remain **0**. Project and component tests are not full stock passes. T025/T031, general numeric constraints, traits, patterns, user Drop, printing and worker isolation remain incomplete. The parser is pinned rust-analyzer; VM/LRIR and query integration are project code. Guest JIT stays deferred until interpreter convergence; release and Linux acceptance are unestablished.

All subagents are stopped; root owns implementation and asynchronous tests. The separate wildcard draft preserves current M017 and numeric fallback while adding an unexecuted stock float-literal control. Its208 copied cases/349 originals are draft inputs, not MAIN acceptance. The inspected reference-operand case exposes later numeric-use and reference-operator work; no execution pass is claimed.

Evidence: [accepted root milestone review](artifacts/macos-first/m017-unicode-declaration-world-v8/integrated-main-milestone-v1/root-terminal-review-v1.json). Earlier sections below are historical checkpoints.

# Current status — M017 components integrated; MAIN milestone pending

Root reviewed the stopped subagent handoffs and integrated the tested M017 partial foundation on macOS ARM64/Rust1.99.0. Scoped evidence covers **575 Rust test bodies in10 unfiltered binaries and3 Python importer controls**, with all60 registered M017 controls observed once. The successful37-body V7 prefix is retained from a runner that later failed five Unicode harness setups; the repaired V8 suffix exited0. Exact raw logs, inventories, source ownership, unchanged-prefix inputs and upstream provenance were independently checked. Prior failures remain preserved; no single all-green combined runner is claimed.

The integration adds owned dyn/opaque terms, source-header arrays, reference elision, canonical Unicode lifetime identity, immutable frame tables and transaction-region admission guards. Root repaired two handoff adapters and a real normal-stack overflow in recursive header lowering. Deep admitted120-layer binder structure and excessive130-layer typed refusal passed without raising stack settings or changing the pinned parser/quotas. Tested production, assertions and original bytes were copied with fresh mtimes.

MAIN now retains **206 official rustc cases/347 unchanged original files** with both original licenses. Full stock fixture/compiletest passes remain **0**. These are component controls, not complete original body or diagnostic acceptance. M017/TR001/TR002, T025/T031, general numeric source inference, traits, patterns, user Drop, printing and worker isolation remain incomplete. The parser is pinned rust-analyzer; VM/LRIR and query integration are project code. Guest JIT remains deferred until interpreter convergence. All subagents are stopped; root owns implementation and background tests.

The required fresh integrated MAIN milestone is pending. The last accepted earlier-source macOS milestone remains **2,421 project bodies/181 executable summaries**. Release and Linux acceptance remain unestablished.

Evidence: [scoped collection](artifacts/macos-first/m017-unicode-declaration-world-v8/scoped-validation-v1/independent-collection-v1.json) and [integration receipt](artifacts/macos-first/m017-unicode-declaration-world-v8/integration-v1.json). Earlier sections below are historical checkpoints.

# Current status — macOS milestone accepted; interpreter convergence continues

The repaired MAIN milestone passed on macOS ARM64/Rust1.99.0: **2,421 project test bodies across181 executable summaries**, zero failed, ignored, measured or filtered. Actual root session22217, gate and runner exited0 after2,140,237ms on unchanged source8068a263. Fresh gates00–03 passed with one new build identity. All17 existing M017 controls, both numeric-discard controls and four new checkout-ownership controls ran once. Direct terminal, independent raw-report/inventory/source review and all preserved failure receipts were checked before root release. These counts are project tests, not full stock rustc fixture passes.

A previous attempted MAIN validation ran cached binaries anchored in candidate-source-v5, observed2,060 passes/7 procedural-macro preparation failures and wrote that candidate's receipts. It is preserved as a failed attempt, not MAIN acceptance. The helper now pins its tool target to its launch directory; validation checks explicit canonical checkout ownership. The successful replacement used a fresh MAIN-owned target. Host debug information was disabled; optimization stayed0 and assertions/unwind semantics were retained. No timeout cause or performance gain is claimed.

MAIN retains189 official rustc cases/316 unchanged original files and both licenses. Full stock compiletest/fixture passes remain **0**. The interpreter supports a limited primitive subset, reached calls and bounded ordinary panic cleanup. Rust-compatible numeric fallback includes closed literal islands and discarded expressions; general numeric source inference, traits, patterns, user Drop, printing and worker isolation remain incomplete. T025/T031 and M017 remain in progress. The pinned parser is rust-analyzer; VM/LRIR and query integration are project code. Guest JIT stays deferred until interpreter convergence.

Subagents are stopped. The separate frozen M017 successor adds owned dyn/opaque terms, source-header arrays/elision/NFC lifetime handling and checked immutable frame tables, with206 copied cases/347 unchanged originals. Its60 registered M017 controls remain semantically unexecuted and it is unintegrated. Wildcard initialization work also remains unexecuted/unintegrated. Release and Linux acceptance are unestablished.

Evidence: [current root milestone review](artifacts/macos-first/m013-prepared-tool-target-coherence-v1/owned-main-milestone-v2/root-terminal-review-v1.json). Earlier sections below are historical checkpoints.

# Current status — assignment macOS milestone accepted; interpreter work continues

T044 scoped integration now includes source-selected primitive local compound assignment, checked arithmetic before replacement, and a separate arithmetic result/Unit assignment result. The unchanged official assign-expr-unit-type.rs program executes both helpers and all12 original assertions through a fresh LRR CLI. Its3 compound-kind entries and2 calls are checked. Explicit native references compiled and ran the unchanged source in original2015 and adapted2024 editions. Primitive selection occurs before operands, with RHS before local LHS read; generic/overloaded/projected/reference-RHS source integration remains pending.

Bounded intrinsic tuple/array local replacement reserves its full transfer batch, commits the new value at the original binding before old cleanup, preserves its location epoch, and retains detached old obligations across suspension/cancellation/partial moves. The four unchanged dynamic-drop source-derived recipe controls use disclosed prepared String arrays and the original's owning2021 edition through explicit source preparation. They do not execute its allocator, coroutine, pattern or userDrop behavior. Assignment now validates the exact local owner before fuel/resource effects.

The frozen V19 candidate passed all34 planned commands:478/478 library bodies,37 T044 controls,73 T042 and41 T043 controls,17 affected integration binaries, strict Clippy, ledger/import/integrity/architecture audits, and the fresh CLI/native fixture checks. All eight H002 aliases are covered; the separate2,000-frame control was not selected in this candidate run. Actual runner82322/exit0 and collector60256/exit0, all68 command logs, artifacts and unchanged source were collected before integration. Failed V15–V18 snapshots/diagnostics remain preserved; no test was skipped or an error treated as success.

The fresh integrated MAIN macOS milestone passed 2,378 project test bodies across 179 executables, with zero failed, ignored or filtered bodies. All 17 commands exited0, including gates00–03, strict Clippy, formatting, architecture, upstream integrity and a fresh CLI. All73 T042,41 T043 and37 T044 controls, eight H002 aliases and the2,000-frame control passed once. Seven unchanged stock-program CLI probes passed and repeat existing observations. Root independently verified all34 distinct command logs, terminal/release receipts, gate and CLI identity, and unchanged tested source. The earlier failed macro-timeout run remains preserved.

MAIN retains169 official cases/289 unchanged original files with both upstream licenses. Stock whole-program observations total14 programs/66 original assertions across tested snapshots; full rustc compiletest passes remain0. UserDrop, general traits/patterns, full T044 and release/native/Linux acceptance remain unfinished. The private T045 panic cleanup and later-argument hint drafts remain unaccepted; asynchronous validation is separate. Guest JIT is deferred until interpreter convergence.

Evidence: [integration receipt](artifacts/macos-first/t044-assignment-integration-v19/integration.json) and [independent validation collection](artifacts/macos-first/t044-owner-admission-successor-v19/validation-v1/final-collection-v1/final-collection-v1.json). Current milestone: [root evidence review](artifacts/macos-first/t044-assignment-integration-v19/current-main-milestone-v1/root-independent-review-v1.json). Earlier headings below are historical checkpoints.

# Current status — observed construction, discard and exact return admission integrated; macOS milestone pending

T044 component integration now includes observed tuple/array construction without invented types, bounded structural Discard, caller-result reservation before return cleanup, and the exact Call-exit continuation repair. Macro-generated argument calls can return to their verified temporary slot. A different canonical resume or result refuses before effects; source/type/initialization checks remain. The previously failing macro predicate integration now returns its required typed ownership fault. User Drop, replacement and panic/unwind remain incomplete.

The frozen candidate passed473/473 library bodies, all17 selected call/scope/source integration binaries, all32 T044 controls,73 T042 and41 T043 controls once, all eight H002 aliases and the2,000-frame test. All27 distinct planned commands exited0 across two segments. The first wrapper exited93 solely because it searched for the separate long-frame event inside selected fixture events; its failure is preserved. Independent collection reconciled the actual raw logs; only the six untouched importer/import/architecture suffix commands followed. No single all-green27-command runner is claimed. Root independently checked all54 command log hashes, compiled artifact hashes and unchanged frozen source/release receipts before integration.

The fresh integrated MAIN macOS milestone is pending. The last accepted prior-source milestone remains2362 project test bodies. MAIN retains167 official rustc cases/287 unchanged original files with pinned upstream provenance and both original licenses. Stock observations remain13 programs/54 original assertions across tested snapshots; full rustc compiletest passes remain0. The separately corrected local replacement draft has four unexecuted stock-derived controls and is excluded from this integration. Full traits, user Drop, guest JIT, native/release and Linux acceptance remain unfinished.

Evidence: [integration receipt](artifacts/macos-first/t044-call-continuation-integration-v13/integration.json) and artifacts/macos-first/t044-call-continuation-draft-v13/validation-v1/combined-collection-v1.json. Earlier headings below are historical checkpoints.

# Current status — bounded return and constructor cleanup milestone accepted

T044 remains in progress. Return cleanup retains the returned owner and suspended caller until bounded scope cleanup completes. Partial constructor guards drain completed fields in reverse completion order and preserve exact child ticket/location acknowledgements across dispatch and interruption. Source-selected user Drop and general trait integration remain pending.

The fresh integrated macOS milestone passed 2,362 project test bodies across 179 executables with zero failed, ignored or filtered bodies. All 17 distinct commands completed across two segments, including formatting, strict Clippy, architecture, gates 00–03, upstream integrity, a fresh CLI and seven unchanged stock-program probes. All 73 T042, 41 T043 and 21 T044 controls passed once; eight H002 aliases and the 2,000-frame control passed once. Root independently verified 34 command log hashes, terminal/release receipts, CLI identity and unchanged source membership before updating this documentation.

The first runner exited 1 solely on a stale wrapper predicate expecting seven T044 events; gate03 itself exited 0 with all 21. Its failure is preserved. Only the nine unexecuted suffix commands ran afterward, without repeating the gate. A failed collector-only attempt is also preserved; corrected read-only collection passed. No single 17-check runner exit 0 is claimed.

MAIN retains 164 official cases / 283 unchanged original files. Recorded stock-program observations remain 13 / 54 original assertions across tested snapshots; today's seven probes repeat existing observations and add no unique passes. Full rustc compiletest passes remain 0. The separate V7 candidate failed four library controls (465 passed / 4 failed); its formatting and Clippy passed, while dependent checks stayed unexecuted. The failures are retained. V9 fixed those setups and passed 471 of 472 library bodies; one unit-pair control assumed a single-step scope scan and failed. V11 passed that repaired control, then failed macro predicate integration (six passed/one failed): return preflight rejected its verified temporary destination with LRR-E-INTERNAL-IR instead of the expected typed LRR-E-OWNERSHIP. Its 22 untouched suffix checks remain unexecuted and the failure is preserved. V13 fixes the exact Call-exit continuation binding and is separately frozen for fresh full-library and integration validation; it is not integrated. V12 was never tested because independent preflight caught its missing fixture registration; V13 corrects only that metadata relative to V12. Bounded structural Discard and the mutable V10 local-replacement draft remain excluded from MAIN. User Drop, replacement, panic/unwind, general traits, guest JIT, release and Linux acceptance remain incomplete.

Evidence: [independent milestone collection](artifacts/macos-first/t044-return-guard-integration-v5/validation-v1/milestone-collection-v1.json), with terminal and source release sidecars.

# Current status — integrated macOS ScopeLeave milestone accepted

The fresh macOS milestone passed 2,348 project test bodies across 179 executables, with zero failures, ignored or filtered bodies. All 17 checks completed: formatting, strict Clippy, architecture, gates 00–03, upstream integrity, a freshly built CLI and seven stock-program probes. All 73 T042, 41 T043 and seven T044 controls passed once. Eight H002 aliases are included in T043; the 2,000-frame regression passed once. Independent collection verified unchanged tested source and released its hold.

The initial Python3.9/tomllib gate failure remains preserved. The corrected continuation retained seven earlier successful checks and reran the failed gate/unexecuted tail under Python3.14. A stale raw report.unexecuted_checks field is preserved and explicitly reconciled against terminal/status and the actual17 successful command rows.

T044 remains in progress. MAIN retains160 official cases /279 unchanged upstream files. Recorded stock-program observations remain13 /54 original assertions across tested snapshots; full rustc compiletest passes remain zero. Return and constructor cleanup candidates remain separately frozen and unvalidated. Selected user Drop, general traits/patterns, guest JIT, release and Linux acceptance remain unfinished.

Evidence: artifacts/macos-first/t044-scope-cleanup-integration-v4/current-main-milestone-v1-python314-continuation/independent-collection-v1.json. Earlier entries below are historical checkpoints.

# Current status — bounded ScopeLeave integrated; fresh macOS milestone pending

T044 is in progress. Bounded ScopeLeave cleanup retains exact owning child
continuations, processes committed fields in declaration order, skips moved fields,
and resumes the same recipe without repeating opcode entry charges. Seven
stock-derived controls passed. Intrinsic adaptations do not execute source-selected
user Drop; unresolved nominal cleanup refuses until its selected witness is available.

Scoped validation passed formatting, strict Clippy, corrected ledger, all 17
integration binaries (177 tests), importer and architecture checks. Library coverage
is split: the initial run had 447 passing tests and one filesystem permission failure;
that exact test passed after writable artifact outputs were restored. No single
all-green 448-test invocation is claimed. All 73 T042, 41 T043 and seven T044 controls,
the separate H002 control and 2,000-frame check passed once in their accepted phases.
The fresh integrated MAIN milestone is pending; the last accepted milestone has 2,341
project test bodies. Failure receipts remain preserved.

MAIN retains 160 official cases / 279 unchanged upstream files and both licenses.
Stock-program observations remain 13 / 54 original assertions across prior snapshots;
full rustc compiletest fixture passes remain zero. Return cleanup and partial guard
successors are separate and unvalidated. Selected user Drop, general traits/patterns,
guest JIT, release and Linux acceptance remain unfinished.

See [component evidence](docs/t044-cleanup-components.md).

# Current status — T043 component accepted; bounded cleanup in development

The fresh macOS milestone passed 2,341 project test bodies across 179 executables,
with zero failures, ignored or filtered bodies. All 17 checks completed, including
formatting, strict Clippy, architecture, fresh gates 00–03, upstream integrity and
seven unchanged-source CLI probes. All 73 T042 and 41 T043 controls, the separate
H002 control and the 2,000-frame bounded guest-stack event passed once.
Independent collection verified the tested source stayed unchanged. A collector-only
schema error is retained; its correction reran collection without rerunning tests.

T043 has provisional component acceptance under Section 57/E-57-001, with explicit
later integration dependencies and pending full fixtures. Release completion is
false. General patterns, constructors, source/type/Drop integration, production
guest JIT and Linux acceptance remain unfinished. T044 cleanup work is private,
untested and unintegrated.

MAIN retains 157 official cases and 276 unchanged upstream files with both licenses.
The 13 recorded stock-program observations and 54 original assertions span tested
snapshots; repeated probes add no unique passes. Full rustc compiletest passes: 0.

# Current status — stock cast and borrow contexts integrated; macOS milestone pending

T043 now resolves primitive cast targets through the existing lexical annotation
guard and propagates an explicit borrowed target context to computed operands.
Hoisted type declarations (including traits) refuse primitive admission before
operand preparation; value-only function names do not shadow the type namespace.
The owners, indexed lifetimes, fuel/cancellation admission and stable encodings
remain explicit. The original temporary-frontier cases retain distinct call sites.

The exact frozen V15 passed441 library and134 integration tests,33T043 controls,
H002 and2000-frame events once, formatting, strict Clippy, ledger and architecture.
Its independent collector and source release passed. V12/V14 failures and V13's
segmented collector repair remain retained. Eight existing scope controls now
appear in T043 traceability; their current MAIN milestone is pending.

MAIN retains157 official cases/276 unchanged files and both original licenses.
Recorded stock observations remain13 with54 original assertions across snapshots;
full rustc compiletest passes0. The preceding accepted macOS milestone has2329
project bodies. This integration requires a fresh milestone. T043 in_progress;
general patterns, constructors, source/type/Drop, guest JIT, release and Linux pending.

# Current status — reference milestone accepted on macOS

The integrated reference components passed the current macOS milestone with
2,329 project test bodies across179 executables,0 failed/ignored. All73 T042 and21
T043 controls,updatedH002 and the2000-frame bounded-stack event passed once. Seven
unchanged-source CLI program observations completed on the freshly built immutable
CLI; these repeat existing coverage and do not increase unique rustc pass counts.

This is segmented acceptance: the initial runner exited1 on Python tomllib
environment failures. Its first seven successful checks and all failure receipts
remain preserved. With Python3.14.8 selected explicitly, the failed gate and nine
unexecuted suffix checks completed; the corrected suffix runner and independent
collector both exited0 on the unchanged frozen source.

MAIN retains153 official cases/270 unchanged original files with both licenses,
13 recorded program observations/54 original assertions across snapshots, and
0 full rustc compiletest fixture passes. T043 remains in_progress. General patterns,
source/type/Drop integration, production guest JIT, release and Linux are unfinished.
Private cast/borrow-context/original-frontier successors remain unvalidated.

[Collected milestone](artifacts/macos-first/t043-reference-components-integration-v9/current-main-reference-milestone-v2/suffix-independent-collection.json).

# Current status — reference components integrated; macOS milestone pending

T043 now supports initialized simple ref/ref mut let bindings. A local/projected/
dereferenced initializer preserves its original checked place; a computed initializer
keeps its one owning temporary in the indexed extending scope. Only the reference
moves into the immutable local binding. Ordinary-call barriers require revalidated
source identity, and unknown constructor frontiers refuse before storage publication.
Intrinsic local-path dereference assertions use checked LRIR and statement-owned
value/place pairs, preserving the reference owner.

The exact frozen candidate independently passed all six scoped commands:431 library
tests,126 integration tests,all21 T043 controls and updated H002 once,exact ledger and
the2000-frame bounded-stack event once. The unchanged stock let-destruct-ref.rs whole
program completed its original one assertion under the explicit Rust2024 profile.
MAIN now retains153 official cases/270 unchanged original files with both licenses;
13 recorded interpreter program observations/54 original assertions across snapshots,
and0 full rustc compiletest fixture passes. Repeated runs are not extra unique passes.

A fresh integrated macOS milestone is pending. The prior accepted MAIN milestone
has2312 project test bodies across179 executables. V5 and V7 failed snapshots and
the V6 wrapper parser anomaly remain preserved. T043 stays in_progress; full patterns,
constructor/Fn-value execution, source/type/Drop integration, production guest JIT,
release and Linux remain incomplete.

# Current status — integrated macOS milestone accepted

The integrated T043 computed-temporary-borrow source passed the fresh macOS
milestone:2312 project test bodies across179 executables,0 failed/ignored, all
16 commands exit0,73 T042 controls and4 T043 controls each once, including the
unchanged2000-frame/256KiB guest-stack regression. Independent raw collection
confirmed the source freeze, gates00–03 and exact freshly built CLI identity.
Six unchanged official programs completed through the immutable CLI copy;
four arithmetic originals exercised29 original assertions. These repeat existing
observations; MAIN remains150 official cases/263 unchanged original files,
12 recorded interpreter program observations/53 original assertions under the
explicit Rust2024 adaptation, and0 full rustc compiletest fixture passes.

T043 remains in progress. A separate V5 candidate failed five selected-call
controls on its unresolved stock std::ops::Drop import (416 library bodies passed,
5 failed); it is not integrated. V6 supplies a stock-derived test-only declaration
facet through normal import resolution and adds a suspension control; validation
is pending. The production source retains the accepted MAIN milestone. Positive constructor/Fn-value execution, full
patterns, source/type/Drop integration, production guest JIT, release and Linux
remain incomplete. The runner's stale unexecuted-check list is retained as a
metadata anomaly; independent logs/status/terminal prove all16 commands ran.

[Collected milestone](artifacts/macos-first/t031-t034-t040-t041-t042-t043-current-main-milestone-v1/independent-collection.json).

## Earlier source snapshot

# Current status — computed temporary borrows integrated

T043 now evaluates computed borrow operands once, retains their indexed owning
cell and forms a checked reference after completion. The stock literal-borrow
initializer passes scope-retention, fuel and cancellation controls; all67 source
execution regressions, strict Clippy, formatting and architecture passed privately.
The official late-initialization drop-order test also exposed a spec contradiction:
E-T043-001 corrects local cleanup to reverse declaration order, with native and
scoped LRR evidence. A fresh integrated MAIN macOS milestone is pending.

T042 retains component acceptance with73 ownership controls; its exact post-ledger
test passed. The preceding macOS gate passed2308 project bodies. That is historical
for the new T043 source, not a current integrated-source pass.

MAIN retains150 official cases/263 unchanged original files with both licenses.
Twelve earlier/current program observations exercise53 original assertions under
the explicit Rust2024 adaptation; full rustc compiletest passes remain0. Newly
imported scope/method cases are component-only or unexecuted. T043 remains
in_progress; conditional constructor extensions, extending patterns, full cleanup,
source/type and Drop integration, production guest JIT, release and Linux are
incomplete.

[Temporary borrow component]( docs/t043-temporary-borrow-components.md).

## Earlier snapshots

# Current status — ownership component accepted; temporary scopes next

T042 has component acceptance under Section57/E-57-001; release_complete is false.
The fresh macOS gate passed2308 project test bodies with no failures or ignored
tests, including all73 ownership controls. Gates00–03, strict Clippy, formatting
and architecture passed. Exact source inputs stayed unchanged.

A separately verified fresh executable completed six stock program probes,
including all29 assertions in the four arithmetic originals. The initial helper's
CLI hash remains unknown; its different executable and the suffix driver's
post-child terminal-writer failure are preserved as qualified segmented evidence.

MAIN retains145 official cases/252 unchanged upstream files. Twelve programs
have interpreter execution observations with53 original assertions under the
Rust2024 adaptation; full rustc compiletest passes remain0. Nominal Copy/Drop,
complete cleanup, symbolic source/type integration, production guest JIT, full
release/performance/security acceptance and Linux remain incomplete.

T043 temporary-scope work is private and unintegrated. Its stock drop-order
regression and native reference passed; new computed-borrow validation is pending.

[Ownership acceptance](docs/t042-component-acceptance.md).

## Earlier snapshots

# Current status — resumable structural Copy integrated

T042 now copies closed Copy tuples/arrays through private iterative destination
guards. Source owners remain live; each destination has distinct field storage.
Dispatch chunks bound field and guard transitions, charge opcode entry once and
poll cancellation. Fuel/quota/stale-source refusal leaves partial results
unpublished. Whole non-Copy structural moves, checked projections, field
assignment and cached partial initialization remain integrated.

The exact Copy V3 candidate passed538 scoped project test bodies and all73 T042
controls, strict Clippy, formatting and architecture. Those counts overlap;
73 controls are included in538. Earlier runner/lint failures are retained with
independent collections. The preceding MAIN milestone passed2295 project bodies
and gates00–03; its final suffix was collected separately. A fresh combined
MAIN milestone is pending after this integration.

MAIN retains145 official rustc cases/252 unchanged original files with both
licenses. Twelve unchanged-source programs have completed interpreter execution
under the explicit Rust2024 adaptation, exercising53 original assertions; these
include earlier tested builds. The newly imported fixed_length_copy.rs and
tup.rs both fail direct CLI execution on missing type context/alias resolution.
Their selected Copy operations have component evidence, not full-program passes.
Full rustc compiletest fixture passes remain0.

T042 remains in_progress. Source-selected nominal Copy/Drop, complete cleanup and
source/type integration are pending. Production guest JIT, full release, security
and performance acceptance and Linux validation remain incomplete.

[Integration evidence](artifacts/macos-first/t042-aggregate-copy-components-integration-v3/integration.json).

## Earlier snapshots

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

[Integration evidence](artifacts/macos-first/t042-field-structural-components-integration-v1/integration.json).

## Earlier snapshots

# Current status — cached aggregate initialization integrated

T042 aggregates now retain accounted field bitsets and cached completeness.
Field moves/reinitialization update owning ancestors; whole ordinary transfers
reject incomplete values. Cleanup emits one checked field/skip/completion step
and preserves external owners of moved fields. Local assignment reserves its full
batch before RHS transfer. Scoped validation passed503 tests/all38 controls,
formatting, strict Clippy and architecture on unchanged initialization V1 source.
Two external runner errors are retained; all actual commands passed without repeats.
The fresh combined MAIN macOS milestone is pending; its predecessor passed2256
project tests and all11 checks.

MAIN retains140 official cases/247 unchanged originals and both rustc licenses.
Unchanged-source execution observations remain11 programs/53 original assertions
under explicit Rust2024 adaptation; full compiletest fixture acceptance remains0.
General aggregate Copy, source-selected nominal Drop, guest projected moved-field
reinitialization, production JIT, release and Linux remain incomplete. T042 stays
in_progress. A separate original Unit program candidate is unvalidated/unintegrated.

[Initialization integration](artifacts/macos-first/t042-initialization-components-integration-v1/integration.json).

## Earlier snapshots

# Current status — assignment ownership components integrated

T042 local assignment now checks destination/RHS aliases and the old-value cleanup
obligation, then reserves the whole transfer/result batch before consuming RHS.
Deferred and moved locals can initialize without reviving old reference epochs.
The exact private candidate passed496 scoped tests/all31 ownership controls,
formatting, strict Clippy and architecture, using independently collected segments.
The first external matcher failure is retained. A fresh MAIN milestone is pending;
the preceding collected macOS milestone passed2256 project tests and all11 checks.

MAIN retains140 official rustc cases/247 unchanged original files and both licenses.
Eleven unchanged-source program observations account for53 original assertions
under explicit Rust2024 execution adaptation. Full rustc compiletest acceptance
remains0. Cached field summaries are a separate draft; general aggregate Copy,
projected reinitialization, guest Drop/JIT, release and Linux remain incomplete.

[Assignment integration](artifacts/macos-first/t042-assignment-components-integration-v3/integration.json).

## Earlier snapshots

# Current status — ownership components integrated

T042 owning-read and tuple field-move components are integrated after scoped
validation:486 project tests, all21 controls, formatting, strict Clippy and the
architecture audit passed on unchanged V7 source. String/mutable-reference reads
transfer one owner; intrinsic scalar reads copy; tuple extraction preserves its
other fields. Closed primitive-reference arguments now initialize callee cells.
General aggregate Copy, nominal Drop policy, assignment/reinitialization and full
T042 integration remain incomplete. A fresh MAIN macOS milestone is pending.

MAIN retains137 official rustc cases/239 unchanged original files. Eleven unchanged
program execution observations now account for53 original assertions, with an
explicit Rust2024 adaptation: ten arithmetic programs plus move-arg.rs. The new
move-arg observation used the tested private CLI; a fresh MAIN observation follows.
Full rustc compiletest acceptance remains0. T041 has component acceptance; the last
collected MAIN milestone passed2235 project tests before this new integration.
Guest JIT, full trait/Drop execution, release and Linux remain incomplete.

[Ownership integration](artifacts/macos-first/t042-owning-partial-components-integration-v7/integration.json).

## Earlier snapshots

# Current status — reference milestone collected

The latest macOS ARM64/Rust 1.99.0 milestone passed all ten checks, fresh gates
00–03 and 2,235 project host/doc tests. All 62 T041 reference controls passed.
T041 now has component acceptance with explicit pending full-integration fixtures;
T042 ownership implementation is now in progress in Section 57 order.

Ten unchanged official arithmetic programs have completed interpreter execution
under an explicit Rust2024 edition adaptation, running all 52 original assertions.
Four were checked with the fresh MAIN CLI; six were previously observed on the
collected scalar-assertion binary before their unchanged originals were integrated.
MAIN retains 127 official cases / 221 original files. Full rustc compiletest
acceptance remains zero. General traits/inference, complete Copy/move/Drop,
guest JIT, release and Linux acceptance remain incomplete.

[Collected milestone](artifacts/macos-first/t031-t034-t040-t041-combined-main-milestone-v1/independent-collection.json) ·
[Reference component acceptance](docs/t041-component-acceptance.md).

## Earlier snapshots

# Current status — stock arithmetic observations and reference work

Ten unchanged official arithmetic programs have completed interpreter execution
under an explicit Rust2024 edition adaptation, executing all52 original assertions.
Four are covered by the fresh MAIN CLI control; six additional programs were
observed on the previously collected scalar-assertion binary, then their exact
original files were integrated. MAIN retains127 official cases/221 original files.
Full rustc compiletest acceptance remains0.

The latest collected macOS milestone passed all10 checks and2228 project host/doc
tests, with0 failed/ignored, including95 T034,56 T040 and55 T041 registered controls.
It predates the whole-array-reference implementation and six new imports; fresh
combined validation is pending. T041 remains in progress with62 registered controls.
The integrated array-reference change passed10 scoped checks, full library337,
CLI27, LRIR14, VM20 and source_run67, including the unchanged2000-frame/256KiB
regression. Seven controls check source type/length, ownership, stale leases,
fuel refusal and complete source-memory accounting. General source/trait inference,
ownership/Drop, guest JIT, release and Linux remain incomplete.

[Collected MAIN milestone](artifacts/macos-first/t041-t034-integrated-main-milestone-v1/independent-collection.json) ·
[Additional stock observations](artifacts/macos-first/t031-stock-div-float-u32-v1-scoped-observations/index.json) ·
[Six-original integration](artifacts/macos-first/t031-stock-six-additional-originals-integration-v1/integration.json).

## Earlier snapshots

# Current status — source-bound fields and temporary references integrated

The interpreter now has tested components for named fields, temporary owning places, computed references and nominal dereferences. Frozen v8 passed 482 host test bodies, including all 56 T040 and 55 T041 controls and the unchanged 2000-frame/256KiB stack regression. Both outer validation wrappers failed after successful childcommands because of collection-code defects; independent collectors reconciled those outcomes without rerunning. Full T041 and a fresh MAIN milestone remain pending. [Integration receipt](artifacts/macos-first/t041-fields-temporaries-components-integration-v8/integration.json).

MAIN now retains 116 official rustc cases / 210 unchanged files; full stock fixture passes remain zero. The unchanged arithmetic programs previously failed at assertion operand integration. A separate scalar assertion implementation is frozen and awaiting validation. General trait/type inference, Drop, guest JIT, release and Linux acceptance remain incomplete. The last full MAIN run failed on AddImpl preparation becoming stale; its isolated pass does not resolve that failure under the full run.

## Earlier snapshots

# Current status — T041 in progress

The latest integrated macOS milestone failed during the original AddImpl procedural-macro test with LRR-E-TOOL-PREPARE-STALE. The same test subsequently passed in isolation; that does not resolve the failure under the full run. The previously integrated stack repair passed its scoped unchanged 256KiB/2000-frame regression. The earlier stack-abort milestone remains historical failure evidence.

MAIN retains 111 official rustc cases / 204 unchanged files, with zero full fixture passes. Unchanged i32-sub.rs fails at assert_eq! operand integration; an independently imported borrowck-fixed-length-vecs.rs fails at missing array element context. Four further numeric programs remain unexecuted. The latest private reference candidate v7 passed 327 of 330 library tests with three failures; corrected v8 validation is running separately. No private successor is integrated on this evidence. Guest JIT, broad trait/type inference, release and Linux acceptance remain unfinished. [Retained observations](artifacts/macos-first/t041-current-observations-20261011/index.json).

## Previous snapshots

The fixed-stack lowering repair is integrated after12 independently collected scopedchecks/448 actualhost bodies, including fullsource_run67 and all77 construction/reference controls exactly once. The original256KiB/2000-frame regression passes; its earlier failures are retained. A fresh MAIN macOS milestone is pending. [Integration](artifacts/macos-first/t041-stack-repair-components-integration-v3.json).

The latest macOS milestone failed on a stack overflow in the unchanged256KiB/2000-frame guest-recursion check. Earlier binaries reported2000 passing bodies and source_run emitted62 additional pass events before abort; all77 current construction/reference controls passed exactly once. The aborted and unobserved tests, interpreter suffix and whole milestone remain unaccepted. [Collected failure](artifacts/macos-first/t041-projections-components-main-milestone-v3-retained-failure/host-outcome-supplement.json).

MAIN retains111 official rustc cases/204 unchanged files; full stock fixture passes0. T041 remains in progress. A bounded-stack repair and field/temporary-receiver successors are frozen, unvalidated drafts. The earlier2180-test successful milestone is historical.

# LRR (development)

LRR implements the fixed architecture in the
[revision 5 specification](lrr_rust2024_macros_traits_implementation_spec.md),
targeting LRR 0.2.0 / `lrr-2024-v2`. It is under construction and is not a complete
Rust runtime or release.

Parsing reuses the pinned rust-analyzer parser and AST. LRR's declaration adapter,
lazy LRIR preparation, interpreter, runtime and incremental query database are
custom implementations. Revision 5 selects Cranelift and LLVM native tiers; guest-code
region recording/lowering, optimization and interpreter/JIT transitions remain unimplemented.

The interpreter executes a limited primitive Rust subset: numeric operations,
blocks, bindings, assignment, conditionals, loops, functions and recursion, with
lazy called-body preparation and a minimal test/assertion harness. Captured-source
namespaces/imports, immutable snapshots, dependency tracking and bounded caches
have component implementations and tests.

Macro/source infrastructure includes the pinned adapter, token/hygiene identities,
item-position declarative expansion, generated imports/functions/macros, explicit
include/environment capture, procedural request/session components and real matched
server controls, epochs/revalidation, and generated physical-module adoption.
Original, captured physical-module and nested generated function-like procedural
discovery have tested MacroRun session ownership, shared budgets and final input
verification before seal publication. Generated procedural outputs settle nested
declarative calls and imports; actual late conflicting imports refuse sealing.
Original replacing attribute discovery has actual-server controls for generated
calls/imports, removal of original declarations and later annotations, cfg_attr
preprocessing and conflicting-export refusal. Generated attribute reanalysis has
actual-server controls for chained replacements, attributes emitted by function
macros and cfg_attr.
Generated derives have ordered execution/append and following-attribute controls;
direct-source compiler-only empty derives have discovery/publication controls.
Original-source derives now have controlled authorized execution, ordered append
integration and source completion/publication. The original declaration retains
its canonical identity after known inert/helper processing. Published type queries
invalidate after source edits. Members can use exact helpers introduced by later
groups after demanding their metadata; outer attributes retain introduction
ordering. Following original-source replacing attributes now preserve earlier
derive output and macro bindings; edited source refuses before attribute dispatch.
Original-source helper inheritance now has forwarding and chaining controls with
sibling/function-like isolation. Full legacy/helper diagnostics and
service/resource/world/edition matrices remain pending.
M013 discovery/worklist component acceptance is complete under E-57-001. Full
MX027/MX028 remain pending; M014 declaration scan/inventory component acceptance is complete under E-57-001; full producer and seal integration remains pending. Ordinary modules inside adopted sources have
capture, staging, completion and atomic publication, with CLI execution tested for
ordinary nested functions, physical paths and runtime assertions. Ordinary modules
also have tested parent/local macro selection, includes, macro-use body ownership
and exports. An M015 generated test catalog candidate now enumerates final names
and attributes after an actual TestCatalogSeal. Its focused macOS checks pass;
the integrated milestone and full MX031/MX032 acceptance remain pending.

The production observed-region JIT, demand-driven trait solver, remaining language
features, permission broker/sandbox, Cargo-native runtime ABI, service integration,
full fixture matrix, performance/security acceptance and reproducible release
artifacts remain unfinished. Cranelift and Cargo/procedural preparation probes
are component evidence, not proof of a complete JIT or native runtime.

See the [implementation state](docs/implementation-state.md),
[goal](docs/implementation-goal.md), and [task ledger](docs/tasks.json) for current
verified evidence and pending work. Component-complete cards and provisional
gates do not imply full revision-4 release acceptance.

A pinned selection of official rustc UI tests is imported with helpers, directives
and licenses. Five native reference modes pass on macOS; LRR reuse is pending.
See [the upstream reuse assessment](docs/rustc-upstream-tests.md).
The local official checkout at `rustc/rust` is the primary source for new language
fixtures. Copied tests retain rustc's original licenses, helpers and directives;
LRR adaptations remain outside the licensed import directory. At the earlier local-borrow checkpoint, the inventory contained
110 cases and 202 unchanged original files from the supplied checkout. Its24 scoped
local-borrow/reference annotation passes and pending-milestone statement are historical. The preceding collected macOS milestone
passed2171 project host/doc tests, with no failed or ignored tests, including all56
T040 component controls; it is historical for the new source. T040 is component-complete
under E-57-001; full constructor integration remains pending with later tasks.
Full stock fixture passes remain 0. The exact post-transition ledger test passed; its external filtered-count reporting
error remains recorded separately. One unchanged empty-derive case passes a real CLI
interpreter run and explicit native metadata reference, recorded separately in the
[current macOS checkpoint](artifacts/macos-first/t040-constructor-source-context-components-main-milestone-v1/independent-collection.json).
The original printing derive auxiliary exposes a recorded
[server/protocol gap](docs/errata/m013-procedural-stdout.md); the installed-server
execution remains failed. A separately prepared matched-source server now completes
all five unchanged printing derives. The unchanged environment auxiliary also passes
three recorded-environment/fallback controls. Typed preparation/MacroRun adoption now passes. The unchanged original source
also completes all five native derives, settles its worklist and publishes Foo.
Its macro_rules-generated registrations use one demand-driven declaration expansion
and captured 2015 definition/2024 call sites. Full guest/tier and CLI oracles remain
pending; terminal refusal is separate evidence.
Full compiletest, LRR tier and JIT case acceptance remains pending;
component assertions do not imply end-to-end Rust compatibility.

Implement and test with Rust 1.99.0 on macOS first (current host: ARM64):

```sh
cargo +1.99.0 test --workspace --locked
cargo +1.99.0 run --locked -p xtask -- gate 03
```

Linux x86-64 is a later port. Docker is optional supplementary tooling and does
not gate macOS progress. Linux support requires its own actual acceptance evidence.
Historical bootstrap and Cargo-reuse probes remain preserved in the artifacts
and documentation; they are not release or cross-platform support claims.
