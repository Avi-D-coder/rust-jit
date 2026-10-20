# Shared discriminant initializer demand — separate source review

Inspected V486 source fingerprint
dd808f8d98c5e84205c8881cbc944ae65f90cc3227c8fac9623082f8017ba319,
baseline e53603b0676f689dcfbfe078ebc9a1c886d9595f. Solo /root.
This is a held runtime component; no full feature, solver or JIT acceptance.

1. No assertion/expression evaluator added. Source-selected
   LibraryFunction::DiscriminantValue reaches prepare_facade_discriminants,
   which consumes only existing frame slots, then the same initializer
   coordinator as ordinary PrimitiveCast. The supplemental Python control
   invokes the actual CLI; it does not interpret or expand Rust.
2. No literal/break-only macro executor or alternate grammar. Original,
   user-macro, wrapper and standard-macro control origins use ordinary source
   capture, hygienic expansion, BodyIndex, lowering/verifier and VM calls.
   Full origin acceptance depends on the actual control terminals below.
3. No result/operand inference or method-name rule. The function discriminator
   is the existing source-selected closed runtime intrinsic, not caller text.
   Its actual source integer/descriptor and shared scalar reader are unchanged.
   Generic operator typing in unchanged repr128 remains a separate failure.
4. No new original-only refusal. The existing missing initializer row refusal
   now states the actual absent source fact instead of claiming an origin rule.
   prepare_enum_discriminant_inner uses the same anonymous source definition
   and ordinary initializer mechanism for every admitted declaration. Missing
   generated initializer publication remains an unclosed prerequisite.
5. No pattern machinery, unselected executable arm preparation, static check or
   borrow checker. Only reached live operands select a pending variant. CallSeal
   restarts the normal dispatch loop before sealing/moving any arguments; the
   existing Initializer continuation preserves the caller frame/slots/leases.
6. Removed the per-visit full operation filter/Vec construction in
   prepare_enum_discriminants_inner. install_bound computes ordinary cast
   operation-index/slot metadata once after prerequisite/currentness checks;
   InstalledRecipe retains an Arc slice, and enum_casts shares it. Preparation
   probes relevant runtime operands after resume; completed entries do not
   fetch schemas. A genuinely pending variant fetches its memoized source
   schema and cached constant row once through the existing cold coordinator.
   No installed scalar execution parses/plans source. Cold index cost O(ops),
   retained size O(casts), Arc clone O(1), runtime probing O(relevant casts).
   These are structural observations, not measured performance claims.
7. enum_casts uses the same get_current validation as the removed scan. No new
   namespace census/map construction, unrelated invalidation or global lookup
   is introduced. The existing one-counter pending fast path remains. Intrinsic
   selection and source-row caches retain their existing currentness owners.
8. No BodyIndex/parser deep copies or whole-file searches added. The new Arc
   index shares immutable LRIR positions; frames retain their mutable state.
   Existing first-install VerifiedRecipe clone remains unchanged. No new
   resource accounting/caps, unsafe or backend/query framework is introduced.

Common runtime reference extraction is now referent_enum_variant; both pending
preparation and referent_discriminant use its actual checked place/active
variant. pending_discriminant shares table readiness with cast preparation.
prepare_enum_discriminant_inner owns source anchor/row demand, existing
const_cells reuse and agreement-checked table refinement. The ordinal/zero
fallback removed in the previous checkpoint is not reintroduced.

V485 uses the exact pre-change CLI and a prefix-preserving repr128 adaptation:
it fails the first nonliteral intrinsic with the E013 missing-evaluation error.
The same source compiles/runs under rustc1.99 edition2024 (separately labeled
reference evidence). V486 library and CLI builds pass. Initial ordinary2024
controls complete five selected discriminant reads and their repetition with
initializer counts5/5 while source call boundaries grow10/20. The unchanged
whole repr128 still fails generic operator typing; enum_discr,
discriminant_size and actually_not_an_enum-discriminant complete in owning2015.
The full remaining origin/edition controls and closing guards are pending at
this source review, never inferred from the initial controls.

Remaining obligations: full upstream source facts/solver/root/projection/member
integration; mem::Discriminant's associated field type; generated initializer
row publication; enum/provenance images and general borrowed views; nonliteral
layout demand; explicit source-withdrawal/NeedInput/fault-injection matrix for
this added path; full runtime inventory and interpreter correctness. Existing
source-generic initializer packet holds remain unchanged. A retained-index
Arc-identity test and asymptotic measurements are not supplied by these guest
output controls. No task, milestone, full fixture, Linux or JIT gate is closed.

Collected final evidence at the same V486 fingerprint: both builds0, both
existing controls0,32/32isolated control executions and16/16repeat pairs pass.
Every pair has5/5initializer frames and10/20source-call boundaries; all recorded
fault/effect/unsupported candidate counts are0. The32rows are LRR adaptations,
not32stock tests. Actual stock batch passes3/4 in owning2015: enum_discr551,
discriminant_size4684 and actually_not_an_enum-discriminant1033 matched VM
dispatch/outcomes with unit/empty-stream/cleanup captures. repr128 remains
failed28attempts/27outcomes at its generic operator, before these initializer
cases. No newly passing whole upstream case is attributed to this change.
The source, HEAD and both binary guards match before/after every execution;
all processes terminate and the closing guard terminates0. The retained Arc
property is source-reviewed, not inferred from output or advertised as a
measured speedup. The source-withdrawal/NeedInput/fault and index-identity
negative controls listed above remain unexecuted requirements.

Renewal review: install_call_renewal::renew_call rejects any
previous.operations != next.operations (also key/origin/exit differences)
before atomically replacing an installed recipe's source evidence. Its retained
cast index therefore remains valid across that renewal; it is not silently
reused for a different executable shape. No other InstalledRecipe replacement
route was found in the inspected install modules. This is source evidence,
not a newly executed renewal negative regression.
