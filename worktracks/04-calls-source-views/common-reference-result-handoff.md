# Common reference results — successor architecture record

Owner `/root/common_reference_sources`, coordinator `LOCAL-024`, October 15,
2026. T024/T032/M017/M018, WT04-02/03 and WT08-02. This is a successor to the
frozen parameter packet `79a130cf8bd1ca51c59cd609df71fe020e2b9a4303d43630b8f1c79b5338f9b2`;
its evidence remains attached to those bytes. **Uncompiled/unexecuted design and
implementation candidate; no task/fixture/integration acceptance.** The sole
validator owns Cargo/native execution; this worker runs none and adds no agents.

## Bounded invariant and minimal witness

A reached function returning a reference must retain declared source/member,
pointee/access and signature-owned result/input region relation through actual
caller provenance and ordinary cleanup. Its return ticket must contain a live
checked reference to the admitted input target; no descriptor or raw payload
creates source or lifetime authority. Return/caller resume remains exact once.

Primary unchanged official witness:
`rustc/rust/tests/ui/regions/regions-addr-of-ret.rs`, rustc commit
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, SHA256
`9082b9f9faea5db67c36136721ef4153951027cda2fc4d80eb78e34f3c1dc914`.
Its `fn f(x: &isize) -> &isize { return &*x; }` is the minimal required reference
result/reborrow operation. No original is changed. Whole main additionally needs
ordinary common println/format/output and numeric-local source preparation;
a source/component witness never establishes its stock runtime/full-fixture pass.

## Ordinary source route and actual shared entry points

Current `CallResolver/SourceCallable -> ParseFile/FunctionSignature ->
ClosedReferenceSyntax -> SignatureNames/SourceTypeOwner or generated current
Type seal -> ClosedReferenceParameter -> bind_references -> call contracts ->
GenericEnv -> ordinary VM Return -> RunMachine::begin_return_cleanup ->
advance_return_cleanup -> caller.resume_call` is the route to extend. Existing
Frame source slots, parameter cells and ReturnContinuation remain the mutable
owners. All source origins use the actual original/retained/generated parse,
declaration scope, edition, header, current source/epoch/seal and Type authority.

`ClosedReferenceSyntax` will share its actual RefType/NameRef capture for typed
parameter/result slots; no new parser or evaluator. A new opaque
`closed_reference_results` proof retains only a source-certified primitive
reference result and its checked relation to the admitted input-region
certificate. It lives inside existing `ClosedReferenceContracts`, avoiding
public packet-shape exceptions and duplicated constructors. Extend only narrow
`call_contracts::prepare`, `GenericEnv::checked_bytes` and source-return admission
for this certificate. Every altered normalized result table must still fail
opaque proof validation.

The checked store will provide a read-only opaque target receipt captured from
the actual caller reference cell. It retains store session, root/final location
stamps and target/access facts, never a host pointer. Return validation reads the
actual ticket, verifies mutable ticket ownership, actual reference record and
location lease, and compares the same target before cleanup and before resume.
A reference to callee-owned storage must fail before caller publication. Caller
source region restoration is authorized only by the selected declared result
relation and the already admitted physical argument target. Reborrows may have a
new lease ID; equal final values/layouts do not establish the target relation.

## Dependency and authority boundaries

Coordinator explicitly allocated narrow APIs in `frontend/call_contracts.rs`,
`vm/context.rs`, `runtime/cell.rs`, `engine/run_return.rs` and
`engine/run.rs::check_source_return` to this worker. New reference-result and
store proof modules/tests are disjoint from the pattern worker. No pattern,
lexical parser, expansion producer, assembly or receipt paths are edited.

Supported result relations are ordinary single-reference input elision and
explicit named lifetime equality from the selected signature. Ambiguous,
static-origin, general outlives/coercion, generic pointee, string/slice and
nested-owner prerequisites stay explicit if unsupported. Do not synthesize
region equality from runtime type or erase existing source refusals. Static
borrow checking remains cargo-check territory, not a new interpreter checker.

## Cold and hot costs

Preparation uses the existing memoized ParseFile/signature query, exact retained
Fn/RefType token ranges and shared current Type-name proof. It inspects no cold
executable body/arm; generated parameters/results use one authority mechanism.
Retained parse handles use Arc. New receipt storage is fixed size; result proof
and source terms use checked Section 32.2 accounting and existing budgets.
The runtime return path reads a prepared certificate and actual ticket/lease;
it must not parse source, rebuild a signature/namespace or plan a body.
Existing SourceCallable/current reference-certificate checks and their global
namespace census remain held cost debt, not a new performance guarantee.
The ordinary bounded cleanup continuation and actual store ownership do all
cleanup work; no alternate evaluator, synthetic frame or replay is introduced.

## Eight-mode review plan before integration

1. Inspect ordinary source call and ReturnContinuation ownership; no assertion
   or other alternate evaluator. MA001/MA008 remain separately held elsewhere.
2. All generated RefType/member nodes use ordinary parser and source rows;
   no literal/break-only execution (MA004/MA005 remain held).
3. Source Type/name authority determines pointee/access and result linkage;
   no macro identity, method whitelist or numeric/layout inference.
4. Preserve genuine generated source/origin/current Type joins; never remove
   generated refusals without these proofs. Report remaining source gaps.
5. Only signature/header and reached ticket demanded; no cold result/branch
   executable readiness requirement or duplicated pattern engine.
6. Memoized source preparation; immutable result certificate consumed at runtime,
   no parse or full plan reconstruction during Return/cleanup/resume.
7. No new full namespace census in return operations; relevant source/currentness
   guards stay sound and existing global-census debt stays explicit.
8. Exact syntax locations and shared immutable handles; no whole-file lookup or
   deep BodyIndex copy. Account new retained terms/receipts and checked sizes.

A separate final review will record exact successor source hashes, inspected
symbols, remaining routes/costs and actual available terminals. None of these
checkboxes establishes implementation correctness or runtime acceptance.

## Required evidence and acceptance limits

Use unchanged official source plus LRR controls outside originals: handwritten,
user macro and wrapper ordinary reference-result calls/reborrows; actual indexed
node/VM trace and exact-once counters; named/elided input-region linkage; shadow,
source/epoch and packet tamper refusals; ticket store-session/owner/access/target
mismatch; caller-target survival across cleanup; callee-local escape refusal
before resume; suspension and panic/cleanup continuation where supported.
Standard macro and whole-main output dependencies stay separate. The sole
validator must freeze this successor, collect exact commands/terminals and run
scoped controls, strict checks and current milestone. No source-only or adapted
witness becomes a stock runtime/full-fixture pass. No JIT gate changes.


## Implemented successor and final source review

Source baseline at final review: `fa071ee4d381e88645a837b23574534e1aeeaf80`.
The initial bounded architecture record above preceded implementation; the
following describes actual source, with all execution still uncompiled/unexecuted.

`ClosedReferenceTypeSyntax` is one ordinary parser-owned RefType capture shared
by parameters and results, with exact NameRef range and private syntax role.
The existing parameter wrapper preserves its ordinal API. Result capture uses
that same parser/Type-name authority, single-reference input elision or exact
named lifetime relation, and the already checked physical reference bindings.
Multiple inputs under one named lifetime retain only the existing supported
source-region equality; no outlives solver or static borrow checking was added.
Static-origin, general generic/str/slice/nested pointee and unsupported owner
relations remain explicit gaps.

`ReferenceResultContract` lives inside existing opaque
`ClosedReferenceContracts`, retaining exact source, allowed input bindings and
optional known source-selected Ref descriptor. `call_contracts::prepare` supplies
that certified source to the existing signature substitution; no public packet
field alone can create result authority. `GenericEnv::checked_bytes` validates
both normalized source/result descriptor and the complete parameter/result
certificate. Its readonly `reference_result_source` supplies the prepared actual
caller region through the common root-result hint.

The peer-owned IR/VM bridge supplies `HintRecipe::DefinitionResult` under recipe
schema11. Coordinator root thunk glue selects it only for an actual root RefType
return. The verifier and Frame require the exact root entry/result and the
matching prepared definition. Frame/step consume the admitted result source,
then use ordinary Ref descriptor materialization; they do not infer source from
runtime layout. Peer also fixes ordinary reference preflight_return by checking
the actual live reference cell instead of applying primitive scalar preview.
These are required shared dependencies of this successor, not worker-owned
independent acceptance. Their final packet identity must be recorded by the
coordinator with the combined successor.

`GuestStore::reference_target` captures the actual store session, root/final
location stamps, pointee/access and **every original enclosing owner link**.
`validate_reference_record` is factored through the same borrowed-parts
`validate_reference_target` service. Return validation replays both original
input authority and actual returned record, including parent stamps, object ID,
variant, field membership and child ownership. Chains must match as well as the
endpoints. It validates actual ticket ownership, source-declared access/pointee
and target record before cleanup and before caller publication. No positive
reference/ticket/receipt is fabricated. A new reborrow may have a different
lease ID while retaining the same complete checked target path.

The coordinator source review correctly rejected endpoints as sufficient proof.
A new adversarial store control changes field ownership/membership while keeping
root/final stamps unchanged. A fresh actual returned lease follows that new
valid path, but replay of the original receipt fails. This tests the extra
original-chain requirement rather than merely obtaining the same scalar value.
Normal store mutations preserve their existing epoch/owner checks; the test's
private adversarial mutation is not a production API.

New receipt buffers use checked chain sizes and are accounted in retained
reference contracts. `capture_budgeted` checks remaining source-call metadata
before allocating target receipts; the exact argument evidence loop in
`prepare_source_call` bounds their aggregate storage, and `bind_references`
checks the receipt heap before cloning retained bindings. Conservative duplicate
charges are possible; no unaccounted target-chain clone or speed claim is used.
The same store slice also supplies the pattern peer's separately requested
`preflight_pattern_bindings/reserve_pattern_bindings` APIs: checked combined
cell/move-ticket reservation before any pending binding moves. Pattern transaction
semantics and tests remain peer-owned.

`begin_return_cleanup` admits the actual return ticket against the current
opaque result proof and its actual frame source. It permits the exact caller
input source or a real callee reborrow source while requiring the actual complete
target receipt. The ordinary result source restored on resume is its certified
caller region. `advance_return_cleanup` rechecks the ticket/target before each
bounded cleanup turn and after cleanup, before resuming the retained caller.
The existing source/call action, fuel, allocation and exact-once continuation
owners are preserved; no synthetic cell/frame or alternate evaluator was added.
Callee-local targets are refused before cleanup can publish the caller.

Separate source review against the worker-owned source fingerprint below:

1. Inspected `ReferenceResultContract::capture`, ordinary `call_contracts::prepare`,
   `begin_return_cleanup`, `advance_return_cleanup` and `check_source_return`.
   They extend actual CallResolver/GenericEnv/VM return ownership, not an AST or
   assertion evaluator. Existing MA001/MA008 alternate routes remain held.
2. Actual generated Fn/RefType/NameRef nodes use the same source signature and
   ordinary VM operations as handwritten/retained syntax; no literal/break
   executor. MA004/MA005 removal is still outside this slice.
3. `SignatureNames`, `SourceTypeOwner` and generated current Type/origin receipts
   establish primitive pointee meaning. Ref descriptors are selected from those
   source facts, then compared with actual records. No macro-name, method-name,
   numeric-context or runtime-layout source selection was added.
4. Generated source uses `require_reference_type` with opaque exact syntax and
   the existing actual definition/header/scope/seal/record/origin checks.
   Original/retained headers use their genuine source join. Unsupported
   source/region classes remain explicit; no blanket origin refusal was removed
   without authority.
5. Preparation inspects selected signature syntax and actual reference arguments;
   it lowers no cold executable body, alternative result or unselected arm.
   No pattern implementation was introduced. Controls keep the original stock
   main and an unknown cold body unprepared; assertions are unexecuted.
6. Return/hint/cleanup code consumes retained source certificates and actual
   store facts; it does no parsing, signature reconstruction or namespace plan
   construction. `ParseFile` is the existing immutable memoized query. **The
   common call preparation still recaptures immutable header/reference syntax
   at reached call boundaries rather than retaining one declaration plan in the
   pure query memo.** This pre-existing parameter-path cost, also used by the
   new result join, remains a prepare-once architecture hold. A memoized common
   declaration/reference source plan is the precise remaining prerequisite;
   this candidate cannot claim complete source preparation/performance repair.
7. New return validation checks the existing exact database/source revision pins
   of its admitted parameter proofs plus actual current callee rows; it never
   rebuilds namespace maps. Existing entry namespace census and conservative
   unrelated-publication invalidation remain held and unmeasured. Relevant
   sealed dependency-bucket replacement is still required; safety refusals were
   preserved rather than bypassed.
8. Parser selection uses retained exact function/type syntax ranges; no selected
   whole-file scan or production BodyIndex copy was added. New retained owner
   chains/source terms are explicitly size-checked/accounted. Ownership chain
   replay is O(depth); result candidates are O(number of selected input
   references), and source header checks are O(selected header size). No release
   performance inference follows from these inspected costs.

The MA audit numbering is distinct from these recurring modes. All eight
legacy shortcut removals and complete macro architecture acceptance remain held.
If peer/root glue changes after this review, the affected shared source/VM/hint
conclusions need a fresh combined-packet review.

## Actual available terminals and test scope

Worker terminals: targeted `rustfmt --edition 2024 --config skip_children=true`
exit0; `git diff --check` exit0. No Cargo/test/native oracle was run, no new
acceptance receipt exists, and no commits/pushes/ledger edits were made.
The previous frozen parameter/pattern packet keeps its own exact evidence.

Primary upstream import is unchanged, 129bytes, official git blob
`bf95709848fb137161515ac94daadc56f5ac0870`. Its only directive is `//@ run-pass`;
no auxiliaries/revisions/sidecars are present. Coordinator owns the manifest row.
The new engine probe appends an external `lrr_probe` while leaving all original
source bytes, f and main intact. It selects only the probe and actual original f;
this is an LRR component adapter, never a whole-stock-main runtime pass.

New/extended controls are unexecuted: actual original f reborrow/cleanup/caller
resume, callee-local escape refusal before cleanup, ambiguous result elision,
all-four-edition handwritten/named/generated/wrapper reference result forwarding
through a second actual source call, shared reborrow from a mutable input,
caller-target preservation, exact started/completed/call counters and cold-body
queries; result packet/source/epoch tampering and root-entry/definition authority;
store reborrow identity, equal-layout wrong target, cross-store handle collision,
mutable ticket owner/access, stale target, and changed original enclosing chain;
source receipt quota refusal without owner/counter changes.

Required validation filters include `reference_result`, `common_reference_results`,
existing `closed_reference_calls`, generated result/parameter authority, source
signature tests, peer DefinitionResult/transaction controls, strict workspace
checks and the full current milestone on the **combined frozen successor**.
Suspension/resume, failure-only formatting, panic/custom Drop and standard macro
matrix entries remain unexecuted/gaps; they cannot be inferred from the new
cleanup assertions. Full original main still needs common println/output;
macro-lifetime additionally needs nested generated members, str/static provenance.
No runtime/full-fixture/task/JIT completion is claimed.

## Worker source identity

The following aggregate hashes each UTF-8 path, one NUL and its exact bytes in
listed order. Worker-owned aggregate:
`27cbd3f4487bc9776149d4d735fb0ed01403657788ea37b3477e269b3912a765`.
Peer/root IR/VM/thunk glue must be included separately in the combined packet.

| Path | SHA256 | Bytes |
| --- | --- | --- |
| `crates/lrr/src/frontend/call_contracts.rs` | `7ec9dc15fe0b245aee52e670751a5b7aba60932188b96c9f5c2106600fec071c` | 56708 |
| `crates/lrr/src/frontend/closed_reference_calls.rs` | `f2056447ae62177b5b13a11d75f8bb066c0f7fae90b0d8e0f787783370e23419` | 42769 |
| `crates/lrr/src/frontend/closed_reference_results.rs` | `14543f480cebdb3dea3902a700e7fbfd839e26c31863548b4df9ec9acdec7be1` | 12873 |
| `crates/lrr/src/frontend/generated_result_namespace.rs` | `67a3fd59780db7961d5f0f5a1bd03728a1300b7f64076b6bc442c716955d2058` | 28742 |
| `crates/lrr/src/syntax/closed_reference_signatures.rs` | `39cb3e08030342d21f236f20d7b93affab1340979992e7f0f33c9a0c9cd97976` | 8437 |
| `crates/lrr/src/runtime/cell.rs` | `910b8562f69851e9542f1ffdd678a7577b7229e37b38b39f5c164a48127c2657` | 200403 |
| `crates/lrr/src/runtime/cell_reference_return.rs` | `96c56aac234807d7a9e54321c28b8a68245eca9c0e71ea8da1a94581f1fcb882` | 3245 |
| `crates/lrr/src/runtime/cell_reference_return_tests.rs` | `a8aa7a1ea13993bc582ceb7d299228f9024896f2d58421be6d9b167a310e5876` | 8159 |
| `crates/lrr/src/vm/context.rs` | `9e79c6acde06a5ffee7c6cae04954eef09939b7f34da4007c141b902da41d2a4` | 13031 |
| `crates/lrr/src/engine/run.rs` | `5c82cf47ca60edebc3ff0f39993b0629b323b5df16b80f1dd5baaca296a8b343` | 110142 |
| `crates/lrr/src/engine/run_return.rs` | `387c696431a195bf55e2c1fbdb9e10a28d46501c7388cb184815eaa76fb51f7c` | 10740 |
| `crates/lrr/src/engine/run_reference_result_tests.rs` | `79a8551c052715d0bec309ba7e4a4c2c703a6014267adfd1d8c7f325d527dd85` | 3523 |
| `crates/lrr/src/service/retained_reference_source_tests.rs` | `656cb8ac3cd7fd77d7df29190d81d218644086446d82a00a2a9db5cc621bc0ab` | 9529 |
| `fixtures/upstream/rustc-checkout/tests/ui/regions/regions-addr-of-ret.rs` | `9082b9f9faea5db67c36136721ef4153951027cda2fc4d80eb78e34f3c1dc914` | 129 |

All listed worker source leaves remain unchanged after final handoff. Any later
fix has a successor identity; old source/validation evidence remains attached to
its actual bytes.
