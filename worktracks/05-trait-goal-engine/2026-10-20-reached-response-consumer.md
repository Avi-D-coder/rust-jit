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

## Independent commit-path prerequisite review after V534

Read-only review, October 20, before the production solver join lands. Baseline stamps: `engine/run.rs` `1942c42579b40fa8a8ad277926a68bd5de9d90363712eec95c33f481757c19c0`; `engine/run_call_inputs.rs` `059eaed89b5c23aac4d15c8f6ef0e3dfbd1c8c3a09c29a8a168aa0c558dd72c6`; `runtime/cell.rs` `cbe6a83aae11eb550dc230cf482c7f792c9d110da9570ad6855f7fec8177cd65`; `runtime/cell_wide_integer.rs` `6faf900ece091f8c17b3c87a0e6b78d52542b05a5e84c0020e6a992b02158222`; `frontend/call_contracts.rs` `44f6b4b4c7f957266b2043d90ae3d0f76d50032d23f52e082d722424a806345f`; `runtime/shape.rs` `918001dcf13843a6388fd1c2ade3c78b763391f03b965eb8619f44642edc2a00`; `traits/upstream/root_variables.rs` `73a88378cd44a48332e991f6d9200677cd3b38f73691c554c6087d55d5acb1a5`. These stamps describe the inspected prerequisite APIs, not a reviewed or tested production consumer. No source edits, Cargo or isolated checks were made by this pass.

The actual CallSeal result expectation is still computed below the argument bind_literal_fallback loop at this baseline. The lead must factor that existing exact result-slot, selected-hint, ordinary source hint, HintRecipe and frame-bound-view logic once and execute it before packet capture. Supplying captured None or the packet's old expectation back to require_current would not validate the current ordinary result contract. The actual operation/phase/result slot and active pending attempt must authorize the response, not merely the reusable source node.

Existing GuestStore numeric APIs are single-record mutations, not an atomic multi-answer batch. materialize and materialize_ticket reject an incompatible existing shared binding but assign binding immediately after a per-literal preview; materialize_ticket also allocates a result ticket. Calling either sequentially for several solver answers would leave earlier bindings changed if a later answer fails. bind_literal_fallback is the old unconstrained i32/f64 fallback, never an upstream answer consumer. Frame::set_hint is not suitable for reached argument cells: preflight_hint explicitly refuses a slot whose storage already executed.

The reusable pure store path is preview_scalar(cell, Some(expected)) through literal_value and preview_integer/preview_float, or wide_literal_value. It validates actual live storage/ownership/arena data and uses the same full-width numeric record for i128/u128 without allocation. Important qualification: preview_scalar chooses effective_type(value).or(expected), so an already-bound literal can ignore a conflicting proposed expected descriptor. A batch must separately check the expected retained binding and proposed descriptor, then verify the preview's actual type_id. It must preflight all original cells/values/record families and alias conflicts before any assignment. Existing seal_arguments demonstrates the shared validate/reserve/commit ownership pattern but requires effective concrete types already; it cannot substitute for literal binding or solver inference.

Convert an owned original-variable Primitive answer through existing frontend::call_contracts::primitive_runtime_id -> runtime::shape::PrimitiveIdentity::from_recipe. Width alone loses i64/isize and u64/usize identity; do not write another TypeId mapping. RootVariables::resolved(original_key) already deeply resolves the original variable in its retained owner after response application; a GoalEvaluation goal or fresh inference owner is not that answer. Export checked owned Primitive values before scoped IR/owner ends, preserving unresolved/ambiguous/error/external-obligation outcomes.

The engine must join each original numeric key to its retained ArgumentSnapshot slot/cell/state/ValueRepr/effective Type through actual occurrence ordinals, retaining shared record identity. Source NodeId or spelling is not a store handle, and generic Type-parameter keys are not literal binding targets. Duplicate actual numeric records must have one consistent proposed descriptor; validate every alias and every packet/source/factory/root/result/attempt guard before the first write. All potential conversion/allocation/failure work must finish before commit. After that, exclusive store ownership must make assignment completion nonfailing; a fallible validation loop after the first write would not establish failed-batch atomicity.

ReachedCallAttempt Arc currently distinguishes attempts but carries no consumption state. The production pending selection must retain explicit packet/response progress across genuine NeedInput and preserve an applied phase after consuming the response. Dropping the packet to None and allowing another capture would not prove once-per-attempt consumption. Existing argument contract/materialization and call-action/frame continuation remain the ownership/cleanup route after a successful numeric commit.

Eight-mode prerequisite assessment: (1)-(3) only existing ordinary call/store and pinned original-variable APIs are proposed, with no alternate evaluator, primitive enumeration or context-specific numeric rule; (4) origin/source proof must remain common and opaque missing joins explicit; (5) no unrelated executable arm/body check is authorized; (6) capture and immutable source/query plans must be retained across NeedInput rather than reconstructed at retry; (7) currentness must remain exact dependency/attempt scoped rather than world namespace comparison; (8) retain the actual BodyIndex Arc and direct source occurrences without body clone or source search. Final implementation still needs a separate exact-source review. The next acceptance baseline is the unchanged eight-program runtime selection from V534 (five retained runtime passes and three actual failures), with no isolated-control addition or inference of complete interpreter/full-fixture/JIT acceptance.

## Connected production implementation, after V534

M020 / LOCAL-049: the unchanged `tests/ui/enum-discriminant/repr128.rs` fails at its first reached comparison because the ordinary CallSeal defaults the second argument before its declared projection/trait constraints can determine it. V534 is the executable baseline (five other runtime passes, three failures), not an isolated source witness. Extend `RunMachine::CallSeal`, `run_call_inputs::ReachedCallInputs`, the existing CLI compile-input coordinator and the pinned upstream root entry. Capture and retain actual selected argument cells/source views, caller/callee handles and result expectation before fallback; fulfill in upstream, then preflight and commit original numeric record answers through GuestStore before the existing contract/argument/frame path. No assertion identity or macro-specific route participates. Supersede early numeric fallback only where a real numeric call constraint demand exists; no alternate solver, missing-fact-as-negative or fake completeness token.

The coordinator retains a dormant authenticated ImplCatalog request and fulfills only its matching actual Discover demand outside pure queries, using the existing source/compiler/module preparation service and source publication. It retains the owning input tree; no source snapshot or mock world is created. Cold work is immutable catalog/type/query preparation; retries retain the packet and once-per-selection application state. Dispatch uses prepared input/currentness checks, never whole namespace comparisons or body copies. Source, expansion and caller/callee guards run before numeric mutation; ordinary argument ownership/continuation/cleanup remains authoritative. Full unchanged eight-program runtime rerun, exact-source review against all eight modes and actual terminal evidence are required. Unsupported source closure or external constraints remain explicit failures, not compatibility passes.

## Implemented ordinary GuestStore binding batch, source-only handoff

Parent allocated only `runtime/cell.rs` for the M020 reached-call consumer prerequisite; API `GuestStore::bind_numeric_literals(&mut self, &[(CellId, TypeId)]) -> Result<(), Diagnostic>`. Source SHA-256 `62f9ff54466e41cc3157d1f1f7f2ce7381de5d13c2b7a3249ac0eafd92642183` (52 added lines). No test, Cargo or runtime execution was performed for this addition. Actual unchanged repr128 and the retained eight-program V534 batch remain the acceptance witnesses. No isolated controls were added.

Preflight reads each actual live cell with admit_move, verifies an exact existing runtime primitive is in the record's integer/float family, rejects a conflicting existing shared binding, and reuses literal_value plus the existing storage contract. Numeric conversion therefore keeps the common narrow/wide integer and float behavior. In particular it does not add overflowing_literals lint enforcement or a new integer range refusal: existing integer conversion intentionally retains its Rust runtime bit behavior. No descriptor interning or ticket/arena allocation is performed by those previews. All proposed (actual LiteralId, exact TypeId) pairs are buffered before commit; sorting checks conflicting answers for shared records, then deduplication retains one write per real record. No source spelling, inferred width, fabricated cell or new solver semantics participates.

Only after every cell, record, conversion, storage contract and cross-row conflict check succeeds does binding-only commit begin. This uses the already preflighted arena handles under exclusive mutable GuestStore ownership; no callbacks, arena removal/reset or allocation intervene. Each get_mut has an invariant expect rather than a recoverable error path, because only other binding fields change after preflight and cannot invalidate a handle. Thus no recoverable failure or allocation occurs after the first binding write. The method does not move cells, rewrite ValueRepr, create tickets or claim VM action/attempt consumption. Same-type existing bindings are admitted, so the engine must separately retain original snapshot/currentness and enforce once-per-attempt response consumption.

Separate source-review pass compared preflight with existing materialize/materialize_ticket and literal_value, checking float/int family mismatch, nonprimitive descriptor rejection, already bound conflicts, repeated cell/record proposals and stable full-width record identity. All checks precede mutation. Cost is O(answer rows log answer rows) cold response admission with O(answer rows) temporary metadata, then O(unique actual records) binding writes; no global source/namespace/body scan or executable planning. No timing or atomic-failure execution evidence is claimed.

Eight-mode review: (1) no expression evaluator; (2) no macro-only executor or syntax case; (3) this method consumes concrete descriptors and performs storage admission without choosing a primitive/member; (4) source origin never selects storage semantics; (5) no pattern/cold-branch preparation; (6) one batch preflight/commit without retry planning; (7) only supplied actual cells and their records are read, with no namespace census; (8) no BodyIndex/parser/source copying. Superseded fallback and custom legacy semantic routes remain held until actual ordinary solver consumers replace them. The lead owns result expectation factoring, current packet/root guards and once-consumed application; this store component alone does not establish any runtime gain or interpreter/full-fixture/JIT gate.

## Independent live consumer review before combined runtime validation

Read-only source stamp, October 20: `engine/run.rs` `0b09a56415eef86f42dc7964de6d00c28b632ab5a808180a089af6b92037d572`; `engine/run_call_inputs.rs` `8ed92534e96537c3a2cee1037cd7fc3dd566fd41bedf1fcda03103e44b98142f`; `engine/run_trait_calls.rs` `4b14f89567b9392bfdcbd6e9352b2022b7e894d8fd1055dfbee0f6cc8a6b216f`; `traits/upstream/reached_call.rs` `49474cd13d26aeffeee84f94061cd58d57f5fbbd078420dfc1e0cbe4fd6aa7d7`; `cli/mod.rs` `a5b1b21da37879446a6617e46dbd6ab5d4820852993918496c34f61a7ce31595`; store remains `62f9ff54466e41cc3157d1f1f7f2ce7381de5d13c2b7a3249ac0eafd92642183`. No Cargo or guest execution for this source. Active insert-only metadata publication is a separately owned in-flight slice, not covered by this stamp.

Review found and owners corrected three issues before this stamp: the actual CallSeal CallSiteId needed explicit NodeId(site.0) conversion; the first applied bool skipped currentness on a downstream suspended retry; and the first response retained a world digest without its actual source/query guards. Current code uses the same ordinary result expectation computation once ahead of capture and literal fallback, preserving actual result-slot identity, written/source hints and final calling-frame bound view. Capture occurs under the actual queried ordinary CallSeal after selection and typed-plan validation. Both selected-call constructors initialize packet/response/binding retention absent.

A real pending attempt retains its original source packet across NeedInput. A solved response retains SourceFactBundle, caller clauses, verified caller application/environment, caller/callee Arcs and caller descriptor. Response::require_current checks the actual source/query closure and callable/application guards before store commit and every later applied-state retry. Packet postcommit checks retain exact PC, BodyIndex Arc, pending attempt/callee/signature/slots, typed/owner guards, caller source/body/environment/callable, result expectation, cell/state/ValueRepr, and original or exact committed effective Type. The retained immutable committed binding rows replace the bool; retry does not reapply a response or recapture an applied packet. Required Arc allocations and all primitive conversions precede the store batch, whose first write begins only after its own complete validation.

Original numeric keys join actual stored argument snapshots through retained source occurrence ordinals. Primitive answers use the ordinary primitive_runtime_id service. Unresolved original numeric families now return an explicit missing ordinary source constraint closure diagnostic; held, ambiguous or failed solver outcomes never invoke fallback as successful answers. The selected-bound/root driver retains genuine argument Coerce child obligations and performs real pinned fulfillment in one source-admitted inference owner before post-Coerce clause incidence. Canonical child contexts inherit only that active source root lineage; original-variable export stays owned and nonescaping. Nonempty external constraints retain an explicit hold.

The first all-Trait metadata demand census was corrected: potential metadata now follows actual original numeric occurrences in reached actual relations into formal ParamKey occurrences and source Q80 Implemented trait/projection parent metadata. Recursive additions follow actual complete candidate clause bindings, with every candidate retained. Actual post-Coerce SourceRuntimeDemands remains the clause selector; preparation does not solve or reject candidates. Complete whole-Q80 encoding before incidence and unavailable query domains remain explicit holds. This is not the complete future-use source component, and the unresolved export hold must not be reported as that closure.

Eight-mode re-review: (1)-(2) same ordinary source CallPrepare/CallSeal/contracts/frame continuation, no assertion or macro evaluator; (3) only actual pinned relations/fulfillment choose an exported primitive, with existing legacy custom routes still named debt; (4) ordinary indexed original/generated source proofs share capture, opaque missing-node/reference adjustments remain typed gaps; (5) no executable cold branch, body-wide static check or pattern executor is added; (6) packets, source/query inputs and owned responses persist across suspension, though retained query preparation is cold work and full prepare-once/performance closure is not claimed; (7) original source and exact candidate/predicate query guards replace an unproved world digest, without a new world namespace comparison; (8) BodyIndex/parent/query handles remain shared, source occurrences direct. Postcommit packet binding lookup currently scans the retained rows per argument, so its worst-case checking cost is quadratic in that call's arguments; no measured speed or scaling claim is made. All lifetime, failure, suspension, origin/currentness and exact-once requirements still need actual execution evidence.

No further actionable mechanical/currentness issue was found within this bounded stamp. That source conclusion is not a build, unchanged main, full-fixture or interpreter gate. The next authorized validation remains required lib/CLI/digester builds followed by all eight unchanged official programs in the tracked runtime-only selection, preserving all failures and comparing against V534's five runtime-contract passes and three failures. No isolated-control additions or native reference execution are scheduled.

## Lead separate source review for V536 connected candidate

Baseline HEAD `08c177d83dda20a283e06709c92cfffc19eee6be`; aggregate frozen inputs `59632bf13237984efd95beacb81dc793c944814bef444d11bea01d66f16383ab` (8751 inputs). V535 build failed on the actual Arc/index pointer mismatch; none of its eight programs executed. The successor compares the underlying retained index. These source observations do not assert an interpreter pass.

Modes 1–3 (related audit findings MA001, MA003–MA008; audit numbers are separate): the added consumer is the ordinary `RunMachine::CallSeal` route, selecting no macro identity or method-name exception. It calls the actual pinned `poll_reached_call`, not another evaluator/unifier. Root answers bind the original numeric cell records through `GuestStore::bind_numeric_literals`; final contracts/argument packets/body entry remain existing common mechanisms. Existing legacy inference/owner-matching machinery is migration debt; this candidate does not close its removal obligations.

Mode 4 (related audit findings MA004, MA005, MA008): original and attached ordinary generated syntax use the same body/source guards and indexed occurrence transport. Legacy opaque runtime macro source joins remain explicitly held where they cannot supply admitted source evidence; no generated-source bypass or fabricated source range was added. These contexts are unexecuted in this candidate.

Mode 5 (audit MA002 plus the separately numbered cold-branch failure mode): no pattern evaluator changed; preparation reads actual declarations/clauses and demanded metadata, never unselected executable callee bodies. Whole-Q80 preparation can still hold on unsupported source recipe families; complete future-use constraint closure is unfinished. An exported unresolved numeric answer now holds rather than defaulting without that evidence.

Modes 6–8 (the separately numbered repeated-planning, namespace-scan and body-copy failure modes): capture is retained per actual selected attempt, with immutable BodyIndex/callable/plan handles. Genuine NeedInput returns before argument fallback and never repeats evaluated argument effects. The ordinary result expectation calculation was moved intact before capture. Applied state retains exact committed binding rows and the real response/query closure; retries validate the same cells/value/state/source/result/attempt and postbinding descriptors instead of recapturing/reapplying. PreparedSourceCall retains that response after pending selection retirement; preparation completion, repeated prepared CallSeal, CallExit and source return recheck it. No new complete namespace map or parsed-file search occurs in this join. Catalog construction/source metadata closure is cold and demand-triggered; its new publication APIs append authentic immutable rows and refuse conflicting active entries. No performance claim is made.

The same production engine is used for other entry services; an entry missing a required coordinator locator now reports an explicit catalog gap instead of silently selecting an older numeric fallback route. End-to-end integration for those services and full architecture matrices remain incomplete. The primary acceptance selection remains eight unchanged official rustc mains, no isolated controls. Required terminals are pending in V536.

Exact inspected lead/store sources:

- `crates/lrr/src/engine/run.rs` `d23bbf3ba343f865a4f6b2c92df7a0b6863263c8bca12729c207614f85f7c92e`
- `crates/lrr/src/engine/run_call_inputs.rs` `8ed92534e96537c3a2cee1037cd7fc3dd566fd41bedf1fcda03103e44b98142f`
- `crates/lrr/src/engine/run_trait_calls.rs` `4b14f89567b9392bfdcbd6e9352b2022b7e894d8fd1055dfbee0f6cc8a6b216f`
- `crates/lrr/src/engine/run_value_calls.rs` `ca42ad953f6b262793839f69e7ce6b1205625cf0e1e25af9548264b7e0d7ea24`
- `crates/lrr/src/cli/mod.rs` `a5b1b21da37879446a6617e46dbd6ab5d4820852993918496c34f61a7ce31595`
- `crates/lrr/src/cli/proc_macros.rs` `cb565211225cd2c6a1cb0f42f8898c1093cfc8c09f85353d643faadd808a07e1`
- `crates/lrr/src/runtime/cell.rs` `62f9ff54466e41cc3157d1f1f7f2ce7381de5d13c2b7a3249ac0eafd92642183`
