# Common reference parameter sources — bounded architecture handoff

Owner `/root/common_reference_sources`, coordinator `LOCAL-024`; October 15,
2026. T024/T032/M014/M017/M018, WT04-01/02 and WT08-02. Baseline HEAD
`53ab9d3f97eb4e963b65f24d39c0df1ea9e0669e`, with inherited and concurrent changes
preserved. This is an **uncompiled, unexecuted source component**, not an
integration, task closure, stock runtime/full-fixture pass or macro architecture
acceptance. The sole validator owns all Cargo/test execution.

## Invariant and ordinary source route

An admitted function's reference parameter retains its actual source/member
identity, current Type-name authority and signature-owned region regardless of
original, retained or generated source origin. Caller regions/provenance come
from actual admitted frame slots and checked reference cells, never from a
runtime layout or a primitive-looking spelling.

The minimal witness is moving `fn take(x:&i32)->i32{*x}` from handwritten source
into arbitrary user-macro item output. Before this change,
`ClosedReferenceParameter::capture` returned `None` solely when its published
source had `GeneratedSyntax`, before consulting ordinary parameter syntax.
Retained original functions additionally could not find their structural parser
in the original-source-only `Database::prepared_parse` registry.

The unchanged primary upstream witness is
`fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-call.rs`.
Its SHA256 is `92dce9e6d86786593d7f073831208a8dab8de6a4beee7a62e1b46fbe4efe981c`,
identical to the official checkout at rustc commit
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`. Existing tests in
`closed_reference_calls.rs` already inspect its two distinct named/elided region
owners and actual caller reference/source tamper controls. No original was
changed or recopied. `rustc/rust/tests/ui/macros/macro-lifetime.rs` is an inspected
next frontier: its generated `&str` result and static-origin argument need
additional common infrastructure and are explicitly unaccepted here.

The existing route is `CallResolver`/`SourceCallable` current declaration rows ->
ordinary `ParseFile`/`FunctionSignature` -> parser-owned `ClosedReferenceSyntax`
-> `SignatureNames` plus source-owned Type namespace -> `ClosedReferenceParameter`
-> ordinary argument hints and `bind_references` -> existing call packet and VM.
`ClosedReferenceArgument::capture`, `validate_value` and contract validation keep
actual frame/source/region/cell/session/lease/access checks. Frame continuations,
reference owners and ordinary return/panic cleanup are unchanged.

## Implemented join and boundaries

`ClosedReferenceParameter::capture` now demands the existing pure memoized
`ParseFile` query for the actual published source. It does not parse display
text or invoke expansion. Original/retained names use existing
`type_name_namespace::require_primitive_name(SourceTypeOwner)`; its
`retained_original_declaration` bridge continues to require unchanged genuine
original headers and current source admission.

Generated reference names use
`generated_result_namespace::require_reference(&Database, &FunctionSignature,
&ClosedReferenceSyntax)`. The opaque ordinary-parser proof carries its exact
parameter `NameRef` range, source key, selected signature and ordinal.
`require_name` shares the existing generated-result authority checks: exact
published generated definition/header/context/source, genuine current expansion
record/discovery seal, current owning Type receipt, target origin closure and
foreign-definer Type authority. The result wrapper retains its existing
full-signature replay checks. Reference authority is independent of result
syntax; a unit result is covered. Named and elided regions still derive from
the actual signature. Static-origin parameters remain explicitly unsupported.
No generic spelling/range constructor, macro-name rule or runtime inference was
added. No shared `call_contracts.rs` or `service/mod.rs` edit is required from
this worker; the coordinator owns the service test-module registration.

`ParsedFile::function_signature` and `closed_reference_parameter` now select the
exact function interval with existing `syntax_at`; the existing header/body/
definition/profile checks remain. There is no whole-file scan fallback. Exact
range lookup follows the retained tree path under its existing depth bound;
header token capture remains proportional to the selected header.

No evaluator was removed by this bounded join. The superseded production routes
are the generated-origin early refusal and original-cache-only parser selection,
plus the two selected-signature whole-file function scans. All eight historical
macro shortcuts remain held outside this slice.

## Costs and preparation demand

Only the entered parameter's current parser/signature/name facts are demanded.
The parse is retained through `Arc` and the ordinary memo framework; no callee
executable body, unselected arm or cold function is lowered to answer this
parameter demand. New target range storage is fixed size and included by the
existing `size_of` accounting; source proof metadata remains checked/bounded.
The reference join reuses its opaque parser proof and does not reconstruct the
generated signature a second time. Existing generated-result replay and original
Type namespace reconstruction remain cold-path cost debt. Installed execution
uses existing prepared operations; reference proof currentness still checks its
existing revision/source/namespace stamps. The existing global namespace census
in capture/currentness is held architecture failure mode 7, not repaired or
advertised as efficient. There are no measured speed or scaling claims.

## Source-aware eight-mode review

Separate review against the exact six-file fingerprint below:

1. `capture -> SignatureNames -> ClosedReferenceParameter -> bind_references`
   is the existing ordinary call path; no assertion/expression evaluator was
   added. MA001 and MA008 elsewhere remain held.
2. `closed_reference_parameter` reads actual generated ordinary Fn/RefType/
   NameRef nodes. No literal/break macro executor was added; MA004/MA005 are
   unaffected.
3. `SignatureNames` and shared current Type absence receipts select primitive
   meaning, independent of macro identity, method name or runtime descriptor.
   MA003/MA006 elsewhere remain held.
4. The generated early refusal is replaced only by the opaque source proof,
   exact generated rows, target origins and current Type seal/record checks.
   Original/retained admission uses `SourceTypeOwner`. General mixed-site,
   nested-owner, string/static reference sources remain explicit gaps.
5. The join inspects only selected signature/parameter syntax; it adds no
   pattern engine and no cold executable-body/arm shape requirements.
6. Reference parsing uses memoized `ParseFile`; generated parameter authority
   reuses the already captured proof. Existing result replay and Type namespace
   rebuild are unchanged limitations, not fixed by this slice.
7. `lexical_namespace_stamps` and existing whole-world currentness comparisons
   remain unchanged and held. No unrelated-invalidation/scaling acceptance.
8. Both selected-function signature searches use exact `syntax_at`; immutable
   parsed trees use retained Arc handles. No production BodyIndex copy or
   whole-file node-search fallback was added.

These are inspected source conclusions, not executed route assertions. The
separately numbered MA audit findings are not the same numbering as this list.

## Validation and acceptance holds

Actual terminals by this worker: targeted `rustfmt --edition 2024 --config
skip_children=true` exited 0; `git diff --check` on all six leaves exited 0.
No Rust/Cargo test or native oracle was run. No commit/push/ledger update was
performed. A redundant new upstream signature test was removed because the
existing unchanged-original control already provides stronger evidence.

New source-only controls are four frontend tests under `generated_reference_`
covering all four editions, user/wrapper generated parameters with unit results,
named/elided region identities and mutability, Type shadow/current epoch/foreign
signature refusals, explicit static refusal and foreign-definer Type shadow;
and two service tests under `retained_reference_` covering the ordinary call/
BodyStructure/Thunk trace matrix and retained parameter proof epoch refusal.
The ordinary matrix asserts the result, actual call count, frame depth and
started/completed operation equality. Their assertions are **unexecuted**.

The sole validator should run those filters, existing
`closed_reference_calls::tests`, `generated_result_namespace` authority controls,
signature/source tests, strict format/Clippy checks and the current milestone on
frozen combined source. Preserve all failure terminals. Required missing closure
includes actual standard-macro common-route execution, stock whole-main runtime
and full-fixture acceptance, broader string/static references, suspension and
complete macro architecture/removal matrix. A matching result alone is
insufficient. Generated member publication gaps exposed by validation must be
repaired through the common source/member mechanism, never through an evaluator.

## Frozen source identity

The aggregate SHA256 is obtained by appending each listed UTF-8 path, one NUL,
and its exact bytes in this order:
`f42f253bd2c66c243d6b0ddeea199fbc073ffa489ad38d2a82934243a15ec0e3`.

| Path | SHA256 |
| --- | --- |
| `crates/lrr/src/frontend/closed_reference_calls.rs` | `d462f1214ad5cb0b230dadc98ab3a8509cf73173b59706b0399aa2b424764f0d` |
| `crates/lrr/src/frontend/generated_result_namespace.rs` | `ffbf658f5553fe7efc57410e81f496db8238373cbcf20c88951b00ea4faabca6` |
| `crates/lrr/src/frontend/generated_reference_source_tests.rs` | `f977eebf995f7226e353f6a0cf3789ac599f03c91a967a9472884ef127e2a912` |
| `crates/lrr/src/syntax/closed_reference_signatures.rs` | `4dcea8df143a813018171a75be7be8161495fe08cb03b7b33b9b450a167e7fd9` |
| `crates/lrr/src/syntax/signatures.rs` | `282b7c551edd7b1942efc24afcf45de83009c51e6c06c7f65dc2ef6a6853a462` |
| `crates/lrr/src/service/retained_reference_source_tests.rs` | `c4396fd0541a54ae9ee85a3254cf9cbfc620388731f9ba006cc3a6b472f10e36` |

All six leaves remain unchanged after this handoff. Any later fix or formatting
requires a successor fingerprint and rechecking affected review/validation
conclusions; prior evidence stays attached to its original source.
