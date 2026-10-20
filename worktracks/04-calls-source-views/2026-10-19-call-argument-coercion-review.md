# T053 / M018 / M025 bound call coercion — separate source review

Baseline c48e7a271700f14ea746fc52b59ce083d7f1c8b3. V423 source fingerprint
65ead2612c91af3aece126ca0738e87281633024ec2410f6ddf74189de65b198.
Source review, pending terminal runtime acceptance below. V422's bad include
path is a recorded build failure, not passing evidence.

Inspected run.rs call-start hint installation and evaluated ArgumentType
collection, run_argument_hints::apply_context, frame_source_storage::
argument_source_view, call_contracts::prepare_contracts and bind, runtime
cell_pointers::{pointer_coercion,pointer_type_coercion,callable_identity,
pointer_family_interchange}, cell_unsizing::unsizing_coercion,
cell.rs::seal_arguments, vm::call::ArgumentPacket::seal. Q47/Q73 alias/type
preparation and guarded selected call admission remain unchanged.

The one changed contract loop, before its existing Param branch, presents a
coerced ArgumentType only for an already-bound actual callee ParamKey with
both retained source and runtime bindings. The descriptor predicate accepts
existing pointer conversions; the source is the selected binding, not a
nominal reconstructed from display text. Existing symbolic/runtime/source
binding checks still run. Unbound inference stays on its original route.
Pointer-family interchange already exists in storage contracts; explicit
source/descriptor binding is retained rather than replaced with an erased
fn-pointer descriptor. The original cell is still sealed and converted by the
existing transactional store route. No move, allocation, effect or call occurs
inside the pure descriptor admission. Capturing closures are refused by the
same captures-empty predicate used by physical fn-item conversion; ordinary
callable identity and dispatcher lookup remain source-identity based.

Eight recurring modes (distinct from MA audit IDs):
1. No assertion/expression compiler or evaluator. Only common call contracts
and the existing pointer coercion predicate changed.
2. No generated literal/break execution or context-only opcode. Generated
source joins ordinary indexed calls; full matrix execution is pending.
3. No method whitelist/numeric inference/macro identity rule. This removes a
bound-generic coercion discrepancy. MA003's unrelated historic numeric debt is
not closed. Source equality in Unifier is untouched.
4. No original-only/generated-source refusal or authority relaxation. Parameter
owner identity, source guards and actual descriptors remain required.
5. No new pattern engine or eager unselected-arm/callee body preparation. The
operation acts only on evaluated argument metadata and selected bindings.
6. No repeated parsing/planning or query fetch during installed operations.
Existing cold signature/type preparation is unchanged; per argument the added
work is bounded descriptor lookups and a source-term clone only if converted,
followed by the existing symbolic checks. No timing/scaling claim.
7. No namespace-map construction, comparison or global invalidation added.
Actual callee guards/source dependency checks remain on the existing route.
8. No parser/BodyIndex deep copy or whole-file node search. The small retained
ArgumentType is per reached boundary and is not a second executable body.

MA002 shared-pattern and MA006 planning audit obligations are not closed by
this call coercion change. No resource accounting, count/time/byte caps, static
checker, sandbox, unsafe, native backend or new trait/inference algorithm.
Concrete signature, raw-pointer/unsizing, trait-object and reference-certificate
paths remain existing debt, with no claim of broad compatibility from this fix.
Acceptance requires actual frozen-source original runtime captures, origin
matrix and relevant retained controls; matching source review alone is not a
pass or completion of T053/M018/M025/interpreter/JIT.

V423 terminal: both builds and all7 selected controls pass. Maintained upstream
harness accepts the unchanged original in its owning2015 edition; supplemental
2018/2021/2024 captures complete with17dispatches/outcomes,1source boundary,
unit/empty guest streams/cleanup and no host stderr. The after-runtime guard was
taken before host controls finished; the separate after-controls guard is the
actual final closing check and also matches. Do not use the early check as the
final control closure. The initial selector typo ran zero tests and is preserved.

V424 changes only the new host control to inspect ordinary CallPrepare(22),
CallSeal(19), CheckParameters(20) counters in each of16edition/origin runs.
This closes the gap between output equality and executed ordinary call evidence.
Fingerprint ff8a3acfac0b60f4e7e1020c76d6077996ec42234515ddd38c93ffa043fd3bb0. Production is unchanged
fromV423; its runtime receipts stay attached toV423 and are not relabeled as
V424 executions. Final strengthened control evidence is pending.

V424 terminal: lib build0; strengthened route control0; all16edition/origin
rows have ordinary prepare/seal/parameter counters1/1/1. Source/HEAD/binary
closing guards match. This plus the existing V423 Q47/Q73 actual indexed-node
source/guard control establishes the scoped common route, not broader feature
closure. V423 both builds and7controls/runtime4editions remain their own receipts.
One new distinct upstream runtime-contract case; full card and gates stay held.
