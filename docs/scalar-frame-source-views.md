# T032 scalar frame source views — work in progress

The scalar interpreter carries owned source terms separately from runtime
layout descriptors. Accepted callee parameters retain their declaration's rigid
parameter keys. Parameter reads, inferred locals, declared generic locals,
result transfers and break values preserve those keys. Fresh primitive-producing
operations clear contextual source metadata; their source view comes from the
actual primitive descriptor when a call argument is captured.

Explicit caller type parameters resolve through the caller's declaration scope
and accepted runtime substitution. Call-start facts preserve both the selected
argument's contextual source term and its runtime hint, including nested calls.
Written argument evaluation still determines the actual source term: a contextual
hint cannot change a different parameter into the expected parameter. Returning a
rigid parameter different from the declared result rejects even when their runtime
descriptors are equal. Call results translate the callee's declared result through
the accepted source substitution before resuming the caller.

The regression controls execute explicit and inferred forwarding through
parameters, initialized and deferred locals, and nested calls. Negative controls
exercise two distinct rigid parameters instantiated with the same runtime type,
explicit primitive substitution against a rigid argument, and a mismatched rigid
return. These controls use actual interpreted values and typed faults.

Frame admission and retained call-stack accounting include the actual sizes of
slot, local, and parameter source metadata under checked byte limits.

Only primitive and rigid parameter terms are stored by this component. It does
not supply the full SourceViewId/ParamEnvId tables, composite layouts, general
source checking of assignment, trait dictionaries, coercions, member dispatch,
generated suspension or native/JIT integration. T032 and release acceptance remain
in progress. Linux acceptance remains pending.
