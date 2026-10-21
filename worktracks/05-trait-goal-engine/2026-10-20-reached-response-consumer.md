# M020 / LOCAL-049: ordinary reached-call response consumer

Precode source review during frozen V528. The unchanged witness is upstream
`tests/ui/enum-discriminant/repr128.rs`: `discr` must infer a literal argument
from its real projected bound before `bind_literal_fallback`, without executing
callee bodies or changing primitive widths by trait/macro name. No new code or
runtime pass is asserted by this record.

The ordinary route is `engine/run.rs` CallSeal after actual lexical/member
selection and typed-argument revalidation, through `run_call_inputs` and
`prepare_source_call`, into the existing Frame/call continuation. Original and
admitted generated calls already use that route. The packet captures actual
source terms and numeric families; `source_input_attempt` distinguishes repeated
visits to the same PC and retains identity over suspension. It does not prove
root completion or grant a VM commit.

Concrete ordering that the integration must preserve:

1. Resolve the existing result expectation before literal fallback. The current
   CallSeal computes it below fallback; move that same common logic ahead of
   capture instead of adding a second expectation resolver. Preserve the exact
   result-slot check for retained selections and the final bound source view.
2. Capture once per pending attempt and retain the owned packet over NeedInput.
   Add the actual verified result slot/phase/action identity to the consumer;
   supplying the packet's old expected Type back as a current expectation is
   not revalidation. Request an authentic dormant catalog through the retained
   host coordinator only when the source demand needs it.
3. Prepare real query inputs outside solver callbacks; perform actual argument
   relations and pinned fulfillment on one scoped owner. Required coercion child
   goals must settle before numeric incidence is observed. Keep callee bounds
   as goals, never caller assumptions. An unused ground bound remains unforced.
   The existing direct-incidence metadata is not a complete numeric constraint
   closure; join the shared source constraint component before fallback.
4. Read actual original RootVariables after the canonical response has been
   applied. `GoalEvaluation.goal` is not the answer. Export owned terms only
   after required certainty, residual obligations and external region/opaque
   obligations are handled; UnsupportedType/unknown values cannot imply success.
5. Recheck the exact retained source/attempt/result/slots/cells/state/value/Type
   dependencies before any GuestStore mutation. Resolve all descriptor and
   numeric representability operations first; a rejected second argument must
   not leave the first bound. Consume the response once and use the existing
   argument contracts/materialization and frame continuation. No repeated guest
   arguments, call selection or executable preparation is permitted on retry.

Cold work follows actual source/type/member demand and immutable retained plans;
installed ordinary call execution should read retained decisions and relevant
runtime guards. No global namespace comparison, parser/body copy, second pattern
engine, numeric candidate enumeration or macro-specific path is authorized.
The historical custom numeric/trait algorithms must stay named migration debt
until their actual consumers are replaced and deleted; new helpers alone do not
close that migration.

Required evidence: unchanged owning-edition runtime; ordinary/user/wrapper/std
macro contexts where applicable; repeated identical call site; exact-once argument
effects across real NeedInput; unchanged legitimate suspension; source and result
withdrawal; failed-batch atomicity; cold-branch source-call witness; actual root
entry/cache lineage and residual/external-constraint controls. Source review
against the eight recurring failures must use the final exact fingerprint.
M019/M020 and interpreter/JIT gates remain incomplete.
