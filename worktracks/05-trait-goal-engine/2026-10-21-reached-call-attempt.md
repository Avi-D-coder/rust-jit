# M020 / LOCAL-049 and shared LOCAL-044: reached-call attempt identity

Before code: retained `ReachedCallInputs::require_current` compares PC/source,
callee, slots/cells/values and GenericEnv handles, but a loop can revisit the
same PC using the same cached callee and cells. Those comparisons do not prove
that the current pending call is the original inference attempt. The packet
also needs the current expected result source and descriptor at consumption.
The unchanged `repr128.rs` repeated `discr` calls are the runtime target; actual
repeated-site/suspension/invalidation controls remain required outside originals.

Extend the common `SelectedArgumentCall` pending selection with a private lazy
Arc identity, captured by its one source-input owner. Create no token for legacy
calls that never request a packet. Pending-call cloning/suspension shares that
token; a fresh selection starts without it. Source capture retains it; recheck
requires pointer identity in the current pending selection. No counter, limit,
name dispatch, evaluator, parsing or whole-body check is involved. The packet is
owned and not Clone. Expected result revalidation compares both source Type and
runtime descriptor using the existing ordinary call hint, not a newly inferred
or macro-specific expectation. Input inference and solver admission remain
separate from this identity check.

Shared source/call entry points: RunMachine::finish_selected_argument_call,
run_trait_calls::prepare_selected_argument_call, the common CallSeal pending map,
capture_reached_call_inputs and ReachedCallInputs::require_current. Original and
generated calls use these same selected-call records. Engine/VM source is shared
under the existing lead reservation; upstream fact/fulfillment workers own
separate files. Lazy token capture costs O(1) only when this consumer is requested;
currentness already checks actual argument snapshots. No new namespace scan,
BodyIndex clone or source query is added to execution.

Required closure: direct macOS build; actual repeated call-site fresh-attempt
refusal, unchanged legitimate suspension/resume, source/result withdrawal and
exact-once effects controls; then actual admitted upstream response consumption
and unchanged fixture runtime. No source guard is an upstream root certificate.
The root and runtime consumer remain inactive and no acceptance is claimed by
this bounded prerequisite.
