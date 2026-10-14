# T043 computed temporary borrows

The integrated lowering executes the operand at its exact indexed child PC, then
forms ValuePlace from its completed owning cell and publishes Borrow in a separate
phase. Forming the address does not copy/move the target. Existing checked source,
descriptor, permission, fuel and resource admission remains authoritative.

The stock first let in `methods/call_method_unknown_referent.rs`, function a,
provides the original literal suffix and source ranges. Its initializer and
BindLocal/statement cleanup execute unchanged; the target survives until the
annotated block ends. Separate fuel and cancellation checks preserve its owner
and epoch without publishing a reference. No remainder of that method-error
fixture or its expected diagnostics is claimed to pass.

Frozen V3 passed all3 new controls plus67 source_run bodies, workspace formatting,
strict LRR Clippy and architecture. The unchanged V2 local-order regression passed
separately; pinned native rustc confirmed reverse declaration order for the
official late-initialization/or-pattern example. E-T043-001 records the exact
contradiction and correction. Full native compiletest/LRR fixture acceptance is0.

T043 is in progress. Conditional tuple-struct/variant extension requires selected
source constructor identity; ordinary calls must not gain extension. Extending
pattern execution, complete Rust2024 if-let/else, block-tail and short-circuit
cleanup still require later pattern, source/type, Drop and facade integration.
No capitalization heuristic, default-width guess, nominal policy erasure or
reference-helper lifetime patch is introduced. Full JIT/release/Linux remain false.

[Scoped collection](../artifacts/macos-first/t043-temporary-borrow-draft-v3/scoped-validation-v1/independent-collection-v2.json).
A fresh current MAIN milestone is pending after integration.

## Private V5 ordinary-call candidate — validation pending

The private successor adds selected source function identity to CallPrepare facts.
The worker publishes a bounded node-indexed lifetime barrier after existing fuel
and cancellation admission, before argument evaluation. Unknown conditional
frontiers refuse at the reached borrow and checked slot publication with
LRR-E-TEMPORARY-SCOPE. Body/site/current-source checks preserve exact identity;
capitalization is never a constructor certificate.

Seven new controls derive call ranges from unchanged rvalue-borrow-scope-error.rs
and temporary-lifetime-extension-tuple-ctor.rs. They exercise ordinary barriers,
reference-pattern frontier metadata, unknown Fn-value admission, uppercase source
function identity, stale contexts, fuel and cancellation. Nominal operand calls,
Fn-value alias execution, whole compile-fail bodies and expected diagnostics are
unexecuted. Positive tuple/variant extension and full T043 remain mandatory.

H002-EXTENSION-8 is explicitly strengthened from unproven default-scope storage
publication to exact unresolved-scope refusal before cell/fuel/handle effects,
retaining producer ownership of the unconsumed ticket. Historical evidence keeps
the earlier control name and assumption. This draft has no validation or
integration claim; MAIN remains the prior integrated V3 while its milestone runs.

## V5 retained failure and V6 import-preparation repair

Actual V5 runner88788/1 stopped after formatting0, strict Clippy0 and library
command101:416 passed/5 failed. The five new selected-call controls all stopped
on the original unresolved std::ops::Drop import before callable selection.
Ledger, eight integrations and architecture were unexecuted, not passes.
Independent failed collection retains exact logs/source and no integration.

V6 keeps every original caller/borrow source byte unchanged. Test-only syntax
facts extract the exact public Drop trait declaration from the copied official
borrowck-describe-field-generic-param-owned-box.rs and place it under a public
ops module in a separately captured SDK declaration view. The wrapper and omitted
outer compiler lang attribute are explicit adaptations outside the upstream tree;
the declaration bytes, original full support source and expected stderr remain
retained with hashes/licenses. Both source views publish together. Existing
NameNamespaces dependency and resolve_imports APIs bind the original import;
the harness requires exact resolved IDs and no pending/missing/denied/ambiguous
result before selecting the ordinary function. No producer is manually removed.

An added control proves unresolved selection remains NeedProducer with no
callee signature/thunk fetch, store/fuel effects or worker barrier publication,
then succeeds after source-facet resolution. The declaration facet supplies no
Drop witness/body execution, runtime SDK or full compile-fail diagnostics.
V6 and its eight controls are pending validation and unintegrated; full T043 and
release requirements remain unchanged.

## Private initialized reference patterns (V7, validation pending)

Simple initialized ref/ref mut let patterns reserve one checked value/place pair
for the reference result, owned by the statement scope. The existing extending
pattern planner owns a computed initializer in the containing block. Literal/rvalue
results use ValuePlace; local/projected/deref places preserve the already evaluated
address without ReadLocal, ReadPlace or ResultTransfer. Borrow checks target source,
descriptor, mutability, budget and fuel before publishing the reference. BindLocal
moves only the reference and keeps the binding immutable even for ref mut. The
referent remains live through statement cleanup and retires at block cleanup.

A written primitive target annotation supplies a Reference hint with the original
let source identity. It is checked even for place initializers. Array, generic and
reference target annotations require pending symbolic integration and are refused
before initializer execution; no runtime descriptor is used to invent source types.
General tuple/record/or/@/uninitialized/let-else patterns remain mandatory later work.

The original two lets in borrowck/cannot-assign-borrowed-ref-in-slice.rs and first two
lets in binding/let-destruct-ref.rs provide acceptance components. The copied sources,
original compile-error stderr, directives, hashes, licenses and upstream commit are
unchanged. The stock error body is not run and its Rust borrow-checker rejection is
not asserted by a relaxed managed-reference runtime component. The original run-pass
case's final assertion is also unexecuted in these controls.

Six tests cover shared/mutable lease identity and cleanup, immutable pointer binding,
original-local place identity without owning reads, one literal evaluation, fuel
refusal after address formation, cancellation before address formation, and bounded
even-value/odd-place deterministic reservations. These are adaptations outside the
original fixture directory. No complete task or fixture pass is claimed.

## V7 failure and V8 source-owned recipe repair

V7 actual72480/1 passed formatting/strict Clippy but five ref controls failed in
pure recipe admission, before any reached borrow recipe could execute: 423 library
bodies passed,5 failed. Remaining commands were unexecuted. The unchanged candidate
and independent collected failure remain retained at t043-ref-pattern-draft-v7.

V8 indexes the direct initializer under its initialized reference statement, keeping
its deterministic source preorder and existing stable operand encoding. Commit
admission requires one direct initializer operand and the statement's reserved
reference value. ValuePlace must match that exact operand's value/place pair, and
Borrow must target its checked place and the reserved reference destination. A
forged unrelated source/result is refused without guest or fuel publication.

The stock let-destruct-ref.rs final assertion contains a primitive read through a
local reference. V8 plans MakePlace/DerefPlace/ReadPlace in that exact macro node's
reserved even-value/odd-place pairs and enters each ordinary operation at its own
charged macro scalar phase. Structural admission excludes other nodes' storage.
Lexical lookup uses the original source invocation and exact local binding; a
parameter or non-local address, a nested dereference or other unsupported operand
continues to refuse. The reference value is neither moved nor copied to observe its
primitive referent. No new opcode, helper backend, native symbol or build authority
is introduced. The whole unchanged run-pass program must complete its actual
assertion; this remains unvalidated and does not establish full compiletest parity.

## V8 acceptance and V9 paired scratch-place scope ownership

V8 actual70178/0 passed six commands on its unchanged3765-input source:430 library
bodies and126 integration events,all20 T043 plus updated H002 once,exact ledger and
2000-frame event once. Independent collection rechecked all270 upstream files and
source hashes; scoped hold was released. Whole unchanged let-destruct-ref.rs main
completed its original one assert_eq(x,*y), through normal CLI capture/discovery and
checked runtime macro lowering under the explicit Rust2024 adaptation. It supplies
one new program observation, not full compiler/tier/JIT acceptance.

Review then found the new macro place companions still had initial root scope0,
whereas their reserved values had the original statement scope. V9 assigns both
halves of runtime temporary/scalar pairs to the exact same indexed scope before
frame anchor creation; checked addition and SlotKind::Place reject missing pairs.
The source-derived ownership control checks all macro pairs in the same original
fixture, confirms its statement scope differs from root, and checks the initialized
ref statement's reserved reference pair too. No guest opcode or fuel executes in
this structural control. No scope/default-lifetime guess or descriptor substitute
is introduced. V9 remains unvalidated/unintegrated and T043 remains in_progress.

## Integrated V9 component acceptance

The guarded integration preserves the current MAIN history and every task status.
All production and fixture bytes match tested V9. Its431-library/126-integration
scoped validation and independent source release passed; a fresh MAIN milestone
is pending. T043/full fixtures/JIT/release/Linux remain incomplete.

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

[Collected milestone](../artifacts/macos-first/t043-reference-components-integration-v9/current-main-reference-milestone-v2/suffix-independent-collection.json).


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


## Current provisional component acceptance


The lexical scope, reached temporary borrow and source-context component is accepted
on macOS ARM64/Rust 1.99.0. This records the implemented component in the fixed
Section 57 order; it does not fulfill the complete T043 language contract.

Fresh integrated MAIN membership fd49c1fc4a4a84939131e9e0183dcfa66cc267961560d2ffdf950ba264525980
passed all 17 commands with runner session 19130 exit 0: 2,341 host bodies across
179 executables, no failed/ignored/filtered tests; all 41 T043, 73 T042 and separate
H002 controls and the 2,000-frame regression passed once. Seven unchanged stock CLI
probes passed, repeating existing observations. Upstream integrity verified all
157 cases/276 files at ae0fe511b15ab760d7e963b13389d1bc2ab42b18 with both licenses.

The first independent collector failed on a schema assumption about the CLI path.
The retained correction completed with actual exit 0 and no test or Cargo rerun.
Collection and release hashes bind the unchanged source, logs, fresh gates and CLI.

The component covers indexed lifetime storage, original local declaration order,
reached computed temporary borrows, selected ordinary-call barriers, simple initialized
ref/ref mut bindings, borrowed target contexts and lexical cast type admission.
Unknown constructor frontiers refuse explicitly. Positive source constructor/Fn-value
integration, arbitrary extending patterns, source-selected Drop transcripts, complete
edition/generated-source combinations and full fixture oracles remain required.
All final fixtures and dependencies remain explicitly pending in docs/tasks.json.
Full rustc compiletest passes, full T043 acceptance, release and Linux acceptance
remain false. Production JIT recording/lowering/transitions remain unimplemented.

[Independent collection](../artifacts/macos-first/t043-stock-namespace-integration-v15/current-main-milestone-v1/independent-collection.json).
