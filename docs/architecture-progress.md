# Architecture progress

Last accepted milestone:2,415 project bodies across181 executables, gates00–03 passed on macOS/Rust1.99.0. MAIN retains189 official cases/316 original files; full stock fixture passes remain0. M017 is in progress and guest JIT is deferred. See [current state](implementation-state.md) for evidence; numerical checkpoints below are historical.

The parser is reused from rust-analyzer. LRR adapts the pinned AST into owned
declaration facts and prepares only reached executable bodies. Parsing Rust
syntax does not imply support for executing every Rust construct.

The custom interpreter executes a limited primitive subset: arithmetic, bindings,
assignment, blocks, branches, loops, calls, recursion and basic assertions. A
direct CLI observation on 2026-10-08 macOS ran a loop and recursive factorial,
returned 42, installed 66 LRIR recipes and completed 333 operations. An unused
body containing unresolved names was not executable-prepared.

Cranelift is reused as the native backend. Current JIT code compiles synthetic
bootstrap functions and verifies explicit symbol registration. It does not compile
guest LRIR. Observed-region recording, lowering, guards and VM/JIT transitions
remain unimplemented.

Macro matching, transcription, syntax bridging and hygiene reuse pinned
rust-analyzer components. Procedural execution uses the matched external server
and prepared macro-host artifacts. LRR implements the owned token/provenance
adapter, explicit authority/session coordination, source discovery, namespace
integration, suspension, invalidation and seal publication. Cargo supplies build
planning/preparation tooling; the complete Cargo-native runtime bridge is pending.

LRR's query/cache database, lazy LRIR, interpreter/runtime, capability integration
and demand-driven trait goal engine are project-specific work. The trait engine,
remaining language operations, production JIT, broker/sandbox, native runtime,
performance/security gates and release are not complete. This implementation does
not reuse rustc type checking or MIR execution.

The fixed revision-4 Section57 sequence places M001–M016 before later type/runtime
and JIT milestones. M013 component acceptance is complete; M014 declaration scanning/inventory is in progress. There are
33 component-accepted task cards, six in progress and 101 unstarted; task counts
do not measure equivalent work or a reliable overall completion percentage.

The last integrated macOS checkpoint has 1798 host/doc test passes, 50 component
assertions across the exercised imported rustc subset; twenty cases are now imported, with the two M014 cases exercised only as syntax/inventory assertions, and zero full cases from
the supplied checkout accepted by LRR. Original-source helper metadata controls
now pass, including permission denial and occurrence-bound receipts. Original
derive ambiguity and private paths now report exact diagnostics before helper
metadata preparation or execution, with publication refused. One unchanged
empty-derive case also passes a whole-source CLI interpreter run and explicit
native metadata reference; this is separately recorded from full compiletest or
JIT acceptance. Ordered original-source occurrence inputs now preserve later
groups and attributes, including repeated paths and member cfg. Executing the
unchanged `multiple-derives.rs` auxiliary exposes E-M013-001: macro stdout corrupts
the matched binary protocol. Its terminal-refusal control passes; that procedural
installed-server fixture remains failed, with zero completed expansions. A separately
prepared matched-source server preserves the API6 protocol on a duplicated
descriptor and routes macro stdout to diagnostics. Its positive control completes
all five unchanged derives, and the unchanged tracked-env auxiliary passes three
environment/fallback cases. Typed preparation/MacroRun adoption now passes with generated source/seal
publication and changed-source refusal. The unchanged original derive-order source now completes all five native derives
through MacroRun and publishes canonical Foo. One pinned declaration expansion
produces the macro-host headers, reused with captured 2015/2024 definition/call
sites. Full guest/tier and CLI oracles remain pending. Adapted source cases now dispatch controlled derives through the authorized
MacroRun and integrate their real output through generated discovery. The ordered
case completes three occurrences across two groups and one nested declarative
expansion. Preceding source-prefix snapshots prevent self/later-output visibility;
forged, duplicate, foreign-run and stale-source append attempts are refused.
Original declarations now restore with canonical source identities after known
inert/helper processing; completion proofs replay before sealing and publication.
A published original type query suspends after its source is edited. Following original-source replacing attributes pass three controls for append preservation, prior macro bindings and stale-source refusal. Original-source forwarding and chaining now preserve exact helper environments with sibling/function-like isolation. Full legacy/helper/service/resource/world/edition matrices remain pending. Original member helpers may now demand metadata from
later groups before dispatch; outer helpers retain introduction-position checks.
The copied helper-name collision case has an adapted real-server control. Full stock-test execution and increasing actual language
coverage remain required; internal test counts are not compatibility counts.

See [the ledger](tasks.json), [current state](implementation-state.md),
[last checkpoint](../artifacts/macos-first/m014-procedural-arguments-checkpoint.json),
[parser](../crates/lrr/src/syntax/parse.rs),
[VM](../crates/lrr/src/vm/step.rs) and
[synthetic JIT bootstrap](../crates/lrr/src/jit/abi.rs).
