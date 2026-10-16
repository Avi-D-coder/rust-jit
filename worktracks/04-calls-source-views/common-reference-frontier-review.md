# Next unchanged reference witness frontier — read-only review

October 15, 2026; `/root/common_reference_sources`, coordinator `LOCAL-024`.
This proposes a next bounded slice; it does not implement or authorize one,
change a task status, import a new original, or establish a current runtime pass.
The six implementation leaves in `common-reference-source-handoff.md` remain
unchanged at aggregate `f42f253bd2c66c243d6b0ddeea199fbc073ffa489ad38d2a82934243a15ec0e3`.
No Cargo/test/native-oracle command was run for this review.

## Actual evidence and witness classification

The historical `2026-10-14-reference-parameter-runtime.json` records whole
unchanged `regions-infer-call.rs` main in all editions, and V123 also records its
runtime-contract pass. Those old outcomes are retained; they do not establish
the new common generated-source route. The current six-leaf reference source
candidate is still uncompiled/unexecuted.

The actual frozen V123 report is
`artifacts/macos-first/root-stock-local-numeric-validation-v1/agent-validation-v123/stock-cli-batch-2/runtime-report/report.json`,
source member `f8f33fe025284209a31a39f4a6afdf419c435679c77f3d3d231014db058d033d`;
its terminal is historical `1737e7/1`. It records:

- `tests/ui/regions/regions-simple.rs` fails with
  `reached println requires pending formatter and managed output integration`.
  Its earlier mutable borrow, dereference and assignment are not the reported
  stopping point. Current source still has ordinary `DerefPlace`/`AssignPlace`
  lowering. This case should stay a whole-program common output/formatting
  dependency, not be relabeled a reference-parameter failure.
- `tests/ui/regions/regions-infer-borrow-scope-within-loop-ok.rs` fails at
  `let x: Box<_> = Box::new(3)` with
  `local type requires later symbolic type resolution`. Generic `borrow<T>`
  reference-result transport is later in that original. Its first frontier is
  source-selected `Box<_>` local type/member preparation; a reference-only
  patch cannot honestly claim this whole fixture.

Both imported originals match their official rustc checkout sources at commit
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`:
`regions-simple.rs` SHA256
`eb0b190737724638de6556e1378399af9687f57cfa99d89422461fde6bb1eb12`;
`regions-infer-borrow-scope-within-loop-ok.rs` SHA256
`397d69eb4733ff856d4e35860d4fabfacb7a6a45d33457b90567f9e072b5065d`.
These terminals have not been rerun against current source.

The next small unchanged official source witness for reference results is
`rustc/rust/tests/ui/regions/regions-addr-of-ret.rs`, SHA256
`9082b9f9faea5db67c36136721ef4153951027cda2fc4d80eb78e34f3c1dc914`:

```rust
fn f(x: &isize) -> &isize { return &*x; }
```

Its whole main additionally needs numeric local source propagation and ordinary
`println!` expansion/formatting/output. The original has not been imported or
executed in this review. Copy it unchanged with its directives/license/provenance
when the slice is claimed; do not strip println or count an adapted main as the
stock runtime-contract pass.

The selected generated-reference witness
`rustc/rust/tests/ui/macros/macro-lifetime.rs`, SHA256
`7a536ccb92dfb6bcbf99149caa7d326eeeea5c75d23c86f26eab189ab4192a08`,
generates `fn f<'a>(arg: &'a str) -> &'a str { arg }` inside main, then calls it
with a string literal and constrains the result to `&'static str`. It exposes
body-local generated member publication, `str` source/view admission, static
literal provenance and reference result transport. These are real prerequisites;
none can be replaced by a macro-name exception or runtime descriptor inference.
`regions-infer-call-2.rs` adds a capturing closure, generic `FnOnce` dispatch and
a reference-result-producing higher-order call. It is a larger subsequent
frontier, not an alternative tiny parameter test. `regions-infer-call-3.rs` is
compile-fail lifetime checking and belongs to separate diagnostic/static-checker
reporting; it does not authorize adding a borrow checker.

## Smallest proposed common-infrastructure component

T032/M017/M018, WT04-02/03: **source-certified reference result linkage**, first
for a selected function's primitive-pointee reference and existing supported
region equality. This is a header/source component until runtime integration is
explicitly implemented and validated. It cannot close reference returns, macro
architecture or the stock fixture by itself.

The invariant is that a declared result reference's pointee/access and region
relation come from its actual source signature and current owning Type namespace.
An elided result relates to the single input reference region under ordinary
Rust function lifetime elision; named results retain the selected lifetime
parameter identity. The actual caller's region is substituted only from the
admitted reference argument certificate. No arbitrary elided result, ambiguous
input relation, static promotion or outlives proof is invented.

The actual missing joins found in current source are:

1. `syntax/signatures.rs::lower_type` returns `None` for a reference with no
   explicit lifetime, and primitive path leaves are intentionally deferred to
   source name authority. The ordinary parser should capture a typed declared
   reference slot (`Parameter(ordinal)` or `Result`) with exact source/token
   ranges and result-to-input region syntax relation. Extend the existing
   `ClosedReferenceSyntax` source mechanism rather than introduce a result-only
   parser. Header parsing must not prepare the body.
2. `frontend/closed_reference_calls.rs::DeclaredRegion`/`ReferenceBinding` and
   `generated_result_namespace::require_reference` already bind actual parameter
   source and current Type authority. Generalize the source slot proof, preserve
   current source/header/seal/target-origin checks, and expose only a checked
   declared-to-actual region substitution from existing physical bindings.
   `call_contracts.rs` may then retain an opaque reference-result certificate
   beside its descriptor/source contract. The common `SourceTypeOwner` join
   remains the original/retained authority path.
3. `vm/context.rs::GenericEnv::checked_bytes` currently accepts a deferred result
   only when absent or independently certified primitive/alias. It must require
   the new source-produced result proof, account it, reject packet tampering and
   retain the certificate through source/epoch changes. A matching `TypeKey::Ref`
   alone must never authorize that packet.

This component should refuse unsupported multiple-input/region relations and
preserve the existing static-origin refusal. A source-only result plan does not
authorize executing the result. No new opcode, evaluator, compiler hook or
frontend framework is necessary.

## Required later runtime integration, explicitly not part of a header-only pass

`engine/run.rs::check_source_return` handles resolved primitive/rigid results;
its deferred fallback requires a one-token primitive result. `run_return.rs`
constructs primitive return source facts with `primitive_argument_type`, and
caller resumption restores `contracts.source_result` rather than independently
substituting a reference's actual declared input region. These entry points need
an ordinary reference-result continuation, not a synthetic scalar return.

A subsequent bounded runtime slice must inspect the actual return ticket and
its retained `frame.return_source`, validate the live reference record and its
owner/access/target/session using existing store authority, apply only the
source-certified result region substitution, and reserve the caller result
before cleanup. `GuestStore::reference_from_cell` validates cells, not arbitrary
return tickets; factor a checked read-only ticket/record validation service
instead of forging a temporary cell or trusting raw payload bits. Reborrow
leases rooted in caller storage must survive callee cleanup, while a reference
to callee-owned storage must fail before caller resumption. Parameter/source
access and the existing exact-once ReturnContinuation owner remain intact.

Immutable signature and source proof preparation stays in the query slow path;
no source parse, result-plan reconstruction or namespace census is added to
return execution. Relevant sealed dependencies remain required. Existing global
namespace currentness cost must be held or separately repaired, not copied into
a new return guard. Source slot/token indexes and certificate bytes need checked
Section 32.2 accounting. No speed claim follows from structural review.

## Required evidence and architecture review

Use unchanged `regions-addr-of-ret.rs` as the source witness plus supplemental
handwritten/user-macro/wrapper result linkage controls outside the original.
A later applicable standard-macro context must use real ordinary expanded source.
Prove exact indexed Fn/RefType/NameRef roles, current Type shadow/epoch/source
invalidation, wrong result/parameter certificate refusal, named versus elided
region substitutions, and cold body non-preparation. Runtime work additionally
needs actual forwarding/reborrow calls, actual caller source provenance,
mutable/shared permissions, caller-owner survival, callee-local escape refusal,
NeedInput continuation, return panic/cleanup and exact-once operation/call counts.
Keep stock whole-main/CLI/full-fixture outcomes separate from component evidence.

For the eight recurring modes: reuse ordinary calls and ReturnContinuation (1),
ordinary indexed generated syntax (2), current Type/member authority (3), actual
origin/source joins rather than blanket/generated bypasses (4), no pattern or
cold-result execution requirement (5), retained pure result plans (6), relevant
sealed dependencies with existing global-census debt explicit (7), and retained
Arc/exact-range syntax handles (8). MA001–MA008 remain separate audit obligations;
a new result source component does not remove their alternate execution routes.
